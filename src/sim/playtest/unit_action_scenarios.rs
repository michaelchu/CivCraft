//! Automated gameplay scenarios covering unit action behavior.

use super::*;

impl Simulation {
    // The desktop self-test advances treaty timers with the armies held in place.

    pub(crate) fn cooperative_work_playtest_setup(&mut self) -> (u32, u32) {
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

    pub(crate) fn upgrade_playtest_setup(&mut self) -> u32 {
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

    pub(crate) fn zone_control_playtest_setup(&mut self) -> (u32, u32, (i32, i32)) {
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

    pub(crate) fn oil_platform_playtest_setup(&mut self) -> u32 {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = offset_native_by_map(city.x, city.y, 1, 0);
        self.players[HUMAN].known.push(Tech::MINIATURIZATION);
        self.tile_mut(x, y).terrain = Terrain::Ocean;
        self.spawn_unit(HUMAN, UnitKind::Transport, x, y)
    }

    pub(crate) fn naval_cleanup_playtest_setup(&mut self) -> u32 {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = offset_native_by_map(city.x, city.y, 1, 0);
        let tile = self.tile_mut(x, y);
        tile.terrain = Terrain::Sea;
        tile.pollution = true;
        self.spawn_unit(HUMAN, UnitKind::Transport, x, y)
    }

    pub(crate) fn cleanup_playtest_setup(&mut self) -> u32 {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = (city.x, city.y);
        let tile = self.tile_mut(x, y);
        tile.pollution = true;
        tile.fallout = true;
        self.spawn_unit(HUMAN, UnitKind::Worker, x, y)
    }

    pub(crate) fn base_playtest_setup(&mut self) -> u32 {
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

    pub(crate) fn pillage_playtest_setup(&mut self) -> (u32, (i32, i32)) {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = offset_native_by_map(city.x, city.y, 1, 0);
        let tile = self.tile_mut(x, y);
        tile.terrain = Terrain::Plains;
        tile.road = true;
        tile.railroad = true;
        (self.spawn_unit(HUMAN, UnitKind::Warrior, x, y), (x, y))
    }

    pub(crate) fn farmland_playtest_setup(&mut self) -> u32 {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = (city.x, city.y);
        self.players[HUMAN].known.extend([
            Tech::SANITATION,
            Tech::REFRIGERATION,
            Tech::ELECTRICITY,
        ]);
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
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = (city.x, city.y);
        let sea = offset_native_by_map(x, y, 1, 0);
        let shore = offset_native_by_map(x, y, 2, 0);
        self.tile_mut(sea.0, sea.1).terrain = Terrain::Sea;
        self.tile_mut(shore.0, shore.1).terrain = Terrain::Grassland;
        let ship = self.spawn_unit(HUMAN, UnitKind::Caravel, x, y);
        let passenger = self.spawn_unit(HUMAN, UnitKind::Settler, x, y);
        (ship, passenger, shore)
    }

    // The desktop self-test advances treaty timers with the armies held in place.

    pub(crate) fn transport_worker_actions_playtest(&self) -> bool {
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
        let built = gated
            && started
            && boarded
            && game.tile(10, 24).oil_platform
            && game
                .unit_by_id(worker)
                .is_some_and(|unit| unit.work_kind.is_none() && unit.transport == Some(ship));
        built
    }

    pub(crate) fn transport_hud_playtest_setup(&mut self) -> u32 {
        self.units.clear();
        self.cities.clear();
        let position = (20, 24);
        let tile = self.tile_mut(position.0, position.1);
        tile.terrain = Terrain::Ocean;
        tile.oil_platform = false;
        tile.pollution = true;
        self.players[HUMAN].known.push(Tech::MINIATURIZATION);
        let id = self.spawn_unit(HUMAN, UnitKind::Transport, position.0, position.1);
        self.refresh_visibility();
        id
    }
}
