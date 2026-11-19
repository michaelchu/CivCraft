//! AI Leader eligibility, consumption and wonder completion.
use super::{Checks, Scenario};
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::drive::Drive;
use crate::{
    rules::{Building, Tech},
    sim::{Production, Simulation, UnitKind, fixtures::WorldBuilder},
};
pub(super) const SCENARIO: Scenario = Scenario::new(
    "great-leader",
    1,
    ScenarioKind::Actions {
        control: Control::AllAi,
        execute,
    },
);

fn execute(game: &mut Simulation, checks: &mut Checks) {
    let mut world = WorldBuilder::new(game);
    world.grassland_campaign(false);
    world.ai_control(1, true);
    world.known_technologies(1, &Tech::all().collect::<Vec<_>>());
    let city = world.founding_city(1, (10, 24));
    let unbuildable = !game.can_produce(
        game.city_by_id(city).unwrap(),
        Production::Unit(UnitKind::Leader),
    );
    let mut world = WorldBuilder::new(game);
    world.production_stock(city, Production::Building(Building::PYRAMIDS), 0);
    let leader = world.unit(1, UnitKind::Leader, (10, 24));
    game.run_ai_great_leader(&game.unit_by_id(leader).unwrap().clone());
    let consumed = game.unit_by_id(leader).is_none();
    game.process_player_cities(1);
    let completes = game
        .city_by_id(city)
        .is_some_and(|city| city.has_building(Building::PYRAMIDS));
    checks.extend([
        (
            "leader_unbuildable",
            unbuildable,
            "No city can build a Leader even with every technology".into(),
        ),
        (
            "leader_completes_wonder",
            completes,
            "An AI Leader completes its wonder city's wonder in one turn".into(),
        ),
        (
            "leader_consumed",
            consumed,
            "Completing production consumes the Leader".into(),
        ),
    ]);
}
