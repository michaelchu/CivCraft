use super::*;

impl Simulation {
    pub(super) fn shield_output_bonus(&self, city: &City) -> i32 {
        // civ2civ3/effects.ruleset: each industrial building gives 25%,
        // and the active power source adds another 25% for EACH of them.
        // Power sources replace one another, rather than stacking. Hoover
        // Dam's shield effect has City range (its pollution effects differ).
        let industry = [Building::FACTORY, Building::MFG_PLANT]
            .into_iter()
            .filter(|&building| city.has_building(building))
            .count() as i32;
        let powered = [
            Building::HOOVER_DAM,
            Building::SOLAR_PLANT,
            Building::HYDRO_PLANT,
            Building::NUCLEAR_PLANT,
            Building::POWER_PLANT,
        ]
        .into_iter()
        .any(|building| city.has_building(building));
        // At the supported Normal difficulty, neither Restricted's -25%
        // Output_Bonus nor Cheating's +40% Output_Bonus_2 applies.
        100 + 25 * industry * (1 + i32::from(powered))
    }

    pub(super) fn commercial_output_bonus(&self, city: &City) -> i32 {
        100 + 50
            * [
                Building::MARKETPLACE,
                Building::BANK,
                Building::STOCK_EXCHANGE,
            ]
            .into_iter()
            .filter(|&building| city.has_building(building))
            .count() as i32
            + 50 * i32::from(
                city.has_building(Building::SUPER_HIGHWAYS)
                    && city.has_building(Building::STOCK_EXCHANGE),
            )
    }

    pub(super) fn base_city_celebrating(&self, city: &City) -> bool {
        city.size >= 3 && city.was_happy
    }

    pub(super) fn government_food_output(
        &self,
        city: &City,
        gross: i32,
        government: Government,
    ) -> i32 {
        if gross <= 0 || city.owner == BARBARIANS || government == Government::Anarchy {
            return gross;
        }
        let distance = if city.has_building(Building::PALACE)
            || city.has_building(Building::ECCLESIASTICAL_PALACE)
        {
            Some(0)
        } else {
            self.cities
                .iter()
                .filter(|center| {
                    center.owner == city.owner
                        && (center.has_building(Building::PALACE)
                            || center.has_building(Building::ECCLESIASTICAL_PALACE))
                })
                .map(|center| self.map_distance(city.x, city.y, center.x, center.y))
                .min()
        };
        // city_waste's missing-government-center branch loses EVERYTHING;
        // percentage relief does not apply to that branch.
        let Some(distance) = distance else {
            return 0;
        };
        let relief = 50
            + 25 * i32::from(city.has_building(Building::GRANARY))
            + 25 * i32::from(city.has_building(Building::COURTHOUSE));
        let waste = percent_of(gross, distance);
        gross - (waste - percent_of(waste, relief)).clamp(0, gross)
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
        self.tile_mut(neighbor.0, neighbor.1).terrain = Terrain::Ocean;
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
            Building::SUPER_HIGHWAYS,
        ]);
        let commerce = self.split_city_trade(&city, 4, [0, 50, 50]) == [0, 6, 6];
        self.players[1].government = Government::Federation;
        let federation = self.split_city_trade(&city, 0, [100, 0, 0])[2] == 6;
        let mut remote = city.clone();
        remote
            .buildings
            .retain(|&building| building != Building::PALACE);
        (remote.x, remote.y) = offset_native_by_map(12, 24, 30, 0);
        let waste = self.government_food_output(&remote, 100, Government::Communism) == 85;
        self.players[1].government = Government::Despotism;
        self.players[1].rates = [0, 0, 100];
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
        }
        self.cities[0].size = 3;
        self.cities[0].food = 0;
        self.cities[0].was_happy = false;
        self.cities[0]
            .buildings
            .push(Building::SHAKESPEARES_THEATER);
        self.reconcile_city_workers();
        let initial = self.city_tile_yields(&self.cities[0], 12, 24, true, None).0 == 3;
        self.process_cities();
        let first = self.cities[0].was_happy
            && self.cities[0].food == 6
            && !self.city_rapture.contains_key(&city.id)
            && self.city_tile_yields(&self.cities[0], 12, 24, true, None).0 == 3;
        self.process_cities();
        let second = self.city_rapture.get(&city.id) == Some(&1)
            && self.cities[0].food == 12
            && self.city_tile_yields(&self.cities[0], 12, 24, true, None).0 == 4;
        self.cities[0]
            .buildings
            .retain(|&building| building != Building::SHAKESPEARES_THEATER);
        self.players[1].rates = [100, 0, 0];
        self.process_cities();
        let ended = !self.cities[0].was_happy && !self.city_rapture.contains_key(&city.id);
        center
            && minimum
            && harbor
            && pyramids
            && commerce
            && federation
            && waste
            && initial
            && first
            && second
            && ended
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn industry_and_power_apply_real_shield_bonuses_without_stacking_plants() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(Building::POWER_PLANT.data().name, "Power Plant");
        assert!(game.industrial_output_playtest());
    }

    #[test]
    fn hoover_shield_bonus_requires_the_wonders_own_city() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 24);
        let mut city = game.cities[0].clone();
        city.buildings = vec![Building::FACTORY];
        game.completed_wonders.push(Building::HOOVER_DAM);
        assert_eq!(game.shield_output_bonus(&city), 125);
        city.buildings.push(Building::HOOVER_DAM);
        assert_eq!(game.shield_output_bonus(&city), 150);
    }

    #[test]
    fn shield_waste_includes_factory_output_before_integer_rounding() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.industrial_output_playtest());
        let mut city = game.cities[0].clone();
        city.buildings = vec![Building::FACTORY];
        let tiles: Vec<_> = Simulation::city_radius(&city)
            .map(|(_, _, x, y)| (x, y))
            .collect();
        let mut distinguished_order = false;
        for workers in 1..=tiles.len() {
            city.size = workers as i32;
            city.specialists = [0; 3];
            city.worked_tiles = tiles[..workers].to_vec();
            game.players[HUMAN].government = Government::Communism;
            city.buildings.clear();
            let raw = game.city_yields(&city).1;
            city.buildings.push(Building::FACTORY);
            game.players[HUMAN].government = Government::Democracy;
            // Democracy's base shield waste is 25%; this city is on its
            // capital tile, without a courthouse/palace relief in the query.
            let boosted = raw * 125 / 100;
            let expected = boosted - boosted * 25 / 100;
            assert_eq!(game.city_yields(&city).1, expected);
            distinguished_order |= expected != (raw - raw * 25 / 100) * 125 / 100;
        }
        assert!(distinguished_order);
    }
}

