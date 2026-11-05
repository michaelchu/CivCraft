use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SabotageTarget {
    Production,
    Building(Building),
}

enum CityMission {
    Embassy,
    Theft,
    Incite(i32),
    Sabotage,
}

struct AgentRoute {
    city: u32,
    path: Vec<(i32, i32)>,
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

    pub(super) fn ai_city_espionage(&mut self, agent: &Unit) -> bool {
        if agent.moves_left <= 0 {
            return false;
        }
        let budget = self.city_mission_budget(agent.owner);
        let reserved = self.active_agent_targets(agent.owner, Some(agent.id), budget);
        let Some(AgentRoute {
            city: city_id,
            path,
        }) = self.plan_city_mission(agent, budget, true, &reserved)
        else {
            self.agent_city_targets.remove(&agent.id);
            return false;
        };
        self.agent_city_targets.insert(agent.id, city_id);
        let moved = self.follow_path(agent.id, path);
        let action = self
            .units
            .iter()
            .find(|unit| unit.id == agent.id)
            .zip(self.cities.iter().find(|city| city.id == city_id))
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

    fn plan_city_mission(
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
                    && self.map_distance(other.x, other.y, city.x, city.y) <= 1
            }) {
                continue;
            }
            for (x, y) in super::terrain_work::neighbors(city.x, city.y) {
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
                let path = self.path_for_unit(agent, x, y);
                if (x, y) != origin && path.is_empty() {
                    continue;
                }
                let changed_target = self
                    .agent_city_targets
                    .get(&agent.id)
                    .is_some_and(|&target| target != city.id);
                let priority = (changed_target, path.len(), city.id);
                if best
                    .as_ref()
                    .is_none_or(|(previous, _)| priority < *previous)
                {
                    best = Some((priority, path));
                }
            }
        }
        best.map(|((_, _, city), path)| AgentRoute { city, path })
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

    pub(super) fn offensive_agent_production(&self, city: &City) -> Option<UnitKind> {
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
        let target = self.cities.iter().find(|city| city.id == route.city)?;
        let mut projected = agent;
        (projected.x, projected.y) = route
            .path
            .iter()
            .fold((city.x, city.y), |(x, y), &(dx, dy)| {
                offset_native_by_map(x, y, dx, dy)
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
        let travel = (route.path.len() as i32 + kind.stats().moves - 1) / kind.stats().moves;
        let loss = 17 * kind.stats().cost * if kind == UnitKind::Spy { 20 } else { 100 };
        let want = (80 * gain - loss) / 100 - 17 * travel * ((travel + 1) / 2);
        (want > 0).then_some(kind)
    }

    fn diplomatic_threat(&self, city: &City) -> bool {
        self.units.iter().any(|unit| {
            if !unit.kind.is_diplomatic()
                || !self.at_war(city.owner, unit.owner)
                || !self.tile(unit.x, unit.y).known_by[city.owner]
                || self.map_distance(city.x, city.y, unit.x, unit.y) > 2 * unit.kind.stats().moves
            {
                return false;
            }
            super::terrain_work::neighbors(city.x, city.y).any(|(x, y)| {
                if (x, y) == (unit.x, unit.y) {
                    return true;
                }
                let path = self.path_for_unit(unit, x, y);
                !path.is_empty() && path.len() < (2 * unit.kind.stats().moves) as usize
            })
        })
    }

    fn city_has_other_diplomat(&self, city: &City, except: u32) -> bool {
        self.units.iter().any(|unit| {
            unit.id != except
                && unit.owner == city.owner
                && unit.kind.is_diplomatic()
                && unit.transport.is_none()
                && (unit.x, unit.y) == (city.x, city.y)
        })
    }

    fn needs_counterintelligence(&self, city: &City) -> bool {
        !self.city_has_other_diplomat(city, 0)
            && self.units.iter().any(|unit| {
                unit.owner == city.owner
                    && unit.kind.is_military()
                    && unit.transport.is_none()
                    && (unit.x, unit.y) == (city.x, city.y)
            })
            && self.diplomatic_threat(city)
    }

    pub(super) fn counterintelligence_tech_want(&self, owner: usize) -> i32 {
        if self.unit_available(owner, UnitKind::Diplomat)
            || self.unit_available(owner, UnitKind::Spy)
        {
            return 0;
        }
        3000 * self
            .cities
            .iter()
            .filter(|city| city.owner == owner && self.needs_counterintelligence(city))
            .count() as i32
    }

    pub(super) fn counterintelligence_production(&self, city: &City) -> Option<Production> {
        if !self.needs_counterintelligence(city) {
            return None;
        }
        [UnitKind::Spy, UnitKind::Diplomat]
            .into_iter()
            .map(Production::Unit)
            .find(|&target| self.can_produce(city, target))
    }

    pub(super) fn assign_counterintelligence(&mut self, actor: &Unit) -> bool {
        let mut best = None;
        for city in self.cities.iter().filter(|city| {
            city.owner == actor.owner && !self.city_has_other_diplomat(city, actor.id)
        }) {
            let diplomatic = self.diplomatic_threat(city);
            let military = self.city_threats(city).next().is_some();
            if !diplomatic && !military {
                continue;
            }
            if (actor.x, actor.y) == (city.x, city.y) {
                return true;
            }
            let path = self.path_for_unit(actor, city.x, city.y);
            let score = (!diplomatic, path.len(), city.id);
            if !path.is_empty() && best.as_ref().is_none_or(|(previous, _)| score < *previous) {
                best = Some((score, path));
            }
        }
        best.is_some_and(|(_, path)| self.follow_path(actor.id, path))
    }

    pub(crate) fn stealable_technology(&self, owner: usize, other: usize, tech: Tech) -> bool {
        if tech == Tech::FUTURE {
            self.players[other].future_techs > self.players[owner].future_techs
        } else {
            self.has_tech(other, tech) && self.tech_available(owner, tech)
        }
    }

    pub(crate) fn sabotageable_building(building: Building) -> bool {
        building.data().genus == crate::rules::Genus::Improvement
    }

    fn can_target_city_covertly(&self, unit: &Unit, city: &City) -> bool {
        unit.kind.is_diplomatic()
            && unit.moves_left > 0
            && !self.tile(unit.x, unit.y).terrain.is_water()
            && city.owner != unit.owner
            && self.map_distance(unit.x, unit.y, city.x, city.y) <= 1
    }

    pub(crate) fn can_steal_technology(&self, unit: &Unit, city: &City) -> bool {
        city.owner != BARBARIANS && self.can_target_city_covertly(unit, city)
    }

    pub(crate) fn can_sabotage_city(&self, unit: &Unit, city: &City) -> bool {
        self.at_war(unit.owner, city.owner) && self.can_target_city_covertly(unit, city)
    }

    pub(crate) fn can_poison_city(&self, unit: &Unit, city: &City) -> bool {
        unit.kind == UnitKind::Spy
            && unit.transport.is_none()
            && city.size >= 2
            && self.can_sabotage_city(unit, city)
    }

    pub(super) fn poison_city(&mut self, actor: u32, city_id: u32) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == actor).cloned() else {
            return false;
        };
        let Some(index) = self.cities.iter().position(|city| city.id == city_id) else {
            return false;
        };
        let city = self.cities[index].clone();
        if !self.can_poison_city(&unit, &city) {
            return false;
        }
        if !self.resolve_diplomatic_defender(&unit, &city) {
            // Freeciv rolls at 100% after infiltration. The escape roll is separate.
            self.rand_below(100);
            self.reduce_city_population(index);
            if unit.owner == HUMAN || city.owner == HUMAN {
                self.add_event(format!("A Spy poisoned the water supply of {}.", city.name));
            }
            self.finish_covert_mission(&unit, (city.x, city.y), true);
        }
        self.refresh_visibility();
        true
    }

    pub(crate) fn can_sabotage_unit(&self, spy: &Unit, target: &Unit) -> bool {
        spy.kind == UnitKind::Spy
            && spy.moves_left > 0
            && spy.transport.is_none()
            && !self.tile(spy.x, spy.y).terrain.is_water()
            && self.at_war(spy.owner, target.owner)
            && target.hp >= 2
            && self.map_distance(spy.x, spy.y, target.x, target.y) <= 1
            && !self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (target.x, target.y))
            && !self
                .units
                .iter()
                .any(|unit| unit.id != target.id && (unit.x, unit.y) == (target.x, target.y))
    }

    pub(super) fn sabotage_unit(&mut self, actor: u32, target: u32) -> bool {
        let Some(spy) = self.units.iter().find(|unit| unit.id == actor).cloned() else {
            return false;
        };
        let Some(index) = self.units.iter().position(|unit| unit.id == target) else {
            return false;
        };
        let victim = &self.units[index];
        if !self.can_sabotage_unit(&spy, victim)
            || (spy.owner == HUMAN && !self.unit_visible_to_human(victim))
        {
            return false;
        }

        let name = victim.kind.name();
        let notify = spy.owner == HUMAN || victim.owner == HUMAN;
        self.units[index].hp /= 2;
        if notify {
            self.add_event(format!("A Spy sabotaged a {name}."));
        }
        // Unit sabotage escapes from the Spy's tile, unlike city missions.
        self.finish_covert_mission(&spy, (spy.x, spy.y), true);
        self.refresh_visibility();
        true
    }

    pub(crate) fn incite_price(&self, unit: &Unit, city: &City) -> Option<i32> {
        if !self.can_target_city_covertly(unit, city)
            || unit.transport.is_some()
            || self.allied(unit.owner, city.owner)
            || [
                Building::COURTHOUSE,
                Building::PALACE,
                Building::ECCLESIASTICAL_PALACE,
            ]
            .into_iter()
            .any(|building| city.has_building(building))
            || self.has_active_wonder(city.owner, Building::MAUSOLEUM_OF_MAUSOLOS)
        {
            return None;
        }
        let garrison: Vec<_> = self
            .units
            .iter()
            .filter(|unit| (unit.x, unit.y) == (city.x, city.y))
            .collect();
        let units: i64 = garrison
            .iter()
            .map(|unit| i64::from(unit.kind.stats().cost))
            .sum();
        let buildings: i64 = city
            .buildings
            .iter()
            .map(|building| i64::from(building.data().cost))
            .sum();
        let forecast = self.city_forecast(city);
        let (happy, unhappy, angry) = self.city_feelings(city, forecast.luxury);
        let effective_size = (city.size + happy - unhappy - 3 * angry).max(1);
        let distance = self
            .cities
            .iter()
            .filter(|capital| {
                capital.owner == city.owner
                    && (capital.has_building(Building::PALACE)
                        || capital.has_building(Building::ECCLESIASTICAL_PALACE))
            })
            .map(|capital| self.map_distance(city.x, city.y, capital.x, capital.y))
            .min()
            .unwrap_or(32)
            .min(32);
        let stability = (if forecast.disorder { 1 } else { 2 })
            * (if city.was_happy && forecast.happy {
                2
            } else {
                1
            });
        let base = (1000 + i64::from(self.players[city.owner].gold) + 2 * units + buildings)
            * stability
            * i64::from(effective_size);
        let cost_per_citizen = base * 100 / (i64::from(distance + 3) * i64::from(city.size));
        let foreign: i32 = city.foreign_citizens.iter().map(|&(_, count)| count).sum();
        let domestic = city.size - foreign;
        let weighted_citizens = 10 * domestic
            + city
                .foreign_citizens
                .iter()
                .map(|&(owner, count)| {
                    if owner == unit.owner {
                        5 * count
                    } else {
                        7 * count
                    }
                })
                .sum::<i32>();
        let culture = self.nation_scores[city.owner].achievement_culture
            + self
                .cities
                .iter()
                .filter(|other| other.owner == city.owner)
                .map(|other| other.history)
                .sum::<i32>();
        let bonus = 100
            + 100 * i64::from(!garrison.is_empty())
            + 50 * i64::from((culture / 1000).clamp(0, 10));
        Some(
            (cost_per_citizen * i64::from(weighted_citizens) * bonus / 100_000)
                .clamp(0, i64::from(i32::MAX)) as i32,
        )
    }

    pub(super) fn incite_city(&mut self, actor: u32, city_id: u32, quoted_price: i32) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == actor).cloned() else {
            return false;
        };
        let Some(index) = self.cities.iter().position(|city| city.id == city_id) else {
            return false;
        };
        let city = self.cities[index].clone();
        let owner = unit.owner;
        if self.incite_price(&unit, &city) != Some(quoted_price)
            || self.players[owner].gold < quoted_price
        {
            return false;
        }
        if self.resolve_diplomatic_defender(&unit, &city) {
            self.refresh_visibility();
            return true;
        }
        let location = (city.x, city.y);
        if !self.at_war(owner, city.owner) {
            self.record_provocation(city.owner, owner);
        }
        if self.rand_below(100) >= 80 {
            self.finish_covert_mission(&unit, location, false);
            if owner == HUMAN || city.owner == HUMAN {
                self.add_event(format!(
                    "A {} was caught inciting revolt in {}.",
                    unit.kind.name(),
                    city.name
                ));
            }
            self.refresh_visibility();
            return true;
        }

        self.players[owner].gold -= quoted_price;
        if city.size > 1 {
            self.reduce_city_population(index);
        }
        if let Some(tech) = self.random_stealable_technology(owner, city.owner) {
            self.discover(owner, tech);
            if self.players[owner].researching == tech {
                self.players[owner].researching = self.research_target(owner);
            }
        }
        let first_city = !self.cities.iter().any(|other| other.owner == owner);
        let city_after = &mut self.cities[index];
        city_after.transfer_citizens(owner);
        city_after.shields = 0;
        city_after.protected_shields = 0;
        city_after.caravan_shields = 0;
        city_after.production_before_switch = None;
        city_after.bought_production = false;
        city_after.was_happy = false;
        city_after
            .buildings
            .retain(|building| building.data().genus != crate::rules::Genus::SmallWonder);
        if first_city {
            city_after.buildings.push(Building::PALACE);
        }
        for other in &mut self.units {
            if other.owner != city.owner {
                continue;
            }
            let stationed = (other.x, other.y) == location;
            let (dx, dy) = wrapped_map_vector(other.x, other.y, city.x, city.y);
            let in_other_friendly_city = self
                .cities
                .iter()
                .any(|home| home.owner == city.owner && (home.x, home.y) == (other.x, other.y));
            if stationed
                || (other.home_city == Some(city_id)
                    && isohex_vector_distance(dx, dy) <= 1
                    && !in_other_friendly_city)
            {
                other.owner = owner;
                other.home_city = other.home_city.map(|_| city_id);
                other.automated = false;
                other.clear_work();
            }
        }
        self.release_city_support_except(city_id, Some(owner));
        if !self.can_produce(&self.cities[index], self.cities[index].production) {
            self.cities[index].production = self.choose_ai_production(owner, city_id);
        }
        self.prune_trade_routes();
        self.rebuild_city_ownership();
        if owner == HUMAN || city.owner == HUMAN {
            self.add_event(format!(
                "{} revolted and joined {}.",
                city.name, self.players[owner].name
            ));
        }
        self.finish_covert_mission(&unit, location, true);
        self.refresh_visibility();
        self.check_game_over();
        true
    }

    fn charge_covert_movement(&mut self, unit: &Unit, city: &City) {
        let cost = self
            .movement_cost_for(unit, city.x, city.y)
            .unwrap_or(MOVE_FRAGMENTS);
        if let Some(mover) = self.units.iter_mut().find(|other| other.id == unit.id) {
            mover.moves_left = (mover.moves_left - cost).max(0);
        }
    }

    fn finish_covert_mission(&mut self, unit: &Unit, origin: (i32, i32), success: bool) {
        let escape_city = if success && unit.kind == UnitKind::Spy {
            self.cities
                .iter()
                .filter(|home| home.owner == unit.owner)
                .min_by_key(|home| self.map_distance(origin.0, origin.1, home.x, home.y))
                .map(|home| (home.x, home.y, home.name.clone()))
        } else {
            None
        };
        if let Some((x, y, home)) = escape_city
            && self.rand_below(100) < (80 + unit.veteran_power() - 100)
        {
            let spy = self
                .units
                .iter_mut()
                .find(|other| other.id == unit.id)
                .unwrap();
            (spy.x, spy.y) = (x, y);
            spy.transport = None;
            spy.moves_left = 0;
            spy.moved = true;
            spy.state = UnitState::Idle;
            self.promote_unit(unit.id, 100);
            if unit.owner == HUMAN {
                self.add_event(format!("Your Spy escaped to {home}."));
            }
        } else {
            self.units.retain(|other| other.id != unit.id);
            if success && unit.owner == HUMAN {
                self.add_event(format!(
                    "Your {} was captured after the mission.",
                    unit.kind.name()
                ));
            }
        }
    }

    pub(super) fn sabotage_city(
        &mut self,
        actor: u32,
        city_id: u32,
        target: Option<SabotageTarget>,
    ) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == actor).cloned() else {
            return false;
        };
        let Some(index) = self.cities.iter().position(|city| city.id == city_id) else {
            return false;
        };
        let city = self.cities[index].clone();
        if !self.can_sabotage_city(&unit, &city)
            || (target.is_some() && unit.kind != UnitKind::Spy)
            || matches!(target, Some(SabotageTarget::Building(building))
                if !city.has_building(building) || !Self::sabotageable_building(building))
        {
            return false;
        }
        if self.resolve_diplomatic_defender(&unit, &city) {
            self.refresh_visibility();
            return true;
        }

        let mut success = false;
        let name = unit.kind.name();
        let subject = if unit.owner == HUMAN {
            "Your"
        } else {
            "An enemy"
        };
        let mission_chance = if target.is_some() { 40 } else { 80 };
        let outcome = if self.rand_below(100) >= mission_chance {
            format!(
                "{subject} {name} was caught attempting sabotage in {}.",
                city.name
            )
        } else {
            let mut targets: Vec<_> = city
                .buildings
                .iter()
                .copied()
                .filter(|&building| Self::sabotageable_building(building))
                .collect();
            targets.sort_by_key(|building| building.0);
            if target.is_none() && targets.is_empty() && city.shields == 0 {
                self.charge_covert_movement(&unit, &city);
                if unit.owner == HUMAN {
                    self.add_event(format!(
                        "{subject} {name} found nothing to sabotage in {}.",
                        city.name
                    ));
                }
                return true;
            }
            let building = match target {
                Some(SabotageTarget::Production) => None,
                Some(SabotageTarget::Building(building)) => Some(building),
                None if targets.is_empty() || self.rand_below(2) == 1 => None,
                None => Some(targets[self.rand_below(targets.len() as i32) as usize]),
            };
            if building.is_none() {
                let target = &mut self.cities[index];
                success = true;
                target.shields = 0;
                target.protected_shields = 0;
                target.caravan_shields = 0;
                target.production_before_switch = None;
                format!(
                    "{subject} {name} destroyed the production stockpile in {}.",
                    city.name
                )
            } else {
                let building = building.unwrap();
                let vulnerability = if building == Building::CITY_WALLS {
                    50
                } else {
                    100
                };
                let capital = city.has_building(Building::PALACE)
                    || city.has_building(Building::ECCLESIASTICAL_PALACE);
                if self.rand_below(100)
                    < vulnerability - percent_of(vulnerability, 50 * i32::from(capital))
                {
                    success = true;
                    self.cities[index]
                        .buildings
                        .retain(|&built| built != building);
                    format!(
                        "{subject} {name} destroyed {} in {}.",
                        building.data().name,
                        city.name
                    )
                } else {
                    format!(
                        "{subject} {name} was caught sabotaging {} in {}.",
                        building.data().name,
                        city.name
                    )
                }
            }
        };
        if unit.owner == HUMAN || city.owner == HUMAN {
            self.add_event(outcome);
        }
        self.finish_covert_mission(&unit, (city.x, city.y), success);
        self.refresh_visibility();
        true
    }

    fn resolve_diplomatic_defender(&mut self, actor: &Unit, city: &City) -> bool {
        let defender = self
            .units
            .iter()
            .find(|unit| {
                unit.owner != actor.owner
                    && (unit.x, unit.y) == (city.x, city.y)
                    && unit.kind.is_diplomatic()
            })
            .map(|unit| (unit.id, unit.kind, unit.veteran_power()));
        let Some((defender, defender_kind, defender_power)) = defender else {
            return false;
        };
        let resistance = 50
            * i32::from(
                city.has_building(Building::PALACE)
                    || city.has_building(Building::ECCLESIASTICAL_PALACE),
            )
            + 25 * i32::from(matches!(
                self.tile(city.x, city.y).base,
                Some(Base::Fort | Base::Fortress)
            ));
        let chance = 50
            + 25 * (i32::from(actor.kind == UnitKind::Spy)
                - i32::from(defender_kind == UnitKind::Spy))
            + actor.veteran_power()
            - defender_power;
        let chance = (chance - percent_of(chance, resistance)).clamp(0, 100);
        let attacker_won = self.rand_below(100) < chance;
        let loser = if attacker_won { defender } else { actor.id };
        self.units.retain(|unit| unit.id != loser);
        if attacker_won {
            let unit = self
                .units
                .iter_mut()
                .find(|unit| unit.id == actor.id)
                .unwrap();
            unit.moves_left = (unit.moves_left - MOVE_FRAGMENTS).max(0);
        } else if !self.at_war(actor.owner, city.owner) {
            self.record_provocation(city.owner, actor.owner);
        }
        let (winner, modifier) = if attacker_won {
            (actor.id, (100 - chance) * 2)
        } else {
            (defender, chance * 2)
        };
        self.promote_unit(winner, modifier);
        if actor.owner == HUMAN {
            let detail = if attacker_won {
                format!(
                    "Your {} eliminated a defending {}.",
                    actor.kind.name(),
                    defender_kind.name()
                )
            } else {
                format!(
                    "A defending {} eliminated your {}.",
                    defender_kind.name(),
                    actor.kind.name()
                )
            };
            self.add_event(detail);
        }
        true
    }

    fn random_stealable_technology(&mut self, owner: usize, other: usize) -> Option<Tech> {
        let available: Vec<_> = Tech::all()
            .filter(|&tech| self.stealable_technology(owner, other, tech))
            .collect();
        if available.is_empty() {
            self.stealable_technology(owner, other, Tech::FUTURE)
                .then_some(Tech::FUTURE)
        } else {
            Some(available[self.rand_below(available.len() as i32) as usize])
        }
    }

    pub(super) fn steal_technology(
        &mut self,
        actor: u32,
        city_id: u32,
        target: Option<Tech>,
    ) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == actor).cloned() else {
            return false;
        };
        let Some(index) = self.cities.iter().position(|city| city.id == city_id) else {
            return false;
        };
        let city = self.cities[index].clone();
        if !self.can_steal_technology(&unit, &city)
            || target.is_some_and(|tech| {
                unit.kind != UnitKind::Spy
                    || !self.stealable_technology(unit.owner, city.owner, tech)
            })
        {
            return false;
        }
        if self.resolve_diplomatic_defender(&unit, &city) {
            self.refresh_visibility();
            return true;
        }

        let repeated = city.technology_thefts > 0 && unit.kind == UnitKind::Diplomat;
        let failed = repeated
            || (0..=city.technology_thefts + u32::from(target.is_some()))
                .any(|_| self.rand_below(100) >= 80);
        if failed {
            self.units.retain(|other| other.id != actor);
            if !self.at_war(unit.owner, city.owner) {
                self.record_provocation(city.owner, unit.owner);
            }
            if unit.owner == HUMAN {
                self.add_event(if repeated {
                    format!(
                        "{} was prepared for another theft. Your Diplomat was caught.",
                        city.name
                    )
                } else {
                    format!(
                        "Your {} was caught stealing technology from {}.",
                        unit.kind.name(),
                        city.name
                    )
                });
            }
            self.refresh_visibility();
            return true;
        }

        let technology =
            target.or_else(|| self.random_stealable_technology(unit.owner, city.owner));
        let Some(tech) = technology else {
            self.charge_covert_movement(&unit, &city);
            if unit.owner == HUMAN {
                self.add_event(format!("No new technology was found in {}.", city.name));
            }
            return true;
        };

        self.discover(unit.owner, tech);
        if self.players[unit.owner].researching == tech {
            self.players[unit.owner].researching = self.research_target(unit.owner);
        }
        self.cities[index].technology_thefts += 1;
        if !self.at_war(unit.owner, city.owner) {
            self.record_provocation(city.owner, unit.owner);
        }
        if unit.owner == HUMAN {
            self.add_event(format!(
                "Your {} stole {} from {}.",
                unit.kind.name(),
                tech.name(),
                city.name
            ));
        } else if city.owner == HUMAN {
            self.add_event(format!(
                "{} stole {} from {}.",
                self.players[unit.owner].name,
                tech.name(),
                city.name
            ));
        }
        self.finish_covert_mission(&unit, (city.x, city.y), true);
        self.refresh_visibility();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targeted_missions_use_the_chosen_building_and_apply_the_harder_roll() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let spy = game.spy_playtest_setup();
        let foreign = game.cities[0].id;
        game.cities[0]
            .buildings
            .extend([Building::GRANARY, Building::TEMPLE]);
        game.cities[0].shields = 9;
        assert!(!game.apply(Command::TargetedSabotage(
            spy,
            foreign,
            SabotageTarget::Building(Building::PALACE)
        )));
        game.rng = 10;
        assert!(game.apply(Command::TargetedSabotage(
            spy,
            foreign,
            SabotageTarget::Building(Building::TEMPLE)
        )));
        assert!(!game.cities[0].has_building(Building::TEMPLE));
        assert!(game.cities[0].has_building(Building::GRANARY));
        assert_eq!(game.cities[0].shields, 9);

        let (x, y) = offset_native_by_map(game.cities[0].x, game.cities[0].y, 1, 0);
        let second = game.spawn_unit(HUMAN, UnitKind::Spy, x, y);
        game.rng = 1; // 61 would pass an 80% random mission, but fails targeted sabotage.
        assert!(game.apply(Command::TargetedSabotage(
            second,
            foreign,
            SabotageTarget::Production
        )));
        assert_eq!(game.cities[0].shields, 9);
        assert!(game.units.iter().all(|unit| unit.id != second));
    }

    #[test]
    fn espionage_replaces_diplomat_production_and_spy_can_escape_after_repeat_theft() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let spy = game.spy_playtest_setup();
        let foreign = game.cities[0].id;
        let home = game.cities[1].clone();
        assert!(game.can_produce(&home, Production::Unit(UnitKind::Spy)));
        assert!(!game.can_produce(&home, Production::Unit(UnitKind::Diplomat)));
        assert!(game.apply(Command::StealTechnology(spy, foreign)));
        assert!(game.has_tech(HUMAN, Tech::POTTERY));
        assert_eq!(game.cities[0].technology_thefts, 2);
        let escaped = game.units.iter().find(|unit| unit.id == spy).unwrap();
        assert_eq!(
            (escaped.x, escaped.y, escaped.moves_left),
            (home.x, home.y, 0)
        );
    }

    #[test]
    fn sabotage_can_destroy_a_building_or_clear_production_without_recovery() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let diplomat = game.sabotage_playtest_setup();
        let id = game.cities[0].id;
        let actor = game
            .units
            .iter()
            .find(|unit| unit.id == diplomat)
            .unwrap()
            .clone();
        game.cities[0].buildings.push(Building::GRANARY);
        game.rng = 4; // Pass infiltration, choose a building, then pass capital resistance.
        assert!(game.apply(Command::SabotageCity(diplomat, id)));
        assert!(game.cities[0].has_building(Building::PALACE));
        assert!(!game.cities[0].has_building(Building::GRANARY));
        assert_eq!(game.cities[0].shields, 9);

        let next = game.spawn_unit(HUMAN, UnitKind::Diplomat, actor.x, actor.y);
        game.rng = 1;
        assert!(game.apply(Command::SabotageCity(next, id)));
        assert_eq!(game.cities[0].shields, 0);
        assert_eq!(game.cities[0].protected_shields, 0);
        assert!(game.cities[0].production_before_switch.is_none());
    }

    #[test]
    fn successful_theft_grants_one_technology_and_the_city_catches_repeat_diplomats() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let diplomat = game.technology_theft_playtest_setup();
        let city = game.cities[0].id;
        let unit = game
            .units
            .iter()
            .find(|unit| unit.id == diplomat)
            .unwrap()
            .clone();
        assert!(game.apply(Command::StealTechnology(diplomat, city)));
        assert!(game.has_tech(HUMAN, Tech::POTTERY));
        assert!(game.has_tech(1, Tech::POTTERY));
        assert_eq!(game.cities[0].technology_thefts, 1);

        game.players[1].known.push(Tech::BRONZE_WORKING);
        let second = game.spawn_unit(HUMAN, UnitKind::Diplomat, unit.x, unit.y);
        assert!(game.apply(Command::StealTechnology(second, city)));
        assert!(!game.has_tech(HUMAN, Tech::BRONZE_WORKING));
        assert!(game.units.iter().all(|unit| unit.id != second));
    }

    #[test]
    fn a_defending_diplomat_intercepts_the_mission() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let diplomat = game.technology_theft_playtest_setup();
        let (id, x, y) = (game.cities[0].id, game.cities[0].x, game.cities[0].y);
        let guard = game.spawn_unit(1, UnitKind::Diplomat, x, y);
        assert!(game.apply(Command::StealTechnology(diplomat, id)));
        assert!(!game.has_tech(HUMAN, Tech::POTTERY));
        assert_eq!(game.cities[0].technology_thefts, 0);
        assert!(game.units.iter().any(|unit| unit.id == guard));
        assert!(game.units.iter().all(|unit| unit.id != diplomat));
    }
}
