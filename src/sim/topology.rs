//! Simulation compatibility exports for shared map topology.

use super::Tile;

pub(crate) use crate::world::topology::*;

impl RiverEdgeTile for Tile {
    fn has_river(&self) -> bool {
        self.river
    }
    fn is_water(&self) -> bool {
        self.terrain.is_water()
    }
    fn river_edges(&self) -> u8 {
        self.river_edges
    }
    fn set_river_edges(&mut self, edges: u8) {
        self.river_edges = edges;
    }
}

#[cfg(test)]
mod campaign_boundary_tests {
    use super::*;
    use crate::sim::{Command, HUMAN, Simulation, Terrain, UnitKind};

    #[test]
    fn each_campaign_obeys_its_seams_for_movement_sight_routes_and_city_radius() {
        for wrap in [MapWrap::X] {
            let mut sim = Simulation::new(crate::mapgen::generate_with_wrap(42, wrap));
            sim.units.clear();
            sim.cities.clear();
            for tile in &mut sim.tiles {
                tile.terrain = Terrain::Grassland;
                tile.road = false;
                tile.railroad = false;
                tile.river = false;
                tile.base = None;
                tile.hut = false;
                tile.owner = None;
                tile.seen = false;
                tile.visible = false;
                tile.known_by.fill(false);
            }
            let id = sim.spawn_unit(HUMAN, UnitKind::Warrior, 10, 0);
            sim.refresh_visibility();
            let other = (10, wrap.height() - 1);
            assert!(!sim.tile(other.0, other.1).visible);
            assert!(!sim.map_wrap.civ3_neighbors(10, 0).any(|p| p == other));
            let actor = sim.units[0].clone();
            let path = sim.plan_route(&actor, other.0, other.1, true);
            assert!(path.len() > 20);
            assert!(!sim.apply(Command::Move(id, 0, -1)));
            let city = sim.create_city(HUMAN, 10, 0);
            let _ = city;
            let radius: Vec<_> = sim.map_city_radius(sim.cities.last().unwrap()).collect();
            assert!(!radius.iter().any(|p| p.3 >= wrap.height() - 2));
            assert!(radius.iter().all(|p| (0..wrap.height()).contains(&p.3)));
            // Population loss must remove fields across this mode's actual seam.
            let mut edge_city = sim.cities.last().unwrap().clone();
            edge_city.x = 0;
            edge_city.y = 20;
            edge_city.specialists = [0; 3];
            edge_city.worked_tiles = vec![(wrap.width() - 1, 20)];
            edge_city.remove_citizen_assignments(1, wrap);
            assert!(edge_city.worked_tiles.is_empty());
            // Horizontal wrapping is enabled in both modes.
            assert_eq!(wrap.offset(0, 20, -1, 1), Some((wrap.width() - 1, 20)));
        }
    }
}
