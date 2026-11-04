use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Pact {
    #[default]
    War,
    Ceasefire,
    Armistice,
    Peace,
    Alliance,
}

impl Pact {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::War => "War",
            Self::Ceasefire => "Ceasefire",
            Self::Armistice => "Armistice",
            Self::Peace => "Peace",
            Self::Alliance => "Alliance",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Concessions {
    pub(crate) gold: i32,
    pub(crate) technology: Option<Tech>,
    pub(crate) vision: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TreatyOffer {
    pub(crate) pact: Option<Pact>,
    // Terms are ordered by player ID.
    pub(crate) terms: [Concessions; 2],
}

impl TreatyOffer {
    pub(crate) fn bundled(self) -> bool {
        self.terms != [Concessions::default(); 2]
    }
}

#[derive(Default)]
pub(super) struct Relation {
    pact: Pact,
    until: i32,
    first_contact: Option<i32>,
    cause_until: [i32; 2],
    trust: [i32; 2],
    offer: Option<TreatyOffer>,
    vision: [bool; 2],
    offer_after: i32,
}

impl Simulation {
    pub(super) fn relation_key(a: usize, b: usize) -> (usize, usize) {
        (a.min(b), a.max(b))
    }

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

    pub(crate) fn pact_turns(&self, a: usize, b: usize) -> i32 {
        self.diplomacy
            .get(&Self::relation_key(a, b))
            .map_or(0, |relation| (relation.until - self.turn).max(0))
    }

    pub(crate) fn treaty_offer(&self, other: usize) -> Option<TreatyOffer> {
        self.diplomacy
            .get(&Self::relation_key(HUMAN, other))
            .and_then(|relation| relation.offer)
    }

    pub(crate) fn pact_offer(&self, other: usize) -> Option<Pact> {
        self.treaty_offer(other).and_then(|offer| offer.pact)
    }

    pub(crate) fn next_pact(&self, a: usize, b: usize) -> Option<Pact> {
        match self.pact(a, b) {
            Pact::War => Some(Pact::Ceasefire),
            Pact::Ceasefire if self.pact_turns(a, b) <= 4 => Some(Pact::Peace),
            Pact::Peace => Some(Pact::Alliance),
            _ => None,
        }
    }

    pub(crate) fn has_senate(&self, owner: usize) -> bool {
        if self.owns_wonder(owner, "Statue of Liberty") {
            return false;
        }
        matches!(
            self.players[owner].government,
            Government::Federation | Government::Democracy
        ) || (self
            .cities
            .iter()
            .any(|city| city.has_building("United Nations"))
            && self
                .completed_wonders
                .iter()
                .any(|building| building.data().name == "Manhattan Project")
            && !self.apollo_completed
            && self.players[owner]
                .known
                .iter()
                .any(|tech| tech.name() == "Nuclear Fission"))
    }

    pub(crate) fn senate_blocks_cancellation(&self, owner: usize, other: usize) -> bool {
        self.has_senate(owner)
            && self
                .diplomacy
                .get(&Self::relation_key(owner, other))
                .is_none_or(|relation| {
                    relation.cause_until[usize::from(owner > other)] <= self.turn
                })
    }

    pub(crate) fn can_negotiate(&self, other: usize) -> bool {
        other != HUMAN
            && other != BARBARIANS
            && other < self.players.len()
            && (self.cities.iter().any(|city| city.owner == other)
                || self.units.iter().any(|unit| unit.owner == other))
            && (self
                .diplomacy
                .contains_key(&Self::relation_key(HUMAN, other))
                || self
                    .cities
                    .iter()
                    .any(|city| city.owner == other && self.tile(city.x, city.y).visible)
                || self
                    .units
                    .iter()
                    .any(|unit| unit.owner == other && self.tile(unit.x, unit.y).visible))
    }

    fn pact_refusal(
        &self,
        receiver: usize,
        proposer: usize,
        proposed: Pact,
    ) -> Option<&'static str> {
        if self.next_pact(receiver, proposer) != Some(proposed) {
            return Some("We must honor the current treaty before negotiating the next one.");
        }
        if proposed != Pact::Ceasefire
            && (0..self.players.len()).any(|third| {
                third != receiver
                    && third != proposer
                    && third != BARBARIANS
                    && self.player_alive(third)
                    && ((self.allied(proposer, third) && self.at_war(receiver, third))
                        || (proposed == Pact::Alliance
                            && self.allied(receiver, third)
                            && self.at_war(proposer, third)))
            })
        {
            return Some("Your ties to an enemy make this treaty impossible.");
        }
        let Some(relation) = self.diplomacy.get(&Self::relation_key(receiver, proposer)) else {
            return Some("We have not made contact.");
        };
        let trust = relation.trust[usize::from(receiver > proposer)];
        let threshold = match proposed {
            Pact::Ceasefire if self.turn - relation.first_contact.unwrap_or(self.turn) < 15 => {
                return None;
            }
            Pact::Ceasefire => 0,
            Pact::Peace => 1250,
            Pact::Alliance => 2500,
            _ => return Some("That treaty is unavailable."),
        };
        (trust < threshold).then_some("We do not trust you enough for that treaty.")
    }

    pub(super) fn player_alive(&self, owner: usize) -> bool {
        self.cities.iter().any(|city| city.owner == owner)
            || self.units.iter().any(|unit| unit.owner == owner)
    }

    pub(super) fn negotiate(&mut self, other: usize, proposed: Pact) -> bool {
        if !self.can_negotiate(other) {
            return false;
        }
        let key = Self::relation_key(HUMAN, other);
        self.diplomacy
            .entry(key)
            .or_default()
            .first_contact
            .get_or_insert(self.turn);
        if let Some(reason) = self.pact_refusal(other, HUMAN, proposed) {
            self.add_event(format!("{} declined. {reason}", self.players[other].name));
            return false;
        }
        if self.next_pact(HUMAN, other) != Some(proposed) {
            return false;
        }
        self.sign_pact(HUMAN, other, proposed);
        true
    }

    pub(super) fn accept_offer(&mut self, other: usize, offer: TreatyOffer) -> bool {
        if !self.can_negotiate(other) || self.treaty_offer(other) != Some(offer) {
            return false;
        }
        if !self.execute_offer(HUMAN, other, offer) {
            self.decline_offer(other);
            self.add_event("That treaty offer is no longer valid.");
            return false;
        }
        self.add_event(format!(
            "Treaty terms agreed with {}.",
            self.players[other].name
        ));
        true
    }

    fn execute_offer(&mut self, a: usize, b: usize, offer: TreatyOffer) -> bool {
        if offer
            .pact
            .is_some_and(|pact| self.pact_refusal(b, a, pact).is_some())
        {
            return false;
        }
        let key = Self::relation_key(a, b);
        let after = offer.pact.unwrap_or_else(|| self.pact(a, b));
        for (index, (giver, receiver)) in [(key.0, key.1), (key.1, key.0)].into_iter().enumerate() {
            let terms = offer.terms[index];
            if terms.gold < 0
                || self.players[giver].gold < terms.gold
                || terms
                    .technology
                    .is_some_and(|tech| self.tech_sell_price(giver, receiver, tech).is_none())
                || (terms.vision && (after != Pact::Alliance || !self.vision_safe(giver, receiver)))
            {
                return false;
            }
        }
        if !self.ai_accepts_technology_exchange(key.0, key.1, &offer.terms) {
            return false;
        }
        if let Some(pact) = offer.pact {
            self.sign_pact(a, b, pact);
        }
        let relation = self.diplomacy.get_mut(&key).unwrap();
        relation.offer = None;
        for (index, (giver, receiver)) in [(key.0, key.1), (key.1, key.0)].into_iter().enumerate() {
            let terms = offer.terms[index];
            relation.vision[index] |= terms.vision;
            self.players[giver].gold -= terms.gold;
            self.players[receiver].gold += terms.gold;
        }
        for (index, receiver) in [key.1, key.0].into_iter().enumerate() {
            if let Some(tech) = offer.terms[index].technology {
                self.discover(receiver, tech);
                if self.players[receiver].researching == tech {
                    self.players[receiver].researching = self.research_target(receiver);
                }
            }
        }
        if offer.terms.iter().any(|terms| terms.vision) {
            self.refresh_visibility();
        }
        true
    }

    pub(super) fn tech_trade_partner_is_dangerous(&self, owner: usize, other: usize) -> bool {
        let stance = self.pact(owner, other);
        if stance == Pact::Ceasefire
            || (stance == Pact::War
                && self
                    .diplomacy
                    .contains_key(&Self::relation_key(owner, other)))
        {
            return true;
        }
        let has_hostile_ally = (0..self.players.len()).any(|third| {
            third != owner
                && third != other
                && third != BARBARIANS
                && self.player_alive(third)
                && self.allied(other, third)
                && self.at_war(owner, third)
                && self
                    .diplomacy
                    .contains_key(&Self::relation_key(owner, third))
        });
        has_hostile_ally
            || self
                .diplomacy
                .get(&Self::relation_key(owner, other))
                .is_none_or(|relation| {
                    let side = usize::from(owner > other);
                    relation.cause_until[side] > self.turn || relation.trust[side] < 1000
                })
    }

    fn vision_safe(&self, giver: usize, receiver: usize) -> bool {
        self.vision_recipients(receiver).into_iter().all(|third| {
            third == giver
                || third == receiver
                || self.allied(giver, third)
                || !self
                    .diplomacy
                    .contains_key(&Self::relation_key(giver, third))
        })
    }

    pub(super) fn vision_recipients(&self, giver: usize) -> Vec<usize> {
        let mut recipients = vec![giver];
        let mut next = 0;
        while let Some(&source) = recipients.get(next) {
            for (&(a, b), relation) in &self.diplomacy {
                let target = if a == source && relation.vision[0] {
                    b
                } else if b == source && relation.vision[1] {
                    a
                } else {
                    continue;
                };
                if !recipients.contains(&target) {
                    recipients.push(target);
                }
            }
            next += 1;
        }
        recipients
    }

    fn build_offer(&self, a: usize, b: usize, pact: Option<Pact>) -> TreatyOffer {
        let mut offer = TreatyOffer {
            pact,
            ..TreatyOffer::default()
        };
        let after = pact.unwrap_or_else(|| self.pact(a, b));
        if matches!(after, Pact::Peace | Pact::Alliance) {
            // Freeciv scans technologies received by the proposer first.
            let proposer = if a == HUMAN { b } else { a };
            let recipient = if proposer == a { b } else { a };
            let incoming: Vec<_> = Tech::all()
                .filter_map(|tech| {
                    self.tech_sell_price(recipient, proposer, tech)
                        .map(|price| (tech, price))
                })
                .collect();
            let outgoing: Vec<_> = Tech::all()
                .filter_map(|tech| {
                    self.tech_sell_price(proposer, recipient, tech)
                        .map(|price| (tech, price))
                })
                .collect();
            'exchange: for &(received, incoming_value) in &incoming {
                for &(given, outgoing_value) in &outgoing {
                    let balance = incoming_value - outgoing_value;
                    let giver = usize::from(proposer != a);
                    let mut terms = [Concessions::default(); 2];
                    terms[giver].technology = Some(given);
                    terms[1 - giver].technology = Some(received);
                    terms[giver].gold = balance.max(0);
                    terms[1 - giver].gold = (-balance).max(0);
                    if self.players[proposer].gold >= terms[giver].gold
                        && self.players[recipient].gold >= terms[1 - giver].gold
                        && self.ai_accepts_technology_exchange(a, b, &terms)
                    {
                        offer.terms = terms;
                        break 'exchange;
                    }
                }
            }
        }
        if after == Pact::Alliance {
            let existing = self
                .diplomacy
                .get(&(a, b))
                .map_or([false; 2], |relation| relation.vision);
            if self.vision_safe(b, a) && self.vision_safe(a, b) {
                offer.terms[0].vision = !existing[0];
                offer.terms[1].vision = !existing[1];
            }
        }
        offer
    }

