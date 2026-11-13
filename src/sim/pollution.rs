//! Pollution, contamination, and cleanup rules.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum Contamination {
    Pollution,
    Fallout,
    Craters,
}

impl Contamination {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Pollution => "pollution",
            Self::Fallout => "fallout",
            Self::Craters => "craters",
        }
    }
}

// Pinned Freeciv `common/city.c::generate_city_map_indices` builds this
// radius-5 city map under C2C3's ISO|HEX topology, then sorts by (distance,
// dx, dy). The result is the full draw domain used by `cityturn.c::place_pollution`.
// C2C3's game.ruleset sets `init_city_radius_sq = 5`.
const C2C3_CITY_MAP_RADIUS_5: [(i32, i32); 19] = [
    (0, 0),
    (-1, -1),
    (-1, 0),
    (0, -1),
    (0, 1),
    (1, 0),
    (1, 1),
    (-2, -2),
    (-2, -1),
    (-2, 0),
    (-1, -2),
    (-1, 1),
    (0, -2),
    (0, 2),
    (1, -1),
    (1, 2),
    (2, 0),
    (2, 1),
    (2, 2),
];

impl Tile {
    pub(crate) fn contamination_output(&self, value: i32) -> i32 {
        let value = (value - i32::from(self.craters)).max(0);
        let penalty = match (self.pollution, self.fallout) {
            (true, true) => 75,
            (true, false) | (false, true) => 50,
            _ => 0,
        };
        value - percent_of(value, penalty)
    }

    pub(crate) fn has_contamination(&self, target: Contamination) -> bool {
        match target {
            Contamination::Pollution => self.pollution,
            Contamination::Fallout => self.fallout,
            Contamination::Craters => self.craters,
        }
    }
}

impl Simulation {
    pub(super) fn pollution_risk(&self, city: &City, shields: i32) -> i32 {
        let has = |building| city.has_building(building);
        let recycling = has(Building::RECYCLING_CENTER);
        let hydro = has(Building::HYDRO_PLANT);
        let hoover = has(Building::HOOVER_DAM);
        let solar = has(Building::SOLAR_PLANT);
        let nuclear = has(Building::NUCLEAR_PLANT);
        let reduction = if hoover || solar {
            75 + 25 * i32::from(recycling)
        } else if hydro || nuclear {
            50 + 25 * i32::from(recycling)
                + 25 * i32::from(hydro && self.has_active_wonder(city.owner, Building::HOOVER_DAM))
        } else {
            50 * i32::from(recycling)
        };
        let population = [
            Building::FACTORY,
            Building::MFG_PLANT,
            Building::OFFSHORE_PLATFORM,
            Building::SUPER_HIGHWAYS,
        ]
        .into_iter()
        .filter(|&building| has(building))
        .count() as i32
            * 25
            - 50 * i32::from(has(Building::MASS_TRANSIT))
            - 25 * i32::from(self.has_active_wonder(city.owner, Building::EIFFEL_TOWER));
        (percent_of(shields, (100 - reduction).max(0)) + percent_of(city.size, population.max(0))
            - 20)
            .max(0)
    }

    pub(super) fn check_city_pollution(&mut self, city: &City, shields: i32) {
        let risk = self.pollution_risk(city, shields);
        if risk == 0 || self.rand_below(100) >= risk {
            return;
        }
        if self.place_contamination(city, Contamination::Pollution) && city.owner == HUMAN {
            self.add_event(format!("Pollution appeared near {}.", city.name));
        }
    }

