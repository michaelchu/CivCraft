//! Automated gameplay scenarios covering settlement behavior.

use super::*;

impl Simulation {
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
        self.tile_mut(sx, sy).terrain = Terrain::Sea;
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
            tile.terrain = Terrain::Sea;
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
            tile.terrain = Terrain::Sea;
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
            tile.terrain = Terrain::Sea;
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
            tile.terrain = Terrain::Sea;
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

    pub(crate) fn ai_settlement_goal_government_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_settlement_targets.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.road = true;
            tile.river = true;
        }
        self.create_city(1, 5, 8);
        self.cities[0].size = 4;
        self.reconcile_city_workers();
        self.players[1].government = Government::Anarchy;
        self.players[1].known = self.research_path(1, Tech::MONARCHY);
        self.players[1].known.retain(|&tech| tech != Tech::MONARCHY);
        self.players[1].rates = [60, 40, 0];
        let goal = self.ai_goal_government(1);
        let map = self.settlement_citymap(1, None);
        let current = self.city_site_result(1, 15, 40, Government::Anarchy, &map);
        let projected = self.city_site_result(1, 15, 40, goal, &map);
        goal.requirement()
            .is_some_and(|tech| !self.has_tech(1, tech))
            && current.zip(projected).is_some_and(|(current, projected)| {
                current.value != projected.value
                    && self.city_site_value(1, 15, 40) == Some(projected.value)
            })
            && self.players[1].government == Government::Anarchy
            && self.players[1].rates == [60, 40, 0]
    }

    pub(crate) fn ai_settlement_reservations_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_settlement_targets.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.owner = None;
            tile.road = true;
            tile.irrigation = true;
            tile.farmland = false;
            tile.mine = false;
            tile.river = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.hut = false;
        }
        self.players[1].government = Government::Communism;
        let first_id = self.spawn_unit(1, UnitKind::Settlers, 2, 8);
        let second_id = self.spawn_unit(1, UnitKind::Settlers, 20, 24);
        let first = self
            .units
            .iter()
            .find(|unit| unit.id == first_id)
            .unwrap()
            .clone();
        let second = self
            .units
            .iter()
            .find(|unit| unit.id == second_id)
            .unwrap()
            .clone();
        let destination = (10, 24);
        let baseline: Vec<_> = (0..self.tiles.len())
            .map(|index| self.map_wrap.position(index))
            .map(|(x, y)| self.city_site_value(1, x, y))
            .collect();
        self.reserve_settlement_site(&first, destination);
        let Some(reservation) = self.ai_settlement_targets.get(&first_id).copied() else {
            return false;
        };
        let protected = self.city_site_value(1, 10, 24).is_none()
            && reservation.worker.is_some_and(|(x, y)| {
                self.settlement_citymap(1, None)[self.map_wrap.index(x, y)] < 0
            });
        let crowding = (0..self.tiles.len()).any(|index| {
            let (x, y) = self.map_wrap.position(index);
            baseline[index]
                .zip(self.city_site_value(1, x, y))
                .is_some_and(|(old, new)| new < old)
        });
        let distinct = self.best_city_site(&second).is_some_and(|(target, _, _)| {
            self.map_distance(target.0, target.1, destination.0, destination.1) >= 3
        });
        self.units
            .iter_mut()
            .find(|unit| unit.id == first_id)
            .unwrap()
            .moves_left = 0;
        let waiting = self
            .units
            .iter()
            .find(|unit| unit.id == first_id)
            .unwrap()
            .clone();
        self.run_ai_founder(&waiting);
        let retained = self
            .ai_settlement_targets
            .get(&first_id)
            .is_some_and(|entry| entry.destination == destination);
        self.units
            .iter_mut()
            .find(|unit| unit.id == first_id)
            .unwrap()
            .owner = HUMAN;
        self.prune_settlement_reservations(1);
        let released = !self.ai_settlement_targets.contains_key(&first_id)
            && self.city_site_value(1, 10, 24).is_some();
        protected && crowding && distinct && retained && released
    }

    pub(crate) fn ai_settlement_value_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = None;
            tile.owner = None;
            tile.road = true;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.river = false;
            tile.base = None;
            tile.pollution = false;
            tile.fallout = false;
        }
        self.players[1].government = Government::Communism;
        self.players[1].researching = Tech::BRONZE_WORKING;
        let home = (10, 24);
        let baseline = self.city_site_value(1, home.0, home.1);
        // Source: center115*2 (2food/2shield/1trade) + best68
        // +19 individually truncated68/8 for the 20 surrounding BFC tiles.
        let expected = f64::from(298.0_f32 + 100.0_f32 / 299.0 * 200.0 + 152.0);
        let rounded = baseline.is_some_and(|value| (value - expected).abs() < 0.0001);
        let mut reservations = vec![0; self.tiles.len()];
        let reserved_tile = self
            .map_city_radius_at(home.0, home.1)
            .find(|&(_, _, x, y)| (x, y) != home)
            .map(|(_, _, x, y)| (x, y))
            .unwrap();
        reservations[self.map_wrap.index(reserved_tile.0, reserved_tile.1)] = 1;
        let crowded = self
            .city_site_result(1, home.0, home.1, Government::Communism, &reservations)
            .is_some_and(|site| site.value < baseline.unwrap());
        self.tile_mut(home.0, home.1).pollution = true;
        rounded && crowded && self.city_site_value(1, home.0, home.1).is_none()
    }
}

