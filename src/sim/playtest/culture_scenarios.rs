//! Automated checks for Freeciv culture and history behavior.

use super::*;

impl Simulation {
    /// Exercise ruleset-driven city history, ownership transfer, and the
    /// selected C2C3 culture-victory default.
    pub(crate) fn culture_accumulation_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.create_city(HUMAN, 10, 24);
        self.process_player_cities(HUMAN);
        let history_uses_ruleset = self.cities[0].history == 1
            && self.city_culture_score(&self.cities[0]) == 1
            && self.civilization_culture(HUMAN) == 1;

        let mut conquest = Simulation::new(crate::mapgen::generate(42));
        conquest.units.clear();
        conquest.cities.clear();
        conquest.create_city(HUMAN, 10, 24);
        conquest.create_city(1, 20, 24);
        let target_id = conquest.cities[1].id;
        conquest.cities[1].size = 3;
        conquest.cities[1].history = 55;
        conquest.sign_pact(HUMAN, 1, Pact::War);
        let attacker = conquest.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);
        conquest.capture_city_if_needed(attacker, 20, 24);
        let captured = conquest.city_index_by_id(target_id).is_some_and(|index| {
            conquest.cities[index].owner == HUMAN
                && conquest.cities[index].history == 55
                && conquest.civilization_culture(HUMAN) >= 55
        });

        let c2c3_victory_default =
            !self.victory_enabled(crate::rules::freeciv::VictoryCondition::Culture);
        (history_uses_ruleset, captured, c2c3_victory_default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_culture_scenario_checks_freeciv_history_and_transfer() {
        let mut simulation = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            simulation.culture_accumulation_playtest(),
            (true, true, true)
        );
    }
}
