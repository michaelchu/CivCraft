//! Core Civ III Conquests culture and victory rules with CivCraft border integration.

use super::action_rules::SimulationRequirementContext;
use super::{BARBARIANS, City, CultureBuildingAge, Simulation};
use crate::rules::engine_rules::EffectKind;
use crate::rules::{Building, Genus};

fn apply_culture_percent(value: i64, percent: i64) -> i64 {
    value.saturating_mul(100_i64.saturating_add(percent)) / 100
}

const CULTURE_VICTORY_CITY_POINTS: i64 = 20_000;
const CULTURE_VICTORY_STANDARD_EMPIRE_POINTS: i64 = 100_000;
const CULTURE_BUILDING_AGE_BONUS_YEARS: i32 = 1_000;
const CULTURE_BORDER_THRESHOLDS: [i64; 4] = [10, 100, 1_000, 10_000];

/// Return the Civ III border radius for the culture tier, if a tier has been
/// reached. Distances use CivCraft's existing Civ III map metric.
pub(crate) fn city_culture_border_radius_sq(culture: i64) -> Option<i64> {
    match CULTURE_BORDER_THRESHOLDS
        .iter()
        .rposition(|threshold| culture >= *threshold)?
    {
        0 => Some(9),
        1 => Some(16),
        2 => Some(25),
        _ => Some(36),
    }
}

pub(crate) fn next_city_culture_border_threshold(culture: i64) -> Option<i64> {
    CULTURE_BORDER_THRESHOLDS
        .into_iter()
        .find(|threshold| culture < *threshold)
}

fn empire_cultural_victory_winner(
    cultures: impl IntoIterator<Item = (usize, i64)>,
    target: i64,
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
    best.filter(|_| best_value >= target && best_value >= second_value.saturating_mul(2))
}

impl Simulation {
    /// Current local city culture, including stored accumulation and active
    /// CivCraft culture modifiers.
    pub(crate) fn city_culture_score(&self, city: &City) -> i64 {
        i64::from(city.history).saturating_add(apply_culture_percent(
            self.city_effect(city, EffectKind::Performance),
            self.city_effect(city, EffectKind::CulturePct),
        ))
    }

    /// Current civilization culture. City histories are credited to the current
    /// city owner, while culture retained from lost cities is national history.
    pub(crate) fn civilization_culture(&self, owner: usize) -> i64 {
        if owner >= self.players.len() {
            return 0;
        }
        let player_context = SimulationRequirementContext::for_player_at(self, owner, None);
        let national_performance = player_context.effect(EffectKind::NationalPerformance);
        let culture_percent = player_context.effect(EffectKind::CulturePct);
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

    fn culture_building_history(&self, city: &City, building: Building) -> i64 {
        let age = city
            .culture_buildings
            .iter()
            .find(|record| record.building == building)
            .copied()
            .unwrap_or(CultureBuildingAge {
                building,
                builder: city.owner,
                active_since_year: self.year,
            });
        if age.builder != city.owner {
            return 0;
        }
        let mut rate = i64::from(building.data().culture_per_turn);
        if self.year.saturating_sub(age.active_since_year) >= CULTURE_BUILDING_AGE_BONUS_YEARS {
            rate = rate.saturating_mul(2);
        }
        apply_culture_percent(rate, self.city_effect(city, EffectKind::CulturePct))
    }

    pub(crate) fn city_culture_per_turn(&self, city: &City) -> i64 {
        let building_culture =
            if city.owner < self.players.len() && self.players[city.owner].mobilized {
                // Civ III halves building culture while the nation is mobilized.
                city.buildings
                    .iter()
                    .filter(|building| building.data().genus != Genus::Special)
                    .map(|&building| self.culture_building_history(city, building) / 2)
                    .fold(0_i64, i64::saturating_add)
            } else {
                city.buildings
                    .iter()
                    .filter(|building| building.data().genus != Genus::Special)
                    .map(|&building| self.culture_building_history(city, building))
                    .fold(0_i64, i64::saturating_add)
            };
        building_culture.saturating_add(i64::from(self.religion_culture(city)))
    }

    /// Apply Civ III city culture after activity, production, and growth. The
    /// return value indicates that a border milestone has just been reached.
    pub(super) fn record_city_history(&mut self, city_index: usize) -> bool {
        let city = &self.cities[city_index];
        let history = i64::from(city.history);
        let before_tier = city_culture_border_radius_sq(self.city_culture_score(city));
        let gain = self.city_culture_per_turn(city);
        self.cities[city_index].history =
            i32::try_from(history.saturating_add(gain).max(0)).unwrap_or(i32::MAX);
        let city = &self.cities[city_index];
        before_tier != city_culture_border_radius_sq(self.city_culture_score(city))
    }

    /// Apply Freeciv national history at the end of national turn activities.
    pub(super) fn record_player_history(&mut self, owner: usize) {
        if owner >= self.players.len() {
            return;
        }
        let history = i64::from(self.players[owner].history);
        let gain = apply_culture_percent(
            self.player_effect(owner, EffectKind::NationalHistory),
            self.player_effect(owner, EffectKind::CulturePct),
        );
        self.players[owner].history =
            i32::try_from(history.saturating_add(gain).max(0)).unwrap_or(i32::MAX);
    }

    pub(super) fn preserve_city_culture_on_transfer(
        &mut self,
        city_index: usize,
        new_owner: usize,
    ) {
        let old_owner = self.cities[city_index].owner;
        if old_owner == new_owner {
            return;
        }
        let history = self.cities[city_index].history.max(0);
        self.players[old_owner].history = self.players[old_owner].history.saturating_add(history);
        let year = self.year;
        let city = &mut self.cities[city_index];
        city.history = 0;
        // Captured cultural buildings produce nothing for the occupier. Their
        // original builder resumes culture on recapture, but loses the age bonus.
        for record in &mut city.culture_buildings {
            if record.builder == new_owner {
                record.active_since_year = year;
            }
        }
        city.retain_existing_culture_buildings();
    }

    pub(super) fn preserve_city_culture_on_destruction(&mut self, city: &City) {
        let history = city.history.max(0);
        self.players[city.owner].history = self.players[city.owner].history.saturating_add(history);
    }

    pub(super) fn initialize_missing_culture_building_ages(&mut self) {
        let year = self.year;
        for city in &mut self.cities {
            for &building in &city.buildings.clone() {
                if !city
                    .culture_buildings
                    .iter()
                    .any(|record| record.building == building)
                {
                    city.record_culture_building(building, city.owner, year);
                }
            }
        }
    }

    /// Civ III checks both a legendary city and a leading cultural empire.
    pub(super) fn check_cultural_victory(&mut self) {
        if self.game_over.is_some() {
            return;
        }

        let alive: Vec<_> = (0..self.players.len())
            .filter(|&owner| owner != BARBARIANS && self.player_alive(owner))
            .collect();
        for &owner in &alive {
            if self.cities.iter().any(|city| {
                city.owner == owner && self.city_culture_score(city) >= CULTURE_VICTORY_CITY_POINTS
            }) {
                self.game_over = Some(format!(
                    "{} wins a cultural victory with a legendary city.",
                    self.players[owner].name
                ));
                return;
            }
        }

        let cultures: Vec<_> = alive
            .into_iter()
            .map(|owner| (owner, self.civilization_culture(owner)))
            .collect();
        let standard_area = i64::from(crate::world::topology::MAP_W)
            .saturating_mul(i64::from(crate::world::topology::MAP_H));
        let map_area = i64::try_from(self.tiles.len()).unwrap_or(i64::MAX);
        let target = CULTURE_VICTORY_STANDARD_EMPIRE_POINTS
            .saturating_mul(map_area)
            .checked_div(standard_area.max(1))
            .unwrap_or(CULTURE_VICTORY_STANDARD_EMPIRE_POINTS);
        if let Some(winner) = empire_cultural_victory_winner(cultures, target) {
            self.game_over = Some(format!(
                "{} wins a cultural victory through cultural dominance.",
                self.players[winner].name
            ));
        }
    }
}

impl Simulation {
    #[cfg(any(test, feature = "scenarios"))]
    pub(crate) fn city_history(&self, id: u32) -> Option<i32> {
        self.city_by_id(id).map(|city| city.history)
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
    }

