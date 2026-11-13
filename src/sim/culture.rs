//! Freeciv culture history and culture-victory rules for the selected ruleset.

use super::action_rules::SimulationRequirementContext;
use super::{BARBARIANS, City, Simulation};
use crate::rules::freeciv::{RulesetData, VictoryCondition, effect_value};

fn culture_parameter(key: &str) -> i64 {
    let rules = RulesetData::campaign_default();
    let mut matches = rules.parameters.iter().filter(|parameter| {
        parameter.file.eq_ignore_ascii_case("game.ruleset")
            && parameter.section.eq_ignore_ascii_case("culture")
            && parameter.key.eq_ignore_ascii_case(key)
    });
    let parameter = matches
        .next()
        .unwrap_or_else(|| panic!("selected C2C3 ruleset is missing [culture].{key}"));
    assert!(
        matches.next().is_none(),
        "selected C2C3 repeats [culture].{key}"
    );
    parameter
        .integer_value()
        .unwrap_or_else(|error| panic!("invalid selected [culture].{key}: {error}"))
        .unwrap_or_else(|| panic!("selected [culture].{key} has no value"))
}

fn effect_for_city(game: &Simulation, city: &City, kind: &str) -> i64 {
    let context = SimulationRequirementContext::for_city_at(game, city, Some((city.x, city.y)));
    effect_value(&RulesetData::campaign_default().effects, kind, &context)
        .unwrap_or_else(|error| panic!("failed to evaluate city {kind} effect: {error:?}"))
}

fn effect_for_player(game: &Simulation, owner: usize, kind: &str) -> i64 {
    let context = SimulationRequirementContext::for_player_at(game, owner, None);
    effect_value(&RulesetData::campaign_default().effects, kind, &context)
        .unwrap_or_else(|error| panic!("failed to evaluate player {kind} effect: {error:?}"))
}

fn apply_culture_percent(value: i64, percent: i64) -> i64 {
    value.saturating_mul(100_i64.saturating_add(percent)) / 100
}

/// Apply Freeciv's minimum-culture and strict-lead victory test to the
/// already-filtered scores of living non-barbarian players.
fn cultural_victory_winner(
    cultures: impl IntoIterator<Item = (usize, i64)>,
    minimum: i64,
    lead_percent: i64,
) -> Option<usize> {
    let mut best = None;
    let mut best_value = -1_i64;
    let mut second_value = -1_i64;
    for (owner, culture) in cultures {
        if culture > best_value {
            second_value = best_value;
            best = Some(owner);
            best_value = culture;
        } else if culture > second_value {
            second_value = culture;
        }
    }

    best.filter(|_| {
        best_value >= minimum
            && best_value > second_value.saturating_mul(100_i64.saturating_add(lead_percent)) / 100
    })
}

impl Simulation {
    /// Current Freeciv culture score for one city, including its stored history
    /// and active Performance/Culture_Pct effects.
    pub(crate) fn city_culture_score(&self, city: &City) -> i64 {
        i64::from(city.history).saturating_add(apply_culture_percent(
            effect_for_city(self, city, "Performance"),
            effect_for_city(self, city, "Culture_Pct"),
        ))
    }

