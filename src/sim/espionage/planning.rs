//! Candidate scoring and plan construction for the parent automation system.

use super::*;
use std::collections::BTreeSet;

enum CityMission {
    Embassy,
    Theft,
    Incite(i32),
    Sabotage,
}

pub(super) struct AgentRoute {
    pub(super) city: u32,
    pub(super) path: Vec<(i32, i32)>,
    pub(super) cost: i32,
}

impl Simulation {
    fn city_mission(
        &self,
        agent: &Unit,
        city: &City,
        budget: i32,
        allow_sabotage: bool,
    ) -> Option<CityMission> {
        if !self.has_embassy(agent.owner, city.owner) && self.can_establish_embassy(agent, city) {
            return Some(CityMission::Embassy);
        }
        if self.allied(agent.owner, city.owner)
            || city.owner == BARBARIANS
            || self.city_has_other_diplomat(city, 0)
        {
            return None;
        }
        if self.can_steal_technology(agent, city)
            && (agent.kind == UnitKind::Spy || city.technology_thefts == 0)
            && Tech::all()
                .chain([Tech::FUTURE])
                .any(|tech| self.stealable_technology(agent.owner, city.owner, tech))
        {
            return Some(CityMission::Theft);
        }
        if let Some(price) = self
            .incite_price(agent, city)
            .filter(|&price| price <= budget)
        {
            return Some(CityMission::Incite(price));
        }
        (allow_sabotage
            && self.can_sabotage_city(agent, city)
            && (city.shields > 0
                || city
                    .buildings
                    .iter()
                    .copied()
                    .any(Self::sabotageable_building)))
        .then_some(CityMission::Sabotage)
    }

    pub(in crate::sim) fn ai_city_espionage(&mut self, agent: &Unit) -> bool {
        if agent.moves_left <= 0 {
            return false;
        }
        let budget = self.city_mission_budget(agent.owner);
        let reserved = self.active_agent_targets(agent.owner, Some(agent.id), budget);
        let Some(AgentRoute {
            city: city_id,
            path,
            ..
        }) = self.plan_city_mission(agent, budget, true, &reserved)
        else {
            self.agent_city_targets.remove(&agent.id);
            return false;
        };
        self.agent_city_targets.insert(agent.id, city_id);
        let moved = self.follow_civ3_path(agent.id, path);
        let action = self
            .units
            .iter()
            .find(|unit| unit.id == agent.id)
            .zip(self.city_by_id(city_id))
            .and_then(|(unit, city)| self.city_mission(unit, city, budget, true));
        let acted = match action {
            Some(CityMission::Embassy) => self.establish_embassy(agent.id, city_id),
            Some(CityMission::Theft) => self.steal_technology(agent.id, city_id, None),
            Some(CityMission::Incite(price)) => self.incite_city(agent.id, city_id, price),
            Some(CityMission::Sabotage) => self.sabotage_city(agent.id, city_id, None),
            None => false,
        };
        if acted || !moved || self.units.iter().all(|unit| unit.id != agent.id) {
            self.agent_city_targets.remove(&agent.id);
        }
        acted || moved
    }

