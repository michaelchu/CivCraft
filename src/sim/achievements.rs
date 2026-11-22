//! Achievement checks and rewards.
//!
//! The achievements and their thresholds are the campaign's pinned Freeciv
//! `civ2civ3` set. A stored achievement is keyed by its `name`, so renaming one
//! would orphan saved progress.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AchievementKind {
    Spaceship,
    MapKnown,
    Multicultural,
    CulturedCity,
    CulturedNation,
    Metropolis,
    Literate,
    LandAhoy,
}

#[derive(Clone, Copy)]
struct Achievement {
    /// Display name, and the key saved in a nation's achievement set.
    name: &'static str,
    kind: AchievementKind,
    /// Whether only the first nation to qualify earns it.
    unique: bool,
    /// The threshold: a percentage, count or score, depending on the kind.
    value: i32,
    /// Culture history awarded to each nation that earns it.
    culture: i32,
}

/// In evaluation order. Freeciv iterates achievements first, then players.
const ACHIEVEMENTS: [Achievement; 8] = [
    Achievement {
        name: "Spaceship Launch",
        kind: AchievementKind::Spaceship,
        unique: false,
        value: 0,
        culture: 5000,
    },
    Achievement {
        name: "Entire Map Known",
        kind: AchievementKind::MapKnown,
        unique: false,
        value: 100,
        culture: 1000,
    },
    Achievement {
        name: "Land Ahoy",
        kind: AchievementKind::LandAhoy,
        unique: false,
        value: 2,
        culture: 100,
    },
    Achievement {
        name: "Literate",
        kind: AchievementKind::Literate,
        unique: false,
        value: 100,
        culture: 1000,
    },
    Achievement {
        name: "Multicultural",
        kind: AchievementKind::Multicultural,
        unique: false,
        value: 2,
        culture: 1000,
    },
    Achievement {
        name: "Metropolis",
        kind: AchievementKind::Metropolis,
        unique: false,
        value: 20,
        culture: 1000,
    },
    Achievement {
        name: "Cultured City",
        kind: AchievementKind::CulturedCity,
        unique: false,
        value: 1000,
        culture: 1000,
    },
    Achievement {
        name: "Cultured Nation",
        kind: AchievementKind::CulturedNation,
        unique: false,
        value: 10_000,
        culture: 0,
    },
];

impl Achievement {
    fn rule_name(&self) -> &str {
        self.name
    }

    fn name(&self) -> &str {
        self.name
    }

    fn kind(&self) -> AchievementKind {
        self.kind
    }

    fn unique(&self) -> bool {
        self.unique
    }

    fn value(&self) -> i32 {
        self.value
    }

    fn culture(&self) -> i32 {
        self.culture
    }
}

impl Simulation {
    pub(super) fn continent_labels(&self) -> std::sync::Arc<[usize]> {
        self.planning.continents(&self.tiles, self.map_wrap)
    }

    fn map_known_at_percent(&self, owner: usize, percent: i32) -> bool {
        // common/achievements.c rounds the unknown allowance down first, then
        // derives the minimum whole-tile count needed for the requested share.
        let total = self.tiles.len() as i64;
        let max_unknown = total * (100 - i64::from(percent)) / 100;
        let required = total - max_unknown;
        let mut known = 0_i64;
        let mut unknown = 0_i64;
        for tile in &self.tiles {
            if tile.known_by[owner] {
                known += 1;
                if known >= required {
                    return true;
                }
            } else {
                unknown += 1;
                if unknown >= max_unknown {
                    return false;
                }
            }
        }
        false
    }

    fn multicultural_nationality_count(&self, owner: usize) -> usize {
        let mut seen = std::collections::BTreeSet::new();
        for city in self.cities_owned_by(owner) {
            for &(nationality, count) in &city.foreign_citizens {
                if count > 0 {
                    seen.insert(nationality);
                }
            }
            if city.domestic_citizens() > 0 {
                seen.insert(owner);
            }
        }
        seen.len()
    }

    fn has_citizens(&self, owner: usize) -> bool {
        self.cities_owned_by(owner).any(|city| city.size > 0)
    }

