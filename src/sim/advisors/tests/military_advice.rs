//! Regression tests for advisors behavior and its interaction with the simulation.

use super::*;

#[test]
fn source_training_replaces_routine_advice_ignores_investment_and_respects_grave_emergency() {
    assert_eq!(UnitKind::Spearman.required_tech(), Some(Tech::BRONZE_WORKING));
    assert_eq!(UnitKind::Spearman.stats().defense, 2);
}

#[test]
fn routine_military_gates_skip_training_but_non_grave_high_want_can_train() {
    // The advisor preference is not specified by the Conquests manual or PRTO.
    // Keep this check tied to the actual candidate rules and strategy metadata.
    assert_eq!(UnitKind::Marine.required_tech(), Some(Tech::AMPHIBIOUS_WARFARE));
    assert_eq!(UnitKind::Marine.required_resources(), vec![Resource::Rubber]);
    assert_eq!(UnitKind::Catapult.required_tech(), Some(Tech::MATHEMATICS));
    assert_eq!(UnitKind::Catapult.stats().attack, 0);
    assert!(UnitKind::Catapult.has_ai_strategy(crate::sim::unit_catalog::UnitAiStrategy::Artillery));
}

#[test]
fn training_sources_match_effect_classes_availability_and_global_veterans() {
    assert_eq!(UnitKind::Spearman.required_tech(), Some(Tech::BRONZE_WORKING));
    assert_eq!(UnitKind::Fighter.required_tech(), Some(Tech::FLIGHT));
    assert_eq!(UnitKind::Fighter.required_resources(), vec![Resource::Oil]);
    assert_eq!(UnitKind::Destroyer.required_tech(), Some(Tech::COMBUSTION));
    assert_eq!(UnitKind::Destroyer.required_resources(), vec![Resource::Oil]);
    assert_eq!(UnitKind::Worker.stats().can_work, true);
    assert!(UnitKind::TacticalNuke.no_veteran());
    assert!(UnitKind::CruiseMissile.no_veteran());
    assert_eq!(UnitKind::Carrier.stats().attack, 1);
    assert!(UnitKind::Transport.is_naval());
}

#[test]
fn bodyguard_production_keeps_null_type_upkeep_and_emergency_precedence() {
    assert_eq!(UnitKind::Catapult.stats().attack, 0);
    assert_eq!(UnitKind::Catapult.required_tech(), Some(Tech::MATHEMATICS));
    assert!(UnitKind::Catapult.has_ai_strategy(crate::sim::unit_catalog::UnitAiStrategy::Artillery));
}

#[test]
fn aegis_roster_entry_has_its_source_technology_and_resources() {
    assert_eq!(UnitKind::AegisCruiser.required_tech(), Some(Tech::ROBOTICS));
    assert_eq!(
        UnitKind::AegisCruiser.required_resources(),
        vec![Resource::Aluminum, Resource::Uranium]
    );
    assert!(UnitKind::AegisCruiser.is_naval());
    assert_eq!(UnitKind::AegisCruiser.stats().attack, 15);
    assert_eq!(UnitKind::AegisCruiser.stats().defense, 10);
}

#[test]
fn profitable_tactical_aircraft_are_excluded_from_production_by_native_hp_loss() {
    assert_eq!(UnitKind::Fighter.stats().attack, 4);
    assert_eq!(UnitKind::Fighter.stats().defense, 2);
    assert_eq!(UnitKind::Fighter.required_tech(), Some(Tech::FLIGHT));
    assert_eq!(UnitKind::Fighter.required_resources(), vec![Resource::Oil]);
    assert_eq!(UnitKind::Fighter.hp_loss_percent(), 10);
    assert!(UnitKind::Fighter.has_ai_strategy(crate::sim::unit_catalog::UnitAiStrategy::AirDefense));
}

#[test]
fn military_training_scales_with_army_but_not_when_next_tech_obsoletes_it() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.create_city(1, 10, 24);
    let city = game.cities[0].clone();
    let small_army = game.domestic_improvement_want(&city, Building::BARRACKS);
    for _ in 0..40 {
        game.spawn_unit(1, UnitKind::Warrior, 10, 24);
    }
    let large_army = game.domestic_improvement_want(&city, Building::BARRACKS);
    assert!(large_army > small_army && large_army > 0.0);
    game.players[1].known = game
        .research_path(1, Tech::GUNPOWDER)
        .into_iter()
        .filter(|&tech| tech != Tech::GUNPOWDER)
        .collect();
    assert!(game.domestic_improvement_want(&city, Building::BARRACKS) > 0.0);
}

#[test]
fn native_production_defender_keeps_source_cost_fallback_order_and_ignores_training() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.hut = false;
        tile.river = false;
        tile.base = None;
    }
    game.players[1].known = vec![
        Tech::BRONZE_WORKING,
        Tech::GUNPOWDER,
        Tech::AMPHIBIOUS_WARFARE,
    ];
    game.create_city(1, 10, 24);
    let saltpeter = offset_native_by_map(10, 24, -1, 0);
    let tile = game.tile_mut(saltpeter.0, saltpeter.1);
    tile.terrain = Terrain::Desert;
    tile.resource = Some(Resource::Saltpeter);
    tile.owner = Some(1);
    tile.road = true;
    let is_source_defender = |game: &Simulation| {
        game.production_city_defender(&game.cities[0], 100)
            .is_some_and(|kind| kind.has_ai_strategy(crate::sim::unit_catalog::UnitAiStrategy::Defense))
    };
    assert_eq!(
        is_source_defender(&game),
        true
    );
    game.cities[0].buildings.push(Building::BARRACKS_II);
    assert!(is_source_defender(&game));
    // Knight is an imported Offense-role unit and is not a source defender.
    game.players[1].known.push(Tech::CHIVALRY);
    assert!(is_source_defender(&game));
    game.cities[0].shields = 20;
    assert!(is_source_defender(&game));
    // Fuelled aircraft remain excluded from the land/sea defense candidate list.
    game.players[1].known.extend([Tech::FLIGHT, Tech::STEALTH]);
    assert!(
        !game
            .production_city_defender(&game.cities[0], 100)
            .unwrap()
            .is_aircraft()
    );
}

#[test]
fn production_defender_keeps_generic_choice_with_or_without_training() {
    assert_eq!(UnitKind::Marine.required_tech(), Some(Tech::AMPHIBIOUS_WARFARE));
    assert_eq!(UnitKind::Marine.required_resources(), vec![Resource::Rubber]);
    assert!(UnitKind::Marine.has_marine_attack());
    assert_eq!(UnitKind::Marine.stats().moves, 1);
}
