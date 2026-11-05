use super::*;

impl Simulation {
    pub(crate) fn has_tech(&self, owner: usize, tech: Tech) -> bool {
        self.players[owner].known.contains(&tech)
    }

    pub(crate) fn tech_available(&self, owner: usize, tech: Tech) -> bool {
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
        let goal = if ferry_want > 0 && ferry_want > self.advisor_tech_want(owner, goal) {
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
        let baseline = self.research_target(owner);
        let competing = self
            .advisor_tech_want(owner, self.players[owner].research_goal)
            .max(self.ferry_tech_want(owner, Tech::MAP_MAKING));
        let defensive = self.counterintelligence_tech_want(owner);
        let government = self.government_research_want(owner);
        if defensive > competing && government.is_none_or(|(_, want)| defensive >= want) {
            return Tech::ALPHABET;
        }
        if let Some((goal, want)) = government
            && want > competing
            && let Some(&next) = self.research_path(owner, goal).first()
        {
            return next;
        }
        baseline
    }

    pub(crate) fn target_cost(&self, owner: usize, tech: Tech) -> i32 {
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
    }
    pub(super) fn resolve_research_turn(&mut self, owner: usize, science: i32) {
        self.apply_research_upkeep(owner, science);
        let current = self.players[owner].researching;
        let cost = self.target_cost(owner, current);
        if self.players[owner].research >= cost {
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
            self.players[owner].researching = self.next_ai_research_target(owner);
        }
    }
}