    fn achievement_is_met(
        &mut self,
        owner: usize,
        achievement: Achievement,
        labels: &[usize],
    ) -> bool {
        match achievement.kind() {
            AchievementKind::Spaceship => self.spaceships[owner].launched(),
            AchievementKind::MapKnown => self.map_known_at_percent(owner, achievement.value()),
            AchievementKind::Multicultural => {
                let value = achievement.value();
                if value <= 0 {
                    self.has_citizens(owner)
                } else {
                    self.multicultural_nationality_count(owner) >= value as usize
                }
            }
            AchievementKind::CulturedCity => self
                .cities_owned_by(owner)
                .any(|city| self.city_culture_score(city) >= i64::from(achievement.value())),
            AchievementKind::CulturedNation => {
                self.civilization_culture(owner) >= i64::from(achievement.value())
            }
            AchievementKind::Metropolis => self
                .cities_owned_by(owner)
                .any(|city| city.size >= achievement.value()),
            AchievementKind::Literate => {
                self.nation_scores[owner].score_literacy >= achievement.value()
            }
            AchievementKind::LandAhoy => {
                let mut seen = vec![false; labels.iter().copied().max().unwrap_or(0) + 1];
                let mut count = 0;
                for (tile, &label) in self.tiles.iter().zip(labels) {
                    if label == 0 || !tile.known_by[owner] || seen[label] {
                        continue;
                    }
                    seen[label] = true;
                    count += 1;
                    if count >= achievement.value() {
                        return true;
                    }
                }
                false
            }
        }
    }

    fn has_achievement(&self, owner: usize, achievement: Achievement) -> bool {
        self.nation_scores[owner]
            .achievements
            .contains(achievement.rule_name())
    }

    fn achievement_claimed(&self, achievement: Achievement) -> bool {
        self.nation_scores
            .iter()
            .any(|score| score.achievements.contains(achievement.rule_name()))
    }

    fn award_achievement(&mut self, owner: usize, achievement: Achievement) {
        self.nation_scores[owner]
            .achievements
            .insert(achievement.rule_name().to_owned());
        if owner == HUMAN {
            self.add_event(format!("Achievement earned: {}.", achievement.name()));
        }
    }

    pub(super) fn check_achievements(&mut self) {
        let labels = self.continent_labels();

        // Freeciv iterates achievement records first, then player slots. A
        // cohort which claims a nonunique achievement is awarded together;
        // the first claimant is selected with the source fc_rand draw.
        for achievement in ACHIEVEMENTS {
            let already_claimed = self.achievement_claimed(achievement);
            let unique = achievement.unique();
            let mut achievers = Vec::new();
            for owner in 0..self.players.len() {
                if self.has_achievement(owner, achievement) || (unique && already_claimed) {
                    continue;
                }
                if self.achievement_is_met(owner, achievement, &labels) {
                    achievers.push(owner);
                }
            }
            if achievers.is_empty() {
                continue;
            }

            // achievement_plr() selects one initial first claimant even for
            // nonunique achievements. Later players do not consume this draw.
            // fc_rand(1) is intentionally a no-op, as rand_below(1) is here.
            let first = if already_claimed {
                None
            } else {
                Some(achievers[self.rand_below(achievers.len() as i32) as usize])
            };
            if unique {
                let winner = first.expect("a unique unclaimed achievement has a first claimant");
                self.award_achievement(winner, achievement);
                self.players[winner].history += achievement.culture();
            } else {
                for owner in achievers {
                    self.award_achievement(owner, achievement);
                    self.players[owner].history += achievement.culture();
                }
                let _first_claimant = first;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn achievement(name: &str) -> Achievement {
        *ACHIEVEMENTS
            .iter()
            .find(|achievement| achievement.name == name)
            .unwrap()
    }

    #[test]
    fn achievement_rewards_and_thresholds_match_the_pinned_ruleset() {
        assert_eq!(ACHIEVEMENTS.len(), 8);
        assert_eq!(achievement("Spaceship Launch").culture(), 5000);
        assert_eq!(achievement("Entire Map Known").value(), 100);
        assert_eq!(achievement("Land Ahoy").value(), 2);
        assert_eq!(achievement("Metropolis").value(), 20);
        assert_eq!(achievement("Cultured City").value(), 1000);
        assert_eq!(achievement("Cultured Nation").value(), 10_000);
        assert_eq!(achievement("Multicultural").culture(), 1000);
        assert!(!achievement("Entire Map Known").unique());
    }

    #[test]
    fn map_known_threshold_uses_source_rounding_for_whole_tiles() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.tiles.truncate(7);
        for tile in &mut game.tiles {
            tile.known_by.set(HUMAN, false);
        }
        game.tiles[0].known_by.set(HUMAN, true);
        assert!(!game.map_known_at_percent(HUMAN, 25));
        game.tiles[1].known_by.set(HUMAN, true);
        assert!(game.map_known_at_percent(HUMAN, 25));
    }

    #[test]
    fn defeated_nonbarbarian_can_earn_an_unclaimed_culture_achievement() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.retain(|unit| unit.owner != HUMAN);
        game.cities.retain(|city| city.owner != HUMAN);
        game.players[HUMAN].history = 10_000;
        assert!(!game.player_alive(HUMAN));

        game.check_achievements();

        assert!(
            game.nation_scores[HUMAN]
                .achievements
                .contains("Cultured Nation")
        );
        assert_eq!(game.players[HUMAN].history, 10_000);
    }

    #[test]
    fn simultaneous_nonunique_claimants_consume_one_first_awarder_draw() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.units.clear();
        for tile in &mut game.tiles {
            tile.known_by = [false; PLAYER_SLOTS].into();
        }
        game.players[HUMAN].history = 10_000;
        game.players[1].history = 10_000;

        let mut expected = game.clone();
        expected.rand_below(2);
        game.check_achievements();

        assert!(
            game.nation_scores[HUMAN]
                .achievements
                .contains("Cultured Nation")
        );
        assert!(
            game.nation_scores[1]
                .achievements
                .contains("Cultured Nation")
        );
        assert_eq!(game.random_snapshot(), expected.random_snapshot());

        let after_claim = game.random_snapshot();
        game.check_achievements();
        assert_eq!(game.random_snapshot(), after_claim);
    }

