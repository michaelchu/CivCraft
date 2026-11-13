//! Read-only provider registry, independent from analyzer registration.
use crate::sim::{Simulation, observation::Snapshot, trace::Phase};
use std::collections::BTreeSet;

pub(crate) trait SnapshotProvider {
    fn id(&self) -> &'static str;
    fn capture(&self, simulation: &Simulation, snapshot: &mut Snapshot);
}
struct Cities;
impl SnapshotProvider for Cities {
    fn id(&self) -> &'static str {
        "cities"
    }
    fn capture(&self, game: &Simulation, snapshot: &mut Snapshot) {
        snapshot.insert(self.id(), game.observe_cities());
    }
}
struct Civilizations;
impl SnapshotProvider for Civilizations {
    fn id(&self) -> &'static str {
        "civilizations"
    }
    fn capture(&self, game: &Simulation, snapshot: &mut Snapshot) {
        snapshot.insert(self.id(), game.observe_civilizations());
    }
}
struct Units;
impl SnapshotProvider for Units {
    fn id(&self) -> &'static str {
        "units"
    }
    fn capture(&self, game: &Simulation, snapshot: &mut Snapshot) {
        snapshot.insert(self.id(), game.observe_units());
    }
}
pub(crate) const IDS: &[&str] = &["cities", "civilizations", "units"];
pub(crate) fn build(ids: &BTreeSet<String>) -> Result<Vec<Box<dyn SnapshotProvider>>, String> {
    ids.iter()
        .map(|id| -> Result<Box<dyn SnapshotProvider>, String> {
            Ok(match id.as_str() {
                "cities" => Box::new(Cities),
                "civilizations" => Box::new(Civilizations),
                "units" => Box::new(Units),
                _ => return Err(format!("unknown snapshot provider {id}")),
            })
        })
        .collect()
}
pub(crate) fn capture(
    game: &Simulation,
    providers: &[Box<dyn SnapshotProvider>],
    phase: Phase,
) -> Snapshot {
    let mut snapshot = Snapshot::new(game.turn(), phase, game.cities().len());
    for provider in providers {
        provider.capture(game, &mut snapshot);
    }
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    #[derive(Serialize, PartialEq, Debug)]
    struct Custom {
        count: u32,
    }
    #[test]
    fn arbitrary_typed_domains_need_no_snapshot_schema_edit() {
        let mut snapshot = Snapshot::new(1, Phase::Initial, 0);
        snapshot.insert("custom", Custom { count: 7 });
        assert_eq!(snapshot.get::<Custom>("custom"), Some(&Custom { count: 7 }));
        assert_eq!(
            serde_json::to_value(&snapshot).unwrap()["custom"]["count"],
            7
        );
        assert!(snapshot.get::<u32>("custom").is_none());
    }
}
