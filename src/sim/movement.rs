//! Unit movement resolution and its activity, combat, route, and transport rules.

use super::*;

mod activities;
mod combat;
mod routes;
mod transports;
use std::{cmp::Reverse, collections::BinaryHeap};

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(super) struct RouteTile {
    pub(super) terrain: Terrain,
    pub(super) road: bool,
    pub(super) railroad: bool,
    pub(super) river: bool,
    pub(super) city_owner: Option<usize>,
}

impl RouteTile {
    pub(super) fn is_water(self) -> bool {
        self.terrain.is_water()
    }
}

impl From<&Tile> for RouteTile {
    fn from(tile: &Tile) -> Self {
        Self {
            terrain: tile.terrain,
            road: tile.road,
            railroad: tile.railroad,
            river: tile.river,
            city_owner: None,
        }
    }
}

impl Unit {
    fn work_key(&self) -> Option<(i32, i32, WorkKind)> {
        self.work_kind.map(|kind| (self.x, self.y, kind))
    }

    pub(super) fn clear_work(&mut self) {
        self.work_kind = None;
        self.work_left = 0;
        self.work_done = 0;
    }
}

impl Simulation {
    fn naval_movement_bonus(&self, owner: usize) -> i32 {
        MOVE_FRAGMENTS
            * (i32::from(self.has_tech(owner, Tech::NUCLEAR_POWER))
                + i32::from(self.has_active_wonder(owner, Building::LIGHTHOUSE))
                + i32::from(self.has_active_wonder(owner, Building::MAGELLANS_EXPEDITION)))
    }

    pub(crate) fn unit_move_allowance(&self, owner: usize, kind: UnitKind) -> i32 {
        let base = kind.stats().moves * MOVE_FRAGMENTS;
        if kind == UnitKind::Nuclear {
            8 * MOVE_FRAGMENTS
                * (i32::from(self.has_tech(owner, Tech::ADVANCED_FLIGHT))
                    + i32::from(self.has_tech(owner, Tech::ROCKETRY)))
        } else if kind.is_naval() && kind != UnitKind::Trireme {
            base + self.naval_movement_bonus(owner)
        } else {
            base
        }
    }

    fn move_rate_with_bonus(unit: &Unit, bonus: i32) -> i32 {
        // movement.c:utype_move_rate. All bundled veteran move bonuses are
        // zero. DamageSlows is a class flag, absent on Merchant and Air.
        let base = unit.kind.stats().moves * MOVE_FRAGMENTS;
        let damage_slows = unit.kind.unit_class().damage_slows();
        let movement = if damage_slows {
            base * unit.hp / unit.kind.stats().hp
        } else {
            base
        };
        let minimum = if unit.kind.is_missile() {
            0
        } else if unit.kind.is_naval() {
            2 * MOVE_FRAGMENTS
        } else {
            MOVE_FRAGMENTS
        };
        (movement + bonus).max(minimum.min(base))
    }

    pub(super) fn unit_move_rate(&self, unit: &Unit) -> i32 {
        let base = unit.kind.stats().moves * MOVE_FRAGMENTS;
        let mut bonus = if unit.kind == UnitKind::Trireme {
            let center = self
                .cities
                .iter()
                .find(|city| (city.x, city.y) == (unit.x, unit.y));
            MOVE_FRAGMENTS
                * i32::from(
                    self.has_active_wonder(unit.owner, Building::LIGHTHOUSE)
                        && ((center.is_none() && !self.tile(unit.x, unit.y).river)
                            || center.is_some_and(|city| self.is_port(city))),
                )
        } else {
            self.unit_move_allowance(unit.owner, unit.kind) - base
        };
        let tile = self.tile(unit.x, unit.y);
        if !unit.kind.is_naval()
            && !unit.kind.is_aircraft()
            && tile.terrain == Terrain::Mountains
            && !tile.road
            && (!tile.river || unit.kind.is_big_land())
        {
            bonus -= MOVE_FRAGMENTS;
        }
        Self::move_rate_with_bonus(unit, bonus)
    }

