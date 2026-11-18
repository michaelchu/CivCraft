//! Exercises actual exchanges and pact cancellation, rather than proposed actions.
use super::*;

impl Simulation {
    pub(in crate::sim) fn relationship_memory_scenario(
        &mut self,
    ) -> Vec<(&'static str, bool, String)> {
        let _ = self.proactive_trade_scenario();
        let mut checks = Vec::new();
        let commerce = self.relationship_memory(1, 2).commerce;
        checks.push((
            "completed_trade_memory",
            commerce > 0 && self.relationship_memory(2, 1).commerce > 0,
            "An executed AI technology purchase builds directional cooperation".into(),
        ));
        let before = self.encode_campaign().unwrap();
        let _ = self.observe_relationship(1, 2);
        checks.push((
            "readonly_observation",
            before == self.encode_campaign().unwrap(),
            "Inspecting relationship memory does not mutate the campaign".into(),
        ));
        // A third nation is already fighting nation 2; nation 1 joins its ally.
        self.tile_mut(8, 20).terrain = Terrain::Grassland;
        self.create_city(HUMAN, 8, 20);
        self.players[HUMAN].ai = true;
        self.set_scenario_diplomacy(HUMAN, 1, Pact::Alliance, self.turn, [6000; 2]);
        self.set_scenario_diplomacy(HUMAN, 2, Pact::War, self.turn, [0; 2]);
        self.set_scenario_diplomacy(1, 2, Pact::Ceasefire, self.turn, [0; 2]);
        self.diplomacy.get_mut(&(1, 2)).unwrap().until = self.turn + 16;
        let trust = self.diplomacy[&(HUMAN, 1)].trust[0];
        self.declare_ai_war_now(1, 2);
        checks.push((
            "actual_ally_support",
            self.diplomatic_at_war(1, 2)
                && self.relationship_memory(HUMAN, 1).loyalty == policy().support_gain,
            "Joining an ally's existing war earns loyalty only after war starts".into(),
        ));
        checks.push((
            "ally_support_context",
            self.relationship_memory(2, 1).grievance == 0
                && self.diplomacy[&(HUMAN, 1)].trust[0] == trust,
            "Helping an ally is not classified as an unjustified breach or punished by that ally"
                .into(),
        ));
        // A separate branch removes the ally-war justification.
        let mut breach = self.clone();
        breach.set_scenario_diplomacy(HUMAN, 2, Pact::Peace, self.turn, [0; 2]);
        breach.set_scenario_diplomacy(1, 2, Pact::Alliance, self.turn, [6000; 2]);
        breach.declare_ai_war_now(1, 2);
        checks.push((
            "betrayal_counted_once",
            breach.relationship_memory(2, 1).grievance == policy().breach_grievance,
            "Alliance-to-armistice-to-war is one unjustified betrayal".into(),
        ));
        checks
    }
}
