//! Regression tests for advisors behavior and its interaction with the simulation.

use super::*;

#[test]
fn source_training_replaces_routine_advice_ignores_investment_and_respects_grave_emergency() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        game.military_training_advisor_playtest(),
        (true, true, true, true)
    );
}

#[test]
fn routine_military_gates_skip_training_but_non_grave_high_want_can_train() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.diplomacy.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.hut = false;
        tile.resource = None;
        tile.base = None;
    }
    game.players[1].known = vec![Tech::BRONZE_WORKING];
    game.players[1].government = Government::Despotism;
    game.tile_mut(10, 24).terrain = Terrain::Hills;
    game.tile_mut(10, 24).mine = true;
    game.create_city(1, 10, 24);
    let city = game.cities[0].clone();
    // A two-turn threat is urgent enough to request the first defender,
    // but native grave-danger early return does not apply yet.
    let far = offset_native_by_map(10, 24, 2, 0);
    game.spawn_unit(HUMAN, UnitKind::Marines, far.0, far.1);
    assert_eq!(game.city_grave_danger(&city), 0);
    let (before, after) = game.military_build_advice(&city);
    assert!(before.is_some_and(|c| c.want >= MILITARY_EMERGENCY_WANT));
    assert!(
        after.is_some_and(|c| c.production == Production::Building(Building::BARRACKS)
            && c.want == f64::from((before.unwrap().want * 0.5) as f32))
    );
    game.units.clear();
    let charge = game.spawn_unit(1, UnitKind::Catapult, 10, 24);
    game.request_ai_guard(charge);
    game.players[1].known.push(Tech::MASONRY);
    game.cities[0].production = Production::Building(Building::PYRAMIDS);
    assert!(!game.routine_military_advice_allowed(&game.cities[0]));
    assert!(
        game.military_build_advice(&game.cities[0]).1.is_none(),
        "wonder-city stage returns before bodyguard/training advice"
    );
    game.cities[0].production = Production::Unit(UnitKind::Warrior);
    game.players[1].government = Government::Republic;
    for _ in 0..5 {
        game.spawn_unit(1, UnitKind::Warrior, 10, 24);
        game.units.last_mut().unwrap().home_city = Some(city.id);
    }
    assert_eq!(game.city_forecast(&game.cities[0]).shield_surplus(), 0);
    assert!(game.military_build_advice(&game.cities[0]).1.is_none());
    game.units.retain(|u| u.id == charge);
    game.units[0].home_city = None;
    game.cities[0].size = 4;
    game.cities[0].specialists = [0, 0, 2]; // two happy workers remain after luxury
    let abroad = offset_native_by_map(10, 24, 8, 0);
    for _ in 0..3 {
        game.spawn_unit(1, UnitKind::Warrior, abroad.0, abroad.1);
        game.units.last_mut().unwrap().home_city = Some(city.id);
    }
    let forecast = game.city_forecast(&game.cities[0]);
    let (_, effect, _) = game.city_feelings_after_effects(&game.cities[0], forecast.luxury);
    let (_, final_unhappy, _) = game.city_feelings(&game.cities[0], forecast.luxury);
    assert!(
        forecast.shield_surplus() > 0 && final_unhappy > effect,
        "orderly positive-production city must still be gated by unrest added after effects"
    );
    assert!(
        game.military_build_advice(&game.cities[0]).1.is_none(),
        "unrest introduced after effects prevents non-life-saving advice"
    );
}