    pub(super) fn live_route_tile(&self, x: i32, y: i32) -> RouteTile {
        let mut tile = RouteTile::from(self.tile(x, y));
        tile.city_owner = self
            .cities
            .iter()
            .find(|city| (city.x, city.y) == (x, y))
            .map(|city| city.owner);
        tile
    }

    pub(super) fn remember_route_tiles(&mut self, legacy_save: bool) {
        self.route_memory.resize(self.tiles.len(), None);
        for (index, tile) in self.tiles.iter().enumerate() {
            if tile.visible || (legacy_save && tile.seen) {
                let (x, y) = self.map_wrap.position(index);
                self.route_memory[index] = Some(self.live_route_tile(x, y));
            }
        }
    }

    pub(super) fn observed_route_tile(&self, x: i32, y: i32) -> Option<RouteTile> {
        let tile = self.tile(x, y);
        if tile.visible {
            Some(self.live_route_tile(x, y))
        } else if tile.seen {
            self.route_memory[self.map_wrap.index(x, y)]
        } else {
            None
        }
    }

    pub(super) fn observed_move_cost(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
        dest: Option<RouteTile>,
    ) -> Option<i32> {
        let kind = unit.kind;
        if kind.is_aircraft() {
            return Some(MOVE_FRAGMENTS);
        }
        let Some(dest) = dest else {
            return Some(if kind.is_naval() {
                MOVE_FRAGMENTS
            } else if kind.stats().ignores_terrain {
                2
            } else {
                Terrain::Mountains.movement_cost() * MOVE_FRAGMENTS
            });
        };
        let source = self.observed_route_tile(unit.x, unit.y);
        if kind.is_naval() {
            let native = |tile: RouteTile| {
                if kind == UnitKind::Trireme {
                    matches!(tile.terrain, Terrain::Ocean | Terrain::Lake) || tile.river
                } else {
                    tile.terrain.is_water()
                }
            };
            let port = dest
                .city_owner
                .is_some_and(|owner| self.allied(owner, unit.owner))
                && self
                    .map_wrap
                    .neighbors(x, y)
                    .any(|(nx, ny)| self.observed_route_tile(nx, ny).is_none_or(native));
            return (native(dest) || port).then_some(MOVE_FRAGMENTS);
        }
        // common/map.c::tile_move_cost_ptrs and the same class-native rules
        // used by raw_move_cost_for, evaluated on the player's observations.
        // RouteTile keeps its historical save layout; base/owner come from
        // the separately persisted observed map rather than live fogged data.
        let native = |tile: RouteTile, x, y| {
            kind.is_native_on(
                tile.terrain,
                tile.river,
                tile.road,
                tile.railroad,
                self.render_tile(x, y).base,
            )
        };
        let native_dest = native(dest, x, y);
        let city_access = dest.city_owner.is_some_and(|owner| {
            self.allied(owner, unit.owner)
                || self.at_war(owner, unit.owner) && kind.can_occupy_city()
        });
        if !native_dest && !city_access {
            return None;
        }
        if !kind.unit_class().terrain_speed() {
            return Some(MOVE_FRAGMENTS);
        }
        let ignores = kind.stats().ignores_terrain;
        if !native_dest || source.is_some_and(|tile| !native(tile, unit.x, unit.y)) {
            return Some(if ignores { 2 } else { MOVE_FRAGMENTS });
        }
        let restricted = [(unit.x, unit.y), (x, y)].into_iter().any(|(x, y)| {
            self.observed_route_tile(x, y).is_some()
                && self
                    .render_tile(x, y)
                    .owner
                    .is_some_and(|owner| self.at_war(unit.owner, owner))
        });
        let mut cost = dest.terrain.movement_cost() * MOVE_FRAGMENTS;
        if !restricted {
            if dest.railroad && source.is_some_and(|tile| tile.railroad) {
                cost = cost.min(1);
            }
            if dest.road && source.is_some_and(|tile| tile.road)
                || dest.river && !kind.is_big_land() && source.is_some_and(|tile| tile.river)
            {
                cost = cost.min(2);
            }
        }
        Some(if ignores { cost.min(2) } else { cost })
    }