    pub(super) fn decline_offer(&mut self, other: usize) -> bool {
        let Some(relation) = self.diplomacy.get_mut(&Self::relation_key(HUMAN, other)) else {
            return false;
        };
        relation.offer.take().is_some()
    }

    fn sign_pact(&mut self, a: usize, b: usize, proposed: Pact) {
        let relation = self.diplomacy.entry(Self::relation_key(a, b)).or_default();
        relation.pact = if proposed == Pact::Peace {
            Pact::Armistice
        } else {
            proposed
        };
        relation.until = if matches!(proposed, Pact::Peace | Pact::Ceasefire) {
            self.turn + 16
        } else {
            0
        };
        relation.offer = None;
        if a == HUMAN || b == HUMAN {
            let other = if a == HUMAN { b } else { a };
            let detail = if proposed == Pact::Peace {
                " Withdraw from their territory during the 16-turn armistice."
            } else {
                ""
            };
            self.add_event(format!(
                "{} agreed to {}.{detail}",
                self.players[other].name,
                proposed.name().to_lowercase()
            ));
        }
    }

    pub(super) fn cancel_pact(&mut self, owner: usize, other: usize) -> bool {
        let pact = self.pact(owner, other);
        if pact == Pact::War || !self.player_alive(other) {
            return false;
        }
        if self.senate_blocks_cancellation(owner, other) {
            if owner == HUMAN {
                self.add_event(format!(
                    "The Senate refuses to break the {} with {}.",
                    pact.name().to_lowercase(),
                    self.players[other].name
                ));
            }
            return false;
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
        relation.cause_until[usize::from(owner > other)] = 0;
        if pact == Pact::Alliance {
            relation.vision = [false; 2];
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
                self.cancel_pact(ally, owner);
            }
        }
        self.add_event(format!(
            "{} ended the {} with {}.",
            self.players[owner].name,
            pact.name().to_lowercase(),
            self.players[other].name
        ));
        true
    }

