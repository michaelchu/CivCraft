//! Unit checks and fixtures for private simulation behavior.

use super::*;

impl Simulation {
    pub(crate) fn ferry_research_test(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        let (x, y) = (10, 24);
        for (nx, ny) in self.map_wrap.eight_way_neighbors(x, y).chain([(x, y)]) {
            self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        }
        self.create_city(1, x, y);
        self.players[1].known = vec![Tech::ALPHABET];
        self.players[1].research_goal = Tech::BRONZE_WORKING;
        let inland = self.ferry_tech_want(1, Tech::MAP_MAKING) == 0
            && self.research_target(1) == Tech::BRONZE_WORKING;
        // The inland case has several good local sites. Once it is turned
        // into a coastal port, remove those sites so the C2C3 settler advisor
        // has a real ferry need (daisettler.c stops at RESULT_IS_ENOUGH).
        let center_idx = self.map_wrap.index(x, y);
        for (idx, tile) in self.tiles.iter_mut().enumerate() {
            if idx != center_idx {
                tile.terrain = Terrain::Sea;
            }
        }
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

    pub(crate) fn ai_expansion_test(&mut self) -> (bool, bool, bool) {
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
        // The historical oracle map is only 48 tiles tall, so keep the
        // third established city inside its bounded southern edge.
        for (cx, cy) in [(x, y), (20, 38), (24, 44)] {
            self.tile_mut(cx, cy).terrain = Terrain::Grassland;
            self.create_city(1, cx, cy);
        }
        let (ix, iy) = offset_native_by_map(x, y, 0, -7);
        self.tile_mut(ix, iy).terrain = Terrain::Grassland;
        self.tile_mut(ix, iy).river = true;
        self.cities[0].size = 3;
        self.reconcile_city_workers();
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        let wants_settlers =
            self.choose_ai_production(1, self.cities[0].id) == Production::Unit(UnitKind::Settler);
        let id = self.spawn_unit(1, UnitKind::Settler, x, y);
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

    pub(crate) fn ai_settlement_goal_government_test(&mut self) -> bool {
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
        self.players[1].government = Government::ANARCHY;
        self.players[1].known = self.research_path(1, Tech::MONARCHY);
        self.players[1].known.retain(|&tech| tech != Tech::MONARCHY);
        self.players[1].rates = [60, 40, 0];
        let goal = self.ai_goal_government(1);
        let map = self.settlement_citymap(1, None);
        let current = self.city_site_result(1, 15, 40, Government::ANARCHY, &map);
        let projected = self.city_site_result(1, 15, 40, goal, &map);
        goal.requirement()
            .is_some_and(|tech| !self.has_tech(1, tech))
            && current.zip(projected).is_some_and(|(current, projected)| {
                current.value != projected.value
                    && self.city_site_value(1, 15, 40) == Some(projected.value)
            })
            && self.players[1].government == Government::ANARCHY
            && self.players[1].rates == [60, 40, 0]
    }

    pub(crate) fn ai_settlement_value_test(&mut self) -> bool {
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
        self.players[1].government = Government::COMMUNISM;
        self.players[1].researching = Tech::BRONZE_WORKING;
        let home = (10, 24);
        let baseline = self.city_site_value(1, home.0, home.1);
        // Source: center115*2 (2food/2shield/1trade) + best68
        // +19 individually truncated contributions for the surrounding BFC.
        // The selected balance tuning controls how strongly room to grow is
        // valued; keep the known tile outputs and rounding oracle explicit.
        let deemphasis = crate::rules::engine_rules::SettlementAiSettings::campaign()
            .growth_potential_deemphasis;
        let remaining = 19 * (68 / deemphasis);
        let expected = f64::from(298.0_f32 + 100.0_f32 / 299.0 * 200.0 + remaining as f32);
        let rounded = baseline.is_some_and(|value| (value - expected).abs() < 0.0001);
        let mut reservations = vec![0; self.tiles.len()];
        let reserved_tile = self
            .map_city_radius_at(home.0, home.1)
            .find(|&(_, _, x, y)| (x, y) != home)
            .map(|(_, _, x, y)| (x, y))
            .unwrap();
        reservations[self.map_wrap.index(reserved_tile.0, reserved_tile.1)] = 1;
        let crowded = self
            .city_site_result(1, home.0, home.1, Government::COMMUNISM, &reservations)
            .is_some_and(|site| site.value < baseline.unwrap());
        self.tile_mut(home.0, home.1).pollution = true;
        rounded && crowded && self.city_site_value(1, home.0, home.1).is_none()
    }
}

impl Simulation {}

impl Simulation {
    pub(crate) fn settlement_reservation_order_test(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_settlement_targets.clear();
        self.ai_government_cache.clear();
        self.city_rapture.clear();
        self.players[1].government = Government::COMMUNISM;
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
        let id = self.spawn_unit(1, UnitKind::Settler, home.0, home.1);
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
        let initial = self.city_site_result(1, home.0, home.1, Government::COMMUNISM, &map);
        let source_first = initial
            .as_ref()
            .is_some_and(|site| site.worker == Some(first));
        map[self.map_wrap.index(first.0, first.1)] = 1;
        let next = self.city_site_result(1, home.0, home.1, Government::COMMUNISM, &map);
        let source_next = next
            .as_ref()
            .is_some_and(|site| site.worker == Some(second))
            && initial.zip(next).is_some_and(|(a, b)| a.value > b.value);
        map[self.map_wrap.index(first.0, first.1)] = -1;
        let overlap_rejected = self
            .city_site_result(1, home.0, home.1, Government::COMMUNISM, &map)
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
