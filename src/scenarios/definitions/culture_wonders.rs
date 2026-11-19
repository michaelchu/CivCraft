//! Wonder culture doubles with age and follows its original builder on capture.
use super::{Checks, Scenario};
use crate::rules::Building;
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::drive::Drive;
use crate::sim::{HUMAN, Pact, Simulation, UnitKind, fixtures::WorldBuilder};

const ORIGINAL: usize = 1;
const TARGET: (i32, i32) = (20, 24);

pub(super) const SCENARIO: Scenario = Scenario::new(
    "culture-wonder-capture",
    1,
    ScenarioKind::Actions {
        control: Control::Human,
        execute,
    },
);

fn execute(game: &mut Simulation, checks: &mut Checks) {
    let mut world = WorldBuilder::new(game);
    world.clear_units_and_cities();
    world.fertile_tiles(None, Some(HUMAN));
    world.start_at(1, 0);
    world.founding_city(ORIGINAL, (8, 24));
    let city_id = world.founding_city(ORIGINAL, TARGET);
    world.founding_city(HUMAN, (30, 24));
    world.city_population(city_id, 3, None);
    world.city_history(city_id, 500);
    world.culture_building(city_id, Building::SISTINE_CHAPEL);

    world.start_at(2, 999);
    let base_rate = game.city_culture_per_turn(game.city_by_id(city_id).unwrap());
    WorldBuilder::new(game).start_at(3, 1_000);
    let aged_rate = game.city_culture_per_turn(game.city_by_id(city_id).unwrap());

    game.sign_pact(HUMAN, ORIGINAL, Pact::War);
    let attacker = WorldBuilder::new(game).unit(HUMAN, UnitKind::Warrior, TARGET);
    game.capture_city_if_needed(attacker, TARGET.0, TARGET.1);
    let captured_city = game.city_by_id(city_id).unwrap();
    let captured_without_output = captured_city.owner == HUMAN
        && game.city_history(city_id) == Some(0)
        && game.city_culture_per_turn(captured_city) == 0
        && game.civilization_culture(ORIGINAL) >= 500;

    WorldBuilder::new(game).clear_units();
    let recapturer = WorldBuilder::new(game).unit(ORIGINAL, UnitKind::Warrior, TARGET);
    game.capture_city_if_needed(recapturer, TARGET.0, TARGET.1);
    let recaptured_city = game.city_by_id(city_id).unwrap();
    let original_builder_resumes =
        recaptured_city.owner == ORIGINAL && game.city_culture_per_turn(recaptured_city) == 4;

    checks.extend([
        (
            "wonder_uses_audited_culture_rate",
            base_rate == 4,
            "The newly built Sistine Chapel produces four culture points per turn".into(),
        ),
        (
            "wonder_culture_doubles_after_1000_years",
            aged_rate == 8,
            "The wonder's culture output doubles after 1,000 campaign years".into(),
        ),
        (
            "captured_wonder_stops_for_occupier",
            captured_without_output,
            "A conquered city starts fresh and its captured cultural buildings do not produce for the occupier".into(),
        ),
        (
            "original_builder_resumes_at_base_rate_on_recapture",
            original_builder_resumes,
            "The original builder resumes the wonder's base output on recapture, with its age bonus reset".into(),
        ),
    ]);
}
