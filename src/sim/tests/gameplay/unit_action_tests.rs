//! Unit checks and fixtures for private simulation behavior.

use super::*;

impl Simulation {
    pub(crate) fn cooperative_work_fixture(&mut self) -> (u32, u32) {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = (city.x, city.y);
        let tile = self.tile_mut(x, y);
        tile.terrain = Terrain::Plains;
        tile.river = true;
        tile.irrigation = false;
        let first = self.spawn_unit(HUMAN, UnitKind::Worker, x, y);
        let second = self.spawn_unit(HUMAN, UnitKind::Worker, x, y);
        (first, second)
    }

    pub(crate) fn upgrade_fixture(&mut self) -> u32 {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = (city.x, city.y);
        self.players[HUMAN]
            .known
            .extend([Tech::IRON_WORKING, Tech::INVENTION]);
        self.players[HUMAN].gold = 200;
        let id = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        let unit = self.units.last_mut().unwrap();
        unit.veteran = 2;
        unit.hp = 5;
        id
    }

    pub(crate) fn veteran_fixture(&mut self) -> (u32, usize) {
        let city = self
            .cities
            .iter_mut()
            .find(|city| city.owner == HUMAN)
            .unwrap();
        city.buildings.push(Building::BARRACKS);
        city.shields = UnitKind::Warrior.stats().cost;
        // A city founded during the current turn has not produced yet.
        city.founded_turn = self.turn - 1;
        if !self.players[HUMAN].known.contains(&Tech::WARRIOR_CODE) {
            self.players[HUMAN].known.push(Tech::WARRIOR_CODE);
        }
        let id = city.id;
        let expected_rank = self.trained_rank(self.city_by_id(id).unwrap(), UnitKind::Warrior);
        (id, expected_rank)
    }

    pub(crate) fn zone_control_fixture(&mut self) -> (u32, u32, (i32, i32)) {
        self.units.clear();
        let (x, y) = (10, 20);
        let enemy = offset_native_by_map(x, y, 1, 1);
        let destination = offset_native_by_map(x, y, 1, 0);
        for (tx, ty) in [(x, y), enemy, destination] {
            self.tile_mut(tx, ty).terrain = Terrain::Grassland;
        }
        self.spawn_unit(1, UnitKind::Cavalry, enemy.0, enemy.1);
        let warrior = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        let explorer = self.spawn_unit(HUMAN, UnitKind::Explorer, x, y);
        (warrior, explorer, destination)
    }

    pub(crate) fn base_fixture(&mut self) -> u32 {
        if self.cities_owned_by(HUMAN).next().is_none() {
            let (x, y) = self
                .units
                .iter()
                .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
                .map(|unit| (unit.x, unit.y))
                .expect("the base-building fixture needs a human Settler");
            self.create_city(HUMAN, x, y);
        }
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = offset_native_by_map(city.x, city.y, 1, 0);
        self.players[HUMAN]
            .known
            .extend([Tech::CONSTRUCTION, Tech::FLIGHT]);
        let tile = self.tile_mut(x, y);
        tile.terrain = Terrain::Plains;
        tile.river = false;
        self.spawn_unit(HUMAN, UnitKind::Worker, x, y)
    }

    pub(crate) fn transport_worker_actions_test(&self) -> bool {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        game.players[HUMAN].known.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.oil_platform = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.hut = false;
            tile.owner = None;
            tile.base = None;
        }
        game.tile_mut(10, 24).resource = Some(Resource::Oil);
        let ship = game.spawn_unit(HUMAN, UnitKind::Transport, 10, 24);
        let worker = game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24);
        let boarded = game.apply(Command::BoardOrDeboard(worker));
        let gated = !game.apply(Command::Work(worker, WorkKind::OilPlatform));
        game.players[HUMAN].known.push(Tech::MINIATURIZATION);
        let started = game.apply(Command::Work(worker, WorkKind::OilPlatform));
        for _ in 0..10 {
            game.progress_player_unit_activities(HUMAN);
        }

        gated
            && started
            && boarded
            && game.tile(10, 24).oil_platform
            && game
                .unit_by_id(worker)
                .is_some_and(|unit| unit.work_kind.is_none() && unit.transport == Some(ship))
    }
}
