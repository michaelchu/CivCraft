//! Concrete military purchase needs, upkeep buffers and protected research.
use super::{Checks, Scenario};
use crate::{
    rules::Tech,
    sim::{
        AiBuildChoice, Government, Pact, Production, Simulation, UnitKind, fixtures::WorldBuilder,
    },
    world::topology::offset_native_by_map,
};
pub(super) const SCENARIO: Scenario = Scenario::new("wartime-funding", execute);

fn execute(game: &mut Simulation, checks: &mut Checks) {
    WorldBuilder::new(game).funding_campaign();
    let home = game.cities()[0].id;
    let original = game.clone();
    let before = game.encode_campaign().unwrap();
    let income = game.ai_projected_income_at_rates(1, [60, 40, 0]);
    let read_only = game.encode_campaign().unwrap() == before;
    let mut peace = game.clone();
    peace.manage_ai_taxes(1);
    game.sign_pact(1, 2, Pact::War);
    game.manage_ai_taxes(1);
    let funded_war =
        game.players()[1].rates == peace.players()[1].rates && game.players()[1].rates[0] > 0;
    let mut preparation = original.clone();
    WorldBuilder::new(&mut preparation).production_stock(
        home,
        Production::Unit(UnitKind::Warrior),
        1,
    );
    let need = preparation.ai_military_funding_need(1, true);
    let quoted = need.purchase();
    let strategy = preparation.strategic_priorities(1);
    let native = preparation.players()[1].rates;
    let rich = preparation.ai_funding_decision(1, native, &strategy, need);
    WorldBuilder::new(&mut preparation).treasury(1, 0);
    let strategy = preparation.strategic_priorities(1);
    let poor = preparation.ai_funding_decision(
        1,
        native,
        &strategy,
        preparation.ai_military_funding_need(1, true),
    );
    let purchase_funded = quoted > 0
        && rich.rates == native
        && poor.required_income > 0
        && poor.rates[1] >= rich.rates[1];
    let mut upkeep = original.clone();
    let home = upkeep.cities()[0].id;
    for _ in 0..7 {
        let mut world = WorldBuilder::new(&mut upkeep);
        let id = world.unit(1, UnitKind::Warrior, (10, 24));
        world.unit_home_city(id, home);
    }
    WorldBuilder::new(&mut upkeep).treasury(1, 0);
    WorldBuilder::new(&mut upkeep).tax_rates(1, [70, 30, 0]);
    upkeep.manage_ai_taxes(1);
    let bill_funded = upkeep.ai_projected_income(1) >= 0;
    let reserve = upkeep.strategic_priorities(1).gold_reserve;
    let buffer_shared =
        reserve > upkeep.ai_gold_reserve(1) && upkeep.diplomatic_trade_reserve(1) == reserve;
    let mut emergency = original.clone();
    emergency.sign_pact(1, 2, Pact::War);
    let threat = offset_native_by_map(10, 24, 1, 0);
    for _ in 0..3 {
        WorldBuilder::new(&mut emergency).unit(2, UnitKind::Warrior, threat);
    }
    WorldBuilder::new(&mut emergency).refresh_visibility();
    let urgent = emergency.ai_military_funding_need(1, false);
    let urgent_detected = urgent.urgent_purchase > 0;
    let mut transition = original.clone();
    WorldBuilder::new(&mut transition).government_state(1, Government::Feudalism);
    WorldBuilder::new(&mut transition).known_technologies(
        1,
        &[
            Tech::ALPHABET,
            Tech::BRONZE_WORKING,
            Tech::MONARCHY,
            Tech::FEUDALISM,
            Tech::DEMOCRACY,
            Tech::ENGINEERING,
        ],
    );
    for position in [(8, 16), (12, 32), (20, 20), (28, 30)] {
        WorldBuilder::new(&mut transition).founding_city(1, position);
    }
    let home = transition.cities()[0].id;
    for _ in 0..30 {
        let mut world = WorldBuilder::new(&mut transition);
        let id = world.unit(1, UnitKind::Warrior, (10, 24));
        world.unit_home_city(id, home);
    }
    let catapult = WorldBuilder::new(&mut transition).unit(1, UnitKind::Catapult, (10, 24));
    let buffer = transition.strategic_upkeep_buffer(1) + 1;
    WorldBuilder::new(&mut transition).treasury(1, buffer);
    WorldBuilder::new(&mut transition).production_stock(
        home,
        Production::Unit(UnitKind::Warrior),
        0,
    );
    let upgrade_available = transition
        .paid_upgrade(transition.unit_by_id(catapult).unwrap())
        .is_some();
    let mut choices = std::collections::BTreeMap::new();
    choices.insert(
        home,
        AiBuildChoice {
            production: transition.cities()[0].production,
            want: 0.0,
        },
    );
    transition.spend_ai_defense_gold_with_choices(1, &choices);
    let upgrade_held =
        upgrade_available && transition.unit_by_id(catapult).unwrap().kind == UnitKind::Catapult;
    let unit_count = transition.units().len();
    for government in [Government::Anarchy, Government::Democracy] {
        WorldBuilder::new(&mut transition).government_state(1, government);
        let paid = transition.national_unit_gold_upkeep(1).paid;
        let gold = transition.players()[1].gold - paid.iter().map(|(_, cost)| cost).sum::<i32>();
        WorldBuilder::new(&mut transition).treasury(1, gold);
        transition.balance_unit_gold_upkeep(1, &paid);
    }
    let transition_funded =
        transition.units().len() == unit_count && transition.players()[1].gold >= 0;
    checks.extend([
        (
            "transition_upkeep_buffer",
            transition_funded && upgrade_held,
            "Routine upgrades preserve cash for anarchy and the new government's support bill"
                .into(),
        ),
        (
            "funded_war_keeps_science",
            funded_war,
            "A rich scientific nation retains its peacetime funding during an ongoing war".into(),
        ),
        (
            "actual_preparation_purchase",
            purchase_funded,
            "An unfunded concrete purchase raises the cash goal; a funded purchase does not".into(),
        ),
        (
            "upkeep_is_funded",
            bill_funded,
            "Proposed rates cover real unit upkeep when the treasury is empty".into(),
        ),
        (
            "shared_upkeep_buffer",
            buffer_shared,
            "Taxes and trade protect the same upkeep cushion".into(),
        ),
        (
            "urgent_defense_purchase",
            urgent_detected,
            "Nearby enemies create a concrete defense funding need without a preparation flag"
                .into(),
        ),
        (
            "read_only_forecast",
            read_only && income == original.ai_projected_income(1),
            "Budget forecasts preserve state and match current-rate accounting".into(),
        ),
    ]);
}
