//! AI planning for declaring, preparing, and conducting wars.

use super::*;
use crate::sim::normal_paths::{GotoTask, NormalPaths};

const FRONT_RANGE: i32 = 8;

impl WarReason {
    fn requires_readiness(self) -> bool {
        matches!(self, Self::Hatred | Self::Excuse | Self::Spoils)
    }
}

impl Simulation {
    fn valid_war_objective(&self, owner: usize, target: usize, objective: WarObjective) -> bool {
        self.city_by_id(objective.city)
            .is_some_and(|city| city.owner == target)
            && self
                .city_by_id(objective.muster_city)
                .is_some_and(|city| city.owner == owner)
            && (0..self.map_wrap.height()).contains(&objective.staging.1)
            && self
                .city_at(objective.staging.0, objective.staging.1)
                .is_none()
            && self
                .tile(objective.staging.0, objective.staging.1)
                .owner
                .is_none_or(|controller| controller == owner || self.allied(owner, controller))
    }

    fn choose_war_objective(&self, owner: usize, target: usize) -> Option<WarObjective> {
        let continents = self.continent_labels();
        let mut candidates = Vec::new();
        for city in self.cities_owned_by(target) {
            if !self.tile(city.x, city.y).known_by[owner] {
                continue;
            }
            for home in self.cities_owned_by(owner) {
                let distance = self.map_distance(home.x, home.y, city.x, city.y);
                if distance > FRONT_RANGE
                    || continents[self.map_wrap.index(home.x, home.y)]
                        != continents[self.map_wrap.index(city.x, city.y)]
                {
                    continue;
                }
                for (x, y, _) in self.map_wrap.tiles_within_steps(city.x, city.y, 3) {
                    let objective = WarObjective {
                        city: city.id,
                        muster_city: home.id,
                        staging: (x, y),
                    };
                    if !self.tile(x, y).known_by[owner]
                        || !self.pf_native_tile(UnitKind::Warrior, x, y)
                        || continents[self.map_wrap.index(x, y)]
                            != continents[self.map_wrap.index(city.x, city.y)]
                        || !self.valid_war_objective(owner, target, objective)
                    {
                        continue;
                    }
                    let approach = self.map_distance(home.x, home.y, x, y);
                    let remaining = self.map_distance(x, y, city.x, city.y);
                    candidates.push((
                        (
                            distance,
                            approach + remaining,
                            remaining,
                            city.id,
                            home.id,
                            x,
                            y,
                        ),
                        objective,
                    ));
                }
            }
        }
        candidates.sort_unstable_by_key(|&(key, _)| key);
        candidates
            .into_iter()
            .find_map(|((_, _, _, _, home, x, y), objective)| {
                let home = self.city_by_id(home)?;
                let probe = self.make_unit(0, owner, UnitKind::Warrior, home.x, home.y);
                NormalPaths::goto_map(self, &probe, GotoTask::None)
                    .position_turn_astar(self, x, y)
                    .filter(|&turns| turns <= FRONT_RANGE)
                    .map(|_| objective)
            })
    }

    pub(crate) fn coordinated_war_objective(&self, owner: usize) -> Option<WarObjective> {
        self.ai_war_plans
            .iter()
            .filter_map(|(&(planner, target), plan)| {
                let objective = plan.objective?;
                let at_war = self.at_war(owner, target);
                (planner == owner
                    && (at_war || plan.countdown >= 0)
                    && self
                        .city_by_id(objective.city)
                        .is_some_and(|city| city.owner == target)
                    && self
                        .city_by_id(objective.muster_city)
                        .is_some_and(|city| city.owner == owner))
                .then_some((!at_war, objective.city, objective))
            })
            .min_by_key(|&(preparing, city, _)| (preparing, city))
            .map(|(_, _, objective)| objective)
    }

    pub(in crate::sim) fn war_muster_has_garrison(&self, city: &City) -> bool {
        self.war_muster_garrison(city).is_some()
    }