impl Simulation {
    pub(crate) fn founder_work_arbitration_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_settlement_targets.clear();
        self.ai_government_cache.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = Some(Resource::Wheat);
            tile.owner = None;
            tile.hut = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.road = true;
            tile.railroad = true;
            tile.irrigation = true;
            tile.farmland = true;
            tile.river = false;
            tile.mine = false;
        }
        self.players[1].government = Government::Communism;
        self.create_city(1, 12, 24);
        self.tile_mut(12, 24).pollution = true;
        self.climate_current[0] = 1000;
        let id = self.spawn_unit(1, UnitKind::Settlers, 12, 24);
        let founder = self
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        let Some((destination, _, city_want)) = self.best_city_site(&founder) else {
            return false;
        };
        let improvement = self.worker_improvement_want(&founder);
        if city_want <= 0.0 || improvement <= city_want as f32 {
            return false;
        }
        let mut committed = self.clone();
        committed.reserve_settlement_site(&founder, destination);
        let retained = committed.run_ai_founder_with_improvement(&founder, improvement)
            && (committed
                .ai_settlement_targets
                .get(&id)
                .is_some_and(|reservation| reservation.destination == destination)
                || committed
                    .cities
                    .iter()
                    .any(|city| (city.x, city.y) == destination));
        self.run_ai_founder(&founder);
        retained
            && !self.ai_settlement_targets.contains_key(&id)
            && self.cities.len() == 1
            && self.units.iter().any(|unit| {
                unit.id == id && unit.work_kind == Some(WorkKind::Clean(Contamination::Pollution))
            })
    }
}

impl Simulation {
    pub(crate) fn settlement_reservation_order_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_settlement_targets.clear();
        self.ai_government_cache.clear();
        self.city_rapture.clear();
        self.players[1].government = Government::Communism;
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = None;
            tile.owner = None;
            tile.road = true;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.oil_well = false;
            tile.oil_platform = false;
            tile.river = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.base = None;
            tile.hut = false;
        }
        let home = (12, 24);
        let id = self.spawn_unit(1, UnitKind::Settlers, home.0, home.1);
        let founder = self
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        let mut ordered: Vec<_> = self.map_wrap.city_radius(home.0, home.1).collect();
        ordered.sort_by_key(|&(dx, dy, _, _)| city_radius_order_key(dx, dy));
        let mut workers = ordered
            .into_iter()
            .filter(|&(_, _, x, y)| (x, y) != home)
            .map(|(_, _, x, y)| (x, y));
        let first = workers.next().unwrap();
        let second = workers.next().unwrap();
        let mut map = vec![0; self.tiles.len()];
        let initial = self.city_site_result(1, home.0, home.1, Government::Communism, &map);
        let source_first = initial
            .as_ref()
            .is_some_and(|site| site.worker == Some(first));
        map[self.map_wrap.index(first.0, first.1)] = 1;
        let next = self.city_site_result(1, home.0, home.1, Government::Communism, &map);
        let source_next = next
            .as_ref()
            .is_some_and(|site| site.worker == Some(second))
            && initial.zip(next).is_some_and(|(a, b)| a.value > b.value);
        map[self.map_wrap.index(first.0, first.1)] = -1;
        let overlap_rejected = self
            .city_site_result(1, home.0, home.1, Government::Communism, &map)
            .is_none();
        self.reserve_settlement_site(&founder, home);
        let stored = self
            .ai_settlement_targets
            .get(&id)
            .is_some_and(|reservation| {
                reservation.worker == Some(first) && reservation.destination == home
            });
        let reserved =
            self.settlement_citymap(1, None)[self.map_wrap.index(first.0, first.1)] == -1;
        source_first && source_next && overlap_rejected && stored && reserved
    }
}
