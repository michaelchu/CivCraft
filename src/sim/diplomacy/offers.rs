use super::*;

impl Simulation {
    pub(crate) fn treaty_offer(&self, other: usize) -> Option<TreatyOffer> {
        self.diplomacy
            .get(&Self::relation_key(HUMAN, other))
            .and_then(|relation| relation.offer)
    }

    pub(crate) fn pact_offer(&self, other: usize) -> Option<Pact> {
        self.treaty_offer(other).and_then(|offer| offer.pact)
    }

    pub(crate) fn can_negotiate(&self, other: usize) -> bool {
        other != HUMAN
            && other != BARBARIANS
            && other < self.players.len()
            && (self.cities.iter().any(|city| city.owner == other)
                || self.units.iter().any(|unit| unit.owner == other))
            && (self.has_embassy(HUMAN, other)
                || self
                    .diplomacy
                    .contains_key(&Self::relation_key(HUMAN, other))
                || self
                    .cities
                    .iter()
                    .any(|city| city.owner == other && self.tile(city.x, city.y).visible)
                || self
                    .units
                    .iter()
                    .any(|unit| unit.owner == other && self.unit_visible_to_human(unit)))
    }

    pub(in crate::sim) fn accept_offer(&mut self, other: usize, offer: TreatyOffer) -> bool {
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

    pub(crate) fn city_trade_price(&self, city: &City) -> Option<i32> {
        let seller = city.owner;
        if !self.can_negotiate(seller)
            || !matches!(self.pact(HUMAN, seller), Pact::Peace | Pact::Alliance)
            || !self.tile(city.x, city.y).known_by[HUMAN]
            || city.has_building(Building::PALACE)
            || city.has_building(Building::ECCLESIASTICAL_PALACE)
        {
            return None;
        }
        Some(self.city_sale_value(city, HUMAN))
    }

    pub(in crate::sim) fn city_sale_value(&self, city: &City, buyer: usize) -> i32 {
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
        let settler = UnitKind::Settlers.stats().cost;
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

    pub(in crate::sim) fn buy_city(&mut self, id: u32, quoted: i32) -> bool {
        let Some(index) = self.cities.iter().position(|city| city.id == id) else {
            return false;
        };
        let city = self.cities[index].clone();
        if self.city_trade_price(&city) != Some(quoted) || self.players[HUMAN].gold < quoted {
            return false;
        }
        self.players[HUMAN].gold -= quoted;
        self.players[city.owner].gold += quoted;
        self.transfer_traded_city(index, HUMAN);
        self.add_event(format!(
            "Bought {} from {} for {quoted} gold.",
            city.name, self.players[city.owner].name
        ));
        true
    }

    pub(in crate::sim) fn buy_map(&mut self, seller: usize, map: TreatyMap) -> bool {
        if !self.can_negotiate(seller) {
            return false;
        }
        let price = self.map_trade_price(seller, HUMAN, map, self.pact(HUMAN, seller));
        let mut offer = TreatyOffer::default();
        offer.terms[0].gold = price;
        offer.terms[1].access = map.flag();
        let key = Self::relation_key(HUMAN, seller);
        let pending = self.diplomacy.get(&key).and_then(|relation| relation.offer);
        if !self.execute_offer(HUMAN, seller, offer) {
            self.add_event("The map trade could not be completed.");
            return false;
        }
        self.diplomacy.get_mut(&key).unwrap().offer = pending;
        self.add_event(format!(
            "Received the {:?} map from {} for {price} gold.",
            map, self.players[seller].name
        ));
        true
    }

    pub(super) fn execute_offer(&mut self, a: usize, b: usize, offer: TreatyOffer) -> bool {
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
            if !terms.valid_access()
                || terms.city().is_some_and(|id| {
                    !matches!(after, Pact::Peace | Pact::Alliance)
                        || self.cities.iter().all(|city| {
                            city.id != id
                                || city.owner != giver
                                || city.has_building(Building::PALACE)
                                || city.has_building(Building::ECCLESIASTICAL_PALACE)
                        })
                })
                || terms.gold < 0
                || self.players[giver].gold < terms.gold
                || terms
                    .technology
                    .is_some_and(|tech| self.tech_sell_price(giver, receiver, tech).is_none())
                || (terms.shared_vision()
                    && (after != Pact::Alliance || !self.vision_safe(giver, receiver)))
            {
                return false;
            }
        }
        if !self.ai_accepts_treaty_exchange(key.0, key.1, &offer.terms, after) {
            return false;
        }
        if let Some(pact) = offer.pact {
            self.sign_pact(a, b, pact);
        }
        let relation = self.diplomacy.entry(key).or_default();
        relation.offer = None;
        for (index, (giver, receiver)) in [(key.0, key.1), (key.1, key.0)].into_iter().enumerate() {
            let terms = offer.terms[index];
            relation.vision[index] |= terms.shared_vision();
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
        for (index, (giver, receiver)) in [(key.0, key.1), (key.1, key.0)].into_iter().enumerate() {
            if let Some(map) = offer.terms[index].map() {
                self.transfer_treaty_map(giver, receiver, map);
            }
        }
        for (index, receiver) in [key.1, key.0].into_iter().enumerate() {
            if let Some(id) = offer.terms[index].city() {
                let city = self.cities.iter().position(|city| city.id == id).unwrap();
                self.transfer_traded_city(city, receiver);
            }
        }
        if offer.terms.iter().any(|terms| terms.shared_vision()) {
            self.refresh_visibility();
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

    pub(super) fn build_offer(&self, a: usize, b: usize, pact: Option<Pact>) -> TreatyOffer {
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
                        && self.ai_accepts_treaty_exchange(a, b, &terms, after)
                    {
                        offer.terms = terms;
                        break 'exchange;
                    }
                }
            }
        }
        // Freeciv values map clauses, but its default AI does not originate
        // them. Use those values when CivCraft proposes a useful map.
        if a == HUMAN && (after == Pact::Alliance || (after == Pact::Peace && pact.is_none())) {
            for map in [TreatyMap::World, TreatyMap::Sea] {
                let useful = self.tiles.iter().any(|tile| {
                    tile.known_by[b]
                        && !tile.known_by[a]
                        && (map == TreatyMap::World || tile.terrain.is_water())
                });
                if !useful {
                    continue;
                }
                let price = self.map_trade_price(b, a, map, after);
                let mut terms = offer.terms;
                terms[0].gold += price;
                terms[1].access |= map.flag();
                if self.players[a].gold >= terms[0].gold
                    && self.ai_accepts_treaty_exchange(a, b, &terms, after)
                {
                    offer.terms = terms;
                    break;
                }
            }
        }
        if a == HUMAN && matches!(after, Pact::Peace | Pact::Alliance) && pact != Some(Pact::Peace)
        {
            for city in self
                .cities
                .iter()
                .filter(|city| city.owner == b && self.city_founders.get(&city.id) == Some(&a))
            {
                let Some(price) = self.city_trade_price(city) else {
                    continue;
                };
                let mut terms = offer.terms;
                terms[0].gold += price;
                terms[1] = terms[1].with_city(city.id);
                if self.players[a].gold >= terms[0].gold
                    && self.ai_accepts_treaty_exchange(a, b, &terms, after)
                {
                    offer.terms = terms;
                    break;
                }
            }
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
