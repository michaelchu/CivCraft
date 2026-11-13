use crate::analytics::{AnalysisOutput, Analyzer, Descriptor};
use crate::sim::{observation::Snapshot, trace::Entity};
use serde_json::json;
pub(crate) struct Settlement {
    every: i32,
}
impl Settlement {
    pub fn new(every: i32) -> Self {
        Self { every }
    }
}
impl Analyzer for Settlement {
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: "settlement",
            version: 1,
            events: vec![],
            providers: vec!["civilizations", "units"],
            configuration: json!({"check_every_turns":self.every,"first_check_turn":11}),
        }
    }
    fn on_boundary(&mut self, snapshot: &Snapshot, out: &mut AnalysisOutput) {
        if snapshot.turn < 11 || (snapshot.turn - 11) % self.every != 0 {
            return;
        }
        let Some(players) = snapshot.civilizations() else {
            return;
        };
        for player in players {
            out.check(format!("civilization:{}",player.id), "civilization_without_cities_candidate",
                vec![Entity::Civilization(player.id)], player.alive && player.cities == 0,
                "An active civilization has no cities at a periodic check; inspect its remaining units and settlement activity.",
                json!({"owner":player.id,"name":player.name,"units":player.units,"unit_details":snapshot.units().map(|units|units.iter().filter(|u|u.owner==player.id).collect::<Vec<_>>())}));
        }
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