    #[test]
    fn culture_and_literacy_achievements_are_awarded_once() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let city = &mut game.cities[0];
        city.history = 10_000;
        city.size = 20;
        city.buildings
            .extend([Building::LIBRARY, Building::UNIVERSITY]);
        game.refresh_score_snapshot();
        assert_eq!(game.nation_scores[HUMAN].score_literacy, 100);
        let boundary_culture = game.nation_scores[HUMAN].score_culture;
        let expected_reward: i32 = ["Literate", "Metropolis", "Cultured City", "Cultured Nation"]
            .iter()
            .map(|name| achievement(name).culture())
            .sum();
        game.check_achievements();
        let score = &game.nation_scores[HUMAN];
        for name in ["Literate", "Metropolis", "Cultured City", "Cultured Nation"] {
            assert!(score.achievements.contains(name));
        }
        assert_eq!(game.players[HUMAN].history, expected_reward);
        assert_eq!(game.nation_scores[HUMAN].score_culture, boundary_culture);
        let history = game.players[HUMAN].history;
        game.check_achievements();
        assert_eq!(game.players[HUMAN].history, history);
    }

    #[test]
    fn literate_achievement_uses_the_saved_begin_turn_score_snapshot() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        game.cities[0].size = 20;
        game.cities[0]
            .buildings
            .extend([Building::LIBRARY, Building::UNIVERSITY]);
        game.refresh_score_snapshot();
        assert_eq!(game.nation_scores[HUMAN].score_literacy, 100);

        // Live science output changes after begin_turn; the current phase
        // still checks the snapshot computed before that phase.
        game.cities[0]
            .buildings
            .retain(|building| *building != Building::LIBRARY && *building != Building::UNIVERSITY);
        game.check_achievements();

        assert!(game.nation_scores[HUMAN].achievements.contains("Literate"));
        assert_eq!(game.nation_scores[HUMAN].score_literacy, 100);
    }

    /// Mirrors Freeciv behavior.
    #[test]
    fn reference_culture_history_unlocks_culture_achievements() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        game.cities[0].history = 10_000;
        game.check_achievements();
        assert!(
            game.nation_scores[HUMAN]
                .achievements
                .contains("Cultured City")
        );
        assert!(
            game.nation_scores[HUMAN]
                .achievements
                .contains("Cultured Nation")
        );
    }

    #[test]
    fn multicultural_achievement_counts_nationalities_across_owned_cities() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for (_, _, x, y) in Simulation::city_radius_at(10, 20) {
            game.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        game.sign_pact(HUMAN, 1, Pact::War);
        game.create_city(HUMAN, 10, 20);
        let (attacker, target) = game.multicultural_fixture();
        assert!(game.apply(Command::move_by_offset(&game, attacker, 1, 0)));
        assert!(game.cities.iter().any(|city| {
            city.id == target && city.owner == HUMAN && city.foreign_citizens == vec![(1, 2)]
        }));

        assert!(game.apply(Command::EndTurn));

        assert!(
            game.events
                .iter()
                .any(|event| { event.contains("Achievement earned: Multicultural") })
        );
    }
}
