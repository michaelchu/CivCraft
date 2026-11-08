//! Diplomatic contact creation and contact-state transitions.

use super::*;

impl Simulation {
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

    pub(crate) fn contacted_civilizations(&self, owner: usize) -> Vec<usize> {
        let mut contacts: Vec<_> = self
            .diplomacy_neighbors(owner)
            .filter_map(|(other, relation)| {
                (other != owner
                    && other != BARBARIANS
                    && self.player_alive(other)
                    && relation.first_contact.is_some())
                .then_some((relation.first_contact.unwrap_or_default(), other))
            })
            .collect();
        contacts.sort_unstable();
        contacts.into_iter().map(|(_, other)| other).collect()
    }

    pub(crate) fn diplomatic_attitude(&self, owner: usize, other: usize) -> &'static str {
        // UI bands summarize the reference AI's [-1000, 1000] love scale.
        // They are display labels; the underlying diplomacy score is unchanged.
        match self.diplomatic_love(owner, other) {
            love if love <= -600 => "Furious",
            love if love <= -200 => "Annoyed",
            love if love < 200 => "Cautious",
            love if love < 600 => "Polite",
            _ => "Gracious",
        }
    }

    pub(in crate::sim) fn diplomatic_meeting_possible(&self, owner: usize, other: usize) -> bool {
        owner != other
            && owner < self.players.len()
            && other < self.players.len()
            && owner != BARBARIANS
            && other != BARBARIANS
            && self.player_alive(owner)
            && self.player_alive(other)
            && (self.has_embassy(owner, other)
                || self.has_embassy(other, owner)
                || self
                    .diplomatic_contact_until
                    .get(&Self::relation_key(owner, other))
                    .copied()
                    .is_some_and(|until| self.turn < until))
    }

    pub(in crate::sim) fn seed_legacy_diplomatic_contacts(&mut self) {
        // V1–V8 recorded first contact but no last-contact counter. Give known
        // living factions one bounded migration window without replaying
        // physical contact, first-contact callbacks, or random draws.
        let until = self.turn.saturating_add(20);
        for (&(a, b), relation) in &self.diplomacy {
            if a != BARBARIANS
                && b != BARBARIANS
                && relation.first_contact.is_some()
                && self.player_alive(a)
                && self.player_alive(b)
            {
                self.diplomatic_contact_until.entry((a, b)).or_insert(until);
            }
        }
    }

    pub(in crate::sim) fn refresh_diplomatic_contact_effects(&mut self) {
        // srv_main.c:do_have_contacts_effect runs after city refresh at phase
        // end, before the turn advances. Civ2Civ3 grants this via Marco Polo.
        for owner in 0..self.players.len() {
            if owner == BARBARIANS
                || !self.player_alive(owner)
                || !self.has_active_wonder(owner, Building::MARCO_POLOS_EMBASSY)
            {
                continue;
            }
            for other in 0..self.players.len() {
                if other != owner && other != BARBARIANS && self.player_alive(other) {
                    self.make_diplomatic_contact(owner, other);
                }
            }
        }
    }

    pub(crate) fn refresh_diplomatic_contact_at(&mut self, owner: usize, x: i32, y: i32) {
        if owner == BARBARIANS || owner >= self.players.len() || !self.player_alive(owner) {
            return;
        }
        // plrhand.c:maybe_make_contact examines adjacent units/cities, not
        // every visible tile or territorial ownership. Current unit roster
        // has no Flagless type (native skips those units).
        let contacts: std::collections::BTreeSet<_> = self
            .units
            .iter()
            .map(|unit| (unit.owner, unit.x, unit.y))
            .chain(self.cities.iter().map(|city| (city.owner, city.x, city.y)))
            .filter_map(|(other, tx, ty)| {
                (other != owner
                    && other != BARBARIANS
                    && self.map_wrap.civ3_distance(x, y, tx, ty) <= 1)
                    .then_some(other)
            })
            .collect();
        for other in contacts {
            self.make_diplomatic_contact(owner, other);
        }
    }

    pub(in crate::sim) fn make_diplomatic_contact(&mut self, a: usize, b: usize) {
        if a == b
            || a == BARBARIANS
            || b == BARBARIANS
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

    pub(super) fn has_real_embassy(&self, owner: usize, other: usize) -> bool {
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

    pub(crate) fn can_establish_embassy(&self, unit: &Unit, city: &City) -> bool {
        (unit.kind.is_diplomatic() || unit.kind == UnitKind::Explorer)
            && unit.moves_left > 0
            && unit.transport.is_none()
            && city.owner != unit.owner
            && city.owner != BARBARIANS
            && self.map_wrap.civ3_distance(unit.x, unit.y, city.x, city.y) <= 1
            && !self.has_real_embassy(unit.owner, city.owner)
    }

    pub(in crate::sim) fn establish_embassy(&mut self, unit_id: u32, city_id: u32) -> bool {
        let Some(unit) = self.unit_by_id(unit_id) else {
            return false;
        };
        let Some(city) = self.city_by_id(city_id) else {
            return false;
        };
        if !self.can_establish_embassy(unit, city) {
            return false;
        }
        let (owner, other) = (unit.owner, city.owner);
        let spy = unit.kind == UnitKind::Spy;
        let message = format!("An embassy was established in {}.", city.name);
        self.make_diplomatic_contact(owner, other);
        let relation = self
            .diplomacy
            .entry(Self::relation_key(owner, other))
            .or_default();
        relation.embassy[usize::from(owner > other)] = true;
        relation.first_contact.get_or_insert(self.turn);
        if spy {
            self.units
                .iter_mut()
                .find(|unit| unit.id == unit_id)
                .unwrap()
                .moves_left -= 1;
        } else {
            self.units.retain(|unit| unit.id != unit_id);
            self.prune_unit_ai_state();
        }
        if owner == HUMAN {
            self.add_event(message);
        }
        self.refresh_visibility();
        true
    }
}
