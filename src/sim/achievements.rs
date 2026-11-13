//! Achievement checks and rewards from the selected Freeciv ruleset.

use super::*;
use crate::rules::freeciv::{RuleRecord, RulesetData};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AchievementKind {
    Spaceship,
    MapKnown,
    Multicultural,
    CulturedCity,
    CulturedNation,
    Lucky,
    Huts,
    Metropolis,
    Literate,
    LandAhoy,
    Killer,
    Rich,
}

impl AchievementKind {
    fn from_ruleset_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "spaceship" => Self::Spaceship,
            "map_known" => Self::MapKnown,
            "multicultural" => Self::Multicultural,
            "cultured_city" => Self::CulturedCity,
            "cultured_nation" => Self::CulturedNation,
            "lucky" => Self::Lucky,
            "huts" => Self::Huts,
            "metropolis" => Self::Metropolis,
            "literate" => Self::Literate,
            "land_ahoy" => Self::LandAhoy,
            "killer" => Self::Killer,
            "rich" => Self::Rich,
            _ => panic!("unsupported Freeciv achievement type {name:?}"),
        }
    }
}

#[derive(Clone, Copy)]
struct Achievement<'a> {
    record: &'a RuleRecord,
    kind: AchievementKind,
}

impl Achievement<'_> {
    fn rule_name(&self) -> &str {
        self.record.rule_name.as_str()
    }

    fn name(&self) -> &str {
        self.record
            .display_name
            .as_deref()
            .unwrap_or(&self.record.rule_name)
    }

    fn kind(&self) -> AchievementKind {
        self.kind
    }

    fn unique(&self) -> bool {
        self.record
            .boolean_field("unique")
            .unwrap_or_else(|error| panic!("invalid C2C3 achievement unique flag: {error}"))
            .unwrap_or(true)
    }

    fn value(&self) -> i32 {
        self.integer_field("value", 0)
    }

    fn culture(&self) -> i32 {
        self.integer_field("culture", 0)
    }

    fn integer_field(&self, field: &str, default: i32) -> i32 {
        let value = self
            .record
            .integer_field(field)
            .unwrap_or_else(|error| panic!("invalid C2C3 achievement field: {error}"))
            .unwrap_or(i64::from(default));
        i32::try_from(value).unwrap_or_else(|_| {
            panic!(
                "C2C3 achievement {} field {field} is outside i32 range: {value}",
                self.rule_name()
            )
        })
    }
}

impl Simulation {
    pub(super) fn continent_labels(&self) -> Vec<usize> {
        let mut labels = vec![0; self.tiles.len()];
        let mut continent = 0;
        for index in 0..self.tiles.len() {
            if labels[index] != 0 || self.tiles[index].terrain.is_water() {
                continue;
            }
            continent += 1;
            labels[index] = continent;
            let mut pending = vec![self.map_wrap.position(index)];
            while let Some((x, y)) = pending.pop() {
                for (nx, ny) in self.map_wrap.civ3_neighbors(x, y) {
                    let index = self.map_wrap.index(nx, ny);
                    if labels[index] == 0 && !self.tiles[index].terrain.is_water() {
                        labels[index] = continent;
                        pending.push((nx, ny));
                    }
                }
            }
        }
        labels
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
        achievement: Achievement<'_>,
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
            AchievementKind::Lucky => self.rand_below(10_000) < achievement.value(),
            AchievementKind::Huts => {
                // C2C3 defines no Huts achievement; this campaign does not
                // persist Freeciv's per-player huts-entered counter.
                panic!("Freeciv Huts achievement lacks a CivCraft campaign counter")
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
            AchievementKind::Killer => {
                self.nation_scores[owner].units_killed >= achievement.value()
            }
            AchievementKind::Rich => self.players[owner].gold >= achievement.value(),
        }
    }

    fn has_achievement(&self, owner: usize, achievement: Achievement<'_>) -> bool {
        self.nation_scores[owner]
            .achievements
            .contains(achievement.rule_name())
    }

    fn achievement_claimed(&self, achievement: Achievement<'_>) -> bool {
        self.nation_scores
            .iter()
            .any(|score| score.achievements.contains(achievement.rule_name()))
    }

    fn award_achievement(&mut self, owner: usize, achievement: Achievement<'_>) {
        self.nation_scores[owner]
            .achievements
            .insert(achievement.rule_name().to_owned());
        if owner == HUMAN {
            self.add_event(format!("Achievement earned: {}.", achievement.name()));
        }
    }

    pub(super) fn check_achievements(&mut self) {
        let rules = RulesetData::campaign_default();
        let achievements: Vec<_> = rules
            .rule_records
            .iter()
            .filter(|record| record.namespace == "achievement")
            .map(|record| {
                let type_name = record
                    .string_field("type")
                    .unwrap_or_else(|error| panic!("invalid C2C3 achievement type: {error}"))
                    .unwrap_or_else(|| {
                        panic!("C2C3 achievement {} is missing its type", record.rule_name)
                    });
                Achievement {
                    record,
                    kind: AchievementKind::from_ruleset_name(type_name),
                }
            })
            .collect();
        let labels = self.continent_labels();

        // Freeciv iterates achievement records first, then player slots. A
        // cohort which claims a nonunique achievement is awarded together;
        // the first claimant is selected with the source fc_rand draw.
        for achievement in achievements {
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

    fn achievement(rule_name: &str) -> Achievement<'static> {
        let record = RulesetData::campaign_default()
            .rule_records
            .iter()
            .find(|record| {
                record.namespace == "achievement"
                    && record
                        .section
                        .eq_ignore_ascii_case(&format!("achievement_{rule_name}"))
            })
            .unwrap();
        let type_name = record.string_field("type").unwrap().unwrap();
        Achievement {
            record,
            kind: AchievementKind::from_ruleset_name(type_name),
        }
    }

    #[test]
    fn achievement_rewards_and_thresholds_come_from_civ2civ3_records() {
        let records: Vec<_> = RulesetData::campaign_default()
            .rule_records
            .iter()
            .filter(|record| record.namespace == "achievement")
            .collect();
        assert_eq!(records.len(), 8);
        assert_eq!(achievement("spaceship").culture(), 5000);
        assert_eq!(achievement("map_known").value(), 100);
        assert_eq!(achievement("another_island").value(), 2);
        assert_eq!(achievement("metropolis").value(), 20);
        assert_eq!(achievement("cultured_city").value(), 1000);
        assert_eq!(achievement("cultured_nation").value(), 10_000);
        assert_eq!(achievement("multicultural").culture(), 1000);
        assert!(!achievement("map_known").unique());
    }

    #[test]
    fn map_known_threshold_uses_source_rounding_for_whole_tiles() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.tiles.truncate(7);
        for tile in &mut game.tiles {
            tile.known_by[HUMAN] = false;
        }
        game.tiles[0].known_by[HUMAN] = true;
        assert!(!game.map_known_at_percent(HUMAN, 25));
        game.tiles[1].known_by[HUMAN] = true;
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
            tile.known_by = [false; PLAYER_SLOTS];
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
        let expected_reward: i32 = ["literate", "metropolis", "cultured_city", "cultured_nation"]
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

    #[test]
    fn freeciv_culture_history_unlocks_culture_achievements() {
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
        game.create_city(HUMAN, 10, 20);
        let (attacker, target) = game.multicultural_playtest_setup();
        assert!(game.apply(Command::Move(attacker, 1, 0)));
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
