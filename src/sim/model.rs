//! Persistent game entities and compact rules data shared by simulation systems.
//!
//! The child modules separate player/city/unit records from map terrain and
//! unit statistics. Keep these types focused on campaign state and values;
//! turn sequencing and state transitions belong to the sibling systems under
//! `sim`. Serialization derives here define part of the campaign save format,
//! so changing a field may also require a migration in `save::migrations`.

mod entities;
mod terrain;
mod unit_stats;
mod units;

pub(crate) use crate::world::{Resource, Terrain, TerrainBase};
pub(crate) use entities::{City, Player, Production, Unit, UnitOrder, UnitState, WorkKind};
pub(crate) use terrain::Tile;
pub(crate) use units::UnitKind;

#[cfg(test)]
mod resource_terrain_tests {
    use super::*;
    use crate::sim::Simulation;

    #[test]
    fn ivory_is_active_on_plains_but_not_unrelated_or_legacy_glacier_terrain() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let tile = sim.tile_mut(10, 10);
        tile.resource = Some(Resource::Ivory);

        tile.terrain = Terrain::Plains;
        assert_eq!(tile.active_resource(), Some(Resource::Ivory));

        tile.terrain = Terrain::Glacier;
        assert_eq!(tile.active_resource(), None);

        tile.terrain = Terrain::Hills;
        assert_eq!(tile.active_resource(), None);
    }
}

#[cfg(test)]
mod native_type_order_tests {
    use super::*;
    use crate::rules::{Building, Tech};
    use crate::sim::{HUMAN, Simulation, offset_native_by_map};

    #[test]
    fn military_candidate_order_comes_from_the_conquests_roster() {
        let candidates = UnitKind::simple_military();
        assert!(candidates.contains(&UnitKind::Spearman));
        assert!(candidates.contains(&UnitKind::JaguarWarrior));
        assert!(candidates.contains(&UnitKind::ModernParatrooper));
        assert!(!candidates.contains(&UnitKind::CruiseMissile));
        assert!(!candidates.contains(&UnitKind::Fighter));
        assert!(candidates.windows(2).all(|pair| {
            UnitKind::ALL_NATIVE
                .iter()
                .position(|kind| *kind == pair[0])
                < UnitKind::ALL_NATIVE
                    .iter()
                    .position(|kind| *kind == pair[1])
        }));
    }

    #[test]
    fn tactical_defender_candidates_have_equal_source_stats_and_native_order() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.prune_unit_ai_state();
        game.players[1].known = vec![
            Tech::NATIONALISM,
            Tech::AMPHIBIOUS_WARFARE,
            Tech::REPLACEABLE_PARTS,
        ];
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.river = false;
            tile.base = None;
            tile.owner = None;
            tile.hut = false;
        }
        game.create_city(1, 10, 20);
        let rubber = offset_native_by_map(10, 20, 1, 0);
        game.tile_mut(10, 20).road = true;
        let rubber_tile = game.tile_mut(rubber.0, rubber.1);
        rubber_tile.terrain = Terrain::Forest;
        rubber_tile.resource = Some(Resource::Rubber);
        rubber_tile.road = true;
        rubber_tile.owner = Some(1);
        assert!(
            game.unit_buildable(&game.cities[0], UnitKind::Marine),
            "Marine source gates not met"
        );
        assert!(
            game.unit_buildable(&game.cities[0], UnitKind::Guerilla),
            "Guerilla source gates not met"
        );
        assert_eq!(
            UnitKind::Guerilla.stats().defense,
            UnitKind::Marine.stats().defense
        );
        assert_eq!(UnitKind::Guerilla.stats().hp, UnitKind::Marine.stats().hp);
        assert_eq!(UnitKind::Marine.stats().cost, 120);
        assert_eq!(UnitKind::Guerilla.stats().cost, 90);
        let marine_order = UnitKind::ALL_NATIVE
            .iter()
            .position(|kind| *kind == UnitKind::Marine)
            .unwrap();
        let guerilla_order = UnitKind::ALL_NATIVE
            .iter()
            .position(|kind| *kind == UnitKind::Guerilla)
            .unwrap();
        assert!(marine_order < guerilla_order);
    }

    #[test]
    fn ordinary_offense_forecasts_a_counter_defender_even_for_an_empty_city() {
        use crate::sim::{Government, offset_native_by_map};
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.prune_unit_ai_state();
        game.players[1].known = vec![Tech::WARRIOR_CODE, Tech::IRON_WORKING];
        game.players[1].government = Government::Democracy;
        game.players[1].rates = [0, 0, 100];
        game.sign_pact(1, HUMAN, crate::sim::Pact::War);
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Hills;
            tile.resource = Some(Resource::Coal);
            tile.mine = true;
            tile.road = true;
            tile.railroad = true;
            tile.river = true;
            tile.base = None;
            tile.owner = None;
            tile.hut = false;
            tile.known_by.fill(true);
        }
        game.create_city(1, 10, 20);
        game.tile_mut(10, 20).resource = Some(Resource::Iron);
        game.tile_mut(10, 20).road = true;
        let worked: Vec<_> = Simulation::city_radius(&game.cities[0])
            .filter(|&(dx, dy, _, _)| (dx, dy) != (0, 0))
            .map(|(_, _, x, y)| (x, y))
            .collect();
        game.cities[0].size = worked.len() as i32;
        game.cities[0].worked_tiles = worked;
        game.cities[0].buildings.extend([
            Building::FACTORY,
            Building::MFG_PLANT,
            Building::SOLAR_PLANT,
            Building::MARKETPLACE,
            Building::BANK,
            Building::STOCK_EXCHANGE,
        ]);
        let target = offset_native_by_map(10, 20, 4, 0);
        game.tile_mut(target.0, target.1).terrain = Terrain::Hills;
        game.tile_mut(target.0, target.1).resource = None;
        game.tile_mut(target.0, target.1).mine = false;
        game.tile_mut(target.0, target.1).railroad = false;
        game.tile_mut(target.0, target.1).river = false;
        game.create_city(HUMAN, target.0, target.1);
        game.cities.last_mut().unwrap().size = 8;
        assert!(game.city_forecast(&game.cities[0]).shield_surplus() > 45);
        assert!(game.unit_buildable(&game.cities[0], UnitKind::Archer));
        assert!(
            !game.unit_buildable(&game.cities[0], UnitKind::Warrior),
            "known Swordsman successor obsoletes Warrior production"
        );
        assert!(game.unit_buildable(&game.cities[0], UnitKind::Swordsman));
        // This fixture validates imported buildability/obsolescence only.
        // The exact offensive production score is an AI policy choice, so
        // don't encode a specific selected type as a Conquests roster rule.
    }
}
