use super::*;
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

    pub(super) fn is_port(&self, city: &City) -> bool {
        super::terrain_work::neighbors(city.x, city.y)
            .any(|(x, y)| self.tile(x, y).terrain.is_water())
    }

    pub(super) fn naval_city_access(&self, city: &City, kind: UnitKind) -> bool {
        if kind != UnitKind::Trireme {
            return self.is_port(city);
        }
        let native =
            |tile: &Tile| matches!(tile.terrain, Terrain::Ocean | Terrain::Lake) || tile.river;
        native(self.tile(city.x, city.y))
            || super::terrain_work::neighbors(city.x, city.y).any(|(x, y)| native(self.tile(x, y)))
    }

    pub(crate) fn cargo_count(&self, carrier: u32) -> usize {
        self.units
            .iter()
            .filter(|unit| unit.transport == Some(carrier))
            .count()
    }

    pub(super) fn boarding_target(&self, passenger: &Unit, x: i32, y: i32) -> Option<u32> {
        self.units
            .iter()
            .find(|ship| (ship.x, ship.y) == (x, y) && self.can_board_ship(passenger, ship))
            .map(|ship| ship.id)
    }

    fn can_board_ship(&self, passenger: &Unit, ship: &Unit) -> bool {
        passenger.transport != Some(ship.id)
            && self.allied(passenger.owner, ship.owner)
            && ship.kind.can_carry(passenger.kind)
            && (ship.kind != UnitKind::Helicopter
                || self.helicopter_cargo_access(passenger, ship.x, ship.y))
            && self.cargo_count(ship.id) < ship.kind.transport_capacity()
    }

    pub(super) fn board_specific_ship(&mut self, unit_id: u32, ship_id: u32) -> bool {
        let Some(index) = self.units.iter().position(|unit| unit.id == unit_id) else {
            return false;
        };
        let unit = &self.units[index];
        if unit.transport.is_some()
            || !self.units.iter().any(|ship| {
                ship.id == ship_id
                    && (ship.x, ship.y) == (unit.x, unit.y)
                    && self.can_board_ship(unit, ship)
            })
        {
            return false;
        }
        self.set_transport(index, Some(ship_id));
        true
    }

    pub(super) fn embark_adjacent_ship(&mut self, unit_id: u32, ship_id: u32) -> bool {
        let Some(index) = self.units.iter().position(|unit| unit.id == unit_id) else {
            return false;
        };
        let passenger = &self.units[index];
        let Some(ship) = self.units.iter().find(|unit| unit.id == ship_id) else {
            return false;
        };
        let (x, y) = (ship.x, ship.y);
        if passenger.moves_left <= 0
            || passenger.work_left > 0
            || passenger.state == UnitState::Sentry
            || passenger.transport.is_some()
            || !super::terrain_work::neighbors(passenger.x, passenger.y).any(|tile| tile == (x, y))
            || !self.tile(x, y).terrain.is_water()
            || self.treaty_blocks_move(passenger, x, y)
            || !self.can_board_ship(passenger, ship)
            || self.best_defender_id(passenger, x, y).is_some()
            || !self.ai_guard_allows_step(passenger, (x, y))
        {
            return false;
        }
        let old_position = (passenger.x, passenger.y);
        let cost = self.raw_move_cost_for(passenger, x, y);
        let passenger = &mut self.units[index];
        (passenger.x, passenger.y) = (x, y);
        passenger.transport = Some(ship_id);
        passenger.moves_left = (passenger.moves_left - cost).max(0);
        passenger.moved = true;
        passenger.state = UnitState::Idle;
        self.list_order.unit(passenger);
        self.record_transport_change(unit_id, Some(ship_id));
        self.assign_ai_ferry(ship_id, unit_id);
        self.ai_guard_follow_step(unit_id, old_position);
        self.refresh_visibility();
        true
    }

    fn helicopter_cargo_access(&self, passenger: &Unit, x: i32, y: i32) -> bool {
        Self::can_disembark_helicopter_in_field(passenger)
            || self.refuel_site(passenger.owner, x, y)
    }

    fn can_disembark_helicopter_in_field(passenger: &Unit) -> bool {
        passenger.kind.can_helicopter_field_load()
    }

    fn can_leave_transport(&self, passenger: &Unit) -> bool {
        passenger.transport.is_none_or(|id| {
            self.units
                .iter()
                .find(|unit| unit.id == id)
                .is_some_and(|carrier| {
                    carrier.kind != UnitKind::Helicopter
                        || self.helicopter_cargo_access(passenger, passenger.x, passenger.y)
                })
        })
    }

    pub(crate) fn transport_action(&self, unit: &Unit) -> Option<&'static str> {
        if unit.transport.is_some() {
            return ((unit.kind.is_aircraft() || !self.tile(unit.x, unit.y).terrain.is_water())
                && self.can_leave_transport(unit))
            .then_some("Deboard");
        }
        self.boarding_target(unit, unit.x, unit.y).map(|_| "Board")
    }

    pub(super) fn board_or_deboard(&mut self, unit_id: u32) -> bool {
        let Some(index) = self.units.iter().position(|unit| unit.id == unit_id) else {
            return false;
        };
        let unit = &self.units[index];
        if self.transport_action(unit).is_none() {
            return false;
        }
        let carrier = if unit.transport.is_some() {
            None
        } else {
            self.boarding_target(unit, unit.x, unit.y)
        };
        self.set_transport(index, carrier);
        true
    }

    fn set_transport(&mut self, index: usize, carrier: Option<u32>) {
        self.record_transport_change(self.units[index].id, carrier);
        self.units[index].transport = carrier;
        self.units[index].state = UnitState::Idle;
        self.units[index].clear_work();
        let unit = self.units[index].clone();
        if let Some(ship) = carrier {
            self.assign_ai_ferry(ship, unit.id);
        } else {
            self.release_ai_ferry(unit.id);
        }
        self.reconcile_city_workers();
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
                let (x, y) = Self::xy(index);
                self.route_memory[index] = Some(self.live_route_tile(x, y));
            }
        }
    }

    pub(super) fn observed_route_tile(&self, x: i32, y: i32) -> Option<RouteTile> {
        let tile = self.tile(x, y);
        if tile.visible {
            Some(self.live_route_tile(x, y))
        } else if tile.seen {
            self.route_memory[Self::idx(x, y)]
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
                && super::terrain_work::neighbors(x, y)
                    .any(|(nx, ny)| self.observed_route_tile(nx, ny).is_none_or(native));
            return (native(dest) || port).then_some(MOVE_FRAGMENTS);
        }
        // common/map.c::tile_move_cost_ptrs and the same class-native rules
        // used by raw_move_cost_for, evaluated on the player's observations.
        // RouteTile keeps its historical save layout; base/owner come from
        // the separately persisted observed map rather than live fogged data.
        let native = |tile: RouteTile, x, y| {
            !tile.terrain.is_water()
                && (!kind.is_big_land()
                    || !matches!(
                        tile.terrain,
                        Terrain::Jungle | Terrain::Mountains | Terrain::Swamp
                    )
                    || tile.road
                    || tile.railroad
                    || matches!(
                        self.render_tile(x, y).base,
                        Some(Base::Fort | Base::Fortress)
                    ))
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

    pub(crate) fn attack_power(unit: &Unit) -> i32 {
        let stats = unit.kind.stats();
        let mut power = percent_of(stats.attack * POWER_FACTOR, unit.veteran_power());
        if unit.moves_left < MOVE_FRAGMENTS {
            power = power * unit.moves_left.max(0) / MOVE_FRAGMENTS;
        }
        power
    }

    #[cfg(test)]
    pub(crate) fn defense_power(&self, unit: &Unit, x: i32, y: i32) -> i32 {
        self.defense_power_against(unit, x, y, unit.kind)
    }

    pub(super) fn defense_power_against(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
        attacker: UnitKind,
    ) -> i32 {
        self.defense_power_with_building(unit, x, y, attacker, None)
    }

    pub(super) fn defense_power_with_building(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
        attacker: UnitKind,
        building: Option<Building>,
    ) -> i32 {
        // Freeciv applies these as separate integer-multiplication stages:
        // terrain in get_defense_power, then Defend_Bonus, then
        // Fortify_Defense_Bonus in defense_multiplication.
        let mut power = percent_of(
            unit.kind.stats().defense * POWER_FACTOR,
            unit.veteran_power(),
        );
        if unit.kind.has_terrain_defense() {
            power = apply_percent_bonus(power, self.tile(x, y).terrain.defense_bonus());
        }

        let city = self.cities.iter().find(|c| c.x == x && c.y == y);
        let base = self.tile(x, y).base;
        let aerial_attack = attacker.is_aircraft();
        let naval_attack = attacker.is_naval();
        if unit.kind == UnitKind::AegisCruiser && aerial_attack && attacker != UnitKind::Nuclear {
            power *= 5;
        }
        if unit.kind == UnitKind::Destroyer && attacker == UnitKind::Submarine {
            power *= 2;
        }
        let ground_attack = !aerial_attack && !naval_attack;
        let mut defend_bonus = if aerial_attack {
            match base {
                Some(Base::Airstrip) => 50,
                Some(Base::Airbase) => 100,
                Some(Base::Fortress) => 50,
                _ => 0,
            }
        } else {
            base.map_or(0, Base::defense_bonus)
        };
        if let Some(city) = city {
            let has = |candidate| building == Some(candidate) || city.has_building(candidate);
            if ground_attack {
                defend_bonus += CITY_DEFEND_BONUS;
                if has(Building::CITY_WALLS) {
                    defend_bonus += 100;
                }
                if self.has_active_wonder(unit.owner, Building::GREAT_WALL) {
                    defend_bonus += 50;
                }
            }
            if !aerial_attack && city.size >= LARGE_CITY_SIZE {
                defend_bonus += LARGE_CITY_DEFEND_BONUS;
            }
            if naval_attack && has(Building::COASTAL_DEFENSE) {
                defend_bonus += 100;
            }
            if attacker.is_missile() {
                if has(Building::SDI_DEFENSE) {
                    defend_bonus += 100;
                }
            } else if aerial_attack && has(Building::SAM_BATTERY) {
                defend_bonus += 100;
            }
        }
        power = apply_percent_bonus(power, defend_bonus);
        // combat.c:defense_multiplication applies the extra attribute after
        // defense effects and before fortification. River is native only
        // to its configured classes; it is separate from terrain defense.
        power += power * self.pf_extra_class_defense(unit.kind, x, y) / 100;

        // Civ2Civ3 gives CanFortify units without the Cant_Fortify type flag
        // the same 50% city stage even while they are not explicitly fortified.
        let fortify_bonus =
            if unit.state == UnitState::Fortified || (city.is_some() && unit.kind.can_fortify()) {
                FORTIFY_DEFENSE_BONUS
            } else {
                0
            };
        apply_percent_bonus(power, fortify_bonus)
    }

    pub(super) fn combat_firepower(&self, attacker: &Unit, defender: &Unit) -> (i32, i32) {
        let mut attack = attacker.kind.stats().firepower;
        let defense = defender.kind.stats().firepower;
        let in_city = self
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (defender.x, defender.y));
        if in_city && attacker.kind.is_city_buster() {
            attack *= 2;
        }
        if in_city && defender.kind.is_naval() {
            (attack * 2, 1)
        } else {
            (attack, defense)
        }
    }

    pub(super) fn defender_rank(
        &self,
        attacker: &Unit,
        defender: &Unit,
        x: i32,
        y: i32,
    ) -> (i32, i32, i32) {
        let attack_power = Self::attack_power(attacker);
        let defense_power = self.defense_power_against(defender, x, y, attacker.kind);
        let defender_stats = defender.kind.stats();
        let (attack_firepower, defense_firepower) = self.combat_firepower(attacker, defender);
        let attacker_win = combat_win_chance(
            attack_power,
            attacker.hp,
            attack_firepower,
            defense_power,
            defender.hp,
            defense_firepower,
        );

        // get_defender truncates the defender's win chance to five decimal
        // places, then prefers the cheaper unit and finally the higher
        // defense rating when those values tie.
        let defender_value = (100_000.0 * (1.0 - attacker_win)) as i32;
        let rounds_survived = (defender.hp + attack_firepower - 1) / attack_firepower;
        let defense_rating = defense_power * rounds_survived * defense_firepower;
        (defender_value, -defender_stats.cost, defense_rating)
    }

    pub(super) fn best_defender_id(&self, attacker: &Unit, x: i32, y: i32) -> Option<u32> {
        let mut best = None;
        let mut best_rank = None;
        for defender in self.tile_units_ordered(x, y).filter(|unit| {
            self.at_war(unit.owner, attacker.owner)
                && unit.transport.is_none()
                && (!unit.kind.is_aircraft()
                    || attacker.kind.can_attack_aircraft()
                    || (!attacker.kind.is_aircraft() && self.refuel_site(unit.owner, x, y)))
        }) {
            let rank = self.defender_rank(attacker, defender, x, y);
            if best_rank.is_none() || best_rank.is_some_and(|current| rank > current) {
                best = Some(defender.id);
                best_rank = Some(rank);
            }
        }
        best
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
        let Some(index) = self.units.iter().position(|u| u.id == unit_id) else {
            return MoveResult::Blocked;
        };
        if !is_isohex_step(dx, dy) {
            return MoveResult::Blocked;
        }
        let unit = self.units[index].clone();
        if unit.moves_left <= 0 || unit.work_left > 0 || unit.state == UnitState::Sentry {
            return MoveResult::Blocked;
        }
        let (nx, ny) = offset_native_by_map(unit.x, unit.y, dx, dy);
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
        if let Some(city) = self.cities.iter().find(|c| c.x == nx && c.y == ny) {
            if self.at_war(city.owner, unit.owner)
                && (!unit.kind.can_occupy_city()
                    || (unit.transport.is_some()
                        && self.tile(unit.x, unit.y).terrain.is_water()
                        && !unit.kind.has_marine_attack()))
            {
                return MoveResult::Blocked;
            }
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
            .then(|| self.units.iter().find(|u| u.id == unit_id).cloned())
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

    pub(super) fn move_cargo(&mut self, transport: u32, x: i32, y: i32) {
        let mut transports = vec![transport];
        while let Some(transport) = transports.pop() {
            for passenger in self
                .units
                .iter_mut()
                .filter(|unit| unit.transport == Some(transport))
            {
                passenger.x = x;
                passenger.y = y;
                self.list_order.unit(passenger);
                passenger.clear_work();
                transports.push(passenger.id);
            }
        }
    }

    pub(crate) fn can_disband(&self, unit: &Unit, city: Option<&City>) -> bool {
        unit.kind != UnitKind::BarbarianLeader
            && city.is_none_or(|city| {
                self.allied(unit.owner, city.owner)
                    && (unit.y - city.y).abs() <= 2
                    && self.map_distance(unit.x, unit.y, city.x, city.y) <= 1
            })
    }

    pub(super) fn disband_unit(&mut self, id: u32, city_id: Option<u32>) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == id).cloned() else {
            return false;
        };
        let city = city_id.and_then(|id| self.cities.iter().position(|city| city.id == id));
        if city_id.is_some() != city.is_some()
            || !self.can_disband(&unit, city.map(|index| &self.cities[index]))
        {
            return false;
        }
        if let Some(index) = city {
            let city = &mut self.cities[index];
            let cost = unit.kind.stats().cost;
            let shields = cost - cost / 2;
            city.shields += shields;
            city.protected_shields += shields;
            if let Some((_, original_shields)) = &mut city.production_before_switch {
                *original_shields += shields;
            }
            let name = city.name.clone();
            self.add_event(format!(
                "Disbanded {} for {shields} shields in {name}.",
                unit.kind.name()
            ));
        } else {
            self.add_event(format!("Disbanded {}.", unit.kind.name()));
        }

        self.remove_units_with_cargo(vec![id]);
        self.refresh_visibility();
        self.check_game_over();
        true
    }

    pub(super) fn remove_units_with_cargo(&mut self, mut losses: Vec<u32>) {
        let mut index = 0;
        while let Some(&lost) = losses.get(index) {
            let mut cargo: Vec<_> = self
                .units
                .iter()
                .filter(|unit| unit.transport == Some(lost) && !losses.contains(&unit.id))
                .map(|unit| (!self.can_leave_transport(unit), unit.clone()))
                .collect();
            cargo.sort_by_key(|(helpless, unit)| {
                (!*helpless, unit.kind != UnitKind::BarbarianLeader)
            });
            for (helpless, passenger) in cargo {
                if !self.rescue_transport_cargo(lost, &passenger, helpless, &losses) {
                    losses.push(passenger.id);
                }
            }
            index += 1;
        }
        self.units.retain(|unit| !losses.contains(&unit.id));
        for lost in losses {
            self.discard_unit_ai(lost);
        }
    }

    fn rescue_transport_cargo(
        &mut self,
        lost: u32,
        passenger: &Unit,
        helpless: bool,
        losses: &[u32],
    ) -> bool {
        let tile = (passenger.x, passenger.y);
        let can_survive = !helpless
            && (passenger.kind.is_aircraft() || !self.tile(tile.0, tile.1).terrain.is_water());
        let mut destination = tile;
        let mut replacement = None;
        if !can_survive {
            if !helpless {
                replacement = self
                    .units
                    .iter()
                    .filter(|ship| (ship.x, ship.y) == tile && self.can_board_ship(passenger, ship))
                    .filter_map(|ship| {
                        let mut outermost = ship;
                        let mut depth = 0;
                        let mut total_moves = 0;
                        loop {
                            if losses.contains(&outermost.id) || outermost.id == passenger.id {
                                return None;
                            }
                            total_moves += outermost.moves_left + self.unit_move_rate(outermost);
                            let Some(parent) = outermost.transport else {
                                break;
                            };
                            depth += 1;
                            if depth >= 5 {
                                return None;
                            }
                            outermost = self.units.iter().find(|unit| unit.id == parent)?;
                        }
                        let busy = ship.state != UnitState::Idle || ship.work_left > 0;
                        let restricted_unload = ship.kind == UnitKind::Helicopter
                            && !Self::can_disembark_helicopter_in_field(passenger);
                        Some((
                            (
                                busy,
                                restricted_unload,
                                depth,
                                -outermost.moves_left,
                                -total_moves,
                            ),
                            ship.id,
                        ))
                    })
                    .min_by_key(|(priority, _)| *priority)
                    .map(|(_, id)| id);
            }
            if replacement.is_none() {
                if passenger.kind != UnitKind::BarbarianLeader {
                    return false;
                }
                let carrier_kind = self.units.iter().find(|unit| unit.id == lost).unwrap().kind;
                let Some(city) = self
                    .cities
                    .iter()
                    .filter(|city| {
                        city.owner == passenger.owner
                            && (!carrier_kind.is_naval()
                                || self.naval_city_access(city, carrier_kind))
                    })
                    .min_by_key(|city| self.map_distance(tile.0, tile.1, city.x, city.y))
                else {
                    return false;
                };
                destination = (city.x, city.y);
            }
        }

        let unit = self
            .units
            .iter_mut()
            .find(|unit| unit.id == passenger.id)
            .unwrap();
        unit.transport = replacement;
        if unit.state == UnitState::Sentry {
            unit.state = UnitState::Idle;
        }
        if destination != tile {
            (unit.x, unit.y) = destination;
            self.move_cargo(passenger.id, destination.0, destination.1);
        }
        true
    }

    pub(super) fn stack_vulnerable(&self, x: i32, y: i32) -> bool {
        // combat.c:is_stack_vulnerable, default killstack=TRUE. None of the
        // supplied Civ2Civ3 extras carry NoStackDeath, including bases.
        !self.cities.iter().any(|city| (city.x, city.y) == (x, y))
    }

    pub(super) fn combat_stack_losses<'a>(
        &'a self,
        loser: &'a Unit,
        winner: &'a Unit,
    ) -> impl Iterator<Item = &'a Unit> {
        let protected = !self.stack_vulnerable(loser.x, loser.y);
        self.units.iter().filter(move |unit| {
            unit.id == loser.id
                || (!protected
                    && (unit.x, unit.y) == (loser.x, loser.y)
                    && self.at_war(winner.owner, unit.owner)
                    && if unit.kind.is_aircraft() {
                        winner.kind.can_attack_aircraft()
                            || (!winner.kind.is_aircraft()
                                && self.refuel_site(unit.owner, unit.x, unit.y))
                    } else {
                        winner.kind.is_aircraft() || unit.kind.is_naval() == winner.kind.is_naval()
                    })
        })
    }

    fn remove_combat_losses(&mut self, loser: &Unit, winner: &Unit) {
        let losses = self
            .combat_stack_losses(loser, winner)
            .map(|unit| unit.id)
            .collect();
        let before = self.units.len();
        self.remove_units_with_cargo(losses);
        self.nation_scores[winner.owner].units_killed += (before - self.units.len()) as i32;
    }

    pub(super) fn resolve_combat(&mut self, attacker_id: u32, defender_id: u32, nx: i32, ny: i32) {
        let Some(attacker_index) = self.units.iter().position(|u| u.id == attacker_id) else {
            return;
        };
        let Some(defender_index) = self.units.iter().position(|u| u.id == defender_id) else {
            return;
        };
        let attacker = self.units[attacker_index].clone();
        let defender = self.units[defender_index].clone();
        // Collect Ransom uses the ordinary attack roll, but its eligibility
        // and payout depend on the entire target stack before combat.
        let ransom_stack = if defender.owner == BARBARIANS
            && defender.kind == UnitKind::BarbarianLeader
            && self.at_war(attacker.owner, defender.owner)
            && attacker.moves_left > 0
            && attacker.kind.is_military()
            && !attacker.kind.is_naval()
            && (!attacker.kind.is_aircraft() || attacker.kind == UnitKind::Helicopter)
            && (!self.tile(attacker.x, attacker.y).terrain.is_water()
                || (attacker.kind.has_marine_attack() || attacker.kind == UnitKind::Helicopter))
        {
            self.units
                .iter()
                .filter(|unit| (unit.x, unit.y) == (nx, ny))
                .try_fold(0, |count, unit| {
                    (unit.kind == UnitKind::BarbarianLeader).then_some(count + 1)
                })
                .unwrap_or(0)
        } else {
            0
        };
        let (attack_firepower, defense_firepower) = self.combat_firepower(&attacker, &defender);

        let attack_power = Self::attack_power(&attacker);
        let defense_power = self.defense_power_against(&defender, nx, ny, attacker.kind);
        let mut att_hp = attacker.hp;
        let mut def_hp = defender.hp;
        if attack_power <= 0 {
            att_hp = 0;
        } else if defense_power <= 0 {
            def_hp = 0;
        }
        let mut visual_hp = self
            .visual_events
            .enabled
            .then(|| vec![(attacker.hp, defender.hp)]);
        let mut rounds = 0;
        while att_hp > 0 && def_hp > 0 {
            if self.rand_below(attack_power + defense_power) >= defense_power {
                def_hp -= attack_firepower;
            } else {
                att_hp -= defense_firepower;
            }
            rounds += 1;
            if let Some(hp) = &mut visual_hp {
                hp.push((att_hp.max(0), def_hp.max(0)));
            }
        }
        if let Some(mut hp) = visual_hp {
            hp.push((att_hp.max(0), def_hp.max(0)));
            self.publish_visual_event(VisualEvent::Combat {
                attacker: attacker.clone(),
                defender: defender.clone(),
                hp,
                city: self.cities.iter().find(|c| (c.x, c.y) == (nx, ny)).cloned(),
                territory: (0..MAP_W * MAP_H)
                    .map(|i| self.displayed_tile_owner(i % MAP_W, i / MAP_W))
                    .collect(),
            });
        }
        att_hp = att_hp.max(0);
        def_hp = def_hp.max(0);

        if let Some(a) = self.units.iter_mut().find(|u| u.id == attacker_id) {
            a.hp = att_hp;
            a.moves_left = if a.kind.bombard_rate() > 0 {
                0
            } else {
                (a.moves_left - MOVE_FRAGMENTS).max(0)
            };
            a.moved = true;
        }
        if let Some(d) = self.units.iter_mut().find(|u| u.id == defender_id) {
            d.hp = def_hp;
        }

        if attack_power > 0 && defense_power > 0 {
            let attack_strength = attack_power * attacker.hp * attack_firepower;
            let defense_strength = defense_power * defender.hp * defense_firepower;
            let total = attack_strength + defense_strength;
            if def_hp == 0 {
                if attacker.kind != UnitKind::CruiseMissile {
                    self.promote_unit(attacker_id, 200 * defense_strength / total);
                }
            } else if att_hp == 0 {
                self.promote_unit(defender_id, 200 * attack_strength / total);
            }
        }

        let attacker_name = attacker.kind.name();
        let defender_name = defender.kind.name();
        let attacker_owner = attacker.owner;
        let defender_owner = defender.owner;
        if def_hp <= 0 {
            if ransom_stack > 0 {
                let ransom = self.players[defender_owner]
                    .gold
                    .clamp(0, 100 * ransom_stack);
                self.players[defender_owner].gold -= ransom;
                self.players[attacker_owner].gold += ransom;
                if attacker_owner == HUMAN {
                    self.add_event(format!(
                        "Collected {ransom} gold in Barbarian Leader ransom."
                    ));
                }
            }
            self.remove_combat_losses(&defender, &attacker);
            self.apply_combat_population_loss(&attacker, nx, ny);
            if attacker.kind == UnitKind::CruiseMissile {
                self.remove_units_with_cargo(vec![attacker_id]);
            }
            let occupied = self.units.iter().any(|unit| {
                (unit.x, unit.y) == (nx, ny) && !self.allied(unit.owner, attacker_owner)
            });
            if !occupied
                && !attacker.kind.is_aircraft()
                && !self.treaty_blocks_move(&attacker, nx, ny)
                && let Some(cost) = self.movement_cost_for(&attacker, nx, ny)
                && self.cities.iter().all(|city| {
                    (city.x, city.y) != (nx, ny)
                        || !self.at_war(city.owner, attacker_owner)
                        || attacker.kind.can_occupy_city()
                })
            {
                self.enter_tile(attacker_id, nx, ny, (cost - MOVE_FRAGMENTS).max(0));
            }
            if attacker_owner == HUMAN || defender_owner == HUMAN {
                self.add_event(format!(
                    "{attacker_name} defeated {defender_name} after {rounds} combat rounds."
                ));
            }
        } else if att_hp <= 0 {
            self.remove_combat_losses(&attacker, &defender);
            if attacker_owner == HUMAN || defender_owner == HUMAN {
                self.add_event(format!(
                    "{defender_name} held the tile against {attacker_name}."
                ));
            }
        }
        if attacker_owner == HUMAN || defender_owner == HUMAN {
            self.refresh_visibility();
        }
        self.check_game_over();
    }

    pub(super) fn path_to(&self, unit_id: u32, tx: i32, ty: i32) -> Vec<(i32, i32)> {
        let Some(unit) = self.units.iter().find(|u| u.id == unit_id) else {
            return Vec::new();
        };
        self.path_for_unit(unit, tx, ty)
    }

    pub(super) fn path_for_unit(&self, unit: &Unit, tx: i32, ty: i32) -> Vec<(i32, i32)> {
        self.plan_route(unit, tx, ty, false)
    }

    pub(super) fn plan_route(
        &self,
        unit: &Unit,
        tx: i32,
        ty: i32,
        observed: bool,
    ) -> Vec<(i32, i32)> {
        self.search_route(unit, tx, ty, observed, i32::MAX)
            .map_or_else(Vec::new, |(path, _)| path)
    }

    pub(super) fn route_cost_within(&self, unit: &Unit, x: i32, y: i32, limit: i32) -> Option<i32> {
        self.search_route(unit, x, y, false, limit)
            .map(|(_, cost)| cost)
    }

    pub(super) fn search_route(
        &self,
        unit: &Unit,
        tx: i32,
        ty: i32,
        observed: bool,
        limit: i32,
    ) -> Option<(Vec<(i32, i32)>, i32)> {
        let start = Self::idx(unit.x, unit.y);
        let goal = Self::idx(tx, ty);
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
            let (cx, cy) = Self::xy(current);
            (step_unit.x, step_unit.y) = (cx, cy);
            let remaining = if spent < unit.moves_left {
                unit.moves_left - spent
            } else {
                allowance - (spent - unit.moves_left.max(0)) % allowance
            };
            for (dx, dy) in ISOHEX_DIRS {
                let (nx, ny) = offset_native_by_map(cx, cy, dx, dy);
                let ni = Self::idx(nx, ny);
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
        if previous[goal].is_none() {
            return None;
        }
        let mut path = Vec::new();
        let mut cursor = goal;
        while cursor != start {
            let Some((prior, direction)) = previous[cursor] else {
                return None;
            };
            path.push(direction);
            cursor = prior;
        }
        path.reverse();
        Some((path, best[goal].0))
    }

    pub(super) fn player_goto(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == id) else {
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
            let Some(unit) = self.units.iter().find(|u| u.id == unit_id) else {
                break;
            };
            if unit.moves_left <= 0 || result == MoveResult::Fought {
                break;
            }
        }
        moved
    }

    pub(crate) fn can_railroad_at(&self, owner: usize, x: i32, y: i32) -> bool {
        let tile = self.tile(x, y);
        tile.road
            && !tile.railroad
            && !tile.terrain.is_water()
            && self.has_tech(owner, Tech::RAILROAD)
            && (!tile.river || self.has_tech(owner, Tech::BRIDGE_BUILDING))
    }

    pub(super) fn begin_work(&mut self, unit_id: u32, kind: WorkKind) -> bool {
        let Some(index) = self.units.iter().position(|u| u.id == unit_id) else {
            return false;
        };
        let unit = self.units[index].clone();
        if (!unit.kind.is_worker()
            && !matches!(
                kind,
                WorkKind::Pillage(_)
                    | WorkKind::BuildBase(_)
                    | WorkKind::Clean(_)
                    | WorkKind::OilPlatform
            ))
            || unit.work_left > 0
            || (unit.transport.is_some()
                && !matches!(kind, WorkKind::Transform | WorkKind::OilPlatform))
        {
            return false;
        }
        let tile = self.tile(unit.x, unit.y);
        let turns = match kind {
            WorkKind::Road
                if !tile.road
                    && (!tile.river || self.has_tech(unit.owner, Tech::BRIDGE_BUILDING)) =>
            {
                tile.terrain.road_time()
            }
            WorkKind::Railroad if self.can_railroad_at(unit.owner, unit.x, unit.y) => 3,
            WorkKind::Irrigation
                if unit.kind.unit_class() != super::unit_catalog::UnitClass::Sea
                    && self.can_irrigate_at(unit.owner, unit.x, unit.y) =>
            {
                tile.terrain.irrigation_time()
            }
            WorkKind::Farmland
                if unit.kind.unit_class() != super::unit_catalog::UnitClass::Sea
                    && self.can_farm_at(unit.owner, unit.x, unit.y) =>
            {
                tile.terrain.irrigation_time()
            }
            WorkKind::Cultivate | WorkKind::Plant | WorkKind::Transform => {
                self.terrain_work(&unit, kind).map_or(0, |(_, time)| time)
            }
            WorkKind::BuildBase(base) => self.base_work_time(&unit, base).unwrap_or(0),
            WorkKind::Clean(target) if unit.kind.can_clean() && tile.has_contamination(target) => 3,
            WorkKind::Pillage(target) if self.can_pillage(&unit, target) => 1,
            WorkKind::OilWell | WorkKind::OilPlatform if self.oil_work(&unit) == Some(kind) => {
                if kind == WorkKind::OilPlatform {
                    10
                } else {
                    tile.terrain.mine_time()
                }
            }
            WorkKind::Mine
                if !tile.mine && unit.kind.unit_class() != super::unit_catalog::UnitClass::Sea =>
            {
                tile.terrain.mine_time()
            }
            _ => 0,
        };
        if turns <= 0 {
            if unit.owner == HUMAN {
                self.add_event(format!(
                    "{} cannot build {} on {}.",
                    unit.kind.name(),
                    kind.name(),
                    tile.terrain.name()
                ));
            }
            return false;
        }
        let worker = &mut self.units[index];
        worker.work_kind = Some(kind);
        worker.work_left = turns * ACTIVITY_FACTOR;
        worker.work_done = 0;
        worker.state = UnitState::Idle;
        let turns = self.project_turns_left(&self.units[index]);
        if unit.owner == HUMAN {
            self.add_event(format!(
                "{} began {} ({turns} turns).",
                unit.kind.name(),
                kind.name()
            ));
        }
        true
    }

    pub(super) fn fortify_unit(&mut self, unit_id: u32) -> bool {
        let (owner, name) = {
            let Some(unit) = self.units.iter_mut().find(|u| u.id == unit_id) else {
                return false;
            };
            if !unit.kind.can_fortify() || unit.transport.is_some() {
                return false;
            }
            unit.state = UnitState::Fortifying;
            (unit.owner, unit.kind.name())
        };
        if owner == HUMAN {
            self.add_event(format!("{name} will be fortified next turn."));
        }
        true
    }

    pub(crate) fn project_turns_left(&self, unit: &Unit) -> i32 {
        let Some(project) = unit.work_key() else {
            return unit.work_turns_left();
        };
        let (progress, rate) = self
            .units
            .iter()
            .filter(|worker| worker.work_key() == Some(project))
            .fold((0, 0), |(progress, rate), worker| {
                (progress + worker.work_done, rate + worker.activity_rate())
            });
        if rate == 0 {
            return unit.work_turns_left();
        }
        let remaining = (unit.work_left + unit.work_done - progress).max(0);
        (remaining + rate - 1) / rate
    }

    pub(super) fn restore_player_units(&mut self, owner: usize) {
        self.apply_workshop_upgrades_for(owner);
        let ids: Vec<_> = self
            .player_units_ordered(owner)
            .map(|unit| unit.id)
            .collect();
        for id in ids {
            let Some(unit) = self.units.iter().find(|unit| unit.id == id) else {
                continue;
            };
            if unit.kind.is_aircraft() {
                self.restore_aircraft_unit(id);
            } else {
                self.restore_ground_unit(id);
            }
        }
        self.finish_aircraft_fuel_deaths(owner);
    }

    fn restore_ground_unit(&mut self, id: u32) {
        let Some(unit) = self.units.iter().find(|unit| unit.id == id) else {
            return;
        };
        let center = self
            .cities
            .iter()
            .find(|city| (city.x, city.y) == (unit.x, unit.y));
        let repair = center.is_some_and(|city| {
            if unit.kind.is_naval() {
                city.has_building(Building::PORT_FACILITY)
            } else {
                self.active_barracks(city)
            }
        });
        let fortress = self.tile(unit.x, unit.y).base == Some(Base::Fortress);
        let hp = if !unit.moved && repair {
            unit.kind.stats().hp
        } else {
            unit.hp + Self::hitpoint_gain(unit, center.is_some(), fortress)
        };
        let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
        unit.hp = hp;
        unit.moved = false;
        if hp <= 0 {
            self.remove_units_with_cargo(vec![id]);
        }
    }

    // Compatibility fixture helper; runtime uses the two source phases.
    pub(super) fn update_activities_and_restore(&mut self) {
        let ids: Vec<_> = (0..self.players.len())
            .flat_map(|owner| self.player_units_ordered(owner))
            .filter(|unit| !unit.kind.is_aircraft())
            .map(|unit| unit.id)
            .collect();
        for id in ids {
            self.restore_ground_unit(id);
        }
        for owner in 0..self.players.len() {
            self.progress_player_unit_activities(owner);
        }
    }

    pub(super) fn progress_player_unit_activities(&mut self, owner: usize) {
        let mut required = std::collections::HashMap::<_, i32>::new();
        for unit in &self.units {
            if let Some(key) = unit.work_key() {
                let time = required.entry(key).or_default();
                *time = (*time).max(unit.work_left + unit.work_done);
            }
        }
        let ids: Vec<_> = self
            .player_units_ordered(owner)
            .map(|unit| unit.id)
            .collect();
        for id in ids {
            let Some(index) = self.units.iter().position(|unit| unit.id == id) else {
                continue;
            };
            let unit = &mut self.units[index];
            let work = unit.work_key();
            if work.is_some() && unit.moves_left > 0 {
                // Native activity_count receives a FULL activity rate whenever
                // any old MP remain, even when that overshoots the project time.
                let rate = unit.activity_rate();
                unit.work_left = (unit.work_left - rate).max(0);
                unit.work_done += rate;
            }
            let fortifies = unit.state == UnitState::Fortifying && unit.moves_left > 0;
            if work.is_some() {
                self.promote_worker(id);
            }
            let rate = self.unit_move_rate(&self.units[index]);
            self.units[index].moves_left = rate;
            if let Some(key @ (x, y, kind)) = work {
                let progress: i32 = self
                    .units
                    .iter()
                    .filter(|unit| unit.work_key() == Some(key))
                    .map(|unit| unit.work_done)
                    .sum();
                if progress >= required[&key] && self.complete_unit_project(id, x, y, kind) {
                    // Native completion clears all matching coworkers before
                    // the next unit's activity/promotion and MP restoration.
                    for unit in &mut self.units {
                        if unit.work_key() == Some(key) {
                            unit.clear_work();
                        }
                    }
                }
            }
            if fortifies && let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) {
                unit.state = UnitState::Fortified;
            }
        }
    }

    fn complete_unit_project(&mut self, id: u32, x: i32, y: i32, kind: WorkKind) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == id).cloned() else {
            return false;
        };
        let owner = unit.owner;
        if matches!(
            kind,
            WorkKind::Cultivate | WorkKind::Plant | WorkKind::Transform
        ) {
            let Some((target, _)) = self.terrain_work(&unit, kind) else {
                return false;
            };
            let source = self.tile(x, y).terrain;
            self.alter_terrain(x, y, target);
            if owner == HUMAN {
                self.add_event(format!(
                    "Your workers changed {} to {}.",
                    source.name(),
                    target.name()
                ));
            }
            return true;
        }
        if let WorkKind::BuildBase(base) = kind {
            if self.base_work_time(&unit, base).is_none() {
                return false;
            }
            self.tile_mut(x, y).base = Some(base);
            if owner == HUMAN {
                self.add_event(format!("Your unit completed a {}.", base.name()));
            }
            return true;
        }
        if let WorkKind::Pillage(target) = kind {
            self.finish_pillage(owner, x, y, target);
            return true;
        }
        let terrain = self.tile(x, y).terrain;
        let valid = match kind {
            WorkKind::Road | WorkKind::Railroad => !terrain.is_water(),
            WorkKind::Irrigation => terrain.irrigation_time() > 0,
            WorkKind::Farmland => self.can_farm_at(owner, x, y),
            WorkKind::Mine => terrain.mine_time() > 0,
            WorkKind::OilWell => {
                self.tile(x, y).mine && matches!(terrain, Terrain::Desert | Terrain::Glacier)
            }
            WorkKind::OilPlatform => terrain == Terrain::DeepOcean,
            WorkKind::Clean(target) => self.tile(x, y).has_contamination(target),
            _ => unreachable!(),
        };
        if !valid {
            return false;
        }
        let tile = self.tile_mut(x, y);
        match kind {
            WorkKind::Road => tile.road = true,
            WorkKind::Railroad => tile.railroad = true,
            WorkKind::Irrigation => {
                tile.irrigation = true;
                tile.mine = false;
                tile.oil_well = false;
            }
            WorkKind::Farmland => tile.farmland = true,
            WorkKind::OilWell => tile.oil_well = true,
            WorkKind::OilPlatform => tile.oil_platform = true,
            WorkKind::Clean(Contamination::Pollution) => tile.pollution = false,
            WorkKind::Clean(Contamination::Fallout) => tile.fallout = false,
            WorkKind::Mine => {
                tile.mine = true;
                tile.irrigation = false;
                tile.farmland = false;
            }
            _ => unreachable!(),
        }
        if owner == HUMAN {
            self.add_event(if matches!(kind, WorkKind::Clean(_)) {
                format!("Your workers cleaned {}.", kind.name())
            } else {
                format!("Your workers completed {}.", kind.name())
            });
        }
        true
    }
}

