use super::*;

impl Simulation {
    /// Controlled war-ending scenario for the diplomacy interaction gallery.
    pub(crate) fn ui_review_compensation_negotiation(&mut self, other: usize) {
        self.players[HUMAN].gold = 250;
        let relation = self
            .diplomacy
            .get_mut(&Self::relation_key(HUMAN, other))
            .expect("the diplomacy fixture establishes contact");
        relation.pact = Pact::Ceasefire;
        relation.until = self.turn + 4;
        relation.trust[usize::from(other > HUMAN)] = 1000;
    }

    /// Worst-case concessions for the isolated, opt-in screenshot gallery.
    pub(crate) fn ui_review_offer(&mut self) -> TreatyOffer {
        let mut terms = Concessions {
            gold: 12345,
            technology: Some(Tech::AMPHIBIOUS_WARFARE),
            access: 3,
        };
        terms = terms
            .with_embassy()
            .with_city(self.cities_owned_by(1).next().unwrap().id);
        let offer = TreatyOffer {
            pact: Some(Pact::Alliance),
            terms: [terms; 2],
        };
        self.diplomacy
            .get_mut(&Self::relation_key(HUMAN, 1))
            .unwrap()
            .offer = Some(offer);
        offer
    }

    pub(crate) fn ui_review_contacts(&mut self) {
        for other in 1..PLAYER_COUNT {
            self.make_diplomatic_contact(HUMAN, other);
            // Established peace lets the gallery exercise city purchase controls.
            let relation = self
                .diplomacy
                .get_mut(&Self::relation_key(HUMAN, other))
                .unwrap();
            relation.pact = Pact::Peace;
            relation.until = 0;
        }
    }

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
        relation.trust[1] = 2500;
        relation.cause_until[1] = 0;
        self.ai_war_plans.retain(|&(owner, _), _| owner != 1);
        self.units
            .retain(|unit| unit.owner != 1 || unit.kind.stats().attack == 0);
        for city in self.cities.iter_mut().filter(|city| city.owner == 1) {
            city.production = Production::Unit(UnitKind::Settlers);
        }
        self.make_diplomatic_contact(HUMAN, 1);
        self.diplomacy_playtest_alternate_contact();
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
            && self.execute_offer(HUMAN, 1, gift)
            && self.has_tech(1, Tech::POTTERY);