    pub(super) fn record_provocation(&mut self, victim: usize, offender: usize) {
        if victim != offender && victim != BARBARIANS && offender != BARBARIANS {
            let relation = self
                .diplomacy
                .entry(Self::relation_key(victim, offender))
                .or_default();
            relation.first_contact.get_or_insert(self.turn);
            relation.cause_until[usize::from(victim > offender)] = self.turn + 2;
        }
    }

    pub(super) fn remember_visible_rivals(&mut self) {
        for owner in self
            .units
            .iter()
            .filter(|unit| self.tiles[Self::idx(unit.x, unit.y)].visible)
            .map(|unit| unit.owner)
            .chain(
                self.cities
                    .iter()
                    .filter(|city| self.tiles[Self::idx(city.x, city.y)].visible)
                    .map(|city| city.owner),
            )
        {
            if owner != HUMAN && owner != BARBARIANS {
                self.diplomacy
                    .entry(Self::relation_key(HUMAN, owner))
                    .or_default()
                    .first_contact
                    .get_or_insert(self.turn);
            }
        }
    }

    pub(super) fn advance_diplomacy(&mut self) {
        let mut transitions = Vec::new();
        for (&(a, b), relation) in &mut self.diplomacy {
            if relation.until > 0 && relation.until <= self.turn {
                relation.pact = if relation.pact == Pact::Armistice {
                    Pact::Peace
                } else {
                    Pact::War
                };
                relation.until = 0;
                relation.offer = None;
                transitions.push((a, b, relation.pact));
            }
        }
        for (a, b, pact) in transitions {
            if pact == Pact::Peace {
                let expelled: Vec<_> = self
                    .units
                    .iter()
                    .filter(|unit| {
                        unit.owner == a && self.tile(unit.x, unit.y).owner == Some(b)
                            || unit.owner == b && self.tile(unit.x, unit.y).owner == Some(a)
                    })
                    .map(|unit| unit.id)
                    .collect();
                self.units.retain(|unit| {
                    !expelled.contains(&unit.id)
                        && unit.transport.is_none_or(|id| !expelled.contains(&id))
                });
            }
            if a == HUMAN || b == HUMAN {
                let other = if a == HUMAN { b } else { a };
                self.add_event(format!(
                    "{} with {}.",
                    if pact == Pact::Peace {
                        "Peace entered into force"
                    } else {
                        "Ceasefire expired"
                    },
                    self.players[other].name
                ));
            }
        }

        // Contact between AI nations comes from units encountering foreign borders
        // or meeting units and cities within one tile.
        let positions: Vec<_> = self
            .units
            .iter()
            .map(|unit| (unit.owner, unit.x, unit.y))
            .chain(self.cities.iter().map(|city| (city.owner, city.x, city.y)))
            .collect();
        for (index, &(a, x, y)) in positions.iter().enumerate() {
            if a == HUMAN || a == BARBARIANS {
                continue;
            }
            for &(b, bx, by) in &positions[index + 1..] {
                if b != a && b != HUMAN && b != BARBARIANS && self.map_distance(x, y, bx, by) <= 1 {
                    self.diplomacy
                        .entry(Self::relation_key(a, b))
                        .or_default()
                        .first_contact
                        .get_or_insert(self.turn);
                }
            }
        }
        let pairs: Vec<_> = self.diplomacy.keys().copied().collect();
        for (a, b) in pairs {
            if !self.player_alive(a) || !self.player_alive(b) {
                continue;
            }
            let pact = self.pact(a, b);
            let enemies = (0..self.players.len())
                .filter(|&third| {
                    third != a
                        && third != b
                        && third != BARBARIANS
                        && self.player_alive(third)
                        && self.at_war(a, third)
                        && self.at_war(b, third)
                })
                .count() as i32;
            let relation = self.diplomacy.get_mut(&(a, b)).unwrap();
            for (index, trust) in relation.trust.iter_mut().enumerate() {
                let change = if pact == Pact::War {
                    -150
                } else if relation.cause_until[index] > self.turn {
                    -1800
                } else {
                    150 + 100 * i32::from(pact == Pact::Alliance) + 75 * enemies
                };
                let updated = *trust + change;
                *trust = (updated - updated * 4 / 100).clamp(-10_000, 10_000);
            }
            let proposed = self.next_pact(a, b).filter(|&pact| {
                self.pact_refusal(b, a, pact).is_none()
                    && (a == HUMAN || self.pact_refusal(a, b, pact).is_none())
            });
            let relation = self.diplomacy.get(&(a, b)).unwrap();
            if relation.offer.is_some() || self.turn < relation.offer_after {
                continue;
            }
            let offer = self.build_offer(a, b, proposed);
            if offer.pact.is_none() && !offer.bundled() {
                continue;
            }
            let relation = self.diplomacy.get_mut(&(a, b)).unwrap();
            relation.offer_after = self.turn
                + match proposed {
                    Some(Pact::Ceasefire) => 9,
                    Some(Pact::Peace) => 12,
                    _ => 13,
                };
            if a == HUMAN {
                relation.offer = Some(offer);
                let what = if offer.bundled() {
                    "a treaty bundle".to_owned()
                } else {
                    proposed.unwrap().name().to_lowercase()
                };
                self.add_event(format!("{} offers {what}.", self.players[b].name));
            } else {
                self.execute_offer(a, b, offer);
            }
        }
    }

