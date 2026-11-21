//! Route construction and traversal helpers for the parent subsystem.

use super::super::pathfinding::reconstruct_path;
use super::*;

type RouteSearchResult = (
    Vec<(i32, usize)>,
    Option<Vec<Option<(usize, (i32, i32))>>>,
    u64,
);

impl Simulation {
    #[cfg(test)]
    pub(in crate::sim) fn path_to(&self, unit_id: u32, tx: i32, ty: i32) -> Vec<(i32, i32)> {
        let Some(unit) = self.unit_by_id(unit_id) else {
            return Vec::new();
        };
        self.path_for_unit(unit, tx, ty)
    }

    pub(in crate::sim) fn path_for_unit(&self, unit: &Unit, tx: i32, ty: i32) -> Vec<(i32, i32)> {
        self.search_route(unit, tx, ty, false, i32::MAX)
            .map_or_else(Vec::new, |(path, _)| path)
    }

    pub(in crate::sim) fn plan_route(
        &self,
        unit: &Unit,
        tx: i32,
        ty: i32,
        observed: bool,
    ) -> Vec<(i32, i32)> {
        self.search_civ3_route(unit, tx, ty, observed, i32::MAX)
            .map_or_else(Vec::new, |(path, _)| path)
    }

    pub(in crate::sim) fn search_civ3_route(
        &self,
        unit: &Unit,
        tx: i32,
        ty: i32,
        observed: bool,
        limit: i32,
    ) -> Option<(Vec<(i32, i32)>, i32)> {
        self.search_route_graph(unit, tx, ty, observed, limit)
    }

    #[cfg(test)]
    pub(in crate::sim) fn route_cost_within(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
        limit: i32,
    ) -> Option<i32> {
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("movement.route_cost_within");
        self.trace
            .count_operation("movement.route_cost_within_calls", 1);
        let start = self.map_wrap.index(unit.x, unit.y);
        let goal = self.map_wrap.index(x, y);
        if start == goal {
            return Some(0);
        }
        let _search_profile = profiler.scope("movement.route_search_graph");
        self.trace
            .count_operation("movement.route_search_graph_calls", 1);
        let (best, _, expanded) = self.search_route_graph_map(unit, &[goal], limit, false, false);
        self.trace
            .count_operation("movement.route_search_graph_expansions", expanded);
        (best[goal].0 != i32::MAX).then_some(best[goal].0)
    }

    /// Return route costs to several destinations from one shared search.
    ///
    /// Destination-specific terminal rules apply to every requested tile, as
    /// they do when searching for each destination independently.
    pub(in crate::sim) fn route_costs_to_many(
        &self,
        unit: &Unit,
        destinations: &[(i32, i32)],
    ) -> Vec<Option<i32>> {
        self.route_costs_to_many_within(unit, destinations, i32::MAX)
    }

    pub(in crate::sim) fn route_costs_to_many_within(
        &self,
        unit: &Unit,
        destinations: &[(i32, i32)],
        limit: i32,
    ) -> Vec<Option<i32>> {
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("movement.route_costs_to_many");
        self.trace
            .count_operation("movement.route_costs_to_many_calls", 1);
        if destinations.is_empty() {
            return Vec::new();
        }
        let goals = destinations
            .iter()
            .map(|&(x, y)| self.map_wrap.index(x, y))
            .collect::<Vec<_>>();
        self.trace
            .count_operation("movement.route_search_graph_calls", 1);
        self.trace
            .count_operation("movement.route_search_graph_multi_calls", 1);
        let (best, _, expanded) = self.search_route_graph_map(unit, &goals, limit, false, false);
        self.trace
            .count_operation("movement.route_search_graph_expansions", expanded);
        self.trace
            .count_operation("movement.route_search_graph_multi_expansions", expanded);
        goals
            .into_iter()
            .map(|goal| (best[goal].0 != i32::MAX).then_some(best[goal].0))
            .collect()
    }

    pub(in crate::sim) fn search_route(
        &self,
        unit: &Unit,
        tx: i32,
        ty: i32,
        observed: bool,
        limit: i32,
    ) -> Option<(Vec<(i32, i32)>, i32)> {
        self.search_civ3_route(unit, tx, ty, observed, limit)
    }

    fn search_route_graph(
        &self,
        unit: &Unit,
        tx: i32,
        ty: i32,
        observed: bool,
        limit: i32,
    ) -> Option<(Vec<(i32, i32)>, i32)> {
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("movement.route_search_graph");
        self.trace
            .count_operation("movement.route_search_graph_calls", 1);
        let start = self.map_wrap.index(unit.x, unit.y);
        let goal = self.map_wrap.index(tx, ty);
        if start == goal {
            return Some((Vec::new(), 0));
        }
        let (best, previous, expanded) =
            self.search_route_graph_map(unit, &[goal], limit, observed, true);
        self.trace
            .count_operation("movement.route_search_graph_expansions", expanded);
        let previous = previous.expect("single-destination route search records its path");
        previous[goal]?;
        let path = reconstruct_path(&previous, start, goal)?;
        Some((path, best[goal].0))
    }

