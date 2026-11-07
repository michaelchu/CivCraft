use super::*;

impl Simulation {
    pub(super) fn map_size_factor(&self) -> i32 {
        (self.map_wrap.width() * self.map_wrap.height() / 1000).max(1)
    }

    pub(in crate::sim) fn ai_on_war_footing(&self, owner: usize) -> bool {
        self.ai_war_plans.iter().any(|(&(planner, target), plan)| {
            planner == owner && plan.countdown >= 0 && self.player_alive(target)
        })
    }

    pub(in crate::sim) fn ai_potentially_hostile(&self, owner: usize, other: usize) -> bool {
        owner != other
            && (self.at_war(owner, other)
                || self.ai_war_countdown(owner, other) >= 0
                || self
                    .diplomacy
                    .get(&Self::relation_key(owner, other))
                    .is_none_or(|relation| relation.first_contact.is_none()))
    }

    pub(in crate::sim) fn advisor_player_dangerous(&self, owner: usize, other: usize) -> bool {
        // advdata.c:adv_is_player_dangerous and the default-AI override.
        // Do not substitute the stricter military target predicate or the
        // alive/contact-filtered default-AI diplomatic intelligence cache.
        if self.players[owner].ai && self.ai_war_countdown(owner, other) >= 0 {
            return true;
        }
        if owner == other {
            return false;
        }
        if self.diplomatic_at_war(owner, other) || self.pact(owner, other) == Pact::Ceasefire {
            return true;
        }
        let allied_with_enemy = (0..self.players.len())
            .any(|third| self.allied(other, third) && self.diplomatic_at_war(owner, third));
        if allied_with_enemy {
            return true;
        }
        self.diplomacy
            .get(&Self::relation_key(owner, other))
            .is_none_or(|relation| {
                let direction = usize::from(owner > other);
                relation.cause_until[direction] > self.turn || relation.trust[direction] < 1000
            })
    }

    pub(super) fn ai_war_countdown(&self, owner: usize, target: usize) -> i32 {
        self.ai_war_plans
            .get(&(owner, target))
            .map_or(-1, |plan| plan.countdown)
    }

    pub(super) fn begin_ai_war_plan(
        &mut self,
        owner: usize,
        target: usize,
        delay: i32,
        reason: WarReason,
    ) {
        if self.ai_war_countdown(owner, target) != -1 {
            return;
        }
        self.ai_war_plans.insert(
            (owner, target),
            WarPlan {
                countdown: delay,
                reason,
            },
        );
        if target == HUMAN {
            self.add_event(format!(
                "{} is preparing for war with you in {delay} turns.",
                self.players[owner].name
            ));
        }
    }

    pub(super) fn ai_space_race_leader(&self) -> Option<usize> {
        (0..self.players.len())
            .filter(|&owner| owner != BARBARIANS && self.player_alive(owner))
            .filter(|&owner| self.spaceships[owner].built.iter().any(|&n| n > 0))
            .min_by_key(|&owner| match self.spaceships[owner].arrival_year() {
                Some(year) => (0, year as i32),
                None => (1, 0),
            })
    }

