use super::{BASE_TECH_COST, MIN_TECH_COST, SCIENCEBOX_PERCENT, TECH_COST_PERCENT, percent_of};

mod entities;
mod terrain;
mod unit_stats;
mod units;

pub(crate) use entities::{City, Player, Production, Unit, UnitState, WorkKind};
pub(crate) use terrain::{Resource, Terrain, Tile};
pub(crate) use units::UnitKind;

pub(super) fn research_cost(requirements: i32) -> i32 {
    let base = (BASE_TECH_COST * requirements).max(MIN_TECH_COST);
    percent_of(percent_of(base, TECH_COST_PERCENT), SCIENCEBOX_PERCENT).max(1)
}

#[cfg(test)]
mod resource_terrain_tests {
    use super::*;
    use crate::sim::Simulation;

    #[test]
    fn ivory_is_active_on_plains_and_glaciers_but_not_unrelated_terrain() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let tile = sim.tile_mut(10, 10);
        tile.resource = Some(Resource::Ivory);

        tile.terrain = Terrain::Plains;
        assert_eq!(tile.active_resource(), Some(Resource::Ivory));

        tile.terrain = Terrain::Glacier;
        assert_eq!(tile.active_resource(), Some(Resource::Ivory));

        tile.terrain = Terrain::Hills;
        assert_eq!(tile.active_resource(), None);
    }
}

#[cfg(test)]
mod native_type_order_tests {
    use super::*;
    use crate::rules::{Building, Tech};
    use crate::sim::{HUMAN, Simulation};

    #[test]
    fn simple_military_cache_excludes_nonmil_fuelled_large_and_native_only_sea_types() {
        for kind in UnitKind::SIMPLE_MILITARY {
            assert!(!kind.is_civilian());
            assert_eq!(kind.fuel_capacity(), 0);
            assert!(kind.transport_capacity() < 8);
        }
        for kind in [
            UnitKind::Trireme,
            UnitKind::Caravel,
            UnitKind::Galleon,
            UnitKind::Transport,
            UnitKind::Carrier,
            UnitKind::Submarine,
            UnitKind::Fighter,
            UnitKind::Bomber,
            UnitKind::CruiseMissile,
            UnitKind::Nuclear,
            UnitKind::Diplomat,
        ] {
            assert!(!UnitKind::SIMPLE_MILITARY.contains(&kind), "{kind:?}");
        }
        assert!(UnitKind::SIMPLE_MILITARY.contains(&UnitKind::Helicopter));
        let position = |kind| {
            UnitKind::SIMPLE_MILITARY
                .iter()
                .position(|&candidate| candidate == kind)
                .unwrap()
        };
        assert!(position(UnitKind::Partisan) < position(UnitKind::Marines));
        assert!(position(UnitKind::Paratroopers) < position(UnitKind::Horseman));
        assert!(position(UnitKind::Knights) < position(UnitKind::Catapult));
        assert!(position(UnitKind::AegisCruiser) < position(UnitKind::Battleship));
    }

    #[test]
    fn tactical_equal_defense_and_cost_uses_later_native_type() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.prune_unit_ai_state();
        game.players[1].known = vec![Tech::GUERILLA_WARFARE, Tech::AMPHIBIOUS_WARFARE];
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.river = false;
            tile.base = None;
            tile.owner = None;
            tile.hut = false;
        }
        game.create_city(1, 10, 20);
        let attacker = game.spawn_unit(HUMAN, UnitKind::Cannon, 10, 20);
        let attacker = game.unit_by_id(attacker).unwrap();
        let defender = game.defender_versus(&game.cities[0], attacker).unwrap();
        assert_eq!(
            UnitKind::Partisan.stats().defense,
            UnitKind::Marines.stats().defense
        );
        assert_eq!(UnitKind::Partisan.stats().hp, UnitKind::Marines.stats().hp);
        assert_eq!(
            UnitKind::Partisan.stats().cost,
            UnitKind::Marines.stats().cost
        );
        assert_eq!(defender.kind, UnitKind::Marines);
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
        assert!(game.unit_buildable(&game.cities[0], UnitKind::Warrior));
        assert!(game.unit_buildable(&game.cities[0], UnitKind::Archer));
        assert!(game.unit_buildable(&game.cities[0], UnitKind::Legion));
        // kill_something_with retains the buildable counter-defender TYPE
        // even for an empty nearby city. Candidate profits therefore do not
        // tie: Legion's stronger attack beats the cheaper Warrior/Archer.
        assert_eq!(
            game.offensive_production(&game.cities[0]),
            Some(UnitKind::Legion)
        );
    }
}
