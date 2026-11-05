use super::*;

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
    }

    pub(crate) fn tech_available(&self, owner: usize, tech: Tech) -> bool {
        if tech.data().is_none() && tech != Tech::FUTURE {
            return false;
        }
        if tech == Tech::FUTURE {
            return Tech::all().all(|advance| self.has_tech(owner, advance));
        }
        !self.has_tech(owner, tech)
            && tech
                .prerequisites()
                .iter()
                .all(|&req| self.has_tech(owner, req))
    }

    pub(super) fn research_path(&self, owner: usize, goal: Tech) -> Vec<Tech> {
        if goal.data().is_none() && goal != Tech::FUTURE {
            return Vec::new();
        }
        fn visit(game: &Simulation, owner: usize, tech: Tech, path: &mut Vec<Tech>) {
            if game.has_tech(owner, tech) || path.contains(&tech) {
                return;
            }
            for &req in tech.prerequisites() {
                visit(game, owner, req, path);
            }
            path.push(tech);
        }

        let mut path = Vec::new();
        if goal == Tech::FUTURE {
            for advance in Tech::all() {
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

    fn select_ai_research(&mut self, owner: usize) -> (Tech, i32, i32) {
        if !self.players[owner].ai {
            return (self.research_target(owner), 0, 0);
        }
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

    fn source_research_scores(wants: &[f32], paths: &[Vec<Tech>]) -> (Vec<i32>, Vec<i32>) {
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

    pub(crate) fn native_unset_research_goal_playtest(&self) -> (bool, bool, bool) {
        let mut sim = self.clone();
        sim.players[1].known.clear();
        sim.players[1].researching = Tech::BRONZE_WORKING;
        sim.players[1].research_goal = Tech::POTTERY;
        sim.players[1].research = 17;
        let cache = sim.ai_tech_wants.clone();
        sim.discover(1, Tech::POTTERY);
        let gift_clears = sim.players[1].research_goal == Tech::UNSET
            && sim.players[1].researching == Tech::BRONZE_WORKING
            && sim.players[1].research == 17
            && sim.ai_tech_wants == cache;

        sim.players[1].known.clear();
        sim.players[1].research_goal = Tech::GUNPOWDER;
        sim.ai_tech_wants
            .insert(1, vec![-1000.5; Tech::all().count()]);
        sim.select_ai_research(1);
        let negative_clears = sim.players[1].research_goal == Tech::UNSET;
        let known = sim.players[1].known.clone();
        let bulbs = sim.players[1].research;
        sim.discover(1, Tech::UNSET);
        let sentinel = Tech::UNSET.0 == Tech::FUTURE.0 + 1
            && !sim.tech_available(1, Tech::UNSET)
            && sim.research_path(1, Tech::UNSET).is_empty()
            && sim.target_cost(1, Tech::UNSET) == 0
            && sim.players[1].known == known
            && sim.players[1].research == bulbs
            && sim.research_target(1) != Tech::UNSET;
        (gift_clears, negative_clears, sentinel)
    }

    pub(crate) fn native_research_arithmetic_playtest(&self) -> (bool, bool, bool) {
        let paths = vec![
            vec![Tech(0)],
            vec![Tech(1)],
            vec![Tech(0), Tech(1), Tech(2)],
            vec![Tech(1), Tech(3)],
        ];
        // Golden vectors compiled from literal C compound assignments with
        // float wants and int values/goal_values (daitech.c123-153).
        let positive = Self::source_research_scores(&[1.75, -1.5, 7.75, -3.125], &paths)
            == (vec![3, 0, 7, -3], vec![3, 0, 3, -1]);
        let negative = Self::source_research_scores(&[-4.75, 2.5, -7.75, 3.125], &paths)
            == (vec![-6, 1, -7, 3], vec![-6, 1, -4, 2]);
        let precision = Self::source_research_scores(&[16_777_216.0, 0.0, 4.75, 0.75], &paths)
            == (vec![16_777_218, 1, 4, 0], vec![16_777_218, 1, 5_592_407, 0]);
        (positive, negative, precision)
    }

    pub(crate) fn ai_research_switching_playtest(&mut self) -> bool {
        self.units.clear();
        let home = (10, 24);
        for (x, y) in super::terrain_work::neighbors(home.0, home.1).chain([home]) {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.river = true;
            tile.road = true;
        }
        self.create_city(1, home.0, home.1);
        self.cities[0].size = 4;
        self.reconcile_city_workers();
        self.players[1].known.clear();
        // Advisor selection can prefer a deep advance which choose_tech will
        // reject. Bring that candidate to the frontier for the switching checks.
        let best = loop {
            self.ai_tech_wants.insert(1, self.advisor_tech_wants(1));
            let (candidate, _, _) = self.select_ai_research(1);
            if self.tech_available(1, candidate) {
                break candidate;
            }
            for tech in self.research_path(1, candidate) {
                if tech != candidate && !self.has_tech(1, tech) {
                    self.players[1].known.push(tech);
                }
            }
        };
        let Some(alternative) = Tech::all().find(|&tech| {
            if tech == best || !self.tech_available(1, tech) || self.target_cost(1, tech) <= 6 {
                return false;
            }
            self.players[1].researching = tech;
            let (_, want, current) = self.select_ai_research(1);
            want - current > 3
        }) else {
            return false;
        };
        self.players[1].researching = alternative;
        let near_completion = self.target_cost(1, alternative) - 1;
        self.players[1].research = near_completion;
        self.manage_ai_research(1);
        let held = self.players[1].researching == alternative
            && self.players[1].research == near_completion;
        self.players[1].research = 1;
        self.manage_ai_research(1);
        let switched = self.players[1].researching == best && self.players[1].research == 0;
        self.players[1].researching = alternative;
        self.players[1].research = 3;
        self.ai_free_bulbs.insert(1, 3);
        self.manage_ai_research(1);
        held && switched && self.players[1].researching == best && self.players[1].research == 3
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
        if tech.data().is_none() && tech != Tech::FUTURE {
            return 0;
        }
        let player = &self.players[owner];
        let base = if tech == Tech::FUTURE {
            research_cost(player.known.len() as i32 + player.future_techs + 1)
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
            .filter_map(|tech| tech.data())
            .map(|advance| {
                i64::from((advance.requirement_count * BASE_TECH_COST).max(MIN_TECH_COST))
            })
            .sum();
        let future = i64::from(player.future_techs);
        let known = player.known.len() as i64 + 1;
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
        if tech.data().is_none() && tech != Tech::FUTURE {
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
mod tests {
    use super::*;

    #[test]
    fn unset_goal_is_distinct_and_clears_on_gift_and_negative_advisor_values() {
        let sim = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            sim.native_unset_research_goal_playtest(),
            (true, true, true)
        );
    }

    #[test]
    fn human_gift_clears_goal_without_allowing_unset_as_active_research() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.players[HUMAN].known.clear();
        sim.players[HUMAN].research_goal = Tech::POTTERY;
        sim.players[HUMAN].researching = Tech::BRONZE_WORKING;
        sim.players[HUMAN].research = 9;
        sim.discover(HUMAN, Tech::POTTERY);
        assert_eq!(sim.players[HUMAN].research_goal, Tech::UNSET);
        assert_eq!(sim.players[HUMAN].researching, Tech::BRONZE_WORKING);
        assert_eq!(sim.players[HUMAN].research, 9);
        assert!(!sim.apply(Command::Research(Tech::UNSET)));
        assert_eq!(sim.players[HUMAN].research_goal, Tech::UNSET);
        assert!(sim.apply(Command::Research(Tech::ALPHABET)));
        assert_eq!(sim.players[HUMAN].research_goal, Tech::ALPHABET);
    }

    #[test]
    fn actual_human_science_completion_clears_goal_and_retains_valid_automatic_target() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.players[HUMAN].known.clear();
        sim.players[HUMAN].research_goal = Tech::POTTERY;
        sim.players[HUMAN].researching = Tech::POTTERY;
        sim.players[HUMAN].research = 0;
        let cost = sim.target_cost(HUMAN, Tech::POTTERY);
        sim.resolve_research_turn(HUMAN, cost);
        assert!(sim.has_tech(HUMAN, Tech::POTTERY));
        assert_eq!(sim.players[HUMAN].research_goal, Tech::UNSET);
        assert_ne!(sim.players[HUMAN].researching, Tech::UNSET);
        assert!(sim.tech_available(HUMAN, sim.players[HUMAN].researching));
    }

    #[test]
    fn future_research_retains_its_project_but_clears_its_advisor_goal() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.players[1].known = Tech::all().collect();
        sim.players[1].researching = Tech::FUTURE;
        sim.players[1].research_goal = Tech::FUTURE;
        assert_eq!(sim.select_ai_research(1), (Tech::FUTURE, 1, 1));
        assert_eq!(sim.players[1].research_goal, Tech::UNSET);
        sim.discover(1, Tech::FUTURE);
        assert_eq!(sim.players[1].researching, Tech::FUTURE);
        assert_eq!(sim.players[1].research_goal, Tech::UNSET);
        assert_eq!(sim.players[1].future_techs, 1);
    }

    #[test]
    fn native_research_integer_and_float_compound_vectors_match_c() {
        let sim = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            sim.native_research_arithmetic_playtest(),
            (true, true, true)
        );
        let paths = vec![
            vec![],
            vec![Tech(1)],
            vec![Tech(1), Tech(2)],
            vec![Tech(1), Tech(3)],
        ];
        assert_eq!(
            Simulation::source_research_scores(&[1.75, -1.5, 7.75, -3.125], &paths),
            (vec![0, 0, 7, -3], vec![-1, 0, 3, -1])
        );
        // Native choice.want is assigned an INTEGER per-city quotient.
        let (values, _) = Simulation::source_research_scores(
            &[-4.75, 2.5, -7.75, 3.125],
            &[
                vec![Tech(0)],
                vec![Tech(1)],
                vec![Tech(0), Tech(1), Tech(2)],
                vec![Tech(1), Tech(3)],
            ],
        );
        assert_eq!(
            values.iter().map(|value| value / 2).collect::<Vec<_>>(),
            vec![-3, 0, -3, 1]
        );
    }

    #[test]
    fn player_research_keeps_the_selected_goal_after_an_advance() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[HUMAN].known.clear();
        game.players[HUMAN].research_goal = Tech::GUNPOWDER;
        game.players[HUMAN].researching = game.research_target(HUMAN);
        let first = game.players[HUMAN].researching;
        game.discover(HUMAN, first);
        let expected = game.research_target(HUMAN);
        assert_eq!(game.next_ai_research_target(HUMAN), expected);
        assert_eq!(game.players[HUMAN].research_goal, Tech::GUNPOWDER);
    }

    #[test]
    fn switching_respects_investment_and_exempts_discovery_overflow() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_research_switching_playtest());
    }

    #[test]
    fn blocked_advisor_choice_is_rejected_without_a_frontier_substitute() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        game.create_city(1, 10, 24);
        game.players[1].known.clear();
        game.players[1].research = 0;
        game.ai_free_bulbs.clear();
        game.ai_tech_wants.insert(1, game.advisor_tech_wants(1));
        let (choice, _, _) = game.select_ai_research(1);
        assert!(
            !game.tech_available(1, choice),
            "fixture must prefer a deep advance: {}",
            choice.name()
        );
        let current = Tech::all()
            .find(|&tech| game.tech_available(1, tech))
            .unwrap();
        game.players[1].researching = current;
        game.manage_ai_research(1);
        assert_eq!(game.players[1].researching, current);
        assert_eq!(game.players[1].research, 0);
    }

    #[test]
    fn completed_project_follows_goal_without_running_advisor_selection() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[1].known.clear();
        game.players[1].research_goal = Tech::GUNPOWDER;
        game.players[1].researching = Tech::BRONZE_WORKING;
        game.discover(1, Tech::BRONZE_WORKING);
        assert_eq!(game.players[1].research_goal, Tech::GUNPOWDER);
        assert!(game.tech_available(1, game.players[1].researching));
        assert!(
            game.research_path(1, Tech::GUNPOWDER)
                .contains(&game.players[1].researching)
        );
    }

    #[test]
    fn completed_goal_uses_reservoir_sampling_in_reference_draw_order() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[1].known.clear();
        game.players[1].research_goal = Tech::BRONZE_WORKING;
        game.players[1].researching = Tech::BRONZE_WORKING;
        let initial_rng = game.random_snapshot();
        game.players[1].known.push(Tech::BRONZE_WORKING);
        let mut sampled = Tech::FUTURE;
        let mut count = 0;
        for tech in Tech::all() {
            if game.tech_available(1, tech) {
                count += 1;
                if game.rand_below(count) == 0 {
                    sampled = tech;
                }
            }
        }
        let expected_rng = game.random_snapshot();
        game.restore_random(initial_rng);
        game.players[1].known.clear();
        game.discover(1, Tech::BRONZE_WORKING);
        assert_eq!(game.players[1].researching, sampled);
        assert_eq!(game.random_snapshot(), expected_rng);
        assert_eq!(game.players[1].research_goal, Tech::UNSET);
    }

    #[test]
    fn unrelated_discovery_does_not_exempt_existing_research() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.ai_free_bulbs.clear();
        game.players[1].researching = Tech::BRONZE_WORKING;
        game.players[1].research = 12;
        game.discover(1, Tech::POTTERY);
        assert!(!game.ai_free_bulbs.contains_key(&1));
        game.discover(1, Tech::BRONZE_WORKING);
        assert_eq!(game.ai_free_bulbs.get(&1), Some(&12));
    }

    #[test]
    fn discovery_overflow_can_complete_the_next_goal_in_the_same_update() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[1].known = Tech::all()
            .filter(|&tech| tech != Tech::BRONZE_WORKING && tech != Tech::ALPHABET)
            .collect();
        game.players[1].research_goal = Tech::ALPHABET;
        game.players[1].researching = Tech::BRONZE_WORKING;
        game.players[1].research =
            game.target_cost(1, Tech::BRONZE_WORKING) + game.target_cost(1, Tech::ALPHABET) + 7;
        let upkeep = game.technology_upkeep(1);
        game.resolve_research_turn(1, upkeep);
        assert!(game.has_tech(1, Tech::BRONZE_WORKING));
        assert!(game.has_tech(1, Tech::ALPHABET));
        assert_eq!(game.players[1].researching, Tech::FUTURE);
        assert_eq!(game.players[1].research, 7);
        assert_eq!(game.ai_free_bulbs[&1], 7);
    }

    #[test]
    fn completed_ai_tree_continues_with_future_technology() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[1].known = Tech::all().collect();
        game.players[1].researching = Tech::FUTURE;
        assert_eq!(game.next_ai_research_target(1), Tech::FUTURE);
    }
}
