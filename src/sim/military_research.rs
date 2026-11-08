//! Threat-counter research feedback from daitech.c:dai_wants_defender_against.
use super::unit_catalog::UnitFlag;
use super::*;

#[derive(Clone, Copy)]
struct CounterCandidate {
    defense: i32,
    available: bool,
    later: bool,
    goal: Option<(Tech, i32)>,
}

fn choose_counter(candidates: &[CounterCandidate], mut want: i32) -> Option<(Tech, i32)> {
    let mut best_available = None;
    let mut available_defense = 0;
    for candidate in candidates {
        if candidate.available && candidate.defense > available_defense {
            available_defense = candidate.defense;
            best_available = Some(candidate);
        }
    }
    let mut best = None;
    let mut cost = i32::MAX;
    for candidate in candidates {
        if candidate.defense <= available_defense || candidate.available || !candidate.later {
            continue;
        }
        if let Some((tech, bulbs)) = candidate.goal
            && bulbs < cost
        {
            cost = bulbs;
            best = Some((tech, candidate.defense));
        }
    }
    let (tech, defense) = best?;
    if best_available.is_some() && 2 * defense <= 3 * available_defense {
        want /= 2;
    }
    Some((tech, want))
}

impl Simulation {
    pub(super) fn counter_defender_request(
        &self,
        city: &City,
        attacker: UnitKind,
        want: i32,
    ) -> Option<(Tech, i32)> {
        // The native function queries Defend_Bonus using ATTACKER type as
        // the primary type context, deliberately not a virtual defender.
        let bonus = 100 + self.advisor_city_defend_bonus(city, attacker);
        let candidates: Vec<_> = UnitKind::ALL_NATIVE
            .into_iter()
            .map(|kind| {
                // These are ALL combat-bonus rows in the bundled ruleset. There
                // are no ScramblesPct/DefenseDivider rows, so their coefficients
                // are respectively zero and100 in the native cache expression.
                let multiplier = if kind == UnitKind::AegisCruiser
                    && attacker.has_flag(UnitFlag::AirAttacker)
                {
                    500
                } else if kind == UnitKind::Destroyer && attacker.has_flag(UnitFlag::Submarine) {
                    200
                } else {
                    100
                };
                let goal = kind
                    .required_tech()
                    .filter(|&tech| !self.has_tech(city.owner, tech))
                    .and_then(|tech| {
                        let path = self.research_path(city.owner, tech);
                        (!path.is_empty()).then(|| {
                            (
                                tech,
                                path.into_iter()
                                    .map(|required| self.target_cost(city.owner, required))
                                    .sum(),
                            )
                        })
                    });
                CounterCandidate {
                    defense: kind.stats().defense * bonus * multiplier / 100,
                    available: self.unit_buildable(city, kind),
                    later: self.unit_buildable_later(city, kind),
                    goal,
                }
            })
            .collect();
        // No supplied unit has a building prerequisite: the native
        // utype_needs_improvement branch cannot run for this fixed ruleset.
        choose_counter(&candidates, want)
    }

    pub(super) fn counter_defender_tech_feedback(&self, city: &City) -> Vec<(Tech, f32)> {
        let mut feedback = Vec::new();
        for (enemy, _, turns) in self.city_threat_arrivals(city) {
            let want = self.advisor_threat_vulnerability(city, enemy) / turns.max(1);
            if let Some((tech, delta)) = self.counter_defender_request(city, enemy.kind, want) {
                feedback.push((tech, delta as f32));
            }
        }
        feedback
    }

    pub(super) fn counter_defender_tech_wants(&self, city: &City) -> Vec<f32> {
        let mut wants = vec![0.0_f32; Tech::all().count()];
        for (tech, delta) in self.counter_defender_tech_feedback(city) {
            wants[tech.0] += delta;
        }
        wants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attacker_specific_aegis_bonus_requests_a_nominally_weaker_defender() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Sea;
        }
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        let goal = UnitKind::AegisCruiser.required_tech().unwrap();
        game.players[1].known = Tech::all().filter(|&tech| tech != goal).collect();
        game.create_city(1, 10, 24);
        assert_eq!(
            game.counter_defender_request(&game.cities[0], UnitKind::Fighter, 101),
            Some((goal, 101))
        );
        assert_eq!(
            game.counter_defender_request(&game.cities[0], UnitKind::Warrior, 101),
            None
        );
    }

    #[test]
    fn counter_selection_matches_literal_native_c_vectors() {
        for line in include_str!("../../tests/fixtures/freeciv_counter_research_vectors.tsv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let x: Vec<i32> = line
                .split_whitespace()
                .map(|word| word.parse().unwrap())
                .collect();
            let mut candidates = Vec::new();
            for unit in x[1..25].chunks(6) {
                candidates.push(CounterCandidate {
                    defense: unit[0] * 100,
                    available: unit[1] != 0,
                    later: unit[2] != 0,
                    goal: (unit[3] >= 0 && unit[5] != 0)
                        .then_some((Tech(unit[3] as usize), unit[4])),
                });
            }
            let actual = choose_counter(&candidates, x[0]);
            let expected = (x[25] >= 0).then_some((Tech(x[25] as usize), x[26]));
            assert_eq!(actual, expected, "{line}");
        }
    }

    #[test]
    fn threat_counter_feedback_uses_the_source_assessment_without_a_second_danger_heuristic() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.counter_defender_research_playtest(),
            (true, true, true)
        );
    }

    #[test]
    fn counter_half_threshold_and_first_native_equal_cost_tie_are_exact() {
        let candidate = |defense, available, tech: Option<usize>| CounterCandidate {
            defense,
            available,
            later: true,
            goal: tech.map(|tech| (Tech(tech), 100)),
        };
        let base = candidate(2, true, None);
        assert_eq!(
            choose_counter(&[base, candidate(3, false, Some(1))], 101),
            Some((Tech(1), 50))
        );
        assert_eq!(
            choose_counter(&[base, candidate(4, false, Some(1))], 101),
            Some((Tech(1), 101))
        );
        assert_eq!(
            choose_counter(
                &[
                    base,
                    candidate(3, false, Some(1)),
                    candidate(9, false, Some(2))
                ],
                101
            ),
            Some((Tech(1), 50))
        );
    }
}
