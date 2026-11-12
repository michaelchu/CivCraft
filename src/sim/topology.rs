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
            assert_eq!(
                sim.map_wrap.civ3_neighbors(10, 0).any(|p| p == other),
                false
            );
            let actor = sim.units[0].clone();
            let path = sim.plan_route(&actor, other.0, other.1, true);
            assert!(path.len() > 20);
            assert!(!sim.apply(Command::Move(id, 0, -1)));
            let city = sim.create_city(HUMAN, 10, 0);
            let _ = city;
            let radius: Vec<_> = sim.map_city_radius(sim.cities.last().unwrap()).collect();
            assert_eq!(radius.iter().any(|p| p.3 >= wrap.height() - 2), false);
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

#[cfg(test)]
mod campaign_geometry_tests {
    use super::*;
    use crate::sim::{Command, HUMAN, Pact, Simulation, UnitKind};
    #[test]
    fn both_geometries_coexist_while_campaigns_found_and_advance() {
        for seed in [42, 1337] {
            let mut games = [MapWrap::X]
                .map(|wrap| Simulation::new(crate::mapgen::generate_with_wrap(seed, wrap)));
            for sim in &mut games {
                assert_eq!(sim.tiles.len(), sim.map_wrap.tile_count());
                assert_eq!(sim.route_memory.len(), sim.tiles.len());
                let founder = sim
                    .units
                    .iter()
                    .find(|u| u.owner == HUMAN && u.kind == UnitKind::Settler)
                    .unwrap()
                    .id;
                assert!(sim.apply(Command::FoundCity(founder)));
                for owner in 1..sim.civilization_count {
                    // This geometry test exercises turn advancement, not the
                    // diplomatic outcome of nearby start locations.
                    sim.sign_pact(HUMAN, owner, Pact::Peace);
                }
            }
            for turn in 0..12 {
                for sim in &mut games {
                    assert!(
                        sim.apply(Command::EndTurn),
                        "seed {seed}, turn {turn}, game over: {:?}, human units: {:?}, cities: {:?}, events: {:?}",
                        sim.game_over,
                        sim.units
                            .iter()
                            .filter(|unit| unit.owner == HUMAN)
                            .map(|unit| (unit.id, unit.kind, unit.x, unit.y, unit.hp))
                            .collect::<Vec<_>>(),
                        sim.cities
                            .iter()
                            .filter(|city| city.owner == HUMAN)
                            .map(|city| (city.id, city.x, city.y, city.size))
                            .collect::<Vec<_>>(),
                        sim.events.iter().rev().take(12).collect::<Vec<_>>()
                    );
                }
            }
            for sim in &games {
                assert!(
                    sim.units
                        .iter()
                        .all(|u| (0..sim.map_wrap.width()).contains(&u.x)
                            && (0..sim.map_wrap.height()).contains(&u.y))
                );
                for city in &sim.cities {
                    let radius: Vec<_> = sim.map_city_radius(city).collect();
                    assert!(
                        radius
                            .iter()
                            .all(|&(dx, dy, _, _)| civ3_rank_distance(dx, dy) <= 2)
                    );
                    assert_eq!(radius.len(), 21);
                }
            }
        }
    }
}