    pub(super) fn movement_cost_for(&self, unit: &Unit, nx: i32, ny: i32) -> Option<i32> {
        if unit.kind.is_aircraft() {
            return Some(MOVE_FRAGMENTS);
        }
        let dest = self.tile(nx, ny);
        if unit.kind.is_naval() {
            let trireme = unit.kind == UnitKind::Trireme;
            let native = if trireme {
                matches!(dest.terrain, Terrain::Ocean | Terrain::Lake) || dest.river
            } else {
                dest.terrain.is_water()
            };
            return (native
                || self.cities.iter().any(|city| {
                    (city.x, city.y) == (nx, ny)
                        && self.allied(city.owner, unit.owner)
                        && self.naval_city_access(city, unit.kind)
                }))
            .then_some(MOVE_FRAGMENTS);
        }
        let city_access = self.cities.iter().any(|city| {
            (city.x, city.y) == (nx, ny)
                && (self.allied(city.owner, unit.owner)
                    || self.at_war(city.owner, unit.owner) && unit.kind.can_occupy_city())
        });
        (self.pf_native_tile(unit.kind, nx, ny) || city_access)
            .then(|| self.raw_move_cost_for(unit, nx, ny))
    }

    pub(super) fn under_foreign_control(
        &self,
        owner: usize,
        x: i32,
        y: i32,
        observed: bool,
    ) -> bool {
        self.units.iter().any(|unit| {
            if (observed && !self.unit_visible_to_human(unit))
                || self.tile(unit.x, unit.y).terrain.is_water()
                || self.map_distance(x, y, unit.x, unit.y) > 1
            {
                return false;
            }
            let occupied_foreign_city = self.cities.iter().any(|city| {
                (city.x, city.y) == (unit.x, unit.y) && !self.allied(owner, city.owner)
            });
            occupied_foreign_city
                || (!self.allied(owner, unit.owner)
                    && unit.transport.is_none()
                    && !unit.kind.has_no_zoc()
                    && unit.kind.unit_class().has_zoc())
        })
    }

    pub(super) fn zone_blocks_step(&self, unit: &Unit, from: (i32, i32), to: (i32, i32)) -> bool {
        self.zone_blocks_route(unit, from, to, false)
    }

    fn zone_blocks_route(
        &self,
        unit: &Unit,
        from: (i32, i32),
        to: (i32, i32),
        observed: bool,
    ) -> bool {
        if unit.kind.ignores_zoc()
            || [from, to].into_iter().any(|(x, y)| {
                let tile = if observed {
                    self.observed_route_tile(x, y)
                } else {
                    Some(self.live_route_tile(x, y))
                };
                tile.is_none_or(|tile| tile.terrain.is_water() || tile.city_owner.is_some())
            })
            || self
                .units
                .iter()
                .any(|other| (other.x, other.y) == to && self.allied(unit.owner, other.owner))
        {
            return false;
        }
        self.under_foreign_control(unit.owner, from.0, from.1, observed)
            && self.under_foreign_control(unit.owner, to.0, to.1, observed)
    }

    fn try_capture_units(&mut self, actor: &Unit, x: i32, y: i32) -> bool {
        if !actor.kind.can_capture_units()
            || self.tile(x, y).terrain.is_water()
            || self.cities.iter().any(|city| (city.x, city.y) == (x, y))
        {
            return false;
        }
        let targets: Vec<_> = self
            .units
            .iter()
            .filter(|unit| (unit.x, unit.y) == (x, y))
            .cloned()
            .collect();
        if targets.is_empty()
            || targets.iter().any(|unit| {
                !self.at_war(actor.owner, unit.owner)
                    || !unit.kind.is_capturable()
                    || unit.transport.is_some()
                    || self.cargo_count(unit.id) > 0
            })
        {
            return false;
        }
        let home = actor.home_city.filter(|&id| {
            self.cities
                .iter()
                .any(|city| city.id == id && city.owner == actor.owner)
        });
        for target in &targets {
            self.transfer_unit(target, actor.owner, home);
        }
        let capturer = self
            .units
            .iter_mut()
            .find(|unit| unit.id == actor.id)
            .unwrap();
        capturer.moves_left = (capturer.moves_left - MOVE_FRAGMENTS).max(0);
        capturer.state = UnitState::Idle;
        capturer.moved = true;
        if actor.owner == HUMAN || targets.iter().any(|unit| unit.owner == HUMAN) {
            self.add_event(format!(
                "{} captured {} units.",
                actor.kind.name(),
                targets.len()
            ));
        }
        self.refresh_visibility();
        self.check_game_over();
        true
    }

