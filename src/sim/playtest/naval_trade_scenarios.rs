//! Automated gameplay scenarios covering naval trade behavior.

use super::*;

impl Simulation {
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
        let home_city = self.city_by_id(home).unwrap();
        let (x, y) = (home_city.x, home_city.y);
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Sea;
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
        let city = self.city_by_id(home).unwrap();
        self.cities_owned_by(1).count() >= 5
            && self.can_produce(city, Production::Unit(UnitKind::Freight))
            && self.merchant_production_choice(city).is_none()
    }

    pub(crate) fn ai_merchant_production_playtest_setup(&mut self) -> (u32, u32) {
        let (merchant, home, destination) = self.ai_merchant_playtest_setup();
        self.remove_units_with_cargo(vec![merchant]);
        let index = self.city_index_by_id(home).unwrap();
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
        let human = self.cities_owned_by(HUMAN).next().unwrap();
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
        self.tile_mut(tx, ty).terrain = Terrain::Sea;
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
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = (city.x, city.y);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let (tx, ty) = offset_native_by_map(nx, ny, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Sea;
        self.tile_mut(tx, ty).terrain = Terrain::Grassland;
        self.tile_mut(tx, ty).base = None;
        self.players[HUMAN].known.extend([
            Tech::NAVIGATION,
            Tech::STEAM_ENGINE,
            Tech::ENGINEERING,
            Tech::STEEL,
            Tech::MASS_PRODUCTION,
        ]);
        let city = self.cities_owned_by(HUMAN).next().unwrap();
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
                Terrain::Ocean
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
            self.cities_owned_by(HUMAN).next().unwrap(),
            UnitKind::AegisCruiser,
        );
        self.players[HUMAN].known.push(Tech::NUCLEAR_POWER);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Sea;
        let docked = self.spawn_unit(HUMAN, UnitKind::AegisCruiser, x, y);
        self.units.last_mut().unwrap().hp = 15;
        let at_sea = self.spawn_unit(HUMAN, UnitKind::AegisCruiser, nx, ny);
        let ship = self.units.last_mut().unwrap();
        ship.hp = 15;
        ship.moved = true;
        self.update_activities_and_restore();
        let docked = self.unit_by_id(docked).unwrap();
        let at_sea = self.unit_by_id(at_sea).unwrap();
        (docked.hp, docked.moves_left, at_sea.moves_left, rank)
    }

    pub(crate) fn merchant_capture_playtest_setup(&mut self) -> (u32, u32, u32) {
        let home = self.cities_owned_by(HUMAN).next().unwrap();
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
        let city = self.cities_owned_by(HUMAN).next().unwrap();
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

    pub(crate) fn ai_ferry_registry_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_ferries = Default::default();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Sea;
            tile.hut = false;
        }
        let ship = self.spawn_unit(1, UnitKind::Transport, 10, 24);
        let first = self.spawn_unit(1, UnitKind::Settlers, 10, 24);
        let second = self.spawn_unit(1, UnitKind::Freight, 10, 24);
        self.request_ai_ferry(first);
        let available = self.ferry_available_count(1) == 1;
        let assigned = self.assign_ai_ferry(ship, first)
            && self.ferry_available_count(1) == 0
            && !self.ferry_can_take(ship, second)
            && self.cargo_count(ship) == 0;
        self.board_specific_ship(first, ship);
        self.board_specific_ship(second, ship);
        let kept_boss = self.ferry_boss(ship) == Some(first);
        self.release_ai_ferry(first);
        let released = self.ferry_available_count(1) == 1 && self.cargo_count(ship) == 2;
        // A released boat can appoint carried cargo rather than remain idle.
        self.refresh_ferry_boss(ship);
        let appointed = self.ferry_boss(ship).is_some() && self.ferry_available_count(1) == 0;
        let boss = self.ferry_boss(ship).unwrap();
        self.remove_units_with_cargo(vec![boss]);
        let death_released = self.ferry_available_count(1) == 1;
        available
            && assigned
            && kept_boss
            && released
            && appointed
            && death_released
            && self.ai_ferries.valid(&self.units)
    }

    pub(crate) fn ai_merchant_valuation_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        let origin = (10, 24);
        for (step, owner, size) in [
            (0, 1, 30),
            (10, HUMAN, 30),
            (1, HUMAN, 1),
            (2, HUMAN, 20),
            (11, 1, 1),
            (12, 2, 10),
        ] {
            let (x, y) = offset_native_by_map(origin.0, origin.1, step, 0);
            self.create_city(owner, x, y);
            self.cities.last_mut().unwrap().size = size;
        }
        self.sign_pact(1, HUMAN, Pact::Alliance);
        self.sign_pact(2, HUMAN, Pact::Alliance);
        self.trade_routes = vec![
            (self.cities[0].id, self.cities[2].id),
            (self.cities[0].id, self.cities[3].id),
            (self.cities[1].id, self.cities[4].id),
            (self.cities[1].id, self.cities[5].id),
        ];
        let home = self.cities[0].clone();
        let destination = self.cities[1].clone();
        let mut merchant = self.make_unit(0, 1, UnitKind::Freight, home.x, home.y);
        merchant.home_city = Some(home.id);
        let charged = self.merchant_trade_value(&merchant, &home, &destination, 0);
        let lost = self.route_value(&destination, &self.cities[4]);
        self.cities[4].owner = 2;
        let uncharged = self.merchant_trade_value(&merchant, &home, &destination, 0);
        let accounts_for_partner =
            lost > 0 && ((uncharged - charged) - f64::from(lost) / 0.05).abs() < 0.0001;
        let mut wonder = home;
        wonder.production = Production::Building(Building::PYRAMIDS);
        wonder.shields = 0;
        let starting = self.merchant_wonder_value(&merchant, &wonder, 0);
        wonder.shields = 1;
        let started = self.merchant_wonder_value(&merchant, &wonder, 0);
        accounts_for_partner
            && starting > started
            && (started - f64::from(8 * merchant.kind.stats().cost)).abs() < 0.0001
    }

    pub(crate) fn ai_merchant_coastal_pickup_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_merchant_missions.clear();
        self.merchant_mission_attempts.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Sea;
            tile.resource = None;
            tile.hut = false;
            tile.river = false;
            tile.known_by[1] = true;
        }
        let home = (10, 24);
        let coast = offset_native_by_map(home.0, home.1, 1, 0);
        let sea = offset_native_by_map(home.0, home.1, 2, 0);
        let destination = offset_native_by_map(home.0, home.1, 8, 0);
        for (x, y) in [home, coast, destination] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        self.create_city(1, home.0, home.1);
        let origin = self.cities.last().unwrap().id;
        self.create_city(HUMAN, destination.0, destination.1);
        let target = self.cities.last().unwrap().id;
        self.sign_pact(1, HUMAN, Pact::Alliance);
        let passenger = self.spawn_unit(1, UnitKind::Freight, coast.0, coast.1);
        self.units.last_mut().unwrap().home_city = Some(origin);
        let ship = self.spawn_unit(1, UnitKind::Galleon, sea.0, sea.1);
        let ordinary_coast = !self.cities.iter().any(|city| (city.x, city.y) == coast);
        self.run_ai_player(1);
        // Galleon is a native ferry (Transport is an adv.worker). Its lower
        // move rate needs a second phase to complete this sea crossing.
        if self.units.iter().any(|unit| unit.id == passenger) {
            self.update_activities_and_restore();
            self.run_ai_player(1);
        }
        ordinary_coast
            && self.trade_routes.contains(&(origin, target))
            && self.units.iter().all(|unit| unit.id != passenger)
            && self
                .units
                .iter()
                .any(|unit| unit.id == ship && (unit.x, unit.y) != sea)
    }

    pub(crate) fn ai_merchant_missions_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_merchant_missions.clear();
        self.merchant_mission_attempts.clear();
        self.turn = 0;
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.road = true;
            tile.railroad = false;
            tile.river = false;
            tile.hut = false;
        }
        let home = (10, 24);
        for (step, owner, size) in [(0, 1, 20), (5, HUMAN, 20), (6, 2, 60)] {
            let (x, y) = offset_native_by_map(home.0, home.1, step, 0);
            self.create_city(owner, x, y);
            self.cities.last_mut().unwrap().size = size;
        }
        self.sign_pact(1, HUMAN, Pact::Alliance);
        self.sign_pact(1, 2, Pact::Peace);
        let actor = self.spawn_unit(1, UnitKind::Freight, home.0, home.1);
        self.units.last_mut().unwrap().home_city = Some(self.cities[0].id);
        let merchant = self.units.last().unwrap().clone();
        self.set_random_seed(1); // Native fc_rand(2)'s first draw is0: allied-only.
        self.prepare_merchant_mission(&merchant);
        let allied = self
            .ai_merchant_missions
            .get(&actor)
            .is_some_and(|m| m.city == self.cities[1].id);
        let rng = self.random_snapshot();
        self.cities[2].size = 100;
        self.load_ai_merchant_ferries(1);
        let retained = self.random_snapshot() == rng
            && self
                .ai_merchant_missions
                .get(&actor)
                .is_some_and(|m| m.city == self.cities[1].id);
        self.ai_merchant_missions.remove(&actor);
        self.merchant_mission_attempts.clear();
        self.set_random_seed(2); // Native first draw is1: profitable nonwar destination.
        self.prepare_merchant_mission(&merchant);
        let neutral = self
            .ai_merchant_missions
            .get(&actor)
            .is_some_and(|m| m.city == self.cities[2].id);
        allied && retained && neutral && self.ai_merchant_boat_timeout_playtest()
    }

    fn ai_merchant_boat_timeout_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_merchant_missions.clear();
        self.merchant_mission_attempts.clear();
        self.turn = 0;
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Sea;
            tile.resource = None;
            tile.hut = false;
        }
        let home = (10, 24);
        let destination = (10, 40);
        for (owner, (x, y)) in [(1, home), (HUMAN, destination)] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.sign_pact(1, HUMAN, Pact::Alliance);
        let actor = self.spawn_unit(1, UnitKind::Freight, home.0, home.1);
        self.units.last_mut().unwrap().home_city = Some(self.cities[0].id);
        self.load_ai_merchant_ferries(1);
        let assigned = self
            .ai_merchant_missions
            .get(&actor)
            .is_some_and(|m| m.city == self.cities[1].id);
        self.cities[0].production = Production::Building(Building::PYRAMIDS);
        self.turn = 15;
        self.load_ai_merchant_ferries(1);
        let waited = self
            .ai_merchant_missions
            .get(&actor)
            .is_some_and(|m| m.city == self.cities[1].id);
        self.turn = 16;
        self.load_ai_merchant_ferries(1);
        assigned
            && waited
            && self
                .ai_merchant_missions
                .get(&actor)
                .is_some_and(|m| m.city == self.cities[0].id && m.wonder)
    }
}
