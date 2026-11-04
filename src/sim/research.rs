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
        self.research_path(owner, self.players[owner].research_goal)
            .first()
            .copied()
            .or_else(|| {
                Tech::all()
                    .filter(|&tech| self.tech_available(owner, tech))
                    .min_by_key(|tech| tech.cost())
            })
            .unwrap_or(Tech::FUTURE)
    }

    pub(crate) fn target_cost(&self, owner: usize, tech: Tech) -> i32 {
        if tech == Tech::FUTURE {
            let player = &self.players[owner];
            research_cost(player.known.len() as i32 + player.future_techs + 1)
        } else {
            tech.cost()
        }
    }

    pub(super) fn discover(&mut self, owner: usize, tech: Tech) {
        if tech == Tech::FUTURE {
            self.players[owner].future_techs += 1;
        } else {
            self.players[owner].known.push(tech);
        }
        if owner == HUMAN {
            self.add_event(format!("Research complete: {}.", tech.name()));
        }
    }
}
