use super::*;

fn espionage_game() -> Simulation {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.create_city(HUMAN, 5, 5);
    game.create_city(1, 20, 20);
    let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
    game.players[HUMAN].known.extend([writing, Tech::ESPIONAGE]);
    game.players[1].known.extend([writing, Tech::ESPIONAGE]);
    game.players[HUMAN].gold = 20_000;
    game.players[1].gold = 20_000;
    game.players[1].ai = true;
    game.cities
        .iter_mut()
        .filter(|city| city.owner == HUMAN || city.owner == 1)
        .for_each(|city| city.buildings.push(Building::INTELLIGENCE_AGENCY));
    game.make_diplomatic_contact(HUMAN, 1);
    game.diplomacy
        .get_mut(&Simulation::relation_key(HUMAN, 1))
        .unwrap()
        .pact = Pact::Peace;
    game
}

fn set_embassy(game: &mut Simulation, owner: usize, opponent: usize) {
    let direction = usize::from(owner > opponent);
    game.diplomacy
        .entry(Simulation::relation_key(owner, opponent))
        .or_default()
        .embassy[direction] = true;
}

#[test]
fn establishing_an_embassy_reports_the_capital_and_investigation_reports_city_assets() {
    let mut game = espionage_game();
    let gold = game.players[HUMAN].gold;

    assert!(game.perform_espionage(
        1,
        EspionageMission::EstablishEmbassy,
        None,
        EspionageRisk::Immediate,
    ));
    assert!(game.has_real_embassy(HUMAN, 1));
    assert!(game.players[HUMAN].gold < gold);
    assert!(
        game.events
            .iter()
            .any(|event| event.contains("Intelligence report"))
    );

    let city = game.cities.iter().find(|city| city.owner == 1).unwrap().id;
    assert!(game.perform_espionage(
        1,
        EspionageMission::InvestigateCity,
        Some(city),
        EspionageRisk::Immediate,
    ));
    assert!(
        game.events
            .iter()
            .any(|event| event.contains("Cost:") && event.contains("Intelligence report"))
    );
}

#[test]
fn a_planted_spy_can_steal_technology_and_discovery_removes_it_and_starts_ai_war_planning() {
    let mut base = espionage_game();
    set_embassy(&mut base, HUMAN, 1);
    base.players[1].known.push(Tech::ALPHABET);
    base.players[1].known.push(Tech::POTTERY);
    base.diplomacy.get_mut(&(HUMAN, 1)).unwrap().spy[0] = Some(SpyAsset { veteran: false });

    let mut stolen_technology = false;
    for seed in 0..256 {
        let mut game = base.clone();
        game.set_random_seed(seed);
        if game.perform_espionage(
            1,
            EspionageMission::StealTechnology,
            None,
            EspionageRisk::Safely,
        ) && (game.has_tech(HUMAN, Tech::ALPHABET) || game.has_tech(HUMAN, Tech::POTTERY))
        {
            stolen_technology = true;
            break;
        }
    }
    assert!(
        stolen_technology,
        "a safe operation should eventually succeed"
    );

    let mut caught = false;
    for seed in 0..256 {
        let mut game = base.clone();
        game.set_random_seed(seed);
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = -1_000;
        game.perform_espionage(
            1,
            EspionageMission::StealTechnology,
            None,
            EspionageRisk::Immediate,
        );
        let relation = &game.diplomacy[&(HUMAN, 1)];
        if relation.spy[0].is_none() {
            assert!(relation.trust[1] < -1_000);
            assert!(relation.cause_until[1] > game.turn);
            game.advance_ai_war_plans();
            assert!(game.ai_war_plans.contains_key(&(1, HUMAN)));
            caught = true;
            break;
        }
    }
    assert!(
        caught,
        "a discovered operation should lose its Spy and provoke an AI response"
    );
}

#[test]
fn ai_can_establish_embassies_and_plant_spies_through_the_shared_mission_rules() {
    let mut base = espionage_game();
    let mut embassy_seed = None;
    for seed in 0..256 {
        let mut game = base.clone();
        game.set_random_seed(seed);
        game.run_ai_espionage(1);
        if game.has_real_embassy(1, HUMAN) {
            embassy_seed = Some(seed);
            break;
        }
    }
    assert!(
        embassy_seed.is_some(),
        "AI should eventually establish an embassy"
    );

    set_embassy(&mut base, 1, HUMAN);
    let mut planted = false;
    for seed in 0..256 {
        let mut game = base.clone();
        game.set_random_seed(seed);
        game.run_ai_espionage(1);
        if game.diplomacy[&(HUMAN, 1)].spy[1].is_some() {
            planted = true;
            break;
        }
    }
    assert!(planted, "AI should eventually plant a Spy");

    let mut operation_succeeded = false;
    for seed in 0..256 {
        let mut game = base.clone();
        game.players[HUMAN].known.push(Tech::ALPHABET);
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().spy[1] = Some(SpyAsset { veteran: false });
        game.set_random_seed(seed);
        if game.perform_espionage_for(
            1,
            HUMAN,
            EspionageMission::StealTechnology,
            None,
            EspionageRisk::Safely,
        ) && game.has_tech(1, Tech::ALPHABET)
        {
            operation_succeeded = true;
            break;
        }
    }
    assert!(
        operation_succeeded,
        "AI Spy missions must resolve using the same outcomes"
    );
}

