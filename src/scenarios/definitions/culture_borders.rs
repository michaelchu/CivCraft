//! Accumulated city culture claims territory when it crosses a border tier.
use super::{Checks, Scenario};
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::drive::Drive;
use crate::sim::{HUMAN, Simulation, fixtures::WorldBuilder};

pub(super) const SCENARIO: Scenario = Scenario::new(
    "culture-borders",
    2,
    ScenarioKind::Actions {
        control: Control::Human,
        execute,
    },
);

fn execute(game: &mut Simulation, checks: &mut Checks) {
    let mut world = WorldBuilder::new(game);
    world.clear_units_and_cities();
    world.fertile_tiles(None, Some(HUMAN));
    let city_id = world.founding_city(HUMAN, (10, 24));
    world.city_history(city_id, 99);

    let origin = (10, 24);
    let rank_distances = game.map_wrap.civ3_rank_distances_from(origin.0, origin.1);
    let target_index = rank_distances
        .iter()
        .enumerate()
        .find_map(|(index, &distance)| {
            let (x, y) = game.map_wrap.position(index);
            (distance == 4 && !game.tile(x, y).terrain.is_water()).then_some(index)
        })
        .unwrap();
    let target = game.map_wrap.position(target_index);
    let unclaimed_before = game.tile(target.0, target.1).owner.is_none();
    game.process_player_cities(HUMAN);

    let city = game.city_by_id(city_id).unwrap();
    let fat_x_footprint = (0..game.map_wrap.tile_count()).all(|index| {
        let distance = rank_distances[index];
        let inside_radius = i64::from(distance) * i64::from(distance) <= 16;
        let (x, y) = game.map_wrap.position(index);
        (game.tile(x, y).owner == Some(HUMAN)) == inside_radius
    });
    checks.extend([
        (
            "culture_reaches_second_border_tier",
            game.city_culture_score(city) == 100,
            "The Palace's culture output carries the city across the 100 point tier".into(),
        ),
        (
            "culture_expands_claimed_territory",
            unclaimed_before && game.tile(target.0, target.1).owner == Some(HUMAN),
            "A tile four steps away is claimed when the city crosses the 100 point culture tier"
                .into(),
        ),
        (
            "culture_border_uses_fat_x_footprint",
            fat_x_footprint,
            "The 100 point tier claims exactly the rank-four fat-X region outside the city-working radius"
                .into(),
        ),
    ]);
}
