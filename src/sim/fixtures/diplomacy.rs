//! Typed diplomacy preparation for deterministic fixtures.

use super::WorldBuilder;
use crate::sim::Pact;

impl WorldBuilder<'_> {
    pub(crate) fn clear_relationships(&mut self) {
        self.game.diplomacy.clear();
        self.game.vassalage = Default::default();
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

    pub(crate) fn provocation(&mut self, victim: usize, offender: usize) {
        self.game.set_scenario_provocation(victim, offender);
    }

    pub(crate) fn immediate_war_plan(&mut self, owner: usize, target: usize) {
        self.game.ai_war_plans.insert(
            (owner, target),
            crate::sim::diplomacy::WarPlan {
                countdown: 0,
                reason: crate::sim::diplomacy::WarReason::Behavior,
                objective: None,
                readiness_waited: u16::MAX,
            },
        );
    }
}

impl WorldBuilder<'_> {
    /// A threatened AI neighbor, a protector and a third independent nation.
    /// Modern Armor makes the force ratio explicit without simulating a long war.
    pub(crate) fn vassalage_campaign(&mut self, kind: crate::sim::VassalKind) {
        use crate::{
            rules::Tech,
            sim::{HUMAN, UnitKind, VassalKind},
        };
        self.clear_campaign_entities();
        for (owner, position) in [(HUMAN, (8, 20)), (1, (12, 24)), (2, (18, 30))] {
            self.founding_city(owner, position);
            self.treasury(owner, 1000);
        }
        self.known_technologies(HUMAN, &[Tech::FEUDALISM]);
        self.ai_control(1, true);
        for (a, b) in [(0, 1), (0, 2), (1, 2)] {
            self.diplomacy(a, b, Pact::Peace, 0, [6000, 6000]);
            self.embassy(a, b);
            self.embassy(b, a);
        }
        self.diplomatic_state(
            0,
            1,
            if kind == VassalKind::Capitulation {
                Pact::War
            } else {
                Pact::Peace
            },
            [6000, 6000],
        );
        self.grassland((12, 23));
        self.unit(1, UnitKind::Warrior, (12, 24));
        for _ in 0..3 {
            self.unit(HUMAN, UnitKind::ModernArmor, (12, 23));
        }
        if kind == VassalKind::Voluntary {
            self.grassland((13, 24));
            for _ in 0..3 {
                self.unit(2, UnitKind::ModernArmor, (13, 24));
            }
            self.diplomatic_state(1, 2, Pact::War, [0, 0]);
        }
    }
}
