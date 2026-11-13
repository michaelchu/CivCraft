//! Diplomatic contact creation and contact-state transitions.

use super::*;

impl Simulation {
    pub(super) fn diplomacy_effect_value(&self, owner: usize, kind: &str) -> i64 {
        let rules = crate::rules::freeciv::RulesetData::campaign_default();
        let context = super::super::action_rules::SimulationRequirementContext::for_player_at(
            self, owner, None,
        )
        .with_government(self.players[owner].government);
        crate::rules::freeciv::effect_value(&rules.effects, kind, &context)
            .unwrap_or_else(|error| panic!("could not evaluate campaign {kind} effect: {error:?}"))
    }

    pub(super) fn player_has_no_diplomacy(&self, owner: usize) -> bool {
        self.diplomacy_effect_value(owner, "No_Diplomacy") > 0
    }

    pub(super) fn diplomacy_neighbors(
        &self,
        owner: usize,
    ) -> impl Iterator<Item = (usize, &Relation)> + '_ {
        self.diplomacy
            .iter()
            .filter_map(move |(&(a, b), relation)| {
                let other = if a == owner {
                    b
                } else if b == owner {
                    a
                } else {
                    return None;
                };
                Some((other, relation))
            })
    }

    pub(in crate::sim) fn diplomatic_love(&self, owner: usize, other: usize) -> i32 {
        self.diplomacy
            .get(&Self::relation_key(owner, other))
            .map_or(0, |relation| {
                relation.trust[usize::from(owner > other)] / 10
            })
    }

    pub(in crate::sim) fn diplomatic_meeting_possible(&self, owner: usize, other: usize) -> bool {
        owner != other
            && owner < self.players.len()
            && other < self.players.len()
            && self.player_alive(owner)
            && self.player_alive(other)
            && !self.player_has_no_diplomacy(owner)
            && !self.player_has_no_diplomacy(other)
            && (self.has_embassy(owner, other)
                || self.has_embassy(other, owner)
                || self
                    .diplomatic_contact_until
                    .get(&Self::relation_key(owner, other))
                    .copied()
                    .is_some_and(|until| self.turn < until))
    }

    pub(in crate::sim) fn refresh_diplomatic_contact_effects(&mut self) {
        // srv_main.c:do_have_contacts_effect runs after city refresh at phase
        // end, before the turn advances. Civ2Civ3 grants this via Marco Polo.
        for owner in 0..self.players.len() {
            if !self.player_alive(owner)
                || !self.has_active_wonder(owner, Building::MARCO_POLOS_EMBASSY)
            {
                continue;
            }
            for other in 0..self.players.len() {
                if other != owner && self.player_alive(other) {
                    self.make_diplomatic_contact(owner, other);
                }
            }
        }
    }

    pub(crate) fn refresh_diplomatic_contact_at(&mut self, owner: usize, x: i32, y: i32) {
        if owner >= self.players.len() || !self.player_alive(owner) {
            return;
        }
        // plrhand.c:maybe_make_contact examines adjacent units/cities, not
        // every visible tile or territorial ownership.
        let contacts: std::collections::BTreeSet<_> = self
            .units
            .iter()
            .map(|unit| (unit.owner, unit.x, unit.y))
            .chain(self.cities.iter().map(|city| (city.owner, city.x, city.y)))
            .filter_map(|(other, tx, ty)| {
                (other != owner && self.map_wrap.civ3_distance(x, y, tx, ty) <= 1).then_some(other)
            })
            .collect();
        for other in contacts {
            self.make_diplomatic_contact(owner, other);
        }
    }

    pub(in crate::sim) fn make_diplomatic_contact(&mut self, a: usize, b: usize) {
        if a == b
            || a >= self.players.len()
            || b >= self.players.len()
            || !self.player_alive(a)
            || !self.player_alive(b)
        {
            return;
        }
        let first_contact = self
            .diplomacy
            .get(&Self::relation_key(a, b))
            .is_none_or(|relation| relation.first_contact.is_none());
        self.initialize_diplomatic_contact(a, b);
        if first_contact && (a == HUMAN || b == HUMAN) {
            let other = if a == HUMAN { b } else { a };
            self.add_event(format!(
                "You have made contact with the {}, ruled by {}.",
                self.players[other].name,
                self.players[other].leader_name()
            ));
        }
        // Native decrements before advancing the turn and exempts the first
        // contact turn. Later physical contacts refresh the ordinary20 turns.
        // No_Diplomacy still permits the first-contact state transition, but
        // Freeciv does not grant either player a contact timer in that case.
        if self.player_has_no_diplomacy(a) || self.player_has_no_diplomacy(b) {
            return;
        }
        let first_turn = self.diplomacy[&Self::relation_key(a, b)].first_contact == Some(self.turn);
        self.diplomatic_contact_until.insert(
            Self::relation_key(a, b),
            self.turn.saturating_add(20 + i32::from(first_turn)),
        );
    }

    pub(super) fn initialize_diplomatic_contact(&mut self, a: usize, b: usize) {
        let key = Self::relation_key(a, b);
        if self
            .diplomacy
            .get(&key)
            .is_some_and(|relation| relation.first_contact.is_some())
        {
            return;
        }
        let common_ally = (0..self.players.len()).any(|third| {
            third != a
                && third != b
                && third != BARBARIANS
                && self.player_alive(third)
                && self.allied(a, third)
                && self.allied(b, third)
        });
        let relation = self.diplomacy.entry(key).or_default();
        relation.first_contact = Some(self.turn);
        if common_ally && relation.pact == Pact::War {
            // plrhand.c:get_default_diplstate sets immediate Peace, without
            // an armistice, when both newly met players share an ally.
            relation.pact = Pact::Peace;
            relation.until = 0;
            for pair in [(a, b), (b, a)] {
                let memory = self
                    .ai_diplomatic_memory
                    .entry(pair)
                    .or_insert_with(|| DiplomaticMemory::for_pair(pair.0, pair.1));
                if Self::pact_rank(memory.strongest_pact) < Self::pact_rank(Pact::Peace) {
                    memory.strongest_pact = Pact::Peace;
                }
            }
        }
        // dai_diplomacy_first_contact initializes each AI's own attitude once.
        for owner in [a, b] {
            if owner != BARBARIANS && self.players[owner].ai {
                let adjustment = (2 - self.rand_below(5)) * 10;
                self.diplomacy.get_mut(&key).unwrap().trust[usize::from(owner == key.1)] +=
                    adjustment;
            }
        }
    }

    pub(in crate::sim) fn has_real_embassy(&self, owner: usize, other: usize) -> bool {
        self.diplomacy
            .get(&Self::relation_key(owner, other))
            .is_some_and(|relation| relation.embassy[usize::from(owner > other)])
    }

    pub(crate) fn has_embassy(&self, owner: usize, other: usize) -> bool {
        owner != other
            && other != BARBARIANS
            && (self.has_real_embassy(owner, other)
                || self.has_active_wonder(owner, Building::MARCO_POLOS_EMBASSY))
    }

    pub(in crate::sim) fn clear_outgoing_embassies(&mut self, owner: usize) {
        for (&(a, b), relation) in &mut self.diplomacy {
            if owner == a {
                relation.embassy[0] = false;
            } else if owner == b {
                relation.embassy[1] = false;
            }
        }
    }
}
