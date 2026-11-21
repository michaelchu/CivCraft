//! Religion uses shared strategic goals and only own or public information.
use super::super::ai_strategy::StrategicPriorities;
use super::*;

struct GoalContext {
    shared: StrategicPriorities,
    culture: i32,
    cities: Vec<u32>,
}
#[derive(Clone, Copy, Default, Serialize)]
struct PolicyScore {
    alignment: i32,
    culture: i32,
    content: i32,
    defense: i32,
    spread: i32,
    diplomacy: i32,
}
impl PolicyScore {
    fn total(self) -> i32 {
        self.alignment + self.culture + self.content + self.defense + self.spread + self.diplomacy
    }
}
#[derive(Serialize)]
struct Decision {
    owner: usize,
    economy: i32,
    science: i32,
    culture: i32,
    founding: Option<u32>,
    declared: Option<u32>,
    policy: FaithPolicy,
    previous_score: i32,
    selected_score: i32,
    cooldown: i32,
    previous_declared: Option<u32>,
    previous_policy: FaithPolicy,
    previous_components: PolicyScore,
    selected_components: PolicyScore,
    required_improvement: i32,
    best_declared: Option<u32>,
    best_policy: FaithPolicy,
    best_components: PolicyScore,
    missionary_opportunity: i32,
    research_candidate: Option<usize>,
    research_interest: f32,
    actual_research: usize,
    research_blocked: &'static str,
}
impl trace::Payload for Decision {
    const KIND: &'static str = "religion.ai_decision";
}

