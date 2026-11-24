use crate::analytics::{AnalysisOutput, Analyzer, Descriptor};
use crate::sim::{observation::Snapshot, trace::Entity};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Default)]
struct ExpansionCounts {
    settlers_produced: u64,
    cities_founded: u64,
    site_searches: u64,
    expanded_tiles: u64,
    viable_candidates: u64,
    searches_with_candidate: u64,
    selected_sites: u64,
}

pub(crate) struct Settlement {
    every: i32,
    expansion: BTreeMap<usize, ExpansionCounts>,
}
impl Settlement {
    pub fn new(every: i32) -> Self {
        Self {
            every,
            expansion: BTreeMap::new(),
        }
    }
}
impl Analyzer for Settlement {
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: "settlement",
            version: 2,
            events: vec![
                "production.completed",
                "settlement.city_founded",
                "settlement.site_selected",
                "settlement.site_search_completed",
            ],
            providers: vec!["civilizations", "units"],
            configuration: json!({"check_every_turns":self.every,"first_check_turn":11}),
        }
    }
    fn on_event(&mut self, event: &crate::sim::trace::Event, _: &mut AnalysisOutput) {
        let Some(owner) = event.entities.iter().find_map(|entity| match entity {
            Entity::Civilization(id) => Some(*id),
            _ => None,
        }) else {
            return;
        };
        let counts = self.expansion.entry(owner).or_default();
        match event.kind.as_str() {
            "production.completed"
                if event.payload.get("item").and_then(|item| item.as_str()) == Some("Settler") =>
            {
                counts.settlers_produced += 1;
            }
            "settlement.city_founded" => counts.cities_founded += 1,
            "settlement.site_selected" => counts.selected_sites += 1,
            "settlement.site_search_completed" => {
                counts.site_searches += 1;
                let candidates = event
                    .payload
                    .get("viable_candidates")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0);
                counts.viable_candidates += candidates;
                counts.searches_with_candidate += u64::from(candidates > 0);
                counts.expanded_tiles += event
                    .payload
                    .get("expanded_tiles")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0);
            }
            _ => {}
        }
    }
    fn on_boundary(&mut self, snapshot: &Snapshot, out: &mut AnalysisOutput) {
        let Some(players) = snapshot.civilizations() else {
            return;
        };
        let units = snapshot.units();
        let mut expansion = Vec::new();
        for player in players {
            let counts = self.expansion.entry(player.id).or_default();
            let active_settlers = units.map_or(0, |units| {
                units
                    .iter()
                    .filter(|unit| unit.owner == player.id && unit.can_found)
                    .count()
            });
            expansion.push(json!({
                "civilization": player.id,
                "cities": player.cities,
                "settlers_produced": counts.settlers_produced,
                "cities_founded": counts.cities_founded,
                "active_settlers": active_settlers,
                "site_searches": counts.site_searches,
                "site_search_expanded_tiles": counts.expanded_tiles,
                "viable_candidates_across_searches": counts.viable_candidates,
                "site_searches_with_candidate": counts.searches_with_candidate,
                "selected_sites": counts.selected_sites,
            }));
            if snapshot.turn < 11 || (snapshot.turn - 11) % self.every != 0 {
                continue;
            }
            out.check(format!("civilization:{}",player.id), "civilization_without_cities_candidate",
                vec![Entity::Civilization(player.id)], player.alive && player.cities == 0,
                "An active civilization has no cities at a periodic check; inspect its remaining units and settlement activity.",
                json!({"owner":player.id,"name":player.name,"units":player.units,"unit_details":snapshot.units().map(|units|units.iter().filter(|u|u.owner==player.id).collect::<Vec<_>>())}));
        }
        out.metric("expansion_by_civilization", expansion);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{observation::CivilizationSnapshot, trace::Phase};
    #[test]
    fn checks_keep_original_periodic_schedule_and_do_not_resolve_missing_data() {
        let mut analyzer = Settlement::new(10);
        let mut out = AnalysisOutput::default();
        for (turn, cities, observed) in [
            (10, 0, true),
            (11, 0, true),
            (12, 1, true),
            (21, 1, false),
            (31, 1, true),
        ] {
            let mut snapshot = Snapshot::new(turn, Phase::TurnEnd, cities);
            if observed {
                snapshot.insert(
                    "civilizations",
                    vec![CivilizationSnapshot {
                        id: 0,
                        name: "Test".into(),
                        alive: true,
                        cities,
                        units: 1,
                        gold: 0,
                        score: 0,
                        culture: 0,
                        culture_per_turn: 0,
                        strategy: None,
                    }],
                );
            }
            out.context("settlement", turn, Some(turn), None);
            analyzer.on_boundary(&snapshot, &mut out);
            if turn == 21 {
                assert_eq!(out.findings[0].status, "open");
            }
        }
        assert_eq!(out.findings.len(), 1);
        assert_eq!(out.findings[0].first_observed_turn, 11);
        assert_eq!(out.findings[0].resolved_turn, Some(31));
    }
}
