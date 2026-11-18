//! Controlled strategy regression, shared by Rust and headless tests.
use super::*;

impl Simulation {
    pub(in crate::sim) fn shared_strategy_scenario(&mut self) -> Vec<(&'static str, bool, String)> {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        self.ai_war_plans.clear();
        for player in &mut self.players {
            player.ai = false;
        }
        for (owner, x, y) in [(1, 10, 24), (2, 18, 30)] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
            let player = &mut self.players[owner];
            player.ai = true;
            player.known = vec![Tech::ALPHABET];
            player.researching = Tech::POTTERY;
            player.research_goal = Tech::UNSET;
            player.research = 0;
            player.last_science = 20;
            player.gold = 5000;
        }
        self.make_diplomatic_contact(1, 2);
        self.sign_pact(1, 2, Pact::Peace);
        let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
        self.players[2].known.push(writing);
        self.clear_ai_tech_wants(1);
        self.add_ai_tech_want(1, Tech::POTTERY, 100.0);
        self.add_ai_tech_want(1, writing, 100.0);
        self.players[1].civilization_id = Some("rome".into());
        let ordinary = self.select_ai_research(1).0;
        self.players[1].research_goal = Tech::UNSET;
        let ordinary_bid = self.diplomatic_technology_value(1, 2, writing, false);
        self.players[1].civilization_id = Some("korea".into());
        let scientific = self.select_ai_research(1).0;
        self.players[1].research_goal = Tech::UNSET;
        let scientific_bid = self.diplomatic_technology_value(1, 2, writing, false);
        let peaceful = self.strategic_priorities(1);
        self.sign_pact(1, 2, Pact::War);
        let wartime = self.strategic_priorities(1);
        self.sign_pact(1, 2, Pact::Peace);
        self.cities[0]
            .buildings
            .extend(std::iter::repeat_n(Building::LIBRARY, 20));
        self.players[1].gold = 0;
        let crisis = self.strategic_priorities(1);
        let protected = self.diplomatic_trade_reserve(1) >= crisis.gold_reserve
            && crisis.gold_reserve > self.ai_gold_reserve(1);
        let mut recovered = self.clone();
        recovered.players[1].gold = 5000;
        let recovery = recovered.strategic_priorities(1);
        // End the scenario in a normal playable state with valid research.
        self.cities[0].buildings.clear();
        self.players[1].gold = 5000;
        vec![
            (
                "trait_research_choice",
                ordinary == Tech::POTTERY && scientific == writing,
                "Equal advisor wants resolve differently for Rome and scientific Korea".into(),
            ),
            (
                "consistent_trade_interest",
                matches!((ordinary_bid, scientific_bid), (Ok(a), Ok(b)) if b > a),
                "The science preference also raises willingness to buy Writing".into(),
            ),
            (
                "wartime_priority",
                peaceful.wars == 0 && wartime.wars == 1 && wartime.military > peaceful.military,
                "A contacted living enemy raises military priority".into(),
            ),
            (
                "solvency_reserve",
                crisis.economic_crisis && protected,
                "Projected losses raise the reserve shared with economic diplomacy".into(),
            ),
            (
                "optional_war_recovery",
                !crisis.can_start_optional_war() && recovery.can_start_optional_war(),
                "An insolvent nation postpones discretionary expansion until it can afford it"
                    .into(),
            ),
        ]
    }
}