    pub(in crate::sim) fn search_route_graph_map(
        &self,
        unit: &Unit,
        goals: &[usize],
        limit: i32,
        observed: bool,
        capture_path: bool,
    ) -> RouteSearchResult {
        let start = self.map_wrap.index(unit.x, unit.y);
        let mut goal_tiles = vec![false; self.tiles.len()];
        for &goal in goals {
            goal_tiles[goal] = true;
        }
        let goal_count = goal_tiles.iter().filter(|&&goal| goal).count();
        let mut goal_tiles_settled = vec![false; self.tiles.len()];
        let mut terminal_only = vec![false; self.tiles.len()];
        let mut settled_goal_count = 0;
        let mut queue = BinaryHeap::from([Reverse((0, 0, 0, start))]);
        let mut best = vec![(i32::MAX, usize::MAX); self.tiles.len()];
        let mut previous = capture_path.then(|| vec![None; self.tiles.len()]);
        let mut sequence = 0;
        let mut step_unit = unit.clone();
        let allowance = self.unit_move_rate(unit).max(1);
        let profile = self.step_movement_profile(unit.kind);
        let (_, war_mask, _) = self.pact_masks(unit.owner);
        best[start] = (0, 0);
        let mut expanded = 0_u64;
        while let Some(Reverse((spent, steps, _, current))) = queue.pop() {
            if best[current] != (spent, steps) {
                continue;
            }
            expanded += 1;
            if goal_tiles[current] && !goal_tiles_settled[current] {
                goal_tiles_settled[current] = true;
                settled_goal_count += 1;
            }
            if settled_goal_count == goal_count {
                break;
            }
            if terminal_only[current] {
                continue;
            }
            let (cx, cy) = self.map_wrap.position(current);
            (step_unit.x, step_unit.y) = (cx, cy);
            let remaining = if spent < unit.moves_left {
                unit.moves_left - spent
            } else {
                allowance - (spent - unit.moves_left.max(0)) % allowance
            };
            for (dx, dy) in CIV3_DIRS {
                let Some((nx, ny)) = self.map_wrap.civ3_offset(cx, cy, dx, dy) else {
                    continue;
                };
                let ni = self.map_wrap.index(nx, ny);
                if best[ni] <= (spent, steps + 1) {
                    continue;
                }
                let dest = self.tile(nx, ny);
                let source = self.tile(cx, cy);
                let remembered = observed.then(|| self.observed_route_tile(nx, ny)).flatten();
                let cost = if observed {
                    self.observed_move_cost_with_profile(&step_unit, nx, ny, remembered, &profile)
                } else {
                    self.movement_cost_with_profile(&step_unit, nx, ny, &profile)
                };
                let terminal_fallback = cost.is_none()
                    && goal_tiles[ni]
                    && (!observed || dest.visible)
                    && dest.terrain.is_water()
                    && self.boarding_target(unit, nx, ny).is_some();
                let cost = cost.or_else(|| {
                    terminal_fallback
                        .then(|| self.raw_move_cost_with_profile(unit, nx, ny, &profile))
                });
                let Some(cost) = cost else {
                    continue;
                };
                let candidate = (spent + cost.min(remaining), steps + 1);
                if candidate.0 > limit || candidate >= best[ni] {
                    continue;
                }
                let enemy_block;
                if observed {
                    let rivals = self
                        .units
                        .iter()
                        .filter(|other| {
                            (other.x, other.y) == (nx, ny) && self.unit_visible_to_human(other)
                        })
                        .map(|other| other.owner)
                        .chain(remembered.and_then(|tile| tile.city_owner));
                    let blocked = rivals.clone().any(|owner| {
                        !self.military_access(unit.owner, owner)
                            && (!goal_tiles[ni]
                                || !unit.kind.is_military()
                                || !self.at_war(unit.owner, owner))
                    }) || !unit.kind.is_civilian()
                        && self
                            .displayed_tile_owner(nx, ny)
                            .is_some_and(|owner| self.pact(unit.owner, owner) == Pact::Peace);
                    if blocked {
                        continue;
                    }
                    enemy_block = rivals
                        .into_iter()
                        .any(|owner| self.at_war(unit.owner, owner));
                } else {
                    if self.treaty_blocks_move(unit, nx, ny) {
                        continue;
                    }
                    enemy_block = self.any_unit_at(nx, ny, |other| {
                        other.owner < 64 && ((war_mask >> other.owner) & 1) != 0
                    });
                    if enemy_block && !goal_tiles[ni] {
                        continue;
                    }
                }
                if !enemy_block
                    && (!observed || (remembered.is_some() && source.seen))
                    && self.zone_blocks_route(unit, (cx, cy), (nx, ny), observed)
                {
                    continue;
                }
                best[ni] = candidate;
                terminal_only[ni] = terminal_fallback || enemy_block;
                if let Some(previous) = &mut previous {
                    previous[ni] = Some((current, (dx, dy)));
                }
                sequence += 1;
                queue.push(Reverse((candidate.0, candidate.1, sequence, ni)));
            }
        }
        (best, previous, expanded)
    }
}
