//! Automated gameplay scenarios covering city economy behavior.

use super::*;

impl Simulation {
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
        self.players[1].government = Government::Despotism;
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

    pub(crate) fn barbarian_output_waste_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
        }
        self.players[BARBARIANS].government = Government::Despotism;
        self.players[BARBARIANS].known.clear();
        self.create_city(BARBARIANS, 12, 24);
        let mut city = self.cities[0].clone();
        let no_center = !city.has_building(Building::PALACE);
        let base = self.government_output_waste(&city, 100, 100, None) == (90, 80);
        city.buildings.push(Building::COURTHOUSE);
        let courthouse = self.government_output_waste(&city, 100, 100, None) == (95, 90);
        city.buildings.clear();
        city.owner = 1;
        let ordinary =
            self.government_output_waste_under(&city, 100, 100, None, Government::Despotism)
                == (0, 0);
        no_center && base && courthouse && ordinary
    }

    pub(crate) fn city_output_rules_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.city_rapture.clear();
        self.ai_government_cache.clear();
        self.players[1].government = Government::Communism;
        self.players[1].known.clear();
        self.players[1].gold = 100_000;
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
            tile.hut = false;
        }
        self.create_city(1, 12, 24);
        let mut city = self.cities[0].clone();
        let center = self.city_tile_yields(&city, 12, 24, true, None) == (2, 2, 1);
        self.tile_mut(12, 24).terrain = Terrain::Forest;
        let minimum = self.city_tile_yields(&city, 12, 24, true, None) == (1, 3, 0);
        self.tile_mut(12, 24).terrain = Terrain::Plains;
        let neighbor = offset_native_by_map(12, 24, 1, 0);
        self.tile_mut(neighbor.0, neighbor.1).terrain = Terrain::Sea;
        city.buildings.push(Building::HARBOR);
        let harbor = self
            .city_tile_yields(&city, neighbor.0, neighbor.1, false, None)
            .0
            == 2;
        city.buildings.push(Building::PYRAMIDS);
        let pyramids = self.city_tile_yields(&city, 12, 24, true, None).1 == 3;
        city.buildings.extend([
            Building::MARKETPLACE,
            Building::BANK,
            Building::STOCK_EXCHANGE,
        ]);
        // Keep this fixture on the three city commerce bonuses. The separate
        // output-effects regression covers Super Highways' additional bonus.
        let commerce = self.split_city_trade(&city, 4, [0, 50, 50]) == [0, 5, 5];
        self.players[1].government = Government::Feudalism;
        // Feudalism is a Conquests government; it adds no trade by itself.
        let feudalism_has_no_extra_trade =
            self.split_city_trade(&city, 0, [100, 0, 0]) == [0, 0, 0];
        let mut remote = city.clone();
        remote
            .buildings
            .retain(|&building| building != Building::PALACE);
        (remote.x, remote.y) = offset_native_by_map(12, 24, 18, 0);
        // C2C3 applies 50% food-waste relief; Communism does not cancel the
        // distance component. At distance 18, 9% of gross food is wasted.
        let waste = self.government_food_output(&remote, 100, Government::Communism) == 91;
        self.players[1].government = Government::Republic;
        city.size = 3;
        city.was_happy = false;
        let ordinary_trade = self.city_tile_yields(&city, city.x, city.y, true, None).2;
        city.was_happy = true;
        let celebrating_trade = self.city_tile_yields(&city, city.x, city.y, true, None).2;
        let celebration = celebrating_trade == ordinary_trade + 1;
        center
            && minimum
            && harbor
            && pyramids
            && commerce
            && feudalism_has_no_extra_trade
            && waste
            && celebration
    }

    pub(crate) fn industrial_output_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        let home = (10, 24);
        self.players[HUMAN].government = Government::Communism;
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Hills;
            tile.resource = None;
            tile.hut = false;
            tile.mine = true;
            tile.pollution = false;
            tile.fallout = false;
        }
        self.create_city(HUMAN, home.0, home.1);
        let mut city = self.cities[0].clone();
        city.size = 6;
        city.specialists = [0; 3];
        city.worked_tiles = Self::city_radius(&city)
            .take(6)
            .map(|(_, _, x, y)| (x, y))
            .collect();
        city.buildings.clear();
        let baseline = self.city_yields(&city).1;
        city.buildings.push(Building::FACTORY);
        let factory = self.city_yields(&city).1 == baseline * 125 / 100;
        city.buildings.push(Building::MFG_PLANT);
        let manufacturing = self.city_yields(&city).1 == baseline * 150 / 100;
        city.buildings.push(Building::POWER_PLANT);
        let powered = self.city_yields(&city).1 == baseline * 200 / 100;
        city.buildings.extend([
            Building::NUCLEAR_PLANT,
            Building::HYDRO_PLANT,
            Building::SOLAR_PLANT,
            Building::HOOVER_DAM,
        ]);
        let no_stacking = self.city_yields(&city).1 == baseline * 200 / 100;
        city.buildings
            .retain(|&building| !matches!(building, Building::FACTORY | Building::MFG_PLANT));
        let no_industry = self.city_yields(&city).1 == baseline;
        baseline > 3 && factory && manufacturing && powered && no_stacking && no_industry
    }

    pub(crate) fn city_worker_lifecycle_playtest(&self) -> (bool, bool, bool) {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Mountains;
            tile.resource = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.irrigation = false;
            tile.mine = false;
            tile.owner = None;
        }
        game.players[HUMAN].government = Government::Communism;
        game.players[HUMAN].known = vec![Tech::COMMUNISM];
        let center = (12, 24);
        let grass = offset_native_by_map(center.0, center.1, -1, 0);
        let coal = [
            offset_native_by_map(center.0, center.1, 1, 0),
            offset_native_by_map(center.0, center.1, 0, 1),
        ];
        game.tile_mut(center.0, center.1).terrain = Terrain::Grassland;
        game.tile_mut(grass.0, grass.1).terrain = Terrain::Grassland;
        for (x, y) in coal {
            game.tile_mut(x, y).terrain = Terrain::Hills;
            game.tile_mut(x, y).resource = Some(Resource::Coal);
        }
        game.create_city(HUMAN, center.0, center.1);
        let founded =
            game.cities[0].worked_tiles == vec![grass] && game.cities[0].specialists == [0; 3];
        game.cities[0].food = Self::growth_box(1) - 1;
        game.process_cities();
        let grew = game.cities[0].size == 2
            && coal
                .into_iter()
                .all(|tile| game.cities[0].worked_tiles.contains(&tile))
            && !game.cities[0].worked_tiles.contains(&grass)
            && game.cities[0].specialists == [0; 3];
        let id = game.cities[0].id;
        game.reduce_city_population(0);
        let reduced = game.cities[0].id == id
            && game.cities[0].size == 1
            && game.cities[0].worked_tiles == vec![grass]
            && game.cities[0].specialists == [0; 3];
        (founded, grew, reduced)
    }
}

