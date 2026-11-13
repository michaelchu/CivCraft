//! Passive runtime instrumentation. No callbacks or gameplay decisions live here.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Phase {
    #[default]
    Initial,
    ScenarioActions,
    EndActivities,
    CityProcessing,
    WorldEvents,
    UnitActivities,
    AiPlanning,
    TurnEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub(crate) enum Entity {
    City(u32),
    Unit(u32),
    Civilization(usize),
}

pub(crate) trait Payload: Serialize {
    const KIND: &'static str;
    const VERSION: u16 = 1;
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Event {
    pub sequence: u64,
    pub turn: i32,
    pub phase: Phase,
    pub source: String,
    pub kind: String,
    pub schema_version: u16,
    pub entities: Vec<Entity>,
    pub cause: Option<u64>,
    pub payload: serde_json::Value,
}

impl Event {
    pub fn decode<'a, P: Payload + serde::Deserialize<'a>>(&'a self) -> Result<P, String> {
        if self.kind != P::KIND || self.schema_version != P::VERSION {
            return Err("event kind/schema does not match requested payload".into());
        }
        P::deserialize(&self.payload).map_err(|e| e.to_string())
    }
}

#[derive(Default)]
pub(crate) struct Collector {
    enabled: bool,
    kinds: BTreeSet<String>,
    capacity: usize,
    next_sequence: u64,
    dropped: u64,
    phase: Phase,
    pending: Vec<Event>,
}

// Simulation clones are used for speculation. They must not share or duplicate traces.
impl Clone for Collector {
    fn clone(&self) -> Self {
        Self::default()
    }
}
impl Collector {
    pub fn enabled(kinds: BTreeSet<String>, capacity: usize) -> Self {
        Self {
            enabled: true,
            kinds,
            capacity,
            next_sequence: 1,
            ..Self::default()
        }
    }
    pub fn wants<P: Payload>(&self) -> bool {
        self.enabled
            && (self.kinds.contains("*")
                || self.kinds.contains(P::KIND)
                || self.kinds.contains(&format!(
                    "{}.*",
                    P::KIND.split('.').next().unwrap_or(P::KIND)
                )))
    }
    /// Payload construction is lazy. Return the sequence only when retained.
    pub fn emit<P: Payload>(
        &mut self,
        turn: i32,
        entities: impl FnOnce() -> Vec<Entity>,
        payload: impl FnOnce() -> P,
    ) -> Option<u64> {
        self.emit_caused(turn, None, entities, payload)
    }
    pub fn emit_caused<P: Payload>(
        &mut self,
        turn: i32,
        cause: Option<u64>,
        entities: impl FnOnce() -> Vec<Entity>,
        payload: impl FnOnce() -> P,
    ) -> Option<u64> {
        if !self.wants::<P>() {
            return None;
        }
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        if self.pending.len() >= self.capacity {
            self.dropped += 1;
            return None;
        }
        self.pending.push(Event {
            sequence,
            turn,
            phase: self.phase,
            source: P::KIND.split('.').next().unwrap_or(P::KIND).to_owned(),
            kind: P::KIND.into(),
            schema_version: P::VERSION,
            entities: entities(),
            cause,
            payload: serde_json::to_value(payload()).expect("typed trace payload must serialize"),
        });
        Some(sequence)
    }
    pub fn phase(&mut self, phase: Phase) {
        self.phase = phase;
    }
    pub fn drain(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.pending)
    }
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Serialize)]
    struct Example;
    impl Payload for Example {
        const KIND: &'static str = "example.observed";
    }
    #[test]
    fn disabled_and_full_collectors_do_not_build_payloads() {
        let mut collector = Collector::default();
        collector.emit::<Example>(1, || panic!("disabled"), || panic!("disabled"));
        let mut collector = Collector::enabled(BTreeSet::from(["*".to_owned()]), 0);
        collector.emit::<Example>(1, || panic!("full"), || panic!("full"));
        assert_eq!(collector.dropped(), 1);
        assert!(collector.clone().drain().is_empty());
        assert!(!collector.clone().enabled);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::{
        random::FreecivRng,
        sim::Simulation,
        world::{GeneratedMap, GeneratedTile, Terrain, TerrainBase, topology::MapWrap},
    };
    #[derive(Serialize)]
    struct Example {
        value: i32,
    }
    impl Payload for Example {
        const KIND: &'static str = "example.changed";
    }
    #[test]
    fn subscriptions_order_causes_and_overflow_are_observable() {
        let mut collector = Collector::enabled(BTreeSet::from(["example.*".into()]), 2);
        collector.phase(Phase::CityProcessing);
        let first = collector.emit(1, || vec![Entity::City(1)], || Example { value: 1 });
        collector.emit_caused(1, first, || vec![Entity::City(1)], || Example { value: 2 });
        collector.emit(1, Vec::new, || Example { value: 3 });
        let events = collector.drain();
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].sequence, 2);
        assert_eq!(events[1].cause, Some(1));
        assert_eq!(events[0].phase, Phase::CityProcessing);
        assert_eq!(collector.dropped(), 1);
    }
    #[test]
    fn tracing_and_observation_leave_campaign_and_random_state_unchanged() {
        // Two civilizations on a small, deterministic world exercise the real
        // turn orchestrator without a 3,600-tile campaign's AI search cost.
        let map_wrap = MapWrap::with_dimensions(16, 16).unwrap();
        let map = GeneratedMap {
            seed: 42,
            map_seed: 42,
            map_wrap,
            game_random: FreecivRng::new(42),
            tiles: vec![
                GeneratedTile {
                    terrain: Terrain::Grassland,
                    base_terrain: TerrainBase::Grassland,
                    altitude: 100,
                    hut: false,
                    river: false,
                    river_edges: 0,
                    resource: None,
                };
                map_wrap.tile_count()
            ],
            starts: vec![(3, 4), (11, 12)],
            civilization_count: 2,
        };
        let mut plain = Simulation::new(map);
        plain.enable_all_civilizations_as_ai();
        let mut traced = plain.clone();
        traced.trace = Collector::enabled(BTreeSet::from(["*".into()]), 10000);
        let mut event_count = 0;
        let mut event_kinds = BTreeSet::new();
        for _ in 0..3 {
            plain.simulate_turn();
            traced.simulate_turn();
            let _ = traced.observe_cities();
            let _ = traced.observe_units();
            let _ = traced.observe_civilizations();
            assert_eq!(
                bincode::serialize(&plain).unwrap(),
                bincode::serialize(&traced).unwrap()
            );
            assert_eq!(
                bincode::serialize(&plain.native_random).unwrap(),
                bincode::serialize(&traced.native_random).unwrap()
            );
            let events = traced.trace.drain();
            event_count += events.len();
            event_kinds.extend(events.into_iter().map(|event| event.kind));
        }
        assert!(event_count > 0);
        assert!(event_kinds.contains("settlement.city_founded"));
        assert!(event_kinds.contains("cities.growth_applied"));
        let bytes = bincode::serialize(&traced).unwrap();
        let mut loaded: Simulation = bincode::deserialize(&bytes).unwrap();
        assert!(!loaded.trace.wants::<Example>());
        assert!(loaded.trace.drain().is_empty());
        let mut cloned = traced.clone();
        assert!(!cloned.trace.wants::<Example>());
        assert!(cloned.trace.drain().is_empty());
    }
}
