//! Regression tests for advisors behavior and its interaction with the simulation.

use super::*;

#[test]
fn military_training_scales_with_army_but_not_when_next_tech_obsoletes_it() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.create_city(1, 10, 24);
    let city = game.cities[0].clone();
    let small_army = game.domestic_improvement_want(&city, Building::BARRACKS);
    for _ in 0..40 {
        game.spawn_unit(1, UnitKind::Warrior, 10, 24);
    }
    let large_army = game.domestic_improvement_want(&city, Building::BARRACKS);
    assert!(large_army > small_army && large_army > 0.0);
    game.players[1].known = Tech::all()
        .filter(|&tech| tech.is_civ3_advance() && tech != Tech::GUNPOWDER)
        .collect();
    assert_eq!(
        game.research_path(1, Tech::GUNPOWDER),
        vec![Tech::GUNPOWDER]
    );
    // Only Gunpowder remains; the Civ III barracks becomes obsolete when it
    // is discovered, so Freeciv's advisor subtracts the entire want.
    assert_eq!(
        game.domestic_improvement_want(&city, Building::BARRACKS),
        0.0
    );
}

#[test]
fn production_defender_requires_defense_role_and_is_stable_under_training() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
    let choose = |game: &Simulation| {
        game.production_city_defender(&game.cities[0], 100)
            .expect("fixture must offer an eligible defender")
    };
    let untrained = choose(&game);
    assert!(
        super::super::action_rules::freeciv_unit_has_role(untrained, "DefendGood")
            || super::super::action_rules::freeciv_unit_has_role(untrained, "DefendOk")
    );
    game.cities[0].buildings.push(Building::BARRACKS_II);
    assert_eq!(
        choose(&game),
        untrained,
        "training changed this selector's choice"
    );
    let is_source_defender = |game: &Simulation| {
        let kind = choose(game);
        super::super::action_rules::freeciv_unit_has_role(kind, "DefendGood")
            || super::super::action_rules::freeciv_unit_has_role(kind, "DefendOk")
    };
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
