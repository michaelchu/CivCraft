//! Border growth, nearby-city competition, and conquest snapshots.
use super::{Checks, Scenario};
use crate::rules::Building;
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::drive::Drive;
use crate::sim::{HUMAN, Pact, Simulation, UnitKind, fixtures::WorldBuilder};

const OTHER: usize = 1;
const GROWTH_ORIGIN: (i32, i32) = (12, 24);
const GROWTH_THRESHOLDS: [i32; 4] = [10, 100, 1_000, 10_000];

pub(super) const GROWTH: Scenario = Scenario::new(
    "culture-border-growth",
    1,
    ScenarioKind::Actions {
        control: Control::Human,
        execute: grow_by_tier,
    },
);

pub(super) const NEARBY_CITY: Scenario = Scenario::new(
    "culture-border-nearby-city",
    1,
    ScenarioKind::Actions {
        control: Control::Human,
        execute: contest_with_nearby_city,
    },
);

pub(super) const MULTI_CITY: Scenario = Scenario::new(
    "culture-border-multi-city",
    1,
    ScenarioKind::Actions {
        control: Control::Human,
        execute: compete_with_multiple_cities,
    },
);

pub(super) const CONQUEST: Scenario = Scenario::new(
    "culture-border-conquest",
    1,
    ScenarioKind::Actions {
        control: Control::Human,
        execute: capture_culture_city,
    },
);

fn grow_by_tier(game: &mut Simulation, checks: &mut Checks) {
    let mut world = WorldBuilder::new(game);
    world.clear_units_and_cities();
    world.fertile_tiles(None, Some(HUMAN));
    let city_id = world.founding_city(HUMAN, GROWTH_ORIGIN);
    world.finish();

    let mut rows = Vec::new();
    let mut all_tiers_claimed = true;
    for (tier, threshold) in GROWTH_THRESHOLDS.into_iter().enumerate() {
        // Palace culture adds one point during ordinary city processing. Set
        // the fixture just below each milestone to exercise the real trigger.
        WorldBuilder::new(game).city_history(city_id, threshold - 1);
        game.process_player_cities(HUMAN);
        let rank = 3 + tier as i32;
        let rank_distances = game
            .map_wrap
            .civ3_rank_distances_from(GROWTH_ORIGIN.0, GROWTH_ORIGIN.1);
        let target_index = rank_distances
            .iter()
            .enumerate()
            .find_map(|(index, &distance)| {
                let (x, y) = game.map_wrap.position(index);
                (distance == rank && !game.tile(x, y).terrain.is_water()).then_some(index)
            })
            .unwrap();
        let target = game.map_wrap.position(target_index);
        let claimed = game.tile(target.0, target.1).owner == Some(HUMAN);
        all_tiers_claimed &= claimed;
        let city = game.city_by_id(city_id).unwrap();
        rows.push(format!(
            "culture={} tier={} rank{} at {:?}={}\n{}",
            game.city_culture_score(city),
            tier + 1,
            rank,
            target,
            if claimed { "claimed" } else { "open" },
            local_map(game, GROWTH_ORIGIN, 7),
        ));
    }
    checks.push((
        "culture_border_growth_reaches_each_tier",
        all_tiers_claimed,
        rows.join("\n---\n"),
    ));
}

