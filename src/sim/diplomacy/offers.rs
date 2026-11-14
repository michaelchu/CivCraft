//! Diplomatic offer creation, validation, and resolution.

use super::*;

impl Simulation {
    pub(crate) fn treaty_offer(&self, other: usize) -> Option<TreatyOffer> {
        self.diplomacy
            .get(&Self::relation_key(HUMAN, other))
            .and_then(|relation| relation.offer.clone())
    }

    pub(crate) fn pact_offer(&self, other: usize) -> Option<Pact> {
        self.treaty_offer(other).and_then(|offer| offer.pact)
    }

    pub(crate) fn can_negotiate(&self, other: usize) -> bool {
        self.diplomatic_meeting_possible(HUMAN, other)
    }

    pub(crate) fn treaty_trade_technologies(&self, seller: usize, buyer: usize) -> Vec<Tech> {
        Tech::all()
            .filter(|&tech| self.tech_sell_price(seller, buyer, tech).is_some())
            .collect()
    }

    pub(crate) fn treaty_trade_cities(&self, seller: usize, receiver: usize) -> Vec<u32> {
        if seller == receiver || seller >= self.players.len() || receiver >= self.players.len() {
            return Vec::new();
        }
        self.cities
            .iter()
            .filter(|city| {
                city.owner == seller
                    && self.tile(city.x, city.y).known_by[receiver]
                    && !city.has_building(Building::PALACE)
                    && !city.has_building(Building::ECCLESIASTICAL_PALACE)
            })
            .map(|city| city.id)
            .collect()
    }

    pub(crate) fn treaty_can_offer_embassy(&self, giver: usize, receiver: usize) -> bool {
        !self.has_real_embassy(receiver, giver)
    }

    pub(crate) fn treaty_can_offer_vision(&self, giver: usize, receiver: usize) -> bool {
        giver != receiver
            && giver < self.players.len()
            && receiver < self.players.len()
            && !self.gives_shared_vision(giver, receiver)
    }

    pub(in crate::sim) fn propose_treaty(&mut self, other: usize, offer: TreatyOffer) -> bool {
        if other == HUMAN || other >= self.players.len() || !self.can_negotiate(other) {
            return false;
        }
        if offer.terms[0].gold > self.players[HUMAN].gold {
            self.add_event("Your treasury cannot cover that treaty offer.");
            return false;
        }
        if let Some(pact) = offer.pact
            && let Some(reason) = self.pact_refusal(other, HUMAN, pact)
        {
            self.add_event(format!("{} declined. {reason}", self.players[other].name));
            return false;
        }
        if !self.execute_offer(HUMAN, other, &offer) {
            if let Some(pact) = offer.pact
                && let Some((terms_balance, _)) = self.ai_treaty_clause_balance(
                    other,
                    HUMAN,
                    &offer.terms[1],
                    &offer.terms[0],
                    pact,
                )
                && let balance = terms_balance + self.pact_clause_value(other, HUMAN, pact)
                && balance < 0
            {
                let additional = (-balance).min(i64::from(i32::MAX));
                self.add_event(format!(
                    "{} requires at least {additional} more gold or equivalent concessions for that treaty.",
                    self.players[other].name
                ));
                return false;
            }
            self.add_event(format!(
                "{} rejected your treaty terms.",
                self.players[other].name
            ));
            return false;
        }
        self.add_event(format!(
            "{} accepted your treaty terms.",
            self.players[other].name
        ));
        true
    }

