//! Leader selection and leader-specific game behavior.

use super::exploration::IndexedExplorerQueue;
use super::pf_actions::PfAction;
use super::*;
use crate::rules::Genus;

struct LeaderPaths {
    costs: Vec<i32>,
    order: Vec<usize>,
}

impl Simulation {
    /// A combat-earned Great Leader can form an Army in an owned city,
    /// provided the empire can support another one (one Army per four cities).
    pub(crate) fn can_form_army(&self, unit: &Unit) -> bool {
        unit.kind == UnitKind::Leader
            && unit.moves_left > 0
            && unit.transport.is_none()
            && self
                .cities
                .iter()
                .any(|city| city.owner == unit.owner && (city.x, city.y) == (unit.x, unit.y))
            && self
                .cities
                .iter()
                .filter(|city| city.owner == unit.owner)
                .count()
                / crate::rules::engine_rules::GreatLeaderSettings::campaign().cities_per_army
                > self
                    .units
                    .iter()
                    .filter(|army| army.owner == unit.owner && army.kind == UnitKind::Army)
                    .count()
    }

    pub(in crate::sim) fn form_army(&mut self, unit_id: u32) -> bool {
        let Some(leader) = self
            .unit_by_id(unit_id)
            .filter(|unit| self.can_form_army(unit))
            .cloned()
        else {
            return false;
        };
        let city_id = self
            .cities
            .iter()
            .find(|city| city.owner == leader.owner && (city.x, city.y) == (leader.x, leader.y))
            .unwrap()
            .id;
        self.remove_units_with_cargo_because(vec![unit_id], lifecycle::UnitRemoval::Expended);
        let army_id = self.spawn_unit_with_origin(
            leader.owner,
            UnitKind::Army,
            leader.x,
            leader.y,
            lifecycle::UnitOrigin::ArmyFormed {
                leader_id: unit_id,
                city_id,
            },
        );
        let army = self
            .units
            .iter_mut()
            .find(|unit| unit.id == army_id)
            .unwrap();
        army.home_city = Some(city_id);
        army.home_order = army_id;
        army.moves_left = army.moves_left.min(leader.moves_left);
        if leader.owner == HUMAN {
            self.add_event("A Great Leader formed an Army.");
        }
        if self.trace.wants::<military_observation::LeaderEvent>() {
            let capacity = self
                .unit_by_id(army_id)
                .map(|army| self.unit_transport_capacity(army));
            self.trace_leader_event(military_observation::LeaderEvent {
                owner: leader.owner,
                action: "formed_army",
                unit_id: Some(unit_id),
                leader_id: Some(unit_id),
                army_id: Some(army_id),
                city_id: Some(city_id),
                kind: Some(UnitKind::Army.name()),
                building: None,
                attacked: None,
                denominator: None,
                heroic_epic: None,
                shields_saved: None,
                can_form_army: Some(true),
                members_now: Some(0),
                capacity,
            });
        }
        true
    }

    /// Whether a Great Leader may complete `city`'s current production: it must
    /// be in its owner's city, and that city must be building an improvement,
    /// small wonder or Great Wonder it can legally build. Coinage, spaceship
    /// parts and units are excluded, and so is production already completed or
    /// bought this turn.
    pub(crate) fn can_complete_production(&self, unit: &Unit, city: &City) -> bool {
        unit.kind == UnitKind::Leader
            && unit.moves_left > 0
            && unit.owner == city.owner
            && (unit.x, unit.y) == (city.x, city.y)
            && self.leader_can_complete_in(city)
    }

    /// The part of the rule that depends only on the city and its production.
    pub(in crate::sim) fn leader_can_complete_in(&self, city: &City) -> bool {
        !city.bought_production
            && matches!(city.production, Production::Building(building)
            if matches!(
                building.data().genus,
                Genus::Improvement | Genus::SmallWonder | Genus::GreatWonder
            ))
            && self.can_produce(city, city.production)
            && city.shields < city.production.cost()
    }

    /// Sets the city's stock to the item's cost and consumes the Leader. The
    /// production is then locked for the turn exactly as a purchase locks it,
    /// so the stock cannot be moved to a different item.
    pub(in crate::sim) fn complete_production(&mut self, unit_id: u32, city_id: u32) -> bool {
        let Some(unit) = self.unit_by_id(unit_id).cloned() else {
            return false;
        };
        let Some(city_index) = self
            .cities
            .iter()
            .position(|city| city.id == city_id && self.can_complete_production(&unit, city))
        else {
            return false;
        };
        let can_form_army = self.can_form_army(&unit);
        let city = &mut self.cities[city_index];
        let shields_saved = (city.production.cost() - city.shields).max(0);
        let building = match city.production {
            Production::Building(building) => Some(building.data().name),
            Production::Unit(_) => None,
        };
        city.shields = city.production.cost();
        city.bought_production = true;
        let city_name = city.name.clone();
        self.remove_units_with_cargo_because(vec![unit_id], lifecycle::UnitRemoval::Expended);
        self.add_event(format!("A Leader completed production in {city_name}."));
        if self.trace.wants::<military_observation::LeaderEvent>() {
            self.trace_leader_event(military_observation::LeaderEvent {
                owner: unit.owner,
                action: "rushed_building",
                unit_id: Some(unit_id),
                leader_id: Some(unit_id),
                army_id: None,
                city_id: Some(city_id),
                kind: None,
                building,
                attacked: None,
                denominator: None,
                heroic_epic: None,
                shields_saved: Some(shields_saved),
                can_form_army: Some(can_form_army),
                members_now: None,
                capacity: None,
            });
        }
        true
    }

