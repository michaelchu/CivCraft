//! Treaty and pact state transitions.

use super::*;

const DEFAULT_CEASEFIRE_LENGTH: i32 = 16;
const DEFAULT_ARMISTICE_LENGTH: i32 = 16;

impl Simulation {
    pub(crate) fn pact(&self, a: usize, b: usize) -> Pact {
        self.diplomacy
            .get(&Self::relation_key(a, b))
            .map_or(Pact::War, |relation| relation.pact)
    }

    pub(crate) fn at_war(&self, a: usize, b: usize) -> bool {
        a != b && self.pact(a, b) == Pact::War
    }

    pub(crate) fn allied(&self, a: usize, b: usize) -> bool {
        a == b || self.pact(a, b) == Pact::Alliance
    }

    pub(in crate::sim) fn pact_masks(&self, owner: usize) -> (u64, u64, u64) {
        let mut allied = 0_u64;
        let mut war = 0_u64;
        let mut peace = 0_u64;
        for other in 0..self.players.len().min(64) {
            if other == owner {
                allied |= 1_u64 << other;
            } else {
                match self.pact(owner, other) {
                    Pact::Alliance => allied |= 1_u64 << other,
                    Pact::War => war |= 1_u64 << other,
                    Pact::Peace => peace |= 1_u64 << other,
                    Pact::Ceasefire | Pact::Armistice => {}
                }
            }
        }
        (allied, war, peace)
    }

    pub(crate) fn pact_turns(&self, a: usize, b: usize) -> i32 {
        self.diplomacy
            .get(&Self::relation_key(a, b))
            .map_or(0, |relation| (relation.until - self.turn).max(0))
    }

    pub(crate) fn next_pact(&self, a: usize, b: usize) -> Option<Pact> {
        if !self.pact_transition_allowed(a, b) || self.vassal_master(b).is_some() {
            return None;
        }
        match self.pact(a, b) {
            Pact::War => Some(Pact::Ceasefire),
            Pact::Ceasefire if self.pact_turns(a, b) <= 4 => Some(Pact::Peace),
            Pact::Peace => Some(Pact::Alliance),
            _ => None,
        }
    }

    pub(crate) fn has_senate(&self, owner: usize) -> bool {
        owner < self.players.len() && self.player_effect(owner, EffectKind::HasSenate) > 0
    }

    pub(crate) fn senate_blocks_cancellation(&self, owner: usize, other: usize) -> bool {
        self.has_senate(owner)
            && self.player_effect(owner, EffectKind::NoAnarchy) <= 0
            && self
                .diplomacy
                .get(&Self::relation_key(owner, other))
                .is_none_or(|relation| {
                    relation.cause_until[usize::from(owner > other)] <= self.turn
                })
    }