    pub(in crate::sim) fn accept_offer(&mut self, other: usize, offer: TreatyOffer) -> bool {
        // Native acceptance looks up the existing meeting; contact expiry
        // prevents opening a new meeting, not concluding an existing one.
        if other >= self.players.len()
            || !self.player_alive(other)
            || self.treaty_offer(other).as_ref() != Some(&offer)
        {
            return false;
        }
        if !self.execute_offer(HUMAN, other, &offer) {
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

    pub(crate) fn map_trade_price(
        &self,
        seller: usize,
        buyer: usize,
        map: TreatyMap,
        pact: Pact,
    ) -> i32 {
        if pact == Pact::Alliance {
            return 0;
        }
        let cities = |owner| {
            self.cities
                .iter()
                .filter(|city| city.owner == owner)
                .count() as i32
        };
        let key = Self::relation_key(seller, buyer);
        let trust = self
            .diplomacy
            .get(&key)
            .map_or(0, |relation| relation.trust[usize::from(seller != key.0)]);
        match map {
            TreatyMap::World => {
                (40 * cities(seller).max(1) * if pact == Pact::Peace { 1 } else { 2 }).max(-trust)
            }
            TreatyMap::Sea => (15 * cities(buyer)).max(-7 * trust / 10),
        }
    }

    pub(in crate::sim) fn city_clause_value(&self, city: &City, buyer: usize) -> i32 {
        let value = 15 * self.city_gold_worth(city);
        if self.city_founders.get(&city.id) == Some(&buyer) {
            value / 2
        } else {
            value
        }
    }

    pub(in crate::sim) fn city_gold_worth(&self, city: &City) -> i32 {
        let seller = city.owner;
        let forecast = self.city_forecast(city);
        let settler = UnitKind::Settler.stats().cost;
        let growth: i32 = (1..city.size).map(Self::growth_box).sum();
        let units: i32 = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == seller
                    && unit.home_city == Some(city.id)
                    && (unit.x, unit.y) == (city.x, city.y)
            })
            .map(|unit| {
                let cost = unit.kind.stats().cost;
                if self.production_successor(city, unit.kind).is_some() {
                    cost - cost / 2
                } else {
                    cost
                }
            })
            .sum();
        let buildings: i32 = city
            .buildings
            .iter()
            .map(|&building| {
                let data = building.data();
                data.cost
                    * if self.building_is_obsolete(city, building, None) {
                        1
                    } else if data.genus == crate::rules::Genus::Improvement {
                        2
                    } else {
                        4
                    }
            })
            .sum();
        let output = forecast.food
            + forecast.shields
            + forecast.trade
            + forecast.gross_gold
            + forecast.gross_science
            + forecast.luxury;
        let value =
            2 * (2 * settler + settler * settler / 20) + growth + 10 * output + units + buildings;
        if forecast.disorder {
            3 * value / 4
        } else {
            value
        }
    }