    // daiunit.c's reverse query is deliberately NOT an ordinary enemy route:
    // its owner is the leader; it has no ZOC/occupant callback, assumes full
    // enemy movement, and considers the target attackable even for transports.
    fn leader_paths(&self, unit: &Unit, reverse: Option<(usize, usize)>) -> LeaderPaths {
        self.leader_paths_mode(unit, reverse, false)
    }

    fn leader_paths_mode(
        &self,
        unit: &Unit,
        reverse: Option<(usize, usize)>,
        observed: bool,
    ) -> LeaderPaths {
        let rate = self.unit_move_rate(unit);
        let mut actor = unit.clone();
        if let Some((_, owner)) = reverse {
            actor.owner = owner;
        }
        let initial = if reverse.is_some() {
            rate
        } else {
            unit.moves_left.max(0)
        };
        let start = self.map_wrap.index(unit.x, unit.y);
        let mut result = LeaderPaths {
            costs: vec![i32::MAX; self.tiles.len()],
            order: Vec::new(),
        };
        // Reverse PF rejects rawcost >= rate*(3+1), including its own
        // initial zero-cost position when the candidate's rate is zero.
        if reverse.is_some() && rate == 0 {
            return result;
        }
        result.costs[start] = 0;
        let mut queue = IndexedExplorerQueue::new(self.tiles.len());
        queue.put(0, start);
        while let Some((spent, current)) = queue.pop() {
            let spent = spent as i32;
            if result.costs[current] != spent {
                continue;
            }
            if reverse.is_some() && spent >= 4 * rate {
                break;
            }
            result.order.push(current);
            if reverse.is_some_and(|(target, _)| current == target) {
                break;
            }
            let (x, y) = self.map_wrap.position(current);
            let known_here = !observed || self.tile(x, y).known_by[actor.owner];
            let action_here = if reverse.is_some_and(|(target, _)| current == target) {
                PfAction::Attack
            } else if reverse.is_some() {
                PfAction::None
            } else {
                self.pf_default_action(
                    actor.kind,
                    actor.owner,
                    (x, y),
                    known_here,
                    !observed || self.pf_main_sight(actor.owner, x, y),
                    !observed,
                )
            };
            if !matches!(action_here, PfAction::None | PfAction::Impossible) {
                continue;
            }
            if rate == 0 && spent >= initial {
                continue;
            }
            (actor.x, actor.y) = (x, y);
            let remaining = if spent < initial {
                initial - spent
            } else {
                rate - (spent - initial) % rate
            };
            for (dx, dy) in CIV3_DIRS {
                let Some((nx, ny)) = self.map_wrap.civ3_offset(x, y, dx, dy) else {
                    continue;
                };
                let next = self.map_wrap.index(nx, ny);
                let target = reverse.is_some_and(|(target, _)| next == target);
                let known = !observed || self.tile(nx, ny).known_by[actor.owner];
                if known
                    && !actor.kind.is_civilian()
                    && self
                        .tile(nx, ny)
                        .owner
                        .is_some_and(|owner| self.pact(actor.owner, owner) == Pact::Peace)
                    || (reverse.is_none()
                        && known
                        && (!observed || self.tile(x, y).known_by[actor.owner])
                        && self.zone_blocks_step(&actor, (x, y), (nx, ny)))
                {
                    continue;
                }
                let action = if target {
                    PfAction::Attack
                } else if reverse.is_some() {
                    PfAction::None
                } else {
                    self.pf_default_action(
                        actor.kind,
                        actor.owner,
                        (nx, ny),
                        known,
                        !observed || self.pf_main_sight(actor.owner, nx, ny),
                        !observed,
                    )
                };
                if action == PfAction::Impossible {
                    continue;
                }
                let cost = if action != PfAction::None {
                    if reverse.is_none()
                        && known_here
                        && !self.pf_action_step_possible(&actor, action)
                    {
                        continue;
                    }
                    Self::pf_action_move_cost(actor.kind, action, rate)
                } else if !known {
                    if unit.kind.is_naval() {
                        2 * campaign_move_fragments()
                    } else if unit.kind.stats().ignores_terrain {
                        4
                    } else {
                        6 * campaign_move_fragments()
                    }
                } else {
                    let Some(cost) = self.movement_cost_for(&actor, nx, ny).or_else(|| {
                        self.boarding_target(&actor, nx, ny)
                            .map(|_| campaign_move_fragments())
                    }) else {
                        continue;
                    };
                    cost
                };
                let candidate = spent + cost.min(remaining);
                if candidate < result.costs[next] && (reverse.is_none() || candidate < 4 * rate) {
                    let new_entry = result.costs[next] == i32::MAX;
                    self.trace.count_operation(
                        if new_entry {
                            "leader_route.queue_inserts"
                        } else {
                            "leader_route.queue_decrease_keys"
                        },
                        1,
                    );
                    if !new_entry {
                        self.trace.count_operation(
                            "leader_route.queue_frontier_entries_at_decrease_key",
                            queue.frontier_len() as u64,
                        );
                    }
                    result.costs[next] = candidate;
                    queue.put(i64::from(candidate), next);
                }
            }
        }
        result
    }

    pub(super) fn ai_movement_cost_order(&self, unit: &Unit) -> Vec<(i32, i32, i32)> {
        let paths = self.leader_paths(unit, None);
        paths
            .order
            .into_iter()
            .map(|index| {
                let (x, y) = self.map_wrap.position(index);
                (paths.costs[index], x, y)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