impl Simulation {
    pub(crate) fn ai_city_manager_playtest(&self) -> bool {
        let mut scratch = self.clone();
        scratch.units.clear();
        scratch.cities.clear();
        for tile in &mut scratch.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.road = false;
            tile.river = false;
        }
        scratch.create_city(1, 12, 24);
        let mut city = scratch.cities[0].clone();
        city.size = 6;
        city.buildings.push(Building::TEMPLE);
        scratch.players[1].known.push(Tech::MYSTICISM);
        city.worked_tiles.clear();
        city.specialists = [6, 0, 0];
        let arranged = scratch.city_manager_result(&city, &Default::default());
        let output = scratch.city_forecast(&arranged);
        let mut native_order: Vec<_> = Self::city_radius(&city)
            .filter(|&(_, _, x, y)| scratch.tile_can_support_worker(&city, x, y))
            .collect();
        native_order.sort_by_key(|&(dx, dy, _, _)| city_radius_order_key(dx, dy));
        let expected: Vec<_> = native_order
            .into_iter()
            .take(arranged.worked_tiles.len())
            .map(|(_, _, x, y)| (x, y))
            .collect();
        arranged.worked_tiles.len() + arranged.specialists.iter().sum::<i32>() as usize == 6
            && !output.disorder
            && output.food_surplus(6, output.support.food) >= 0
            && arranged.worked_tiles != city.worked_tiles
            && arranged.worked_tiles == expected
    }
}
