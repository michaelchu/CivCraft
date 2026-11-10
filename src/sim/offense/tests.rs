//! Regression tests for the behavior of the parent module.

use super::*;
fn island_fixture() -> (Simulation, City, Unit, (i32, i32)) {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.prune_unit_ai_state();
    game.players[1].known = vec![Tech::WARRIOR_CODE];
    game.players[1].government = Government::Despotism;
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
        tile.known_by.fill(true);
        tile.owner = None;
        tile.hut = false;
        tile.resource = None;
        tile.river = false;
        tile.base = None;
    }
    let home = (10, 24);
    let enemy = offset_native_by_map(home.0, home.1, 8, 0);
    let beach = offset_native_by_map(enemy.0, enemy.1, -1, 0);
    for (x, y) in [home, enemy, beach] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
    }
    game.create_city(1, home.0, home.1);
    game.create_city(HUMAN, enemy.0, enemy.1);
    game.sign_pact(1, HUMAN, Pact::War);
    let city = game.cities[0].clone();
    game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
    let mut seed = game.make_unit(0, 1, UnitKind::Archer, home.0, home.1);
    seed.home_city = Some(city.id);
    seed.veteran = 1;
    (game, city, seed, enemy)
}

#[test]
fn bundled_bases_do_not_get_unconfigured_stack_protection_in_offensive_advice() {
    let (mut game, _, seed, _) = island_fixture();
    let tile = offset_native_by_map(seed.x, seed.y, 1, 0);
    game.tile_mut(tile.0, tile.1).terrain = Terrain::Grassland;
    game.spawn_unit(HUMAN, UnitKind::Warrior, tile.0, tile.1);
    game.spawn_unit(HUMAN, UnitKind::Warrior, tile.0, tile.1);
    for base in [Base::Fortress, Base::Airbase] {
        game.tile_mut(tile.0, tile.1).base = Some(base);
        assert_eq!(game.offensive_victim_count(tile.0, tile.1), 1);
    }
    game.create_city(HUMAN, tile.0, tile.1);
    assert_eq!(game.offensive_victim_count(tile.0, tile.1), 3);
}

#[test]
fn offensive_obsolescence_uses_the_imported_immediate_upgrade_target() {
    let (mut game, city, _, _) = island_fixture();
    game.players[1].known.extend([
        Tech::IRON_WORKING,
        Tech::GUNPOWDER,
        Tech::FEUDALISM,
        UnitKind::Rifleman.required_tech().unwrap(),
    ]);
    assert!(!game.unit_buildable(&city, UnitKind::Musketman));
    assert!(game.unit_available(1, UnitKind::Musketman));
    assert_eq!(UnitKind::Swordsman.obsolete_by(), Some(UnitKind::GallicSwordsman));
    assert!(!game.offensive_obsolete(&city, UnitKind::Swordsman));
    assert_eq!(UnitKind::Pikeman.obsolete_by(), Some(UnitKind::Musketman));
    assert!(game.offensive_obsolete(&city, UnitKind::Pikeman));
}

#[test]
fn nearby_empty_city_retains_unbuilt_rank_zero_defender_type_with_zero_unit_benefit() {
    let (mut game, _, seed, enemy) = island_fixture();
    game.players[HUMAN].known.push(Tech::BRONZE_WORKING);
    let target = OffensiveTarget {
        x: enemy.0,
        y: enemy.1,
        city: Some(game.cities[1].id),
        turns: 1,
        want: 1.0,
        ferry: None,
        new_ferry: false,
    };
    let (defender, benefit) = game.offensive_victim(&seed, &target);
    let defender = defender.expect("native retains def_type despite arrival<=1");
    assert_eq!(defender.kind, UnitKind::Spearman);
    assert_eq!(defender.veteran, 0);
    assert_eq!(benefit, 0.0);
    assert!(game.offensive_defense_squared(&defender, seed.kind, enemy.0, enemy.1, true) > 0);
    let current = game.spawn_unit(HUMAN, UnitKind::Marine, enemy.0, enemy.1);
    let marine = game.units.iter_mut().find(|u| u.id == current).unwrap();
    marine.hp = 1;
    marine.state = UnitState::Fortified;
    let (defender, benefit) = game.offensive_victim(&seed, &target);
    let defender = defender.unwrap();
    assert_eq!(defender.kind, UnitKind::Marine);
    assert_eq!(defender.hp, UnitKind::Marine.stats().hp);
    assert_eq!(defender.state, UnitState::Idle);
    assert_eq!(benefit, UnitKind::Marine.stats().cost as f32);
}