    pub(super) fn pact_refusal(
        &self,
        receiver: usize,
        proposer: usize,
        proposed: Pact,
    ) -> Option<&'static str> {
        if !self.pact_transition_allowed(proposer, receiver) {
            return Some("War and peace are controlled by the master.");
        }
        if receiver >= self.players.len()
            || proposer >= self.players.len()
            || receiver == proposer
            || self.player_has_no_diplomacy(receiver)
            || self.player_has_no_diplomacy(proposer)
        {
            return Some("We cannot conduct diplomacy with you.");
        }
        if proposed == Pact::Alliance && self.relationship_caution(receiver, proposer) {
            return Some("We need to rebuild trust after the broken pact.");
        }
        if self.players[receiver].ai
            && proposed != Pact::Ceasefire
            && self.ai_war_countdown(receiver, proposer) != -1
        {
            return Some("We are preparing for war or remain committed to the current war.");
        }
        if self.next_pact(receiver, proposer) != Some(proposed)
            && !(proposed == Pact::Alliance && self.pact(receiver, proposer) != Pact::Alliance)
        {
            return Some("We must honor the current treaty before negotiating the next one.");
        }
        if proposed != Pact::Ceasefire {
            let live_enemy = (0..self.players.len()).any(|third| {
                third != receiver
                    && third != proposer
                    && third != BARBARIANS
                    && self.player_alive(third)
                    && self.allied(proposer, third)
                    && self.diplomatic_at_war(receiver, third)
            });
            if live_enemy {
                return Some("Your ties to an enemy make this treaty impossible.");
            }
            if self
                .diplomatic_intel(receiver, proposer)
                .allied_with_enemy
                .is_some()
            {
                return Some("Keep your distance from our enemy for a while first.");
            }
            if proposed == Pact::Alliance
                && (0..self.players.len()).any(|third| {
                    third != receiver
                        && third != proposer
                        && third != BARBARIANS
                        && self.player_alive(third)
                        && self.allied(receiver, third)
                        && self.diplomatic_at_war(proposer, third)
                })
            {
                return Some("You are at war with our ally.");
            }
        }
        let empty = Relation::default();
        let relation = self
            .diplomacy
            .get(&Self::relation_key(receiver, proposer))
            .unwrap_or(&empty);
        if relation.first_contact.is_none() && !self.diplomatic_meeting_possible(receiver, proposer)
        {
            return Some("We have not made contact.");
        }
        if proposed == Pact::Alliance && self.diplomatic_love(receiver, proposer) < 100 {
            return Some("We simply do not trust you with an alliance yet.");
        }
        None
    }

    pub(super) fn pact_clause_value(&self, owner: usize, other: usize, proposed: Pact) -> i64 {
        if matches!(proposed, Pact::Ceasefire | Pact::Peace) {
            let recovery = self.diplomatic_peace_value(owner, other);
            if recovery > 0 {
                return recovery;
            }
        }
        let empty = Relation::default();
        let relation = self
            .diplomacy
            .get(&Self::relation_key(owner, other))
            .unwrap_or(&empty);
        let love = self.diplomatic_love(owner, other);
        let missing = match proposed {
            Pact::Peace => love - 125,
            Pact::Alliance => love - 250,
            Pact::Ceasefire if self.turn - relation.first_contact.unwrap_or(0) < 15 => {
                return 0;
            }
            Pact::Ceasefire => love,
            _ => return 0,
        };
        // daidiplomacy.c:greed converts missing love into negative gold value.
        let deficit = i64::from(missing.min(0));
        let mut value = -(deficit * deficit / 10);
        if proposed == Pact::Alliance
            && !matches!(
                self.pact(owner, other),
                Pact::Peace | Pact::Armistice | Pact::Alliance
            )
        {
            let peace_deficit = i64::from((love - 125).min(0));
            value -= peace_deficit * peace_deficit / 10;
        }
        let shared_ally = self
            .diplomatic_intel(owner, other)
            .allied_with_ally
            .is_some();
        if shared_ally {
            value /= 2;
        }
        value
    }

    pub(in crate::sim) fn player_alive(&self, owner: usize) -> bool {
        self.cities.iter().any(|city| city.owner == owner)
            || self.units.iter().any(|unit| unit.owner == owner)
    }

    pub(in crate::sim) fn negotiate(&mut self, other: usize, proposed: Pact) -> bool {
        if !self.can_negotiate(other) {
            return false;
        }
        if let Some(reason) = self.pact_refusal(other, HUMAN, proposed) {
            self.add_event(format!("{} declined. {reason}", self.players[other].name));
            return false;
        }
        let offer = TreatyOffer {
            pact: Some(proposed),
            ..TreatyOffer::default()
        };
        if !self.execute_offer(HUMAN, other, &offer) {
            self.add_event(format!(
                "{} requires compensation for that treaty.",
                self.players[other].name
            ));
            return false;
        }
        true
    }

    pub(in crate::sim) fn sign_pact(&mut self, a: usize, b: usize, proposed: Pact) {
        if !self.pact_transition_allowed(a, b) {
            return;
        }
        let pact = if proposed == Pact::Peace {
            Pact::Armistice
        } else {
            proposed
        };
        let length = self.treaty_length(
            self.vassal_master(a).unwrap_or(a),
            self.vassal_master(b).unwrap_or(b),
            proposed,
        );
        let until = if matches!(proposed, Pact::Peace | Pact::Ceasefire) {
            self.turn.saturating_add(length)
        } else {
            0
        };
        if self.diplomatic_group(a).len() == 1 && self.diplomatic_group(b).len() == 1 {
            self.set_pact_state(a, b, pact, until);
        } else {
            let changes = self.plan_group_pact(a, b, pact, until);
            self.commit_pact_changes(changes);
        }
    }

    /// Applies an already validated final state; callers own group expansion.
    pub(super) fn set_pact_state(&mut self, a: usize, b: usize, proposed: Pact, until: i32) {
        let previous = self.contacted_pact(a, b);
        let relation = self.diplomacy.entry(Self::relation_key(a, b)).or_default();
        if proposed == Pact::War && relation.pact != Pact::War {
            relation.war_since = Some(self.turn);
            relation.war_aggressor = None;
        } else if proposed != Pact::War {
            relation.war_since = None;
            relation.war_aggressor = None;
        }
        relation.pact = proposed;
        relation.until = until;
        relation.offer = None;
        // Native diplomacy has a permanent state independently of physical
        // contact; a treaty through an embassy can change NO_CONTACT without
        // calling first_contact. Its untouched first_contact_turn is zero.
        relation.first_contact.get_or_insert(0);
        for key in [(a, b), (b, a)] {
            if key.0 == BARBARIANS || key.1 == BARBARIANS {
                continue;
            }
            let memory = self
                .ai_diplomatic_memory
                .entry(key)
                .or_insert_with(|| DiplomaticMemory::for_pair(key.0, key.1));
            if Self::pact_rank(proposed) > Self::pact_rank(memory.strongest_pact) {
                memory.strongest_pact = proposed;
            }
        }
        self.trace_diplomacy(a, b, "pact_changed", "treaty_signed", previous, None);
        self.ai_war_plans.remove(&(a, b));
        self.ai_war_plans.remove(&(b, a));
        if matches!(proposed, Pact::Peace | Pact::Armistice) {
            let mut returned_to_normalcy = Vec::new();
            for owner in [a, b] {
                if let Some(player) = self.players.get_mut(owner)
                    && player.mobilized
                {
                    player.mobilized = false;
                    returned_to_normalcy.push(player.name.clone());
                }
            }
            for name in returned_to_normalcy {
                self.add_event(format!("{name} returns to normalcy."));
            }
        }
        if a == HUMAN || b == HUMAN {
            let other = if a == HUMAN { b } else { a };
            let detail = if proposed == Pact::Armistice {
                format!(
                    " Withdraw from their territory during the {}-turn armistice.",
                    (until - self.turn).max(0)
                )
            } else {
                String::new()
            };
            self.add_event(format!(
                "{} agreed to {}.{detail}",
                self.players[other].name,
                if proposed == Pact::Armistice {
                    "peace".to_owned()
                } else {
                    proposed.name().to_lowercase()
                }
            ));
        }
    }

    pub(in crate::sim) fn cancel_pact(&mut self, owner: usize, other: usize) -> bool {
        self.cancel_pact_inner(owner, other, false)
    }

    fn cancel_pact_inner(&mut self, owner: usize, other: usize, forced: bool) -> bool {
        let pact = self.pact(owner, other);
        if self.same_vassal_group(owner, other)
            || (!forced && !self.pact_transition_allowed(owner, other))
        {
            return false;
        }
        if owner == other
            || owner >= self.players.len()
            || other >= self.players.len()
            || pact == Pact::War
            || !self.player_alive(owner)
            || !self.player_alive(other)
        {
            return false;
        }
        if !forced && self.senate_blocks_cancellation(owner, other) {
            if owner == HUMAN {
                self.add_event(format!(
                    "The Senate refuses to break the {} with {}.",
                    pact.name().to_lowercase(),
                    self.players[other].name
                ));
            }
            return false;
        }
        let next = if pact == Pact::Alliance {
            Pact::Armistice
        } else {
            Pact::War
        };
        let changes = self
            .plan_group_pact(
                owner,
                other,
                next,
                if next == Pact::Armistice {
                    self.turn + 16
                } else {
                    0
                },
            )
            .into_iter()
            .filter(|c| (c.first, c.second) != Self::relation_key(owner, other))
            .collect();
        let breach = self.relationship_breach(owner, other, pact, forced);
        if pact != Pact::Alliance {
            self.record_war_incident(owner, other, pact);
        }
        let relation = self
            .diplomacy
            .get_mut(&Self::relation_key(owner, other))
            .unwrap();
        relation.pact = if pact == Pact::Alliance {
            Pact::Armistice
        } else {
            Pact::War
        };
        relation.until = if pact == Pact::Alliance {
            self.turn + 16
        } else {
            0
        };
        relation.offer = None;
        if pact == Pact::Alliance {
            relation.war_since = None;
            relation.war_aggressor = None;
        } else {
            relation.war_since = Some(self.turn);
            relation.war_aggressor = Some(owner);
        }
        relation.cause_until[usize::from(owner > other)] = 0;
        self.trace_diplomacy(
            owner,
            other,
            "pact_changed",
            if forced {
                "forced_cancellation"
            } else {
                "cancellation"
            },
            Some(pact),
            None,
        );
        if breach {
            self.remember_pact_breach(owner, other);
        }
        if pact == Pact::Alliance {
            self.resolve_split_stacks(owner, other);
            self.refresh_visibility();
        } else {
            self.record_provocation(other, owner);
            let mutual_allies: Vec<_> = (0..self.players.len())
                .filter(|&ally| {
                    ally != owner
                        && ally != other
                        && self.player_alive(ally)
                        && self.allied(ally, owner)
                        && self.allied(ally, other)
                })
                .collect();
            for ally in mutual_allies {
                self.record_provocation(ally, owner);
                self.cancel_pact_inner(ally, owner, true);
            }
        }
        self.commit_pact_changes(changes);
        if self.at_war(owner, other) {
            self.remember_ally_support(owner, other);
        }
        self.add_event(format!(
            "{} ended the {} with {}.",
            self.players[owner].name,
            pact.name().to_lowercase(),
            self.players[other].name
        ));
        true
    }

    pub(super) fn break_mutual_alliances_after_war(&mut self, a: usize, b: usize) {
        let mutual_allies: Vec<_> = (0..self.players.len())
            .filter(|&ally| {
                ally != a
                    && ally != b
                    && self.player_alive(ally)
                    && self.allied(ally, a)
                    && self.allied(ally, b)
            })
            .collect();
        for ally in mutual_allies {
            // Freeciv marks the alliance breakup as justified by the new war,
            // so the Senate cannot prevent this automatic treaty change.
            self.cancel_pact_inner(ally, a, true);
            self.cancel_pact_inner(ally, b, true);
        }
    }

    /// The normal armistice expiry cleanup, also used for propagated deadlines.
    pub(super) fn expel_armistice_units(&mut self, a: usize, b: usize) {
        let expelled: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                (!self.military_access(a, b)
                    && unit.owner == a
                    && self.tile(unit.x, unit.y).owner == Some(b)
                    || !self.military_access(a, b)
                        && unit.owner == b
                        && self.tile(unit.x, unit.y).owner == Some(a))
                    && !unit.kind.is_civilian()
                    && !movement_rules::MovementRules::campaign()
                        .terrain_has_named_flag(self.tile(unit.x, unit.y).terrain, "EnterBorders")
                        .expect("campaign terrain has valid flags")
            })
            .map(|unit| unit.id)
            .collect();
        for unit in self.units.iter().filter(|unit| {
            expelled.contains(&unit.id) || unit.transport.is_some_and(|id| expelled.contains(&id))
        }) {
            self.trace
                .unit_removed(self.turn, unit, lifecycle::UnitRemoval::Expelled);
        }
        self.units.retain(|unit| {
            !expelled.contains(&unit.id) && unit.transport.is_none_or(|id| !expelled.contains(&id))
        });
        self.prune_unit_ai_state();
    }

    fn treaty_length(&self, a: usize, b: usize, proposed: Pact) -> i32 {
        let (effect, default_length, take_minimum) = match proposed {
            Pact::Ceasefire => (
                EffectKind::CeasefireLengthPct,
                DEFAULT_CEASEFIRE_LENGTH,
                true,
            ),
            Pact::Peace => (
                EffectKind::ArmisticeLengthPct,
                DEFAULT_ARMISTICE_LENGTH,
                false,
            ),
            _ => return 0,
        };
        let percent_a = self.player_effect(a, effect);
        let percent_b = self.player_effect(b, effect);
        let percent = if take_minimum {
            percent_a.min(percent_b)
        } else {
            percent_a.max(percent_b)
        };
        let length = (i64::from(default_length).saturating_mul(percent) / 100).max(1);
        i32::try_from(length).unwrap_or(i32::MAX)
    }
}

