//! Focused gameplay scenarios for the parent subsystem.

use super::*;

impl super::Simulation {
    pub(crate) fn ai_aircraft_fuel_path_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.owner = None;
            tile.hut = false;
            tile.known_by.fill(true);
        }
        self.spawn_unit(HUMAN, UnitKind::Fighter, 10, 20);
        let point = offset_native_by_map(10, 20, 1, 0);
        let leader = self.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, point.0, point.1);
        self.run_ai_player(BARBARIANS);
        let unreachable = self.units.iter().any(|unit| {
            unit.id == leader && (unit.x, unit.y) == point && unit.moves_left == 2 * MOVE_FRAGMENTS
        });
        // A real refuel point makes the same aircraft's native fuel map
        // reachable; the leader can now follow increasing enemy costs.
        self.create_city(HUMAN, 10, 20);
        self.run_ai_player(BARBARIANS);
        unreachable
            && self.units.iter().any(|unit| {
                unit.id == leader
                    && unit.moves_left == 0
                    && self.map_distance(10, 20, unit.x, unit.y) > 1
            })
    }
}