    pub(super) fn place_contamination(&mut self, city: &City, target: Contamination) -> bool {
        if target == Contamination::Craters {
            // Craters are a Civ III-only state, with no corresponding C2C3
            // extra or `place_pollution` cause. Preserve the existing local
            // adapter if a Civ III caller uses it.
            return self.place_civ3_craters(city);
        }

        // Freeciv samples the full city-map index space on every attempt. A
        // slot that maps beyond CivCraft's bounded polar edge consumes its
        // random draw but does not consume one of the 100 real-tile attempts.
        let mut checked_tiles = 0;
        while checked_tiles < 100 {
            let slot = self.rand_below(C2C3_CITY_MAP_RADIUS_5.len() as i32) as usize;
            let (dx, dy) = C2C3_CITY_MAP_RADIUS_5[slot];
            let Some((x, y)) = self.map_wrap.offset(city.x, city.y, dx, dy) else {
                continue;
            };

            let mut requirement_tile = self.tile(x, y).clone();
            if requirement_tile.terrain == Terrain::Volcano {
                // Volcano is Civ III-only; evaluate the C2C3 terrain flag
                // requirement with the same Mountain fallback used by the
                // existing terrain adapter, while writing to the real tile.
                requirement_tile.terrain = Terrain::Mountains;
            }
            let eligible = self
                .extra_requirements_met_at(
                    city.owner,
                    target.name(),
                    (x, y),
                    Some(&requirement_tile),
                )
                .expect("selected C2C3 contamination extra requirements are supported");
            if eligible {
                let tile = self.tile_mut(x, y);
                match target {
                    Contamination::Pollution => tile.pollution = true,
                    Contamination::Fallout => tile.fallout = true,
                    Contamination::Craters => unreachable!("handled above"),
                }
                return true;
            }
            checked_tiles += 1;
        }
        false
    }

    fn place_civ3_craters(&mut self, city: &City) -> bool {
        let candidates: Vec<_> = self
            .map_city_radius(city)
            .map(|(_, _, x, y)| (x, y))
            .filter(|&(x, y)| {
                let tile = self.tile(x, y);
                !tile.craters && !tile.terrain.is_water()
            })
            .collect();
        if candidates.is_empty() {
            return false;
        }
        let (x, y) = candidates[self.rand_below(candidates.len() as i32) as usize];
        self.tile_mut(x, y).craters = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c2c3_city_map_draw_slots_match_isohex_radius_five_order() {
        fn isohex_distance(dx: i32, dy: i32) -> i32 {
            if (dx < 0 && dy > 0) || (dx > 0 && dy < 0) {
                dx.abs() + dy.abs()
            } else {
                dx.abs().max(dy.abs())
            }
        }

        assert_eq!(C2C3_CITY_MAP_RADIUS_5.len(), 19);
        let mut previous = None;
        for &(dx, dy) in &C2C3_CITY_MAP_RADIUS_5 {
            let distance_squared = isohex_distance(dx, dy).pow(2);
            assert!(
                distance_squared <= 5,
                "offset ({dx},{dy}) is outside radius 5"
            );
            let key = (distance_squared, dx, dy);
            assert!(previous.is_none_or(|before| before < key));
            previous = Some(key);
        }
    }

    #[test]
    fn pollution_placement_counts_only_real_tiles_within_100_attempts() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.tile_mut(12, 0).terrain = Terrain::Grassland;
        game.create_city(HUMAN, 12, 0);
        let city = game.cities[0].clone();

        // Every real source slot is ineligible or already polluted, so the
        // placement must exhaust 100 valid tile checks. Polar slots remain in
        // the draw domain and are skipped without reducing that budget.
        for &(dx, dy) in &C2C3_CITY_MAP_RADIUS_5 {
            let Some((x, y)) = game.map_wrap.offset(city.x, city.y, dx, dy) else {
                continue;
            };
            let tile = game.tile_mut(x, y);
            if (dx, dy) == (0, 0) {
                tile.terrain = Terrain::Grassland;
                tile.pollution = true;
            } else {
                tile.terrain = Terrain::Lake;
                tile.pollution = false;
            }
        }

        let mut expected = game.clone();
        let mut valid_attempts = 0;
        let mut draws = 0;
        while valid_attempts < 100 {
            let slot = expected.rand_below(C2C3_CITY_MAP_RADIUS_5.len() as i32) as usize;
            let (dx, dy) = C2C3_CITY_MAP_RADIUS_5[slot];
            draws += 1;
            if expected.map_wrap.offset(city.x, city.y, dx, dy).is_some() {
                valid_attempts += 1;
            }
        }
        let expected_next = expected.rand_below(1_000_000);

        assert!(!game.place_contamination(&city, Contamination::Pollution));
        assert!(
            draws > valid_attempts,
            "the polar edge must skip source slots"
        );
        assert_eq!(game.rand_below(1_000_000), expected_next);
    }

