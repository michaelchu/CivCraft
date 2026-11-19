//! Typed economy preparation for deterministic fixtures.

use super::WorldBuilder;
use crate::sim::Government;

impl WorldBuilder<'_> {
    pub(crate) fn clear_purchase_reserves(&mut self) {
        self.game.ai_purchase_reserve.clear();
    }

    /// Start under an unlocked government without scheduling a revolution.
    pub(crate) fn government(&mut self, owner: usize, government: Government) {
        assert!(
            owner < self.game.civilization_count,
            "inactive scenario government owner {owner}"
        );
        self.game.set_scenario_government(owner, government);
    }

    pub(crate) fn treasury(&mut self, owner: usize, gold: i32) {
        assert!(owner < self.game.civilization_count);
        self.game.players[owner].gold = gold;
    }

    pub(crate) fn tax_rates(&mut self, owner: usize, rates: [i32; 3]) {
        assert!(
            rates.iter().all(|rate| (0..=100).contains(rate)) && rates.iter().sum::<i32>() == 100
        );
        self.game.players[owner].rates = rates;
    }

    /// Force a government for a transition/upkeep fixture, without revolution timing.
    pub(crate) fn government_state(&mut self, owner: usize, government: Government) {
        self.game.players[owner].government = government;
    }

    /// Establish a route between two existing city IDs without a delivery bonus.
    pub(crate) fn trade_route(&mut self, from: u32, to: u32, goods: &str) {
        assert!(
            self.game.establish_scenario_trade_route(from, to, goods),
            "cannot preset trade route {from} -> {to} for goods {goods:?}"
        );
    }
}
