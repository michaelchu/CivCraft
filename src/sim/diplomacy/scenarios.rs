//! Implements scenarios behavior within the diplomacy subsystem.

use super::*;

impl Simulation {
    pub(crate) fn ai_war_planning_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 20)), (1, (12, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.turn = 30;
        self.sign_pact(HUMAN, 1, Pact::Peace);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::Peace;
        relation.until = 0;
        relation.first_contact = Some(1);
        relation.trust[1] = -5000;
        self.advance_ai_war_plans();
        let preparing = self.ai_on_war_footing(1) && self.pact(HUMAN, 1) == Pact::Peace;
        for _ in 0..4 + self.map_size_factor() {
            self.advance_ai_war_plans();
        }
        preparing
            && self.at_war(HUMAN, 1)
            && !self.ai_on_war_footing(1)
            && self.ai_war_countdown(1, HUMAN) == -20
            && self.pact_clause_value(1, HUMAN, Pact::Ceasefire) < 0
    }

    pub(crate) fn diplomacy_playtest_friendship(&mut self) {
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        // Keep the AI above the native alliance clause threshold after
        // multi-turn scenarios have recorded earlier war incidents.
        relation.trust[1] = 6500;
        relation.offer = None;
        relation.cause_until[1] = 0;
        self.ai_war_plans.retain(|&(owner, _), _| owner != 1);
        if let Some(memory) = self.ai_diplomatic_memory.get_mut(&(1, HUMAN)) {
            memory.asked_peace = 0;
            memory.asked_alliance = 0;
            memory.asked_ceasefire = 0;
        }
        self.units
            .retain(|unit| unit.owner != 1 || unit.kind.stats().attack == 0);
        for city in self.cities.iter_mut().filter(|city| city.owner == 1) {
            city.production = Production::Unit(UnitKind::Settler);
        }
        self.make_diplomatic_contact(HUMAN, 1);
        self.diplomacy_playtest_alternate_contact();
    }

    pub(crate) fn diplomacy_playtest_alliance_offer_setup(&mut self) {
        let relation = self.diplomacy.entry((HUMAN, 1)).or_default();
        relation.trust[1] = 10000;
        relation.cause_until[1] = 0;
        relation.offer = None;
        // The desktop scenario follows a capital attack and civil war. Treat
        // this as a fresh alliance contact so the AI's old war-target ranking
        // does not suppress the offer being tested.
        relation.first_contact = Some(self.turn);
        self.ai_war_plans.remove(&(1, HUMAN));
        self.ai_diplomatic_memory
            .entry((1, HUMAN))
            .or_default()
            .asked_alliance = 0;
        self.make_diplomatic_contact(1, HUMAN);
    }

    pub(crate) fn diplomacy_playtest_alliance_offer_diagnostics(&self) -> String {
        let key = Self::relation_key(HUMAN, 1);
        let relation = &self.diplomacy[&key];
        let first_contact_age = relation
            .first_contact
            .map_or(-1, |turn| self.turn.saturating_sub(turn));
        let target = (0..self.players.len())
            .filter(|&other| {
                other != 1
                    && other != BARBARIANS
                    && self.player_alive(other)
                    && self
                        .diplomacy
                        .get(&Self::relation_key(1, other))
                        .is_some_and(|relation| {
                            relation.first_contact.is_some()
                                && !self.diplomatic_at_war(1, other)
                                && self.turn - relation.first_contact.unwrap() > 15
                                && relation.trust[usize::from(1 > other)] < 10_000
                        })
            })
            .min_by_key(|&other| {
                self.diplomacy[&Self::relation_key(1, other)].trust[usize::from(1 > other)]
            });
        format!(
            "pact {:?}, next {:?}, trust {}, clause {}, refusal {:?}, meeting {}, war plan {}, contact age {}, target {:?}",
            relation.pact,
            self.next_pact(HUMAN, 1),
            relation.trust[1],
            self.pact_clause_value(1, HUMAN, Pact::Alliance),
            self.pact_refusal(1, HUMAN, Pact::Alliance),
            self.diplomatic_meeting_possible(1, HUMAN),
            self.ai_war_countdown(1, HUMAN),
            first_contact_age,
            target
        )
    }

    fn diplomacy_playtest_alternate_contact(&mut self) {
        if !self.player_alive(2)
            && let Some((x, y)) = (0..self.tiles.len())
                .map(|index| self.map_wrap.position(index))
                .find(|&(x, y)| {
                    self.cities
                        .iter()
                        .all(|city| self.map_distance(city.x, city.y, x, y) > 5)
                        && self.units.iter().all(|unit| (unit.x, unit.y) != (x, y))
                })
        {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(2, x, y);
        }
        self.make_diplomatic_contact(1, 2);
        self.sign_pact(1, 2, Pact::Peace);
        let relation = self.diplomacy.get_mut(&(1, 2)).unwrap();
        relation.pact = Pact::Peace;
        relation.until = 0;
        relation.first_contact = Some(self.turn - 20);
        relation.trust = [-2000, 6500];
        relation.cause_until = [0; 2];
        self.ai_war_plans.remove(&(1, 2));
        self.ai_war_plans.remove(&(2, 1));
    }

    pub(crate) fn diplomacy_playtest_allied_trade_baseline(&self) -> Option<i32> {
        let currency = Tech::all().find(|tech| tech.name() == "Currency")?;
        Some(
            self.tech_sell_price_with_advisor(HUMAN, 1, currency, false)?
                - self.tech_sell_price_with_advisor(1, HUMAN, Tech::WARRIOR_CODE, false)?,
        )
    }

    pub(crate) fn diplomacy_playtest_allied_sharing(&mut self) {
        // Arrange a trusted established alliance, preserving the technologies
        // and treasuries whose actual exchange is exercised by the UI.
        if !self.allied(HUMAN, 1) {
            return;
        }
        self.make_diplomatic_contact(HUMAN, 1);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.trust[1] = 6500;
        relation.cause_until = [0; 2];
        self.ai_war_plans.remove(&(1, HUMAN));
        self.ai_war_plans.remove(&(HUMAN, 1));
        self.diplomacy_playtest_alternate_contact();
        self.diplomacy_playtest_tick(1);
    }

    pub(crate) fn ai_treaty_policy_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 20)), (1, (12, 24)), (2, (18, 30))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.turn = 30;
        self.ai_war_plans.clear();
        self.make_diplomatic_contact(HUMAN, 1);
        self.sign_pact(HUMAN, 1, Pact::Peace);
        self.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
        self.sign_pact(1, 2, Pact::Alliance);
        self.diplomacy.entry((HUMAN, 2)).or_default().pact = Pact::War;
        self.players[HUMAN].known = vec![Tech::POTTERY];
        self.players[1].known.clear();
        self.players[2].known.clear();
        let quote = self.tech_trade_quote(HUMAN, 1, Tech::POTTERY, true);
        let mut gift = TreatyOffer::default();
        gift.terms[0].technology = Some(Tech::POTTERY);
        let received = quote.is_some_and(|(_, danger)| danger)
            && self.execute_offer(HUMAN, 1, &gift)
            && self.has_tech(1, Tech::POTTERY);

        // A human can knowingly give vision that leaks to a third party.
        self.diplomacy.get_mut(&(HUMAN, 2)).unwrap().pact = Pact::Peace;
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        self.diplomacy.get_mut(&(1, 2)).unwrap().vision[0] = true;
        let mut vision = TreatyOffer::default();
        vision.terms[0].access = 1;
        let shared = !self.vision_safe(HUMAN, 1) && self.execute_offer(HUMAN, 1, &vision);
        self.cancel_pact(HUMAN, 1);
        let preserved = self.diplomacy[&(HUMAN, 1)].vision[0];

        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::Ceasefire;
        relation.until = self.turn + 16;
        relation.trust[1] = 1000;
        self.ai_diplomatic_intel.clear();
        self.players[HUMAN].gold = 2312;
        let mut alliance = TreatyOffer {
            pact: Some(Pact::Alliance),
            ..TreatyOffer::default()
        };
        alliance.terms[0].gold = 2311;
        let compensated = !self.execute_offer(HUMAN, 1, &alliance) && {
            alliance.terms[0].gold = 2312;
            self.execute_offer(HUMAN, 1, &alliance)
        };
        self.players[1].government = Government::Democracy;
        self.declare_prepared_ai_war(1, HUMAN, WarReason::Hatred);
        let pending = !self.at_war(1, HUMAN)
            && !self.ai_on_war_footing(1)
            && self.ai_war_countdown(1, HUMAN) == -20
            && self.ai_pending_war_targets.get(&1) == Some(&HUMAN)
            && self.players[1]
                .revolution_turn
                .is_some_and(|finish| finish >= self.turn);
        let resumed = self.change_government(1, Government::Despotism)
            && self.at_war(1, HUMAN)
            && !self.ai_pending_war_targets.contains_key(&1);
        received && shared && preserved && compensated && pending && resumed
    }

    #[cfg(test)]
    pub(crate) fn ai_diplomatic_intelligence_playtest(&mut self) -> bool {
        self.units.clear();
        self.turn = 30;
        for (owner, (x, y)) in [(HUMAN, (8, 20)), (1, (12, 24)), (2, (18, 30))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.sign_pact(HUMAN, 1, Pact::Ceasefire);
        self.make_diplomatic_contact(HUMAN, 1);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.until = self.turn + 4;
        relation.trust[1] = 4000;
        self.sign_pact(HUMAN, 2, Pact::Alliance);
        self.diplomacy.entry((1, 2)).or_default().first_contact = Some(1);
        self.refresh_diplomatic_intelligence();
        let remembered = self.diplomatic_intel(1, HUMAN).allied_with_enemy == Some(2);
        let broken = self.apply(Command::CancelPact(2));
        let held = !self.apply(Command::Negotiate(1, Pact::Peace))
            && self.pact(HUMAN, 1) == Pact::Ceasefire;
        self.refresh_diplomatic_intelligence();
        let agreed = self.apply(Command::Negotiate(1, Pact::Peace))
            && self.pact(HUMAN, 1) == Pact::Armistice;
        remembered && broken && held && agreed
    }

    #[cfg(test)]
    pub(crate) fn ai_observer_incident_cadence_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 24)), (1, (18, 24)), (2, (28, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.turn = 30;
        let distant = self.spawn_unit(HUMAN, UnitKind::Warrior, 2, 24);
        self.tile_mut(2, 24).known_by[1] = true;
        self.tile_mut(2, 24).owner = Some(1);
        let fogged = !self.unit_visible_to(1, self.unit_by_id(distant).unwrap());
        let hidden = self.spawn_unit(HUMAN, UnitKind::Submarine, 22, 24);
        let stealth = !self.unit_visible_to(1, self.unit_by_id(hidden).unwrap());
        self.spawn_unit(1, UnitKind::Destroyer, 21, 23);
        let detected = self.unit_visible_to(1, self.unit_by_id(hidden).unwrap());
        self.make_diplomatic_contact(HUMAN, 1);
        self.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
        self.record_diplomatic_incident(HUMAN, Some(1), DiplomaticIncident::Theft, false);
        let offended = self.diplomacy[&(HUMAN, 1)].trust[1] == -1420;
        self.sign_pact(HUMAN, 1, Pact::Ceasefire);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.until = self.turn + 4;
        relation.trust[1] = 3000;
        self.advance_diplomatic_offers();
        let offered = self.pact_offer(1) == Some(Pact::Peace);
        self.decline_offer(1);
        self.advance_diplomatic_offers();
        let paced = self.pact_offer(1).is_none()
            && self.ai_diplomatic_memory[&(1, HUMAN)].asked_peace == 11;
        fogged && stealth && detected && offended && offered && paced
    }

    #[cfg(test)]
    pub(crate) fn ai_diplomatic_love_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 20)), (1, (12, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.turn = 30;
        self.sign_pact(HUMAN, 1, Pact::Peace);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::Peace;
        relation.until = 0;
        relation.first_contact = Some(1);
        relation.trust[1] = 1000;
        self.advance_ai_love();
        let growth = self.diplomacy[&(HUMAN, 1)].trust[1] == 1100;
        self.record_provocation(1, HUMAN);
        self.advance_ai_love();
        let resentment = self.diplomacy[&(HUMAN, 1)].trust[1] == -1200;
        self.diplomacy.get_mut(&(HUMAN, 1)).unwrap().cause_until[1] = 0;
        self.players[HUMAN].gold = 1000;
        let mut gift = TreatyOffer::default();
        gift.terms[0].gold = 110;
        let accepted = self.execute_offer(HUMAN, 1, &gift);
        growth && resentment && accepted && self.diplomacy[&(HUMAN, 1)].trust[1] == -200
    }

    pub(crate) fn ai_space_strategy_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 20)), (1, (12, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.turn = 30;
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.first_contact = Some(self.turn);
        relation.trust[1] = 5000;
        self.spaceship_playtest_setup();
        self.advance_ai_war_plans();
        let warned = self.diplomacy[&(HUMAN, 1)].trust[1] == 4000
            && self.ai_space_warnings[&(1, HUMAN)] >= 9
            && !self.ai_on_war_footing(1)
            && self.allied(HUMAN, 1);
        self.advance_ai_war_plans();
        let quiet = self.diplomacy[&(HUMAN, 1)].trust[1] == 4000;
        if !self.launch_spaceship(HUMAN) {
            return false;
        }
        self.advance_ai_war_plans();
        warned
            && quiet
            && self.ai_war_countdown(1, HUMAN) == 1
            && self.pact(HUMAN, 1) == Pact::Armistice
    }

    pub(crate) fn city_clause_playtest(&mut self) -> bool {
        self.units.clear();
        // This fixture exercises server-side clause validation and transfer;
        // AI acceptance of city clauses has its own C2C3-specific checks.
        self.players[1].ai = false;
        for (owner, point) in [(HUMAN, (8, 24)), (1, (12, 24)), (1, (14, 24))] {
            self.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
            self.create_city(owner, point.0, point.1);
        }
        let capital = self.cities[1].id;
        let id = self.cities[2].id;
        let value = self.city_clause_value(&self.cities[2], HUMAN);
        if value <= 0 {
            return false;
        }
        let seller_gold = self.players[1].gold;
        self.players[HUMAN].gold = value;
        let mut offer = TreatyOffer::default();
        offer.terms[HUMAN].gold = value;
        offer.terms[1] = offer.terms[1].with_city(id);
        let mut capital_offer = offer.clone();
        capital_offer.terms[1] = capital_offer.terms[1].with_city(capital);
        self.diplomacy
            .entry(Self::relation_key(HUMAN, 1))
            .or_default()
            .pact = Pact::War;
        let rejected = !self.execute_offer(HUMAN, 1, &capital_offer)
            && self.players[HUMAN].gold == value
            && self.cities[1].owner == 1;
        let executed = self.execute_offer(HUMAN, 1, &offer);
        rejected
            && executed
            && self.players[HUMAN].gold == 0
            && self.players[1].gold == seller_gold + value
            && self.cities[2].owner == HUMAN
    }

    pub(crate) fn treaty_map_playtest(&mut self) -> bool {
        self.units.clear();
        self.sign_pact(HUMAN, 1, Pact::Ceasefire);
        let land = (18, 35);
        let sea = self.map_wrap.civ3_neighbor(land.0, land.1, 2).unwrap();
        let unexplored = self.map_wrap.civ3_neighbor(land.0, land.1, 3).unwrap();
        for (point, terrain, known) in [
            (land, Terrain::Grassland, true),
            (sea, Terrain::Sea, true),
            (unexplored, Terrain::Sea, false),
        ] {
            let tile = self.tile_mut(point.0, point.1);
            tile.terrain = terrain;
            tile.known_by[1] = known;
            tile.known_by[HUMAN] = false;
            tile.visible = false;
            tile.seen = false;
        }
        self.create_city(HUMAN, 3, 3);
        // Meet away from the map probes so contact reveals none of the test tiles.
        self.tile_mut(22, 10).terrain = Terrain::Grassland;
        self.create_city(1, 22, 10);
        let envoy = offset_native_by_map(22, 10, 1, 0);
        self.tile_mut(envoy.0, envoy.1).terrain = Terrain::Grassland;
        self.make_diplomatic_contact(HUMAN, 1);

        let price = self.map_trade_price(1, HUMAN, TreatyMap::Sea, Pact::Ceasefire);
        let mut sea_offer = TreatyOffer::default();
        sea_offer.terms[HUMAN].gold = price;
        sea_offer.terms[1] = sea_offer.terms[1].set_map(Some(TreatyMap::Sea));
        self.players[HUMAN].gold = price - 1;
        let refused = !self.execute_offer(HUMAN, 1, &sea_offer)
            && !self.tile(sea.0, sea.1).seen
            && self.players[HUMAN].gold == price - 1;
        self.players[HUMAN].gold = price;
        let paid = self.execute_offer(HUMAN, 1, &sea_offer)
            && self.players[HUMAN].gold == 0
            && self.tile(sea.0, sea.1).seen
            && !self.tile(land.0, land.1).seen;

        self.sign_pact(HUMAN, 1, Pact::Alliance);
        let mut world_offer = TreatyOffer::default();
        world_offer.terms[1] = world_offer.terms[1].set_map(Some(TreatyMap::World));
        let world = self.execute_offer(HUMAN, 1, &world_offer)
            && self.tile(land.0, land.1).seen
            && !self.tile(unexplored.0, unexplored.1).seen
            && !self.tile(sea.0, sea.1).visible
            && !self.tile(land.0, land.1).visible
            && self.vision_recipients(1) == [1];
        refused && paid && world
    }

    pub(crate) fn city_clause_bundle_playtest(&mut self) -> bool {
        self.units.clear();
        // Keep this fixture focused on atomic server validation/transfer;
        // C2C3 AI refusal rules are tested separately.
        self.players[1].ai = false;
        for (owner, (x, y)) in [(HUMAN, (8, 24)), (1, (14, 24)), (HUMAN, (10, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.transfer_traded_city(2, 1);
        for (x, y) in [(16, 24), (20, 24)] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(1, x, y);
        }

        let point = (23, 36);
        let tile = self.tile_mut(point.0, point.1);
        tile.known_by[1] = true;
        tile.known_by[HUMAN] = false;
        tile.seen = false;
        let key = (HUMAN, 1);
        self.diplomacy.entry(key).or_default().pact = Pact::War;
        self.players[HUMAN].known.clear();
        self.players[1].known.clear();
        self.players[HUMAN].gold = 20_000;
        self.make_diplomatic_contact(HUMAN, 1);
        let city_value = self.city_clause_value(&self.cities[2], HUMAN);
        let map_value = self.map_trade_price(1, HUMAN, TreatyMap::World, Pact::War);
        let total = city_value + map_value;
        let mut offer = TreatyOffer::default();
        offer.terms[0].gold = total;
        offer.terms[1] = offer.terms[1]
            .with_city(self.cities[2].id)
            .set_map(Some(TreatyMap::World));

        self.cities[2].buildings.push(Building::PALACE);
        let rejected = !self.execute_offer(HUMAN, 1, &offer)
            && self.players[HUMAN].gold == 20_000
            && self.cities[2].owner == 1
            && !self.tile(point.0, point.1).seen;
        self.cities[2]
            .buildings
            .retain(|building| *building != Building::PALACE);
        rejected
            && self.execute_offer(HUMAN, 1, &offer)
            && self.players[HUMAN].gold == 20_000 - total
            && self.cities[2].owner == HUMAN
            && self.tile(point.0, point.1).seen
    }
}
