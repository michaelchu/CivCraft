use super::*;

impl Simulation {
    pub(crate) fn native_turn_orchestration_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = false;
            tile.hut = false;
            tile.base = None;
            tile.pollution = false;
            tile.fallout = false;
        }
        self.players[HUMAN].known = vec![Tech::BRONZE_WORKING];
        self.create_city(HUMAN, 10, 24);
        // Keep a rival alive so the first end-turn does not end the game.
        self.create_city(1, 22, 40);
        self.cities[0].production = Production::Building(Building::BARRACKS);
        self.cities[0].shields = self.cities[0].production.cost();
        let soldier = self.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
        self.units.last_mut().unwrap().hp = 1;
        self.end_turn();
        let built_after_healing = self.cities[0].has_building(Building::BARRACKS)
            && self
                .units
                .iter()
                .find(|unit| unit.id == soldier)
                .is_some_and(|unit| unit.hp == 5);
        self.end_turn();
        built_after_healing
            && self
                .units
                .iter()
                .find(|unit| unit.id == soldier)
                .is_some_and(|unit| unit.hp == UnitKind::Warrior.stats().hp)
    }

    pub(crate) fn tribal_upkeep_playtest_setup(&mut self) {
        // This scenario specifically exercises shield upkeep. New games use
        // the ruleset's Despotism default, which converts eligible upkeep.
        self.players[HUMAN].government = Government::Tribal;
        self.players[HUMAN].revolution_target = None;
        self.players[HUMAN].revolution_turn = None;
        self.reconcile_city_workers();
    }
    pub(crate) fn ai_ground_invasion_playtest(&mut self) -> bool {
        let (infantry, bomber, city) = self.ai_ground_invasion_playtest_setup();
        let target = self.cities.iter().find(|target| target.id == city).unwrap();
        let guard = self
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && (unit.x, unit.y) == (target.x, target.y))
            .unwrap()
            .clone();
        let actor = self.units.iter().find(|unit| unit.id == infantry).unwrap();
        let distant =
            self.map_distance(actor.x, actor.y, target.x, target.y) > actor.kind.stats().moves;
        self.run_ai_player(1);
        distant
            && self.ai_ground_targets.get(&infantry) == Some(&city)
            && self
                .units
                .iter()
                .any(|unit| unit.id == bomber && unit.moved)
            && self
                .units
                .iter()
                .find(|unit| unit.id == guard.id)
                .is_none_or(|unit| unit.hp < guard.hp)
    }

    pub(crate) fn ai_aircraft_advisor_playtest(&mut self) -> bool {
        let (fighter, victim) = self.ai_aircraft_advisor_playtest_setup();
        let plane = self.units.iter().find(|unit| unit.id == fighter).unwrap();
        let target = self.units.iter().find(|unit| unit.id == victim).unwrap();
        let positive_tactical_value = self.aircraft_target_value(plane, target.x, target.y) > 0.0;
        let production_gated = self.aircraft_production_choice(&self.cities[0]).is_none();
        self.run_ai_player(1);
        positive_tactical_value
            && production_gated
            && !self.units.iter().any(|unit| unit.id == victim)
            && self
                .units
                .iter()
                .any(|unit| unit.id == fighter && unit.owner == 1)
    }

    pub(crate) fn ai_nuclear_advisor_playtest(&mut self) -> bool {
        let (missile, victim) = self.ai_nuclear_advisor_playtest_setup();
        let initial_moves = self
            .units
            .iter()
            .find(|unit| unit.id == missile)
            .unwrap()
            .moves_left;
        self.run_ai_player(1);
        self.units
            .iter()
            .any(|unit| unit.id == missile && unit.moves_left == initial_moves)
            && self.units.iter().any(|unit| unit.id == victim)
    }
    pub(crate) fn ai_merchant_road_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.hut = false;
        }
        let origin = (10, 24);
        let point = |step| offset_native_by_map(origin.0, origin.1, step, 0);
        let (x, y) = point(3);
        self.tile_mut(x, y).river = true;
        self.tile_mut(origin.0, origin.1).river = true;
        self.create_city(HUMAN, x, y);
        self.create_city(1, origin.0, origin.1);
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        let city = self.cities[0].id;
        let home = self.cities[1].id;
        let id = self.spawn_unit(1, UnitKind::Caravan, origin.0, origin.1);
        self.units.last_mut().unwrap().home_city = Some(home);
        let caravan = self.units.last().unwrap().clone();
        self.run_ai_merchant(&caravan);
        let slow = !self.merchant_playtest_route_exists(home, city);
        let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) else {
            return false;
        };
        *unit = caravan.clone();
        for step in 0..=2 {
            let (x, y) = point(step);
            self.tile_mut(x, y).road = true;
        }
        self.run_ai_merchant(&caravan);
        slow && self.merchant_playtest_route_exists(home, city)
    }

    pub(crate) fn ai_threat_route_cost_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Mountains;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.owner = None;
            tile.known_by[1] = true;
        }
        let start = (10, 24);
        let point = |step| offset_native_by_map(start.0, start.1, step, 0);
        let city = point(4);
        self.create_city(1, city.0, city.1);
        let enemy = self.spawn_unit(HUMAN, UnitKind::Horseman, start.0, start.1);
        let arrival = |sim: &Self| {
            sim.city_threat_arrivals(&sim.cities[0])
                .find(|(unit, _, _)| unit.id == enemy)
                .map(|(_, _, turns)| turns)
        };
        let rough = arrival(self).is_none();
        for step in 0..=4 {
            let (x, y) = point(step);
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        let open = arrival(self) == Some(2);
        for step in 0..=4 {
            let (x, y) = point(step);
            self.tile_mut(x, y).road = true;
        }
        // Three neutral road steps cost two fragments each, but the last
        // enemy-owned city step costs six. Native PF floors 12 / 12 to one.
        let hostile_roads = arrival(self) == Some(1);
        let mut friendly = self.units.iter().find(|u| u.id == enemy).unwrap().clone();
        friendly.owner = 1;
        // Compare the same corridor without an enemy occupying its origin.
        self.units.retain(|unit| unit.id != enemy);
        let friendly_roads = self
            .search_route(&friendly, city.0, city.1, false, i32::MAX)
            .is_some_and(|(_, cost)| cost / self.unit_move_rate(&friendly) == 0);
        rough && open && hostile_roads && friendly_roads
    }

    pub(crate) fn city_buyback_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, point) in [(HUMAN, (8, 24)), (1, (12, 24)), (HUMAN, (10, 24))] {
            self.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
            self.create_city(owner, point.0, point.1);
        }
        for point in [(20, 10), (24, 40)] {
            self.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
            self.create_city(1, point.0, point.1);
        }
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        let id = self.cities[2].id;
        self.transfer_traded_city(2, 1);
        self.spawn_unit(HUMAN, UnitKind::Diplomat, 12, 24);
        let discount = self.city_trade_price(&self.cities[2]);
        let founder = self.city_founders.remove(&id);
        let full = self.city_trade_price(&self.cities[2]);
        self.city_founders.insert(id, HUMAN);
        let (Some(discount), Some(full)) = (discount, full) else {
            return false;
        };
        self.players[HUMAN].gold = discount;
        founder == Some(HUMAN)
            && discount == full / 2
            && discount > 0
            && self.apply(Command::BuyCity(id, discount))
            && self.cities[2].owner == HUMAN
            && self.players[HUMAN].gold == 0
            && self.city_founders.get(&id) == Some(&HUMAN)
    }

    pub(crate) fn city_trade_playtest(&mut self) -> bool {
        self.units.clear();
        let capital = (12, 24);
        let town = (10, 24);
        let field = (9, 24);
        for point in [capital, town, field] {
            self.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
        }
        self.create_city(1, capital.0, capital.1);
        self.create_city(1, town.0, town.1);
        for point in [(20, 10), (24, 40)] {
            self.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
            self.create_city(1, point.0, point.1);
        }
        self.tile_mut(3, 3).terrain = Terrain::Grassland;
        self.create_city(HUMAN, 3, 3);
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        self.spawn_unit(HUMAN, UnitKind::Diplomat, capital.0, capital.1);
        self.tile_mut(town.0, town.1).known_by[HUMAN] = true;
        let id = self.cities[1].id;
        let capital_id = self.cities[0].id;
        self.cities[1].size = 3;
        self.cities[1].shields = 7;
        self.cities[1].food = 5;
        self.cities[1].buildings.push(Building::GRANARY);
        let guard = self.spawn_unit(1, UnitKind::Warrior, town.0, town.1);
        let worker = self.spawn_unit(1, UnitKind::Worker, field.0, field.1);
        self.units.last_mut().unwrap().home_city = Some(id);
        let rehome = self.spawn_unit(1, UnitKind::Warrior, capital.0, capital.1);
        self.units.last_mut().unwrap().home_city = Some(id);
        self.sign_pact(1, 2, Pact::Alliance);
        let visitor = self.spawn_unit(2, UnitKind::Warrior, town.0, town.1);
        let allied_visitor = self.spawn_unit(1, UnitKind::Warrior, field.0, field.1);
        let Some(price) = self.city_trade_price(&self.cities[1]) else {
            return false;
        };
        self.players[HUMAN].gold = price;
        let before = self.players[1].gold;
        let rejected = self.city_trade_price(&self.cities[0]).is_none()
            && !self.apply(Command::BuyCity(id, price - 1))
            && self.cities[1].owner == 1;
        let purchased = self.apply(Command::BuyCity(id, price));
        let city = &self.cities[1];
        let displaced = self.units.iter().any(|unit| {
            unit.id == visitor
                && unit.owner == 2
                && (unit.x, unit.y) != town
                && (unit.y - town.1).abs() <= 4
        });
        let allied_stayed = self
            .units
            .iter()
            .any(|unit| unit.id == allied_visitor && unit.owner == 1 && (unit.x, unit.y) == field);
        displaced
            && allied_stayed
            && rejected
            && purchased
            && self.players[HUMAN].gold == 0
            && self.players[1].gold == before + price
            && city.owner == HUMAN
            && city.size == 3
            && city.food == 5
            && city.shields == 7
            && city.has_building(Building::GRANARY)
            && city.domestic_citizens() == 0
            && self
                .units
                .iter()
                .any(|unit| unit.id == guard && unit.owner == HUMAN && unit.home_city.is_none())
            && self
                .units
                .iter()
                .any(|unit| unit.id == worker && unit.owner == HUMAN && unit.home_city == Some(id))
            && self.units.iter().any(|unit| {
                unit.id == rehome && unit.owner == 1 && unit.home_city == Some(capital_id)
            })
    }

    pub(crate) fn treaty_map_playtest(&mut self) -> bool {
        self.sign_pact(HUMAN, 1, Pact::Ceasefire);
        let land = (10, 24);
        let sea = (11, 24);
        let unexplored = (12, 24);
        for (point, terrain, known) in [
            (land, Terrain::Grassland, true),
            (sea, Terrain::Ocean, true),
            (unexplored, Terrain::Ocean, false),
        ] {
            let tile = self.tile_mut(point.0, point.1);
            tile.terrain = terrain;
            tile.known_by[1] = known;
            tile.known_by[HUMAN] = false;
            tile.visible = false;
            tile.seen = false;
        }
        self.create_city(HUMAN, 3, 3);
        // Meet away from the map probes so contact itself reveals none of
        // the tiles whose knowledge must be transferred by the treaty.
        self.tile_mut(22, 10).terrain = Terrain::Grassland;
        self.create_city(1, 22, 10);
        let envoy = offset_native_by_map(22, 10, 1, 0);
        self.tile_mut(envoy.0, envoy.1).terrain = Terrain::Grassland;
        self.spawn_unit(HUMAN, UnitKind::Diplomat, envoy.0, envoy.1);
        let price = self.map_trade_price(1, HUMAN, TreatyMap::Sea, Pact::Ceasefire);
        self.players[HUMAN].gold = price - 1;
        let refused = !self.apply(Command::BuyMap(1, TreatyMap::Sea))
            && !self.tile(sea.0, sea.1).seen
            && self.players[HUMAN].gold == price - 1;
        self.players[HUMAN].gold = price;
        let paid = self.apply(Command::BuyMap(1, TreatyMap::Sea))
            && self.players[HUMAN].gold == 0
            && self.tile(sea.0, sea.1).seen
            && !self.tile(land.0, land.1).seen;
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        let world = self.apply(Command::BuyMap(1, TreatyMap::World))
            && self.tile(land.0, land.1).seen
            && !self.tile(unexplored.0, unexplored.1).seen
            && !self.tile(sea.0, sea.1).visible
            && !self.tile(land.0, land.1).visible
            && self.vision_recipients(1) == [1];
        refused && paid && world
    }

    pub(crate) fn ai_persistent_mission_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.known_by[1] = true;
        }
        let origin = (10, 24);
        let far = offset_native_by_map(origin.0, origin.1, 7, 0);
        let close = offset_native_by_map(origin.0, origin.1, 2, -2);
        self.create_city(1, origin.0, origin.1);
        self.create_city(HUMAN, far.0, far.1);
        let first = self.cities[1].id;
        let agent = self.spawn_unit(1, UnitKind::Diplomat, origin.0, origin.1);
        self.run_ai_player(1);
        let assigned = self.agent_city_targets.get(&agent) == Some(&first);
        self.create_city(HUMAN, close.0, close.1);
        self.update_activities_and_restore();
        self.run_ai_player(1);
        let retained = self.agent_city_targets.get(&agent) == Some(&first);
        self.cities.retain(|city| city.id != first);
        self.update_activities_and_restore();
        self.run_ai_player(1);
        let alternative = self.cities.last().unwrap().id;
        assigned
            && retained
            && (self.agent_city_targets.get(&agent) == Some(&alternative)
                || self.has_embassy(1, HUMAN))
    }

    pub(crate) fn ai_offensive_recruitment_playtest(&mut self) -> bool {
        self.units.clear();
        let home = (10, 24);
        let target = offset_native_by_map(home.0, home.1, 2, 0);
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
        }
        self.create_city(1, home.0, home.1);
        self.create_city(HUMAN, target.0, target.1);
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let id = self.cities[0].id;
        self.players[1].known = vec![Tech::ALPHABET];
        self.players[1].researching = Tech::GUNPOWDER;
        self.players[HUMAN].known = vec![Tech::ALPHABET];
        let unprofitable = self.offensive_agent_production(&self.cities[0]).is_none();
        self.players[HUMAN].known.push(Tech::POTTERY);
        let recruited = self.choose_ai_production(1, id) == Production::Unit(UnitKind::Diplomat);
        let second_home = offset_native_by_map(home.0, home.1, 6, 0);
        let second_target = offset_native_by_map(home.0, home.1, 8, 0);
        self.create_city(1, second_home.0, second_home.1);
        self.cities[0].production = Production::Unit(UnitKind::Diplomat);
        let queued_blocks_duplicate = self.offensive_agent_production(&self.cities[2]).is_none();
        self.cities[0].production = Production::Unit(UnitKind::Warrior);
        self.spawn_unit(1, UnitKind::Diplomat, home.0, home.1);
        let active_blocks_duplicate = self.offensive_agent_production(&self.cities[0]).is_none();
        self.create_city(HUMAN, second_target.0, second_target.1);
        let distinct_target =
            self.offensive_agent_production(&self.cities[2]) == Some(UnitKind::Diplomat);
        unprofitable
            && recruited
            && queued_blocks_duplicate
            && active_blocks_duplicate
            && distinct_target
    }

    pub(crate) fn ai_city_sabotage_playtest(&mut self) -> bool {
        self.units.clear();
        let target = (10, 24);
        let approach = offset_native_by_map(target.0, target.1, -1, 0);
        for (x, y) in [target, approach] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        self.create_city(HUMAN, target.0, target.1);
        self.create_city(1, approach.0, approach.1);
        self.players[HUMAN].known = vec![Tech::ALPHABET];
        self.players[1].known = vec![Tech::ALPHABET];
        let id = self.cities[0].id;
        let envoy = self.spawn_unit(1, UnitKind::Diplomat, approach.0, approach.1);
        if !self.establish_embassy(envoy, id) {
            return false;
        }
        self.cities[0].shields = 10;
        self.spawn_unit(1, UnitKind::Diplomat, approach.0, approach.1);
        let agent = self.units.last().unwrap().clone();
        self.sign_pact(1, HUMAN, Pact::Peace);
        let abstained = !self.ai_city_espionage(&agent) && self.cities[0].shields == 10;
        self.sign_pact(1, HUMAN, Pact::War);
        self.set_random_seed(1);
        self.run_ai_player(1);
        abstained && self.cities[0].shields == 0 && self.cities[0].has_building(Building::PALACE)
    }

    pub(crate) fn ai_incite_revolt_playtest(&mut self) -> bool {
        self.units.clear();
        let target = (10, 24);
        let home = offset_native_by_map(target.0, target.1, -1, 0);
        let capital = offset_native_by_map(target.0, target.1, 6, 0);
        for (x, y) in [target, home, capital] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        self.create_city(HUMAN, capital.0, capital.1);
        self.create_city(HUMAN, target.0, target.1);
        self.create_city(1, home.0, home.1);
        self.players[HUMAN].known = vec![Tech::ALPHABET];
        self.players[1].known = vec![Tech::ALPHABET];
        let agent = self.spawn_unit(1, UnitKind::Diplomat, home.0, home.1);
        let target_id = self.cities[1].id;
        if !self.establish_embassy(agent, target_id) {
            return false;
        }
        self.spawn_unit(1, UnitKind::Diplomat, home.0, home.1);
        self.refresh_visibility();
        let diplomat = self.units.last().unwrap().clone();
        let Some(price) = self.incite_price(&diplomat, &self.cities[1]) else {
            return false;
        };
        self.players[1].gold = price - 1;
        let declined = !self.ai_city_espionage(&diplomat) && self.cities[1].owner == HUMAN;
        self.players[1].gold = price;
        self.set_random_seed(1);
        self.run_ai_player(1);
        declined
            && self.cities[1].owner == 1
            && self.cities[0].owner == HUMAN
            && self.players[1].gold == 0
    }

    pub(crate) fn ai_city_mission_playtest(&mut self) -> bool {
        self.units.clear();
        let target = (10, 24);
        let start = offset_native_by_map(target.0, target.1, -2, 0);
        let approach = offset_native_by_map(target.0, target.1, -1, 0);
        for (x, y) in [target, start, approach] {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        self.create_city(HUMAN, target.0, target.1);
        self.create_city(1, start.0, start.1);
        self.players[HUMAN].known = vec![Tech::POTTERY];
        self.players[1].known = vec![Tech::ALPHABET];
        self.spawn_unit(1, UnitKind::Diplomat, start.0, start.1);
        self.refresh_visibility();
        self.run_ai_player(1);
        let embassy = self.has_embassy(1, HUMAN) && !self.has_embassy(HUMAN, 1);
        self.spawn_unit(1, UnitKind::Diplomat, start.0, start.1);
        self.set_random_seed(1);
        self.run_ai_player(1);
        embassy && self.has_tech(1, Tech::POTTERY) && self.cities[0].technology_thefts == 1
    }

    pub(crate) fn ai_counterintelligence_playtest(&mut self) -> bool {
        self.units.clear();
        let city = (10, 24);
        let friendly = offset_native_by_map(city.0, city.1, -1, 0);
        let enemy = offset_native_by_map(city.0, city.1, 1, 0);
        for (x, y) in [city, friendly, enemy] {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        self.create_city(1, city.0, city.1);
        self.players[1].known.retain(|&tech| tech != Tech::ALPHABET);
        self.players[1].research_goal = Tech::POTTERY;
        self.spawn_unit(1, UnitKind::Warrior, city.0, city.1);
        self.spawn_unit(HUMAN, UnitKind::Diplomat, enemy.0, enemy.1);
        self.refresh_visibility();
        self.ai_tech_wants.insert(1, self.advisor_tech_wants(1));
        let researched = self.counterintelligence_tech_want(1) == 3000
            && self.next_ai_research_target(1) == Tech::ALPHABET;
        self.players[1]
            .known
            .extend([Tech::ALPHABET, Tech::ESPIONAGE]);
        self.prepare_ai_city_defenses(1);
        let recruited = self.cities[0].production == Production::Unit(UnitKind::Spy);

        let guard = self.spawn_unit(1, UnitKind::Diplomat, friendly.0, friendly.1);
        let protects = |game: &Self| {
            game.units
                .iter()
                .any(|unit| unit.id == guard && (unit.x, unit.y) == city)
        };
        self.run_ai_player(1);
        let arrived = protects(self);
        self.update_activities_and_restore();
        self.run_ai_player(1);
        researched
            && recruited
            && arrived
            && protects(self)
            && self
                .counterintelligence_production(&self.cities[0])
                .is_none()
    }

    pub(crate) fn ai_bribery_playtest(&mut self) -> bool {
        self.units.clear();
        let start = (10, 24);
        let adjacent = offset_native_by_map(start.0, start.1, 1, 0);
        for (x, y) in [start, adjacent] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.tile_mut(x, y).hut = false;
        }
        self.create_city(1, start.0, start.1);
        let home = self.cities[0].id;
        let actor = self.spawn_unit(1, UnitKind::Diplomat, start.0, start.1);
        self.units.last_mut().unwrap().home_city = Some(home);
        let diplomat = self.units.last().unwrap().clone();
        let target = self.spawn_unit(HUMAN, UnitKind::Worker, adjacent.0, adjacent.1);
        self.refresh_visibility();
        self.ai_purchase_reserve.insert(1, 100);
        let Some(price) = self.bribe_price(&diplomat, self.units.last().unwrap()) else {
            return false;
        };
        self.players[1].gold = price + 99;
        let declined = !self.ai_bribe_nearby(&diplomat) && self.players[1].gold == price + 99;

        let victim = self.units.last_mut().unwrap();
        victim.kind = UnitKind::Horseman;
        victim.hp = UnitKind::Horseman.stats().hp;
        let Some(price) = self.bribe_price(&diplomat, self.units.last().unwrap()) else {
            return false;
        };
        self.players[1].gold = price;
        self.ai_bribe_nearby(&diplomat);
        declined
            && self.players[1].gold == 0
            && self.units.iter().all(|unit| unit.id != target)
            && self.units.iter().any(|unit| {
                unit.owner == 1 && unit.kind == UnitKind::Horseman && unit.home_city == Some(home)
            })
            && self
                .units
                .iter()
                .any(|unit| unit.id == actor && (unit.x, unit.y) == adjacent)
    }

    pub(crate) fn ai_reserve_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_war_plans.clear();
        self.ai_purchase_reserve.clear();
        self.ai_government_cache.clear();
        self.players[1].government = Government::Despotism;
        self.players[1].known = vec![Tech::EXPLOSIVES];
        self.players[1].researching = Tech::BRONZE_WORKING;
        self.players[1].research = 0;
        self.players[1].last_science = 0;
        self.players[1].rates = [60, 40, 0];
        let home = (10, 24);
        for (_, _, x, y) in Self::city_radius_at(home.0, home.1) {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = true;
            tile.road = true;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.hut = false;
        }
        self.create_city(1, home.0, home.1);
        let city = self.cities[0].id;
        // A real balanced 40% tax floor makes the source +10% coffers refill
        // observable. With zero expenses, the 60% science cap already forces
        // 40% tax and hides that increase.
        for _ in 0..5 {
            let id = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
            let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
            unit.home_city = Some(city);
            unit.home_order = id;
        }
        self.spawn_unit(1, UnitKind::Worker, home.0, home.1);
        let Some((_, price)) = self.paid_upgrade(self.units.last().unwrap()) else {
            return false;
        };
        self.players[1].gold = 3;
        self.choose_ai_rates(1);
        let baseline = self.players[1].rates[1];
        self.spend_ai_defense_gold(1);
        self.choose_ai_rates(1);
        let refilling =
            self.ai_gold_reserve(1) == price && self.players[1].rates[1] == baseline + 10;
        self.players[1].gold = price * 2 + 1;
        self.spend_ai_defense_gold(1);
        self.choose_ai_rates(1);
        refilling && self.ai_gold_reserve(1) == 2 && self.players[1].rates[1] == baseline
    }

    pub(crate) fn ai_civilian_upgrade_playtest(&mut self) -> bool {
        self.units.clear();
        let home = (10, 24);
        let field = offset_native_by_map(home.0, home.1, 1, 0);
        self.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
        self.tile_mut(field.0, field.1).terrain = Terrain::Grassland;
        self.create_city(1, home.0, home.1);
        self.players[1]
            .known
            .extend([Tech::EXPLOSIVES, Tech::GUNPOWDER]);
        let worker = self.spawn_unit(1, UnitKind::Worker, home.0, home.1);
        let field_worker = self.spawn_unit(1, UnitKind::Worker, field.0, field.1);
        let guard = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let rival = self.spawn_unit(HUMAN, UnitKind::Explorer, field.0, field.1);
        let kind = |game: &Self, id| game.units.iter().find(|unit| unit.id == id).unwrap().kind;
        let Some((_, price)) =
            self.paid_upgrade(self.units.iter().find(|unit| unit.id == worker).unwrap())
        else {
            return false;
        };
        self.refresh_visibility();
        self.ai_war_plans.insert(
            (1, HUMAN),
            super::diplomacy::WarPlan {
                countdown: 0,
                reason: super::diplomacy::WarReason::Hatred,
            },
        );
        self.players[1].gold = price + 3;
        self.spend_ai_defense_gold(1);
        let wartime = kind(self, worker) == UnitKind::Worker && self.players[1].gold == price + 3;

        self.sign_pact(1, HUMAN, Pact::Peace);
        self.ai_war_plans.remove(&(1, HUMAN));
        self.units.retain(|unit| unit.id != rival);
        self.players[1].gold = price + 2;
        self.spend_ai_defense_gold(1);
        let reserve = kind(self, worker) == UnitKind::Worker && self.players[1].gold == price + 2;

        let replenished = self.ai_gold_reserve(1) + 1;
        self.players[1].gold = price + replenished;
        self.spend_ai_defense_gold(1);
        wartime
            && reserve
            && self.players[1].gold == replenished
            && kind(self, worker) == UnitKind::Engineer
            && kind(self, field_worker) == UnitKind::Worker
            && kind(self, guard) == UnitKind::Warrior
    }

    pub(crate) fn ai_defense_sale_playtest(&mut self) -> bool {
        self.units.clear();
        let front = (10, 24);
        let enemy = offset_native_by_map(front.0, front.1, 1, 0);
        for (x, y) in [front, enemy] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        self.create_city(1, front.0, front.1);
        self.turn += 1;
        self.cities[0]
            .buildings
            .extend([Building::CITY_WALLS, Building::GRANARY]);
        self.spawn_unit(HUMAN, UnitKind::Cannon, enemy.0, enemy.1);
        self.prepare_ai_city_defenses(1);
        let Some(price) = self.rush_cost(&self.cities[0]) else {
            return false;
        };
        let expenses = self.building_upkeep(&self.cities[0]);
        self.players[1].gold = price + expenses - 1;
        self.spend_ai_defense_gold(1);
        let city = &self.cities[0];
        city.bought_production
            && city.sold_building
            && city.has_building(Building::CITY_WALLS)
            && !city.has_building(Building::GRANARY)
            && self.players[1].gold == expenses - 1 + Building::GRANARY.data().cost
    }

    pub(crate) fn ai_defense_purchase_playtest(&mut self) -> bool {
        self.units.clear();
        let front = (10, 24);
        let enemy = offset_native_by_map(front.0, front.1, 1, 0);
        let rear = offset_native_by_map(front.0, front.1, 9, 0);
        for (x, y) in [front, enemy, rear] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        self.create_city(1, front.0, front.1);
        self.create_city(1, rear.0, rear.1);
        self.turn += 1;
        self.spawn_unit(HUMAN, UnitKind::Cannon, enemy.0, enemy.1);
        self.prepare_ai_city_defenses(1);
        let Some(price) = self.rush_cost(&self.cities[0]) else {
            return false;
        };
        self.players[1].gold = price - 1;
        self.spend_ai_defense_gold(1);
        let declined = !self.cities[0].bought_production && self.players[1].gold == price - 1;

        self.players[1].gold = price;
        self.spend_ai_defense_gold(1);
        let purchased = self.cities[0].bought_production
            && !self.cities[1].bought_production
            && self.players[1].gold == 0;
        self.process_cities();
        declined
            && purchased
            && self
                .units
                .iter()
                .any(|unit| unit.owner == 1 && unit.kind.is_military() && (unit.x, unit.y) == front)
    }

    pub(crate) fn fog_route_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.seen = false;
            tile.visible = false;
            tile.road = false;
            tile.railroad = false;
        }
        let start = (10, 24);
        let middle = offset_native_by_map(start.0, start.1, 1, 0);
        let goal = offset_native_by_map(start.0, start.1, 2, 0);
        self.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);
        let warrior = self.units.last().unwrap().clone();
        let route = self.plan_route(&warrior, goal.0, goal.1, true);
        self.tile_mut(middle.0, middle.1).terrain = Terrain::Ocean;
        let terrain_hidden =
            route == [(1, 0), (1, 0)] && self.plan_route(&warrior, goal.0, goal.1, true) == route;
        self.tile_mut(middle.0, middle.1).terrain = Terrain::Grassland;
        self.tile_mut(middle.0, middle.1).seen = true;
        self.spawn_unit(1, UnitKind::Warrior, middle.0, middle.1);
        let unit_hidden = self.plan_route(&warrior, goal.0, goal.1, true) == route;
        self.tile_mut(middle.0, middle.1).visible = true;
        let detour = self.plan_route(&warrior, goal.0, goal.1, true);
        terrain_hidden && unit_hidden && !detour.is_empty() && detour[0] != (1, 0)
    }

    pub(crate) fn remembered_route_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            // The scenario measures a road detour. Generated rivers or
            // hostile infrastructure ownership would change its real cost.
            tile.river = false;
            tile.base = None;
            tile.owner = None;
            tile.last_seen_owner = None;
            tile.visible = true;
            tile.seen = true;
            tile.known_by[HUMAN] = true;
        }
        let start = (10, 24);
        let point = |dx, dy| offset_native_by_map(start.0, start.1, dx, dy);
        let middle = point(1, 0);
        let goal = point(2, 0);
        for (x, y) in [start, point(0, -1), point(1, -1), goal] {
            self.tile_mut(x, y).road = true;
        }
        self.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);
        let warrior = self.units.last().unwrap().clone();
        self.remember_route_tiles(false);
        self.remember_render_map(&[vec![true; self.tiles.len()]]);
        let route = self.plan_route(&warrior, goal.0, goal.1, true);
        let bridge = point(1, -1);
        let tile = self.tile_mut(bridge.0, bridge.1);
        tile.visible = false;
        tile.terrain = Terrain::Ocean;
        tile.road = false;
        let units = std::mem::take(&mut self.units);
        self.create_city(1, middle.0, middle.1);
        self.units = units;
        let remembered =
            route.len() == 3 && self.plan_route(&warrior, goal.0, goal.1, true) == route;
        self.tile_mut(bridge.0, bridge.1).visible = true;
        let updated = self.plan_route(&warrior, goal.0, goal.1, true);
        remembered && updated == [(1, 0), (1, 0)]
    }

    pub(crate) fn road_route_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.hut = false;
            tile.seen = true;
        }
        let start = (10, 24);
        let point = |dx, dy| offset_native_by_map(start.0, start.1, dx, dy);
        let goal = point(2, 0);
        for (x, y) in [start, point(0, -1), point(1, -1), goal] {
            self.tile_mut(x, y).road = true;
        }
        self.remember_route_tiles(true);
        let unit = self.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);
        self.path_to(unit, goal.0, goal.1).len() == 3
            && self.apply(Command::GoTo(unit, goal.0, goal.1))
            && self
                .units
                .iter()
                .any(|unit| (unit.x, unit.y) == goal && unit.moves_left == 0)
    }

    pub(crate) fn nested_cargo_rescue_playtest(&mut self) -> bool {
        let sea = (10, 24);
        self.tile_mut(sea.0, sea.1).terrain = Terrain::Ocean;
        let carrier = self.spawn_unit(HUMAN, UnitKind::Carrier, sea.0, sea.1);
        let helicopter = self.spawn_unit(HUMAN, UnitKind::Helicopter, sea.0, sea.1);
        assert!(self.board_specific_ship(helicopter, carrier));
        for _ in 1..UnitKind::Carrier.transport_capacity() {
            let fighter = self.spawn_unit(HUMAN, UnitKind::Fighter, sea.0, sea.1);
            assert!(self.board_specific_ship(fighter, carrier));
        }
        let transport = self.spawn_unit(HUMAN, UnitKind::Transport, sea.0, sea.1);
        let marine = self.spawn_unit(HUMAN, UnitKind::Marines, sea.0, sea.1);
        assert!(self.board_specific_ship(marine, transport));
        let rescued = self.apply(Command::Disband(transport, None))
            && self
                .units
                .iter()
                .any(|unit| unit.id == marine && unit.transport == Some(helicopter));
        let second = self.spawn_unit(HUMAN, UnitKind::Transport, sea.0, sea.1);
        let partisan = self.spawn_unit(HUMAN, UnitKind::Partisan, sea.0, sea.1);
        assert!(self.board_specific_ship(partisan, second));
        // Free a seat on the helicopter, then lose both surface ships together.
        self.remove_units_with_cargo(vec![marine]);
        self.remove_units_with_cargo(vec![second, carrier]);
        rescued
            && self.units.iter().all(|unit| unit.id != partisan)
            && self
                .units
                .iter()
                .any(|unit| unit.id == helicopter && unit.transport.is_none())
    }

    pub(crate) fn cargo_rescue_playtest(&mut self) -> (bool, bool, bool) {
        let sea = (10, 24);
        let coast = offset_native_by_map(sea.0, sea.1, 1, 0);
        self.tile_mut(sea.0, sea.1).terrain = Terrain::Ocean;
        self.tile_mut(coast.0, coast.1).terrain = Terrain::Grassland;
        let ship = self.spawn_unit(HUMAN, UnitKind::Transport, sea.0, sea.1);
        let backup = self.spawn_unit(HUMAN, UnitKind::Transport, sea.0, sea.1);
        let passenger = self.spawn_unit(HUMAN, UnitKind::Warrior, sea.0, sea.1);
        let boarded = self.board_specific_ship(passenger, ship);
        self.units.last_mut().unwrap().state = UnitState::Sentry;
        let rescued = boarded
            && self.apply(Command::Disband(ship, None))
            && self.units.iter().any(|unit| {
                unit.id == passenger
                    && unit.transport == Some(backup)
                    && unit.state == UnitState::Idle
            });

        let hostile = self.spawn_unit(1, UnitKind::Transport, sea.0, sea.1);
        let drowned = self.apply(Command::Disband(backup, None))
            && self.units.iter().all(|unit| unit.id != passenger)
            && self.cargo_count(hostile) == 0;

        self.create_city(BARBARIANS, coast.0, coast.1);
        let raider = self.spawn_unit(BARBARIANS, UnitKind::Transport, sea.0, sea.1);
        let leader = self.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, sea.0, sea.1);
        let embarked = self.board_specific_ship(leader, raider);
        self.remove_units_with_cargo(vec![raider]);
        let evacuated = embarked
            && self.units.iter().any(|unit| {
                unit.id == leader && (unit.x, unit.y) == coast && unit.transport.is_none()
            });
        (rescued, drowned, evacuated)
    }

    pub(crate) fn disband_playtest(&mut self) -> (bool, bool, bool) {
        let center = (10, 24);
        let adjacent = offset_native_by_map(center.0, center.1, 1, 0);
        let remote = offset_native_by_map(center.0, center.1, 4, 0);
        self.tile_mut(center.0, center.1).terrain = Terrain::Grassland;
        self.create_city(HUMAN, center.0, center.1);
        self.players[HUMAN].known.push(Tech::POTTERY);
        let city = self.cities.last_mut().unwrap();
        let id = city.id;
        let original = city.production;
        city.shields = 8;
        let target = Production::Building(Building::GRANARY);
        let switched = self.apply(Command::Produce(id, target));
        let unit = self.spawn_unit(HUMAN, UnitKind::Warrior, adjacent.0, adjacent.1);
        let contribution = UnitKind::Warrior.stats().cost / 2;
        let recovered = switched
            && self.apply(Command::Disband(unit, Some(id)))
            && self.units.iter().all(|candidate| candidate.id != unit)
            && self.cities.last().unwrap().shields == 4 + contribution
            && self.apply(Command::Produce(id, original))
            && self.cities.last().unwrap().shields == 8 + contribution
            && self.apply(Command::Produce(id, target))
            && self.cities.last().unwrap().shields == 4 + contribution;

        let unit = self.spawn_unit(HUMAN, UnitKind::Warrior, remote.0, remote.1);
        let outside = !self.apply(Command::Disband(unit, Some(id)))
            && self.apply(Command::Disband(unit, None))
            && self.units.iter().all(|candidate| candidate.id != unit)
            && self.cities.last().unwrap().shields == 4 + contribution;

        let ship = self.spawn_unit(HUMAN, UnitKind::Transport, center.0, center.1);
        let passenger = self.spawn_unit(HUMAN, UnitKind::Warrior, center.0, center.1);
        let boarded = self.board_specific_ship(passenger, ship);
        let unloaded = boarded
            && self.apply(Command::Disband(ship, None))
            && self
                .units
                .iter()
                .any(|unit| unit.id == passenger && unit.transport.is_none());
        (recovered, outside, unloaded)
    }

    pub(crate) fn ransom_playtest(&mut self) -> bool {
        self.units.clear();
        let start = (10, 24);
        let target = offset_native_by_map(start.0, start.1, 1, 0);
        for point in [start, target] {
            let tile = self.tile_mut(point.0, point.1);
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        let mut passed = true;
        for (kind, protected, treasury, expected) in [
            (UnitKind::Warrior, false, 300, 200),
            (UnitKind::Warrior, false, 125, 125),
            (UnitKind::Warrior, true, 300, 0),
            (UnitKind::Frigate, false, 300, 0),
        ] {
            self.units.clear();
            self.players[BARBARIANS].gold = treasury;
            let before = self.players[HUMAN].gold;
            let attacker = self.spawn_unit(HUMAN, kind, start.0, start.1);
            for _ in 0..2 {
                self.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, target.0, target.1);
            }
            if protected {
                self.spawn_unit(BARBARIANS, UnitKind::Worker, target.0, target.1);
            }
            passed &= self.try_move_unit(attacker, 1, 0) == MoveResult::Fought
                && self.players[HUMAN].gold == before + expected
                && self.players[BARBARIANS].gold == treasury - expected;
        }
        passed
    }

    pub(crate) fn fleet_escort_playtest(&mut self) -> (bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.known_by[1] = true;
        }
        let sea = (10, 24);
        let guard = offset_native_by_map(sea.0, sea.1, -1, 0);
        let threat = offset_native_by_map(sea.0, sea.1, 1, 0);
        let ship = self.spawn_unit(1, UnitKind::Transport, sea.0, sea.1);
        let passenger = self.spawn_unit(1, UnitKind::Marines, sea.0, sea.1);
        self.units.last_mut().unwrap().transport = Some(ship);
        let escort = self.spawn_unit(1, UnitKind::AegisCruiser, guard.0, guard.1);
        self.request_ai_guard(ship);
        let enemy = self.spawn_unit(HUMAN, UnitKind::Frigate, threat.0, threat.1);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(1);
        self.run_ai_player(1);
        let intercepted = self.units.iter().all(|unit| unit.id != enemy)
            && self
                .units
                .iter()
                .any(|unit| unit.id == escort && (unit.x, unit.y) == threat);
        let protected = self
            .units
            .iter()
            .any(|unit| unit.id == passenger && unit.transport == Some(ship));
        (intercepted, protected)
    }

    pub(crate) fn coordinated_fleet_playtest(&mut self, remote: bool) -> (bool, bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.hut = false;
            tile.resource = None;
        }
        let home = (10, 24);
        let point = |step| offset_native_by_map(home.0, home.1, step, 0);
        let target = point(9);
        for (x, y) in [home, point(8), target] {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.known_by[1] = true;
        }
        self.create_city(1, home.0, home.1);
        self.create_city(HUMAN, target.0, target.1);
        let guard = self.spawn_unit(1, UnitKind::Marines, home.0, home.1);
        for _ in 0..4 {
            self.spawn_unit(1, UnitKind::Marines, home.0, home.1);
        }
        for _ in 0..3 {
            self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
            self.units.last_mut().unwrap().hp = 1;
        }
        let first = self.spawn_unit(1, UnitKind::Trireme, home.0, home.1);
        let second_start = if remote { point(-5) } else { home };
        let second = self.spawn_unit(1, UnitKind::Trireme, second_start.0, second_start.1);
        self.units.last_mut().unwrap().moves_left = 0;
        self.set_random_seed(1);
        self.run_ai_player(1);
        let waited = !remote
            || (self.cargo_count(first) == 0
                && self.cargo_count(second) == 0
                && self
                    .units
                    .iter()
                    .filter(|unit| unit.kind == UnitKind::Marines && (unit.x, unit.y) == home)
                    .count()
                    == 5);
        let mut embarked = self.cargo_count(first) == 2 && self.cargo_count(second) == 2;
        let mut synchronized = false;
        for _ in 0..14 {
            self.update_activities_and_restore();
            self.run_ai_player(1);
            embarked |= self.cargo_count(first) == 2 && self.cargo_count(second) == 2;
            synchronized |= embarked
                && self.cargo_count(first) == 0
                && self.cargo_count(second) == 0
                && self
                    .units
                    .iter()
                    .find(|unit| unit.id == first)
                    .zip(self.units.iter().find(|unit| unit.id == second))
                    .is_some_and(|(a, b)| (a.x, a.y) == (b.x, b.y));
            if self
                .cities
                .iter()
                .any(|city| city.owner == 1 && (city.x, city.y) == target)
            {
                break;
            }
        }
        let captured = self
            .cities
            .iter()
            .any(|city| city.owner == 1 && (city.x, city.y) == target);
        let guarded = self
            .units
            .iter()
            .any(|unit| unit.id == guard && (unit.x, unit.y) == home);
        (embarked && waited, synchronized, captured && guarded)
    }

    pub(crate) fn overseas_assault_playtest(&mut self, defended: bool) -> (bool, bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.hut = false;
            tile.resource = None;
        }
        let home = (10, 24);
        let point = |step| offset_native_by_map(home.0, home.1, step, 0);
        let target = point(9);
        for (x, y) in [home, point(8), target] {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.known_by[1] = true;
        }
        self.create_city(1, home.0, home.1);
        self.create_city(HUMAN, target.0, target.1);
        let guard = self.spawn_unit(1, UnitKind::Marines, home.0, home.1);
        let attacker = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let (x, y) = point(-1);
        let boat = self.spawn_unit(1, UnitKind::Galleon, x, y);
        let mut rejected = true;
        if defended {
            for _ in 0..2 {
                self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
                self.units.last_mut().unwrap().hp = 1;
            }
            rejected = !self.assault_transport_playtest_has_orders(1);
            for _ in 0..2 {
                self.spawn_unit(1, UnitKind::Marines, home.0, home.1);
            }
            self.set_random_seed(1);
        }
        self.run_ai_player(1);
        let embarked = self
            .units
            .iter()
            .any(|unit| unit.id == attacker && unit.transport == Some(boat));
        for _ in 0..7 {
            self.update_activities_and_restore();
            self.run_ai_player(1);
            if self
                .cities
                .iter()
                .any(|city| city.owner == 1 && (city.x, city.y) == target)
            {
                break;
            }
        }
        let guarded = self
            .units
            .iter()
            .any(|unit| unit.id == guard && (unit.x, unit.y) == home);
        let captured = self
            .cities
            .iter()
            .any(|city| city.owner == 1 && (city.x, city.y) == target);
        (embarked && rejected, guarded, captured)
    }

    pub(crate) fn offensive_production_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.resource = None;
        }
        let home = (10, 24);
        let point = |step| offset_native_by_map(home.0, home.1, step, 0);
        for step in 0..=5 {
            let (x, y) = point(step);
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.known_by[1] = true;
        }
        self.create_city(1, home.0, home.1);
        // Keep this scenario about offensive unit choice after the source
        // military advisor's veteran-building substitution.
        self.cities[0].buildings.push(Building::BARRACKS);
        let (x, y) = point(5);
        self.create_city(HUMAN, x, y);
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        self.players[1]
            .known
            .extend([Tech::THE_WHEEL, Tech::MATHEMATICS]);
        self.cities[0].shields = 100;
        let choice = self.choose_ai_production(1, self.cities[0].id);
        let selected = choice == Production::Unit(UnitKind::Chariot);
        let (ix, iy) = point(-2);
        self.tile_mut(ix, iy).terrain = Terrain::Grassland;
        self.tile_mut(ix, iy).known_by[1] = true;
        self.create_city(HUMAN, ix, iy);
        let start = point(1);
        let attacker = self.spawn_unit(1, UnitKind::Chariot, start.0, start.1);
        self.run_ai_player(1);
        let advanced = self
            .units
            .iter()
            .any(|unit| unit.id == attacker && [point(2), point(3)].contains(&(unit.x, unit.y)));
        let (x, y) = point(2);
        self.tile_mut(x, y).terrain = Terrain::Ocean;
        // Native offense may simulate a ferry across a cut land corridor.
        // Exclude the nearer secondary target and block the BigLand seed's
        // target approaches as well; this makes the selected job unreachable.
        let secondary = point(-2);
        self.tile_mut(secondary.0, secondary.1).known_by[1] = false;
        let destination = point(5);
        for (x, y) in super::terrain_work::neighbors(destination.0, destination.1) {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Mountains;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.base = None;
        }
        (
            selected,
            advanced,
            self.offensive_production(&self.cities[0]).is_none(),
        )
    }

    pub(crate) fn domestic_building_playtest(&mut self) -> (bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Glacier;
            tile.resource = None;
            tile.river = false;
        }
        let (x, y) = (10, 24);
        for (_, _, tx, ty) in Self::city_radius_at(x, y) {
            let tile = self.tile_mut(tx, ty);
            tile.terrain = Terrain::Grassland;
            tile.road = true;
            tile.river = true;
            tile.irrigation = true;
        }
        self.create_city(1, x, y);
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        let city = self.cities[0].id;
        self.players[1].government = Government::Republic;
        self.players[1].rates = [80, 20, 0];
        let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
        self.players[1]
            .known
            .extend([writing, Tech::POTTERY, Tech::CONSTRUCTION]);
        self.cities[0].size = 3;
        self.reconcile_city_workers();
        let science = self.choose_ai_production(1, city) == Production::Building(Building::LIBRARY);
        self.cities[0].size = Self::city_size_limit(&self.cities[0]);
        self.cities[0].food = Self::growth_box(self.cities[0].size);
        self.players[1].rates = [30, 10, 60];
        self.reconcile_city_workers();
        let growth =
            self.choose_ai_production(1, city) == Production::Building(Building::AQUEDUCT_RIVER);
        (science, growth)
    }

    pub(crate) fn government_worker_preview_playtest(&mut self) -> (bool, bool) {
        self.units.clear();
        let home = (10, 24);
        for (_, _, x, y) in Self::city_radius_at(home.0, home.1) {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Desert;
            tile.resource = None;
            tile.river = false;
        }
        let poor = offset_native_by_map(home.0, home.1, -1, 0);
        let fertile = offset_native_by_map(home.0, home.1, 1, 0);
        self.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
        self.tile_mut(fertile.0, fertile.1).terrain = Terrain::Grassland;
        self.tile_mut(fertile.0, fertile.1).river = true;
        self.create_city(1, home.0, home.1);
        self.cities[0].worked_tiles = vec![poor];
        self.cities[0].specialists = [0; 3];
        self.players[1].government = Government::Republic;
        let current = self.government_city_value(&self.cities[0]);
        let mut expected = self.cities[0].clone();
        expected.worked_tiles = vec![fertile];
        let best = self.government_city_value(&expected);
        let projected = self.project_government_output(1);
        (
            projected == best && best > current,
            self.cities[0].worked_tiles == [poor] && self.cities[0].specialists == [0; 3],
        )
    }

    pub(crate) fn ai_government_playtest(&mut self) -> (bool, bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.ai_government_cache.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.road = true;
            tile.river = true;
        }
        self.create_city(1, 10, 24);
        self.cities[0].size = 4;
        self.reconcile_city_workers();
        self.players[1].government = Government::Tribal;
        self.players[1].known = self.research_path(1, Tech::MONARCHY);
        self.players[1].known.retain(|&tech| tech != Tech::MONARCHY);
        self.players[1].rates = [60, 40, 0];
        self.turn = 20;
        self.players[1].research_goal = Tech::BRONZE_WORKING;
        self.players[1].researching = Tech::BRONZE_WORKING;
        self.players[1].research = 0;
        self.clear_ai_tech_wants(1);
        let research = self
            .government_research_want(1)
            .is_some_and(|(goal, want)| {
                self.add_ai_tech_want(1, goal, want as f32);
                let chosen = self.next_ai_research_target(1);
                self.manage_ai_research(1);
                goal == Tech::MONARCHY
                    && want >= 100
                    && chosen == goal
                    && self.players[1].research_goal == goal
                    && self.players[1].researching == goal
            });
        self.choose_ai_government(1);
        let locked = self.players[1].revolution_target != Some(Government::Monarchy);
        // Finish any available-government transition before unlocking the goal.
        // Keep the real advisor cache; technology changes availability at once.
        if let Some(finish) = self.players[1].revolution_turn {
            self.turn = finish;
            self.finish_revolutions();
            // The game disallows a second government change in the same
            // turn as adoption; evaluate the new unlock on the next turn.
            self.turn += 1;
        }
        self.players[1].known.push(Tech::MONARCHY);
        self.choose_ai_government(1);
        let chose = self.players[1].revolution_target == Some(Government::Monarchy)
            && self.players[1].government == Government::Anarchy;
        let finish = self.players[1].revolution_turn;
        self.players[1].known.push(Tech::DEMOCRACY);
        self.choose_ai_government(1);
        let kept = self.players[1].revolution_target == Some(Government::Monarchy)
            && self.players[1].revolution_turn == finish;
        if let Some(turn) = finish {
            self.turn = turn;
            self.finish_revolutions();
        }
        (
            research,
            locked,
            chose,
            kept && self.players[1].government == Government::Monarchy,
        )
    }

    pub(crate) fn ai_paradrop_stack_playtest(&mut self) -> (bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by[1] = true;
            tile.hut = false;
            tile.base = None;
        }
        let home = (10, 24);
        let target = offset_native_by_map(home.0, home.1, 6, 0);
        self.create_city(1, home.0, home.1);
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let paratrooper = self.spawn_unit(1, UnitKind::Paratroopers, home.0, home.1);
        let unit = self.units.last().unwrap().clone();
        self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
        let declined = !self.run_ai_paratrooper(&unit);
        for _ in 0..2 {
            self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
        }
        self.set_random_seed(1);
        let struck = self.run_ai_paratrooper(&unit)
            && self.units.iter().all(|unit| unit.owner != HUMAN)
            && self.units.iter().any(|unit| {
                unit.id == paratrooper && (unit.x, unit.y) == target && unit.moves_left == 0
            });
        (declined, struck)
    }

    pub(crate) fn ai_paradrop_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        let home = (10, 24);
        let front = offset_native_by_map(home.0, home.1, 6, 0);
        let enemy = offset_native_by_map(home.0, home.1, -6, 0);
        let threat = offset_native_by_map(front.0, front.1, 1, 0);
        for (x, y) in [home, front, enemy, threat] {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.known_by[1] = true;
        }
        self.create_city(1, home.0, home.1);
        self.create_city(1, front.0, front.1);
        self.create_city(HUMAN, enemy.0, enemy.1);
        self.spawn_unit(HUMAN, UnitKind::Cannon, threat.0, threat.1);
        let guard = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let para = self.spawn_unit(1, UnitKind::Paratroopers, home.0, home.1);
        self.run_ai_player(1);
        let defended = self
            .units
            .iter()
            .any(|unit| unit.id == para && (unit.x, unit.y) == front)
            && self
                .units
                .iter()
                .any(|unit| unit.id == guard && (unit.x, unit.y) == home)
            && self
                .cities
                .iter()
                .any(|city| city.owner == HUMAN && (city.x, city.y) == enemy);
        let defender = self.spawn_unit(HUMAN, UnitKind::Warrior, enemy.0, enemy.1);
        let raider = self.spawn_unit(1, UnitKind::Paratroopers, home.0, home.1);
        let raider_unit = self.units.last().unwrap().clone();
        let refused = !self.run_ai_paratrooper(&raider_unit);
        self.units.retain(|unit| unit.id != defender);
        let captured = self.run_ai_paratrooper(&raider_unit)
            && self
                .units
                .iter()
                .any(|unit| unit.id == raider && (unit.x, unit.y) == enemy)
            && self
                .cities
                .iter()
                .any(|city| city.owner == 1 && (city.x, city.y) == enemy);
        (defended, refused, captured)
    }

    pub(crate) fn paradrop_playtest_setup(&mut self) -> (u32, (i32, i32), bool) {
        self.units.clear();
        let home = (10, 24);
        let target = offset_native_by_map(home.0, home.1, 6, 0);
        let outside = offset_native_by_map(home.0, home.1, 11, 0);
        for (x, y) in [home, target, outside] {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.known_by[HUMAN] = true;
            tile.hut = false;
        }
        self.create_city(HUMAN, home.0, home.1);
        self.create_city(1, target.0, target.1);
        let kind = Production::Unit(UnitKind::Paratroopers);
        let locked = !self.can_produce(&self.cities[0], kind);
        self.players[HUMAN].known.push(Tech::COMBINED_ARMS);
        let unlocked = self.can_produce(&self.cities[0], kind);
        let actor = self.spawn_unit(HUMAN, UnitKind::Paratroopers, home.0, home.1);
        let out_of_range = !self.apply(Command::Paradrop(actor, outside.0, outside.1));
        (actor, target, locked && unlocked && out_of_range)
    }

    pub(crate) fn ai_fighter_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.hut = false;
        }
        let home = (10, 24);
        let near = offset_native_by_map(home.0, home.1, 3, 0);
        let far = offset_native_by_map(home.0, home.1, 8, 0);
        self.create_city(1, home.0, home.1);
        self.tile_mut(near.0, near.1).known_by[1] = true;
        self.tile_mut(far.0, far.1).known_by[1] = true;
        let fighter = self.spawn_unit(1, UnitKind::Fighter, home.0, home.1);
        let close = self.spawn_unit(HUMAN, UnitKind::Bomber, near.0, near.1);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(1);
        self.run_ai_player(1);
        let intercepted = self.units.iter().all(|unit| unit.id != close);
        self.restore_aircraft();
        let refueled = self.units.iter().any(|unit| {
            unit.id == fighter
                && (unit.x, unit.y) == home
                && unit.fuel == UnitKind::Fighter.fuel_capacity()
        });
        self.update_activities_and_restore();
        if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == fighter) {
            unit.fuel = 1;
        }
        let distant = self.spawn_unit(HUMAN, UnitKind::Bomber, far.0, far.1);
        self.units.last_mut().unwrap().hp = 1;
        self.run_ai_player(1);
        let skipped = self.units.iter().any(|unit| unit.id == distant)
            && self
                .units
                .iter()
                .any(|unit| unit.id == fighter && (unit.x, unit.y) == home);
        (intercepted, skipped, refueled)
    }

    pub(crate) fn ai_carrier_sortie_playtest(&mut self, reposition: bool) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        self.sign_pact(1, HUMAN, Pact::War);
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.base = None;
            tile.hut = false;
            tile.owner = None;
            tile.known_by[1] = true;
        }
        let deck = (10, 24);
        let target = offset_native_by_map(deck.0, deck.1, if reposition { 11 } else { 2 }, 0);
        self.tile_mut(target.0, target.1).terrain = Terrain::Grassland;
        let carrier = self.spawn_unit(1, UnitKind::Carrier, deck.0, deck.1);
        // Cargo load order appoints the Fighter as passenger-in-charge.
        // The reserve Bomber does not receive an independent cargo dispatch.
        let fighter = self.spawn_unit(1, UnitKind::Fighter, deck.0, deck.1);
        assert!(self.board_specific_ship(fighter, carrier));
        let bomber = self.spawn_unit(1, UnitKind::Bomber, deck.0, deck.1);
        assert!(self.board_specific_ship(bomber, carrier));
        let victim = self.spawn_unit(HUMAN, UnitKind::Cannon, target.0, target.1);
        self.units.last_mut().unwrap().hp = if reposition { 2 } else { 1 };
        self.set_random_seed(1);
        self.run_ai_player(1);
        let station = self
            .units
            .iter()
            .find(|unit| unit.id == carrier)
            .map(|unit| (unit.x, unit.y))
            .unwrap();
        let launched =
            self.units
                .iter()
                .any(|unit| unit.id == fighter && unit.moved)
                && self.units.iter().all(|unit| unit.id != victim)
                && self.units.iter().any(|unit| {
                    unit.id == bomber && unit.transport == Some(carrier) && !unit.moved
                });
        self.restore_aircraft();
        self.update_activities_and_restore();
        self.units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap()
            .fuel = 1;
        self.run_ai_player(1);
        self.restore_aircraft();
        let steady = self
            .units
            .iter()
            .any(|unit| unit.id == carrier && (unit.x, unit.y) == station);
        let recovered = self.units.iter().any(|unit| {
            unit.id == fighter
                && unit.transport == Some(carrier)
                && unit.fuel == UnitKind::Fighter.fuel_capacity()
        });
        let source_mission = if reposition {
            // Native ferries do not reposition to extend an aircraft sortie.
            station == deck && self.units.iter().any(|unit| unit.id == victim) && !launched
        } else {
            launched
        };
        (source_mission, steady, recovered)
    }

    pub(crate) fn ai_bomber_playtest(&mut self) -> (bool, bool, bool) {
        // Retain the existing desktop hook name; this now tests the native
        // Fighter sortie. OneAttack Bomber's strict PF cutoff is tested apart.
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        self.sign_pact(1, HUMAN, Pact::War);
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.hut = false;
            tile.owner = None;
            tile.known_by[1] = true;
        }
        let home = (10, 24);
        let target = offset_native_by_map(home.0, home.1, 2, 0);
        self.create_city(1, home.0, home.1);
        let fighter = self.spawn_unit(1, UnitKind::Fighter, home.0, home.1);
        let victim = self.spawn_unit(HUMAN, UnitKind::Cannon, target.0, target.1);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(1);
        self.run_ai_player(1);
        let struck = self.units.iter().all(|unit| unit.id != victim)
            && self
                .units
                .iter()
                .any(|unit| unit.id == fighter && unit.moved);
        self.restore_aircraft();
        self.update_activities_and_restore();
        self.units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap()
            .fuel = 1;
        self.run_ai_player(1);
        self.restore_aircraft();
        let returned = self.units.iter().any(|unit| {
            unit.id == fighter
                && (unit.x, unit.y) == home
                && unit.fuel == UnitKind::Fighter.fuel_capacity()
        });
        self.cities.clear();
        self.update_activities_and_restore();
        let guarded = self.spawn_unit(HUMAN, UnitKind::Cannon, target.0, target.1);
        self.units.last_mut().unwrap().hp = 1;
        self.run_ai_player(1);
        let skipped = self
            .units
            .iter()
            .any(|unit| unit.id == guarded && unit.hp == 1)
            && self
                .units
                .iter()
                .any(|unit| unit.id == fighter && (unit.x, unit.y) == home && !unit.moved);
        (struck, returned, skipped)
    }

    pub(crate) fn ai_counterattack_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        let (x, y) = (10, 24);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(1, x, y);
        let marine = self.spawn_unit(1, UnitKind::Marines, x, y);
        let cannon = self.spawn_unit(HUMAN, UnitKind::Cannon, nx, ny);
        self.units.last_mut().unwrap().hp = 1;
        self.run_ai_player(1);
        let held = self.units.iter().any(|unit| unit.id == cannon)
            && self
                .units
                .iter()
                .any(|unit| unit.id == marine && (unit.x, unit.y) == (x, y));
        self.update_activities_and_restore();
        let guard = self.spawn_unit(1, UnitKind::Warrior, x, y);
        self.units.last_mut().unwrap().moves_left = 0;
        self.set_random_seed(1);
        self.run_ai_player(1);
        let attack = self.units.iter().all(|unit| unit.id != cannon)
            && self
                .units
                .iter()
                .any(|unit| unit.id == guard && (unit.x, unit.y) == (x, y));

        self.units.clear();
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        let enemy = self.spawn_unit(HUMAN, UnitKind::Marines, nx, ny);
        self.run_ai_player(1);
        let refused = self
            .units
            .iter()
            .any(|unit| unit.id == enemy && unit.hp == 20)
            && self
                .units
                .iter()
                .filter(|unit| unit.owner == 1 && (unit.x, unit.y) == (x, y))
                .count()
                == 2;
        (held, attack, refused)
    }

    pub(crate) fn ai_road_reinforcement_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.road = false;
            tile.railroad = false;
            tile.known_by[1] = true;
        }
        let home = (10, 24);
        let point = |step| offset_native_by_map(home.0, home.1, step, 0);
        let front = point(3);
        let enemy = point(4);
        self.create_city(1, home.0, home.1);
        self.create_city(1, front.0, front.1);
        let marine = self.spawn_unit(1, UnitKind::Marines, home.0, home.1);
        let guard = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        self.spawn_unit(1, UnitKind::Warrior, front.0, front.1);
        self.spawn_unit(HUMAN, UnitKind::Cannon, enemy.0, enemy.1);
        let too_far = !self.reinforce_ai_cities(1).contains(&marine);
        for step in 0..=3 {
            let (x, y) = point(step);
            self.tile_mut(x, y).road = true;
        }
        let dispatched = self.reinforce_ai_cities(1).contains(&marine);
        too_far
            && dispatched
            && self
                .units
                .iter()
                .any(|unit| unit.id == marine && (unit.x, unit.y) == front)
            && self
                .units
                .iter()
                .any(|unit| unit.id == guard && (unit.x, unit.y) == home)
    }

    pub(crate) fn ai_reinforcements_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.road = false;
        }
        let home = (10, 24);
        let front = offset_native_by_map(home.0, home.1, 2, 0);
        let enemy = offset_native_by_map(home.0, home.1, 4, 0);
        self.create_city(1, home.0, home.1);
        self.create_city(1, front.0, front.1);
        let guard = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let marine = self.spawn_unit(1, UnitKind::Marines, home.0, home.1);
        self.spawn_unit(1, UnitKind::Warrior, front.0, front.1);
        self.spawn_unit(HUMAN, UnitKind::Cannon, enemy.0, enemy.1);
        let position = |game: &Self, id| {
            game.units
                .iter()
                .find(|unit| unit.id == id)
                .map(|unit| (unit.x, unit.y))
        };
        self.run_ai_player(1);
        let dispatched = position(self, marine).is_some_and(|tile| tile != home && tile != front);
        for _ in 0..2 {
            self.update_activities_and_restore();
            self.run_ai_player(1);
        }
        (
            dispatched,
            position(self, marine) == Some(front),
            position(self, guard) == Some(home),
        )
    }

    pub(crate) fn ai_fortifications_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.base = None;
            tile.hut = false;
        }
        let (x, y) = (10, 24);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let (tx, ty) = offset_native_by_map(x, y, 2, 0);
        self.create_city(1, x, y);
        self.players[1]
            .known
            .extend([Tech::GUNPOWDER, Tech::MASONRY]);
        let city = self.cities[0].id;
        let immediate = self.spawn_unit(HUMAN, UnitKind::Cannon, nx, ny);
        let empty = self.choose_ai_production(1, city) == Production::Unit(UnitKind::Musketeers);
        self.units.retain(|unit| unit.id != immediate);
        let cannon = self.spawn_unit(HUMAN, UnitKind::Cannon, tx, ty);
        let guard = self.spawn_unit(1, UnitKind::Musketeers, x, y);
        self.spawn_unit(1, UnitKind::Musketeers, x, y);
        self.cities[0].production = Production::Building(Building::GRANARY);
        // A two-turn occupier has urgency1 (not the adjacent grave-danger11).
        // Two defenders now exceed urgency, so the native defense advisor
        // considers walls instead of another unit. Routine military/training
        // and domestic competitors are intentionally a separate decision.
        let walls = self.city_defense_production(&self.cities[0])
            == Some(Production::Building(Building::CITY_WALLS));
        let defender = self.units.iter().find(|unit| unit.id == guard).unwrap();
        let land_bonus =
            self.defense_power_with_building(
                defender,
                x,
                y,
                UnitKind::Cannon,
                Some(Building::CITY_WALLS),
            ) > self.defense_power_with_building(defender, x, y, UnitKind::Cannon, None);
        self.units.retain(|unit| unit.id != cannon);
        self.tile_mut(nx, ny).terrain = Terrain::Ocean;
        self.spawn_unit(HUMAN, UnitKind::Battleship, nx, ny);
        let defender = self.units.iter().find(|unit| unit.id == guard).unwrap();
        let ordinary = self.defense_power_with_building(defender, x, y, UnitKind::Battleship, None);
        // Native dai_find_source_building(NULL) ignores class requirements:
        // its first positive provider can still be City Walls for sea danger.
        // Preserve that advisor quirk while checking the real combat effects.
        let naval = self.city_threats(&self.cities[0]).next().is_some()
            && self.city_defense_production(&self.cities[0])
                == Some(Production::Building(Building::CITY_WALLS))
            && self.defense_power_with_building(
                defender,
                x,
                y,
                UnitKind::Battleship,
                Some(Building::CITY_WALLS),
            ) == ordinary
            && self.defense_power_with_building(
                defender,
                x,
                y,
                UnitKind::Battleship,
                Some(Building::COASTAL_DEFENSE),
            ) > ordinary;
        (empty, walls && land_bonus, naval)
    }

    pub(crate) fn ai_city_defense_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        let (x, y) = (10, 24);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(1, x, y);
        self.players[1]
            .known
            .extend([Tech::GUNPOWDER, Tech::AMPHIBIOUS_WARFARE]);
        // The literal source comparison permits a higher-desire later unit
        // over stock+40; the stock limit is not a hard affordability filter.
        let budget = self.preferred_city_defender(&self.cities[0]) == UnitKind::Marines;
        self.cities[0].shields = 20;
        let stocked = self.preferred_city_defender(&self.cities[0]) == UnitKind::Marines;
        self.cities[0].shields = 0;
        // Isolate the defender recruitment choice from the native veteran
        // training-building stage, which can replace a sub-emergency bid.
        self.cities[0].buildings.push(Building::BARRACKS_II);
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        self.cities[0].production = Production::Building(Building::GRANARY);
        self.prepare_ai_city_defenses(1);
        let quiet = self.cities[0].production == Production::Building(Building::GRANARY);
        self.spawn_unit(HUMAN, UnitKind::Cannon, nx, ny);
        self.prepare_ai_city_defenses(1);
        // The complete native upgrade chain makes Musketeers obsolete once
        // Marines are available, even if intermediate Riflemen tech is absent.
        let responded = self.cities[0].production == Production::Unit(UnitKind::Marines);
        (budget && stocked, quiet, responded)
    }

    pub(crate) fn fogged_border_playtest(&mut self) -> (bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by = [false; PLAYER_SLOTS];
        }
        let (x, y) = (10, 24);
        let coast = offset_native_by_map(x, y, 1, 0);
        let sea = offset_native_by_map(x, y, 2, 0);
        let lake = offset_native_by_map(x, y, 0, -2);
        for step in 1..=8 {
            let (sx, sy) = offset_native_by_map(x, y, step, 0);
            self.tile_mut(sx, sy).terrain = Terrain::Ocean;
        }
        // Enlarge the connected sea so it cannot qualify as an enclosed lake.
        for step in 3..=8 {
            for dy in 0..=4 {
                let (sx, sy) = offset_native_by_map(x, y, step, dy);
                self.tile_mut(sx, sy).terrain = Terrain::Ocean;
            }
        }
        self.tile_mut(lake.0, lake.1).terrain = Terrain::Lake;
        self.create_city(1, x, y);
        let water = self.tile(coast.0, coast.1).owner == Some(1)
            && self.tile(sea.0, sea.1).owner.is_none()
            && self.tile(lake.0, lake.1).owner == Some(1);

        let scout = self.spawn_unit(HUMAN, UnitKind::Explorer, x, y);
        self.refresh_visibility();
        let saw_claim = self.displayed_tile_owner(x, y) == Some(1);
        self.units.retain(|unit| unit.id != scout);
        self.refresh_visibility();
        self.cities.clear();
        self.rebuild_city_ownership();
        let remembered =
            self.tile(x, y).owner.is_none() && self.displayed_tile_owner(x, y) == Some(1);
        self.spawn_unit(HUMAN, UnitKind::Explorer, x, y);
        self.refresh_visibility();
        let updated = self.displayed_tile_owner(x, y).is_none();
        (water, saw_claim && remembered && updated)
    }

    pub(crate) fn city_borders_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by = [false; PLAYER_SLOTS];
        }
        let (x, y) = (10, 24);
        let point = |step| offset_native_by_map(x, y, step, 0);
        let (rx, ry) = point(4);
        self.create_city(HUMAN, x, y);
        self.create_city(1, rx, ry);
        let owner = |game: &Self, step| {
            let (x, y) = point(step);
            game.tile(x, y).owner
        };
        let tie = owner(self, 2) == Some(HUMAN);
        self.cities[1].size = 12;
        self.rebuild_city_ownership();
        let fog_restricted = owner(self, 8).is_none();
        for tile in &mut self.tiles {
            tile.known_by[1] = true;
        }
        self.rebuild_city_ownership();
        let strength = owner(self, 2) == Some(1)
            && owner(self, 1) == Some(HUMAN)
            && owner(self, 0) == Some(HUMAN);
        let expanded = owner(self, 8) == Some(1);
        self.cities[1].size = 1;
        self.rebuild_city_ownership();
        (
            tie,
            strength && fog_restricted,
            expanded && owner(self, 8).is_none(),
        )
    }

    pub(crate) fn ferry_research_playtest(&mut self) -> (bool, bool, bool) {
        let (x, y) = (10, 24);
        for (nx, ny) in super::terrain_work::neighbors(x, y).chain([(x, y)]) {
            self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        }
        self.create_city(1, x, y);
        self.players[1].known = vec![Tech::ALPHABET];
        self.players[1].research_goal = Tech::BRONZE_WORKING;
        let inland = self.ferry_tech_want(1, Tech::MAP_MAKING) == 0
            && self.research_target(1) == Tech::BRONZE_WORKING;
        let (sx, sy) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(sx, sy).terrain = Terrain::Ocean;
        let path = self.research_path(1, Tech::MAP_MAKING);
        let coastal = self.ferry_tech_want(1, Tech::MAP_MAKING) == 500
            && path.first().copied() == Some(self.research_target(1));
        for tech in path {
            self.players[1].researching = self.research_target(1);
            if self.players[1].researching != tech {
                return (inland, coastal, false);
            }
            let cost = self.target_cost(1, tech);
            self.resolve_research_turn(1, cost + self.technology_upkeep(1));
        }
        let next = self.players[1].researching;
        let advanced = self.has_tech(1, Tech::MAP_MAKING)
            && self.ferry_tech_want(1, Tech::MAP_MAKING) == 0
            && next != Tech::MAP_MAKING
            && self.tech_available(1, next)
            && self
                .research_path(1, self.players[1].research_goal)
                .contains(&next);
        (inland, coastal, advanced)
    }

    pub(crate) fn settlement_ferry_production_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.resource = None;
            tile.hut = false;
        }
        let home = (10, 24);
        let island = offset_native_by_map(home.0, home.1, 6, 0);
        self.tile_mut(home.0, home.1).terrain = Terrain::Forest;
        self.tile_mut(island.0, island.1).terrain = Terrain::Grassland;
        self.create_city(1, home.0, home.1);
        self.cities[0].size = 3;
        self.cities[0].food = 20;
        let city = self.cities[0].id;
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let before = self.choose_ai_production(1, city);
        self.players[1].known.push(Tech::MAP_MAKING);
        let production = self.choose_ai_production(1, city);
        let boat = Production::Unit(UnitKind::Trireme);
        let queued = before != boat && production == boat;
        if !queued {
            return (false, false, false);
        }
        self.cities[0].production = production;
        self.cities[0].shields = production.cost();
        self.process_cities();
        let settler = Production::Unit(UnitKind::Settlers);
        let sequenced = self.units.iter().any(|unit| unit.kind == UnitKind::Trireme)
            && self.cities[0].production == settler;
        if !sequenced {
            return (true, false, false);
        }
        self.cities[0].shields = settler.cost();
        self.process_cities();
        for _ in 0..8 {
            self.update_activities_and_restore();
            self.run_ai_player(1);
            if self.cities.len() > 1 {
                break;
            }
        }
        let founded = self
            .cities
            .iter()
            .any(|city| city.owner == 1 && (city.x, city.y) == island);
        (queued, sequenced, founded)
    }

    pub(crate) fn offshore_pickup_playtest(&mut self, inland: bool) -> (bool, bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.resource = None;
            tile.hut = false;
        }
        let (x, y) = (10, 24);
        let shore = offset_native_by_map(x, y, 1, 0);
        let island = offset_native_by_map(x, y, 9, 0);
        for (tx, ty) in [(x, y), shore, island] {
            self.tile_mut(tx, ty).terrain = Terrain::Grassland;
        }
        if inland {
            for (tx, ty) in super::terrain_work::neighbors(x, y) {
                self.tile_mut(tx, ty).terrain = Terrain::Grassland;
            }
        }
        self.create_city(1, x, y);
        let start = if inland { (x, y) } else { shore };
        let founder = self.spawn_unit(1, UnitKind::Settlers, start.0, start.1);
        self.units.last_mut().unwrap().home_city = Some(self.cities[0].id);
        let (sx, sy) = offset_native_by_map(x, y, -6, 0);
        let ferry = self.spawn_unit(1, UnitKind::Galleon, sx, sy);
        self.run_ai_player(1);
        let waiting = self.units.iter().any(|unit| {
            unit.id == founder
                && unit.transport.is_none()
                && if inland {
                    (unit.x, unit.y) != start
                } else {
                    (unit.x, unit.y) == start
                }
        }) && self
            .units
            .iter()
            .any(|unit| unit.id == ferry && (unit.x, unit.y) != (sx, sy));
        let mut embarked = false;
        for _ in 0..12 {
            self.update_activities_and_restore();
            self.run_ai_player(1);
            embarked |= self
                .units
                .iter()
                .any(|unit| unit.id == founder && unit.transport == Some(ferry));
            if self.cities.len() > 1 {
                break;
            }
        }
        let founded = self
            .cities
            .iter()
            .any(|city| city.owner == 1 && (city.x, city.y) == island);
        (waiting, embarked, founded)
    }

    pub(crate) fn overseas_settlement_playtest(&mut self, inland: bool) -> (bool, bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.resource = None;
            tile.hut = false;
        }
        let (x, y) = (10, 24);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.create_city(1, x, y);
        let home = self.cities[0].id;
        self.cities[0].size = 3;
        let (ix, iy) = offset_native_by_map(x, y, 8, 0);
        self.tile_mut(ix, iy).terrain = Terrain::Grassland;
        if inland {
            for (bx, by) in super::terrain_work::neighbors(ix, iy) {
                self.tile_mut(bx, by).terrain = Terrain::Glacier;
            }
        }
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        let (sx, sy) = offset_native_by_map(x, y, -1, 0);
        let ferry = self.spawn_unit(1, UnitKind::Galleon, sx, sy);
        let production = self.choose_ai_production(1, home) == Production::Unit(UnitKind::Settlers);
        let founder = self.spawn_unit(1, UnitKind::Settlers, x, y);
        self.units.last_mut().unwrap().home_city = Some(home);
        self.run_ai_player(1);
        let embarked = self.units.iter().any(|unit| {
            unit.id == founder && unit.transport == Some(ferry) && (unit.x, unit.y) != (x, y)
        });
        let mut landed_before_founding = false;
        for _ in 0..8 {
            if self.cities.len() > 1 {
                break;
            }
            self.update_activities_and_restore();
            self.run_ai_player(1);
            landed_before_founding |= self.units.iter().any(|unit| {
                unit.id == founder && unit.transport.is_none() && (unit.x, unit.y) != (x, y)
            });
        }
        let founded = self
            .cities
            .iter()
            .any(|city| city.owner == 1 && (city.x, city.y) == (ix, iy))
            && self.units.iter().all(|unit| unit.id != founder);
        (
            production,
            embarked,
            founded && (!inland || landed_before_founding),
        )
    }

    pub(crate) fn ai_expansion_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.resource = None;
            tile.hut = false;
        }
        let (x, y) = (10, 24);
        for step in 0..=6 {
            let (tx, ty) = offset_native_by_map(x, y, step, 0);
            self.tile_mut(tx, ty).terrain = Terrain::Grassland;
        }
        for (cx, cy) in [(x, y), (20, 38), (24, 48)] {
            self.tile_mut(cx, cy).terrain = Terrain::Grassland;
            self.create_city(1, cx, cy);
        }
        let (ix, iy) = offset_native_by_map(x, y, 0, -7);
        self.tile_mut(ix, iy).terrain = Terrain::Grassland;
        self.tile_mut(ix, iy).river = true;
        self.cities[0].size = 3;
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        let wants_settlers =
            self.choose_ai_production(1, self.cities[0].id) == Production::Unit(UnitKind::Settlers);
        let id = self.spawn_unit(1, UnitKind::Settlers, x, y);
        let founder = self.units.last().unwrap();
        let destination = self.best_city_site(founder).map(|(site, _, _)| site);
        let reachable = destination
            .is_some_and(|site| (3..=6).any(|step| site == offset_native_by_map(x, y, step, 0)));
        self.run_ai_player(1);
        let first_leg = self
            .units
            .iter()
            .any(|unit| unit.id == id && (unit.x, unit.y) != (x, y))
            && self.cities.len() == 3;
        for _ in 0..6 {
            if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) {
                unit.moves_left = MOVE_FRAGMENTS;
            } else {
                break;
            }
            self.run_ai_player(1);
        }
        let fourth_city = first_leg
            && self.cities.len() == 4
            && self
                .cities
                .last()
                .is_some_and(|city| Some((city.x, city.y)) == destination && city.owner == 1);
        (wants_settlers, reachable, fourth_city)
    }

    pub(crate) fn marines_playtest_setup(&mut self) -> (u32, u32, u32, bool) {
        let (x, y) = (12, 24);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let (hx, hy) = offset_native_by_map(x, y, -5, 0);
        self.tile_mut(x, y).terrain = Terrain::Ocean;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.tile_mut(hx, hy).terrain = Terrain::Grassland;
        self.create_city(HUMAN, hx, hy);
        self.create_city(1, nx, ny);
        let target = Production::Unit(UnitKind::Marines);
        let locked = !self.can_produce(&self.cities[0], target);
        self.players[HUMAN].known.push(Tech::AMPHIBIOUS_WARFARE);
        let available = self.can_produce(&self.cities[0], target);
        let boat = self.spawn_unit(HUMAN, UnitKind::Galleon, x, y);
        let infantry = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        self.units.last_mut().unwrap().transport = Some(boat);
        let marines = self.spawn_unit(HUMAN, UnitKind::Marines, x, y);
        self.units.last_mut().unwrap().transport = Some(boat);
        let guard = self.spawn_unit(1, UnitKind::Diplomat, nx, ny);
        self.refresh_visibility();
        (marines, infantry, guard, locked && available)
    }

    pub(crate) fn combat_population_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        // Isolate population and automatic rearrangement from random terrain.
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.oil_well = false;
            tile.pollution = false;
            tile.fallout = false;
        }
        let (x, y) = (12, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let (wx, wy) = offset_native_by_map(x, y, 8, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(1, nx, ny);
        self.create_city(1, wx, wy);
        let target = self.cities.len() - 2;
        let wonder_city = target + 1;
        self.cities[target].size = 5;
        self.cities[target].specialists = [0, 0, 3];
        self.cities[target].worked_tiles = Self::city_radius_at(nx, ny)
            .map(|(_, _, x, y)| (x, y))
            .filter(|&p| p != (nx, ny))
            .take(2)
            .collect();
        self.cities[target].food = Self::growth_box(5);
        self.spawn_unit(1, UnitKind::Warrior, nx, ny);

        let attack = |game: &mut Self, kind| {
            let attacker = game.spawn_unit(HUMAN, kind, x, y);
            // An undefending unit makes the result deterministic. The other
            // defender prevents occupation from causing its own population loss.
            let defender = game.spawn_unit(1, UnitKind::Diplomat, nx, ny);
            game.resolve_combat(attacker, defender, nx, ny);
            game.cities[target].size
        };

        let light_attack = attack(self, UnitKind::Warrior) == 5;
        self.cities[target].buildings.push(Building::CITY_WALLS);
        let walls = attack(self, UnitKind::Cannon) == 5;
        self.cities[target]
            .buildings
            .retain(|&b| b != Building::CITY_WALLS);
        self.cities[wonder_city]
            .buildings
            .push(Building::GREAT_WALL);
        let great_wall = attack(self, UnitKind::Cannon) == 5;
        self.players[1].known.push(Tech::MACHINE_TOOLS);
        let exposed = attack(self, UnitKind::Cannon) == 4
            && self.cities[target].food == Self::growth_box(4)
            // Native CM follows citizen removal. With a full granary and
            // plain Grassland, three workers feed size four and the last
            // citizen becomes a Taxman (the Palace boosts gold) rather than
            // staying an entertainer.
            && self.cities[target].specialists == [0, 1, 0]
            && self.cities[target].worked_tiles.len() == 3;
        self.cities[target].size = 1;
        self.cities[target].specialists = [0; 3];
        light_attack && walls && great_wall && exposed && attack(self, UnitKind::Cannon) == 1
    }

    pub(crate) fn obsolete_wonders_playtest(&mut self) -> (bool, bool, bool) {
        let naval_speed = |game: &Self| game.unit_move_allowance(HUMAN, UnitKind::Destroyer);
        let science = |game: &Self| game.split_city_trade(&game.cities[0], 0, [100, 0, 0])[0];
        let base_speed = naval_speed(self);
        let base_science = science(self);
        let city = &self.cities[0];
        let science_tiles = 1 + city.worked_tiles.len().min(city.tile_workers() as usize) as i32;
        self.record_history(0);
        let base_history = self.cities[0].history;
        let wonders = [
            Building::GREAT_WALL,
            Building::LIGHTHOUSE,
            Building::MAGELLANS_EXPEDITION,
            Building::COPERNICUS_OBSERVATORY,
        ];
        self.cities[0].buildings.extend(wonders);
        self.players[1].known.extend([
            Tech::ENGINEERING,
            Tech::NUCLEAR_POWER,
            Tech::COMPUTERS,
            Tech::MACHINE_TOOLS,
        ]);
        let active = naval_speed(self) == base_speed + 2 * MOVE_FRAGMENTS
            && science(self) == base_science + science_tiles
            && self.has_active_wonder(HUMAN, Building::GREAT_WALL);
        self.record_history(0);
        let active_history = self.cities[0].history
            == 2 * base_history
                + wonders
                    .into_iter()
                    .map(Building::history_per_turn)
                    .sum::<i32>();
        self.players[HUMAN].known.push(Tech::ENGINEERING);
        let lighthouse_ended = naval_speed(self) == base_speed + MOVE_FRAGMENTS;
        self.players[HUMAN].known.extend([
            Tech::NUCLEAR_POWER,
            Tech::COMPUTERS,
            Tech::MACHINE_TOOLS,
        ]);
        // Nuclear Power's own movement bonus replaces Magellan's bonus.
        let naval_ended = lighthouse_ended && naval_speed(self) == base_speed + MOVE_FRAGMENTS;
        let prior_history = self.cities[0].history;
        self.record_history(0);
        let history_retained = self.cities[0].history == prior_history + base_history
            && wonders.iter().all(|b| self.cities[0].buildings.contains(b));
        (
            active && active_history,
            naval_ended
                && science(self) == base_science
                && !self.has_active_wonder(HUMAN, Building::GREAT_WALL),
            history_retained,
        )
    }

    pub(crate) fn theatre_and_crusade_playtest(&mut self) -> (bool, bool, bool) {
        let (x, y) = (self.cities[0].x, self.cities[0].y);
        let home = self.cities[0].id;
        let (tx, ty) = offset_native_by_map(x, y, 8, 0);
        let (rx, ry) = offset_native_by_map(x, y, -8, 0);
        self.create_city(HUMAN, tx, ty);
        self.create_city(1, rx, ry);
        self.players[HUMAN].government = Government::Democracy;
        self.cities[0].size = 4;
        let (ux, uy) = offset_native_by_map(x, y, 4, 0);
        self.tile_mut(ux, uy).owner = None;
        for _ in 0..2 {
            self.spawn_unit(HUMAN, UnitKind::Warrior, ux, uy);
            self.units.last_mut().unwrap().home_city = Some(home);
        }

        let pressure = |game: &Self| game.city_military_unhappiness(&game.cities[0]);
        let upkeep = |game: &Self| {
            game.city_gold_upkeep(&game.cities[0])
                .iter()
                .map(|(_, cost)| cost)
                .sum::<i32>()
        };
        let original_pressure = pressure(self);
        let baseline = original_pressure == 2 && upkeep(self) == 2;
        self.cities[1]
            .buildings
            .push(Building::KING_RICHARDS_CRUSADE);
        let crusade = baseline && pressure(self) == original_pressure - 1 && upkeep(self) == 0;
        self.players[HUMAN].known.push(Tech::COMMUNISM);
        let obsolete = pressure(self) == original_pressure && upkeep(self) == 2;

        self.cities[1]
            .buildings
            .push(Building::SHAKESPEARES_THEATER);
        self.completed_wonders.push(Building::SHAKESPEARES_THEATER);
        self.cities[2].specialists[2] = 1;
        let luxury =
            |game: &Self, index| game.split_city_trade(&game.cities[index], 0, [100, 0, 0])[2];
        let theatre = luxury(self, 0) == 1 && luxury(self, 1) == 6 && luxury(self, 2) == 3;
        self.cities[1]
            .buildings
            .retain(|&b| b != Building::SHAKESPEARES_THEATER);
        (
            theatre && luxury(self, 0) == 0 && luxury(self, 2) == 3,
            crusade,
            obsolete,
        )
    }

    pub(crate) fn wonder_output_playtest(&mut self) -> (bool, bool, bool) {
        let (x, y) = (self.cities[0].x, self.cities[0].y);
        let (wx, wy) = offset_native_by_map(x, y, 1, 0);
        for (tx, ty, river) in [(x, y, true), (wx, wy, false)] {
            let tile = self.tile_mut(tx, ty);
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = river;
            tile.road = false;
            tile.pollution = false;
            tile.fallout = false;
        }
        self.players[HUMAN].government = Government::Anarchy;
        self.cities[0].size = 2;
        self.cities[0].worked_tiles = vec![(wx, wy)];
        self.cities[0].specialists = [0; 3];
        let base_trade = self.city_yields(&self.cities[0]).2;
        self.cities[0].buildings.push(Building::COLOSSUS);
        let zero_tile_unchanged = self.city_yields(&self.cities[0]).2 == base_trade + 1;
        self.tile_mut(wx, wy).river = true;
        let trade_bonus =
            zero_tile_unchanged && self.city_yields(&self.cities[0]).2 == base_trade + 3;
        self.players[HUMAN].known.push(Tech::AUTOMOBILE);
        let colossus_obsolete = self.city_yields(&self.cities[0]).2 == base_trade + 1;

        let (sx, sy) = offset_native_by_map(x, y, 5, 0);
        let (rx, ry) = offset_native_by_map(x, y, -5, 0);
        self.create_city(HUMAN, sx, sy);
        self.create_city(1, rx, ry);
        self.players[HUMAN].government = Government::Monarchy;
        self.cities[0].buildings.extend([
            Building::HANGING_GARDENS,
            Building::JS_BACHS_CATHEDRAL,
            Building::CURE_FOR_CANCER,
            Building::MARKETPLACE,
        ]);
        let luxury =
            |game: &Self, city: usize| game.split_city_trade(&game.cities[city], 0, [100, 0, 0])[2];
        let luxury_scoped = luxury(self, 0) == 12 && luxury(self, 1) == 4 && luxury(self, 2) == 0;
        self.players[HUMAN].known.push(Tech::ELECTRICITY);
        let gardens_obsolete = luxury(self, 0) == 6 && luxury(self, 1) == 4;
        (
            trade_bonus,
            luxury_scoped,
            colossus_obsolete && gardens_obsolete,
        )
    }

    pub(crate) fn barracks_playtest(&mut self) -> (bool, bool, bool) {
        let index = self
            .cities
            .iter()
            .position(|city| city.owner == HUMAN)
            .unwrap();
        let (x, y) = (self.cities[index].x, self.cities[index].y);
        self.cities[index]
            .buildings
            .extend([Building::BARRACKS, Building::SUN_TZUS_WAR_ACADEMY]);
        let stacked = self.trained_rank(&self.cities[index], UnitKind::Warrior) == 2;
        let unit = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        self.units.last_mut().unwrap().hp = 1;
        self.update_activities_and_restore();
        let healed = self
            .units
            .iter()
            .any(|unit_info| unit_info.id == unit && unit_info.hp == 10);
        self.players[HUMAN].known.push(Tech::GUNPOWDER);
        let retained = self.active_barracks(&self.cities[index])
            && self.trained_rank(&self.cities[index], UnitKind::Warrior) == 2;
        (stacked, healed, retained)
    }

    pub(crate) fn ai_inland_ferry_playtest_setup(&mut self) -> (u32, u32, u32, u32) {
        let result @ (_, _, _, destination) = self.ai_merchant_ferry_playtest_setup();
        let city = self
            .cities
            .iter()
            .find(|city| city.id == destination)
            .unwrap();
        let radius: Vec<_> = Self::city_radius(city).map(|(_, _, x, y)| (x, y)).collect();
        for (x, y) in radius {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.road = true;
        }
        result
    }

    pub(crate) fn ai_ferry_dispatch_playtest_setup(&mut self) -> (u32, u32, u32, u32) {
        let result @ (_, ferry, _, _) = self.ai_merchant_ferry_playtest_setup();
        let ship = self.units.iter_mut().find(|unit| unit.id == ferry).unwrap();
        (ship.x, ship.y) = offset_native_by_map(ship.x, ship.y, 7, 0);
        result
    }

    pub(crate) fn ai_merchant_ferry_playtest_setup(&mut self) -> (u32, u32, u32, u32) {
        let (merchant, home, destination) = self.ai_merchant_playtest_setup();
        let home_city = self.cities.iter().find(|city| city.id == home).unwrap();
        let (x, y) = (home_city.x, home_city.y);
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.hut = false;
        }
        for city in &self.cities {
            self.tiles[self.map_wrap.index(city.x, city.y)].terrain = Terrain::Grassland;
        }
        let cargo = self
            .units
            .iter_mut()
            .find(|unit| unit.id == merchant)
            .unwrap();
        (cargo.x, cargo.y) = (x, y);
        let ferry = self.spawn_unit(1, UnitKind::Galleon, x, y);
        (merchant, ferry, home, destination)
    }

    pub(crate) fn ai_trade_production_gate_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        self.create_city(HUMAN, 10, 24);
        let (home, _) = self.ai_merchant_production_playtest_setup();
        self.sign_pact(1, HUMAN, Pact::Alliance);
        for step in [10, 14, 18, 22] {
            let (x, y) = offset_native_by_map(10, 24, step, 0);
            self.create_city(1, x, y);
        }
        let city = self.cities.iter().find(|city| city.id == home).unwrap();
        self.cities.iter().filter(|city| city.owner == 1).count() >= 5
            && self.can_produce(city, Production::Unit(UnitKind::Freight))
            && self.merchant_production_choice(city).is_none()
    }

    pub(crate) fn ai_merchant_production_playtest_setup(&mut self) -> (u32, u32) {
        let (merchant, home, destination) = self.ai_merchant_playtest_setup();
        self.remove_units_with_cargo(vec![merchant]);
        let index = self.cities.iter().position(|city| city.id == home).unwrap();
        let (x, y) = (self.cities[index].x, self.cities[index].y);
        for step in 0..5 {
            let (tx, ty) = offset_native_by_map(x, y, -step, 0);
            let tile = self.tile_mut(tx, ty);
            tile.terrain = Terrain::Grassland;
            tile.road = true;
            tile.hut = false;
        }
        let city = self
            .cities
            .iter_mut()
            .find(|city| city.id == destination)
            .unwrap();
        city.size = 45;
        city.food = 500;
        self.players[1]
            .known
            .extend([Tech::TRADE, Tech::THE_CORPORATION]);
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        let corridor: Vec<_> = (1..5)
            .map(|step| offset_native_by_map(x, y, -step, 0))
            .collect();
        self.units
            .retain(|unit| !corridor.contains(&(unit.x, unit.y)));
        self.cities[index].shields = 50;
        self.cities[index].production = self.choose_ai_production(1, home);
        (home, destination)
    }

    pub(crate) fn ai_merchant_playtest_setup(&mut self) -> (u32, u32, u32) {
        let human = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y, destination) = (human.x, human.y, human.id);
        let (mx, my) = offset_native_by_map(x, y, 1, 0);
        let (nx, ny) = offset_native_by_map(x, y, 5, 0);
        self.tile_mut(mx, my).terrain = Terrain::Grassland;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(1, nx, ny);
        let home = self.cities.last().unwrap().id;
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        let merchant = self.spawn_unit(1, UnitKind::Freight, mx, my);
        self.units.last_mut().unwrap().home_city = Some(home);
        (merchant, home, destination)
    }

    pub(crate) fn hunter_production_playtest_setup(&mut self) -> (u32, u32) {
        let (x, y) = (10, 20);
        let (tx, ty) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(tx, ty).terrain = Terrain::Ocean;
        self.tile_mut(tx, ty).known_by[1] = true;
        self.tile_mut(tx, ty).hut = false;
        self.create_city(1, x, y);
        let city = self.cities.last().unwrap().id;
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        let target = self.spawn_unit(HUMAN, UnitKind::Battleship, tx, ty);
        self.units.last_mut().unwrap().hp = 1;
        self.players[1]
            .known
            .extend([Tech::COMBUSTION, Tech::ROCKETRY]);
        let production = self.choose_ai_production(1, city);
        let port = self.cities.last_mut().unwrap();
        port.production = production;
        port.shields = 120;
        (city, target)
    }

    pub(crate) fn ai_submarine_playtest_setup(&mut self) -> (u32, u32, u32) {
        let (submarine, missile, target, _) = self.submarine_playtest_setup();
        for unit in &mut self.units {
            if unit.id == submarine || unit.id == missile {
                unit.owner = 1;
            }
            if unit.id == target {
                unit.owner = HUMAN;
                unit.kind = UnitKind::Carrier;
            }
            if unit.id == missile {
                unit.transport = Some(submarine);
            }
        }
        let (x, y) = self
            .units
            .iter()
            .find(|unit| unit.id == target)
            .map(|unit| (unit.x, unit.y))
            .unwrap();
        self.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
        self.units.last_mut().unwrap().transport = Some(target);
        (submarine, missile, target)
    }

    pub(crate) fn submarine_playtest_setup(&mut self) -> (u32, u32, u32, bool) {
        let (submarine, missile, target) = self.cruise_missile_playtest_setup();
        let ship = self
            .units
            .iter_mut()
            .find(|unit| unit.id == submarine)
            .unwrap();
        ship.kind = UnitKind::Submarine;
        ship.hp = UnitKind::Submarine.stats().hp;
        let mut destroyer = self
            .units
            .iter()
            .find(|unit| unit.id == target)
            .unwrap()
            .clone();
        destroyer.kind = UnitKind::Destroyer;
        let defense =
            |attacker| self.defense_power_against(&destroyer, destroyer.x, destroyer.y, attacker);
        let doubled = defense(UnitKind::Submarine) == 2 * defense(UnitKind::Cruiser);
        (submarine, missile, target, doubled)
    }

    pub(crate) fn surface_warship_playtest(&mut self) -> (bool, bool, bool) {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = (city.x, city.y);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let (tx, ty) = offset_native_by_map(nx, ny, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Ocean;
        self.tile_mut(tx, ty).terrain = Terrain::Grassland;
        self.tile_mut(tx, ty).base = None;
        self.players[HUMAN].known.extend([
            Tech::NAVIGATION,
            Tech::STEAM_ENGINE,
            Tech::ENGINEERING,
            Tech::STEEL,
            Tech::MASS_PRODUCTION,
        ]);
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let production = self.can_produce(city, Production::Unit(UnitKind::Destroyer))
            && self.can_produce(city, Production::Unit(UnitKind::Cruiser))
            && self.can_produce(city, Production::Unit(UnitKind::Battleship))
            && !self.can_produce(city, Production::Unit(UnitKind::Frigate))
            && !self.can_produce(city, Production::Unit(UnitKind::Ironclad));
        let frigate = self.spawn_unit(HUMAN, UnitKind::Frigate, x, y);
        let upgrade = self
            .units
            .iter()
            .find(|unit| unit.id == frigate)
            .and_then(|unit| self.paid_upgrade(unit))
            .is_some_and(|(kind, _)| kind == UnitKind::Destroyer);
        let battleship = self.spawn_unit(HUMAN, UnitKind::Battleship, nx, ny);
        let defender = self.spawn_unit(1, UnitKind::Diplomat, tx, ty);
        let shore_attack = self.apply(Command::Move(battleship, 1, 0))
            && self.units.iter().all(|unit| unit.id != defender)
            && self
                .units
                .iter()
                .any(|unit| unit.id == battleship && (unit.x, unit.y) == (nx, ny));
        (production, upgrade, shore_attack)
    }

    pub(crate) fn trireme_playtest_setup(&mut self) -> (u32, u32) {
        let (x, y) = (10, 20);
        for step in 0..=2 {
            let (nx, ny) = offset_native_by_map(x, y, step, 0);
            let tile = self.tile_mut(nx, ny);
            tile.terrain = if step == 2 {
                Terrain::DeepOcean
            } else {
                Terrain::Grassland
            };
            tile.river = step != 2;
            tile.hut = false;
            tile.owner = None;
        }
        let ship = self.spawn_unit(HUMAN, UnitKind::Trireme, x, y);
        let cargo = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        (ship, cargo)
    }

    pub(crate) fn naval_bonus_playtest(&mut self) -> (i32, i32, i32, usize) {
        let city = self
            .cities
            .iter_mut()
            .find(|city| city.owner == HUMAN)
            .unwrap();
        city.buildings.extend([
            Building::LIGHTHOUSE,
            Building::MAGELLANS_EXPEDITION,
            Building::PORT_FACILITY,
        ]);
        let (x, y) = (city.x, city.y);
        let rank = self.trained_rank(
            self.cities.iter().find(|city| city.owner == HUMAN).unwrap(),
            UnitKind::AegisCruiser,
        );
        self.players[HUMAN].known.push(Tech::NUCLEAR_POWER);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Ocean;
        let docked = self.spawn_unit(HUMAN, UnitKind::AegisCruiser, x, y);
        self.units.last_mut().unwrap().hp = 15;
        let at_sea = self.spawn_unit(HUMAN, UnitKind::AegisCruiser, nx, ny);
        let ship = self.units.last_mut().unwrap();
        ship.hp = 15;
        ship.moved = true;
        self.update_activities_and_restore();
        let docked = self.units.iter().find(|unit| unit.id == docked).unwrap();
        let at_sea = self.units.iter().find(|unit| unit.id == at_sea).unwrap();
        (docked.hp, docked.moves_left, at_sea.moves_left, rank)
    }

    pub(crate) fn science_wonder_playtest(&mut self) -> (bool, i32, i32) {
        let home = self
            .cities
            .iter()
            .position(|city| city.owner == HUMAN)
            .unwrap();
        let (x, y) = (self.cities[home].x, self.cities[home].y);
        let (nx, ny) = offset_native_by_map(x, y, 5, 0);
        self.create_city(1, nx, ny);
        let rival = self.cities.len() - 1;
        self.cities[home].buildings.push(Building::LIBRARY);
        self.cities[rival].buildings.push(Building::LIBRARY);
        let before = self.players[HUMAN].known.len();
        self.finish_building(self.cities[home].id, HUMAN, Building::GREAT_LIBRARY, 300);
        (
            self.players[HUMAN].known.len() == before + 1,
            self.split_city_trade(&self.cities[home], 10, [100, 0, 0])[0],
            self.split_city_trade(&self.cities[rival], 10, [100, 0, 0])[0],
        )
    }

    pub(crate) fn amphitheater_playtest(&mut self) -> (bool, bool, i32) {
        let index = self
            .cities
            .iter()
            .position(|city| city.owner == HUMAN)
            .unwrap();
        self.players[HUMAN].government = Government::Republic;
        self.cities[index].size = 8;
        self.cities[index].specialists = [0; 3];
        self.cities[index].buildings.push(Building::AMPHITHEATER);
        let before = self.city_mood(&self.cities[index], 0).0;
        self.players[HUMAN].known.push(Tech::ELECTRICITY);
        let after = self.city_mood(&self.cities[index], 0).0;
        let history = Building::AMPHITHEATER.history_per_turn();
        (before, after, history)
    }

    pub(crate) fn partisan_playtest_setup(&mut self) -> (u32, u32) {
        let (unit, city) = self.multicultural_playtest_setup();
        self.players[1].government = Government::Communism;
        self.players[1]
            .known
            .extend([Tech::COMMUNISM, Tech::GUNPOWDER]);
        self.players[HUMAN].known.push(Tech::GUERILLA_WARFARE);
        let city_info = self.cities.iter().find(|entry| entry.id == city).unwrap();
        for (_, _, x, y) in Self::city_radius(city_info) {
            self.tile_mut(x, y).terrain = Terrain::Hills;
        }
        (unit, city)
    }

    pub(crate) fn multicultural_playtest_setup(&mut self) -> (u32, u32) {
        let home = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let approach = offset_native_by_map(home.x, home.y, 4, 0);
        let target = offset_native_by_map(home.x, home.y, 5, 0);
        for (x, y) in [approach, target] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.tile_mut(x, y).hut = false;
        }
        self.create_city(1, target.0, target.1);
        let city = self.cities.last_mut().unwrap();
        city.size = 3;
        let city_id = city.id;
        let unit = self.spawn_unit(HUMAN, UnitKind::Warrior, approach.0, approach.1);
        (unit, city_id)
    }

    pub(crate) fn achievement_playtest_setup(&mut self) -> i32 {
        let city = self
            .cities
            .iter()
            .find(|city| city.owner == HUMAN)
            .unwrap()
            .id;
        self.finish_building(city, HUMAN, Building::APOLLO_PROGRAM, 600);
        let before = self.civilization_score(HUMAN);
        self.check_achievements();
        let scored = self.civilization_score(HUMAN);
        self.check_achievements();
        if self.civilization_score(HUMAN) == scored {
            scored - before
        } else {
            0
        }
    }

    pub(crate) fn turn_limit_playtest_setup(&mut self) {
        self.turn = super::score::TURN_LIMIT;
        self.nation_scores[HUMAN].units_built = 1000;
    }

    pub(crate) fn manual_spaceship_playtest_setup(&mut self) {
        let city = self
            .cities
            .iter()
            .find(|city| city.owner == HUMAN)
            .unwrap()
            .id;
        self.finish_building(city, HUMAN, Building::APOLLO_PROGRAM, 600);
    }

    pub(crate) fn manual_spaceship_playtest_build_structural(&mut self) {
        let city = self
            .cities
            .iter()
            .find(|city| city.owner == HUMAN)
            .unwrap()
            .id;
        self.finish_building(city, HUMAN, Building::SPACE_STRUCTURAL, 150);
    }

    pub(crate) fn spaceship_playtest_setup(&mut self) {
        let city = self
            .cities
            .iter()
            .find(|city| city.owner == HUMAN)
            .unwrap()
            .id;
        self.finish_building(city, HUMAN, Building::APOLLO_PROGRAM, 600);
        for (building, count) in [
            (Building::SPACE_MODULE, 3),
            (Building::SPACE_COMPONENT, 2),
            (Building::SPACE_STRUCTURAL, 8),
        ] {
            for _ in 0..count {
                self.finish_building(city, HUMAN, building, building.data().cost);
            }
        }
        self.year = 1880;
    }

    pub(crate) fn spaceship_playtest_finish_flight(&mut self) {
        for _ in 0..30 {
            if !self.apply(Command::EndTurn) {
                break;
            }
        }
    }

    pub(crate) fn airlift_playtest_setup(&mut self) -> (u32, u32, u32) {
        let source = self
            .cities
            .iter_mut()
            .find(|city| city.owner == HUMAN)
            .unwrap();
        source.buildings.push(Building::AIRPORT);
        let (x, y) = (source.x, source.y);
        let (nx, ny) = offset_native_by_map(x, y, 5, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(HUMAN, nx, ny);
        let destination = self.cities.last().unwrap().id;
        let first = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        let second = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        (first, second, destination)
    }

    pub(crate) fn migrants_playtest_setup(&mut self) -> (u32, u32, u32) {
        let (warrior, freight, city) = self.merchant_capture_playtest_setup();
        let target = self
            .units
            .iter_mut()
            .find(|unit| unit.id == freight)
            .unwrap();
        target.kind = UnitKind::Migrants;
        target.hp = 20;
        (warrior, freight, city)
    }

    pub(crate) fn merchant_capture_playtest_setup(&mut self) -> (u32, u32, u32) {
        let home = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y, home_id) = (home.x, home.y, home.id);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        let warrior = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        self.units.last_mut().unwrap().home_city = Some(home_id);
        let merchant = self.spawn_unit(1, UnitKind::Freight, nx, ny);
        self.units.last_mut().unwrap().hp = 7;
        (warrior, merchant, home_id)
    }

    pub(crate) fn merchant_playtest_setup(&mut self) -> (u32, u32, u32, u32) {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y, home) = (city.x, city.y, city.id);
        let (mx, my) = offset_native_by_map(x, y, 5, 0);
        let (nx, ny) = offset_native_by_map(mx, my, 1, 0);
        self.tile_mut(mx, my).terrain = Terrain::Grassland;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(1, nx, ny);
        let city = self.cities.last_mut().unwrap();
        let destination = city.id;
        city.size = 9;
        self.sign_pact(HUMAN, 1, Pact::Ceasefire);
        self.cities
            .iter_mut()
            .find(|city| city.id == home)
            .unwrap()
            .production = Production::Building(Building::PYRAMIDS);
        let caravan = self.spawn_unit(HUMAN, UnitKind::Caravan, mx, my);
        self.units.last_mut().unwrap().home_city = Some(home);
        let freight = self.spawn_unit(HUMAN, UnitKind::Freight, x, y);
        self.refresh_visibility();
        (caravan, freight, home, destination)
    }

    pub(crate) fn merchant_playtest_route_exists(&self, a: u32, b: u32) -> bool {
        self.trade_routes.contains(&(a, b))
    }

    pub(crate) fn run_plague_playtest(&mut self, protected: bool) -> i32 {
        let index = self
            .cities
            .iter()
            .position(|city| city.owner == HUMAN)
            .unwrap();
        let city = &mut self.cities[index];
        city.size = 9;
        if protected {
            city.buildings.push(Building::AQUEDUCT);
        }
        self.set_random_seed(92); // Native C oracle: plague roll 27/1000.
        self.check_city_plague(index, 0);
        self.cities[index].size
    }

    pub(crate) fn run_disaster_playtest(&mut self, nuclear: bool) -> bool {
        let index = self
            .cities
            .iter()
            .position(|city| city.owner == HUMAN)
            .unwrap();
        self.cities[index].size = 2;
        let plant = if nuclear {
            Building::NUCLEAR_PLANT
        } else {
            Building::MFG_PLANT
        };
        self.cities[index].buildings.push(plant);
        // Native C oracle: the fifth/ fourth frequency draw is 183/738.
        self.set_random_seed(if nuclear { 171 } else { 1110 });
        self.check_disasters();
        let city = &self.cities[index];
        let contamination = if nuclear {
            Contamination::Fallout
        } else {
            Contamination::Pollution
        };
        city.size == 1
            && Self::city_radius(city)
                .any(|(_, _, x, y)| self.tile(x, y).has_contamination(contamination))
    }

    pub(crate) fn climate_playtest_setup(&mut self, warming: bool) {
        for (index, tile) in self.tiles.iter_mut().enumerate() {
            tile.terrain = Terrain::Plains;
            tile.river = false;
            tile.pollution = warming && index < 128;
            tile.fallout = !warming && index < 128;
        }
    }

    pub(crate) fn nuclear_playtest_setup(&mut self, intercepted: bool) -> (u32, u32, u32) {
        let (x, y) = (10, 20);
        let target = offset_native_by_map(x, y, 1, 0);
        let sdi_city = offset_native_by_map(x, y, 3, 0);
        for (tx, ty) in [(x, y), target, sdi_city] {
            self.tile_mut(tx, ty).terrain = Terrain::Grassland;
        }
        self.create_city(1, target.0, target.1);
        let city = self.cities.last_mut().unwrap();
        city.size = 4;
        let city_id = city.id;
        if intercepted {
            self.create_city(1, sdi_city.0, sdi_city.1);
            self.cities
                .last_mut()
                .unwrap()
                .buildings
                .push(Building::SDI_DEFENSE);
        }
        self.players[HUMAN]
            .known
            .extend([Tech::NUCLEAR_FISSION, Tech::ADVANCED_FLIGHT]);
        self.completed_wonders.push(Building::MANHATTAN_PROJECT);
        let missile = self.spawn_unit(HUMAN, UnitKind::Nuclear, x, y);
        let defender = self.spawn_unit(1, UnitKind::Musketeers, target.0, target.1);
        self.set_random_seed(1);
        (missile, defender, city_id)
    }

    pub(crate) fn cruise_missile_playtest_setup(&mut self) -> (u32, u32, u32) {
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        for (tx, ty) in [(x, y), (nx, ny)] {
            let tile = self.tile_mut(tx, ty);
            tile.terrain = Terrain::Ocean;
            tile.hut = false;
            tile.base = None;
        }
        let carrier = self.spawn_unit(HUMAN, UnitKind::Carrier, x, y);
        let missile = self.spawn_unit(HUMAN, UnitKind::CruiseMissile, x, y);
        let target = self.spawn_unit(1, UnitKind::Transport, nx, ny);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(1);
        (carrier, missile, target)
    }

    pub(crate) fn aegis_playtest_setup(&mut self) -> (u32, u32) {
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        for (tx, ty) in [(x, y), (nx, ny)] {
            let tile = self.tile_mut(tx, ty);
            tile.terrain = Terrain::Ocean;
            tile.hut = false;
            tile.base = None;
        }
        let attacker = self.spawn_unit(HUMAN, UnitKind::Helicopter, x, y);
        let cruiser = self.spawn_unit(1, UnitKind::AegisCruiser, nx, ny);
        self.units
            .iter_mut()
            .filter(|unit| [attacker, cruiser].contains(&unit.id))
            .for_each(|unit| unit.hp = 1);
        // This seed makes the first round hinge on the anti-air bonus.
        self.set_random_seed(19);
        (attacker, cruiser)
    }

    pub(crate) fn sam_battery_playtest_setup(&mut self, protected: bool) -> (u32, u32) {
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let tile = self.tile_mut(nx, ny);
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        self.create_city(1, nx, ny);
        if protected {
            self.cities
                .last_mut()
                .unwrap()
                .buildings
                .push(Building::SAM_BATTERY);
        }
        let bomber = self.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
        let defender = self.spawn_unit(1, UnitKind::Musketeers, nx, ny);
        self.set_random_seed(91); // All three native rolls distinguish SAM defense.
        (bomber, defender)
    }

    pub(crate) fn helicopter_playtest_setup(&mut self) -> (u32, u32) {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = (city.x, city.y);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.tile_mut(nx, ny).base = None;
        self.tile_mut(nx, ny).hut = false;
        self.players[HUMAN].known.push(Tech::COMBINED_ARMS);
        let helicopter = self.spawn_unit(HUMAN, UnitKind::Helicopter, x, y);
        let passenger = self.spawn_unit(HUMAN, UnitKind::Musketeers, x, y);
        (helicopter, passenger)
    }

    pub(crate) fn stealth_aircraft_playtest_setup(&mut self) -> (u32, u32) {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = (city.x, city.y);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.players[HUMAN]
            .known
            .extend([Tech::ADVANCED_FLIGHT, Tech::STEALTH]);
        self.players[HUMAN].gold = 1000;
        let bomber = self.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
        let target = self.spawn_unit(1, UnitKind::Diplomat, nx, ny);
        (bomber, target)
    }

    pub(crate) fn carrier_playtest_setup(&mut self) -> (u32, u32) {
        let (x, y) = (10, 20);
        let deck = offset_native_by_map(x, y, 1, 0);
        let sailing = offset_native_by_map(x, y, 2, 0);
        let launch = offset_native_by_map(x, y, 3, 0);
        for (tx, ty) in [(x, y), deck, sailing, launch] {
            self.tile_mut(tx, ty).terrain = Terrain::Ocean;
            self.tile_mut(tx, ty).hut = false;
        }
        let carrier = self.spawn_unit(HUMAN, UnitKind::Carrier, deck.0, deck.1);
        let fighter = self.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
        self.units.last_mut().unwrap().fuel = 1;
        (carrier, fighter)
    }

    pub(crate) fn fighter_playtest_setup(&mut self) -> (u32, u32) {
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        let fighter = self.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
        let bomber = self.spawn_unit(1, UnitKind::Bomber, nx, ny);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(4);
        (fighter, bomber)
    }

    pub(crate) fn bombard_playtest_setup(&mut self) -> (u32, [u32; 2]) {
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Ocean;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        let bomber = self.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
        let targets = [
            self.spawn_unit(1, UnitKind::Diplomat, nx, ny),
            self.spawn_unit(1, UnitKind::Spy, nx, ny),
        ];
        (bomber, targets)
    }

    pub(crate) fn cooperative_work_playtest_setup(&mut self) -> (u32, u32) {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = (city.x, city.y);
        let tile = self.tile_mut(x, y);
        tile.terrain = Terrain::Plains;
        tile.river = true;
        tile.irrigation = false;
        let first = self.spawn_unit(HUMAN, UnitKind::Worker, x, y);
        let second = self.spawn_unit(HUMAN, UnitKind::Worker, x, y);
        (first, second)
    }

    pub(crate) fn work_promotion_playtest_tick(&mut self) {
        self.set_random_seed(20);
        self.update_activities_and_restore();
    }

    pub(crate) fn upgrade_playtest_setup(&mut self) -> u32 {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = (city.x, city.y);
        self.players[HUMAN]
            .known
            .extend([Tech::GUNPOWDER, Tech::INVENTION]);
        self.players[HUMAN].gold = 200;
        let id = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        let unit = self.units.last_mut().unwrap();
        unit.veteran = 2;
        unit.hp = 5;
        id
    }

    pub(crate) fn veteran_playtest_setup(&mut self) -> u32 {
        let city = self
            .cities
            .iter_mut()
            .find(|city| city.owner == HUMAN)
            .unwrap();
        city.buildings.push(Building::BARRACKS);
        city.shields = UnitKind::Warrior.stats().cost;
        city.id
    }

    pub(crate) fn stealth_playtest_setup(&mut self) -> (u32, u32) {
        self.units.clear();
        let (x, y) = (10, 20);
        let adjacent = offset_native_by_map(x, y, 1, 0);
        let target = offset_native_by_map(x, y, 2, 0);
        for (tx, ty) in [(x, y), adjacent, target] {
            self.tile_mut(tx, ty).terrain = Terrain::Grassland;
            self.tile_mut(tx, ty).hut = false;
        }
        let explorer = self.spawn_unit(HUMAN, UnitKind::Explorer, x, y);
        let spy = self.spawn_unit(1, UnitKind::Spy, target.0, target.1);
        self.refresh_visibility();
        (explorer, spy)
    }

    pub(crate) fn incite_playtest_setup(&mut self) -> (u32, u32, u32) {
        let actor = self.technology_theft_playtest_setup();
        self.players[HUMAN].gold = 5000;
        let (x, y) = (self.cities[0].x, self.cities[0].y);
        self.cities[0].size = 2;
        self.cities[0].shields = 7;
        self.cities[0].food = 5;
        self.cities[0]
            .buildings
            .retain(|&building| building != Building::PALACE);
        self.cities[0].buildings.push(Building::GRANARY);
        let city = self.cities[0].id;
        let guard = self.spawn_unit(1, UnitKind::Warrior, x, y);
        self.units.last_mut().unwrap().home_city = Some(city);
        self.refresh_visibility();
        (actor, city, guard)
    }

    pub(crate) fn spy_revolt_playtest_setup(&mut self, fail: bool) -> u32 {
        let (actor, _, _) = self.incite_playtest_setup();
        let spy = self.units.iter_mut().find(|unit| unit.id == actor).unwrap();
        spy.kind = UnitKind::Spy;
        self.set_random_seed(if fail { 3 } else { 2 });
        actor
    }

    pub(crate) fn sabotage_unit_playtest_setup(&mut self) -> (u32, u32) {
        let (actor, target) = self.bribe_playtest_setup();
        self.units
            .iter_mut()
            .find(|unit| unit.id == actor)
            .unwrap()
            .kind = UnitKind::Spy;
        self.units
            .iter_mut()
            .find(|unit| unit.id == target)
            .unwrap()
            .hp = 3;
        self.set_random_seed(1);
        (actor, target)
    }

    pub(crate) fn poison_playtest_setup(&mut self) -> (u32, u32) {
        let spy = self.spy_playtest_setup();
        let city = &mut self.cities[0];
        city.size = 2;
        city.food = 5;
        (spy, city.id)
    }

    pub(crate) fn spy_playtest_setup(&mut self) -> u32 {
        let diplomat = self.technology_theft_playtest_setup();
        self.units
            .iter_mut()
            .find(|unit| unit.id == diplomat)
            .unwrap()
            .kind = UnitKind::Spy;
        let (x, y) = offset_native_by_map(10, 20, 3, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.create_city(HUMAN, x, y);
        self.players[HUMAN].known.push(Tech::ESPIONAGE);
        self.cities[0].technology_thefts = 1;
        self.set_random_seed(2);
        diplomat
    }

    pub(crate) fn sabotage_playtest_setup(&mut self) -> u32 {
        let diplomat = self.embassy_playtest_setup();
        let city = self.cities.last_mut().unwrap();
        city.shields = 9;
        city.protected_shields = 2;
        city.production_before_switch = Some((city.production, 12));
        self.set_random_seed(1);
        diplomat
    }

    pub(crate) fn technology_theft_playtest_setup(&mut self) -> u32 {
        let diplomat = self.embassy_playtest_setup();
        self.players[HUMAN].known = vec![Tech::ALPHABET];
        self.players[1].known = vec![Tech::POTTERY];
        self.set_random_seed(1);
        diplomat
    }

    pub(crate) fn bribe_playtest_setup(&mut self) -> (u32, u32) {
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(HUMAN, x, y);
        let home = self.cities.last().unwrap().id;
        let diplomat = self.spawn_unit(HUMAN, UnitKind::Diplomat, x, y);
        self.units.last_mut().unwrap().home_city = Some(home);
        let target = self.spawn_unit(1, UnitKind::Warrior, nx, ny);
        self.players[HUMAN].gold = 200;
        self.refresh_visibility();
        (diplomat, target)
    }

    pub(crate) fn embassy_playtest_setup(&mut self) -> u32 {
        let (x, y) = (10, 20);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.create_city(1, x, y);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.spawn_unit(HUMAN, UnitKind::Diplomat, nx, ny)
    }

    pub(crate) fn zone_control_playtest_setup(&mut self) -> (u32, u32, (i32, i32)) {
        self.units.clear();
        let (x, y) = (10, 20);
        let enemy = offset_native_by_map(x, y, 1, 1);
        let destination = offset_native_by_map(x, y, 1, 0);
        for (tx, ty) in [(x, y), enemy, destination] {
            self.tile_mut(tx, ty).terrain = Terrain::Grassland;
        }
        self.spawn_unit(1, UnitKind::Warrior, enemy.0, enemy.1);
        let warrior = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        let diplomat = self.spawn_unit(HUMAN, UnitKind::Diplomat, x, y);
        (warrior, diplomat, destination)
    }

    pub(crate) fn oil_platform_playtest_setup(&mut self) -> u32 {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = offset_native_by_map(city.x, city.y, 1, 0);
        self.players[HUMAN].known.push(Tech::MINIATURIZATION);
        self.tile_mut(x, y).terrain = Terrain::DeepOcean;
        self.spawn_unit(HUMAN, UnitKind::Transport, x, y)
    }

    pub(crate) fn naval_cleanup_playtest_setup(&mut self) -> u32 {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = offset_native_by_map(city.x, city.y, 1, 0);
        let tile = self.tile_mut(x, y);
        tile.terrain = Terrain::Ocean;
        tile.pollution = true;
        self.spawn_unit(HUMAN, UnitKind::Transport, x, y)
    }

    pub(crate) fn cleanup_playtest_setup(&mut self) -> u32 {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = (city.x, city.y);
        let tile = self.tile_mut(x, y);
        tile.pollution = true;
        tile.fallout = true;
        self.spawn_unit(HUMAN, UnitKind::Engineer, x, y)
    }

    pub(crate) fn base_playtest_setup(&mut self) -> u32 {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = offset_native_by_map(city.x, city.y, 1, 0);
        self.players[HUMAN]
            .known
            .extend([Tech::CONSTRUCTION, Tech::RADIO]);
        let tile = self.tile_mut(x, y);
        tile.terrain = Terrain::Plains;
        tile.river = false;
        self.spawn_unit(HUMAN, UnitKind::Engineer, x, y)
    }

    pub(crate) fn pillage_playtest_setup(&mut self) -> (u32, (i32, i32)) {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = offset_native_by_map(city.x, city.y, 1, 0);
        let tile = self.tile_mut(x, y);
        tile.terrain = Terrain::Plains;
        tile.road = true;
        tile.railroad = true;
        (self.spawn_unit(HUMAN, UnitKind::Warrior, x, y), (x, y))
    }

    pub(crate) fn terrain_work_playtest_setup(&mut self) -> u32 {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = (city.x, city.y);
        self.players[HUMAN]
            .known
            .extend([Tech::EXPLOSIVES, Tech::FUSION_POWER]);
        self.tile_mut(x, y).terrain = Terrain::Tundra;
        self.spawn_unit(HUMAN, UnitKind::Engineer, x, y)
    }

    pub(crate) fn farmland_playtest_setup(&mut self) -> u32 {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = (city.x, city.y);
        self.players[HUMAN]
            .known
            .extend([Tech::REFRIGERATION, Tech::ELECTRICITY]);
        let tile = self.tile_mut(x, y);
        tile.terrain = Terrain::Plains;
        tile.irrigation = true;
        tile.mine = false;
        self.spawn_unit(HUMAN, UnitKind::Worker, x, y)
    }

    pub(crate) fn production_playtest_setup(&mut self) -> (i32, i32) {
        let city = self
            .cities
            .iter_mut()
            .find(|city| city.owner == HUMAN)
            .unwrap();
        city.buildings.push(Building::GRANARY);
        city.production = Production::Unit(UnitKind::Warrior);
        city.shields = 0;
        city.founded_turn = self.turn - 1;
        self.players[HUMAN].gold = 100;
        (city.x, city.y)
    }

    pub(crate) fn transport_playtest_setup(&mut self) -> (u32, u32, (i32, i32)) {
        let city = self.cities.iter().find(|city| city.owner == HUMAN).unwrap();
        let (x, y) = (city.x, city.y);
        let sea = offset_native_by_map(x, y, 1, 0);
        let shore = offset_native_by_map(x, y, 2, 0);
        self.tile_mut(sea.0, sea.1).terrain = Terrain::Ocean;
        self.tile_mut(shore.0, shore.1).terrain = Terrain::Grassland;
        let ship = self.spawn_unit(HUMAN, UnitKind::Caravel, x, y);
        let passenger = self.spawn_unit(HUMAN, UnitKind::Settlers, x, y);
        (ship, passenger, shore)
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
        // Isolate the quote consumer with an explicitly controlled prior-
        // phase adv_want snapshot. This is not a claim that a locked
        // Marketplace produces this want in the native building advisor.
        // Choose a minimal premium that survives both native integer halves
        // (gold equivalent, then alliance discount) throughout these ticks.
        let until = self.turn + 13;
        let base_bulbs = 3 * self.target_cost(1, currency);
        let premium_want = 4 - base_bulbs.rem_euclid(4);
        let mut retained = vec![0.0; Tech::all().count()];
        retained[currency.0] = (until.max(1) * premium_want) as f32 + 0.25;
        self.ai_tech_wants.insert(1, retained);
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

    /// Set up the late-game conditions used by the desktop self-test.
    pub(crate) fn civil_war_playtest(&mut self) -> Option<u32> {
        let city = self.cities.iter().find(|city| city.owner == HUMAN)?;
        let (x, y) = (city.x, city.y);
        let (capital_x, capital_y) = offset_native_by_map(x, y, 1, 0);
        if self
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (capital_x, capital_y))
        {
            return None;
        }
        self.units
            .retain(|unit| (unit.x, unit.y) != (capital_x, capital_y));
        self.tile_mut(capital_x, capital_y).terrain = Terrain::Grassland;
        // Use a fresh rival so the target is its primary capital.
        let rival = 1;
        for city in self.cities.iter_mut().filter(|city| city.owner == rival) {
            city.buildings
                .retain(|&building| building != Building::PALACE);
        }
        self.create_city(rival, capital_x, capital_y);
        let capital = self.cities.last_mut()?;
        if !capital.buildings.contains(&Building::PALACE) {
            capital.buildings.push(Building::PALACE);
        }
        capital.size = 8;
        for index in 0..self.tiles.len() {
            if self
                .cities
                .iter()
                .filter(|city| city.owner == rival)
                .count()
                >= 10
            {
                break;
            }
            let (nx, ny) = self.map_wrap.position(index);
            if self.map_distance(x, y, nx, ny) > 5 && self.can_found_at(nx, ny) {
                self.create_city(rival, nx, ny);
                self.cities.last_mut()?.size = 8;
            }
        }
        self.players[rival].government = Government::Tribal;
        self.players[rival].rates = [60, 40, 0];
        self.players[HUMAN].government = Government::Democracy;
        self.refresh_visibility();
        Some(self.spawn_unit(HUMAN, UnitKind::Warrior, x, y))
    }
}

