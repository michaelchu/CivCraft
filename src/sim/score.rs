//! Civilization score and ranking calculations.

use super::*;
use crate::rules::Genus;

pub(super) const TURN_LIMIT: i32 = 5000;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct NationScore {
    pub(super) units_built: i32,
    pub(super) units_killed: i32,
    pub(super) achievements: u8,
    pub(super) achievement_culture: i32,
}

impl Simulation {
    pub(super) fn record_history(&mut self, city_index: usize) {
        let city = &self.cities[city_index];
        let history: i32 = city
            .buildings
            .iter()
            .copied()
            .filter(|&building| !self.building_is_obsolete(city, building, None))
            .filter(|&building| {
                building != Building::ECCLESIASTICAL_PALACE || !city.has_building(Building::PALACE)
            })
            .map(Building::history_per_turn)
            .sum();
        self.cities[city_index].history += history;
    }

    pub(crate) fn civilization_score(&self, owner: usize) -> i32 {
        let (mut population, mut wonders, mut culture) = (0, 0, 0);
        for city in self.cities_owned_by(owner) {
            population += city.size;
            wonders += city
                .buildings
                .iter()
                .filter(|building| building.data().genus == Genus::GreatWonder)
                .count() as i32;
            culture += city.history;
        }
        let player = &self.players[owner];
        let units = &self.nation_scores[owner];
        culture += units.achievement_culture;
        let technologies = player.known.len() as i32 + player.future_techs * 5 / 2;
        population
            + technologies * 2
            + wonders * 5
            + self.spaceships[owner].score()
            + units.units_built / 10
            + units.units_killed / 3
            + culture / 50
    }

    pub(crate) fn score_standings(&self) -> Vec<(usize, i32)> {
        let mut scores: Vec<_> = (0..self.players.len())
            .filter(|&owner| owner != BARBARIANS)
            .map(|owner| (owner, self.civilization_score(owner)))
            .collect();
        scores.sort_by_key(|&(owner, score)| (!self.player_alive(owner), std::cmp::Reverse(score)));
        scores
    }

    pub(super) fn check_ranked_endgame(&mut self) -> bool {
        let living: Vec<_> = (0..self.players.len())
            .filter(|&owner| owner != BARBARIANS && self.player_alive(owner))
            .collect();
        if living.len() > 1
            && living
                .iter()
                .enumerate()
                .all(|(i, &owner)| living[..i].iter().all(|&other| self.allied(owner, other)))
        {
            self.game_over = Some("The surviving civilizations won an allied victory.".to_owned());
            return true;
        }
        if self.turn > TURN_LIMIT {
            let (winner, score) = self.score_standings()[0];
            self.game_over = Some(format!(
                "Turn limit reached. {} leads with {score} points.",
                self.players[winner].name
            ));
            return true;
        }
        if self.turn == TURN_LIMIT {
            self.add_event("The game will end after this turn.");
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_counts_history_and_freeciv_integer_weights() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let city = &mut game.cities[0];
        city.size = 7;
        city.buildings
            .extend([Building::LIBRARY, Building::APOLLO_PROGRAM]);
        city.history = 45;
        game.record_history(0);
        assert_eq!(game.cities[0].history, 53);
        game.players[HUMAN].known = vec![Tech::ALPHABET];
        game.players[HUMAN].future_techs = 1;
        game.nation_scores[HUMAN].units_built = 19;
        game.nation_scores[HUMAN].units_killed = 7;
        assert_eq!(game.civilization_score(HUMAN), 7 + 6 + 5 + 1 + 2 + 1);
    }

    #[test]
    fn allied_victory_requires_every_surviving_pair_to_be_allied() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let owners: Vec<_> = (0..game.players.len())
            .filter(|&owner| owner != BARBARIANS && game.player_alive(owner))
            .collect();
        assert!(owners.len() > 2);
        for (index, &owner) in owners.iter().enumerate() {
            for &other in &owners[..index] {
                if (owner, other) != (owners[1], owners[0]) {
                    game.sign_pact(owner, other, Pact::Alliance);
                }
            }
        }
        assert!(!game.check_ranked_endgame());
        game.sign_pact(owners[1], owners[0], Pact::Alliance);
        assert!(game.check_ranked_endgame());
        assert!(game.game_over().unwrap().contains("allied victory"));
    }
}