fn contest_with_nearby_city(game: &mut Simulation, checks: &mut Checks) {
    let mut world = WorldBuilder::new(game);
    world.clear_units_and_cities();
    world.fertile_tiles(None, None);
    let human_city = world.founding_city(HUMAN, (12, 24));
    // Offset the rival diagonally so the scenario exercises a sloped shared
    // frontier instead of the easy horizontal bisector case.
    let rival_city = world.founding_city(OTHER, (16, 27));
    world.city_history(human_city, 999);
    world.city_history(rival_city, 99);
    world.finish();
    game.process_player_cities(HUMAN);
    game.process_player_cities(OTHER);

    let (human_count, rival_count) =
        (0..game.map_wrap.tile_count()).fold((0, 0), |(human, rival), index| {
            let (x, y) = game.map_wrap.position(index);
            match game.tile(x, y).owner {
                Some(HUMAN) => (human + 1, rival),
                Some(OTHER) => (human, rival + 1),
                _ => (human, rival),
            }
        });
    let shared_front = (14, 26);
    let front_owner = game.tile(shared_front.0, shared_front.1).owner;
    let neutral_tiles_near_front = (13..=15)
        .filter(|&x| game.tile(x, 26).owner.is_none())
        .count();
    let map = local_map(game, shared_front, 7);
    let frontier_has_single_transition_per_row = (20..=32).all(|y| {
        let mut previous = None;
        let mut transitions = 0;
        for x in 12..=16 {
            if let Some(owner) = game
                .tile(x, y)
                .owner
                .filter(|owner| *owner == HUMAN || *owner == OTHER)
            {
                if previous.is_some_and(|previous| previous != owner) {
                    transitions += 1;
                }
                previous = Some(owner);
            }
        }
        transitions <= 1
    });
    checks.extend([
        (
            "closer_cities_compete_across_narrower_frontier",
            human_count > 0
                && rival_count > 0
                && game.tile(12, 24).owner == Some(HUMAN)
                && game.tile(16, 27).owner == Some(OTHER)
                && neutral_tiles_near_front < 3,
            format!(
                "cities=H@12,24(1000 culture),R@16,27(100 culture); frontier@14,26={front_owner:?}; neutral tiles near frontier={neutral_tiles_near_front}; owned tiles H={human_count} R={rival_count}"
            ),
        ),
        (
            "cultural_frontier_has_no_row_zigzags",
            frontier_has_single_transition_per_row,
            map,
        ),
    ]);
}

fn compete_with_multiple_cities(game: &mut Simulation, checks: &mut Checks) {
    let mut world = WorldBuilder::new(game);
    world.clear_units_and_cities();
    world.fertile_tiles(None, None);
    let human_north = world.founding_city(HUMAN, (10, 20));
    let human_south = world.founding_city(HUMAN, (10, 28));
    let rival_north = world.founding_city(OTHER, (16, 20));
    let rival_south = world.founding_city(OTHER, (16, 28));

    // Give each city culture so both same-civilization and rival claims overlap.
    for (city, history) in [
        (human_north, 999),
        (human_south, 98),
        (rival_north, 98),
        (rival_south, 999),
    ] {
        world.city_history(city, history);
        world.culture_building(city, Building::TEMPLE);
    }
    world.finish();
    game.process_player_cities(HUMAN);
    game.process_player_cities(OTHER);

    let city_ids = [human_north, human_south, rival_north, rival_south];
    let mut source_counts = [0_usize; 4];
    let mut owner_counts = [0_usize; 2];
    for index in 0..game.map_wrap.tile_count() {
        let (x, y) = game.map_wrap.position(index);
        let tile = game.tile(x, y);
        match tile.owner {
            Some(HUMAN) => owner_counts[0] += 1,
            Some(OTHER) => owner_counts[1] += 1,
            _ => {}
        }
        if let Some(city_id) = tile.border_city
            && let Some(source_index) = city_ids.iter().position(|&id| id == city_id)
        {
            source_counts[source_index] += 1;
        }
    }
    let city_centers_kept = [
        ((10, 20), HUMAN),
        ((10, 28), HUMAN),
        ((16, 20), OTHER),
        ((16, 28), OTHER),
    ]
    .into_iter()
    .all(|(position, owner)| game.tile(position.0, position.1).owner == Some(owner));
    let map = local_claim_map(game, (13, 24), 8, city_ids);
    let city_culture = city_ids
        .iter()
        .map(|&id| {
            game.city_by_id(id)
                .map(|city| game.city_culture_score(city))
        })
        .collect::<Vec<_>>();
    checks.extend([
        (
            "all_cities_keep_distinct_claim_sources",
            city_centers_kept && source_counts.iter().all(|&count| count > 1),
            format!(
                "A=H north, B=H south, C=R north, D=R south; city_ids={city_ids:?}; cultures={city_culture:?}; source_tile_counts={source_counts:?}; owner_tile_counts={owner_counts:?}\n{map}"
            ),
        ),
        (
            "multi_city_frontier_has_no_row_zigzags",
            one_civilization_transition_per_row(game, 18..=30, 9..=17),
            format!(
                "Each map row crosses the H/R frontier at most once; H tiles={}, R tiles={}",
                owner_counts[0], owner_counts[1]
            ),
        ),
    ]);
}