    #[test]
    fn pollution_placement_uses_c2c3_terrain_flags_and_allows_ocean() {
        let mut game = Simulation::new(crate::mapgen::generate(43));
        game.tile_mut(12, 20).terrain = Terrain::Grassland;
        game.create_city(HUMAN, 12, 20);
        let city = game.cities[0].clone();
        let ocean_pos = game
            .map_wrap
            .offset(city.x, city.y, 0, 1)
            .expect("interior city-map slot is on-map");

        for &(dx, dy) in &C2C3_CITY_MAP_RADIUS_5 {
            if let Some((x, y)) = game.map_wrap.offset(city.x, city.y, dx, dy) {
                game.tile_mut(x, y).terrain = Terrain::Lake;
            }
        }
        game.tile_mut(ocean_pos.0, ocean_pos.1).terrain = Terrain::Sea;

        let mut expected = game.clone();
        loop {
            let slot = expected.rand_below(C2C3_CITY_MAP_RADIUS_5.len() as i32) as usize;
            if C2C3_CITY_MAP_RADIUS_5[slot] == (0, 1) {
                break;
            }
        }
        let expected_next = expected.rand_below(1_000_000);

        assert!(game.place_contamination(&city, Contamination::Pollution));
        assert!(game.tile(ocean_pos.0, ocean_pos.1).pollution);
        assert_eq!(game.rand_below(1_000_000), expected_next);
    }

    #[test]
    fn pollution_can_spawn_and_cleanup_restores_output_one_extra_at_a_time() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for (_, _, x, y) in Simulation::city_radius_at(10, 20) {
            game.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        game.create_city(HUMAN, 10, 20);
        let city = game.cities[0].clone();
        assert_eq!(game.pollution_risk(&city, 20), 0);
        game.check_city_pollution(&city, 120);
        let (_, _, x, y) = Simulation::city_radius(&city)
            .find(|&(_, _, x, y)| game.tile(x, y).pollution)
            .unwrap();
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Plains;
        tile.fallout = true;
        assert_eq!(tile.contamination_output(8), 2);
        let worker = game.spawn_unit(HUMAN, UnitKind::Worker, x, y);
        assert!(game.apply(Command::Work(
            worker,
            WorkKind::Clean(Contamination::Pollution)
        )));
        for _ in 0..2 {
            game.update_activities_and_restore();
        }
        assert!(game.tile(x, y).pollution);
        game.update_activities_and_restore();
        assert!(!game.tile(x, y).pollution);
        assert_eq!(game.tile(x, y).contamination_output(8), 4);
    }

    #[test]
    fn craters_reduce_each_tile_yield_but_are_not_a_c2c3_clean_target() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
        tile.river = false;
        tile.road = false;
        tile.railroad = false;
        tile.irrigation = false;
        tile.mine = false;
        tile.craters = true;
        let yields = tile.yields();
        assert_eq!(yields, (2, 0, 0));
        assert_eq!(
            (
                tile.contamination_output(yields.0),
                tile.contamination_output(yields.1),
                tile.contamination_output(yields.2),
            ),
            (1, 0, 0)
        );

        let worker = game.spawn_unit(HUMAN, UnitKind::Worker, x, y);
        assert!(!game.begin_work(worker, WorkKind::Clean(Contamination::Craters)));
        for _ in 0..3 {
            game.update_activities_and_restore();
        }
        assert!(game.tile(x, y).craters);
        assert_eq!(game.tile(x, y).yields(), (2, 0, 0));
    }
}
