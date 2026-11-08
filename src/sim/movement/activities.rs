//! Unit activity execution such as fortifying, roads, and terrain work.

use super::*;

impl Simulation {
    pub(in crate::sim) fn activity_rate(&self, unit: &Unit) -> i32 {
        let mut rate = unit.activity_rate();
        if unit.kind.is_worker() {
            if self.players[unit.owner].government == Government::Democracy {
                rate = rate * 3 / 2;
            }
            if self.has_tech(unit.owner, Tech::REPLACEABLE_PARTS) {
                rate *= 2;
            }
        }
        rate
    }

    fn work_turns_left(&self, unit: &Unit) -> i32 {
        let rate = self.activity_rate(unit);
        (unit.work_left + rate - 1) / rate
    }

    pub(crate) fn can_railroad_at(&self, owner: usize, x: i32, y: i32) -> bool {
        let tile = self.tile(x, y);
        tile.road
            && !tile.railroad
            && !tile.terrain.is_water()
            && self.has_tech(owner, Tech::STEAM_POWER)
            && (!tile.river || self.has_tech(owner, Tech::ENGINEERING))
    }

    pub(in crate::sim) fn begin_work(&mut self, unit_id: u32, kind: WorkKind) -> bool {
        let Some(index) = self.unit_index_by_id(unit_id) else {
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
                if !tile.road && (!tile.river || self.has_tech(unit.owner, Tech::ENGINEERING)) =>
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

    pub(in crate::sim) fn fortify_unit(&mut self, unit_id: u32) -> bool {
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
            return self.work_turns_left(unit);
        };
        let (progress, rate) = self
            .units
            .iter()
            .filter(|worker| worker.work_key() == Some(project))
            .fold((0, 0), |(progress, rate), worker| {
                (
                    progress + worker.work_done,
                    rate + self.activity_rate(worker),
                )
            });
        if rate == 0 {
            return self.work_turns_left(unit);
        }
        let remaining = (unit.work_left + unit.work_done - progress).max(0);
        (remaining + rate - 1) / rate
    }

    pub(in crate::sim) fn restore_player_units(&mut self, owner: usize) {
        self.apply_workshop_upgrades_for(owner);
        let ids: Vec<_> = self
            .player_units_ordered(owner)
            .map(|unit| unit.id)
            .collect();
        for id in ids {
            let Some(unit) = self.unit_by_id(id) else {
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
        let Some(unit) = self.unit_by_id(id) else {
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
    pub(in crate::sim) fn update_activities_and_restore(&mut self) {
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

    pub(in crate::sim) fn progress_player_unit_activities(&mut self, owner: usize) {
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
            let Some(index) = self.unit_index_by_id(id) else {
                continue;
            };
            let rate = self.activity_rate(&self.units[index]);
            let unit = &mut self.units[index];
            let work = unit.work_key();
            if work.is_some() && unit.moves_left > 0 {
                // Native activity_count receives a FULL activity rate whenever
                // any old MP remain, even when that overshoots the project time.
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
        let Some(unit) = self.unit_by_id(id).cloned() else {
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
            WorkKind::OilPlatform => terrain == Terrain::Ocean,
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