#[test]
fn training_sources_match_effect_classes_availability_and_global_veterans() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.hut = false;
    }
    game.create_city(1, 10, 24);
    game.players[1].known.clear();
    let input = |kind| AiBuildChoice {
        production: Production::Unit(kind),
        want: 93.3,
    };
    let choose = |game: &Simulation, kind| {
        let mut choice = input(kind);
        game.adjust_military_training_choice(&game.cities[0], &mut choice);
        choice
    };
    assert_eq!(
        choose(&game, UnitKind::Phalanx).production,
        Production::Building(Building::BARRACKS)
    );
    assert_eq!(
        choose(&game, UnitKind::Phalanx).want,
        46.650001525878906,
        "native f32 assignment then Builder integer division"
    );
    game.players[1].known.push(Tech::GUNPOWDER);
    assert_eq!(
        choose(&game, UnitKind::Musketeers).production,
        Production::Unit(UnitKind::Musketeers),
        "obsolete Barracks cannot be selected after Gunpowder"
    );
    game.players[1].known.push(Tech::MOBILE_WARFARE);
    assert_eq!(
        choose(&game, UnitKind::Marines).production,
        Production::Unit(UnitKind::Marines),
        "obsolete Barracks cannot be selected after Mobile Warfare"
    );
    assert_eq!(
        choose(&game, UnitKind::Fighter).production,
        Production::Unit(UnitKind::Fighter),
        "no Flight means no Airport provider"
    );
    game.players[1].known.push(Tech::FLIGHT);
    for kind in [
        UnitKind::Fighter,
        UnitKind::Helicopter,
        UnitKind::CruiseMissile,
        UnitKind::Nuclear,
    ] {
        assert_eq!(
            choose(&game, kind).production,
            Production::Building(Building::AIRPORT)
        );
    }
    // Native max veteran level zero does not block Nuclear's provider search.
    assert_eq!(game.trained_rank(&game.cities[0], UnitKind::Nuclear), 0);
    game.cities[0].buildings.push(Building::AIRPORT);
    assert_eq!(
        choose(&game, UnitKind::Fighter).production,
        Production::Unit(UnitKind::Fighter)
    );
    let coast = offset_native_by_map(10, 24, 1, 0);
    game.tile_mut(coast.0, coast.1).terrain = Terrain::Sea;
    game.players[1].known.push(Tech::AMPHIBIOUS_WARFARE);
    assert_eq!(
        choose(&game, UnitKind::Destroyer).production,
        Production::Building(Building::PORT_FACILITY)
    );
    assert_eq!(
        choose(&game, UnitKind::Carrier).production,
        Production::Building(Building::PORT_FACILITY),
        "zero attack is not NonMil"
    );
    for kind in [
        UnitKind::Caravel,
        UnitKind::Transport,
        UnitKind::Diplomat,
        UnitKind::Worker,
    ] {
        assert_eq!(
            choose(&game, kind).production,
            Production::Unit(kind),
            "NonMil choices are exempt"
        );
    }
    game.cities[0].buildings.push(Building::LIGHTHOUSE);
    assert_eq!(
        choose(&game, UnitKind::Destroyer).production,
        Production::Unit(UnitKind::Destroyer)
    );
    game.cities[0]
        .buildings
        .retain(|b| *b != Building::LIGHTHOUSE);
    game.cities[0]
        .buildings
        .push(Building::MAGELLANS_EXPEDITION);
    assert_eq!(
        choose(&game, UnitKind::Destroyer).production,
        Production::Building(Building::PORT_FACILITY),
        "combat promotion bonus is not veteran construction"
    );
    game.cities[0].buildings.push(Building::PORT_FACILITY);
    assert_eq!(
        choose(&game, UnitKind::Destroyer).production,
        Production::Unit(UnitKind::Destroyer)
    );
    let mut building = AiBuildChoice {
        production: Production::Building(Building::TEMPLE),
        want: 100.0,
    };
    game.adjust_military_training_choice(&game.cities[0], &mut building);
    assert_eq!(building.production, Production::Building(Building::TEMPLE));
}

#[test]
fn bodyguard_production_keeps_null_type_upkeep_and_emergency_precedence() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        game.ai_bodyguard_production_playtest(),
        (true, true, true, true)
    );
}

#[test]
fn sea_bodyguard_requires_defendgood_role_and_compatible_charge() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.diplomacy.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
        tile.owner = None;
        tile.hut = false;
    }
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    game.create_city(1, 10, 24);
    game.players[1].known = Tech::all().collect();
    let city = game.cities[0].clone();
    let charge = game.spawn_unit(1, UnitKind::Transport, 10, 24);
    game.request_ai_guard(charge);
    assert_eq!(
        game.bodyguard_production_choice(&city, true)
            .map(|(kind, _)| kind),
        Some(UnitKind::AegisCruiser)
    );
    game.units.iter_mut().find(|u| u.id == charge).unwrap().kind = UnitKind::Trireme;
    assert!(
        game.bodyguard_production_choice(&city, true).is_none(),
        "Sea Aegis cannot follow Trireme's River-native class"
    );
}

