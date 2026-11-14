//! Passive runtime instrumentation. No callbacks or gameplay decisions live here.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use std::{cell::RefCell, rc::Rc};

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

#[derive(Default)]
pub(crate) struct Collector {
    enabled: bool,
    kinds: BTreeSet<String>,
    capacity: usize,
    next_sequence: u64,
    dropped: u64,
    phase: Phase,
    pending: Vec<Event>,
    profile_enabled: bool,
    profile_turn: Option<TurnProfileInProgress>,
    profiles: Vec<TurnProfile>,
    operations: RefCell<BTreeMap<&'static str, u64>>,
    span_recorder: Rc<RefCell<SpanRecorder>>,
}

struct TurnProfileInProgress {
    turn: i32,
    city_count: usize,
    unit_count: usize,
    started: Instant,
    phase_started: Instant,
    phase: Phase,
    phases: Vec<PhaseProfile>,
}

#[derive(Default)]
struct SpanRecorder {
    enabled: bool,
    active: Vec<ActiveSpan>,
    completed: BTreeMap<&'static str, SpanAggregate>,
}

struct ActiveSpan {
    id: u64,
    name: &'static str,
    started: Instant,
    child_elapsed: std::time::Duration,
}

#[derive(Clone, Copy, Default)]
struct SpanAggregate {
    calls: u64,
    inclusive_seconds: f64,
    exclusive_seconds: f64,
}

#[derive(Clone)]
pub(crate) struct ProfileHandle(Rc<RefCell<SpanRecorder>>);