#[cfg(test)]
mod ai_integration_tests {
    use super::*;
    #[test]
    fn actual_end_turn_heals_before_new_city_improvements() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.native_turn_orchestration_playtest());
    }
    #[test]
    fn native_seeds_preserve_sam_plague_and_disaster_scenario_preconditions() {
        let fresh = || Simulation::new(crate::mapgen::generate(42));
        let mut hp = Vec::new();
        let mut sizes = Vec::new();
        for protected in [false, true] {
            let mut game = fresh();
            let (bomber, defender) = game.sam_battery_playtest_setup(protected);
            assert!(game.apply(Command::Move(bomber, 1, 0)));
            hp.push(
                game.units
                    .iter()
                    .find(|unit| unit.id == defender)
                    .unwrap()
                    .hp,
            );
            let mut game = fresh();
            let settler = game
                .units
                .iter()
                .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settlers)
                .unwrap()
                .clone();
            game.create_city(HUMAN, settler.x, settler.y);
            sizes.push(game.run_plague_playtest(protected));
        }
        assert_eq!(hp, [17, 20]);
        assert_eq!(sizes, [8, 9]);
        for nuclear in [false, true] {
            let mut game = fresh();
            let settler = game
                .units
                .iter()
                .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settlers)
                .unwrap()
                .clone();
            game.create_city(HUMAN, settler.x, settler.y);
            assert!(game.run_disaster_playtest(nuclear));
        }
    }
    #[test]
    fn combat_population_protection_and_native_worker_rearrangement() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.combat_population_playtest());
    }
    #[test]
    fn fortifications_follow_native_urgency_and_land_sea_effect_classes() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_fortifications_playtest(), (true, true, true));
    }
    #[test]
    fn counterintelligence_fixture_uses_prior_phase_research_wants() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_counterintelligence_playtest());
    }

    #[test]
    fn crusade_relieves_one_raw_military_pressure_and_gold_upkeep() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let id = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settlers)
            .unwrap()
            .id;
        assert!(game.apply(Command::FoundCity(id)));
        assert_eq!(game.theatre_and_crusade_playtest(), (true, true, true));
    }

    #[test]
    fn integrated_advisor_scenarios() {
        let fresh = || Simulation::new(crate::mapgen::generate(42));
        let bribery = fresh().ai_bribery_playtest();
        let government = fresh().ai_government_playtest();
        let ferry = fresh().ferry_research_playtest();
        assert!(bribery);
        assert_eq!(government, (true, true, true, true));
        assert_eq!(ferry, (true, true, true));
        assert!(fresh().ai_civilian_upgrade_playtest());
        assert!(fresh().ai_production_priorities_playtest());
        assert!(fresh().ai_trade_production_gate_playtest());
        assert_eq!(fresh().ai_city_defense_playtest(), (true, true, true));
        assert!(fresh().ai_threat_route_cost_playtest());
        assert!(fresh().ai_ground_invasion_playtest());
        assert!(fresh().ai_aircraft_advisor_playtest());
        assert!(fresh().ai_nuclear_advisor_playtest());
        assert_eq!(fresh().ai_fighter_playtest(), (true, true, true));
    }
}