impl Simulation {
    pub(crate) fn activity_completion_order_playtest(&self) -> (bool, bool, bool, bool) {
        let fresh = || {
            let mut game = self.clone();
            game.units.clear();
            game.cities.clear();
            for tile in &mut game.tiles {
                tile.terrain = Terrain::Mountains;
                tile.road = false;
                tile.river = false;
                tile.base = None;
                tile.hut = false;
            }
            game
        };
        let road_mp = |worker_first: bool| {
            let mut game = fresh();
            let (worker, chariot) = if worker_first {
                let chariot = game.spawn_unit(HUMAN, UnitKind::Chariot, 10, 24);
                (game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24), chariot)
            } else {
                let worker = game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24);
                (worker, game.spawn_unit(HUMAN, UnitKind::Chariot, 10, 24))
            };
            game.begin_work(worker, WorkKind::Road);
            let unit = game
                .units
                .iter_mut()
                .find(|unit| unit.id == worker)
                .unwrap();
            unit.work_done = unit.work_left - ACTIVITY_FACTOR;
            unit.work_left = ACTIVITY_FACTOR;
            game.progress_player_unit_activities(HUMAN);
            let mp = game
                .units
                .iter()
                .find(|unit| unit.id == chariot)
                .unwrap()
                .moves_left;
            (game.tile(10, 24).road, mp)
        };
        let road_order =
            road_mp(true) == (true, 2 * MOVE_FRAGMENTS) && road_mp(false) == (true, MOVE_FRAGMENTS);

        let mut game = fresh();
        let later = game.spawn_unit(HUMAN, UnitKind::Engineer, 10, 24);
        let first = game.spawn_unit(HUMAN, UnitKind::Engineer, 10, 24);
        game.begin_work(first, WorkKind::Road);
        game.begin_work(later, WorkKind::Road);
        let unit = game.units.iter_mut().find(|unit| unit.id == first).unwrap();
        unit.work_done = unit.work_left - 20;
        unit.work_left = 20;
        game.set_random_seed(2); // First 51 fails promotion; the coworker must not roll.
        let before = game.random_snapshot();
        game.rand_below(100);
        let one_roll = game.random_snapshot();
        game.restore_random(before);
        game.progress_player_unit_activities(HUMAN);
        let coworkers = game.tile(10, 24).road
            && game.random_snapshot() == one_roll
            && [first, later].into_iter().all(|id| {
                game.units
                    .iter()
                    .any(|unit| unit.id == id && unit.work_kind.is_none() && unit.veteran == 0)
            });

        let mut game = fresh();
        let first = game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24);
        let later = game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24);
        game.begin_work(first, WorkKind::Road);
        game.begin_work(later, WorkKind::Road);
        for unit in &mut game.units {
            unit.work_done = unit.work_left / 2;
            unit.work_left -= unit.work_done;
            unit.moves_left = 0;
        }
        game.progress_player_unit_activities(HUMAN);
        let precompleted = game.tile(10, 24).road
            && [first, later].into_iter().all(|id| {
                game.units
                    .iter()
                    .any(|unit| unit.id == id && unit.work_kind.is_none())
            });

        let mut game = fresh();
        let engineer = game.spawn_unit(HUMAN, UnitKind::Engineer, 10, 24);
        game.begin_work(engineer, WorkKind::Road);
        let initial_work = game.units.last().unwrap().work_left;
        game.units.last_mut().unwrap().moves_left = 0;
        game.set_random_seed(20); // Native 1 succeeds at 3%, despite zero old MP.
        game.progress_player_unit_activities(HUMAN);
        let unit = game.units.iter().find(|unit| unit.id == engineer).unwrap();
        let zero_mp_promotes =
            unit.veteran == 1 && unit.work_done == 0 && unit.work_left == initial_work;
        game.units.last_mut().unwrap().moves_left = 1;
        let full_rate = game.units.last().unwrap().activity_rate();
        game.progress_player_unit_activities(HUMAN);
        let partial_mp = zero_mp_promotes && game.units.last().unwrap().work_done == full_rate;
        (road_order, coworkers, precompleted, partial_mp)
    }

    pub(crate) fn unit_turn_lifecycle_playtest(&self) -> (bool, bool, bool, bool) {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        game.players[HUMAN].known = vec![Tech::BRONZE_WORKING];
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.hut = false;
            tile.owner = None;
            tile.known_by[HUMAN] = true;
        }
        game.create_city(HUMAN, 10, 24);
        game.cities[0].production = Production::Building(Building::BARRACKS);
        game.cities[0].shields = 100;
        game.cities[0].food = 10;
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
        game.units.last_mut().unwrap().hp = 1;
        game.restore_player_units(HUMAN);
        let ordinary_hp = game.units.iter().find(|unit| unit.id == id).unwrap().hp;
        game.process_cities();
        let built = game.cities[0].has_building(Building::BARRACKS)
            && ordinary_hp < UnitKind::Warrior.stats().hp
            && game.units.iter().find(|unit| unit.id == id).unwrap().hp == ordinary_hp;
        game.restore_player_units(HUMAN);
        let repaired = built
            && game.units.iter().find(|unit| unit.id == id).unwrap().hp
                == UnitKind::Warrior.stats().hp;

        let field = offset_native_by_map(10, 24, 5, 0);
        let guard = game.spawn_unit(HUMAN, UnitKind::Warrior, field.0, field.1);
        game.units.last_mut().unwrap().hp = 1;
        let old_mp = game.units.last().unwrap().moves_left;
        game.fortify_unit(guard);
        let retained = game.units.last().unwrap().moves_left == old_mp;
        game.restore_player_units(HUMAN);
        let unfortified_hp = game.units.last().unwrap().hp == 2;
        game.progress_player_unit_activities(HUMAN);
        let fortified = retained
            && unfortified_hp
            && game.units.last().unwrap().hp == 2
            && game.units.last().unwrap().state == UnitState::Fortified;
        let exhausted = game.spawn_unit(HUMAN, UnitKind::Warrior, field.0, field.1);
        game.units.last_mut().unwrap().moves_left = 0;
        game.fortify_unit(exhausted);
        game.progress_player_unit_activities(HUMAN);
        let waited = game.units.last().unwrap().state == UnitState::Fortifying
            && game.units.last().unwrap().moves_left > 0;

        let base = offset_native_by_map(10, 24, 0, -1);
        game.tile_mut(base.0, base.1).base = Some(Base::Airbase);
        let air = offset_native_by_map(base.0, base.1, 0, -1);
        let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, air.0, air.1);
        game.units.last_mut().unwrap().fuel = 1;
        game.restore_player_units(HUMAN);
        let rescued = game
            .units
            .iter()
            .find(|unit| unit.id == bomber)
            .is_some_and(|unit| (unit.x, unit.y) == base && unit.moved);
        game.progress_player_unit_activities(HUMAN);
        let rescue_moved = rescued
            && game
                .units
                .iter()
                .find(|unit| unit.id == bomber)
                .is_some_and(|unit| unit.moved && unit.moves_left > 0);
        (repaired, fortified, waited, rescue_moved)
    }

    pub(crate) fn native_movement_rules_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.base = None;
            tile.hut = false;
        }
        let home = (10, 24);
        let dest = offset_native_by_map(home.0, home.1, 1, 0);
        for p in [home, dest] {
            self.tile_mut(p.0, p.1).river = true;
        }
        let warrior = self.make_unit(0, HUMAN, UnitKind::Warrior, home.0, home.1);
        let chariot = self.make_unit(0, HUMAN, UnitKind::Chariot, home.0, home.1);
        let boat = self.make_unit(0, HUMAN, UnitKind::Trireme, home.0, home.1);
        let river = self.movement_cost_for(&warrior, dest.0, dest.1) == Some(2)
            && self.movement_cost_for(&chariot, dest.0, dest.1) == Some(6)
            && self.movement_cost_for(&boat, dest.0, dest.1) == Some(6);
        for p in [home, dest] {
            self.tile_mut(p.0, p.1).railroad = true;
        }
        let rail = self.movement_cost_for(&warrior, dest.0, dest.1) == Some(1);
        self.tile_mut(dest.0, dest.1).owner = Some(1);
        let hostile = self.movement_cost_for(&warrior, dest.0, dest.1) == Some(6);
        self.tile_mut(dest.0, dest.1).owner = None;
        self.tile_mut(dest.0, dest.1).terrain = Terrain::Mountains;
        self.tile_mut(dest.0, dest.1).railroad = false;
        let wheel_blocked = self.movement_cost_for(&chariot, dest.0, dest.1).is_none();
        self.tile_mut(dest.0, dest.1).road = true;
        let wheel_road = self.movement_cost_for(&chariot, dest.0, dest.1).is_some();
        self.tile_mut(dest.0, dest.1).terrain = Terrain::Ocean;
        self.spawn_unit(HUMAN, UnitKind::Transport, dest.0, dest.1);
        let explorer = self.spawn_unit(HUMAN, UnitKind::Explorer, home.0, home.1);
        let before = self
            .units
            .iter()
            .find(|u| u.id == explorer)
            .unwrap()
            .moves_left;
        let boarding = self.try_move_unit(explorer, 1, 0) == MoveResult::Moved
            && self
                .units
                .iter()
                .find(|u| u.id == explorer)
                .is_some_and(|u| u.transport.is_some() && u.moves_left == before - 2);
        self.units.clear();
        let enemy = self.spawn_unit(1, UnitKind::BarbarianLeader, home.0, home.1);
        let leader_zoc = self.under_foreign_control(HUMAN, home.0, home.1, false);
        self.units.iter_mut().find(|u| u.id == enemy).unwrap().kind = UnitKind::Carrier;
        let carrier_no_zoc = !self.under_foreign_control(HUMAN, home.0, home.1, false);
        let actors = !UnitKind::Caravan.ignores_zoc()
            && !UnitKind::Migrants.ignores_zoc()
            && !UnitKind::Chariot.ignores_zoc()
            && !UnitKind::Trireme.ignores_zoc()
            && UnitKind::Spy.ignores_zoc();
        (
            river && rail && hostile,
            wheel_blocked && wheel_road && boarding,
            leader_zoc && carrier_no_zoc && actors,
        )
    }

    pub(crate) fn civilian_border_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.road = false;
            tile.railroad = false;
            tile.owner = None;
            tile.visible = true;
            tile.seen = true;
            tile.known_by[HUMAN] = true;
        }
        self.create_city(1, 20, 40);
        self.sign_pact(HUMAN, 1, Pact::Peace);
        self.diplomacy_playtest_tick(16);
        if self.pact(HUMAN, 1) != Pact::Peace {
            return false;
        }
        let home = (10, 24);
        let goal = offset_native_by_map(home.0, home.1, 1, 0);
        for (kind, allowed) in [
            (UnitKind::Explorer, true),
            (UnitKind::Spy, true),
            (UnitKind::Caravan, true),
            (UnitKind::Caravel, true),
            (UnitKind::Warrior, false),
            (UnitKind::Carrier, false),
        ] {
            self.units.clear();
            for position in [home, goal] {
                self.tile_mut(position.0, position.1).terrain = if kind.is_naval() {
                    Terrain::Ocean
                } else {
                    Terrain::Grassland
                };
            }
            self.tile_mut(goal.0, goal.1).owner = Some(1);
            // Each case exercises both remembered-map planning and the real
            // movement command; military zero-attack Carrier stays excluded.
            for tile in &mut self.tiles {
                tile.visible = true;
                tile.seen = true;
            }
            let id = self.spawn_unit(HUMAN, kind, home.0, home.1);
            let unit = self.units.last().unwrap().clone();
            let planned = self.plan_route(&unit, goal.0, goal.1, true);
            if (!planned.is_empty()) != allowed
                || (self.try_move_unit(id, 1, 0) == MoveResult::Moved) != allowed
            {
                return false;
            }
        }
        self.units.clear();
        self.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
        self.tile_mut(goal.0, goal.1).terrain = Terrain::Grassland;
        let explorer = self.spawn_unit(HUMAN, UnitKind::Explorer, home.0, home.1);
        self.spawn_unit(1, UnitKind::Warrior, goal.0, goal.1);
        self.try_move_unit(explorer, 1, 0) == MoveResult::Blocked
    }

    pub(crate) fn damaged_movement_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.players[HUMAN]
            .known
            .retain(|&tech| tech != Tech::NUCLEAR_POWER);
        self.tile_mut(10, 24).terrain = Terrain::Ocean;
        self.tile_mut(12, 24).terrain = Terrain::Grassland;
        let ship = self.spawn_unit(HUMAN, UnitKind::Destroyer, 10, 24);
        let horse = self.spawn_unit(HUMAN, UnitKind::Horseman, 12, 24);
        for unit in &mut self.units {
            unit.hp = 1;
            unit.moves_left = 0;
            unit.moved = true;
        }
        self.update_activities_and_restore();
        let restored = [(ship, 12), (horse, 6)].into_iter().all(|(id, rate)| {
            self.units
                .iter()
                .find(|unit| unit.id == id)
                .is_some_and(|unit| unit.moves_left == rate && self.unit_move_rate(unit) == rate)
        });
        self.units
            .iter_mut()
            .find(|unit| unit.id == ship)
            .unwrap()
            .hp = 15;
        self.players[HUMAN].known.push(Tech::NUCLEAR_POWER);
        restored
            && self.unit_move_rate(self.units.iter().find(|unit| unit.id == ship).unwrap()) == 24
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_completion_is_immediate_and_old_mp_controls_only_work_count() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.activity_completion_order_playtest(),
            (true, true, true, true)
        );
    }

    #[test]
    fn next_phase_refills_use_live_mountain_movement_effects() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        let tile = game.tile_mut(10, 24);
        tile.terrain = Terrain::Mountains;
        tile.road = false;
        tile.river = false;
        let id = game.spawn_unit(HUMAN, UnitKind::Chariot, 10, 24);
        game.units.last_mut().unwrap().moves_left = 0;
        game.units.last_mut().unwrap().moved = true;
        game.progress_player_unit_activities(HUMAN);
        let unit = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert_eq!(
            unit.moves_left,
            UnitKind::Chariot.stats().moves * MOVE_FRAGMENTS - MOVE_FRAGMENTS
        );
        assert!(
            unit.moved,
            "begin-phase MP restoration must not clear end-phase movement"
        );
    }

    #[test]
    fn end_phase_repairs_precede_city_builds_and_next_phase_activities() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.unit_turn_lifecycle_playtest(),
            (true, true, true, true)
        );
    }

    #[test]
    fn native_movement_costs_nativity_boarding_and_zoc_flags() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.native_movement_rules_playtest(), (true, true, true));
    }

    #[test]
    fn big_land_has_no_terrain_defense_and_nuclear_is_not_air_attacker() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        let p = (10, 24);
        game.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
        game.tile_mut(p.0, p.1).base = None;
        for kind in [UnitKind::Catapult, UnitKind::Chariot, UnitKind::Cannon] {
            let unit = game.make_unit(0, HUMAN, kind, p.0, p.1);
            let plain = game.defense_power_against(&unit, p.0, p.1, UnitKind::Warrior);
            game.tile_mut(p.0, p.1).terrain = Terrain::Hills;
            assert_eq!(
                game.defense_power_against(&unit, p.0, p.1, UnitKind::Warrior),
                plain
            );
            game.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
        }
        let aegis = game.make_unit(0, HUMAN, UnitKind::AegisCruiser, p.0, p.1);
        let nuclear = game.defense_power_against(&aegis, p.0, p.1, UnitKind::Nuclear);
        assert_eq!(
            game.defense_power_against(&aegis, p.0, p.1, UnitKind::Bomber),
            nuclear * 5
        );
    }

    #[test]
    fn mountain_movement_effect_and_river_defense_follow_native_classes() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        let p = (10, 24);
        let tile = game.tile_mut(p.0, p.1);
        tile.terrain = Terrain::Mountains;
        tile.road = false;
        tile.river = false;
        tile.base = None;
        let horse = game.make_unit(0, HUMAN, UnitKind::Horseman, p.0, p.1);
        assert_eq!(game.unit_move_rate(&horse), 6);
        game.tile_mut(p.0, p.1).river = true;
        assert_eq!(game.unit_move_rate(&horse), 12);
        let chariot = game.make_unit(0, HUMAN, UnitKind::Chariot, p.0, p.1);
        assert_eq!(game.unit_move_rate(&chariot), 6);
        game.tile_mut(p.0, p.1).road = true;
        assert_eq!(game.unit_move_rate(&chariot), 12);
        game.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
        game.tile_mut(p.0, p.1).river = false;
        let legion = game.make_unit(0, HUMAN, UnitKind::Legion, p.0, p.1);
        let plain = game.defense_power_against(&legion, p.0, p.1, UnitKind::Warrior);
        game.tile_mut(p.0, p.1).river = true;
        assert_eq!(
            game.defense_power_against(&legion, p.0, p.1, UnitKind::Warrior),
            plain + plain / 4
        );
        assert_eq!(
            game.defense_power_against(&chariot, p.0, p.1, UnitKind::Warrior),
            chariot.kind.stats().defense * POWER_FACTOR
        );
    }

    #[test]
    fn civilian_peace_borders_follow_ruleset_flags_and_still_exclude_foreign_stacks() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.civilian_border_playtest());
    }

    #[test]
    fn movement_rates_scale_damage_before_effects_and_use_whole_move_class_minima() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.damaged_movement_playtest());
        game.players[HUMAN]
            .known
            .retain(|&tech| tech != Tech::NUCLEAR_POWER);
        let mut horse = game.make_unit(0, HUMAN, UnitKind::Horseman, 10, 24);
        horse.hp = 5;
        assert_eq!(game.unit_move_rate(&horse), 6);
        horse.hp = 1;
        assert_eq!(game.unit_move_rate(&horse), 6);
        let mut ship = game.make_unit(0, HUMAN, UnitKind::Destroyer, 10, 24);
        ship.hp = 1;
        assert_eq!(game.unit_move_rate(&ship), 12);
        ship.hp = 15;
        assert_eq!(game.unit_move_rate(&ship), 18);
        game.players[HUMAN].known.push(Tech::NUCLEAR_POWER);
        assert_eq!(game.unit_move_rate(&ship), 24);
        for kind in [UnitKind::Freight, UnitKind::Bomber] {
            let mut unit = game.make_unit(0, HUMAN, kind, 10, 24);
            let healthy = game.unit_move_rate(&unit);
            unit.hp = 1;
            assert_eq!(game.unit_move_rate(&unit), healthy);
        }
    }

    #[test]
    fn wounded_ground_and_sea_units_restore_the_same_rates_used_by_pathfinding() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.tile_mut(10, 24).terrain = Terrain::Ocean;
        game.tile_mut(12, 24).terrain = Terrain::Grassland;
        let ship = game.spawn_unit(HUMAN, UnitKind::Destroyer, 10, 24);
        let horse = game.spawn_unit(HUMAN, UnitKind::Horseman, 12, 24);
        for unit in &mut game.units {
            unit.hp = 1;
            unit.moves_left = 0;
            unit.moved = true;
        }
        game.update_activities_and_restore();
        for (id, rate) in [(ship, 12), (horse, 6)] {
            let unit = game.units.iter().find(|unit| unit.id == id).unwrap();
            assert_eq!(unit.moves_left, rate);
            assert_eq!(unit.moves_left, game.unit_move_rate(unit));
        }
    }

    #[test]
    fn newly_created_triremes_get_lighthouse_speed_in_their_actual_location() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.create_city(HUMAN, 10, 24);
        game.cities[0].buildings.push(Building::LIGHTHOUSE);
        game.tile_mut(12, 24).terrain = Terrain::Ocean;
        game.tile_mut(12, 24).river = false;
        let id = game.spawn_unit(HUMAN, UnitKind::Trireme, 12, 24);
        let unit = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert_eq!(unit.moves_left, 24);
        assert_eq!(unit.moves_left, game.unit_move_rate(unit));
    }

    #[test]
    fn capture_requires_an_unescorted_stack_and_spends_one_move_without_advancing() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let (actor, merchant, _) = game.merchant_capture_playtest_setup();
        let (x, y) = offset_native_by_map(10, 20, 1, 0);
        let escort = game.spawn_unit(1, UnitKind::Phalanx, x, y);
        let warrior = game
            .units
            .iter()
            .find(|unit| unit.id == actor)
            .unwrap()
            .clone();
        assert!(!game.try_capture_units(&warrior, x, y));
        game.remove_units_with_cargo(vec![escort]);
        game.spawn_unit(1, UnitKind::Caravan, x, y);
        assert!(game.apply(Command::Move(actor, 1, 0)));
        let warrior = game.units.iter().find(|unit| unit.id == actor).unwrap();
        assert_eq!((warrior.x, warrior.y, warrior.moves_left), (10, 20, 0));
        assert!(!game.units.iter().any(|unit| unit.id == merchant));
        assert_eq!(
            game.units
                .iter()
                .filter(|unit| unit.owner == HUMAN
                    && matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight))
                .count(),
            2
        );
    }

    #[test]
    fn workers_pool_matching_progress_only_when_they_have_movement() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let (first, second) = game.cooperative_work_playtest_setup();
        for id in [first, second] {
            assert!(game.apply(Command::Work(id, WorkKind::Irrigation)));
        }
        assert!(game.apply(Command::Wait(first)));
        game.update_activities_and_restore();
        let idle = game.units.iter().find(|unit| unit.id == first).unwrap();
        assert_eq!(idle.work_done, 0);
        assert_eq!(game.project_turns_left(idle), 2);
        game.update_activities_and_restore();
        assert!(!game.tile(10, 20).irrigation);
        game.update_activities_and_restore();
        assert!(game.tile(10, 20).irrigation);
        assert!(
            game.units
                .iter()
                .filter(|unit| [first, second].contains(&unit.id))
                .all(|unit| unit.work_kind.is_none() && unit.work_done == 0)
        );
    }

    #[test]
    fn pathfinding_routes_around_control_and_friendly_stacks_allow_entry() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
        }
        let (warrior, _, (x, y)) = game.zone_control_playtest_setup();
        assert!(!game.apply(Command::Move(warrior, 1, 0)));
        let detour = game.path_to(warrior, x, y);
        assert!(detour.len() > 1);
        assert_ne!(detour[0], (1, 0));
        game.spawn_unit(HUMAN, UnitKind::Worker, x, y);
        assert_eq!(game.path_to(warrior, x, y), [(1, 0)]);
        assert!(game.apply(Command::Move(warrior, 1, 0)));
    }

    #[test]
    fn a_winning_attacker_cannot_occupy_a_city_with_a_second_defender() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (12, 12);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(nx, ny).terrain = Terrain::Grassland;
        game.create_city(1, nx, ny);
        let attacker = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
        let first = game.spawn_unit(1, UnitKind::Warrior, nx, ny);
        let second = game.spawn_unit(1, UnitKind::Warrior, nx, ny);
        game.units
            .iter_mut()
            .find(|unit| unit.id == first)
            .unwrap()
            .hp = 1;
        game.resolve_combat(attacker, first, nx, ny);
        assert!(!game.units.iter().any(|unit| unit.id == first));
        assert!(game.units.iter().any(|unit| unit.id == second));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == attacker && (unit.x, unit.y) == (x, y))
        );
        assert!(
            game.cities
                .iter()
                .any(|city| (city.x, city.y, city.owner) == (nx, ny, 1))
        );
        game.units
            .iter_mut()
            .find(|unit| unit.id == second)
            .unwrap()
            .hp = 1;
        game.units
            .iter_mut()
            .find(|unit| unit.id == attacker)
            .unwrap()
            .moves_left = MOVE_FRAGMENTS;
        game.resolve_combat(attacker, second, nx, ny);
        assert!(
            game.cities
                .iter()
                .any(|city| (city.x, city.y, city.owner) == (nx, ny, HUMAN))
        );
    }

    #[test]
    fn an_unprotected_stack_loses_hostile_units_with_its_defender() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (12, 12);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(nx, ny).terrain = Terrain::Grassland;
        let attacker = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
        let first = game.spawn_unit(1, UnitKind::Warrior, nx, ny);
        let second = game.spawn_unit(1, UnitKind::Worker, nx, ny);
        game.units
            .iter_mut()
            .find(|unit| unit.id == first)
            .unwrap()
            .hp = 1;
        game.resolve_combat(attacker, first, nx, ny);
        assert!(
            !game
                .units
                .iter()
                .any(|unit| unit.id == first || unit.id == second)
        );
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == attacker && (unit.x, unit.y) == (nx, ny))
        );
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    #[test]
    fn rescue_prefers_idle_carriers_then_uses_future_speed_to_break_ties() {
        for fast_ship_working in [false, true] {
            let mut game = Simulation::new(crate::mapgen::generate(42));
            game.units.clear();
            let (x, y) = (10, 20);
            game.tile_mut(x, y).terrain = Terrain::Ocean;
            let lost = game.spawn_unit(HUMAN, UnitKind::Transport, x, y);
            let slow = game.spawn_unit(HUMAN, UnitKind::Caravel, x, y);
            let fast = game.spawn_unit(HUMAN, UnitKind::Transport, x, y);
            let passenger = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
            assert!(game.board_specific_ship(passenger, lost));
            for ship in game
                .units
                .iter_mut()
                .filter(|unit| [slow, fast].contains(&unit.id))
            {
                ship.moves_left = MOVE_FRAGMENTS;
                if ship.id == fast && fast_ship_working {
                    ship.work_kind = Some(WorkKind::Clean(Contamination::Fallout));
                    ship.work_left = 1;
                }
            }

            game.remove_units_with_cargo(vec![lost]);

            let expected = if fast_ship_working { slow } else { fast };
            assert_eq!(
                game.units
                    .iter()
                    .find(|unit| unit.id == passenger)
                    .unwrap()
                    .transport,
                Some(expected)
            );
        }
    }

    #[test]
    fn engineering_replaces_earlier_ferries_and_transport_carries_heavy_units() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        let (x, y) = (10, 20);
        let (sx, sy) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(sx, sy).terrain = Terrain::Ocean;
        game.create_city(HUMAN, x, y);
        let id = game.cities[0].id;
        let caravel = Production::Unit(UnitKind::Caravel);
        let transport = Production::Unit(UnitKind::Transport);
        game.players[HUMAN].known.push(Tech::MAGNETISM);
        assert!(game.apply(Command::Produce(id, caravel)));
        assert!(!game.apply(Command::Produce(id, transport)));
        game.players[HUMAN].known.push(Tech::ENGINEERING);
        assert!(!game.apply(Command::Produce(id, caravel)));
        game.cities[0].shields = transport.cost();
        game.process_cities();
        let ship = game
            .units
            .iter()
            .find(|unit| unit.kind == UnitKind::Transport)
            .unwrap()
            .id;
        assert!(game.apply(Command::Move(ship, 1, 0)));
        for _ in 0..8 {
            let cannon = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
            assert!(game.apply(Command::Move(cannon, 1, 0)));
        }
        let cannon = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
        assert!(!game.apply(Command::Move(cannon, 1, 0)));
        assert_eq!(game.cargo_count(ship), 8);
        game.tile_mut(sx, sy).fallout = true;
        assert!(game.apply(Command::Work(ship, WorkKind::Clean(Contamination::Fallout))));
        for _ in 0..3 {
            game.update_activities_and_restore();
        }
        assert!(!game.tile(sx, sy).fallout);
        assert_eq!(game.cargo_count(ship), 8);
    }

    #[test]
    fn ports_build_transports_only_with_their_tech_and_upgrade_obsolete_production() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        for (dx, dy) in ISOHEX_DIRS {
            let (tx, ty) = offset_native_by_map(x, y, dx, dy);
            game.tile_mut(tx, ty).terrain = Terrain::Grassland;
        }
        game.create_city(HUMAN, x, y);
        let id = game.cities.last().unwrap().id;
        let caravel = Production::Unit(UnitKind::Caravel);
        let galleon = Production::Unit(UnitKind::Galleon);
        game.players[HUMAN].known.push(Tech::MAGNETISM);
        assert!(!game.apply(Command::Produce(id, caravel)));
        let (sx, sy) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(sx, sy).terrain = Terrain::Lake;
        assert!(game.apply(Command::Produce(id, caravel)));
        assert!(!game.apply(Command::Produce(id, galleon)));
        game.players[HUMAN].known.push(Tech::NAVIGATION);
        assert!(!game.apply(Command::Produce(id, caravel)));
        game.cities.last_mut().unwrap().shields = galleon.cost();
        game.process_cities();
        assert!(
            game.units
                .iter()
                .any(|unit| { unit.kind == UnitKind::Galleon && unit.home_city == Some(id) })
        );
    }

    #[test]
    fn adjacent_embark_respects_cargo_class_and_capacity() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        let (x, y) = (10, 20);
        let (sx, sy) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(sx, sy).terrain = Terrain::Ocean;
        let ship = game.spawn_unit(HUMAN, UnitKind::Caravel, sx, sy);
        let cannon = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
        assert!(!game.apply(Command::Move(cannon, 1, 0)));
        for _ in 0..3 {
            let cargo = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
            assert!(game.apply(Command::Move(cargo, 1, 0)));
            assert!(!game.apply(Command::BoardOrDeboard(cargo)));
        }
        assert_eq!(game.cargo_count(ship), 3);
        let cargo = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        assert!(!game.apply(Command::Move(cargo, 1, 0)));
    }
}