#[test]
fn failed_plant_lockout_decays_once_per_turn_and_embassies_reopen_after_peace() {
    let mut game = espionage_game();
    set_embassy(&mut game, HUMAN, 1);
    let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
    relation.failed_spy_plant_turn[0] = Some(game.turn);
    relation.pact = Pact::War;
    assert!(game.has_real_embassy(HUMAN, 1));
    assert!(game.at_war(HUMAN, 1));

    game.advance_diplomacy();
    assert_eq!(
        game.diplomacy[&(HUMAN, 1)].failed_spy_plant_turn[0],
        Some(game.turn)
    );
    game.turn += 1;
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
    assert!(game.has_embassy(HUMAN, 1));

    let mut expired = false;
    for seed in 0..256 {
        let mut candidate = game.clone();
        candidate.turn = candidate.diplomacy[&(HUMAN, 1)].failed_spy_plant_turn[0].unwrap() + 1;
        candidate.set_random_seed(seed);
        candidate.advance_diplomacy();
        if candidate.diplomacy[&(HUMAN, 1)].failed_spy_plant_turn[0].is_none() {
            expired = true;
            break;
        }
    }
    assert!(
        expired,
        "the retry marker should have a one-third chance to clear each turn"
    );
}

#[test]
fn stolen_world_map_provides_the_rivals_last_seen_tile() {
    let mut game = espionage_game();
    set_embassy(&mut game, HUMAN, 1);
    let (x, y) = (28, 32);
    let scout = game.spawn_unit(1, UnitKind::Explorer, x, y);
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.refresh_visibility();
    game.units.retain(|unit| unit.id != scout);
    game.tile_mut(x, y).terrain = Terrain::Ocean;
    game.refresh_visibility();
    assert!(!game.tile(x, y).known_by[HUMAN]);
    assert_eq!(
        game.remembered_tile(1, x, y).unwrap().terrain,
        Terrain::Grassland
    );
    let outcome = (1..=256)
        .find_map(|seed| {
            let mut candidate = game.clone();
            candidate.set_random_seed(seed);
            candidate.perform_espionage(
                1,
                EspionageMission::StealWorldMap,
                None,
                EspionageRisk::Safely,
            );
            candidate
                .events
                .iter()
                .any(|msg| msg.contains("stole") && msg.contains("world map"))
                .then_some(candidate)
        })
        .expect("at least one mission succeeded");
    assert!(outcome.tile(x, y).known_by[HUMAN]);
    assert_eq!(
        outcome.human_known_tile(x, y).map(|tile| tile.terrain),
        Some(Terrain::Grassland)
    );
}

fn propaganda_cost(game: &Simulation, city: &City) -> i32 {
    game.espionage_cost(
        HUMAN,
        1,
        city,
        EspionageMission::InitiatePropaganda,
        EspionageRisk::Immediate,
    )
}

#[test]
fn propaganda_costs_more_for_a_city_with_buildings_and_wonders() {
    let mut game = espionage_game();
    let index = game.cities.iter().position(|city| city.owner == 1).unwrap();
    game.cities[index].size = 3;
    game.cities[index].buildings.clear();
    let bare = propaganda_cost(&game, &game.cities[index]);

    game.cities[index].buildings.push(Building::GREAT_LIBRARY);
    let with_wonder = propaganda_cost(&game, &game.cities[index]);
    assert!(
        with_wonder >= bare * 2,
        "a Great Wonder must raise the incitement price well above a bare city: {bare} vs {with_wonder}"
    );

    game.cities[index].buildings.clear();
    game.cities[index].buildings.extend([
        Building::HARBOR,
        Building::FACTORY,
        Building::POWER_PLANT,
    ]);
    let with_improvements = propaganda_cost(&game, &game.cities[index]);
    assert!(
        with_improvements > bare,
        "improvements raise the price: {bare} vs {with_improvements}"
    );
}
