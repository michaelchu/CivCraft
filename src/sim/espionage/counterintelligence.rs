//! Detection and prevention of hostile espionage.

use super::*;

impl Simulation {
    pub(super) fn diplomatic_threat(&self, city: &City) -> bool {
        self.units.iter().any(|unit| {
            let allowance = self.unit_move_rate(unit).max(1);
            if !unit.kind.is_diplomatic()
                || !self.at_war(city.owner, unit.owner)
                || !self.tile(unit.x, unit.y).known_by[city.owner]
                || self.map_distance(city.x, city.y, unit.x, unit.y) > 2 * allowance
            {
                return false;
            }
            let mut ready = unit.clone();
            ready.moves_left = allowance;
            self.map_wrap.civ3_neighbors(city.x, city.y).any(|(x, y)| {
                self.route_cost_within(&ready, x, y, 2 * allowance - 1)
                    .is_some()
            })
        })
    }

    pub(super) fn city_has_other_diplomat(&self, city: &City, except: u32) -> bool {
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

    pub(in crate::sim) fn counterintelligence_tech_want(&self, owner: usize) -> i32 {
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

    pub(in crate::sim) fn counterintelligence_production(&self, city: &City) -> Option<Production> {
        if !self.needs_counterintelligence(city) {
            return None;
        }
        [UnitKind::Spy, UnitKind::Diplomat]
            .into_iter()
            .map(Production::Unit)
            .find(|&target| self.can_produce(city, target))
    }

    pub(in crate::sim) fn holds_counterintelligence(&self, actor: &Unit) -> bool {
        self.cities.iter().any(|city| {
            city.owner == actor.owner
                && (actor.x, actor.y) == (city.x, city.y)
                && !self.city_has_other_diplomat(city, actor.id)
                && (self.diplomatic_threat(city) || self.city_threats(city).next().is_some())
        })
    }

    pub(in crate::sim) fn assign_counterintelligence(&mut self, actor: &Unit) -> bool {
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
            let Some((path, cost)) = self.search_route(actor, city.x, city.y, false, i32::MAX)
            else {
                continue;
            };
            let score = (!diplomatic, cost, city.id);
            if !path.is_empty() && best.as_ref().is_none_or(|(previous, _)| score < *previous) {
                best = Some((score, path));
            }
        }
        best.is_some_and(|(_, path)| self.follow_civ3_path(actor.id, path))
    }
}