#[test]
fn fractional_ferry_override_survives_integer_conversion_below_the_discarded_prior_bid() {
    let (mut game, city, _, _) = island_fixture();
    // With no buildable boat the very same virtual Trireme target keeps
    // its unrounded attacker want. Learning Map Making changes only the
    // role callback's ability to substitute that boat in this fixture.
    let raw = game
        .offensive_advice(&city)
        .0
        .expect("real offensive sea job");
    assert_eq!(raw.production, Production::Unit(UnitKind::Archer));
    let fraction = raw.want.fract();
    assert!(fraction > 0.0 && fraction < 1.0);
    game.players[1].known.push(Tech::MAP_MAKING);
    assert!(game.can_produce(&city, Production::Unit(UnitKind::Galley)));
    let prior = AiBuildChoice {
        production: Production::Unit(UnitKind::Archer),
        want: raw.want.floor() + fraction / 2.0,
    };
    assert!(raw.want > prior.want && raw.want - prior.want < 1.0);
    let (final_choice, _, replaced) = game.offensive_advice_with_prior(&city, Some(prior));
    let final_choice = final_choice.unwrap();
    assert!(replaced);
    assert_eq!(final_choice.production, Production::Unit(UnitKind::Galley));
    assert_eq!(final_choice.want, raw.want.floor());
    assert!(
        final_choice.want < prior.want,
        "native already discarded prior before int conversion"
    );
    // A genuinely higher pre-conversion prior keeps its own provenance.
    let stronger = AiBuildChoice {
        want: raw.want + 0.25,
        ..prior
    };
    let (kept, _, replaced) = game.offensive_advice_with_prior(&city, Some(stronger));
    assert!(!replaced);
    assert_eq!(kept.unwrap().production, stronger.production);
    assert_eq!(kept.unwrap().want, stronger.want);
}

#[test]
fn virtual_ferry_stimulates_its_missing_technology_after_a_real_offensive_target() {
    let (game, city, seed, _) = island_fixture();
    assert!(game.offensive_find_ferry(&seed).is_none());
    let target = game
        .offensive_target(&city, &seed)
        .expect("virtual boat finds beachhead");
    assert!(target.new_ferry);
    assert_eq!(target.ferry.as_ref().unwrap().kind, UnitKind::Galley);
    let (choice, research) = game.offensive_advice(&city);
    assert!(choice.is_some());
    assert!(
        research
            .iter()
            .any(|(tech, want)| *tech == Tech::MAP_MAKING && *want > 0.0)
    );
}

#[test]
fn combined_ferry_search_finds_remote_boat_but_seed_rejects_an_unreachable_rendezvous() {
    let (mut game, city, seed, enemy) = island_fixture();
    let sea = offset_native_by_map(seed.x, seed.y, 4, 0);
    let boat_id = game.spawn_unit(1, UnitKind::Galleon, sea.0, sea.1);
    let boat = game
        .offensive_find_ferry(&seed)
        .expect("sea EC search reaches remote ferry");
    assert_eq!(boat.id, boat_id);
    let mut walk = NormalPaths::attack_map(&game, &seed);
    assert!(walk.position_turn(&game, boat.x, boat.y).is_none());
    assert!(
        game.offensive_target(&city, &seed).is_none(),
        "seed cannot board remote ship"
    );
    // Literal process_attacker_want retains its initialized sailing time
    // when its later per-type PF reaches neither boat nor target. The
    // stricter seed stage above prevents this from creating a seed job.
    let target = OffensiveTarget {
        x: enemy.0,
        y: enemy.1,
        city: Some(game.cities[1].id),
        turns: 0,
        want: 1.0,
        ferry: Some(boat.clone()),
        new_ferry: false,
    };
    let mut fmap = NormalPaths::overlap_map(&game, &boat);
    let (x, y, _) = game
        .offensive_beachhead(&mut fmap, seed.kind, seed.owner, enemy.0, enemy.1)
        .unwrap();
    assert_eq!(
        game.offensive_candidate_turns(&seed, &target),
        fmap.position_turn(&game, x, y)
    );
}

#[test]
fn literal_c_kill_desire_and_amortization_vectors_preserve_float_assignments() {
    // Literal extracted native expressions compiled as C (float adv_want,
    // double pow, MORT24), not values calculated by the Rust routine.
    assert_eq!(kill_desire(80.25, 1600, 40, 625, 3).to_bits(), 1153282254);
    assert_eq!(kill_desire(30.0, 400, 20, 225, 1).to_bits(), 1129054208);
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(1, 10, 24);
    let city = game.cities.last().unwrap();
    assert_eq!(
        game.offensive_amortize(city, 1234.5, 7, 0).to_bits(),
        1147477144
    );
}
#[test]
fn offensive_target_feedback_respects_visibility_and_ends_after_learning_the_tech() {
    let game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(game.offensive_research_playtest(), (true, true));
}