    pub(super) fn advance_ai_war_plans(&mut self) {
        let map_size = self.map_size_factor();
        let space_leader = self.ai_space_race_leader();
        let pairs: Vec<_> = self.diplomacy.keys().copied().collect();
        for owner in 0..self.players.len() {
            if owner == BARBARIANS || !self.players[owner].ai || !self.player_alive(owner) {
                continue;
            }
            let contacts: Vec<_> = pairs
                .iter()
                .filter_map(|&(a, b)| {
                    let target = if a == owner {
                        b
                    } else if b == owner {
                        a
                    } else {
                        return None;
                    };
                    (target != BARBARIANS
                        && self.player_alive(target)
                        && self.diplomacy[&(a, b)].first_contact.is_some())
                    .then_some(target)
                })
                .collect();

            for &target in &contacts {
                let relation = &self.diplomacy[&Self::relation_key(owner, target)];
                let side = usize::from(owner > target);
                if !self.at_war(owner, target)
                    && relation.trust[side] < 0
                    && relation.cause_until[side] - self.turn >= 2
                {
                    self.begin_ai_war_plan(owner, target, map_size, WarReason::Behavior);
                }
            }
            if space_leader != Some(owner) {
                for &target in &contacts {
                    if self.at_war(owner, target)
                        || self.ai_war_countdown(owner, target) != -1
                        || !self.spaceships[target].built.iter().any(|&n| n > 0)
                    {
                        continue;
                    }
                    if !self.spaceships[target].launched() {
                        if self
                            .ai_space_warnings
                            .get(&(owner, target))
                            .copied()
                            .unwrap_or(0)
                            == 0
                        {
                            let side = usize::from(owner > target);
                            let relation = self
                                .diplomacy
                                .get_mut(&Self::relation_key(owner, target))
                                .unwrap();
                            relation.trust[side] = (relation.trust[side] - 1000).max(-10_000);
                            let delay = 10 + self.rand_below(6);
                            self.ai_space_warnings.insert((owner, target), delay);
                            if target == HUMAN {
                                self.add_event(format!("{} warns that continued spaceship construction may lead to war.", self.players[owner].name));
                            }
                        }
                    } else if space_leader == Some(target) {
                        let allied = self.allied(owner, target);
                        let side = usize::from(owner > target);
                        let relation = self
                            .diplomacy
                            .get_mut(&Self::relation_key(owner, target))
                            .unwrap();
                        if allied {
                            relation.trust[side] = -10_000;
                            relation.vision[side] = false;
                        }
                        relation.trust[side] = (relation.trust[side] - 5000).max(-10_000);
                        if allied && !self.senate_blocks_cancellation(owner, target) {
                            self.cancel_pact(owner, target);
                        }
                        self.begin_ai_war_plan(owner, target, 2, WarReason::Space);
                    }
                }
            }

            // classic/nations.ruleset supplies a fixed default of 50; current
            // fixed civilization identities have no trait overrides or modifiers.
            let war_threshold = aggression_war_threshold(50);
            let target = contacts
                .iter()
                .copied()
                .filter(|&target| {
                    let relation = &self.diplomacy[&Self::relation_key(owner, target)];
                    !self.at_war(owner, target) && self.turn - relation.first_contact.unwrap() > 15
                })
                .min_by_key(|&target| {
                    self.diplomacy[&Self::relation_key(owner, target)].trust
                        [usize::from(owner > target)]
                });
            let already_at_war = contacts.iter().any(|&target| self.at_war(owner, target));
            if !already_at_war
                && let Some(target) = target
                && self.diplomacy[&Self::relation_key(owner, target)].trust
                    [usize::from(owner > target)]
                    < war_threshold
            {
                let relation = &self.diplomacy[&Self::relation_key(owner, target)];
                let reason = if relation.cause_until[usize::from(owner > target)] > self.turn {
                    WarReason::Excuse
                } else {
                    WarReason::Hatred
                };
                self.begin_ai_war_plan(owner, target, 4 + map_size, reason);
            }

            // Unlike a shared enemy, an enemy of an ally supplies a specific
            // obligation. Freeciv gives this a shorter preparation countdown.
            for &target in &contacts {
                if self.at_war(owner, target)
                    || self.allied(owner, target)
                    || self.ai_war_countdown(owner, target) != -1
                {
                    continue;
                }
                let intel = self.diplomatic_intel(owner, target);
                let defended_ally = intel.at_war_with_ally;
                let shared_ally = intel.allied_with_ally.is_some();
                if defended_ally.is_some()
                    && !shared_ally
                    && (self.pact(owner, target) != Pact::Ceasefire || self.rand_below(5) == 0)
                {
                    self.begin_ai_war_plan(owner, target, 2 + map_size, WarReason::Alliance);
                }
            }
        }

        let plans: Vec<_> = self
            .ai_war_plans
            .iter()
            .map(|(&pair, &plan)| (pair, plan))
            .collect();
        for ((owner, target), plan) in plans {
            if !self.player_alive(owner) || !self.player_alive(target) || !self.players[owner].ai {
                self.ai_war_plans.remove(&(owner, target));
            } else if plan.countdown > 0 {
                self.ai_war_plans
                    .get_mut(&(owner, target))
                    .unwrap()
                    .countdown -= 1;
            } else if plan.countdown < -1 {
                self.ai_war_plans
                    .get_mut(&(owner, target))
                    .unwrap()
                    .countdown += 1;
            } else if plan.countdown == 0 && !self.at_war(owner, target) {
                self.declare_prepared_ai_war(owner, target, plan.reason);
            }
        }
    }

