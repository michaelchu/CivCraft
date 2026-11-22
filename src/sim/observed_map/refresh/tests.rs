use super::*;
use crate::sim::{CIV3_DIRS, HUMAN, Simulation, UnitKind};
use crate::world::Terrain;

#[test]
fn nested_batches_flush_once_and_distinguish_empty_scope_from_full_refresh() {
    let mut refresh = VisibilityRefresh::default();
    let outer = refresh.begin_batch(true);
    let inner = refresh.begin_batch(false);
    let nested = refresh.begin_batch(true);
    refresh.prepare_render_tiles(8);
    for index in [5, 1, 5] {
        refresh.mark_render_tile(index);
    }
    refresh.request_refresh();
    assert!(!refresh.end_batch(nested));
    assert!(!refresh.end_batch(inner));
    assert!(!refresh.start_flush());
    assert!(refresh.end_batch(outer));
    assert!(refresh.start_flush());
    assert_eq!(refresh.take_render_scope(), Some(vec![1, 5]));
    assert_eq!(refresh.take_render_scope(), None);
    refresh.finish_flush();
    assert!(!refresh.start_flush());

    refresh.request_refresh();
    assert!(refresh.start_flush());
    assert_eq!(refresh.take_render_scope(), Some(vec![]));
    refresh.finish_flush();
    refresh.mark_render_tile(3);
    refresh.request_refresh();
    assert!(refresh.start_flush());
    assert_eq!(refresh.take_render_scope(), Some(vec![3]));
    refresh.finish_flush();
}

fn movement_game() -> (Simulation, u32) {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.diplomacy.clear();
    game.observed_map = Default::default();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.hut = false;
        tile.base = None;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
        tile.mine = false;
        tile.irrigation = false;
        tile.known_by.fill(false);
        tile.seen = false;
        tile.visible = false;
        tile.stealth_visible = false;
    }
    let unit = game.spawn_unit(1, UnitKind::Horseman, 10, 24);
    game.sync_list_order();
    game.refresh_visibility();
    (game, unit)
}

#[test]
fn ai_path_waits_for_outer_batch_and_matches_the_normal_path_after_flushing() {
    let (mut game, unit) = movement_game();
    let mut normal = game.clone();
    let direction = 2;
    let step = CIV3_DIRS[direction];
    let mut destination = (10, 24);
    for _ in 0..2 {
        destination = game
            .map_wrap
            .eight_way_neighbor(destination.0, destination.1, direction)
            .unwrap();
    }
    let newly_visible = game
        .map_wrap
        .eight_way_neighbor(destination.0, destination.1, direction)
        .unwrap();
    let revision = game.observed_map.revision;
    let batch = game.visibility_refresh.begin_batch(true);
    assert!(game.follow_grid_path(unit, [step; 2]));
    let actor = game.unit_by_id(unit).unwrap();
    assert_eq!((actor.x, actor.y), destination);
    assert!(!game.tile(newly_visible.0, newly_visible.1).known_by[1]);
    game.flush_deferred_visibility();
    assert_eq!(game.observed_map.revision, revision);
    assert!(game.visibility_refresh.end_batch(batch));
    game.flush_deferred_visibility();
    assert!(game.tile(newly_visible.0, newly_visible.1).known_by[1]);
    assert!(game.observed_map.revision > revision);
    let flushed_revision = game.observed_map.revision;
    game.flush_deferred_visibility();
    assert_eq!(game.observed_map.revision, flushed_revision);

    assert!(normal.follow_grid_path(unit, [step; 2]));
    assert_eq!(
        game.encode_campaign().unwrap(),
        normal.encode_campaign().unwrap()
    );
}

#[test]
fn scoped_path_refresh_is_consumed_before_the_next_full_refresh() {
    let (mut game, unit) = movement_game();
    let far = (30, 8);
    game.spawn_unit(1, UnitKind::Warrior, far.0, far.1);
    game.refresh_visibility();
    assert!(!game.remembered_tile(1, far.0, far.1).unwrap().mine);
    game.tile_mut(far.0, far.1).mine = true;

    assert!(game.follow_grid_path(unit, [(2, 0), (2, 0)]));
    assert!(!game.remembered_tile(1, far.0, far.1).unwrap().mine);
    game.refresh_visibility();
    assert!(game.remembered_tile(1, far.0, far.1).unwrap().mine);
}

#[test]
fn queued_refreshes_clone_independently_and_are_excluded_from_campaign_saves() {
    let (mut game, unit_id) = movement_game();
    game.create_city(HUMAN, 22, 24);
    game.refresh_visibility();
    game.sync_list_order();
    assert!(!game.observed_map.tiles.is_empty());
    let saved = game.encode_campaign().unwrap();
    let remembered = bincode::serialize(&game.observed_map).unwrap();
    let batch = game.visibility_refresh.begin_batch(true);
    let unit = game.unit_by_id(unit_id).unwrap().clone();
    game.record_visibility_render_scope(&unit, (unit.x, unit.y), (unit.x, unit.y));
    game.visibility_refresh.request_refresh();
    assert_eq!(game.encode_campaign().unwrap(), saved);

    let path = std::env::temp_dir().join(format!(
        "civcraft-visibility-refresh-{}.save",
        std::process::id()
    ));
    game.save_campaign(&path).unwrap();
    let mut loaded = Simulation::load_campaign(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!loaded.visibility_refresh.deferred());
    loaded.flush_deferred_visibility();
    assert_eq!(loaded.encode_campaign().unwrap(), saved);
    assert_eq!(
        bincode::serialize(&loaded.observed_map).unwrap(),
        remembered
    );

    let mut clone = game.clone();
    assert!(clone.visibility_refresh.end_batch(batch));
    clone.flush_deferred_visibility();
    assert!(game.visibility_refresh.deferred());
    assert_eq!(bincode::serialize(&game.observed_map).unwrap(), remembered);
    assert!(game.visibility_refresh.end_batch(batch));
    game.flush_deferred_visibility();
    assert_eq!(
        game.encode_campaign().unwrap(),
        clone.encode_campaign().unwrap()
    );
}
