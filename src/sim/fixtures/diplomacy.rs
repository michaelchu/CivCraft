//! Typed diplomacy preparation for deterministic fixtures.

use super::WorldBuilder;
use crate::sim::Pact;

impl WorldBuilder<'_> {
    pub(crate) fn clear_relationships(&mut self) {
        self.game.diplomacy.clear();
        self.game.ai_war_plans.clear();
    }

    /// Trust values are ordered by the two slots passed by the caller.
    pub(crate) fn diplomacy(
        &mut self,
        first: usize,
        second: usize,
        pact: Pact,
        first_contact: i32,
        trust: [i32; 2],
    ) {
        assert!(
            first != second
                && first < self.game.civilization_count
                && second < self.game.civilization_count,
            "diplomacy requires two active scenario civilizations"
        );
        assert!(
            (0..=self.game.turn).contains(&first_contact),
            "first contact cannot occur after the scenario starts"
        );
        self.game
            .set_scenario_diplomacy(first, second, pact, first_contact, trust);
    }

    /// The holder has an embassy with the host, independent of the reverse direction.
    pub(crate) fn embassy(&mut self, holder: usize, host: usize) {
        self.game.grant_scenario_embassy(holder, host);
    }

    /// The receiver sees the tiles currently visible to the giver.
    pub(crate) fn shared_vision(&mut self, giver: usize, receiver: usize) {
        self.game.grant_scenario_vision(giver, receiver);
    }

    /// Override the established pact and trust without running treaty actions.
    pub(crate) fn diplomatic_state(
        &mut self,
        first: usize,
        second: usize,
        pact: Pact,
        mut trust: [i32; 2],
    ) {
        if first > second {
            trust.swap(0, 1);
        }
        let relation = self
            .game
            .diplomacy
            .get_mut(&(first.min(second), first.max(second)))
            .expect("fixture relationship exists");
        relation.pact = pact;
        relation.trust = trust;
    }

    pub(crate) fn treaty_expiry(&mut self, first: usize, second: usize, until: i32) {
        self.game
            .diplomacy
            .get_mut(&(first.min(second), first.max(second)))
            .expect("fixture relationship exists")
            .until = until;
    }

    pub(crate) fn immediate_war_plan(&mut self, owner: usize, target: usize) {
        self.game.ai_war_plans.insert(
            (owner, target),
            crate::sim::diplomacy::WarPlan {
                countdown: 0,
                reason: crate::sim::diplomacy::WarReason::Hatred,
            },
        );
    }
}