pub(crate) struct ProfileSpan {
    recorder: Rc<RefCell<SpanRecorder>>,
    id: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PhaseProfile {
    pub phase: Phase,
    pub elapsed_seconds: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TurnProfile {
    pub turn: i32,
    pub city_count: usize,
    pub unit_count: usize,
    pub elapsed_seconds: f64,
    pub phases: Vec<PhaseProfile>,
    pub spans: Vec<SpanProfile>,
    pub operations: BTreeMap<&'static str, u64>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SpanProfile {
    pub name: &'static str,
    pub calls: u64,
    pub inclusive_seconds: f64,
    pub exclusive_seconds: f64,
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
    pub fn enable_profiling(&mut self) {
        self.profile_enabled = true;
        self.span_recorder.borrow_mut().enabled = true;
    }
    /// A cheap cloned handle lets profiling scopes coexist with mutable access
    /// to the simulation while a module performs its work.
    pub fn profiler(&self) -> ProfileHandle {
        ProfileHandle(self.span_recorder.clone())
    }
    pub fn begin_turn_profile(&mut self, turn: i32, city_count: usize, unit_count: usize) {
        if !self.profile_enabled {
            return;
        }
        let now = Instant::now();
        self.operations.get_mut().clear();
        {
            let mut recorder = self.span_recorder.borrow_mut();
            recorder.active.clear();
            recorder.completed.clear();
        }
        self.profile_turn = Some(TurnProfileInProgress {
            turn,
            city_count,
            unit_count,
            started: now,
            phase_started: now,
            phase: self.phase,
            phases: Vec::new(),
        });
    }
    pub fn count_operation(&self, operation: &'static str, count: u64) {
        if !self.profile_enabled {
            return;
        }
        *self.operations.borrow_mut().entry(operation).or_default() += count;
    }
    pub fn finish_turn_profile(&mut self) {
        let Some(mut current) = self.profile_turn.take() else {
            return;
        };
        debug_assert!(
            self.span_recorder.borrow().active.is_empty(),
            "all profiling scopes must close before the turn profile is finished"
        );
        let now = Instant::now();
        current.phases.push(PhaseProfile {
            phase: current.phase,
            elapsed_seconds: now.duration_since(current.phase_started).as_secs_f64(),
        });
        let spans = std::mem::take(&mut self.span_recorder.borrow_mut().completed)
            .into_iter()
            .map(|(name, aggregate)| SpanProfile {
                name,
                calls: aggregate.calls,
                inclusive_seconds: aggregate.inclusive_seconds,
                exclusive_seconds: aggregate.exclusive_seconds,
            })
            .collect();
        self.profiles.push(TurnProfile {
            turn: current.turn,
            city_count: current.city_count,
            unit_count: current.unit_count,
            elapsed_seconds: now.duration_since(current.started).as_secs_f64(),
            phases: current.phases,
            spans,
            operations: std::mem::take(self.operations.get_mut()),
        });
    }
    pub fn drain_profiles(&mut self) -> Vec<TurnProfile> {
        std::mem::take(&mut self.profiles)
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
        if let Some(current) = &mut self.profile_turn {
            let now = Instant::now();
            current.phases.push(PhaseProfile {
                phase: current.phase,
                elapsed_seconds: now.duration_since(current.phase_started).as_secs_f64(),
            });
            current.phase_started = now;
            current.phase = phase;
        }
        self.phase = phase;
    }
    pub fn drain(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.pending)
    }
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

impl ProfileHandle {
    /// Start a named timing scope. Scopes nest and aggregate by stable name for
    /// the current turn; when profiling is disabled this returns immediately.
    pub fn scope(&self, name: &'static str) -> Option<ProfileSpan> {
        let mut recorder = self.0.borrow_mut();
        if !recorder.enabled {
            return None;
        }
        let id = recorder.active.last().map_or(1, |span| span.id + 1);
        recorder.active.push(ActiveSpan {
            id,
            name,
            started: Instant::now(),
            child_elapsed: std::time::Duration::ZERO,
        });
        Some(ProfileSpan {
            recorder: self.0.clone(),
            id,
        })
    }
}

impl Drop for ProfileSpan {
    fn drop(&mut self) {
        let now = Instant::now();
        let mut recorder = self.recorder.borrow_mut();
        let Some(active) = recorder.active.pop() else {
            return;
        };
        debug_assert_eq!(
            active.id, self.id,
            "profiling scopes must drop in stack order"
        );
        let elapsed = now.duration_since(active.started);
        let aggregate = recorder.completed.entry(active.name).or_default();
        aggregate.calls += 1;
        aggregate.inclusive_seconds += elapsed.as_secs_f64();
        aggregate.exclusive_seconds += elapsed.saturating_sub(active.child_elapsed).as_secs_f64();
        if let Some(parent) = recorder.active.last_mut() {
            parent.child_elapsed += elapsed;
        }
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
        let mut profiled = plain.clone();
        profiled.trace.enable_profiling();
        let mut event_count = 0;
        let mut event_kinds = BTreeSet::new();
        for _ in 0..3 {
            plain.simulate_turn();
            traced.simulate_turn();
            profiled.simulate_turn();
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
            assert_eq!(
                bincode::serialize(&plain).unwrap(),
                bincode::serialize(&profiled).unwrap()
            );
            assert_eq!(
                bincode::serialize(&plain.native_random).unwrap(),
                bincode::serialize(&profiled.native_random).unwrap()
            );
            let events = traced.trace.drain();
            event_count += events.len();
            event_kinds.extend(events.into_iter().map(|event| event.kind));
        }
        assert!(event_count > 0);
        assert!(event_kinds.contains("settlement.city_founded"));
        assert!(event_kinds.contains("cities.growth_applied"));
        assert_eq!(profiled.trace.drain_profiles().len(), 3);
        let bytes = bincode::serialize(&traced).unwrap();
        let mut loaded: Simulation = bincode::deserialize(&bytes).unwrap();
        assert!(!loaded.trace.wants::<Example>());
        assert!(loaded.trace.drain().is_empty());
        let mut cloned = traced.clone();
        assert!(!cloned.trace.wants::<Example>());
        assert!(cloned.trace.drain().is_empty());
    }

    #[test]
    fn opt_in_turn_profiles_record_phases_scopes_entities_and_operations() {
        let mut collector = Collector::default();
        collector.enable_profiling();
        collector.begin_turn_profile(7, 3, 11);
        collector.phase(Phase::EndActivities);
        let profiler = collector.profiler();
        {
            let _scope = profiler.scope("workers");
            std::thread::sleep(std::time::Duration::from_millis(1));
            let _nested = profiler.scope("workers.pathfinding");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        collector.count_operation("routes", 4);
        collector.phase(Phase::CityProcessing);
        collector.finish_turn_profile();
        let profiles = collector.drain_profiles();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].turn, 7);
        assert_eq!((profiles[0].city_count, profiles[0].unit_count), (3, 11));
        assert!(
            profiles[0]
                .phases
                .iter()
                .any(|phase| phase.phase == Phase::EndActivities)
        );
        let workers = profiles[0]
            .spans
            .iter()
            .find(|span| span.name == "workers")
            .unwrap();
        let nested = profiles[0]
            .spans
            .iter()
            .find(|span| span.name == "workers.pathfinding")
            .unwrap();
        assert_eq!(workers.calls, 1);
        assert!(workers.inclusive_seconds >= workers.exclusive_seconds);
        assert!(workers.inclusive_seconds >= nested.inclusive_seconds);
        assert_eq!(nested.calls, 1);
        assert_eq!(profiles[0].operations["routes"], 4);
    }

    #[test]
    fn profiling_scopes_are_absent_when_profiling_is_disabled() {
        let collector = Collector::default();
        assert!(collector.profiler().scope("unused.operation").is_none());
    }
}
