//! Automated gameplay scenarios covering regression behavior.

use super::*;

#[path = "regression_scenarios/diplomacy.rs"]
mod diplomacy;
#[path = "regression_scenarios/early.rs"]
mod early;
#[path = "regression_scenarios/military.rs"]
mod military;
#[path = "regression_scenarios/native_rules.rs"]
mod native_rules;
#[path = "regression_scenarios/research.rs"]
mod research;
#[path = "regression_scenarios/strategy.rs"]
mod strategy;

impl Playtest {
    pub(super) fn advance_regression_scenarios(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
    ) {
        self.advance_early_scenarios(frame, game, buttons);
        self.advance_military_scenarios(frame, game, buttons);
        self.advance_diplomacy_scenarios(frame);
        self.advance_strategy_scenarios(frame);
        self.advance_native_rule_scenarios(frame, game);
        self.advance_research_scenarios(frame, game, buttons);
    }
}