    // The desktop self-test advances treaty timers with the armies held in place.
    pub(crate) fn diplomacy_playtest_tick(&mut self, turns: i32) {
        for _ in 0..turns {
            self.turn += 1;
            self.advance_diplomacy();
        }
    }

    pub(crate) fn diplomacy_playtest_technologies(&mut self, exchange: bool) {
        self.players[HUMAN].known = vec![Tech::ALPHABET];
        self.players[1].known = vec![Tech::ALPHABET];
        if exchange {
            let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
            self.players[HUMAN].known.push(Tech::POTTERY);
            self.players[1].known.push(writing);
            self.players[HUMAN].gold = 100;
            self.players[1].last_science = 4;
        }
    }

    pub(crate) fn diplomacy_playtest_allied_exchange(&mut self) {
        let currency = Tech::all().find(|tech| tech.name() == "Currency").unwrap();
        self.players[HUMAN]
            .known
            .extend([Tech::BRONZE_WORKING, currency]);
        self.players[1]
            .known
            .extend([Tech::BRONZE_WORKING, Tech::WARRIOR_CODE]);
        self.players[1].rates = [40, 60, 0];
        if let Some(city) = self.cities.iter_mut().find(|city| city.owner == 1) {
            city.size = 3;
            city.specialists = [0, 3, 0];
        }
        self.diplomacy_playtest_tick(13);
    }