impl Simulation {
    fn religion_goals(&self, owner: usize, shared: StrategicPriorities) -> GoalContext {
        let policy = &rules().ai;
        let religious = self
            .ai_civilization(owner)
            .is_some_and(|c| ai_traits::trait_enabled(c, "religious"));
        let culture = if self.victory_enabled(crate::rules::engine_rules::VictoryCondition::Culture)
        {
            100 + if self.civilization_culture(owner) >= policy.culture_goal_points {
                policy.culture_goal_bonus
            } else {
                0
            } + if religious {
                policy.religious_culture_bonus
            } else {
                0
            }
        } else {
            0
        };
        let mut cities: Vec<_> = self
            .cities
            .iter()
            .filter(|c| c.owner == owner)
            .map(|c| c.id)
            .collect();
        cities.sort_unstable();
        cities.truncate(policy.maximum_cities);
        GoalContext {
            shared,
            culture,
            cities,
        }
    }
    fn faith_city_value(&self, city: &City, f: &FaithDefinition, g: &GoalContext) -> i32 {
        let a = &rules().ai;
        let output = self.city_forecast(city);
        let (_, unhappy, angry) = self.city_feelings(city, output.luxury);
        let gold = g.shared.economy
            + if g.shared.economic_crisis {
                a.recovery_gold_bonus
            } else {
                0
            };
        f.gold * gold
            + f.science * g.shared.science
            + f.culture * g.culture
            + f.content
                * (a.content_growth_value + (unhappy + angry * 2).min(2) * a.content_unrest_value)
            + f.local_pressure * (g.culture + g.shared.science + gold) / 12
            + (f.outgoing_pressure + f.trade_pressure) * (g.culture + g.shared.science + gold) / 12
    }
    fn faith_goal_value(&self, f: &FaithDefinition, g: &GoalContext) -> i32 {
        let total = g
            .cities
            .iter()
            .filter_map(|id| self.city_by_id(*id))
            .map(|c| self.faith_city_value(c, f, g))
            .sum::<i32>();
        total / g.cities.len().max(1) as i32
    }
    fn religious_partner_weight(&self, owner: usize, other: usize) -> i32 {
        match self.contacted_pact(owner, other) {
            Some(Pact::Alliance) => rules().ai.ally_weight,
            Some(Pact::Peace | Pact::Armistice | Pact::Ceasefire)
                if self.diplomatic_love(owner, other) - self.religion_attitude(owner, other)
                    >= 100 =>
            {
                rules().ai.friend_weight
            }
            Some(Pact::Peace | Pact::Armistice | Pact::Ceasefire) => 1,
            _ => 0,
        }
    }
    fn faith_attitude_for(
        &self,
        declared: Option<u32>,
        policy: FaithPolicy,
        other: Option<u32>,
    ) -> i32 {
        match (declared, other) {
            (Some(a), Some(b)) if a == b => rules().shared_attitude,
            (Some(_), Some(_)) => match policy {
                FaithPolicy::Tolerant => 0,
                FaithPolicy::Devout => rules().devout_difference,
                FaithPolicy::Missionary => rules().missionary_difference,
            },
            _ => 0,
        }
    }
    fn missionary_opportunity(&self, owner: usize, faith: u32, g: &GoalContext) -> i32 {
        let a = &rules().ai;
        // Include all own sources: the city evaluation budget must not hide
        // sources that already saturate a target's incoming-pressure cap.
        let sources: Vec<_> = self
            .cities
            .iter()
            .filter(|c| c.owner == owner && self.city_dominant_faith(c.id) == Some(faith))
            .collect();
        if sources.is_empty() {
            return 0;
        }
        let f = definition(faith).unwrap();
        let routes = self.active_trade_city_pairs();
        let mut targets = BTreeMap::new();
        for &id in &g.cities {
            let city = self.city_by_id(id).unwrap();
            if self.city_dominant_faith(id) != Some(faith) {
                let existing = self
                    .city_faith_definition(city)
                    .map_or(0, |old| self.faith_city_value(city, old, g));
                targets.insert(id, (self.faith_city_value(city, f, g) - existing).max(0));
            }
        }
        for &(left, right) in &routes {
            let destination = if sources.iter().any(|s| s.id == left) {
                Some(right)
            } else if sources.iter().any(|s| s.id == right) {
                Some(left)
            } else {
                None
            };
            let Some(city) = destination.and_then(|id| self.city_by_id(id)) else {
                continue;
            };
            if city.owner != owner
                && self
                    .observed_map
                    .cities
                    .get(&(owner, city.id))
                    .is_some_and(|seen| seen.1.city.owner == city.owner)
            {
                let weight = self.religious_partner_weight(owner, city.owner);
                if weight > 0 && self.diplomatic_love(owner, city.owner) >= 0 {
                    // Foreign beliefs and other foreign sources remain private.
                    targets.insert(city.id, a.foreign_spread_value * weight);
                }
            }
        }
        let mut value = 0;
        for (id, benefit) in targets.into_iter().take(a.maximum_spread_targets) {
            let city = self.city_by_id(id).unwrap();
            let mut base = 0;
            let mut boosted = 0;
            for source in &sources {
                let mut pressure = if source.id != id
                    && city.owner == owner
                    && self
                        .map_wrap
                        .civ3_distance(source.x, source.y, city.x, city.y)
                        <= rules().local_distance
                {
                    rules().local_pressure + f.outgoing_pressure
                } else {
                    0
                };
                if routes.contains(&(source.id, id)) || routes.contains(&(id, source.id)) {
                    pressure = pressure
                        .max(rules().trade_pressure + f.outgoing_pressure + f.trade_pressure);
                }
                if pressure > 0 {
                    base += pressure;
                    boosted += pressure + rules().missionary_pressure;
                }
            }
            let gain = boosted.min(rules().incoming_cap) - base.min(rules().incoming_cap);
            value += gain * benefit;
        }
        value * a.spread_percent / 100
    }
    fn religious_policy_score(
        &self,
        owner: usize,
        faith: Option<u32>,
        policy: FaithPolicy,
        g: &GoalContext,
    ) -> PolicyScore {
        let a = &rules().ai;
        let mut value = PolicyScore::default();
        for id in &g.cities {
            let Some(city) = self.city_by_id(*id) else {
                continue;
            };
            let dominant = self.city_dominant_faith(*id);
            if faith.is_some() && faith == dominant {
                value.alignment += a.adoption_city_value;
            }
            match policy {
                FaithPolicy::Tolerant if dominant.is_some() => {
                    value.culture += rules().tolerant_culture * g.culture
                }
                FaithPolicy::Devout if faith.is_some() && faith == dominant => {
                    let output = self.city_forecast(city);
                    let intrinsic = self.city_faith_definition(city).map_or(0, |f| f.content);
                    let without =
                        self.city_mood_with_religious_content(city, output.luxury, intrinsic);
                    let with = self.city_mood_with_religious_content(
                        city,
                        output.luxury,
                        intrinsic + rules().devout_content,
                    );
                    let mut relief = (without.0 - with.0).max(0);
                    if relief == 0 && city.specialists[2] > 0 {
                        // Governors often hide unrest with entertainers before
                        // religion runs. Value a worker this content could free,
                        // using the same citizen/luxury pipeline, not a flat bonus.
                        let mut employed = city.clone();
                        employed.specialists[2] -= 1;
                        let luxury = self.city_forecast(&employed).luxury;
                        let unrest =
                            self.city_mood_with_religious_content(&employed, luxury, intrinsic);
                        let calmed = self.city_mood_with_religious_content(
                            &employed,
                            luxury,
                            intrinsic + rules().devout_content,
                        );
                        if unrest.1 && !calmed.1 {
                            relief = (unrest.0 - calmed.0).max(0);
                        }
                    }
                    value.content += relief * a.content_unrest_value;
                    if self.religion.cities.get(id).is_some_and(|s| {
                        s.influence.iter().any(|(&f, &v)| Some(f) != faith && v > 0)
                    }) {
                        value.defense += rules().devout_pressure
                            * self.faith_city_value(city, definition(faith.unwrap()).unwrap(), g)
                            * a.local_defense_percent
                            / 100;
                    }
                }
                _ => {}
            }
        }
        if policy == FaithPolicy::Missionary
            && let Some(id) = faith
        {
            value.spread += self.missionary_opportunity(owner, id, g);
        }
        for other in 0..self.civilization_count {
            if other == owner {
                continue;
            }
            let weight = self.religious_partner_weight(owner, other);
            if weight == 0 {
                continue;
            }
            let partner = self.national_faith(other);
            let outward = self.faith_attitude_for(faith, policy, partner.declared);
            let inward = self.faith_attitude_for(partner.declared, partner.policy, faith);
            value.diplomacy += (outward + inward) * weight * a.diplomacy_value;
        }
        value
    }
    fn religious_research_block(&self, owner: usize, p: &StrategicPriorities) -> &'static str {
        if p.economic_crisis {
            return "recovery";
        }
        if p.wars > 0 {
            return "war";
        }
        if p.preparing {
            return "preparation";
        }
        if p.space_program {
            return "space_program";
        }
        if self.religion.religions.iter().any(|f| f.founder == owner) {
            return "already_founded";
        }
        let cities = self.cities_owned_by(owner).count().max(1);
        if self.ai_tech_wants.get(&owner).is_some_and(|wants| {
            wants
                .iter()
                .any(|v| *v >= cities as f32 * rules().ai.research_native_priority_per_city as f32)
        }) {
            return "advisor_priority";
        }
        "none"
    }
    /// Adds an interest to the ordinary advisor, never forces a research switch.
    pub(crate) fn religion_research_interest(
        &self,
        owner: usize,
        tech: Tech,
        shared: &StrategicPriorities,
    ) -> f32 {
        if !self.players[owner].ai || self.religious_research_block(owner, shared) != "none" {
            return 0.0;
        }
        let Some(f) = rules().faiths.iter().find(|f| {
            f.tech() == tech
                && !self.has_tech(owner, tech)
                && !self.religion.religions.iter().any(|r| r.id == f.id)
        }) else {
            return 0.0;
        };
        let path = self.shared_research_path(owner, tech);
        if path.is_empty() || path.len() > rules().ai.research_max_steps {
            return 0.0;
        }
        let g = self.religion_goals(owner, *shared);
        let value = self.faith_goal_value(f, &g);
        let cost = self.target_cost(owner, tech).max(1);
        let want = (value * rules().ai.research_cost_scale / cost / path.len() as i32)
            .min(rules().ai.research_cap_per_city)
            .max(0);
        (want * g.cities.len() as i32) as f32
    }
    pub(super) fn run_religion_ai(&mut self) {
        for owner in 0..self.civilization_count {
            if !self.players[owner].ai || !self.player_alive(owner) {
                continue;
            }
            let shared = self.strategic_priorities(owner);
            let g = self.religion_goals(owner, shared);
            let city = self
                .cities
                .iter()
                .filter(|c| c.owner == owner)
                .min_by_key(|c| (!c.has_building(Building::PALACE), c.id))
                .map(|c| c.id);
            let founding = city.and_then(|city| {
                rules()
                    .faiths
                    .iter()
                    .filter(|f| self.can_found_religion(owner, city, f.id))
                    .max_by_key(|f| (self.faith_goal_value(f, &g), std::cmp::Reverse(f.id)))
                    .map(|f| f.id)
            });
            let mut research = None;
            let mut interest = 0.0;
            for f in &rules().faiths {
                let want = self.religion_research_interest(owner, f.tech(), &shared);
                if want > interest {
                    interest = want;
                    research = Some(f.tech().0);
                }
            }
            if let (Some(city), Some(id)) = (city, founding) {
                self.found_religion(owner, city, id);
            }
            let old = self.national_faith(owner);
            let baseline = self.religious_policy_score(owner, old.declared, old.policy, &g);
            let mut best = (old.declared, old.policy, baseline);
            let required_improvement = rules().ai_minimum_improvement
                + rules().ai.switching_city_cost * g.cities.len() as i32;
            let cooldown = self.religion_cooldown(owner);
            if cooldown == 0 {
                let choices =
                    std::iter::once(None).chain(self.eligible_faiths(owner).into_iter().map(Some));
                for faith in choices {
                    for policy in FaithPolicy::ALL {
                        let value = self.religious_policy_score(owner, faith, policy, &g);
                        if value.total() > best.2.total() {
                            best = (faith, policy, value);
                        }
                    }
                }
                if best.2.total() >= baseline.total() + required_improvement {
                    self.declare_faith(owner, best.0, best.1);
                }
            }
            let current = self.national_faith(owner);
            let research_blocked = self.religious_research_block(owner, &shared);
            let missionary_opportunity = current
                .declared
                .map_or(0, |id| self.missionary_opportunity(owner, id, &g));
            self.trace.emit(
                self.turn,
                || vec![trace::Entity::Civilization(owner)],
                || Decision {
                    owner,
                    economy: shared.economy,
                    science: shared.science,
                    culture: g.culture,
                    founding,
                    declared: current.declared,
                    policy: current.policy,
                    previous_score: baseline.total(),
                    selected_score: if current.declared == best.0 && current.policy == best.1 {
                        best.2.total()
                    } else {
                        baseline.total()
                    },
                    cooldown,
                    previous_declared: old.declared,
                    previous_policy: old.policy,
                    previous_components: baseline,
                    selected_components: if current.declared == best.0 && current.policy == best.1 {
                        best.2
                    } else {
                        baseline
                    },
                    required_improvement,
                    best_declared: best.0,
                    best_policy: best.1,
                    best_components: best.2,
                    missionary_opportunity,
                    research_candidate: research,
                    research_interest: interest,
                    actual_research: self.players[owner].researching.0,
                    research_blocked,
                },
            );
        }
    }

    /// A tie preference for the actor-free trade adapter, using known sites only.
    pub(in crate::sim) fn religious_trade_preference(&self, from: u32, to: u32) -> i32 {
        let Some(source) = self.city_by_id(from) else {
            return 0;
        };
        let Some(target) = self.city_by_id(to) else {
            return 0;
        };
        let n = self.national_faith(source.owner);
        if !self.players[source.owner].ai
            || n.policy != FaithPolicy::Missionary
            || n.declared.is_none()
            || n.declared != self.city_dominant_faith(from)
        {
            return 0;
        }
        if source.owner == target.owner {
            return i32::from(self.city_dominant_faith(to) != n.declared);
        }
        if self
            .observed_map
            .cities
            .get(&(source.owner, to))
            .is_none_or(|seen| seen.1.city.owner != target.owner)
            || self.diplomatic_love(source.owner, target.owner) < 0
        {
            return 0;
        }
        self.religious_partner_weight(source.owner, target.owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::fixtures::WorldBuilder;
    fn pair() -> (Simulation, u32, u32) {
        let mut g = Simulation::new(crate::test_support::seeded_map());
        let mut w = WorldBuilder::new(&mut g);
        w.clear_units_and_cities();
        let a = w.founding_city(1, (10, 24));
        let b = w.founding_city(HUMAN, (24, 24));
        w.known_technologies(
            1,
            &[Tech::MYSTICISM, Tech::CEREMONIAL_BURIAL, Tech::POLYTHEISM],
        );
        w.known_technologies(HUMAN, &[Tech::MEDITATION]);
        w.treasury(1, 5000);
        w.ai_control(1, true);
        w.finish();
        (g, a, b)
    }
    #[test]
    fn research_interest_respects_claims_war_recovery_and_negative_advice() {
        let (mut g, a, b) = pair();
        let p = g.strategic_priorities(1);
        assert!(!p.economic_crisis);
        assert!(g.religion_research_interest(1, Tech::MEDITATION, &p) > 0.0);
        assert_eq!(g.religion_research_interest(1, Tech::POTTERY, &p), 0.0);
        let negative = g.strategic_research_want(1, Tech::MEDITATION, -100.0, &p, 1);
        assert!(negative < 0.0);
        g.sign_pact(1, HUMAN, Pact::War);
        let war = g.strategic_priorities(1);
        assert_eq!(war.wars, 1);
        assert_eq!(g.religion_research_interest(1, Tech::MEDITATION, &war), 0.0);
        g.sign_pact(1, HUMAN, Pact::Peace);
        let mut w = WorldBuilder::new(&mut g);
        w.city_buildings(a, &[Building::LIBRARY; 20]);
        w.treasury(1, 0);
        let recovery = g.strategic_priorities(1);
        assert!(recovery.economic_crisis);
        assert_eq!(
            g.religion_research_interest(1, Tech::MEDITATION, &recovery),
            0.0
        );
        assert!(g.found_religion(HUMAN, b, 1));
        assert_eq!(g.religion_research_interest(1, Tech::MEDITATION, &p), 0.0);
    }
    #[test]
    fn missionary_policy_is_chosen_for_useful_domestic_spread() {
        let (mut g, a, _) = pair();
        let mut w = WorldBuilder::new(&mut g);
        w.founding_city(1, (12, 24));
        w.founding_city(1, (14, 24));
        w.finish();
        assert!(g.found_religion(1, a, 2));
        g.run_religion_ai();
        assert_eq!(g.national_faith(1).declared, Some(2));
        assert_eq!(g.national_faith(1).policy, FaithPolicy::Missionary);
    }
    #[test]
    fn devout_solves_real_disorder_and_releases_a_redundant_policy() {
        let (mut g, capital, _) = pair();
        let mut w = WorldBuilder::new(&mut g);
        w.known_technologies(
            1,
            &Tech::all()
                .filter(|t| t.is_civ3_advance())
                .collect::<Vec<_>>(),
        );
        w.government(1, Government::Democracy);
        w.map_surface(crate::sim::fixtures::TileSurface {
            terrain: Some(crate::world::Terrain::GrasslandShield),
            road: Some(true),
            ..Default::default()
        });
        w.finish();
        assert!(g.found_religion(1, capital, 2));
        // Find a native mood boundary instead of assuming a fixed number of
        // content citizens across governments/ruleset effects.
        let mut boundary = None;
        'sizes: for size in 3..=10 {
            for luxury in (0..=100).step_by(10) {
                let mut w = WorldBuilder::new(&mut g);
                w.city_population(capital, size, Some([0; 3]));
                w.city_workers(capital, size as usize);
                w.tax_rates(1, [100 - luxury, 0, luxury]);
                w.finish();
                let city = g.city_by_id(capital).unwrap();
                let output = g.city_forecast(city);
                let before = g.city_mood_with_religious_content(city, output.luxury, 0);
                let after = g.city_mood_with_religious_content(city, output.luxury, 1);
                if before.1 && !after.1 {
                    boundary = Some((size, output.luxury));
                    break 'sizes;
                }
            }
        }
        assert!(
            boundary.is_some(),
            "fixture must expose a real disorder boundary"
        );
        g.run_religion_ai();
        assert_eq!(g.national_faith(1).policy, FaithPolicy::Devout);
        let city = g.city_by_id(capital).unwrap();
        assert!(!g.city_in_disorder(city, g.city_forecast(city).luxury));

        // Once luxury/specialists already cover the need, current Devout
        // content must not be credited as if it were still helping someone.
        let mut w = WorldBuilder::new(&mut g);
        w.start_at(11, -3500);
        w.city_workers(capital, 0);
        w.city_population(capital, 3, Some([0, 0, 3]));
        w.tax_rates(1, [0, 0, 100]);
        w.finish();
        let goals = g.religion_goals(1, g.strategic_priorities(1));
        let devout = g.religious_policy_score(1, Some(2), FaithPolicy::Devout, &goals);
        assert_eq!(devout.content, 0);
        g.run_religion_ai();
        assert_eq!(g.national_faith(1).policy, FaithPolicy::Tolerant);
    }
    #[test]
    fn foreign_spread_uses_known_connections_without_reading_private_beliefs() {
        let (mut g, a, b) = pair();
        assert!(g.found_religion(1, a, 2));
        WorldBuilder::new(&mut g).diplomacy(HUMAN, 1, Pact::Peace, 1, [1000; 2]);
        g.trade_routes.push(trade_routes::TradeRoute {
            from_city_id: a,
            to_city_id: b,
            goods_rule_name: "Goods".into(),
        });
        g.observed_map.cities.remove(&(1, b));
        let goals = g.religion_goals(1, g.strategic_priorities(1));
        assert_eq!(g.missionary_opportunity(1, 2, &goals), 0);
        let mut w = WorldBuilder::new(&mut g);
        w.unit(1, UnitKind::Warrior, (24, 24));
        w.finish();
        assert!(g.observed_map.cities.contains_key(&(1, b)));
        let known = g.missionary_opportunity(1, 2, &goals);
        assert!(known > 0);
        assert!(g.found_religion(HUMAN, b, 1));
        assert_eq!(g.missionary_opportunity(1, 2, &goals), known);
        let before = g.encode_campaign().unwrap();
        let random = g.native_random.clone();
        g.religion_research_interest(1, Tech::MEDITATION, &goals.shared);
        assert_eq!(g.encode_campaign().unwrap(), before);
        assert_eq!(g.native_random, random);
        // An unseen transfer must not reveal a new trading partner's identity.
        assert!(g.declare_faith(1, Some(2), FaithPolicy::Missionary));
        g.cities.iter_mut().find(|c| c.id == b).unwrap().owner = 2;
        WorldBuilder::new(&mut g).diplomacy(1, 2, Pact::Peace, 1, [1000; 2]);
        assert_eq!(g.missionary_opportunity(1, 2, &goals), 0);
        assert_eq!(g.religious_trade_preference(a, b), 0);
    }
}