    pub(super) fn try_move_unit(&mut self, unit_id: u32, dx: i32, dy: i32) -> MoveResult {
        let Some(index) = self.unit_index_by_id(unit_id) else {
            return MoveResult::Blocked;
        };
        if !is_isohex_step(dx, dy) {
            return MoveResult::Blocked;
        }
        let unit = self.units[index].clone();
        if unit.moves_left <= 0 || unit.work_left > 0 || unit.state == UnitState::Sentry {
            return MoveResult::Blocked;
        }
        let Some((nx, ny)) = self.map_wrap.offset(unit.x, unit.y, dx, dy) else {
            return MoveResult::Blocked;
        };
        if self.treaty_blocks_move(&unit, nx, ny) {
            if unit.owner == HUMAN {
                self.add_event("A treaty prevents entry to that tile.");
            }
            return MoveResult::Blocked;
        }
        if !self.can_leave_transport(&unit) {
            return MoveResult::Blocked;
        }
        if self.try_capture_units(&unit, nx, ny) {
            return MoveResult::Fought;
        }
        if unit.kind == UnitKind::Nuclear && self.nuclear_target_is_hostile(&unit, nx, ny) {
            self.nuclear_strike(&unit, nx, ny);
            return MoveResult::Fought;
        }
        let cost = match self.movement_cost_for(&unit, nx, ny) {
            Some(cost) => cost,
            None if self.tile(nx, ny).terrain.is_water() && !unit.kind.is_naval() => {
                let Some(ship) = self.boarding_target(&unit, nx, ny) else {
                    return MoveResult::Blocked;
                };
                return if self.embark_adjacent_ship(unit_id, ship) {
                    MoveResult::Moved
                } else {
                    MoveResult::Blocked
                };
            }
            // Sea-class warships may attack a non-native tile without entering it.
            None if unit.kind.is_naval()
                && unit.kind != UnitKind::Submarine
                && unit.kind.is_military()
                && self.best_defender_id(&unit, nx, ny).is_some() =>
            {
                MOVE_FRAGMENTS
            }
            None => return MoveResult::Blocked,
        };
        if unit.kind.bombard_rate() > 0
            && !self.tile(nx, ny).terrain.is_water()
            && (self.units.iter().any(|other| {
                (other.x, other.y) == (nx, ny) && self.at_war(unit.owner, other.owner)
            }) || self.cities.iter().any(|city| {
                !unit.kind.can_occupy_city()
                    && (city.x, city.y) == (nx, ny)
                    && self.at_war(unit.owner, city.owner)
            }))
        {
            if unit.transport.is_some() {
                return MoveResult::Blocked;
            }
            return self.try_bombard(&unit, nx, ny);
        }
        let defender = self.best_defender_id(&unit, nx, ny);

        if let Some(defender_id) = defender {
            if unit.kind.stats().attack <= 0
                || unit.kind == UnitKind::Nuclear
                || (unit.kind == UnitKind::Submarine && !self.tile(nx, ny).terrain.is_water())
                || (unit.transport.is_some()
                    && !(unit.kind.is_missile() || unit.kind.has_marine_attack()))
            {
                return MoveResult::Blocked;
            }
            self.resolve_combat(unit_id, defender_id, nx, ny);
            return MoveResult::Fought;
        }

        if self
            .units
            .iter()
            .any(|other| (other.x, other.y) == (nx, ny) && self.at_war(unit.owner, other.owner))
        {
            return MoveResult::Blocked;
        }
        if let Some(city) = self.city_at(nx, ny)
            && self.at_war(city.owner, unit.owner)
            && (!unit.kind.can_occupy_city()
                || (unit.transport.is_some()
                    && self.tile(unit.x, unit.y).terrain.is_water()
                    && !unit.kind.has_marine_attack()))
        {
            return MoveResult::Blocked;
        }

        if self.zone_blocks_step(&unit, (unit.x, unit.y), (nx, ny)) {
            if unit.owner == HUMAN {
                self.add_event("A foreign zone of control blocks that move.");
            }
            return MoveResult::Blocked;
        }

        if !self.ai_guard_allows_step(&unit, (nx, ny)) {
            return MoveResult::Blocked;
        }
        if self.players[unit.owner].ai
            && Self::is_ai_ferry(unit.kind)
            && let Some(boss) = self.ferry_boss(unit_id)
            && self
                .units
                .iter()
                .any(|passenger| passenger.id == boss && passenger.transport == Some(unit_id))
            && !self.ai_guard_prepare_ferry(boss, unit_id)
        {
            return MoveResult::Blocked;
        }

        let carrier = unit
            .kind
            .is_aircraft()
            .then(|| self.boarding_target(&unit, nx, ny))
            .flatten();
        self.enter_tile(unit_id, nx, ny, cost);
        if let Some(ship) = carrier
            && let Some(mover) = self.units.iter_mut().find(|unit| unit.id == unit_id)
        {
            mover.transport = Some(ship);
        }
        if carrier.is_some() {
            self.record_transport_change(unit_id, carrier);
        }
        if unit.owner == HUMAN {
            self.refresh_visibility();
        }
        MoveResult::Moved
    }