    pub(super) fn declare_prepared_ai_war(
        &mut self,
        owner: usize,
        target: usize,
        reason: WarReason,
    ) {
        if matches!(reason, WarReason::Alliance)
            && self
                .diplomatic_intel(owner, target)
                .at_war_with_ally
                .is_none()
        {
            self.ai_war_plans.remove(&(owner, target));
            return;
        }
        let stubbornness = match reason {
            WarReason::Spoils | WarReason::Space => -10,
            WarReason::Hatred | WarReason::Excuse | WarReason::Behavior => -20,
            WarReason::Alliance => -3,
        };
        // dai_go_to_war commits the diplomatic cooldown before consulting
        // the Senate, and revokes only this AI's outgoing shared vision.
        self.ai_war_plans.insert(
            (owner, target),
            WarPlan {
                countdown: stubbornness,
                reason,
            },
        );
        if let Some(relation) = self.diplomacy.get_mut(&Self::relation_key(owner, target)) {
            relation.vision[usize::from(owner > target)] = false;
        }
        self.refresh_visibility();
        if !self.at_war(owner, target) && self.senate_blocks_cancellation(owner, target) {
            let government = self.players[owner].government;
            self.players[owner].government = Government::Anarchy;
            let revolution_helps = !self.senate_blocks_cancellation(owner, target);
            self.players[owner].government = government;
            if revolution_helps && self.begin_ai_war_revolution(owner) {
                // revolution_start runs inside the request above, before
                // native dai_go_to_war assigns its transient war_target.
                self.ai_pending_war_targets.insert(owner, target);
            } else {
                self.ai_war_plans.remove(&(owner, target));
            }
            return;
        }
        self.declare_ai_war_now(owner, target);
    }

    pub(in crate::sim) fn resume_ai_war_after_revolution(&mut self, owner: usize) {
        if let Some(target) = self.ai_pending_war_targets.remove(&owner)
            && self.player_alive(target)
        {
            if let Some(relation) = self.diplomacy.get_mut(&Self::relation_key(owner, target)) {
                relation.vision[usize::from(owner > target)] = false;
            }
            self.declare_ai_war_now(owner, target);
        }
    }

    pub(super) fn declare_ai_war_now(&mut self, owner: usize, target: usize) {
        while !self.at_war(owner, target) {
            if !self.cancel_pact(owner, target) {
                self.ai_war_plans.remove(&(owner, target));
                return;
            }
        }
        let relation = self
            .diplomacy
            .get_mut(&Self::relation_key(owner, target))
            .unwrap();
        let trust = &mut relation.trust[usize::from(owner > target)];
        let love = *trust / 10;
        *trust = (if love > 0 { -1 } else { love } - 125) * 10;
        relation.offer = None;
        if target == HUMAN {
            self.add_event(format!("{} declared war on you.", self.players[owner].name));
        }
        self.refresh_visibility();
    }
}

#[cfg(test)]
pub(super) fn war_planning_game() -> Simulation {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    for (owner, (x, y)) in [(HUMAN, (8, 20)), (1, (12, 24)), (2, (18, 30))] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.create_city(owner, x, y);
    }
    game.turn = 30;
    game.sign_pact(HUMAN, 1, Pact::Peace);
    let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
    relation.pact = Pact::Peace;
    relation.until = 0;
    relation.first_contact = Some(1);
    relation.trust[1] = -5000;
    game.make_diplomatic_contact(HUMAN, 1);
    game
}

#[cfg(test)]
mod tests;