    /// Current Freeciv culture score for a player. City histories are credited
    /// to the city owner, while national history and national performance are
    /// stored and evaluated at player scope.
    pub(crate) fn civilization_culture(&self, owner: usize) -> i64 {
        if owner >= self.players.len() {
            return 0;
        }
        let player_context = SimulationRequirementContext::for_player_at(self, owner, None);
        let rules = RulesetData::campaign_default();
        let national_performance =
            effect_value(&rules.effects, "National_Performance", &player_context).unwrap_or_else(
                |error| panic!("failed to evaluate National_Performance: {error:?}"),
            );
        let culture_percent = effect_value(&rules.effects, "Culture_Pct", &player_context)
            .unwrap_or_else(|error| panic!("failed to evaluate player Culture_Pct: {error:?}"));
        let cities = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| self.city_culture_score(city))
            .fold(0_i64, i64::saturating_add);
        i64::from(self.players[owner].history)
            .saturating_add(apply_culture_percent(national_performance, culture_percent))
            .saturating_add(cities)
    }

    /// Apply Freeciv city history after city activity, production and growth.
    /// C2C3 effects determine the raw history; existing history interest comes
    /// from game.ruleset and may raise or lower the score, but never below zero.
    pub(super) fn record_city_history(&mut self, city_index: usize) {
        let city = &self.cities[city_index];
        let history = i64::from(city.history);
        let gain = apply_culture_percent(
            effect_for_city(self, city, "History"),
            effect_for_city(self, city, "Culture_Pct"),
        )
        .saturating_add(history.saturating_mul(culture_parameter("history_interest_pml")) / 1000);
        self.cities[city_index].history =
            i32::try_from(history.saturating_add(gain).max(0)).unwrap_or(i32::MAX);
    }

    /// Apply Freeciv national history at the end of national turn activities.
    pub(super) fn record_player_history(&mut self, owner: usize) {
        if owner >= self.players.len() {
            return;
        }
        let history = i64::from(self.players[owner].history);
        let gain = apply_culture_percent(
            effect_for_player(self, owner, "National_History"),
            effect_for_player(self, owner, "Culture_Pct"),
        )
        .saturating_add(history.saturating_mul(culture_parameter("history_interest_pml")) / 1000);
        self.players[owner].history =
            i32::try_from(history.saturating_add(gain).max(0)).unwrap_or(i32::MAX);
    }

    /// Freeciv chooses the culture leader among living non-barbarian players,
    /// then applies the selected ruleset's minimum and strict lead percentage.
    pub(super) fn check_cultural_victory(&mut self) {
        if self.game_over.is_some() || !self.victory_enabled(VictoryCondition::Culture) {
            return;
        }

        let minimum = culture_parameter("victory_min_points");
        let lead_percent = culture_parameter("victory_lead_pct");
        let cultures = (0..self.players.len())
            .filter(|&owner| owner != BARBARIANS && self.player_alive(owner))
            .map(|owner| (owner, self.civilization_culture(owner)));
        if let Some(winner) = cultural_victory_winner(cultures, minimum, lead_percent) {
            self.game_over = Some(format!(
                "{} wins a cultural domination victory.",
                self.players[winner].name
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Building;

    #[test]
    fn city_history_uses_selected_building_effects_and_city_culture_score() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(0, 10, 20);
        let index = 0;
        assert!(game.cities[index].has_building(Building::PALACE));
        assert_eq!(game.cities[index].history, 0);

        game.record_city_history(index);

        assert_eq!(game.cities[index].history, 1);
        assert_eq!(game.city_culture_score(&game.cities[index]), 1);
        assert_eq!(game.civilization_culture(0), 1);
    }

    #[test]
    fn national_history_uses_selected_ruleset_and_is_saved_on_player() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.players[0].history, 0);

        game.record_player_history(0);

        // C2C3 has no National_History effects in its selected effects file.
        assert_eq!(game.players[0].history, 0);
        assert_eq!(culture_parameter("history_interest_pml"), 0);
    }

    #[test]
    fn c2c3_culture_victory_is_disabled_by_default() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert!(!game.victory_enabled(VictoryCondition::Culture));
        assert_eq!(culture_parameter("victory_min_points"), 20_000);
        assert_eq!(culture_parameter("victory_lead_pct"), 100);
    }

    #[test]
    fn culture_percent_uses_freeciv_integer_truncation() {
        assert_eq!(apply_culture_percent(5, 50), 7);
        assert_eq!(apply_culture_percent(5, -50), 2);
        assert_eq!(apply_culture_percent(0, 100), 0);
    }

    #[test]
    fn culture_victory_uses_inclusive_minimum_and_strict_lead() {
        let minimum = culture_parameter("victory_min_points");
        let lead = culture_parameter("victory_lead_pct");

        // Freeciv's minimum uses >=; the 100% lead comparison is strict.
        assert_eq!(
            cultural_victory_winner([(0, minimum - 1), (1, 0)], minimum, lead),
            None
        );
        assert_eq!(
            cultural_victory_winner([(0, minimum), (1, (minimum - 1) / 2)], minimum, lead),
            Some(0)
        );
        assert_eq!(
            cultural_victory_winner([(0, 40_000), (1, 20_000)], minimum, lead),
            None
        );
        assert_eq!(
            cultural_victory_winner([(0, 40_001), (1, 20_000)], minimum, lead),
            Some(0)
        );
    }

    #[test]
    fn culture_victory_ties_never_select_a_winner() {
        let minimum = culture_parameter("victory_min_points");
        let lead = culture_parameter("victory_lead_pct");

        assert_eq!(
            cultural_victory_winner([(0, minimum * 2), (1, minimum * 2)], minimum, lead),
            None
        );
    }
}