    #[test]
    fn culture_victory_thresholds_are_core_campaign_rules() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.victory_enabled(crate::rules::engine_rules::VictoryCondition::Culture));
        assert_eq!(CULTURE_VICTORY_CITY_POINTS, 20_000);
        assert_eq!(CULTURE_VICTORY_STANDARD_EMPIRE_POINTS, 100_000);
    }

    #[test]
    fn ai_culture_leader_wins_at_the_normal_turn_boundary() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let city_name = game.create_city(1, 10, 20);
        let city = game
            .cities
            .iter_mut()
            .find(|city| city.name == city_name)
            .unwrap();
        city.history = CULTURE_VICTORY_CITY_POINTS as i32;
        game.refresh_score_snapshot();
        let winner = game.players[1].name.clone();

        game.simulate_turn();

        assert_eq!(
            game.game_over(),
            Some(format!("{winner} wins a cultural victory with a legendary city.").as_str())
        );
    }

    #[test]
    fn culture_percent_uses_freeciv_integer_truncation() {
        assert_eq!(apply_culture_percent(5, 50), 7);
        assert_eq!(apply_culture_percent(5, -50), 2);
        assert_eq!(apply_culture_percent(0, 100), 0);
    }

    #[test]
    fn city_culture_unlocks_border_tiers_at_civ_iii_thresholds() {
        assert_eq!(city_culture_border_radius_sq(0), None);
        assert_eq!(city_culture_border_radius_sq(9), None);
        assert_eq!(city_culture_border_radius_sq(10), Some(9));
        assert_eq!(city_culture_border_radius_sq(100), Some(16));
        assert_eq!(city_culture_border_radius_sq(1_000), Some(25));
        assert_eq!(city_culture_border_radius_sq(10_000), Some(36));
        assert_eq!(next_city_culture_border_threshold(10), Some(100));
        assert_eq!(next_city_culture_border_threshold(10_000), None);
    }

    #[test]
    fn empire_culture_victory_requires_twice_the_runner_up() {
        let target = CULTURE_VICTORY_STANDARD_EMPIRE_POINTS;
        assert_eq!(
            empire_cultural_victory_winner([(0, target - 1), (1, 0)], target),
            None
        );
        assert_eq!(
            empire_cultural_victory_winner([(0, target), (1, target / 2)], target),
            Some(0)
        );
        assert_eq!(
            empire_cultural_victory_winner([(0, target), (1, target / 2 + 1)], target),
            None
        );
    }

    #[test]
    fn culture_victory_ties_never_select_a_winner() {
        assert_eq!(
            empire_cultural_victory_winner(
                [
                    (0, CULTURE_VICTORY_STANDARD_EMPIRE_POINTS),
                    (1, CULTURE_VICTORY_STANDARD_EMPIRE_POINTS)
                ],
                CULTURE_VICTORY_STANDARD_EMPIRE_POINTS
            ),
            None
        );
    }
}