#[cfg(test)]
mod research_selection_tests {
    use super::*;
    use crate::sim::fixtures::WorldBuilder;
    #[test]
    fn religion_interest_reaches_the_normal_research_selector_and_reacts_to_claims() {
        let mut g = Simulation::new(crate::test_support::seeded_map());
        let mut w = WorldBuilder::new(&mut g);
        w.clear_units_and_cities();
        let rival = w.founding_city(HUMAN, (24, 24));
        w.founding_city(1, (10, 24));
        w.known_technologies(1, &[Tech::CEREMONIAL_BURIAL, Tech::MYSTICISM]);
        w.treasury(1, 5000);
        w.ai_control(1, true);
        w.finish();
        g.clear_ai_tech_wants(1);
        let chosen = g.select_ai_research(1).0;
        let goal = g.players[1].research_goal;
        let faith = rules()
            .faiths
            .iter()
            .find(|f| f.tech() == goal)
            .expect("religious research goal enters the native selector");
        assert!(g.shared_research_path(1, goal).contains(&chosen));
        let priorities = g.strategic_priorities(1);
        assert!(g.religion_research_interest(1, goal, &priorities) > 0.0);
        g.players[HUMAN].known = vec![goal];
        assert!(g.found_religion(HUMAN, rival, faith.id));
        assert_eq!(g.religion_research_interest(1, goal, &priorities), 0.0);
    }
}