    pub(super) fn execute_offer(&mut self, a: usize, b: usize, offer: &TreatyOffer) -> bool {
        if offer
            .pact
            .is_some_and(|pact| self.pact_refusal(b, a, pact).is_some())
        {
            return false;
        }
        let key = Self::relation_key(a, b);
        let after = offer.pact.unwrap_or_else(|| self.pact(a, b));
        for (index, (giver, receiver)) in [(key.0, key.1), (key.1, key.0)].into_iter().enumerate() {
            let terms = &offer.terms[index];
            if !terms.valid_access()
                || !terms.cities_are_unique()
                || terms.cities().iter().any(|&id| {
                    self.cities.iter().all(|city| {
                        city.id != id
                            || city.owner != giver
                            || city.has_building(Building::PALACE)
                            || city.has_building(Building::ECCLESIASTICAL_PALACE)
                    })
                })
                || terms.gold < 0
                || self.players[giver].gold < terms.gold
                || terms.technology.is_some_and(|tech| {
                    self.tech_trade_quote(giver, receiver, tech, true).is_none()
                })
                || (terms.embassy() && self.has_real_embassy(receiver, giver))
            {
                return false;
            }
        }
        let mut gifts = Vec::new();
        for (side, (owner, other)) in [(key.0, key.1), (key.1, key.0)].into_iter().enumerate() {
            if !self.players[owner].ai {
                continue;
            }
            let Some((mut balance, mut gift)) = self.ai_treaty_clause_balance(
                owner,
                other,
                &offer.terms[side],
                &offer.terms[1 - side],
                after,
            ) else {
                return false;
            };
            if let Some(pact) = offer.pact {
                if self.pact_refusal(owner, other, pact).is_some() {
                    return false;
                }
                let value = self.pact_clause_value(owner, other, pact);
                balance += value;
                gift &= value >= 0;
            }
            if balance < 0 {
                return false;
            }
            if gift && balance > 0 {
                gifts.push((owner, balance));
            }
        }
        if let Some(pact) = offer.pact {
            self.sign_pact(a, b, pact);
        }
        let relation = self.diplomacy.entry(key).or_default();
        relation.offer = None;
        for (index, (giver, receiver)) in [(key.0, key.1), (key.1, key.0)].into_iter().enumerate() {
            let terms = &offer.terms[index];
            relation.vision[index] |= terms.shared_vision();
            relation.embassy[1 - index] |= terms.embassy();
            self.players[giver].gold -= terms.gold;
            self.players[receiver].gold += terms.gold;
        }
        for (index, (donor, receiver)) in [(key.0, key.1), (key.1, key.0)].into_iter().enumerate() {
            if let Some(tech) = offer.terms[index].technology {
                self.transfer_treaty_technology(donor, receiver, tech);
            }
        }
        for (index, (giver, receiver)) in [(key.0, key.1), (key.1, key.0)].into_iter().enumerate() {
            for map in offer.terms[index].maps() {
                self.transfer_treaty_map(giver, receiver, map);
            }
        }
        for (index, receiver) in [key.1, key.0].into_iter().enumerate() {
            for &id in offer.terms[index].cities() {
                let city = self.city_index_by_id(id).unwrap();
                self.transfer_traded_city(city, receiver);
            }
        }
        if offer.terms.iter().any(|terms| terms.shared_vision()) {
            self.refresh_visibility();
        }
        for (owner, balance) in gifts {
            let cities = self
                .cities
                .iter()
                .filter(|city| city.owner == owner)
                .count() as i64;
            let gain = (balance / (cities * 10 + 1)).min(30 * 150) * 100;
            let side = usize::from(owner > if owner == key.0 { key.1 } else { key.0 });
            let love = &mut self.diplomacy.get_mut(&key).unwrap().trust[side];
            *love = (i64::from(*love) + gain).min(i64::from(i32::MAX)) as i32;
        }
        true
    }

