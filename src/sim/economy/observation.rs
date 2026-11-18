//! Important financial losses; recording never changes debt resolution.
use super::*;
use serde::Serialize;

#[derive(Serialize)]
struct UpkeepFailure {
    owner: usize,
    action: &'static str,
    asset: Option<trace::Entity>,
    item: Option<&'static str>,
    gold_before: i32,
    gold_after: i32,
}
impl trace::Payload for UpkeepFailure {
    const KIND: &'static str = "economy.upkeep_shortfall";
}

impl Simulation {
    pub(in crate::sim) fn trace_upkeep_shortfall(
        &mut self,
        owner: usize,
        action: &'static str,
        asset: Option<trace::Entity>,
        item: Option<&'static str>,
        gold_before: i32,
    ) {
        let gold_after = self.players[owner].gold;
        self.trace.emit(
            self.turn,
            || {
                std::iter::once(trace::Entity::Civilization(owner))
                    .chain(asset)
                    .collect()
            },
            || UpkeepFailure {
                owner,
                action,
                asset,
                item,
                gold_before,
                gold_after,
            },
        );
    }
}
