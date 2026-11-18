//! Controlled integration fixture for proactive economic diplomacy.
use super::*;

impl Simulation {
    pub(in crate::sim) fn proactive_trade_scenario(&mut self) -> Vec<(&'static str, bool, String)> {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        self.ai_war_plans.clear();
        for player in &mut self.players {
            player.ai = false;
        }
        let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
        for (owner, x, y) in [(1, 10, 24), (2, 18, 30)] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
            self.players[owner].ai = true;
            self.players[owner].known = vec![Tech::ALPHABET];
            self.players[owner].researching = Tech::POTTERY;
            self.players[owner].research_goal = Tech::UNSET;
            self.players[owner].last_science = 20;
            self.players[owner].research = 0;
        }
        self.players[2].known.push(writing);
        self.players[2].gold = 0;
        self.make_diplomatic_contact(1, 2);
        self.sign_pact(1, 2, Pact::Alliance);
        self.diplomacy.get_mut(&(1, 2)).unwrap().trust = [6000; 2];
        self.players[1].civilization_id = Some("rome".into());
        let ordinary = self
            .diplomatic_technology_value(1, 2, writing, false)
            .unwrap();
        self.players[1].civilization_id = Some("korea".into());
        let scientific = self
            .diplomatic_technology_value(1, 2, writing, false)
            .unwrap();
        self.players[1].research_goal = writing;
        let goal = self
            .diplomatic_technology_value(1, 2, writing, false)
            .unwrap();
        self.players[1].gold = self.diplomatic_trade_reserve(1);
        let reserve_protected = self.choose_technology_trade(1, 2).selected.is_none();
        self.players[1].gold = 5000;
        let mut risk = self.clone();
        risk.players[1].known.push(Tech::BRONZE_WORKING);
        risk.players[2]
            .known
            .extend([Tech::BRONZE_WORKING, Tech::IRON_WORKING]);
        risk.diplomacy.get_mut(&(1, 2)).unwrap().pact = Pact::Peace;
        risk.diplomacy.get_mut(&(1, 2)).unwrap().trust[1] = 0;
        let security = risk.diplomatic_technology_value(2, 1, Tech::IRON_WORKING, true)
            == Err("military_security");
        self.advance_diplomatic_offers_for(1);
        let purchased =
            self.has_tech(1, writing) && self.players[1].gold < 5000 && self.players[2].gold > 0;
        let conserved = self.players[1].gold + self.players[2].gold == 5000;
        let after = self.players[1].gold;
        self.advance_proactive_trades_for(1);
        let no_duplicate = self.players[1].gold == after;
        vec![
            (
                "allied_cash_purchase",
                purchased,
                "An ally trades despite the old target filter and without a technology to swap"
                    .into(),
            ),
            (
                "reserve_protected",
                reserve_protected,
                "Technology purchases preserve the treasury reserve".into(),
            ),
            (
                "trait_and_goal_priorities",
                scientific > ordinary && goal > scientific,
                "Scientific traits and research goals affect valuation".into(),
            ),
            (
                "military_security",
                security,
                "Military unlocks are not exported to a distrusted nonally".into(),
            ),
            (
                "gold_conserved",
                conserved,
                "Technology sales transfer rather than create gold".into(),
            ),
            (
                "trade_cooldown",
                no_duplicate,
                "The pair cannot be charged again by a repeated phase call".into(),
            ),
        ]
    }
}
