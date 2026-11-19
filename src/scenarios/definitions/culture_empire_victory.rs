//! Distributed city culture can win through the normal empire-victory check.
use super::{Checks, Scenario};
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::{HUMAN, Simulation, fixtures::WorldBuilder};

const LEADER: usize = 1;
const CITY_CULTURE: i32 = 18_000;

pub(super) const SCENARIO: Scenario = Scenario::new(
    "culture-empire-victory",
    1,
    ScenarioKind::Actions {
        control: Control::AllAi,
        execute,
    },
);

fn execute(game: &mut Simulation, checks: &mut Checks) {
    let width = game.map_wrap.width();
    let height = game.map_wrap.height();
    let mut world = WorldBuilder::new(game);
    world.clear_units_and_cities();
    world.fertile_tiles(None, None);
    let columns = [width / 8, width * 3 / 8, width * 5 / 8, width * 7 / 8];
    let rows = [height / 8, height * 3 / 8, height * 5 / 8];
    let locations = rows.into_iter().flat_map(|y| columns.map(|x| (x, y)));
    for location in locations {
        let city_id = world.founding_city(LEADER, location);
        world.city_history(city_id, CITY_CULTURE);
    }
    for location in [(width / 4, height * 7 / 8), (width * 3 / 4, height * 7 / 8)] {
        let city_id = world.founding_city(HUMAN, location);
        world.city_history(city_id, 9_000);
    }

    let map_area = i64::from(game.map_wrap.width()) * i64::from(game.map_wrap.height());
    let target = 100_000_i64 * map_area
        / (i64::from(crate::world::topology::MAP_W) * i64::from(crate::world::topology::MAP_H));
    let leader_before = game.civilization_culture(LEADER);
    let runner_up_before = game.civilization_culture(HUMAN);
    game.simulate_turn();
    let winner = game.game_over().is_some_and(|result| {
        result.contains("wins a cultural victory through cultural dominance")
    });

    checks.extend([
        (
            "empire_culture_meets_map_scaled_target",
            leader_before >= target && leader_before >= runner_up_before * 2,
            format!("The leader begins above the map-scaled target of {target} culture and twice the runner-up"),
        ),
        (
            "distributed_culture_avoids_legendary_city",
            game.cities_owned_by(LEADER)
                .all(|city| game.city_culture_score(city) < 20_000),
            "No individual city reaches the legendary-city threshold".into(),
        ),
        (
            "empire_culture_wins_at_turn_boundary",
            winner,
            "The leading empire wins at the ordinary campaign turn boundary with at least twice its rival's culture".into(),
        ),
    ]);
}