#[cfg(test)]
mod religion_tests {
    use super::*;
    use crate::sim::{FaithPolicy, fixtures::WorldBuilder};
    #[test]
    fn religion_changes_treaty_valuation_without_changing_existing_pacts() {
        let mut g = Simulation::new(crate::test_support::seeded_map());
        let mut w = WorldBuilder::new(&mut g);
        w.clear_units_and_cities();
        let a = w.founding_city(HUMAN, (10, 24));
        let b = w.founding_city(1, (20, 24));
        w.finish();
        g.players[HUMAN].known.push(Tech::MEDITATION);
        assert!(g.found_religion(HUMAN, a, 1));
        WorldBuilder::new(&mut g).city_religion(b, &[(1, 100)], Some(1));
        g.sign_pact(HUMAN, 1, Pact::Peace);
        let original_pact = g.pact(HUMAN, 1);
        let baseline = g.pact_clause_value(HUMAN, 1, Pact::Peace);
        assert!(g.declare_faith(HUMAN, Some(1), FaithPolicy::Tolerant));
        assert!(g.declare_faith(1, Some(1), FaithPolicy::Tolerant));
        assert!(g.pact_clause_value(HUMAN, 1, Pact::Peace) > baseline);
        assert_eq!(g.pact(HUMAN, 1), original_pact);
    }
}
