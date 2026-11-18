//! Controlled financial-policy checks; ordinary headless turns follow setup.
use super::*;

impl Simulation {
    pub(in crate::sim) fn wartime_funding_scenario(&mut self) -> Vec<(&'static str, bool, String)> {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        self.ai_war_plans.clear();
        self.ai_purchase_reserve.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = true;
            tile.road = true;
            tile.hut = false;
        }
        for (owner, x, y) in [(1, 10, 24), (1, 18, 30), (2, 26, 24)] {
            self.create_city(owner, x, y);
        }
        self.turn += 1; // Rush eligibility requires an established city.
        self.players[1].ai = true;
        self.players[1].civilization_id = Some("korea".into());
        self.players[1].government = Government::Monarchy;
        self.players[1].known = vec![Tech::ALPHABET, Tech::BRONZE_WORKING, Tech::MONARCHY];
        self.players[1].researching = Tech::POTTERY;
        self.players[1].research = 0;
        self.players[1].last_science = 0;
        self.players[1].gold = 5000;
        self.players[1].rates = [60, 40, 0];
        self.reconcile_city_workers();
        self.make_diplomatic_contact(1, 2);
        self.sign_pact(1, 2, Pact::Peace);
        let original = self.clone();
        let before = self.encode_campaign().unwrap();
        let income = self.ai_projected_income_at_rates(1, [60, 40, 0]);
        let read_only = self.encode_campaign().unwrap() == before;
        let mut peace = self.clone();
        peace.manage_ai_taxes(1);
        self.sign_pact(1, 2, Pact::War);
        self.manage_ai_taxes(1);
        let funded_war =
            self.players[1].rates == peace.players[1].rates && self.players[1].rates[0] > 0;
        let mut preparation = original.clone();
        preparation.cities[0].production = Production::Unit(UnitKind::Warrior);
        preparation.cities[0].shields = 1;
        let need = preparation.ai_military_funding_need(1, true);
        let quoted = need.purchase();
        let strategy = preparation.strategic_priorities(1);
        let native = preparation.players[1].rates;
        let rich = preparation.ai_funding_decision(1, native, &strategy, need);
        preparation.players[1].gold = 0;
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
        let home = upkeep.cities[0].id;
        for _ in 0..7 {
            let id = upkeep.spawn_unit(1, UnitKind::Warrior, 10, 24);
            upkeep
                .units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .home_city = Some(home);
        }
        upkeep.players[1].gold = 0;
        upkeep.players[1].rates = [70, 30, 0];
        upkeep.manage_ai_taxes(1);
        let bill_funded = upkeep.ai_projected_income(1) >= 0;
        let reserve = upkeep.strategic_priorities(1).gold_reserve;
        let buffer_shared =
            reserve > upkeep.ai_gold_reserve(1) && upkeep.diplomatic_trade_reserve(1) == reserve;
        let mut emergency = original.clone();
        emergency.sign_pact(1, 2, Pact::War);
        let threat = offset_native_by_map(10, 24, 1, 0);
        for _ in 0..3 {
            emergency.spawn_unit(2, UnitKind::Warrior, threat.0, threat.1);
        }
        emergency.refresh_visibility();
        let urgent = emergency.ai_military_funding_need(1, false);
        let urgent_detected = urgent.urgent_purchase > 0;
        let mut transition = original.clone();
        transition.players[1].government = Government::Feudalism;
        transition.players[1]
            .known
            .extend([Tech::FEUDALISM, Tech::DEMOCRACY, Tech::ENGINEERING]);
        for (x, y) in [(8, 16), (12, 32), (20, 20), (28, 30)] {
            transition.create_city(1, x, y);
        }
        let home = transition.cities[0].id;
        for _ in 0..30 {
            let id = transition.spawn_unit(1, UnitKind::Warrior, 10, 24);
            transition
                .units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .home_city = Some(home);
        }
        let catapult = transition.spawn_unit(1, UnitKind::Catapult, 10, 24);
        transition.players[1].gold = transition.strategic_upkeep_buffer(1) + 1;
        transition.cities[0].production = Production::Unit(UnitKind::Warrior);
        transition.cities[0].shields = 0;
        let upgrade_available = transition
            .paid_upgrade(transition.unit_by_id(catapult).unwrap())
            .is_some();
        let mut choices = std::collections::BTreeMap::new();
        choices.insert(
            home,
            advisors::AiBuildChoice {
                production: transition.cities[0].production,
                want: 0.0,
            },
        );
        transition.spend_ai_defense_gold_with_choices(1, &choices);
        let upgrade_held = upgrade_available
            && transition.unit_by_id(catapult).unwrap().kind == UnitKind::Catapult;
        let unit_count = transition.units.len();
        for government in [Government::Anarchy, Government::Democracy] {
            transition.players[1].government = government;
            let paid = transition.national_unit_gold_upkeep(1).paid;
            transition.players[1].gold -= paid.iter().map(|(_, cost)| cost).sum::<i32>();
            transition.balance_unit_gold_upkeep(1, &paid);
        }
        let transition_funded =
            transition.units.len() == unit_count && transition.players[1].gold >= 0;
        vec![
            (
                "transition_upkeep_buffer",
                transition_funded && upgrade_held,
                "Routine upgrades preserve cash for anarchy and the new government's support bill"
                    .into(),
            ),
            (
                "funded_war_keeps_science",
                funded_war,
                "A rich scientific nation retains its peacetime funding during an ongoing war"
                    .into(),
            ),
            (
                "actual_preparation_purchase",
                purchase_funded,
                "An unfunded concrete purchase raises the cash goal; a funded purchase does not"
                    .into(),
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
        ]
    }
}