    pub(super) fn enter_tile(&mut self, unit_id: u32, nx: i32, ny: i32, cost: i32) {
        let old_position = self
            .units
            .iter()
            .find(|unit| unit.id == unit_id)
            .map(|unit| (unit.x, unit.y));
        if self
            .units
            .iter()
            .any(|unit| unit.id == unit_id && unit.transport.is_some())
        {
            self.release_ai_ferry(unit_id);
        }
        let visual_mover = self
            .visual_events
            .enabled
            .then(|| self.unit_by_id(unit_id).cloned())
            .flatten();
        if let Some(mover) = self.units.iter_mut().find(|u| u.id == unit_id) {
            mover.x = nx;
            mover.y = ny;
            mover.transport = None;
            mover.moves_left = (mover.moves_left - cost).max(0);
            mover.moved = true;
            mover.state = UnitState::Idle;
            self.list_order.unit(mover);
        }
        self.record_transport_change(unit_id, None);
        let owner = self
            .units
            .iter()
            .find(|unit| unit.id == unit_id)
            .map(|unit| unit.owner);
        self.move_cargo(unit_id, nx, ny);
        // Native movement updates the moving player's vision immediately,
        // including AI discovery, before contact callbacks and hut outcomes.
        self.refresh_visibility();
        if let Some(unit) = visual_mover {
            let from = (unit.x, unit.y);
            self.publish_visual_event(VisualEvent::Move {
                unit,
                from,
                to: (nx, ny),
            });
        }
        self.capture_city_if_needed(unit_id, nx, ny);
        if let Some(owner) = owner {
            self.refresh_diplomatic_contact_at(owner, nx, ny);
        }
        self.enter_village(unit_id, nx, ny);
        if let Some(old_position) = old_position {
            self.ai_guard_follow_step(unit_id, old_position);
        }
    }

    pub(super) fn player_goto(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.unit_by_id(id) else {
            return false;
        };
        let path = self.plan_route(unit, x, y, true);
        self.follow_path(id, path)
    }

    pub(super) fn follow_path(
        &mut self,
        unit_id: u32,
        path: impl IntoIterator<Item = (i32, i32)>,
    ) -> bool {
        let mut moved = false;
        for (dx, dy) in path {
            let result = self.try_move_unit(unit_id, dx, dy);
            if result == MoveResult::Blocked {
                break;
            }
            moved = true;
            let Some(unit) = self.unit_by_id(unit_id) else {
                break;
            };
            if unit.moves_left <= 0 || result == MoveResult::Fought {
                break;
            }
        }
        moved
    }
}

#[cfg(test)]
#[path = "movement/tests.rs"]
mod tests;