    pub(in crate::sim) fn tech_trade_partner_is_dangerous(
        &self,
        owner: usize,
        other: usize,
    ) -> bool {
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

    fn technology_exchange_terms(
        &self,
        proposer: usize,
        recipient: usize,
        after: Pact,
    ) -> [Concessions; 2] {
        let (a, b) = Self::relation_key(proposer, recipient);
        let mut offer = TreatyOffer::default();
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
                let mut terms = [Concessions::default(), Concessions::default()];
                terms[giver].technology = Some(given);
                terms[1 - giver].technology = Some(received);
                terms[giver].gold = balance.max(0);
                terms[1 - giver].gold = (-balance).max(0);
                if self.players[proposer].gold >= terms[giver].gold
                    && self.players[recipient].gold >= terms[1 - giver].gold
                    && self.ai_accepts_treaty_exchange(a, b, &terms, after)
                {
                    offer.terms = terms;
                    break 'exchange;
                }
            }
        }
        offer.terms
    }

    pub(super) fn source_allied_offer(&self, owner: usize, other: usize) -> TreatyOffer {
        let key = Self::relation_key(owner, other);
        let side = usize::from(owner > other);
        let mut offer = TreatyOffer {
            terms: self.technology_exchange_terms(owner, other, Pact::Alliance),
            ..TreatyOffer::default()
        };
        let existing = self.diplomacy[&key].vision;
        let mut gives_vision = existing[side];
        if !gives_vision && self.vision_safe(owner, other) {
            offer.terms[side].access |= 1;
            gives_vision = true;
        }
        if gives_vision && !existing[1 - side] && (other == HUMAN || self.vision_safe(other, owner))
        {
            offer.terms[1 - side].access |= 1;
        }
        if !self.has_embassy(owner, other) {
            offer.terms[1 - side] = offer.terms[1 - side].with_embassy();
        }
        if !self.has_embassy(other, owner) {
            offer.terms[side] = offer.terms[side].with_embassy();
        }
        offer
    }

    #[cfg(test)]
    pub(super) fn build_offer(&self, a: usize, b: usize, pact: Option<Pact>) -> TreatyOffer {
        let mut offer = TreatyOffer {
            pact,
            ..TreatyOffer::default()
        };
        let after = pact.unwrap_or_else(|| self.pact(a, b));
        if matches!(after, Pact::Peace | Pact::Alliance) {
            let proposer = if a == HUMAN { b } else { a };
            let recipient = if proposer == a { b } else { a };
            offer.terms = self.technology_exchange_terms(proposer, recipient, after);
        }
        if after == Pact::Alliance {
            let existing = self
                .diplomacy
                .get(&(a, b))
                .map_or([false; 2], |relation| relation.vision);
            if self.vision_safe(b, a) && self.vision_safe(a, b) {
                offer.terms[0].access |= u64::from(!existing[0]);
                offer.terms[1].access |= u64::from(!existing[1]);
            }
        }
        offer
    }

    pub(in crate::sim) fn decline_offer(&mut self, other: usize) -> bool {
        let Some(relation) = self.diplomacy.get_mut(&Self::relation_key(HUMAN, other)) else {
            return false;
        };
        relation.offer.take().is_some()
    }
}

