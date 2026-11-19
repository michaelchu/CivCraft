//! Production and transport phases build a ferry, then a founder, then settle.
use super::{Checks, Scenario};
use crate::{
    rules::Tech,
    sim::{Production, Simulation, UnitKind, fixtures::WorldBuilder},
};
pub(super) const SCENARIO: Scenario = Scenario::all_ai("settlement-ferry", execute);
fn execute(game: &mut Simulation, checks: &mut Checks) {
    let (city, island) = WorldBuilder::new(game).settlement_ferry_coast();
    let before = game.choose_ai_production(1, city);
    WorldBuilder::new(game).add_technology(1, Tech::MAP_MAKING);
    let production = game.choose_ai_production(1, city);
    let boat = Production::Unit(UnitKind::Galley);
    let queued = before != boat && production == boat;
    let mut sequenced = false;
    let mut founded = false;
    if queued {
        WorldBuilder::new(game).production_stock(city, production, production.cost());
        game.process_cities();
        let settler = Production::Unit(UnitKind::Settler);
        sequenced = game
            .units()
            .iter()
            .any(|unit| unit.kind == UnitKind::Galley)
            && game.cities()[0].production == settler;
        if sequenced {
            WorldBuilder::new(game).production_stock(city, settler, settler.cost());
            game.process_cities();
            for _ in 0..8 {
                game.update_activities_and_restore();
                game.run_ai_player(1);
                if game.cities().len() > 1 {
                    break;
                }
            }
            founded = game.cities().iter().any(|city| {
                city.owner == 1
                    && game
                        .map_wrap
                        .civ3_distance(city.x, city.y, island.0, island.1)
                        <= 2
            });
        }
    }
    checks.extend([
        (
            "ferry_queued",
            queued,
            "AI selected a ferry for an overseas settlement plan".into(),
        ),
        (
            "settler_sequenced",
            sequenced,
            "AI sequenced a Settler after completing the ferry".into(),
        ),
        (
            "island_founded",
            founded,
            "AI transported its founder and established the island city".into(),
        ),
    ]);
}
