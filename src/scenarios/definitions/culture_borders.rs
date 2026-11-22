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
    let mut path = vec![origin];
    for _ in 0..3 {
        let previous = *path.last().unwrap();
        path.push(
            game.map_wrap
                .eight_way_neighbor(previous.0, previous.1, 3)
                .unwrap(),
        );
    }
    // Three steps along a map axis: outside the 21-tile fat X (radius 2) but
    // inside the next expansion (radius 3).
    let target = path[3];
    let unclaimed_before = game.tile(target.0, target.1).owner.is_none();
    game.process_player_cities(HUMAN);

    let city = game.city_by_id(city_id).unwrap();
    checks.extend([
        (
            "culture_reaches_second_border_tier",
            game.city_culture_score(city) == 100,
            "The Palace's culture output carries the city across the 100 point tier".into(),
        ),
        (
            "culture_expands_claimed_territory",
            unclaimed_before && game.tile(target.0, target.1).owner == Some(HUMAN),
            "A tile three Civ III map steps away along a map axis is claimed when the culture border tier expands"
                .into(),
        ),
    ]);
}
