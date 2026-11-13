//! Technology availability, cost, and research progression.

use super::*;
use crate::rules::TechEra;
use crate::rules::freeciv::{ResearchRulesetSettings, RulesetData, effect_value};
use crate::sim::action_rules::SimulationRequirementContext;
use std::sync::OnceLock;

pub(super) fn campaign_research_settings() -> &'static ResearchRulesetSettings {
    static SETTINGS: OnceLock<ResearchRulesetSettings> = OnceLock::new();
    SETTINGS.get_or_init(|| {
        RulesetData::campaign_default()
            .research_settings()
            .unwrap_or_else(|error| panic!("invalid C2C3 research settings: {error}"))
    })
}

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
        if !tech.is_civ3_advance() && tech != Tech::FUTURE {
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
                .civ3_prerequisites()
                .unwrap_or(&[])
                .iter()
                .all(|&req| self.has_tech(owner, req))
    }

    /// Highest era reached after acquiring every critical advance in each
    /// preceding era. The result also gates access to later-era technologies.
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
            let current_era = game.research_era(owner);
            for era in TechEra::ALL
                .into_iter()
                .filter(|era| *era >= current_era && *era < tech.era())
            {
                for required in Tech::all()
                    .filter(|advance| advance.era() == era && advance.required_for_civ3_era())
                {
                    visit(game, owner, required, path);
                }
            }
            for &req in tech.civ3_prerequisites().unwrap_or(&[]) {
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
        let goal = self.research_goal_for_target(owner);
        self.research_path(owner, goal)
            .first()
            .copied()
            .or_else(|| {
                let current = self.players[owner].researching;
                self.tech_available(owner, current).then_some(current)
            })
            .unwrap_or(Tech::FUTURE)
    }

    /// Select a new target when Freeciv would call choose_tech(). Goal steps
    /// take precedence; without one, choose_random_tech() reservoir-samples
    /// every currently researchable advance in source ID order.
    pub(super) fn choose_research_target(&mut self, owner: usize) -> Tech {
        let goal = self.research_goal_for_target(owner);
        if let Some(step) = self.research_path(owner, goal).first().copied() {
            return step;
        }
        self.random_available_research_target(owner)
    }

    fn research_goal_for_target(&self, owner: usize) -> Tech {
        let goal = self.players[owner].research_goal;
        if self.players[owner].ai && goal != Tech::UNSET {
            let ferry_want = self.ferry_tech_want(owner, Tech::MAP_MAKING);
            if ferry_want > 0 && ferry_want as f32 > self.advisor_tech_want(owner, goal) {
                return Tech::MAP_MAKING;
            }
        }
        goal
    }

    fn random_available_research_target(&mut self, owner: usize) -> Tech {
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
            self.choose_research_tech(owner, choice, free);
        }
    }

    pub(crate) fn target_cost(&self, owner: usize, tech: Tech) -> i32 {
        self.technology_cost(owner, tech, false)
    }

    fn technology_cost(&self, owner: usize, tech: Tech, loss_value: bool) -> i32 {
        if !tech.is_civ3_advance() && tech != Tech::FUTURE {
            return 0;
        }
        let player = &self.players[owner];
        if tech != Tech::FUTURE && !loss_value && self.has_tech(owner, tech) {
            return 0;
        }
        let rules = RulesetData::campaign_default();
        let settings = campaign_research_settings();
        let base = match settings.cost_style {
            crate::rules::freeciv::ResearchCostStyle::Linear => {
                if tech == Tech::FUTURE {
                    i64::from(settings.base_tech_cost)
                        * i64::from(
                            player
                                .known
                                .iter()
                                .filter(|known| known.is_civ3_advance())
                                .count() as i32
                                + player.future_techs
                                + 1,
                        )
                } else {
                    let requirement_count = tech
                        .civ3_requirement_count()
                        .expect("researchable C3 technology has a Freeciv requirement count");
                    i64::from(settings.base_tech_cost)
                        .saturating_mul(i64::from(requirement_count))
                        .max(i64::from(settings.min_tech_cost))
                }
            }
        };

        // common/research.c::research_total_bulbs_required applies the
        // active Tech_Cost_Pct effect, embassy leakage, then sciencebox.
        let context = SimulationRequirementContext::for_player_at(self, owner, None)
            .with_government(player.government);
        let cost_percent = effect_value(&rules.effects, "Tech_Cost_Pct", &context)
            .expect("campaign Tech_Cost_Pct effects are evaluable");
        let mut cost = base as f64 * cost_percent as f64 / 100.0;
        if settings.leakage_style == crate::rules::freeciv::TechnologyLeakageStyle::Embassies
            && effect_value(&rules.effects, "Tech_Leakage", &context)
                .expect("campaign Tech_Leakage effects are evaluable")
                > 0
        {
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
                if other != owner && knows && self.has_embassy(owner, other) {
                    sources += 1;
                }
            }
            if alive > 0 {
                let leak = cost * f64::from(sources) * f64::from(settings.tech_leak_percent)
                    / f64::from(alive)
                    / 100.0;
                cost = (cost - leak).max(0.0);
            }
        }
        cost = (cost * f64::from(settings.sciencebox_percent) / 100.0).max(1.0);
        cost as i32
    }

    /// Port of `server/techtools.c::choose_tech` for C2C3's non-multiresearch
    /// setting. Free bulbs from a completed discovery are exempt from the
    /// switch penalty; one abandoned project is retained for resumption.
    pub(super) fn choose_research_tech(&mut self, owner: usize, tech: Tech, free_bulbs: i32) {
        if !self.tech_available(owner, tech) {
            return;
        }
        let player = &mut self.players[owner];
        if player.researching == tech {
            return;
        }
        let investable = player.research.saturating_sub(free_bulbs);
        if investable > 0 && player.researching_saved.is_none() {
            player.research_saved = player.research;
            player.researching_saved = Some(player.researching);
            let loss =
                investable.saturating_mul(campaign_research_settings().tech_penalty_percent) / 100;
            player.research = player.research.saturating_sub(loss).max(0);
        } else if player.researching_saved == Some(tech) {
            player.research = player.research_saved;
            player.research_saved = 0;
            player.researching_saved = None;
        }
        player.researching = tech;
    }

    pub(super) fn apply_research_penalty(&mut self, owner: usize, tech: Tech, percent: i32) {
        let penalty = self.target_cost(owner, tech).saturating_mul(percent) / 100;
        self.players[owner].research = self.players[owner].research.saturating_sub(penalty);
        self.players[owner].researching_saved = None;
        self.players[owner].research_saved = 0;
    }

    fn player_effect_value(&self, owner: usize, kind: &str) -> i64 {
        let rules = RulesetData::campaign_default();
        let context = SimulationRequirementContext::for_player_at(self, owner, None)
            .with_government(self.players[owner].government);
        effect_value(&rules.effects, kind, &context)
            .unwrap_or_else(|error| panic!("could not evaluate campaign {kind} effects: {error:?}"))
    }

    pub(crate) fn technology_upkeep(&self, owner: usize) -> i32 {
        let player = &self.players[owner];
        let settings = campaign_research_settings();
        let known_cost: i64 = player
            .known
            .iter()
            .filter(|tech| tech.is_civ3_advance())
            .map(|tech| {
                let count = tech
                    .civ3_requirement_count()
                    .expect("known C3 advance has a Freeciv requirement count");
                (i64::from(settings.base_tech_cost) * i64::from(count))
                    .max(i64::from(settings.min_tech_cost))
            })
            .sum();
        let future = i64::from(player.future_techs);
        let techs_researched = player
            .known
            .iter()
            .filter(|tech| tech.is_civ3_advance())
            .count() as i64
            + future
            + 1;
        let future_cost = if future > 0 {
            i64::from(settings.base_tech_cost)
                * (future * (2 * techs_researched + future + 1) + 2 * techs_researched)
                / 2
        } else {
            0
        };
        let cost_percent = self.player_effect_value(owner, "Tech_Cost_Pct");
        let free = self.player_effect_value(owner, "Tech_Upkeep_Free");
        let cities = match settings.upkeep_style {
            crate::rules::freeciv::TechnologyUpkeepStyle::Cities => self
                .cities
                .iter()
                .filter(|city| city.owner == owner)
                .count() as f64,
        };
        let base = (known_cost + future_cost) as f64;
        let upkeep = base
            * (cost_percent as f64 / 100.0 + 1.0)
            * (f64::from(settings.sciencebox_percent) / 100.0)
            / f64::from(settings.upkeep_divider);
        ((upkeep - free as f64).max(0.0) * cities) as i32
    }

    pub(super) fn apply_research_upkeep(&mut self, owner: usize, income: i32) {
        let upkeep = self.technology_upkeep(owner);
        self.players[owner].last_science = income - upkeep;
        // Freeciv's per-city update_bulbs() adds normal science output to the
        // suspended project's pool. The later negative upkeep update runs
        // after researching_saved is cleared, so only city science belongs
        // here; do not add net income after upkeep.
        if self.players[owner].researching_saved.is_some() {
            self.players[owner].research_saved =
                self.players[owner].research_saved.saturating_add(income);
        }
        self.players[owner].research += income - upkeep;
        let settings = campaign_research_settings();
        let forgiveness = settings.tech_loss_forgiveness_percent;
        let loss_threshold = -self
            .target_cost(owner, self.players[owner].researching)
            .saturating_mul(forgiveness)
            / 100;
        if self.players[owner].research >= 0
            || forgiveness < 0
            || self.players[owner].research >= loss_threshold
        {
            return;
        }
        let lost = if self.players[owner].future_techs > 0 {
            Some(Tech::FUTURE)
        } else {
            let known = &self.players[owner].known;
            let eligible: Vec<_> = Tech::all()
                .filter(|tech| {
                    known.contains(tech)
                        && (settings.tech_loss_allow_holes
                            || known.iter().all(|other| {
                                !other.civ3_prerequisites().unwrap_or(&[]).contains(tech)
                            }))
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
        let restore_percent = campaign_research_settings().tech_loss_restore_percent;
        if restore_percent < 0 {
            self.players[owner].research = 0;
        } else {
            self.players[owner].research +=
                self.technology_cost(owner, tech, true) * restore_percent / 100;
        }
        if tech == Tech::FUTURE {
            self.players[owner].future_techs -= 1;
        } else {
            self.players[owner].known.retain(|&known| known != tech);
        }
        self.players[owner].researching_saved = None;
        self.players[owner].research_saved = 0;
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
        let advance = match campaign_research_settings().free_technology_style {
            crate::rules::freeciv::FreeTechnologyStyle::Cheapest => {
                if let Some(cost) = available
                    .iter()
                    .map(|&tech| self.target_cost(owner, tech))
                    .min()
                {
                    let cheapest: Vec<_> = available
                        .into_iter()
                        .filter(|&tech| self.target_cost(owner, tech) == cost)
                        .collect();
                    cheapest[self.rand_below(cheapest.len() as i32) as usize]
                } else {
                    Tech::FUTURE
                }
            }
        };
        self.discover(owner, advance);
        if self.players[owner].researching == advance {
            let free = self.players[owner].research;
            let next = self.choose_research_target(owner);
            self.choose_research_tech(owner, next, free);
        }
    }

    pub(super) fn discover(&mut self, owner: usize, tech: Tech) {
        if !tech.is_civ3_advance() && tech != Tech::FUTURE {
            return;
        }
        let was_researching = self.players[owner].researching == tech;
        if self.players[owner].ai && was_researching {
            self.ai_free_bulbs
                .insert(owner, self.players[owner].research);
        }
        // `server/techtools.c::tech_researched` forgets the one suspended
        // project after any newly discovered technology.
        self.players[owner].researching_saved = None;
        self.players[owner].research_saved = 0;
        if tech == Tech::FUTURE {
            self.players[owner].future_techs += 1;
        } else {
            self.players[owner].known.push(tech);
            if !self.global_advances.contains(&tech) {
                self.global_advances.push(tech);
            }
            if matches!(tech, Tech::RAILROAD | Tech::STEAM_POWER) {
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
            let free = self.ai_free_bulbs.get(&owner).copied().unwrap_or(0);
            let next = self.post_discovery_ai_target(owner);
            self.choose_research_tech(owner, next, free);
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
                for &req in tech.civ3_prerequisites().unwrap_or(&[]) {
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
                let bonus = self.choose_research_target(owner);
                if bonus != Tech::FUTURE {
                    self.discover(owner, bonus);
                }
            }
            if !self.players[owner].ai {
                let free = self.players[owner].research;
                let next = self.choose_research_target(owner);
                self.choose_research_tech(owner, next, free);
            }
        }
    }
}

#[cfg(test)]
#[path = "research/tests.rs"]
mod tests;