        // A human can knowingly give vision that leaks to a third party.
        self.diplomacy.get_mut(&(HUMAN, 2)).unwrap().pact = Pact::Peace;
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        self.diplomacy.get_mut(&(1, 2)).unwrap().vision[0] = true;
        let mut vision = TreatyOffer::default();
        vision.terms[0].access = 1;
        let shared = !self.vision_safe(HUMAN, 1) && self.execute_offer(HUMAN, 1, vision);
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
        let compensated = !self.execute_offer(HUMAN, 1, alliance) && {
            alliance.terms[0].gold = 2312;
            self.execute_offer(HUMAN, 1, alliance)
        };
        self.players[1].government = Government::Democracy;
        self.players[1].revolution_target = None;
        self.players[1].revolution_turn = Some(self.turn - 10);
        self.declare_prepared_ai_war(1, HUMAN, WarReason::Hatred);
        let pending = !self.at_war(1, HUMAN)
            && !self.ai_on_war_footing(1)
            && self.ai_war_countdown(1, HUMAN) == -20
            && self.ai_pending_war_targets.get(&1) == Some(&HUMAN)
            && self.players[1]
                .revolution_turn
                .is_some_and(|finish| finish > self.turn);
        let resumed = self.change_government(1, Government::Despotism)
            && self.at_war(1, HUMAN)
            && !self.ai_pending_war_targets.contains_key(&1);
        received && shared && preserved && compensated && pending && resumed
    }

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

    pub(crate) fn embassy_contact_playtest(&mut self) -> bool {
        self.units.clear();
        self.turn = 30;
        for (owner, (x, y)) in [(HUMAN, (8, 24)), (1, (18, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.diplomacy.remove(&(HUMAN, 1));
        self.diplomatic_contact_until.remove(&(HUMAN, 1));
        let adjacent = offset_native_by_map(18, 24, 1, 0);
        self.tile_mut(adjacent.0, adjacent.1).terrain = Terrain::Grassland;
        let visitor = self.spawn_unit(HUMAN, UnitKind::Diplomat, adjacent.0, adjacent.1);
        let met =
            self.can_negotiate(1) && self.diplomatic_contact_until[&(HUMAN, 1)] == self.turn + 21;
        self.units.retain(|unit| unit.id != visitor);
        self.diplomacy_playtest_tick(21);
        self.decline_offer(1);
        let expired = !self.can_negotiate(1) && !self.negotiate(1, Pact::Ceasefire);
        self.spawn_unit(HUMAN, UnitKind::Diplomat, adjacent.0, adjacent.1);
        let renewed = self.can_negotiate(1);
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        self.players[HUMAN].known = vec![Tech::ALPHABET];
        self.players[1].known = vec![Tech::ALPHABET];
        let offer = self.source_allied_offer(1, HUMAN);
        let clauses = offer.terms.iter().all(|terms| terms.embassy());
        self.diplomacy.get_mut(&(HUMAN, 1)).unwrap().offer = Some(offer);
        let accepted = self.apply(Command::AcceptOffer(1, offer));
        self.diplomatic_contact_until.clear();
        met && expired
            && renewed
            && clauses
            && accepted
            && self.has_real_embassy(HUMAN, 1)
            && self.has_real_embassy(1, HUMAN)
            && self.can_negotiate(1)
    }

    pub(crate) fn ai_incident_action_playtest(&mut self) -> bool {
        self.units.clear();
        let (actor, target) = self.bribe_playtest_setup();
        self.make_diplomatic_contact(HUMAN, 1);
        self.sign_pact(HUMAN, 1, Pact::Peace);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::Peace;
        relation.until = 0;
        relation.trust[1] = 0;
        let actor_unit = self.unit_by_id(actor).unwrap();
        let target_unit = self.unit_by_id(target).unwrap();
        let Some(price) = self.bribe_price(actor_unit, target_unit) else {
            return false;
        };
        self.players[HUMAN].gold = price;
        let stale = !self.apply(Command::BribeUnit(actor, target, price + 1))
            && self.diplomacy[&(HUMAN, 1)].trust[1] == 0;
        let bribed = self.apply(Command::BribeUnit(actor, target, price));
        stale
            && bribed
            && self
                .units
                .iter()
                .any(|unit| unit.kind == UnitKind::Warrior && unit.owner == HUMAN)
            && self.diplomacy[&(HUMAN, 1)].trust[1] == -1420
            && self.diplomacy[&(HUMAN, 1)].cause_until[1] == self.turn + 2
    }

    pub(crate) fn ai_observer_incident_cadence_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 24)), (1, (18, 24)), (2, (28, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.turn = 30;
        let distant = self.spawn_unit(HUMAN, UnitKind::Warrior, 26, 24);
        self.tile_mut(26, 24).known_by[1] = true;
        self.tile_mut(26, 24).owner = Some(1);
        let fogged = !self.unit_visible_to(1, self.unit_by_id(distant).unwrap());
        let hidden = self.spawn_unit(HUMAN, UnitKind::Submarine, 20, 24);
        let stealth = !self.unit_visible_to(1, self.unit_by_id(hidden).unwrap());
        self.spawn_unit(1, UnitKind::Destroyer, 19, 23);
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
        let accepted = self.execute_offer(HUMAN, 1, gift);
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

    pub(crate) fn city_bundle_playtest(&mut self) -> bool {
        self.units.clear();
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
        self.diplomacy.entry(key).or_default().pact = Pact::Peace;
        self.players[HUMAN].known.clear();
        self.players[1].known.clear();
        self.players[HUMAN].gold = 20_000;
        self.make_diplomatic_contact(HUMAN, 1);
        let offer = self.build_offer(HUMAN, 1, None);
        if offer.terms[1].city() != Some(self.cities[2].id)
            || offer.terms[1].map() != Some(TreatyMap::World)
        {
            return false;
        }

        self.cities[2].buildings.push(Building::PALACE);
        self.diplomacy.get_mut(&key).unwrap().offer = Some(offer);
        let rejected = !self.apply(Command::AcceptOffer(1, offer))
            && self.players[HUMAN].gold == 20_000
            && self.cities[2].owner == 1
            && !self.tile(point.0, point.1).seen;
        self.cities[2]
            .buildings
            .retain(|building| *building != Building::PALACE);
        self.diplomacy.get_mut(&key).unwrap().offer = Some(offer);
        rejected
            && self.apply(Command::AcceptOffer(1, offer))
            && self.players[HUMAN].gold == 20_000 - offer.terms[0].gold
            && self.cities[2].owner == HUMAN
            && self.tile(point.0, point.1).seen
    }

    pub(crate) fn ai_map_offer_playtest(&mut self) -> bool {
        self.create_city(HUMAN, 3, 3);
        self.create_city(1, 18, 32);
        let point = (11, 24);
        let tile = self.tile_mut(point.0, point.1);
        tile.terrain = Terrain::Ocean;
        tile.known_by[1] = true;
        tile.known_by[HUMAN] = false;
        tile.visible = false;
        tile.seen = false;
        let relation = self.diplomacy.entry((HUMAN, 1)).or_default();
        relation.pact = Pact::Peace;
        relation.trust = [0; 2];
        self.players[HUMAN].gold = 15;
        let offer = self.build_offer(HUMAN, 1, None);
        if offer.terms[1].map() != Some(TreatyMap::Sea) || offer.terms[0].gold != 15 {
            return false;
        }
        self.diplomacy.get_mut(&(HUMAN, 1)).unwrap().offer = Some(offer);
        self.apply(Command::AcceptOffer(1, offer))
            && self.players[HUMAN].gold == 0
            && self.treaty_offer(1).is_none()
            && self.tile(point.0, point.1).seen
            && !self.tile(point.0, point.1).visible
    }
}
