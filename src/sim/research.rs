//! Technology availability, cost, and research progression.

use super::*;
use crate::rules::TechEra;

impl Simulation {
    pub(crate) fn research_unit_unlocks(tech: Tech) -> Vec<&'static str> {
        UnitKind::ALL_NATIVE
            .into_iter()
            .filter(|kind| !kind.has_no_build() && kind.required_tech() == Some(tech))
            .map(UnitKind::name)
            .collect()
    }

    pub(crate) fn has_tech(&self, owner: usize, tech: Tech) -> bool {
        self.players[owner].known.contains(&tech)
            || tech
                .civ3_equivalent()
                .is_some_and(|equivalent| self.players[owner].known.contains(&equivalent))
    }

    pub(crate) fn resource_visible_to(&self, owner: usize, resource: Resource) -> bool {
        resource
            .visibility_tech()
            .is_none_or(|tech| self.has_tech(owner, tech))
    }

    pub(crate) fn tech_available(&self, owner: usize, tech: Tech) -> bool {
        if tech.data().is_none() && tech != Tech::FUTURE {
            return false;
        }
        if tech == Tech::FUTURE {
            return Tech::all()
                .filter(|advance| advance.is_civ3_advance())
                .all(|advance| self.has_tech(owner, advance));
        }
        if !tech.is_civ3_advance() {
            return false;
        }
        !self.has_tech(owner, tech)
            && tech.era() <= self.research_era(owner)
            && tech
                .prerequisites()
                .iter()
                .all(|&req| self.has_tech(owner, req))
    }

    /// Highest Civ III era whose required advances have all been discovered.
    /// Optional advances and CivCraft extension technologies do not hold back
    /// the next era.
    pub(crate) fn research_era(&self, owner: usize) -> TechEra {
        let mut current = TechEra::Ancient;
        for era in [TechEra::Ancient, TechEra::MiddleAges, TechEra::Industrial] {
            if Tech::all()
                .filter(|tech| tech.era() == era && tech.required_for_civ3_era())
                .all(|tech| self.has_tech(owner, tech))
            {
                current = match era {
                    TechEra::Ancient => TechEra::MiddleAges,
                    TechEra::MiddleAges => TechEra::Industrial,
                    TechEra::Industrial => TechEra::Modern,
                    TechEra::Modern => unreachable!(),
                };
            } else {
                break;
            }
        }
        current
    }

    pub(super) fn research_path(&self, owner: usize, goal: Tech) -> Vec<Tech> {
        if (goal.data().is_none() || !goal.is_civ3_advance()) && goal != Tech::FUTURE {
            return Vec::new();
        }
        fn visit(game: &Simulation, owner: usize, tech: Tech, path: &mut Vec<Tech>) {
            if game.has_tech(owner, tech) || path.contains(&tech) {
                return;
            }
            if tech.is_civ3_advance() {
                for era in TechEra::ALL.iter().take(tech.era().index()) {
                    for required in Tech::all().filter(|candidate| {
                        candidate.era() == *era && candidate.required_for_civ3_era()
                    }) {
                        visit(game, owner, required, path);
                    }
                }
            }
            for &req in tech.prerequisites() {
                visit(game, owner, req, path);
            }
            path.push(tech);
        }

        let mut path = Vec::new();
        if goal == Tech::FUTURE {
            for advance in Tech::all().filter(|advance| advance.is_civ3_advance()) {
                visit(self, owner, advance, &mut path);
            }
        }
        visit(self, owner, goal, &mut path);
        path
    }

    pub(super) fn research_target(&self, owner: usize) -> Tech {
        let goal = self.players[owner].research_goal;
        let ferry_want = self.ferry_tech_want(owner, Tech::MAP_MAKING);
        let goal = if ferry_want > 0 && ferry_want as f32 > self.advisor_tech_want(owner, goal) {
            Tech::MAP_MAKING
        } else {
            goal
        };
        self.research_path(owner, goal)
            .first()
            .copied()
            .or_else(|| {
                Tech::all()
                    .filter(|&tech| self.tech_available(owner, tech))
                    .min_by_key(|tech| tech.cost())
            })
            .unwrap_or(Tech::FUTURE)
    }

    pub(super) fn next_ai_research_target(&mut self, owner: usize) -> Tech {
        self.select_ai_research(owner).0
    }

    pub(super) fn select_ai_research(&mut self, owner: usize) -> (Tech, i32, i32) {
        if !self.players[owner].ai {
            return (self.research_target(owner), 0, 0);
        }
        // Score vectors are indexed by serialized tech ID, including gaps in
        // the player-facing Civ III subset. Legacy advances have empty paths.
        let advances: Vec<_> = Tech::all().collect();
        let paths: Vec<_> = advances
            .iter()
            .map(|&tech| self.research_path(owner, tech))
            .collect();
        if self.players[owner].researching == Tech::FUTURE
            && !advances
                .iter()
                .any(|&tech| self.tech_available(owner, tech))
        {
            self.players[owner].research_goal = Tech::UNSET;
            return (Tech::FUTURE, 1, 1);
        }
        // Government, city and effect contributions already belong to the
        // phase's persistent adv_want (float) snapshot. Selection consumes it
        // without recomputing after cities or technologies have changed.
        let wants: Vec<_> = advances
            .iter()
            .map(|&tech| self.advisor_tech_want(owner, tech))
            .collect();
        let (values, goal_values) = Self::source_research_scores(&wants, &paths);
        let mut goal = None;
        let mut choice = None;
        for (index, &tech) in advances.iter().enumerate() {
            let path = &paths[index];
            if !path.is_empty() {
                let goal_value = goal_values[index];
                if goal_value > -1 && goal.is_none_or(|(_, current)| goal_value > current) {
                    goal = Some((tech, goal_value));
                }
            }
            // research_invention_gettable(..., TRUE) accepts known root
            // requirements, not ordinary prerequisites. Classic has no
            // restrictive roots; choose_tech performs the frontier check later.
            if values[index] > -1 && choice.is_none_or(|(_, current)| values[index] > current) {
                choice = Some((tech, values[index]));
            }
        }
        self.players[owner].research_goal = goal.map_or(Tech::UNSET, |(tech, _)| tech);
        let cities = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .count()
            .max(1) as i32;
        let current = values
            .get(self.players[owner].researching.0)
            .copied()
            .unwrap_or(0)
            / cities;
        let (tech, want) = choice.unwrap_or((self.players[owner].researching, -1));
        (tech, want / cities, current)
    }

    pub(in crate::sim) fn source_research_scores(
        wants: &[f32],
        paths: &[Vec<Tech>],
    ) -> (Vec<i32>, Vec<i32>) {
        let mut values = vec![0_i32; wants.len()];
        // daitech.c123-127 uses int += adv_want. Each compound addition
        // converts the existing integer to f32, adds, then truncates back to
        // int. Truncating wants once before this loop changes the result.
        for (index, path) in paths
            .iter()
            .enumerate()
            .filter(|(_, path)| !path.is_empty())
        {
            values[index] = (values[index] as f32 + wants[index]) as i32;
            for prerequisite in path.iter().filter(|&&tech| tech != Tech(index)) {
                let target = &mut values[prerequisite.0];
                *target = (*target as f32 + wants[index] / path.len() as f32) as i32;
            }
        }
        // The goal pass is entirely integer arithmetic, including division;
        // neither the path average nor the later per-city average is f32.
        let goals = paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                if path.is_empty() {
                    return -1;
                }
                let mut value = values[index];
                for prerequisite in path.iter().filter(|&&tech| tech != Tech(index)) {
                    value += values[prerequisite.0];
                }
                value / path.len() as i32
            })
            .collect();
        (values, goals)
    }

    pub(super) fn manage_ai_research(&mut self, owner: usize) {
        if !self.players[owner].ai || owner == BARBARIANS {
            return;
        }
        let (choice, want, current_want) = self.select_ai_research(owner);
        let current = self.players[owner].researching;
        if choice == current {
            return;
        }
        let researched = self.players[owner].research;
        let free = self.ai_free_bulbs.get(&owner).copied().unwrap_or(0);
        let penalty = (researched - free).max(0);
        // daitech.c:dai_manage_tech compares the per-city gain with invested
        // bulbs, and refuses to abandon a project already near completion.
        if want - current_want > penalty
            && penalty.saturating_add(researched) <= self.target_cost(owner, current)
            && self.tech_available(owner, choice)
        {
            self.players[owner].researching = choice;
            // techtools.c:choose_tech, default techpenalty=100. Bulbs carried
            // from a discovery this phase are exempt from the switching cost.
            self.players[owner].research = researched - penalty;
        }
    }

    pub(crate) fn target_cost(&self, owner: usize, tech: Tech) -> i32 {
        if (tech.data().is_none() || !tech.is_civ3_advance()) && tech != Tech::FUTURE {
            return 0;
        }
        let player = &self.players[owner];
        let base = if tech == Tech::FUTURE {
            research_cost(
                player
                    .known
                    .iter()
                    .filter(|tech| tech.is_civ3_advance())
                    .count() as i32
                    + player.future_techs
                    + 1,
            )
        } else {
            tech.cost()
        };
        let mut alive = 0;
        let mut sources = 0;
        for other in 0..self.players.len() {
            if !self.player_alive(other) {
                continue;
            }
            alive += 1;
            let knows = if tech == Tech::FUTURE {
                self.players[other].future_techs > player.future_techs
            } else {
                self.has_tech(other, tech)
            };
            if knows && self.has_embassy(owner, other) {
                sources += 1;
            }
        }
        if alive == 0 {
            base
        } else {
            (base * (alive - sources) / alive).max(1)
        }
    }

    pub(crate) fn technology_upkeep(&self, owner: usize) -> i32 {
        let player = &self.players[owner];
        let known_cost: i64 = player
            .known
            .iter()
            .filter(|tech| tech.is_civ3_advance())
            .filter_map(|tech| tech.data())
            .map(|advance| {
                i64::from((advance.requirement_count * BASE_TECH_COST).max(MIN_TECH_COST))
            })
            .sum();
        let future = i64::from(player.future_techs);
        let known = player
            .known
            .iter()
            .filter(|tech| tech.is_civ3_advance())
            .count() as i64
            + 1;
        let future_cost = if future > 0 {
            i64::from(BASE_TECH_COST) * (future * (2 * known + future + 1) + 2 * known) / 2
        } else {
            0
        };
        // Civ2Civ3 uses the default Normal science factor (100) plus the
        // ruleset's Tech_Cost_Pct (300) for maintenance calculations.
        let scaled = (known_cost + future_cost)
            * i64::from(100 + TECH_COST_PERCENT)
            * i64::from(SCIENCEBOX_PERCENT);
        let free = if player.government == Government::Anarchy {
            102
        } else {
            3
        };
        let divider = 6000 * 100 * 100;
        let cities = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .count() as i64;
        (((scaled - free * divider).max(0) * cities) / divider) as i32
    }

    pub(super) fn apply_research_upkeep(&mut self, owner: usize, income: i32) {
        let upkeep = self.technology_upkeep(owner);
        self.players[owner].last_science = income - upkeep;
        self.players[owner].research += income - upkeep;
        if self.players[owner].research >= 0 {
            return;
        }
        let lost = if self.players[owner].future_techs > 0 {
            Some(Tech::FUTURE)
        } else {
            let known = &self.players[owner].known;
            let eligible: Vec<_> = Tech::all()
                .filter(|tech| {
                    known.contains(tech)
                        && known
                            .iter()
                            .all(|other| !other.prerequisites().contains(tech))
                })
                .collect();
            if eligible.is_empty() {
                None
            } else {
                Some(eligible[self.rand_below(eligible.len() as i32) as usize])
            }
        };
        let Some(tech) = lost else {
            return;
        };
        self.players[owner].research += self.target_cost(owner, tech) / 2;
        if tech == Tech::FUTURE {
            self.players[owner].future_techs -= 1;
        } else {
            self.players[owner].known.retain(|&known| known != tech);
            self.players[owner].researching = self.research_target(owner);
        }
        if owner == HUMAN {
            self.add_event(format!(
                "Insufficient science to maintain {}. The technology was lost.",
                tech.name()
            ));
        }
        if tech != Tech::FUTURE {
            self.handle_government_tech_loss(owner, tech);
        }
    }

    pub(super) fn grant_cheapest_advance(&mut self, owner: usize) {
        let available: Vec<_> = Tech::all()
            .filter(|&tech| self.tech_available(owner, tech))
            .collect();
        let advance = if let Some(cost) = available.iter().map(|tech| tech.cost()).min() {
            let cheapest: Vec<_> = available
                .into_iter()
                .filter(|tech| tech.cost() == cost)
                .collect();
            cheapest[self.rand_below(cheapest.len() as i32) as usize]
        } else {
            Tech::FUTURE
        };
        self.discover(owner, advance);
        if self.players[owner].researching == advance {
            self.players[owner].researching = self.research_target(owner);
        }
    }

    pub(super) fn discover(&mut self, owner: usize, tech: Tech) {
        if (tech.data().is_none() || !tech.is_civ3_advance()) && tech != Tech::FUTURE {
            return;
        }
        let was_researching = self.players[owner].researching == tech;
        if self.players[owner].ai && was_researching {
            self.ai_free_bulbs
                .insert(owner, self.players[owner].research);
        }
        if tech == Tech::FUTURE {
            self.players[owner].future_techs += 1;
        } else {
            self.players[owner].known.push(tech);
            if tech == Tech::RAILROAD {
                let centers: Vec<_> = self
                    .cities
                    .iter()
                    .filter(|city| city.owner == owner)
                    .map(|city| (city.x, city.y))
                    .collect();
                for (x, y) in centers {
                    self.tile_mut(x, y).railroad = true;
                }
            }
        }
        if owner == HUMAN {
            self.add_event(format!("Research complete: {}.", tech.name()));
        }
        // techtools.c:found_new_tech clears a completed goal for every
        // research owner, regardless of whether the discovery was the
        // current project or a gift/trade.
        if self.players[owner].research_goal == tech {
            self.players[owner].research_goal = Tech::UNSET;
        }
        if self.players[owner].ai && was_researching {
            self.players[owner].researching = self.post_discovery_ai_target(owner);
        }
    }

    fn post_discovery_ai_target(&mut self, owner: usize) -> Tech {
        let goal = self.players[owner].research_goal;
        if goal.data().is_some() {
            // common/tech.c advance_req_iter traverses the first prerequisite
            // immediately and queues additional branches in encounter order.
            let mut pending = vec![goal];
            let mut done = Vec::new();
            let mut index = 0;
            while index < pending.len() {
                let tech = pending[index];
                if self.tech_available(owner, tech) {
                    return tech;
                }
                let mut first = true;
                for &req in tech.prerequisites() {
                    if !done.contains(&req) {
                        done.push(req);
                        if first {
                            pending[index] = req;
                            first = false;
                        } else {
                            pending.push(req);
                        }
                    }
                }
                if first {
                    index += 1;
                }
            }
        }
        // techtools.c:pick_random_tech uses reservoir sampling in advance-ID
        // order, including the first random draw with a bound of one.
        let mut choice = Tech::FUTURE;
        let mut count = 0;
        for tech in Tech::all() {
            if self.tech_available(owner, tech) {
                count += 1;
                if self.rand_below(count) == 0 {
                    choice = tech;
                }
            }
        }
        choice
    }

    pub(super) fn resolve_research_turn(&mut self, owner: usize, science: i32) {
        self.apply_research_upkeep(owner, science);
        loop {
            let current = self.players[owner].researching;
            let cost = self.target_cost(owner, current);
            if self.players[owner].research < cost {
                break;
            }
            self.players[owner].research -= cost;
            let first_bonus = current
                .data()
                .is_some_and(|advance| advance.first_discovery_bonus)
                && self
                    .players
                    .iter()
                    .all(|player| !player.known.contains(&current));
            self.discover(owner, current);
            if first_bonus {
                let bonus = self.research_target(owner);
                if bonus != Tech::FUTURE {
                    self.discover(owner, bonus);
                }
            }
            if !self.players[owner].ai {
                self.players[owner].researching = self.research_target(owner);
            }
        }
    }
}

#[cfg(test)]
#[path = "research/tests.rs"]
mod tests;
