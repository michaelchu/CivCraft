//! Automated culture accumulation scenarios.

use super::*;

impl Simulation {
    /// Exercise the city-turn culture path at and before the documented age
    /// doubling point. The exact first qualifying turn still needs Conquests
    /// observation; this is a regression check for CivCraft's current boundary.
    pub(crate) fn culture_accumulation_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.year = 999;
        self.create_city(HUMAN, 10, 24);
        self.cities[0].buildings.clear();
        self.cities[0].food = 20;
        self.cities[0].buildings.push(Building::TEMPLE);
        self.cities[0].record_culture_building_completion(Building::TEMPLE, HUMAN, 0);

        self.process_player_cities(HUMAN);
        let before_boundary = self.cities[0].culture_for_civilization(HUMAN) == 2;
        self.year = 1_000;
        self.process_player_cities(HUMAN);
        let at_boundary = self.cities[0].culture_for_civilization(HUMAN) == 6
            && self.civilization_culture(HUMAN) == 6;

        let mut territory = Simulation::new(crate::mapgen::generate(42));
        territory.units.clear();
        for tile in &mut territory.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by = [false; PLAYER_SLOTS];
            tile.known_by[HUMAN] = true;
            tile.owner = None;
            tile.border_city = None;
            tile.last_seen_owner = None;
        }
        territory.create_city(HUMAN, 10, 24);
        territory.cities[0].food = 20;
        territory.cities[0].add_culture(HUMAN, 9);
        let origin = (territory.cities[0].x, territory.cities[0].y);
        let mut next_ring = origin;
        for _ in 0..2 {
            next_ring = territory
                .map_wrap
                .civ3_neighbor(next_ring.0, next_ring.1, 0)
                .expect("culture test ring stays on the map");
        }
        territory.rebuild_city_ownership();
        let unclaimed_before_turn = territory.tile(next_ring.0, next_ring.1).owner.is_none();
        territory.end_turn();
        let threshold_reached = territory.cities[0].culture_for_civilization(HUMAN) == 10;
        let border_refreshed = territory.tile(next_ring.0, next_ring.1).owner == Some(HUMAN);
        let turn_refresh = unclaimed_before_turn && threshold_reached && border_refreshed;

        (before_boundary, at_boundary, turn_refresh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_culture_scenario_checks_city_turn_accrual_and_doubling() {
        let mut simulation = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            simulation.culture_accumulation_playtest(),
            (true, true, true)
        );
    }
}
