//! Leader selection and leader-specific game behavior.

use super::exploration::ExplorerQueue;
use super::pf_actions::PfAction;
use super::*;

mod playtest;

struct LeaderPaths {
    costs: Vec<i32>,
    order: Vec<usize>,
}

impl Simulation {
    pub(crate) fn can_help_wonder(&self, unit: &Unit, city: &City) -> bool {
        unit.kind == UnitKind::Leader
            && unit.moves_left > 0
            && unit.owner == city.owner
            && (unit.x, unit.y) == (city.x, city.y)
            && matches!(city.production, Production::Building(_))
            && city.shields < city.production.cost()
    }

    pub(in crate::sim) fn help_wonder(&mut self, unit_id: u32, city_id: u32) -> bool {
        let Some(unit) = self.unit_by_id(unit_id).cloned() else {
            return false;
        };
        let Some(city_index) = self
            .cities
            .iter()
            .position(|city| city.id == city_id && self.can_help_wonder(&unit, city))
        else {
            return false;
        };
        let city_name = self.cities[city_index].name.clone();
        self.cities[city_index].shields = self.cities[city_index].production.cost();
        self.remove_units_with_cargo(vec![unit_id]);
        self.add_event(format!("A Leader hurried production in {city_name}."));
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
        let mut queue = ExplorerQueue::default();
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
                        2 * MOVE_FRAGMENTS
                    } else if unit.kind.stats().ignores_terrain {
                        4
                    } else {
                        6 * MOVE_FRAGMENTS
                    }
                } else {
                    let Some(cost) = self
                        .movement_cost_for(&actor, nx, ny)
                        .or_else(|| self.boarding_target(&actor, nx, ny).map(|_| MOVE_FRAGMENTS))
                    else {
                        continue;
                    };
                    cost
                };
                let candidate = spent + cost.min(remaining);
                if candidate < result.costs[next] && (reverse.is_none() || candidate < 4 * rate) {
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
#[path = "leaders/tests.rs"]
mod tests;