#[cfg(test)]
mod city_output_tests {
    use super::*;

    fn fixture() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.city_rapture.clear();
        game.players[1].government = Government::Communism;
        game.players[1].known.clear();
        game.players[1].gold = 100_000;
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.owner = None;
            tile.road = false;
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
        game.create_city(1, 12, 24);
        game
    }

    #[test]
    fn shared_city_output_desktop_fixture() {
        let mut game = fixture();
        assert!(game.city_output_rules_playtest());
    }

    #[test]
    fn harbor_uses_sea_flag_and_offshore_platform_respects_deep_ocean_oil() {
        let mut game = fixture();
        let mut city = game.cities[0].clone();
        city.buildings
            .extend([Building::HARBOR, Building::OFFSHORE_PLATFORM]);
        let point = offset_native_by_map(city.x, city.y, 1, 0);
        for (terrain, food, shields) in [
            (Terrain::Ocean, 2, 1),
            (Terrain::DeepOcean, 2, 0),
            (Terrain::Lake, 2, 1),
        ] {
            game.tile_mut(point.0, point.1).terrain = terrain;
            let output = game.city_tile_yields(&city, point.0, point.1, false, None);
            assert_eq!((output.0, output.1), (food, shields), "{terrain:?}");
        }
        game.tile_mut(point.0, point.1).terrain = Terrain::DeepOcean;
        game.tile_mut(point.0, point.1).oil_platform = true;
        assert_eq!(
            game.city_tile_yields(&city, point.0, point.1, false, None)
                .1,
            1
        );
    }

    #[test]
    fn pyramids_increments_only_positive_shields_after_center_add_and_railroad() {
        let mut game = fixture();
        let mut city = game.cities[0].clone();
        city.buildings.push(Building::PYRAMIDS);
        let point = offset_native_by_map(city.x, city.y, 1, 0);
        assert_eq!(
            game.city_tile_yields(&city, point.0, point.1, false, None)
                .1,
            0
        );
        assert_eq!(
            game.city_tile_yields(&city, city.x, city.y, true, None).1,
            2
        );
        game.tile_mut(city.x, city.y).terrain = Terrain::Forest;
        game.tile_mut(city.x, city.y).railroad = true;
        //2terrain +1railroad +1center +1Pyramids =5; Railroad doesn't
        // multiply either additive building effect.
        assert_eq!(
            game.city_tile_yields(&city, city.x, city.y, true, None).1,
            5
        );
    }

    #[test]
    fn center_uses_civ2civ3_additive_shield_and_zero_minimum_outputs() {
        let mut game = fixture();
        let city = game.cities[0].clone();
        game.tile_mut(city.x, city.y).terrain = Terrain::Forest;
        assert_eq!(
            game.city_tile_yields(&city, city.x, city.y, true, None),
            (1, 3, 0)
        );
        game.tile_mut(city.x, city.y).terrain = Terrain::Desert;
        game.tile_mut(city.x, city.y).mine = true;
        assert_eq!(
            game.city_tile_yields(&city, city.x, city.y, true, None).0,
            0
        );
    }

    #[test]
    fn supermarket_penalty_and_contamination_follow_native_integer_order() {
        let mut game = fixture();
        let mut city = game.cities[0].clone();
        game.players[1].government = Government::Despotism;
        city.buildings.push(Building::SUPERMARKET);
        let point = offset_native_by_map(city.x, city.y, 1, 0);
        let tile = game.tile_mut(point.0, point.1);
        tile.irrigation = true;
        tile.farmland = true;
        tile.pollution = true;
        //3food *150%=4; penalty2 =>3;50%punishment =>2.
        assert_eq!(
            game.city_tile_yields(&city, point.0, point.1, false, None)
                .0,
            2
        );
        city.was_happy = true;
        city.size = 3;
        //was_happy alone is NOT CityStatus Celebration.
        assert_eq!(
            game.city_tile_yields(&city, point.0, point.1, false, None)
                .0,
            2
        );
        game.city_rapture.insert(city.id, 1);
        game.tile_mut(point.0, point.1).pollution = false;
        assert_eq!(
            game.city_tile_yields(&city, point.0, point.1, false, None)
                .0,
            4
        );
    }

    #[test]
    fn highways_stock_exchange_and_federation_luxury_apply_before_percent_bonus() {
        let mut game = fixture();
        let mut city = game.cities[0].clone();
        city.worked_tiles.clear();
        city.specialists = [0; 3];
        city.buildings.extend([
            Building::MARKETPLACE,
            Building::BANK,
            Building::STOCK_EXCHANGE,
        ]);
        assert_eq!(game.split_city_trade(&city, 4, [0, 50, 50]), [0, 5, 5]);
        city.buildings.push(Building::SUPER_HIGHWAYS);
        assert_eq!(game.split_city_trade(&city, 4, [0, 50, 50]), [0, 6, 6]);
        game.players[1].government = Government::Federation;
        assert_eq!(game.split_city_trade(&city, 0, [100, 0, 0]), [0, 0, 6]);
    }

    #[test]
    fn hypothetical_government_center_uses_own_location_before_scanning_existing_centers() {
        let game = fixture();
        let mut city = game.cities[0].clone();
        city.buildings.clear();
        (city.x, city.y) = offset_native_by_map(city.x, city.y, 30, 0);
        assert_eq!(
            game.government_food_output(&city, 100, Government::Despotism),
            85
        );
        assert_eq!(
            game.government_output_waste_under(&city, 100, 100, None, Government::Despotism),
            (30, 20)
        );
        city.buildings.push(Building::ECCLESIASTICAL_PALACE);
        // city.c:nearest_gov_center's first branch uses evaluated city,
        // before the real city's building list has been changed in the map.
        assert_eq!(
            game.government_food_output(&city, 100, Government::Despotism),
            100
        );
        assert_eq!(
            game.government_output_waste_under(&city, 100, 100, None, Government::Despotism),
            (95, 90)
        );
    }

    #[test]
    fn food_waste_uses_distance_then_relief_with_native_rounding_and_no_center_rule() {
        let mut game = fixture();
        let mut city = game.cities[0].clone();
        city.buildings
            .retain(|&building| building != Building::PALACE);
        (city.x, city.y) = offset_native_by_map(city.x, city.y, 30, 0);
        assert_eq!(
            game.government_food_output(&city, 100, Government::Communism),
            85
        );
        city.buildings.push(Building::GRANARY);
        assert_eq!(
            game.government_food_output(&city, 100, Government::Communism),
            92
        );
        city.buildings.push(Building::COURTHOUSE);
        assert_eq!(
            game.government_food_output(&city, 100, Government::Communism),
            100
        );
        game.cities[0].buildings.clear();
        assert_eq!(
            game.government_food_output(&city, 100, Government::Communism),
            0
        );
        assert_eq!(
            game.government_food_output(&city, 100, Government::Anarchy),
            100
        );
        city.owner = BARBARIANS;
        assert_eq!(
            game.government_food_output(&city, 100, Government::Despotism),
            100
        );
    }

    #[test]
    fn celebration_trade_uses_prior_happiness_while_tile_penalty_uses_rapture_history() {
        let mut game = fixture();
        let mut city = game.cities[0].clone();
        game.players[1].government = Government::Monarchy;
        assert_eq!(
            game.city_tile_yields(&city, city.x, city.y, true, None).2,
            1
        );
        city.was_happy = true;
        city.size = 3;
        assert_eq!(
            game.city_tile_yields(&city, city.x, city.y, true, None).2,
            2
        );
        game.players[1].government = Government::Republic;
        assert_eq!(
            game.city_tile_yields(&city, city.x, city.y, true, None).2,
            3
        );
        city.size = 2;
        assert_eq!(
            game.city_tile_yields(&city, city.x, city.y, true, None).2,
            2
        );
    }
}