impl Simulation {
    pub(super) fn advance_diplomatic_offers(&mut self) {
        let mut vision_changed = false;
        for owner in 0..self.players.len() {
            if owner == BARBARIANS || !self.players[owner].ai || !self.player_alive(owner) {
                continue;
            }
            // This precedes target/countdown/contact spam exclusions in the
            // source: an existing unsafe grant must still be withdrawn.
            let revoke: Vec<_> = self
                .diplomacy_neighbors(owner)
                .filter_map(|(other, relation)| {
                    (relation.vision[usize::from(owner > other)]
                        && (!self.allied(owner, other) || !self.vision_safe(owner, other)))
                    .then_some(other)
                })
                .collect();
            for other in revoke {
                self.diplomacy
                    .get_mut(&Self::relation_key(owner, other))
                    .unwrap()
                    .vision[usize::from(owner > other)] = false;
                vision_changed = true;
            }
            let contacts: Vec<_> = (0..self.players.len())
                .filter(|&other| {
                    other != owner
                        && other != BARBARIANS
                        && self.player_alive(other)
                        && (self
                            .diplomacy
                            .get(&Self::relation_key(owner, other))
                            .is_some_and(|relation| relation.first_contact.is_some())
                            || self.has_embassy(owner, other)
                            || self.has_embassy(other, owner))
                })
                .collect();
            // dai_diplomacy_actions retains this target even if its love was
            // above the war threshold. Its offer loop still skips that player.
            let mut target = contacts
                .iter()
                .copied()
                .filter(|&other| {
                    let Some(relation) = self.diplomacy.get(&Self::relation_key(owner, other))
                    else {
                        return false;
                    };
                    relation.first_contact.is_some()
                        && !self.diplomatic_at_war(owner, other)
                        && self.turn - relation.first_contact.unwrap() > 15
                        && relation.trust[usize::from(owner > other)] < 10_000
                })
                .min_by_key(|&other| {
                    self.diplomacy[&Self::relation_key(owner, other)].trust
                        [usize::from(owner > other)]
                });
            for other in contacts {
                if Some(other) == target
                    || self.ai_war_countdown(owner, other) >= 0
                    || !self.diplomatic_meeting_possible(owner, other)
                {
                    continue;
                }
                self.diplomacy
                    .entry(Self::relation_key(owner, other))
                    .or_default();
                let memory = self
                    .ai_diplomatic_memory
                    .entry((owner, other))
                    .or_insert_with(|| DiplomaticMemory::for_pair(owner, other));
                memory.asked_peace = memory.asked_peace.saturating_sub(1).max(0);
                memory.asked_alliance = memory.asked_alliance.saturating_sub(1).max(0);
                memory.asked_ceasefire = memory.asked_ceasefire.saturating_sub(1).max(0);
                memory.spam = memory.spam.saturating_sub(1).max(0);
                if let Some(warning) = self.ai_space_warnings.get_mut(&(owner, other)) {
                    *warning = warning.saturating_sub(1).max(0);
                }
                if memory.spam > 0 && self.players[other].ai {
                    continue;
                }
                if memory.spam == 0 {
                    let spam = if self.allied(owner, other) {
                        6 + self.rand_below(8)
                    } else {
                        3 + self.rand_below(4)
                    };
                    self.ai_diplomatic_memory
                        .get_mut(&(owner, other))
                        .unwrap()
                        .spam = spam;
                }
                let key = Self::relation_key(owner, other);
                let (proposed, offer) = if self.allied(owner, other) {
                    let at_war_with_ally = self
                        .diplomatic_intel(owner, other)
                        .at_war_with_ally
                        .is_some();
                    if at_war_with_ally {
                        continue;
                    }
                    // The original reuses `target` while checking an ally's
                    // failure to fight our enemies. Preserve that loop effect.
                    target = (0..self.players.len()).find(|&third| {
                        third != owner
                            && third != BARBARIANS
                            && self.player_alive(third)
                            && self.diplomatic_at_war(owner, third)
                            && !self.diplomatic_at_war(other, third)
                    });
                    if self.diplomacy[&key].trust[usize::from(owner > other)] <= 5000 {
                        continue;
                    }
                    (None, self.source_allied_offer(owner, other))
                } else {
                    if self.pact(owner, other) != Pact::War
                        && self
                            .diplomatic_intel(owner, other)
                            .at_war_with_ally
                            .is_some()
                    {
                        continue;
                    }
                    let Some(pact) = self.next_pact(owner, other).filter(|&pact| {
                        self.pact_refusal(owner, other, pact).is_none()
                            && self.pact_clause_value(owner, other, pact) >= 0
                    }) else {
                        continue;
                    };
                    let memory = &self.ai_diplomatic_memory[&(owner, other)];
                    let asked = match pact {
                        Pact::Peace => memory.asked_peace,
                        Pact::Alliance => memory.asked_alliance,
                        _ => memory.asked_ceasefire,
                    };
                    if other == HUMAN && asked > 0 {
                        continue;
                    }
                    (
                        Some(pact),
                        TreatyOffer {
                            pact: Some(pact),
                            ..TreatyOffer::default()
                        },
                    )
                };
                if offer.pact.is_none() && !offer.bundled() {
                    continue;
                }
                if let Some(pact) = proposed {
                    let memory = self.ai_diplomatic_memory.get_mut(&(owner, other)).unwrap();
                    if other == HUMAN {
                        match pact {
                            Pact::Peace => memory.asked_peace = 12,
                            Pact::Alliance => memory.asked_alliance = 13,
                            _ => memory.asked_ceasefire = 9,
                        }
                    }
                }
                if other == HUMAN {
                    self.diplomacy.get_mut(&key).unwrap().offer = Some(offer.clone());
                    let what = if offer.bundled() {
                        "a treaty bundle".to_owned()
                    } else {
                        proposed.unwrap().name().to_lowercase()
                    };
                    self.add_event(format!("{} offers {what}.", self.players[owner].name));
                } else {
                    self.execute_offer(key.0, key.1, &offer);
                }
            }
        }
        if vision_changed {
            self.refresh_visibility();
        }
    }
}
