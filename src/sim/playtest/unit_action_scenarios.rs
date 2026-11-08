//! Automated gameplay scenarios covering unit action behavior.

use super::*;
use super::super::workers::WorkerRequest;

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

    pub(crate) fn work_promotion_playtest_tick(&mut self) {
        self.set_random_seed(20);
        self.update_activities_and_restore();
    }

    pub(crate) fn upgrade_playtest_setup(&mut self) -> u32 {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
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
        self.spawn_unit(HUMAN, UnitKind::Engineer, x, y)
    }

    pub(crate) fn base_playtest_setup(&mut self) -> u32 {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
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
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = offset_native_by_map(city.x, city.y, 1, 0);
        let tile = self.tile_mut(x, y);
        tile.terrain = Terrain::Plains;
        tile.road = true;
        tile.railroad = true;
        (self.spawn_unit(HUMAN, UnitKind::Warrior, x, y), (x, y))
    }

    pub(crate) fn terrain_work_playtest_setup(&mut self) -> u32 {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = (city.x, city.y);
        self.players[HUMAN]
            .known
            .extend([Tech::EXPLOSIVES, Tech::NUCLEAR_POWER]);
        self.tile_mut(x, y).terrain = Terrain::Tundra;
        self.spawn_unit(HUMAN, UnitKind::Engineer, x, y)
    }

    pub(crate) fn farmland_playtest_setup(&mut self) -> u32 {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
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
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = (city.x, city.y);
        let sea = offset_native_by_map(x, y, 1, 0);
        let shore = offset_native_by_map(x, y, 2, 0);
        self.tile_mut(sea.0, sea.1).terrain = Terrain::Sea;
        self.tile_mut(shore.0, shore.1).terrain = Terrain::Grassland;
        let ship = self.spawn_unit(HUMAN, UnitKind::Caravel, x, y);
        let passenger = self.spawn_unit(HUMAN, UnitKind::Settlers, x, y);
        (ship, passenger, shore)
    }

    // The desktop self-test advances treaty timers with the armies held in place.

    pub(crate) fn transport_worker_actions_playtest(&self) -> (bool, bool) {
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
        let ship = game.spawn_unit(HUMAN, UnitKind::Transport, 10, 24);
        let gated = !game.apply(Command::Work(ship, WorkKind::OilPlatform));
        game.players[HUMAN].known.push(Tech::MINIATURIZATION);
        let started = game.apply(Command::Work(ship, WorkKind::OilPlatform));
        for _ in 0..2 {
            game.progress_player_unit_activities(HUMAN);
        }
        let built = gated
            && started
            && game.tile(10, 24).oil_platform
            && game.units.last().unwrap().work_kind.is_none()
            && game.units.last().unwrap().veteran == 0;
        game.tile_mut(10, 24).pollution = true;
        let clean = game.apply(Command::Work(
            ship,
            WorkKind::Clean(Contamination::Pollution),
        ));
        game.progress_player_unit_activities(HUMAN);
        let cleaned = clean && !game.tile(10, 24).pollution;

        // A real port allows a Sea actor onto land, but its action enablers
        // still exclude land mining, irrigation, cultivation and planting.
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        game.create_city(HUMAN, 10, 24);
        game.players[HUMAN]
            .known
            .extend([Tech::ELECTRICITY, Tech::FUSION_POWER]);
        let rejects_land = [
            WorkKind::Mine,
            WorkKind::Irrigation,
            WorkKind::Farmland,
            WorkKind::Plant,
            WorkKind::Cultivate,
            WorkKind::Transform,
        ]
        .into_iter()
        .all(|kind| !game.apply(Command::Work(ship, kind)));
        (built && cleaned, rejects_land)
    }

    pub(crate) fn world_seam_work_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.hut = false;
            tile.river = false;
            tile.irrigation = false;
            tile.owner = None;
        }
        let home = (10, 0);
        let site = (10, 1);
        let water = offset_native_by_map(site.0, site.1, -1, -1);
        self.tile_mut(water.0, water.1).terrain = Terrain::Sea;
        self.create_city(HUMAN, home.0, home.1);
        let port = self.is_port(&self.cities[0]);
        let radius =
            Self::city_radius(&self.cities[0]).count() == Self::city_radius_at(10, 24).count();
        let claimed = self.tile(water.0, water.1).owner == Some(HUMAN);
        let worker = self.spawn_unit(HUMAN, UnitKind::Worker, site.0, site.1);
        let started = self.apply(Command::Work(worker, WorkKind::Irrigation));
        for _ in 0..5 {
            self.update_activities_and_restore();
        }
        water.1 == self.map_wrap.height() - 1
            && port
            && radius
            && claimed
            && started
            && self.tile(site.0, site.1).irrigation
    }

    pub(crate) fn transport_auto_worker_playtest(&self) -> bool {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        game.players[1].known.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.resource = None;
            tile.owner = None;
            tile.pollution = false;
            tile.fallout = false;
            tile.hut = false;
            tile.river = false;
            tile.base = None;
            tile.known_by[1] = true;
        }
        let center = (10, 24);
        let target = offset_native_by_map(center.0, center.1, 1, 0);
        game.tile_mut(center.0, center.1).terrain = Terrain::Grassland;
        game.create_city(1, center.0, center.1);
        game.cities[0].worked_tiles = vec![target];
        game.cities[0].specialists = [0; 3];
        game.tile_mut(target.0, target.1).pollution = true;
        let ship = game.spawn_unit(1, UnitKind::Transport, center.0, center.1);
        let mut requests = vec![WorkerRequest {
            city: game.cities[0].id,
            destination: target,
            activity: WorkKind::Clean(Contamination::Pollution),
            want: 1,
        }];
        game.run_worker_phase_with_requests(1, &mut requests);
        let assigned = game.units.iter().any(|unit| {
            unit.id == ship
                && (unit.x, unit.y) == target
                && unit.work_kind == Some(WorkKind::Clean(Contamination::Pollution))
        });
        game.progress_player_unit_activities(1);
        assigned && !game.tile(target.0, target.1).pollution
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

    pub(crate) fn native_roster_actions_playtest(&self) -> (bool, bool, bool, bool) {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.base = None;
        }
        game.players[HUMAN].known.clear();
        game.players[HUMAN].government = Government::Despotism;
        game.create_city(HUMAN, 10, 20);
        let city = game.cities[0].clone();
        let unavailable = [
            UnitKind::Pikemen,
            UnitKind::Riflemen,
            UnitKind::Armor,
            UnitKind::Awacs,
            UnitKind::Fanatics,
            UnitKind::Explorer,
            UnitKind::Leader,
            UnitKind::Storm,
        ]
        .into_iter()
        .all(|kind| !game.can_produce(&city, Production::Unit(kind)));
        game.players[HUMAN].known.extend([
            Tech::all().find(|tech| tech.name() == "Feudalism").unwrap(),
            Tech::BRONZE_WORKING,
        ]);
        let pikemen = game.can_produce(&city, Production::Unit(UnitKind::Pikemen))
            && !game.can_produce(&city, Production::Unit(UnitKind::Phalanx));
        game.players[HUMAN].known.push(Tech::AMPHIBIOUS_WARFARE);
        // A later successor obsoletes the entire chain even when intermediate
        // technologies are absent (scenario grants and traded technologies).
        let successor = game.available_unit_successor(HUMAN, UnitKind::Phalanx)
            == Some(UnitKind::Marines)
            && !game.can_produce(&city, Production::Unit(UnitKind::Pikemen));
        game.players[HUMAN].known.extend([
            Tech::NATIONALISM,
            Tech::all().find(|tech| tech.name() == "Seafaring").unwrap(),
        ]);
        let gov_gate = !game.unit_available(HUMAN, UnitKind::Fanatics);
        game.players[HUMAN].government = Government::Fundamentalism;
        let requirements = unavailable
            && pikemen
            && successor
            && gov_gate
            && game.unit_available(HUMAN, UnitKind::Fanatics)
            && game.unit_available(HUMAN, UnitKind::Explorer);

        let defender = game.make_unit(0, HUMAN, UnitKind::Warrior, city.x, city.y);
        let firepower = [
            UnitKind::Artillery,
            UnitKind::Howitzer,
            UnitKind::CruiseMissile,
        ]
        .into_iter()
        .all(|kind| {
            let attacker = game.make_unit(1, 1, kind, city.x + 1, city.y);
            let city_fp = game.combat_firepower(&attacker, &defender).0;
            let mut field = defender.clone();
            field.x += 4;
            city_fp == 2 * kind.stats().firepower
                && game.combat_firepower(&attacker, &field).0 == kind.stats().firepower
        });
        let pos = (14, 24);
        game.tile_mut(pos.0, pos.1).terrain = Terrain::Mountains;
        game.tile_mut(pos.0, pos.1).river = true;
        let big_land = [
            UnitKind::MechInf,
            UnitKind::Armor,
            UnitKind::Artillery,
            UnitKind::Howitzer,
        ]
        .into_iter()
        .all(|kind| {
            let unit = game.make_unit(0, HUMAN, kind, pos.0, pos.1);
            !game.pf_native_tile(kind, pos.0, pos.1)
                && game.pf_extra_class_defense(kind, pos.0, pos.1) == 0
                && game.unit_move_rate(&unit)
                    == ((kind.stats().moves - 1) * MOVE_FRAGMENTS).max(MOVE_FRAGMENTS)
        });
        let combat = firepower && big_land;

        game.players[HUMAN].government = Government::Monarchy;
        for _ in 0..5 {
            let id = game.spawn_unit(HUMAN, UnitKind::Fanatics, city.x, city.y);
            let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
            unit.home_city = Some(city.id);
        }
        let support = game.city_unit_upkeep(&city);
        let normal_food = support.food == 1 && support.gold == 0 && support.shields == 0;
        game.players[HUMAN].government = Government::Fundamentalism;
        let support = game.city_unit_upkeep(&city);
        let free = support.food == 0 && support.gold == 0 && support.shields == 0;
        let id = game.units[0].id;
        let joined = game.join_city(id, city.id)
            && game.cities[0].size == city.size + 1
            && !game.units.iter().any(|unit| unit.id == id);
        let upkeep = normal_food && free && joined;

        let flags = [
            UnitKind::Legion,
            UnitKind::Musketeers,
            UnitKind::Riflemen,
            UnitKind::Marines,
        ]
        .into_iter()
        .all(UnitKind::has_marine_attack)
            && !UnitKind::Riflemen.can_helicopter_field_load()
            && [
                UnitKind::AlpineTroops,
                UnitKind::Fanatics,
                UnitKind::Partisan,
                UnitKind::Marines,
                UnitKind::Paratroopers,
            ]
            .into_iter()
            .all(UnitKind::can_helicopter_field_load)
            && [
                UnitKind::Pikemen,
                UnitKind::Riflemen,
                UnitKind::AlpineTroops,
                UnitKind::Fanatics,
            ]
            .into_iter()
            .all(UnitKind::can_build_fortress)
            && [UnitKind::Leader, UnitKind::Storm, UnitKind::BarbarianLeader]
                .into_iter()
                .all(UnitKind::is_unbribable);
        (requirements, combat, upkeep, flags)
    }
}
