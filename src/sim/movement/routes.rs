//! Route construction and traversal helpers for the parent subsystem.

use super::super::pathfinding::reconstruct_path;
use super::*;

impl Simulation {
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

    pub(in crate::sim) fn route_cost_within(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
        limit: i32,
    ) -> Option<i32> {
        self.search_route(unit, x, y, false, limit)
            .map(|(_, cost)| cost)
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
        let start = self.map_wrap.index(unit.x, unit.y);
        let goal = self.map_wrap.index(tx, ty);
        if start == goal {
            return Some((Vec::new(), 0));
        }
        let mut queue = BinaryHeap::from([Reverse((0, 0, 0, start))]);
        let mut best = vec![(i32::MAX, usize::MAX); self.tiles.len()];
        let mut previous = vec![None; self.tiles.len()];
        let mut sequence = 0;
        let mut step_unit = unit.clone();
        let allowance = self.unit_move_rate(unit).max(1);
        best[start] = (0, 0);
        while let Some(Reverse((spent, steps, _, current))) = queue.pop() {
            if best[current] != (spent, steps) {
                continue;
            }
            if current == goal {
                break;
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
                let dest = self.tile(nx, ny);
                let source = self.tile(cx, cy);
                let remembered = observed.then(|| self.observed_route_tile(nx, ny)).flatten();
                let cost = if observed {
                    self.observed_move_cost(&step_unit, nx, ny, remembered)
                } else {
                    self.movement_cost_for(&step_unit, nx, ny)
                }
                .or_else(|| {
                    (ni == goal
                        && (!observed || dest.visible)
                        && dest.terrain.is_water()
                        && self.boarding_target(unit, nx, ny).is_some())
                    .then_some(if unit.kind.stats().ignores_terrain {
                        2
                    } else {
                        MOVE_FRAGMENTS
                    })
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
                        !self.allied(unit.owner, owner)
                            && (ni != goal
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
                    enemy_block = self
                        .units
                        .iter()
                        .any(|u| u.x == nx && u.y == ny && self.at_war(u.owner, unit.owner));
                    if enemy_block && ni != goal {
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
                previous[ni] = Some((current, (dx, dy)));
                sequence += 1;
                queue.push(Reverse((candidate.0, candidate.1, sequence, ni)));
            }
        }
        previous[goal]?;
        let path = reconstruct_path(&previous, start, goal)?;
        Some((path, best[goal].0))
    }
}
