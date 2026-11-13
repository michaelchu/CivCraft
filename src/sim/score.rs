//! Civilization score and ranking calculations.

use super::*;
use crate::rules::Genus;

pub(super) const TURN_LIMIT: i32 = 5000;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct NationScore {
    pub(super) units_built: i32,
    pub(super) units_killed: i32,
    /// Freeciv achievement rule names are stable save identities; the source
    /// supports up to 40 configured achievements.
    pub(super) achievements: std::collections::BTreeSet<String>,
    /// Freeciv refreshes score.culture, literacy and the cached score inputs
    /// at begin_turn, then reads them after the player phase. Keep those
    /// boundary values across saves made mid-turn.
    pub(super) score_culture: i64,
    pub(super) score_literacy: i32,
    /// Remaining cached inputs used by get_civ_score() at the turn boundary.
    pub(super) score_population: i32,
    pub(super) score_technologies: i32,
    pub(super) score_wonders: i32,
    pub(super) score_spaceship: i32,
}

impl Simulation {
    /// Rebuild the subset of Freeciv's begin-turn score cache consumed by
    /// culture victory and the Literate achievement.
    pub(super) fn refresh_score_snapshot(&mut self) {
        let snapshots: Vec<_> = (0..self.players.len())
            .map(|owner| {
                let (population, technologies, wonders, spaceship) = if owner == BARBARIANS {
                    (0, 0, 0, 0)
                } else {
                    let population = self.cities_owned_by(owner).map(|city| city.size).sum();
                    let technologies = self.players[owner].known.len() as i32
                        + self.players[owner].future_techs * 5 / 2;
                    let wonders = self
                        .cities_owned_by(owner)
                        .flat_map(|city| city.buildings.iter())
                        .filter(|building| building.data().genus == Genus::GreatWonder)
                        .count() as i32;
                    (
                        population,
                        technologies,
                        wonders,
                        self.spaceships[owner].score(),
                    )
                };
                (
                    self.civilization_culture(owner),
                    self.literacy_score(owner),
                    population,
                    technologies,
                    wonders,
                    spaceship,
                )
            })
            .collect();
        for (score, (culture, literacy, population, technologies, wonders, spaceship)) in
            self.nation_scores.iter_mut().zip(snapshots)
        {
            score.score_culture = culture;
            score.score_literacy = literacy;
            score.score_population = population;
            score.score_technologies = technologies;
            score.score_wonders = wonders;
            score.score_spaceship = spaceship;
        }
    }

    /// Port of common/achievements.c::get_literacy over the inputs built by
    /// server/score.c::calc_civ_score.
    fn literacy_score(&self, owner: usize) -> i32 {
        if owner == BARBARIANS {
            // calc_civ_score initializes these fields, then returns early for
            // barbarian players.
            return 0;
        }
        let mut population = 0;
        let mut literate = 0;
        for city in self.cities_owned_by(owner) {
            let city_population = city.size * (city.size + 1) * 5;
            population += city_population;
            let bonus = self.science_building_bonus(city).clamp(0, 100);
            literate += percent_of(city_population, bonus);
        }
        match population {
            0 => 0,
            1..=9999 => literate * 100 / population,
            _ => literate / (population / 100),
        }
    }

    pub(crate) fn civilization_score(&self, owner: usize) -> i32 {
        // Freeciv's total_player_citizens() reads the mood/specialist counts
        // cached by calc_civ_score(), so population shares the begin-turn
        // snapshot boundary with technology, wonders, spaceship, and culture.
        let units = &self.nation_scores[owner];
        // get_civ_score() uses the culture value cached by calc_civ_score.
        let snapshot = &self.nation_scores[owner];
        snapshot.score_population
            + snapshot.score_technologies * 2
            + snapshot.score_wonders * 5
            + snapshot.score_spaceship
            + units.units_built / 10
            + units.units_killed / 3
            + i32::try_from(snapshot.score_culture / 50).unwrap_or(i32::MAX)
    }

    pub(crate) fn score_standings(&self) -> Vec<(usize, i32)> {
        let mut scores: Vec<_> = (0..self.players.len())
            .filter(|&owner| {
                owner != BARBARIANS && (owner < self.civilization_count || owner > BARBARIANS)
            })
            .map(|owner| (owner, self.civilization_score(owner)))
            .collect();
        scores.sort_by_key(|&(owner, score)| (!self.player_alive(owner), std::cmp::Reverse(score)));
        scores
    }

    pub(super) fn check_allied_victory(&mut self) -> bool {
        let living: Vec<_> = (0..self.players.len())
            .filter(|&owner| owner != BARBARIANS && self.player_alive(owner))
            .collect();
        if living.len() > 1
            && self.victory_enabled(crate::rules::freeciv::VictoryCondition::Allied)
            && living
                .iter()
                .enumerate()
                .all(|(i, &owner)| living[..i].iter().all(|&other| self.allied(owner, other)))
        {
            self.game_over = Some("The surviving civilizations won an allied victory.".to_owned());
            return true;
        }
        false
    }

    pub(super) fn check_turn_limit(&mut self) -> bool {
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
        game.players[HUMAN].known = vec![Tech::ALPHABET];
        game.players[HUMAN].future_techs = 1;
        game.nation_scores[HUMAN].units_built = 19;
        game.nation_scores[HUMAN].units_killed = 7;
        game.refresh_score_snapshot();
        // City history is retained as the Freeciv culture score input.
        assert_eq!(game.civilization_score(HUMAN), 7 + 6 + 1 + 2);
    }

    #[test]
    fn begin_turn_snapshot_holds_culture_and_literacy_until_next_refresh() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        game.cities[0].size = 20;
        game.cities[0].history = 2_000;
        game.cities[0]
            .buildings
            .extend([Building::LIBRARY, Building::UNIVERSITY]);

        game.refresh_score_snapshot();
        let culture = game.nation_scores[HUMAN].score_culture;
        assert_eq!(game.nation_scores[HUMAN].score_literacy, 100);
        assert_eq!(culture, game.civilization_culture(HUMAN));

        game.players[HUMAN].history += 500;
        game.cities[0]
            .buildings
            .retain(|building| *building != Building::LIBRARY && *building != Building::UNIVERSITY);
        assert_ne!(game.civilization_culture(HUMAN), culture);
        assert_eq!(game.nation_scores[HUMAN].score_culture, culture);
        assert_eq!(game.nation_scores[HUMAN].score_literacy, 100);

        game.refresh_score_snapshot();
        assert_eq!(
            game.nation_scores[HUMAN].score_culture,
            game.civilization_culture(HUMAN)
        );
        assert_eq!(game.nation_scores[HUMAN].score_literacy, 0);
    }

    #[test]
    fn endgame_score_uses_cached_technology_and_wonder_counts() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        game.cities[0].size = 7;
        game.refresh_score_snapshot();
        let score = game.civilization_score(HUMAN);

        game.players[HUMAN].known.push(Tech::ALPHABET);
        game.cities[0].buildings.push(Building::PYRAMIDS);
        game.cities[0].size += 3;
        assert_eq!(game.civilization_score(HUMAN), score);

        game.refresh_score_snapshot();
        assert_eq!(game.civilization_score(HUMAN), score + 10);
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
        assert!(!game.check_allied_victory());
        game.sign_pact(owners[1], owners[0], Pact::Alliance);
        assert!(game.check_allied_victory());
        assert!(game.game_over().unwrap().contains("allied victory"));
    }
}
