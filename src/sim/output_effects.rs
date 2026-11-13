//! Ruleset effects that modify city and player output.

use super::*;

impl Simulation {
    #[cfg(test)]
    pub(super) fn shield_output_bonus(&self, city: &City) -> i32 {
        self.city_output_bonus_percent(city, "Shield", self.players[city.owner].government, None)
    }

    pub(super) fn base_city_celebrating(&self, city: &City) -> bool {
        city.size
            >= crate::sim::economy::ruleset::CityEconomySettings::campaign().celebrate_size_limit
            && city.was_happy
    }

    pub(super) fn government_food_output(
        &self,
        city: &City,
        gross: i32,
        government: Government,
    ) -> i32 {
        self.c2c3_city_output_after_bonus(city, gross, "Food", government, None)
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
    fn science_wonders_add_output_to_center_and_worked_tiles_in_freeciv_order() {
        let mut game = fixture();
        // The Great Library bonus has a World + survives requirement. Keep
        // every civilization below its obsolescence advance in this fixture.
        for player in &mut game.players {
            player.known.clear();
        }
        game.players[1].rates = [100, 0, 0];
        let city_id = game.cities[0].id;
        let center = (game.cities[0].x, game.cities[0].y);
        let worked = offset_native_by_map(center.0, center.1, 1, 0);
        game.cities[0].size = 2;
        game.cities[0].worked_tiles.push(worked);
        game.cities[0].buildings.push(Building::LIBRARY);
        game.finish_building(city_id, 1, Building::GREAT_LIBRARY, 300);
        assert_eq!(
            game.c2c3_city_output_additions(
                &game.cities[0],
                "Science",
                game.players[1].government,
            ),
            4
        );
        game.create_city(
            1,
            offset_native_by_map(center.0, center.1, 8, 0).0,
            center.1,
        );
        let rival_id = game.cities[1].id;
        game.cities[1].buildings.push(Building::LIBRARY);
        assert_eq!(game.city_trade(&game.cities[0], 10)[0], 28);
        assert_eq!(game.city_trade(&game.cities[1], 10)[0], 20);

        game.cities[0]
            .buildings
            .push(Building::COPERNICUS_OBSERVATORY);
        game.completed_wonders
            .push(Building::COPERNICUS_OBSERVATORY);
        let worked_output_tiles = 1 + game.cities[0]
            .worked_tiles
            .iter()
            .filter(|&&tile| tile != center)
            .count() as i32;
        assert_eq!(
            game.c2c3_city_output_additions(
                &game.cities[0],
                "Science",
                game.players[1].government,
            ),
            4 + worked_output_tiles
        );
        assert_eq!(
            game.city_trade(&game.cities[0], 0)[0],
            (4 + worked_output_tiles) * 2
        );
        assert_eq!(game.cities[1].id, rival_id);
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
            (Terrain::Sea, 2, 1),
            (Terrain::Ocean, 2, 0),
            (Terrain::Lake, 2, 1),
        ] {
            game.tile_mut(point.0, point.1).terrain = terrain;
            let output = game.city_tile_yields(&city, point.0, point.1, false, None);
            assert_eq!((output.0, output.1), (food, shields), "{terrain:?}");
        }
        game.tile_mut(point.0, point.1).terrain = Terrain::Ocean;
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
    fn highways_and_stock_exchange_apply_before_percent_bonus() {
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
        game.players[1].government = Government::Feudalism;
        assert_eq!(game.split_city_trade(&city, 0, [100, 0, 0]), [0, 0, 0]);
    }

    #[test]
    fn hypothetical_government_center_uses_own_location_before_scanning_existing_centers() {
        let game = fixture();
        let mut city = game.cities[0].clone();
        city.buildings.clear();
        (city.x, city.y) = offset_native_by_map(city.x, city.y, 18, 0);
        assert_eq!(
            game.government_food_output(&city, 100, Government::Despotism),
            91
        );
        assert_eq!(
            game.government_output_waste_under(&city, 100, 100, None, Government::Despotism),
            (54, 44)
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
        (city.x, city.y) = offset_native_by_map(city.x, city.y, 18, 0);
        assert_eq!(
            game.government_food_output(&city, 100, Government::Communism),
            91
        );
        city.buildings.push(Building::GRANARY);
        assert_eq!(
            game.government_food_output(&city, 100, Government::Communism),
            95
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
        // C2C3 Grassland has no base trade. Road trade makes the center's
        // positive output eligible for its celebration increment.
        game.tile_mut(city.x, city.y).road = true;
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

    #[test]
    fn shakespeare_luxury_tile_additions_follow_player_city_and_world_scopes() {
        let mut game = fixture();
        game.create_city(1, 20, 24);
        game.create_city(2, 30, 24);
        game.cities[0]
            .buildings
            .push(Building::SHAKESPEARES_THEATER);

        let luxury =
            |game: &Simulation, city: &City| game.split_city_trade(city, 0, [100, 0, 0])[2];
        assert_eq!(luxury(&game, &game.cities[0]), 6); // player + city
        assert_eq!(luxury(&game, &game.cities[1]), 1); // player only
        assert_eq!(luxury(&game, &game.cities[2]), 0); // neither scope

        // The selected ruleset's specialist effect is World+survives, unlike
        // the center tile's Player and City effects above.
        game.cities[2].specialists[2] = 1;
        assert_eq!(luxury(&game, &game.cities[2]), 2);
        game.completed_wonders.push(Building::SHAKESPEARES_THEATER);
        assert_eq!(luxury(&game, &game.cities[2]), 3);

        game.cities[0]
            .buildings
            .retain(|&building| building != Building::SHAKESPEARES_THEATER);
        assert_eq!(luxury(&game, &game.cities[0]), 0);
        assert_eq!(luxury(&game, &game.cities[1]), 0);
        assert_eq!(luxury(&game, &game.cities[2]), 3);
    }
}
