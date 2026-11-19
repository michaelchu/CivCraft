use super::*;

fn game() -> Simulation {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.sync_list_order();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = Some(Resource::Wheat);
        tile.owner = None;
        tile.hut = false;
    }
    game
}

fn balanced(snapshot: &LifecycleSnapshot) {
    for u in &snapshot.unit_totals {
        let arrived = u.initial + u.created + u.transferred_in + u.upgraded_in + u.unobserved_added;
        let departed = u.destroyed
            + u.consumed
            + u.disbanded
            + u.transferred_out
            + u.upgraded_out
            + u.unobserved_removed;
        assert_eq!(arrived, u.existing + departed, "{u:?}");
        assert!(u.built <= u.created);
    }
    for c in &snapshot.city_totals {
        assert_eq!(
            c.initial + c.created + c.conquered + c.received + c.unobserved_added,
            c.existing
                + c.lost_to_conquest
                + c.transferred_out
                + c.destroyed
                + c.unobserved_removed,
            "{c:?}"
        );
    }
}

#[test]
fn production_and_loss_within_one_boundary_survive_event_overflow() {
    let mut game = game();
    game.trace = trace::Collector::enabled(std::collections::BTreeSet::from(["*".into()]), 1);
    game.start_lifecycle_observation();
    // Keep this unit test focused on overflow retention; scenario coverage owns
    // the larger production/founding/conquest/trade/disaster sequence.
    game.create_city(0, 10, 24);
    let factory = game.cities.last().unwrap().id;
    let index = game.city_index_by_id(factory).unwrap();
    game.cities[index].size = 2;
    game.cities[index].specialists = [0; 3];
    game.cities[index].production = Production::Unit(UnitKind::Warrior);
    game.cities[index].shields = Production::Unit(UnitKind::Warrior).cost();
    game.reconcile_city_workers();
    game.process_player_cities(0);
    let built = game
        .units
        .iter()
        .find(|unit| unit.home_city == Some(factory) && unit.kind == UnitKind::Warrior)
        .unwrap()
        .id;
    game.remove_units_with_cargo_because(vec![built], UnitRemoval::Combat);
    game.reconcile_lifecycle_observation();
    let history = game.observe_lifecycle();
    assert!(history.changes.iter().any(|entry| matches!(entry.change, Change::UnitCreated { ref unit, origin: UnitOrigin::Built { city_id } } if unit.id == built && city_id == factory)));
    assert!(history.changes.iter().any(|entry| matches!(entry.change, Change::UnitRemoved { ref unit, reason: UnitRemoval::Combat } if unit.id == built)));
    assert!(game.trace.dropped() > 0);
    let first = game.observe_lifecycle();
    assert!(first.complete_since_start);
    balanced(&first);
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(game.observe_lifecycle()).unwrap()
    );
    game.trace.clear_lifecycle_changes();
    let next = game.observe_lifecycle();
    assert!(next.changes.is_empty());
    assert_eq!(
        first.unit_totals.iter().map(|u| u.built).sum::<u64>(),
        next.unit_totals.iter().map(|u| u.built).sum::<u64>()
    );
    balanced(&next);
}

#[test]
fn filters_do_not_disable_snapshot_history_and_transfers_are_not_deaths() {
    let mut game = game();
    let id = game.spawn_unit(1, UnitKind::Worker, 10, 24);
    game.trace = trace::Collector::enabled(Default::default(), 1);
    game.start_lifecycle_observation();
    let original = game.unit_by_id(id).unwrap().clone();
    let gained = game.transfer_unit(&original, 0, None);
    game.reconcile_lifecycle_observation();
    let history = game.observe_lifecycle();
    assert!(history.complete_since_start);
    assert!(!game.units.iter().any(|u| u.id == id));
    assert!(game.units.iter().any(|u| u.id == gained && u.owner == 0));
    assert_eq!(
        history
            .unit_totals
            .iter()
            .map(|u| u.built + u.created + u.destroyed)
            .sum::<u64>(),
        0
    );
    assert_eq!(
        history
            .unit_totals
            .iter()
            .map(|u| u.transferred_in)
            .sum::<u64>(),
        1
    );
    assert_eq!(
        history
            .unit_totals
            .iter()
            .map(|u| u.transferred_out)
            .sum::<u64>(),
        1
    );
    assert!(game.trace.drain().is_empty());
    balanced(&history);
}

#[test]
fn uninstrumented_fixture_edits_are_reported_as_unknown_not_destroyed() {
    let mut game = game();
    game.spawn_unit(0, UnitKind::Warrior, 10, 24);
    game.start_lifecycle_observation();
    game.units.clear();
    game.create_city(0, 10, 24);
    game.cities[0].owner = 1;
    game.reconcile_lifecycle_observation();
    let history = game.observe_lifecycle();
    assert!(!history.complete_since_start);
    assert_eq!(history.unobserved_changes, 2);
    assert_eq!(
        history.unit_totals.iter().map(|u| u.destroyed).sum::<u64>(),
        0
    );
    assert_eq!(
        history
            .unit_totals
            .iter()
            .map(|u| u.unobserved_removed)
            .sum::<u64>(),
        1
    );
    assert_eq!(
        history.city_totals.iter().map(|c| c.conquered).sum::<u64>(),
        0
    );
    balanced(&history);
}

#[test]
fn observing_a_loaded_campaign_starts_with_a_baseline_not_invented_builds() {
    let mut game = game();
    game.spawn_unit(0, UnitKind::Warrior, 10, 24);
    game.create_city(0, 10, 24);
    game.start_lifecycle_observation();
    let history = game.observe_lifecycle();
    assert!(history.history_available && history.complete_since_start);
    assert_eq!(history.since_turn, Some(game.turn));
    assert!(history.changes.is_empty());
    assert_eq!(history.unit_totals[0].initial, 1);
    assert_eq!(history.unit_totals[0].built, 0);
    assert_eq!(history.city_totals[0].initial, 1);
    let cloned = game.clone();
    assert!(!cloned.observe_lifecycle().history_available);
    balanced(&history);
}
