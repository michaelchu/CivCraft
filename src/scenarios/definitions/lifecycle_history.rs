//! Production, loss, founding, capture and transfer remain in lifecycle history.
use super::{Checks, Scenario};
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::{
    Pact, Production, Simulation, UnitKind,
    fixtures::WorldBuilder,
    lifecycle::{Change, CityRemoval, TransferReason, UnitOrigin, UnitRemoval},
};
pub(super) const SCENARIO: Scenario = Scenario::new(
    "lifecycle-history",
    1,
    ScenarioKind::Actions {
        control: Control::AllAi,
        execute,
    },
);
fn execute(game: &mut Simulation, checks: &mut Checks) {
    game.ensure_lifecycle_observation();
    let mut world = WorldBuilder::new(game);
    world.fertile_tiles(None, None);
    let factory = world.founding_city(0, (10, 24));
    world.city_population(factory, 2, Some([0; 3]));
    world.production_stock(
        factory,
        Production::Unit(UnitKind::Warrior),
        Production::Unit(UnitKind::Warrior).cost(),
    );
    game.reconcile_city_workers();
    game.process_player_cities(0);
    let built = game
        .units()
        .iter()
        .find(|u| u.home_city == Some(factory) && u.kind == UnitKind::Warrior)
        .map(|u| u.id);
    if let Some(id) = built {
        game.remove_units_with_cargo_because(vec![id], UnitRemoval::Combat);
    }
    let founder = WorldBuilder::new(game).unit(0, UnitKind::Settler, (26, 40));
    game.found_city(founder);
    let spare = WorldBuilder::new(game).unit(0, UnitKind::Worker, (10, 24));
    game.disband_unit(spare, None);

    let enemy = WorldBuilder::new(game).founding_city(1, (28, 12));
    WorldBuilder::new(game).city_population(enemy, 3, None);
    game.sign_pact(0, 1, Pact::War);
    let attacker = WorldBuilder::new(game).unit(0, UnitKind::Warrior, (28, 12));
    game.capture_city_if_needed(attacker, 28, 12);
    let traded = WorldBuilder::new(game).founding_city(1, (20, 24));
    WorldBuilder::new(game).unit(1, UnitKind::Worker, (20, 24));
    let index = game.city_index_by_id(traded).unwrap();
    game.transfer_traded_city(index, 0);
    game.remove_city_because(enemy, CityRemoval::Disaster, None);
    game.reconcile_lifecycle_observation();
    let history = game.observe_lifecycle();
    let changes = &history.changes;
    checks.extend([
            ("unit_built", changes.iter().any(|e| matches!(&e.change, Change::UnitCreated { unit, origin: UnitOrigin::Built { city_id } } if *city_id == factory && Some(unit.id) == built)), "A real city production completion records its unit ID and producing city".into()),
            ("unit_destroyed", changes.iter().any(|e| matches!(&e.change, Change::UnitRemoved { unit, reason: UnitRemoval::Combat } if Some(unit.id) == built)), "A built unit removed within the same observation interval remains in history".into()),
            ("founder_consumed", changes.iter().any(|e| matches!(&e.change, Change::UnitRemoved { unit, reason: UnitRemoval::FoundedCity } if unit.id == founder)), "Founding consumes its Settler without classifying it as destroyed".into()),
            ("unit_disbanded", changes.iter().any(|e| matches!(&e.change, Change::UnitRemoved { unit, reason: UnitRemoval::Disbanded } if unit.id == spare)), "Voluntary disbanding is distinct from destruction".into()),
            ("city_conquered", changes.iter().any(|e| matches!(&e.change, Change::CityTransferred { city, new_owner: 0, reason: TransferReason::Conquest } if city.id == enemy)), "Conquest records the previous and new owner".into()),
            ("city_traded", changes.iter().any(|e| matches!(&e.change, Change::CityTransferred { city, new_owner: 0, reason: TransferReason::Trade } if city.id == traded)), "A traded city is not classified as conquered".into()),
            ("city_destroyed", changes.iter().any(|e| matches!(&e.change, Change::CityDestroyed { city, reason: CityRemoval::Disaster, .. } if city.id == enemy)), "A conquered city can also be destroyed in the same interval".into()),
            ("history_complete", history.complete_since_start, "All changes reconcile with the live unit and city rosters".into()),
    ]);
}