    pub(in crate::sim) fn war_muster_garrison(&self, city: &City) -> Option<u32> {
        self.tile_units_ordered(city.x, city.y)
            .filter(|unit| {
                unit.owner == city.owner
                    && unit.transport.is_none()
                    && unit.kind.is_military()
                    && !unit.kind.is_naval()
                    && !unit.kind.is_aircraft()
                    && unit.kind.stats().defense > 0
            })
            .max_by_key(|unit| {
                (
                    unit.kind.stats().defense * unit.hp,
                    std::cmp::Reverse(unit.id),
                )
            })
            .map(|unit| unit.id)
    }

    pub(in crate::sim) fn can_join_ground_offensive(kind: UnitKind) -> bool {
        kind.is_military()
            && !kind.is_naval()
            && !kind.is_aircraft()
            && kind.can_occupy_city()
            && kind.stats().attack >= 2
    }

    pub(in crate::sim) fn war_force_ready(&self, owner: usize, objective: WarObjective) -> bool {
        let Some(city) = self.city_by_id(objective.city) else {
            return false;
        };
        let Some(home) = self.city_by_id(objective.muster_city) else {
            return false;
        };
        if !self.war_muster_has_garrison(home) {
            return false;
        }
        let troops: Vec<_> = self
            .tile_units_ordered(objective.staging.0, objective.staging.1)
            .filter(|unit| {
                unit.owner == owner
                    && unit.transport.is_none()
                    && Self::can_join_ground_offensive(unit.kind)
            })
            .collect();
        if troops.len() < 2 {
            return false;
        }
        // Cities conceal their garrison. Estimate that garrison separately
        // from troops that can visibly reinforce it from surrounding tiles.
        let mut expected = self.make_unit(0, city.owner, UnitKind::Spearman, city.x, city.y);
        expected.state = UnitState::Fortified;
        let mut defenders = vec![&expected];
        for (x, y, distance) in self.map_wrap.tiles_within_steps(city.x, city.y, 3) {
            if distance == 0 {
                continue;
            }
            defenders.extend(self.tile_units_ordered(x, y).filter(|unit| {
                unit.owner == city.owner
                    && unit.kind.is_military()
                    && !unit.kind.is_aircraft()
                    && !unit.kind.is_naval()
                    && unit.kind.stats().defense > 0
                    && self.unit_visible_to(owner, unit)
            }));
        }
        self.assault_capture_chance_against(city, &troops, &defenders) >= 0.70
    }

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
            && !self.same_vassal_group(owner, other)
            && (self.at_war(owner, other)
                || self.ai_war_countdown(owner, other) >= 0
                || self
                    .diplomacy
                    .get(&Self::relation_key(owner, other))
                    .is_none_or(|relation| relation.first_contact.is_none()))
    }

    pub(in crate::sim) fn advisor_player_dangerous(&self, owner: usize, other: usize) -> bool {
        if self.same_vassal_group(owner, other) {
            return false;
        }
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
                objective: None,
            },
        );
        self.trace_diplomacy(
            owner,
            target,
            "war_plan_started",
            reason.observation_name(),
            self.contacted_pact(owner, target),
            None,
        );
        if target == HUMAN {
            let name = &self.players[owner].name;
            let message = if reason.requires_readiness() {
                format!("{name} is preparing for war with you.")
            } else {
                format!("{name} is preparing for war with you in {delay} turns.")
            };
            self.add_event(message);
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

    #[cfg(test)]
    pub(super) fn advance_ai_war_plans(&mut self) {
        for owner in 0..self.players.len() {
            self.advance_ai_war_plans_for(owner);
        }
    }

    pub(super) fn advance_ai_war_plans_for(&mut self, owner: usize) {
        if self.vassal_master(owner).is_some() {
            return;
        }
        let map_size = self.map_size_factor();
        let space_leader = self.ai_space_race_leader();
        let pairs: Vec<_> = self.diplomacy.keys().copied().collect();
        if owner == BARBARIANS || !self.players[owner].ai || !self.player_alive(owner) {
            return;
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
                    && !self.same_vassal_group(owner, target)
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
                            self.add_event(format!(
                                "{} warns that continued spaceship construction may lead to war.",
                                self.players[owner].name
                            ));
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

        // Keep the selected C2C3 aggression baseline, then apply the
        // owner-authorized strategic policy preference for military action.
        let strategy = self.strategic_priorities(owner);
        let aggression =
            crate::sim::ai_traits::fixed_default("aggressive") + (strategy.military - 100) / 5;
        let war_threshold = aggression_war_threshold(aggression);
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
                    - (self.relationship_war_adjustment(owner, target)
                        - self.religion_attitude(owner, target) * 10)
            });
        let already_at_war = contacts.iter().any(|&target| self.at_war(owner, target));
        if !already_at_war
            && !strategy.preparing
            && strategy.can_start_optional_war()
            && let Some(target) = target
            && self.diplomacy[&Self::relation_key(owner, target)].trust[usize::from(owner > target)]
                < war_threshold
                    + (self.relationship_war_adjustment(owner, target)
                        - self.religion_attitude(owner, target) * 10)
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

        let plans: Vec<_> = self
            .ai_war_plans
            .iter()
            .filter(|(pair, _)| pair.0 == owner)
            .map(|(&pair, &plan)| (pair, plan))
            .collect();
        for ((owner, target), plan) in plans {
            if !self.player_alive(owner) || !self.player_alive(target) || !self.players[owner].ai {
                self.ai_war_plans.remove(&(owner, target));
                continue;
            }
            if plan.countdown >= 0
                && plan.reason.requires_readiness()
                && !self.at_war(owner, target)
            {
                let objective = plan
                    .objective
                    .filter(|objective| self.valid_war_objective(owner, target, *objective))
                    .or_else(|| self.choose_war_objective(owner, target));
                self.ai_war_plans
                    .get_mut(&(owner, target))
                    .unwrap()
                    .objective = objective;
                if plan.countdown == 0 {
                    let reason = match objective {
                        None => {
                            self.ai_war_plans
                                .get_mut(&(owner, target))
                                .unwrap()
                                .countdown = -10;
                            "no_reachable_border_objective"
                        }
                        Some(_) if strategy.economic_crisis => "economic_recovery",
                        Some(_)
                            if !strategy.can_start_optional_war()
                                || contacts.iter().any(|&other| self.at_war(owner, other)) =>
                        {
                            "other_strategic_commitment"
                        }
                        Some(objective) if !self.war_force_ready(owner, objective) => {
                            "assembling_force"
                        }
                        Some(_) => {
                            self.declare_prepared_ai_war(owner, target, plan.reason);
                            continue;
                        }
                    };
                    self.trace_diplomacy(
                        owner,
                        target,
                        "war_deferred",
                        reason,
                        self.contacted_pact(owner, target),
                        None,
                    );
                    continue;
                }
            }
            if plan.countdown > 0 {
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
        let previous = self.contacted_pact(owner, target);
        self.trace_diplomacy(
            owner,
            target,
            "war_decision",
            reason.observation_name(),
            previous,
            None,
        );
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
                objective: self
                    .ai_war_plans
                    .get(&(owner, target))
                    .and_then(|plan| plan.objective),
            },
        );
        if let Some(relation) = self.diplomacy.get_mut(&Self::relation_key(owner, target)) {
            relation.vision[usize::from(owner > target)] = false;
        }
        self.refresh_visibility();
        if !self.at_war(owner, target) && self.senate_blocks_cancellation(owner, target) {
            let government = self.players[owner].government;
            self.players[owner].government = Government::during_revolution();
            let revolution_helps = !self.senate_blocks_cancellation(owner, target);
            self.players[owner].government = government;
            self.trace_diplomacy(
                owner,
                target,
                "war_deferred",
                "senate_blocks_cancellation",
                self.contacted_pact(owner, target),
                None,
            );
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

    pub(in crate::sim) fn declare_ai_war_now(&mut self, owner: usize, target: usize) {
        if !self.pact_transition_allowed(owner, target) {
            return;
        }
        let previous = self.contacted_pact(owner, target);
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
        self.trace_diplomacy(
            owner,
            target,
            "war_declared",
            "prepared_ai_war",
            previous,
            None,
        );
        if target == HUMAN {
            self.add_contextual_notification(
                format!("{} declared war on you.", self.players[owner].name),
                super::super::notifications::NotificationCategory::Diplomacy,
                super::super::notifications::NotificationPriority::Important,
                None,
                Some(owner),
            );
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