    pub(crate) fn diplomacy_playtest_hidden_city(&self, owner: usize) -> Option<(i32, i32)> {
        self.cities
            .iter()
            .find(|city| city.owner == owner && !self.tile(city.x, city.y).visible)
            .map(|city| (city.x, city.y))
    }

    pub(crate) fn diplomacy_playtest_unit(&mut self, rival: usize) -> Option<(u32, i32, i32)> {
        let city = self.cities.iter().find(|city| city.owner == rival)?;
        let (x, y) = (city.x, city.y);
        let (start_x, start_y) = offset_native_by_map(x, y, -1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(start_x, start_y).terrain = Terrain::Grassland;
        let id = self.spawn_unit(HUMAN, UnitKind::Warrior, start_x, start_y);
        Some((id, x, y))
    }

    pub(super) fn treaty_blocks_move(&self, unit: &Unit, x: i32, y: i32) -> bool {
        let blocked = |owner| !self.allied(unit.owner, owner) && !self.at_war(unit.owner, owner);
        let occupied = self
            .units
            .iter()
            .any(|other| (other.x, other.y) == (x, y) && blocked(other.owner))
            || self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (x, y) && blocked(city.owner));
        occupied
            || self
                .tile(x, y)
                .owner
                .is_some_and(|owner| self.pact(unit.owner, owner) == Pact::Peace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn technology_price_discounts_allies_before_adding_leakage() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[1].known = vec![Tech::POTTERY];
        game.players[HUMAN].known.clear();
        game.players[2].known.clear();
        assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), Some(45));
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), Some(22));
        game.sign_pact(HUMAN, 2, Pact::Alliance);
        let other = game.diplomacy.entry((1, 2)).or_default();
        other.pact = Pact::Peace;
        other.trust[0] = 1000;
        assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), Some(33));
        game.sign_pact(1, 2, Pact::Ceasefire);
        assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), None);
    }

    #[test]
    fn low_science_can_make_a_balanced_offer_unacceptable_to_the_ai() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.diplomacy_playtest_technologies(true);
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
        let offer = game.build_offer(HUMAN, 1, None);
        assert!(offer.terms.iter().all(|term| term.technology.is_some()));
        assert!(game.ai_accepts_technology_exchange(HUMAN, 1, &offer.terms));
        game.players[1].last_science = 0;
        assert!(!game.ai_accepts_technology_exchange(HUMAN, 1, &offer.terms));
        game.players[1].research = -1;
        assert!(!game.ai_accepts_technology_exchange(HUMAN, 1, &offer.terms));
    }

    #[test]
    fn stale_bundle_applies_no_gold_technology_pact_or_vision() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.create_city(1, 20, 20);
        game.diplomacy_playtest_technologies(true);
        let key = Simulation::relation_key(HUMAN, 1);
        let relation = game.diplomacy.entry(key).or_default();
        relation.pact = Pact::Peace;
        relation.trust = [3000; 2];
        let offer = game.build_offer(HUMAN, 1, Some(Pact::Alliance));
        assert!(offer.terms[0].gold > 0 && offer.terms[1].technology.is_some());
        game.diplomacy.get_mut(&key).unwrap().offer = Some(offer);
        game.players[HUMAN].gold = offer.terms[0].gold - 1;
        let balances = [game.players[HUMAN].gold, game.players[1].gold];
        assert!(!game.apply(Command::AcceptOffer(1, offer)));
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        assert_eq!([game.players[HUMAN].gold, game.players[1].gold], balances);
        assert!(!game.has_tech(HUMAN, offer.terms[1].technology.unwrap()));
        assert_eq!(game.vision_recipients(1), vec![1]);
    }

    #[test]
    fn an_armistice_disbands_units_that_fail_to_withdraw() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.create_city(1, 20, 20);
        let soldier = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 20);
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.diplomacy_playtest_tick(15);
        assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
        assert!(game.units.iter().any(|unit| unit.id == soldier));
        game.diplomacy_playtest_tick(1);
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        assert!(!game.units.iter().any(|unit| unit.id == soldier));
    }

    #[test]
    fn breaking_an_alliance_removes_visitors_from_former_allied_cities() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.create_city(1, 20, 20);
        let soldier = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 20);
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        assert!(game.apply(Command::CancelPact(1)));
        assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == soldier && (unit.x, unit.y) != (20, 20))
        );
    }

    #[test]
    fn senate_enforces_treaties_unless_provoked_or_waived() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.sign_pact(HUMAN, 1, Pact::Ceasefire);
        game.players[HUMAN].government = Government::Federation;
        assert!(!game.apply(Command::CancelPact(1)));
        game.record_provocation(HUMAN, 1);
        assert!(game.apply(Command::CancelPact(1)));
        assert!(game.at_war(HUMAN, 1));

        let find = |name| {
            Building::all()
                .find(|building| building.data().name == name)
                .unwrap()
        };
        game.sign_pact(HUMAN, 1, Pact::Ceasefire);
        game.cities[0].buildings.push(find("Statue of Liberty"));
        assert!(game.apply(Command::CancelPact(1)));

        game.cities[0]
            .buildings
            .retain(|building| building.data().name != "Statue of Liberty");
        game.players[HUMAN].government = Government::Tribal;
        game.cities[0].buildings.push(find("United Nations"));
        game.completed_wonders.push(find("Manhattan Project"));
        let fission = Tech::all()
            .find(|tech| tech.name() == "Nuclear Fission")
            .unwrap();
        game.players[HUMAN].known.push(fission);
        assert!(game.has_senate(HUMAN));
        game.apollo_completed = true;
        assert!(!game.has_senate(HUMAN));
    }
}