fn one_civilization_transition_per_row(
    game: &Simulation,
    rows: std::ops::RangeInclusive<i32>,
    columns: std::ops::RangeInclusive<i32>,
) -> bool {
    rows.into_iter().all(|y| {
        let mut previous = None;
        let mut transitions = 0;
        for x in columns.clone() {
            if let Some(owner) = game
                .tile(x, y)
                .owner
                .filter(|owner| *owner == HUMAN || *owner == OTHER)
            {
                if previous.is_some_and(|previous| previous != owner) {
                    transitions += 1;
                }
                previous = Some(owner);
            }
        }
        transitions <= 1
    })
}

fn local_claim_map(
    game: &Simulation,
    center: (i32, i32),
    radius: i32,
    city_ids: [u32; 4],
) -> String {
    let symbols = ['A', 'B', 'C', 'D'];
    let mut output = String::new();
    for y in center.1 - radius..=center.1 + radius {
        for x in center.0 - radius..=center.0 + radius {
            let tile = game.tile(x, y);
            output.push(match tile.border_city {
                Some(city_id) => city_ids
                    .iter()
                    .position(|&id| id == city_id)
                    .map_or('?', |index| symbols[index]),
                None if !tile.terrain.is_water() => '.',
                None => '~',
            });
        }
        if y != center.1 + radius {
            output.push('\n');
        }
    }
    output
}

fn capture_culture_city(game: &mut Simulation, checks: &mut Checks) {
    let positions: Vec<_> = (0..game.map_wrap.tile_count())
        .map(|index| game.map_wrap.position(index))
        .collect();
    let mut world = WorldBuilder::new(game);
    world.clear_units_and_cities();
    world.fertile_tiles(None, None);
    for position in positions {
        world.mark_known(HUMAN, position);
        world.mark_known(OTHER, position);
    }
    world.founding_city(HUMAN, (10, 24));
    let target_city = world.founding_city(OTHER, (20, 24));
    world.city_population(target_city, 3, None);
    world.city_history(target_city, 9_999);
    world.finish();
    game.process_player_cities(OTHER);
    let remote_tile = (21, 24);
    let before_owner = game.tile(remote_tile.0, remote_tile.1).owner;

    game.sign_pact(HUMAN, OTHER, Pact::War);
    let attacker = WorldBuilder::new(game).unit(HUMAN, UnitKind::Warrior, (20, 24));
    game.capture_city_if_needed(attacker, 20, 24);

    let captured = game.city_by_id(target_city).unwrap();
    let after_owner = game.tile(remote_tile.0, remote_tile.1).owner;
    let transferred = captured.owner == HUMAN && game.city_history(target_city) == Some(0);
    let remote_claim_released = before_owner == Some(OTHER) && after_owner != Some(OTHER);
    checks.push((
        "conquest_transfers_city_and_rebuilds_borders",
        transferred && remote_claim_released,
        format!(
            "target city owner={}; local culture={}; border tile@21,24 owner before={before_owner:?}, after={after_owner:?}\n{}",
            captured.owner,
            game.city_culture_score(captured),
            local_map(game, (20, 24), 8),
        ),
    ));
}

fn local_map(game: &Simulation, center: (i32, i32), radius: i32) -> String {
    let mut output = String::new();
    for y in center.1 - radius..=center.1 + radius {
        for x in center.0 - radius..=center.0 + radius {
            let tile = game.tile(x, y);
            output.push(match tile.owner {
                Some(HUMAN) => 'H',
                Some(OTHER) => 'R',
                Some(_) => '?',
                None if !tile.terrain.is_water() => '.',
                None => '~',
            });
        }
        if y != center.1 + radius {
            output.push('\n');
        }
    }
    output
}