    fn city_mission_budget(&self, owner: usize) -> i32 {
        let expenses: i32 = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| self.building_upkeep(city) + self.city_unit_upkeep(city).gold)
            .sum();
        self.players[owner].gold - 2 * expenses
    }

    pub(super) fn plan_city_mission(
        &self,
        agent: &Unit,
        budget: i32,
        allow_sabotage: bool,
        reserved: &BTreeSet<u32>,
    ) -> Option<AgentRoute> {
        let mut best = None;
        let origin = (agent.x, agent.y);
        for city in self.cities.iter().filter(|city| {
            city.owner != agent.owner
                && city.owner != BARBARIANS
                && !reserved.contains(&city.id)
                && self.tile(city.x, city.y).known_by[agent.owner]
        }) {
            if self.units.iter().any(|other| {
                other.id != agent.id
                    && other.owner == agent.owner
                    && other.kind.is_diplomatic()
                    && self
                        .map_wrap
                        .civ3_distance(other.x, other.y, city.x, city.y)
                        <= 1
            }) {
                continue;
            }
            for (x, y) in self.map_wrap.civ3_neighbors(city.x, city.y) {
                if self.tile(x, y).terrain.is_water()
                    || self.best_defender_id(agent, x, y).is_some()
                    || self.cities.iter().any(|city| {
                        (city.x, city.y) == (x, y) && !self.allied(agent.owner, city.owner)
                    })
                {
                    continue;
                }
                let mut projected = agent.clone();
                (projected.x, projected.y) = (x, y);
                if self
                    .city_mission(&projected, city, budget, allow_sabotage && (x, y) == origin)
                    .is_none()
                {
                    continue;
                }
                let Some((path, cost)) = self.search_route(agent, x, y, false, i32::MAX) else {
                    continue;
                };
                let changed_target = self
                    .agent_city_targets
                    .get(&agent.id)
                    .is_some_and(|&target| target != city.id);
                let priority = (changed_target, cost, city.id);
                if best
                    .as_ref()
                    .is_none_or(|(previous, _)| priority < *previous)
                {
                    best = Some((priority, path));
                }
            }
        }
        best.map(|((_, cost, city), path)| AgentRoute { city, path, cost })
    }

    fn active_agent_targets(
        &self,
        owner: usize,
        before: Option<u32>,
        budget: i32,
    ) -> BTreeSet<u32> {
        let mut reserved: BTreeSet<_> = self
            .units
            .iter()
            .filter(|agent| {
                agent.owner == owner && agent.kind.is_diplomatic() && Some(agent.id) != before
            })
            .filter_map(|agent| self.agent_city_targets.get(&agent.id).copied())
            .filter(|id| {
                self.cities
                    .iter()
                    .any(|city| city.id == *id && city.owner != owner)
            })
            .collect();
        for agent in self.units.iter().filter(|agent| {
            agent.owner == owner
                && agent.kind.is_diplomatic()
                && agent.transport.is_none()
                && !self.agent_city_targets.contains_key(&agent.id)
                && before.is_none_or(|id| agent.id < id)
        }) {
            let defending = self.cities.iter().any(|city| {
                city.owner == owner
                    && (city.x, city.y) == (agent.x, agent.y)
                    && !self.city_has_other_diplomat(city, agent.id)
                    && (self.diplomatic_threat(city) || self.city_threats(city).next().is_some())
            });
            if !defending
                && let Some(route) = self.plan_city_mission(agent, budget, true, &reserved)
            {
                reserved.insert(route.city);
            }
        }
        reserved
    }

    pub(in crate::sim) fn offensive_agent_production(&self, city: &City) -> Option<UnitKind> {
        self.offensive_agent_choice(city).map(|(kind, _)| kind)
    }

    pub(in crate::sim) fn offensive_agent_choice(&self, city: &City) -> Option<(UnitKind, f64)> {
        let owner = city.owner;
        let kind = [UnitKind::Spy, UnitKind::Diplomat]
            .into_iter()
            .find(|&kind| self.can_produce(city, Production::Unit(kind)))?;
        let agent = self.make_unit(0, owner, kind, city.x, city.y);
        let budget = self.city_mission_budget(owner);
        let mut reserved = self.active_agent_targets(owner, None, budget);
        for other in self
            .cities
            .iter()
            .filter(|other| other.owner == owner && other.id != city.id)
        {
            if let Production::Unit(kind) = other.production
                && kind.is_diplomatic()
                && self.counterintelligence_production(other).is_none()
            {
                let trainee = self.make_unit(0, owner, kind, other.x, other.y);
                if let Some(route) = self.plan_city_mission(&trainee, budget, false, &reserved) {
                    reserved.insert(route.city);
                }
            }
        }
        let route = self.plan_city_mission(&agent, budget, false, &reserved)?;
        let target = self.city_by_id(route.city)?;
        let mut projected = agent;
        (projected.x, projected.y) =
            route
                .path
                .iter()
                .fold((city.x, city.y), |(x, y), &(dx, dy)| {
                    self.map_wrap
                        .civ3_offset(x, y, dx, dy)
                        .expect("planned step is real")
                });
        let mut gain = 0;
        if !self.allied(owner, target.owner) && !self.city_has_other_diplomat(target, 0) {
            if (kind == UnitKind::Spy || target.technology_thefts == 0)
                && Tech::all()
                    .chain([Tech::FUTURE])
                    .any(|tech| self.stealable_technology(owner, target.owner, tech))
            {
                gain = 18 * self.target_cost(owner, self.players[owner].researching);
            }
            if let Some(price) = self
                .incite_price(&projected, target)
                .filter(|&price| price <= budget)
                && self.at_war(owner, target.owner)
            {
                let (food, shields, trade) = self.city_yields(target);
                gain = gain.max((30 * food + 17 * shields + 18 * trade) * 17 - 18 * price);
            }
        }
        let allowance = self.unit_move_allowance(owner, kind).max(1);
        let travel = (route.cost / allowance + 1).max(1);
        let delay = travel * ((travel + 1) / 2);
        let loss = 17 * kind.stats().cost * if kind == UnitKind::Spy { 20 } else { 100 };
        let want = (80 * gain - loss) / 100 - 17 * delay;
        if want <= 0 {
            return None;
        }
        // ai/default/daitools.c:military_amortize uses the greater of this
        // city's production and the empire average to estimate build time.
        let outputs: Vec<_> = self
            .cities
            .iter()
            .filter(|other| other.owner == owner)
            .map(|other| self.city_forecast(other).shield_surplus())
            .collect();
        let average = outputs.iter().sum::<i32>() / outputs.len().max(1) as i32;
        let production = self
            .city_forecast(city)
            .shield_surplus()
            .max(average)
            .max(1);
        let build_time = kind.stats().cost / production;
        let mut want = f64::from(want) * (23.0_f64 / 24.0).powi(delay + build_time);
        if !self.has_embassy(owner, target.owner) {
            want = want.max(99.0);
        }
        Some((kind, want))
    }
}
