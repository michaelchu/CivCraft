//! Settlement advice reprices commitments while retaining cached site values.
use super::{Checks, Scenario};
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::drive::Drive;
use crate::sim::{
    AiBuildChoice, Government, Production, Simulation, UnitKind, fixtures::WorldBuilder,
};
pub(super) const SCENARIO: Scenario = Scenario::new(
    "expansion-balance",
    1,
    ScenarioKind::Actions {
        control: Control::AllAi,
        execute,
    },
);
fn execute(game: &mut Simulation, checks: &mut Checks) {
    let mut world = WorldBuilder::new(game);
    world.clear_units_and_cities();
    world.fertile_tiles(Some(1), Some(1));
    world.government_state(1, Government::Despotism);
    let city_id = world.founding_city(1, (10, 24));
    world.city_population(city_id, 4, Some([0; 3]));
    game.reconcile_city_workers();
    let city = game.cities()[0].clone();
    let early = game.cached_settlement_choice(&city, None);
    let cached = game.cities()[0].settlement_advice_cache();
    for _ in 0..30 {
        WorldBuilder::new(game).unit(1, UnitKind::Settler, (city.x, city.y));
    }
    let backlog = game.cached_settlement_choice(&city, None);
    let fresh_pressure = backlog.is_none() && game.cities()[0].settlement_advice_cache() == cached;
    WorldBuilder::new(game).clear_units();
    let recovered = game.cached_settlement_choice(&city, None);
    let recovered = recovered.zip(early).is_some_and(|(a, b)| a.want == b.want);
    let quote = |want| AiBuildChoice {
        production: Production::Unit(UnitKind::Settler),
        want,
    };
    checks.extend([
            ("early_expansion", early.is_some(), "A young empire still wants a viable new city".into()),
            ("pending_founders_reprice_cached_advice", fresh_pressure, "An existing founder backlog suppresses new production without waiting for the site-cache timer".into()),
            ("expansion_recovers_when_backlog_clears", recovered, "Clearing commitments restores the cached site's original adjusted want".into()),
        ]);
    WorldBuilder::new(game).founding_city(1, (20, 24));
    for _ in 0..2 {
        WorldBuilder::new(game).unit(1, UnitKind::Settler, (city.x, city.y));
    }
    let without_queue = game
        .price_settlement_expansion(&city, quote(400.0))
        .map(|c| c.want);
    let queued_city = game.cities()[1].id;
    let stock = game.cities()[1].shields;
    WorldBuilder::new(game).production_stock(
        queued_city,
        Production::Unit(UnitKind::Settler),
        stock,
    );
    let with_queue = game
        .price_settlement_expansion(&city, quote(400.0))
        .map(|c| c.want);
    checks.push((
        "queued_founders_have_a_cost",
        with_queue < without_queue,
        "Other cities' existing founder production counts as committed expansion".into(),
    ));
    for i in 2..20 {
        WorldBuilder::new(game).founding_city(1, (4 + (i % 5) * 6, 4 + (i / 5) * 6));
    }
    let ordinary = game.price_settlement_expansion(&city, quote(400.0));
    let exceptional = game.price_settlement_expansion(&city, quote(10_000.0));
    checks.push((
        "large_empire_prices_marginal_expansion",
        ordinary.is_none(),
        "A crowded empire rejects an ordinary site after administration and space costs".into(),
    ));
    checks.push((
        "valuable_sites_are_not_hard_capped",
        exceptional.is_some(),
        "A sufficiently valuable opportunity can justify expansion beyond twenty cities".into(),
    ));
}