#[test]
fn profitable_tactical_aircraft_are_excluded_from_production_by_native_hp_loss() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (fighter, victim) = game.ai_aircraft_advisor_playtest_setup();
    // Keep the profitable industrial scenario independent of the new
    // Despotism default's per-tile output penalty.
    game.players[1].government = Government::Communism;
    game.units.retain(|unit| unit.id != fighter);
    let target = game
        .units
        .iter()
        .find(|unit| unit.id == victim)
        .unwrap()
        .clone();
    // Naval stacks lose only their defender. Use an exposed land stack
    // whose native stack-loss benefit really grows with its value.
    game.units.retain(|unit| unit.id != victim);
    game.tile_mut(target.x, target.y).terrain = Terrain::Grassland;
    for _ in 0..20 {
        game.spawn_unit(HUMAN, UnitKind::Marines, target.x, target.y);
    }
    // Keep a positive tactical opportunity independently of the native
    // unit-type HP-loss check that excludes offensive production.
    let home = game.cities[0].clone();
    let workers: Vec<_> = Simulation::city_radius(&home)
        .filter(|&(_, _, x, y)| (x, y) != (target.x, target.y))
        .map(|(_, _, x, y)| {
            let tile = game.tile_mut(x, y);
            tile.terrain = Terrain::Hills;
            tile.mine = true;
            tile.owner = Some(1);
            (x, y)
        })
        .filter(|&position| position != (home.x, home.y))
        .take(4)
        .collect();
    game.cities[0].worked_tiles = workers;
    game.cities[0].buildings.extend([
        Building::TEMPLE,
        Building::CATHEDRAL,
        Building::AMPHITHEATER,
        Building::BARRACKS,
        Building::BARRACKS_II,
        Building::BARRACKS_III,
        Building::AIRPORT,
        Building::PORT_FACILITY,
    ]);
    let invasion = offset_native_by_map(home.x, home.y, 1, 0);
    game.tile_mut(invasion.0, invasion.1).terrain = Terrain::Grassland;
    game.spawn_unit(HUMAN, UnitKind::Marines, invasion.0, invasion.1);
    let city = game.cities[0].clone();
    let mut tactical = game.make_unit(0, 1, UnitKind::Fighter, city.x, city.y);
    tactical.home_city = Some(city.id);
    tactical.veteran = game.trained_rank(&city, UnitKind::Fighter);
    assert!(game.aircraft_target_value(&tactical, target.x, target.y) > 0.0);
    assert!(game.aircraft_production_choice(&city).is_none());
    let defense = game.native_city_defense_choice(&city).0.unwrap();
    assert!(defense.want >= MILITARY_EMERGENCY_WANT);
    assert!(game.advisor_guard_city_assessment(&city).grave > 0);
    let choice = game.ai_build_choice(&city).unwrap();
    // A Fighter can itself be a CT_DEFENDER. Compare the role's exact
    // bid, rather than assuming every aircraft kind is an attacker.
    assert_eq!(choice.production, defense.production);
    assert_eq!(choice.want, defense.want);
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
    game.players[1].known = vec![Tech::GUNPOWDER, Tech::AMPHIBIOUS_WARFARE];
    game.create_city(1, 10, 24);
    assert_eq!(
        game.production_city_defender(&game.cities[0], 100),
        Some(UnitKind::Marines)
    );
    game.cities[0].buildings.push(Building::BARRACKS_II);
    assert_eq!(
        game.production_city_defender(&game.cities[0], 100),
        Some(UnitKind::Marines)
    );
    // Knights occur AFTER Marines in native simple_ai_types. At stock0
    // the cheaper40-shield candidate replaces a60-shield best even though
    // it has lower static desirability; at stock20 this fallback ends.
    game.players[1].known.push(Tech::CHIVALRY);
    assert_eq!(
        game.production_city_defender(&game.cities[0], 100),
        Some(UnitKind::Knights)
    );
    game.cities[0].shields = 20;
    assert_eq!(
        game.production_city_defender(&game.cities[0], 100),
        Some(UnitKind::Marines)
    );
    // Fuelled DefendGood aircraft are not in the source simple list.
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (stock, quiet, _) = game.ai_city_defense_playtest();
    assert!(stock && quiet);
    assert_eq!(
        game.native_city_defense_choice(&game.cities[0])
            .0
            .unwrap()
            .production,
        Production::Unit(UnitKind::Marines)
    );
    game.cities[0]
        .buildings
        .retain(|&b| b != Building::BARRACKS_II);
    assert_eq!(
        game.city_defense_production(&game.cities[0]),
        Some(Production::Unit(UnitKind::Marines))
    );
}
