//! Automated gameplay scenarios covering movement behavior.

use super::super::normal_paths::GotoTask;
use super::*;

impl Simulation {
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
        let wrap = self.map_wrap;
        let east = |position: (i32, i32)| wrap.civ3_offset(position.0, position.1, 2, 0).unwrap();
        let first = east(start);
        let middle = east(first);
        let third = east(middle);
        let goal = east(third);
        for (x, y) in [start, first, middle, third, goal] {
            self.tile_mut(x, y).road = true;
        }
        self.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);
        let warrior = self.units.last().unwrap().clone();
        let route = self.plan_route(&warrior, goal.0, goal.1, true);
        self.tile_mut(middle.0, middle.1).terrain = Terrain::Sea;
        let terrain_hidden =
            !route.is_empty() && self.plan_route(&warrior, goal.0, goal.1, true) == route;
        self.tile_mut(middle.0, middle.1).terrain = Terrain::Grassland;
        self.tile_mut(middle.0, middle.1).seen = true;
        self.spawn_unit(1, UnitKind::Warrior, middle.0, middle.1);
        let unit_hidden = self.plan_route(&warrior, goal.0, goal.1, true) == route;
        self.tile_mut(middle.0, middle.1).visible = true;
        let detour = self.plan_route(&warrior, goal.0, goal.1, true);
        terrain_hidden && unit_hidden && !detour.is_empty() && detour != route
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
        tile.terrain = Terrain::Sea;
        tile.road = false;
        let units = std::mem::take(&mut self.units);
        self.create_city(1, middle.0, middle.1);
        self.units = units;
        let remembered =
            route.len() == 2 && self.plan_route(&warrior, goal.0, goal.1, true) == route;
        self.tile_mut(bridge.0, bridge.1).visible = true;
        let updated = self.plan_route(&warrior, goal.0, goal.1, true);
        remembered && updated.len() == 2 && updated != route
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
        let wrap = self.map_wrap;
        let east = |position: (i32, i32)| wrap.civ3_offset(position.0, position.1, 2, 0).unwrap();
        let first = east(start);
        let second = east(first);
        let goal = east(second);
        for (x, y) in [start, first, second, goal] {
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
        self.tile_mut(sea.0, sea.1).terrain = Terrain::Sea;
        let carrier = self.spawn_unit(HUMAN, UnitKind::Carrier, sea.0, sea.1);
        let helicopter = self.spawn_unit(HUMAN, UnitKind::Helicopter, sea.0, sea.1);
        assert!(self.board_specific_ship(helicopter, carrier));
        for _ in 1..UnitKind::Carrier.transport_capacity() {
            let fighter = self.spawn_unit(HUMAN, UnitKind::Fighter, sea.0, sea.1);
            assert!(self.board_specific_ship(fighter, carrier));
        }
        let transport = self.spawn_unit(HUMAN, UnitKind::Transport, sea.0, sea.1);
        let marine = self.spawn_unit(HUMAN, UnitKind::Marine, sea.0, sea.1);
        assert!(self.board_specific_ship(marine, transport));
        let rescued = self.apply(Command::Disband(transport, None))
            && self
                .units
                .iter()
                .any(|unit| unit.id == marine && unit.transport == Some(helicopter));
        let second = self.spawn_unit(HUMAN, UnitKind::Transport, sea.0, sea.1);
        let partisan = self.spawn_unit(HUMAN, UnitKind::Guerilla, sea.0, sea.1);
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

    pub(crate) fn cargo_rescue_playtest(&mut self) -> (bool, bool) {
        let sea = (10, 24);
        self.tile_mut(sea.0, sea.1).terrain = Terrain::Sea;
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

        (rescued, drowned)
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

    pub(crate) fn fleet_escort_playtest(&mut self) -> (bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Sea;
            tile.known_by[1] = true;
        }
        let sea = (10, 24);
        let guard = offset_native_by_map(sea.0, sea.1, -1, 0);
        let threat = offset_native_by_map(sea.0, sea.1, 1, 0);
        let ship = self.spawn_unit(1, UnitKind::Transport, sea.0, sea.1);
        let passenger = self.spawn_unit(1, UnitKind::Marine, sea.0, sea.1);
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
            tile.terrain = Terrain::Sea;
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
        let guard = self.spawn_unit(1, UnitKind::Marine, home.0, home.1);
        for _ in 0..4 {
            self.spawn_unit(1, UnitKind::Marine, home.0, home.1);
        }
        for _ in 0..3 {
            self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
            self.units.last_mut().unwrap().hp = 1;
        }
        let first = self.spawn_unit(1, UnitKind::Galley, home.0, home.1);
        let second_start = if remote { point(-5) } else { home };
        let second = self.spawn_unit(1, UnitKind::Galley, second_start.0, second_start.1);
        self.units.last_mut().unwrap().moves_left = 0;
        self.set_random_seed(1);
        self.run_ai_player(1);
        let waited = !remote
            || (self.cargo_count(first) == 0
                && self.cargo_count(second) == 0
                && self
                    .units
                    .iter()
                    .filter(|unit| unit.kind == UnitKind::Marine && (unit.x, unit.y) == home)
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
                    .zip(self.unit_by_id(second))
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
            tile.terrain = Terrain::Sea;
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
        let guard = self.spawn_unit(1, UnitKind::Marine, home.0, home.1);
        let attacker = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        // The transport policy needs a legal minimum assault force: one
        // home guard plus two military attackers.
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
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
                self.spawn_unit(1, UnitKind::Marine, home.0, home.1);
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

    pub(crate) fn activity_completion_order_playtest(&self) -> (bool, bool, bool, bool) {
        let fresh = || {
            let mut game = self.clone();
            game.units.clear();
            game.cities.clear();
            for tile in &mut game.tiles {
                tile.terrain = Terrain::Mountains;
                tile.road = false;
                tile.river = false;
                tile.base = None;
                tile.hut = false;
            }
            game
        };
        let road_mp = |worker_first: bool| {
            let mut game = fresh();
            let (worker, chariot) = if worker_first {
                let chariot = game.spawn_unit(HUMAN, UnitKind::Chariot, 10, 24);
                (game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24), chariot)
            } else {
                let worker = game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24);
                (worker, game.spawn_unit(HUMAN, UnitKind::Chariot, 10, 24))
            };
            game.begin_work(worker, WorkKind::Road);
            let unit = game
                .units
                .iter_mut()
                .find(|unit| unit.id == worker)
                .unwrap();
            unit.work_done = unit.work_left - ACTIVITY_FACTOR;
            unit.work_left = ACTIVITY_FACTOR;
            game.progress_player_unit_activities(HUMAN);
            let mp = game
                .units
                .iter()
                .find(|unit| unit.id == chariot)
                .unwrap()
                .moves_left;
            (game.tile(10, 24).road, mp)
        };
        let road_order =
            road_mp(true) == (true, 2 * MOVE_FRAGMENTS) && road_mp(false) == (true, MOVE_FRAGMENTS);

        let mut game = fresh();
        let later = game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24);
        let first = game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24);
        game.begin_work(first, WorkKind::Road);
        game.begin_work(later, WorkKind::Road);
        let unit = game.units.iter_mut().find(|unit| unit.id == first).unwrap();
        unit.work_done = unit.work_left - 20;
        unit.work_left = 20;
        game.set_random_seed(2);
        let before = game.random_snapshot();
        game.progress_player_unit_activities(HUMAN);
        let coworkers = game.tile(10, 24).road
            && game.random_snapshot() == before
            && [first, later].into_iter().all(|id| {
                game.units.iter().any(|unit| {
                    unit.id == id
                        && unit.work_kind.is_none()
                        && unit.veteran == crate::sim::civ3_unit_rules::default_experience_rank()
                })
            });

        let mut game = fresh();
        let first = game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24);
        let later = game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24);
        game.begin_work(first, WorkKind::Road);
        game.begin_work(later, WorkKind::Road);
        for unit in &mut game.units {
            unit.work_done = unit.work_left / 2;
            unit.work_left -= unit.work_done;
            unit.moves_left = 0;
        }
        game.progress_player_unit_activities(HUMAN);
        let precompleted = game.tile(10, 24).road
            && [first, later].into_iter().all(|id| {
                game.units
                    .iter()
                    .any(|unit| unit.id == id && unit.work_kind.is_none())
            });

        let mut game = fresh();
        let worker = game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24);
        game.begin_work(worker, WorkKind::Road);
        let initial_work = game.units.last().unwrap().work_left;
        game.units.last_mut().unwrap().moves_left = 0;
        game.set_random_seed(20);
        let before = game.random_snapshot();
        game.progress_player_unit_activities(HUMAN);
        let unit = game.unit_by_id(worker).unwrap();
        let no_zero_mp_activity = unit.veteran
            == crate::sim::civ3_unit_rules::default_experience_rank()
            && unit.work_done == 0
            && unit.work_left == initial_work
            && game.random_snapshot() == before;
        game.units.last_mut().unwrap().moves_left = 1;
        let full_rate = game.units.last().unwrap().activity_rate();
        game.progress_player_unit_activities(HUMAN);
        let no_work_promotion = game.units.last().unwrap().veteran
            == crate::sim::civ3_unit_rules::default_experience_rank()
            && game.units.last().unwrap().work_done == full_rate;
        (
            road_order,
            coworkers,
            precompleted,
            no_zero_mp_activity && no_work_promotion,
        )
    }

    pub(crate) fn unit_turn_lifecycle_playtest(&self) -> (bool, bool, bool, bool) {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        game.players[HUMAN].known = vec![Tech::BRONZE_WORKING];
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.hut = false;
            tile.owner = None;
            tile.known_by[HUMAN] = true;
        }
        game.create_city(HUMAN, 10, 24);
        game.cities[0].production = Production::Building(Building::BARRACKS);
        game.cities[0].shields = 100;
        game.cities[0].food = 10;
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
        game.units.last_mut().unwrap().hp = 1;
        game.restore_player_units(HUMAN);
        let ordinary_hp = game.unit_by_id(id).unwrap().hp;
        game.process_cities();
        let built = game.cities[0].has_building(Building::BARRACKS)
            && ordinary_hp < UnitKind::Warrior.stats().hp
            && game.unit_by_id(id).unwrap().hp == ordinary_hp;
        game.restore_player_units(HUMAN);
        let repaired = built && game.unit_by_id(id).unwrap().hp == UnitKind::Warrior.stats().hp;

        let field = offset_native_by_map(10, 24, 5, 0);
        let guard = game.spawn_unit(HUMAN, UnitKind::Warrior, field.0, field.1);
        game.units.last_mut().unwrap().hp = 1;
        let old_mp = game.units.last().unwrap().moves_left;
        game.fortify_unit(guard);
        let retained = game.units.last().unwrap().moves_left == old_mp;
        let guard_before_heal = game.unit_by_id(guard).unwrap().clone();
        let field_hp = 1 + Self::hitpoint_gain(&guard_before_heal, false);
        game.restore_player_units(HUMAN);
        let unfortified_hp = game.units.last().unwrap().hp == field_hp;
        game.progress_player_unit_activities(HUMAN);
        let fortified = retained
            && unfortified_hp
            && game.units.last().unwrap().hp == field_hp
            && game.units.last().unwrap().state == UnitState::Fortified;
        let exhausted = game.spawn_unit(HUMAN, UnitKind::Warrior, field.0, field.1);
        game.units.last_mut().unwrap().moves_left = 0;
        game.fortify_unit(exhausted);
        game.progress_player_unit_activities(HUMAN);
        let waited = game.units.last().unwrap().state == UnitState::Fortifying
            && game.units.last().unwrap().moves_left > 0;

        let base = offset_native_by_map(10, 24, 0, -1);
        game.tile_mut(base.0, base.1).base = Some(Base::Airbase);
        let air = offset_native_by_map(base.0, base.1, 0, -1);
        let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, air.0, air.1);
        game.units.last_mut().unwrap().fuel = 1;
        game.restore_player_units(HUMAN);
        let rescued = game
            .units
            .iter()
            .find(|unit| unit.id == bomber)
            .is_some_and(|unit| (unit.x, unit.y) == base && unit.moved);
        game.progress_player_unit_activities(HUMAN);
        let rescue_moved = rescued
            && game
                .units
                .iter()
                .find(|unit| unit.id == bomber)
                .is_some_and(|unit| unit.moved && unit.moves_left > 0);
        (repaired, fortified, waited, rescue_moved)
    }

    pub(crate) fn native_movement_rules_playtest(&mut self) -> (bool, bool, bool) {
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
        let home = (10, 24);
        let dest = offset_native_by_map(home.0, home.1, 1, 0);
        for p in [home, dest] {
            self.tile_mut(p.0, p.1).river = true;
        }
        let warrior = self.make_unit(0, HUMAN, UnitKind::Warrior, home.0, home.1);
        let chariot = self.make_unit(0, HUMAN, UnitKind::Chariot, home.0, home.1);
        let boat = self.make_unit(0, HUMAN, UnitKind::Galley, home.0, home.1);
        let river = self.movement_cost_for(&warrior, dest.0, dest.1) == Some(2)
            && self.movement_cost_for(&chariot, dest.0, dest.1) == Some(6)
            && self.movement_cost_for(&boat, dest.0, dest.1) == Some(6);
        for p in [home, dest] {
            self.tile_mut(p.0, p.1).railroad = true;
        }
        let rail = self.movement_cost_for(&warrior, dest.0, dest.1) == Some(1);
        self.tile_mut(dest.0, dest.1).owner = Some(1);
        let hostile = self.movement_cost_for(&warrior, dest.0, dest.1) == Some(6);
        self.tile_mut(dest.0, dest.1).owner = None;
        self.tile_mut(dest.0, dest.1).terrain = Terrain::Mountains;
        self.tile_mut(dest.0, dest.1).railroad = false;
        let wheel_blocked = self.movement_cost_for(&chariot, dest.0, dest.1).is_none();
        self.tile_mut(dest.0, dest.1).road = true;
        let wheel_road = self.movement_cost_for(&chariot, dest.0, dest.1).is_some();
        self.tile_mut(dest.0, dest.1).terrain = Terrain::Sea;
        self.spawn_unit(HUMAN, UnitKind::Transport, dest.0, dest.1);
        let explorer = self.spawn_unit(HUMAN, UnitKind::Explorer, home.0, home.1);
        let before = self
            .units
            .iter()
            .find(|u| u.id == explorer)
            .unwrap()
            .moves_left;
        let boarding = self.try_move_unit(explorer, 1, 0) == MoveResult::Moved
            && self
                .units
                .iter()
                .find(|u| u.id == explorer)
                .is_some_and(|u| u.transport.is_some() && u.moves_left == before - 2);
        self.units.clear();
        let enemy = self.spawn_unit(1, UnitKind::Cavalry, home.0, home.1);
        let leader_zoc = self.under_foreign_control(HUMAN, home.0, home.1, false);
        self.units.iter_mut().find(|u| u.id == enemy).unwrap().kind = UnitKind::Carrier;
        let carrier_no_zoc = !self.under_foreign_control(HUMAN, home.0, home.1, false);
        let actors = !UnitKind::Worker.ignores_zoc()
            && !UnitKind::Chariot.ignores_zoc()
            && !UnitKind::Galley.ignores_zoc()
            && UnitKind::Explorer.ignores_zoc();
        (
            river && rail && hostile,
            wheel_blocked && wheel_road && boarding,
            leader_zoc && carrier_no_zoc && actors,
        )
    }

    pub(crate) fn civilian_border_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.road = false;
            tile.railroad = false;
            tile.owner = None;
            tile.visible = true;
            tile.seen = true;
            tile.known_by[HUMAN] = true;
        }
        self.create_city(1, 20, 40);
        self.sign_pact(HUMAN, 1, Pact::Peace);
        self.diplomacy_playtest_tick(16);
        if self.pact(HUMAN, 1) != Pact::Peace {
            return false;
        }
        let home = (10, 24);
        let goal = offset_native_by_map(home.0, home.1, 1, 0);
        for (kind, allowed) in [
            (UnitKind::Worker, true),
            (UnitKind::Settler, true),
            (UnitKind::Explorer, true),
            (UnitKind::Caravel, false),
            (UnitKind::Warrior, false),
            (UnitKind::Carrier, false),
        ] {
            self.units.clear();
            for position in [home, goal] {
                self.tile_mut(position.0, position.1).terrain = if kind.is_naval() {
                    Terrain::Sea
                } else {
                    Terrain::Grassland
                };
            }
            self.tile_mut(goal.0, goal.1).owner = Some(1);
            // Peace permits defenseless civilians to pass, but does not grant
            // military units right of passage. Check planning and execution.
            for tile in &mut self.tiles {
                tile.visible = true;
                tile.seen = true;
            }
            let id = self.spawn_unit(HUMAN, kind, home.0, home.1);
            let unit = self.units.last().unwrap().clone();
            let planned = self.plan_route(&unit, goal.0, goal.1, true);
            let moved = self.try_move_unit(id, 1, 0) == MoveResult::Moved;
            if planned.is_empty() == allowed || moved != allowed {
                return false;
            }
        }
        self.units.clear();
        self.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
        self.tile_mut(goal.0, goal.1).terrain = Terrain::Grassland;
        let explorer = self.spawn_unit(HUMAN, UnitKind::Explorer, home.0, home.1);
        self.spawn_unit(1, UnitKind::Warrior, goal.0, goal.1);
        self.try_move_unit(explorer, 1, 0) == MoveResult::Blocked
    }

    pub(crate) fn damaged_movement_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.players[HUMAN]
            .known
            .retain(|&tech| tech != Tech::NUCLEAR_POWER);
        self.tile_mut(10, 24).terrain = Terrain::Sea;
        self.tile_mut(12, 24).terrain = Terrain::Grassland;
        let ship_id = self.spawn_unit(HUMAN, UnitKind::Destroyer, 10, 24);
        let horse = self.spawn_unit(HUMAN, UnitKind::Horseman, 12, 24);
        for unit in &mut self.units {
            unit.hp = 1;
            unit.moves_left = 0;
            unit.moved = true;
        }
        self.update_activities_and_restore();
        let restored = [ship_id, horse].into_iter().all(|id| {
            self.units
                .iter()
                .find(|unit| unit.id == id)
                .is_some_and(|unit| unit.moves_left == self.unit_move_rate(unit))
        });
        let ship = self
            .units
            .iter_mut()
            .find(|unit| unit.id == ship_id)
            .unwrap();
        ship.hp = ship.max_hit_points();
        self.players[HUMAN].known.push(Tech::NUCLEAR_POWER);
        let ship = self.unit_by_id(ship_id).unwrap();
        restored && self.unit_move_rate(ship) == self.unit_move_allowance(HUMAN, ship.kind)
    }

    pub(crate) fn ai_source_goto_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Sea;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.base = None;
            tile.owner = None;
            tile.hut = false;
            tile.known_by.fill(true);
        }
        let id = self.spawn_unit(1, UnitKind::Caravel, 10, 20);
        let map_wrap = self.map_wrap;
        let east = move |point: (i32, i32), tiles: usize| {
            (0..tiles).fold(point, |(x, y), _| map_wrap.civ3_neighbor(x, y, 2).unwrap())
        };
        let stack = east((10, 20), 1);
        let goal = east((10, 20), 2);
        for _ in 0..6 {
            self.spawn_unit(1, UnitKind::Battleship, stack.0, stack.1);
        }
        let actor = self
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        let risky_detour = self
            .ai_goto_path(&actor, goal, GotoTask::None)
            .is_some_and(|(_, cost)| cost == 3 * MOVE_FRAGMENTS);
        let result = self.ai_goto_unit(id, goal, GotoTask::None);
        let sailed = result.alive
            && result.moved
            && result.at_goal
            && self.unit_goto_tiles.get(&id) == Some(&goal);
        self.units.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
        }
        let id = self.spawn_unit(1, UnitKind::Swordsman, 10, 20);
        let target = east((10, 20), 1);
        let defender = self.spawn_unit(BARBARIANS, UnitKind::Warrior, target.0, target.1);
        self.unit_goto_tiles.insert(id, (20, 40));
        let result = self.ai_goto_unit(id, target, GotoTask::Attack);
        risky_detour
            && sailed
            && result.alive
            && result.moved
            && self.units.iter().all(|unit| unit.id != defender)
            && self.unit_goto_tiles.get(&id) == Some(&(20, 40))
    }
}
