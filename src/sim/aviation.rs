//! Aircraft, fuel, airlift, and paradrop rules.

use super::*;

mod aircraft_ai;
mod paradrop;

enum CityBombardTarget {
    Unit(Unit),
    Population(u32),
    Building(u32, crate::rules::Building),
}

#[cfg(test)]
fn selected_unit_class_flag(kind: UnitKind, flag: &str) -> Option<bool> {
    crate::sim::action_rules::freeciv_unit_class_flag(kind, flag)
}

fn civ3_bombard_unit(kind: UnitKind) -> Option<()> {
    let range = if kind.bombard_range() > 0 {
        kind.bombard_range()
    } else if kind.is_aircraft() {
        // Civ III aircraft use operational range for air bombard missions;
        // their BIQ bombard_range field is zero.
        kind.operational_range()
    } else {
        0
    };
    (!kind.is_missile() && kind.bombard_strength() > 0 && kind.bombard_rate() > 0 && range > 0)
        .then_some(())
}

fn civ3_air_superiority_unit(kind: UnitKind) -> bool {
    matches!(
        kind,
        UnitKind::Fighter | UnitKind::JetFighter | UnitKind::F15 | UnitKind::StealthFighter
    )
}

fn bombardable_terrain_improvements(sim: &Simulation, x: i32, y: i32) -> Vec<PillageTarget> {
    if sim.city_at(x, y).is_some() || sim.units.iter().any(|unit| (unit.x, unit.y) == (x, y)) {
        return Vec::new();
    }
    let tile = sim.tile(x, y);
    if tile.railroad {
        return vec![PillageTarget::Railroad];
    }

    let mut infrastructure = Vec::new();
    if tile.road {
        infrastructure.push(PillageTarget::Road);
    }
    if tile.mine {
        infrastructure.push(PillageTarget::Mine);
    }
    if tile.irrigation {
        infrastructure.push(PillageTarget::Irrigation);
    }
    if !infrastructure.is_empty() {
        return infrastructure;
    }

    let mut other = Vec::new();
    if tile.farmland {
        other.push(PillageTarget::Farmland);
    }
    if tile.oil_well {
        other.push(PillageTarget::OilWell);
    }
    if tile.oil_platform {
        other.push(PillageTarget::OilPlatform);
    }
    if let Some(base) = tile.base {
        other.push(PillageTarget::Base(base));
    }
    other
}

fn terrain_bombard_defense(sim: &Simulation, x: i32, y: i32) -> i32 {
    let tile = sim.tile(x, y);
    let terrain_bonus = match tile.terrain {
        Terrain::Forest | Terrain::Jungle => 25,
        Terrain::Hills => 50,
        Terrain::Mountains | Terrain::Volcano => 100,
        _ => 10,
    };
    16 + 16 * terrain_bonus / 100
        + if tile.base == Some(Base::Fortress) {
            16 * 25 / 100
        } else {
            0
        }
}

fn bombard_terrain_improvement(sim: &mut Simulation, x: i32, y: i32, target: PillageTarget) {
    let tile = sim.tile_mut(x, y);
    match target {
        PillageTarget::Road => tile.road = false,
        PillageTarget::Railroad => tile.railroad = false,
        PillageTarget::Irrigation => tile.irrigation = false,
        PillageTarget::Farmland => tile.farmland = false,
        PillageTarget::Mine => tile.mine = false,
        PillageTarget::OilWell => tile.oil_well = false,
        PillageTarget::OilPlatform => tile.oil_platform = false,
        PillageTarget::Base(base) => tile.base = base.underlying_after_removal(),
    }
}

fn city_object_bombard_defense(sim: &Simulation, city: &City, attacker: &Unit) -> i32 {
    let mut defense = terrain_bombard_defense(sim, city.x, city.y)
        - if sim.tile(city.x, city.y).base == Some(Base::Fortress) {
            4
        } else {
            0
        };
    defense += if city.size >= 13 {
        16
    } else if city.size >= 7 || city.has_building(crate::rules::Building::CITY_WALLS) {
        8
    } else {
        0
    };
    if attacker.kind.is_naval() && city.has_building(crate::rules::Building::COASTAL_DEFENSE) {
        defense += 8;
    }
    defense
}

fn selected_bombard_blocking_actions() -> Option<Vec<String>> {
    let rules = crate::rules::freeciv::RulesetData::campaign_default();
    let mut parameters = rules.parameters.iter().filter(|parameter| {
        parameter.file.eq_ignore_ascii_case("actions.ruleset")
            && parameter.section.eq_ignore_ascii_case("actions")
            && parameter.key.eq_ignore_ascii_case("bombard_blocked_by")
    });
    let parameter = parameters.next()?;
    if parameters.next().is_some() {
        return None;
    }
    let actions = parameter.string_list_value().ok()??;
    (!actions.is_empty()).then_some(actions)
}

impl Simulation {
    /// Whether the Conquests BIQ gives this unit usable bombard strength,
    /// rate, and a tile or aircraft operational range.
    pub(super) fn has_civ3_bombard_capability(&self, kind: UnitKind) -> bool {
        civ3_bombard_unit(kind).is_some()
    }

    pub(crate) fn can_set_air_superiority(&self, unit: &Unit) -> bool {
        civ3_air_superiority_unit(unit.kind)
            && unit.kind.operational_range() >= 2
            && (self.refuel_site(unit.owner, unit.x, unit.y)
                || self.aboard_refueling_transport(unit))
    }

    pub(crate) fn can_precision_bomb(&self, unit: &Unit) -> bool {
        matches!(
            unit.kind,
            UnitKind::StealthBomber | UnitKind::StealthFighter
        ) && self.has_tech(unit.owner, crate::rules::Tech::SMART_WEAPONS)
            && self.can_begin_bombard(unit)
    }

    pub(super) fn start_air_superiority(&mut self, id: u32) -> bool {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return false;
        };
        if !self.can_set_air_superiority(&unit) {
            return false;
        }
        let Some(index) = self.unit_index_by_id(id) else {
            return false;
        };
        self.units[index].order = Some(UnitOrder::AirSuperiority {
            x: unit.x,
            y: unit.y,
        });
        self.units[index].air_superiority_shot = false;
        self.unit_goto_tiles.remove(&id);
        true
    }

    /// Resolve Conquests-style Air Superiority detection for a mission target.
    /// The manual gives the defensive radius but not detection odds; the PTW
    /// guide's 50% normal / 5% stealth values are used as supplementary data.
    pub(super) fn try_air_superiority_interceptions(
        &mut self,
        attacking_id: u32,
        target: (i32, i32),
    ) -> bool {
        let Some(incoming) = self.unit_by_id(attacking_id).cloned() else {
            return false;
        };
        if !incoming.kind.is_aircraft() {
            return true;
        }
        let interceptors: Vec<_> = self
            .units
            .iter()
            .filter_map(|unit| {
                let Some(UnitOrder::AirSuperiority { x, y }) = unit.order else {
                    return None;
                };
                (self.at_war(unit.owner, incoming.owner)
                    && !unit.air_superiority_shot
                    && civ3_air_superiority_unit(unit.kind)
                    && (unit.x, unit.y) == (x, y)
                    && self.can_set_air_superiority(unit)
                    && self.map_wrap.civ3_distance(x, y, target.0, target.1)
                        <= unit.kind.operational_range() / 2)
                    .then_some((unit.id, unit.kind))
            })
            .collect();
        for (interceptor_id, _) in interceptors {
            if self.unit_by_id(interceptor_id).is_none() {
                continue;
            }
            let detection_chance = if matches!(
                incoming.kind,
                UnitKind::StealthBomber | UnitKind::StealthFighter
            ) {
                5
            } else {
                50
            };
            if self.rand_below(100) >= detection_chance {
                continue;
            }
            if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == interceptor_id) {
                unit.air_superiority_shot = true;
            }
            self.resolve_combat_inner(
                interceptor_id,
                attacking_id,
                target.0,
                target.1,
                None,
                false,
            );
            if self.unit_by_id(attacking_id).is_none() {
                return false;
            }
        }
        self.unit_by_id(attacking_id).is_some()
    }

    fn civ3_bombard_range(kind: UnitKind) -> i32 {
        if kind.bombard_range() > 0 {
            kind.bombard_range()
        } else if kind.is_aircraft() {
            kind.operational_range()
        } else {
            0
        }
    }

    fn bombard_blocked_by_action(
        &self,
        actor: &Unit,
        x: i32,
        y: i32,
        blocked_by: &[String],
    ) -> bool {
        self.tile_units_ordered(x, y).any(|target| {
            if !self.unit_visible_to(actor.owner, target) {
                return false;
            }
            blocked_by.iter().any(|action| {
                self.action_available_for_unit_against_unit(actor, action, target)
                    .unwrap_or(true)
            })
        })
    }

    fn ai_bombard_preempted_by_priority_action(&self, actor: &Unit, x: i32, y: i32) -> bool {
        ["Capture Units", "Collect Ransom"].iter().any(|action| {
            self.tile_units_ordered(x, y).any(|target| {
                self.unit_visible_to(actor.owner, target)
                    && self
                        .action_available_for_unit_against_unit(actor, action, target)
                        .is_ok_and(|available| available)
            })
        })
    }

    pub(crate) fn can_begin_bombard(&self, unit: &Unit) -> bool {
        (unit.owner == HUMAN || self.players.get(unit.owner).is_some_and(|player| player.ai))
            && self.has_civ3_bombard_capability(unit.kind)
            && unit.moves_left > 0
            && (unit.kind.fuel_capacity() == 0
                || self.refuel_site(unit.owner, unit.x, unit.y)
                || self.aboard_refueling_transport(unit))
    }

    fn bombard_action_available_at_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        let city_action = self.city_at(x, y).is_some_and(|city| {
            self.tile(x, y).known_by[unit.owner]
                && city.owner != unit.owner
                && self.at_war(unit.owner, city.owner)
        });
        let unit_action = self.tile_units_ordered(x, y).any(|target| {
            target.owner != unit.owner
                && self.at_war(unit.owner, target.owner)
                && target.transport.is_none()
                && !(unit.kind.fuel_capacity() > 0 && target.kind.fuel_capacity() > 0)
                && self.unit_visible_to(unit.owner, target)
                && self.combat_target_reachable(unit.kind, target.kind, x, y)
        });
        let terrain_action = !city_action
            && !self.units.iter().any(|other| (other.x, other.y) == (x, y))
            && !bombardable_terrain_improvements(self, x, y).is_empty();
        city_action || unit_action || terrain_action
    }

    fn bombard_target_available_for_order(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.has_civ3_bombard_capability(unit.kind)
            || (unit.transport.is_some() && !self.aboard_refueling_transport(unit))
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
            || (x, y) == (unit.x, unit.y)
            || self.map_wrap.civ3_distance(unit.x, unit.y, x, y)
                > Self::civ3_bombard_range(unit.kind)
        {
            return false;
        }
        let Some(blocked_by) = selected_bombard_blocking_actions() else {
            return false;
        };
        !self.bombard_blocked_by_action(unit, x, y, &blocked_by)
            && self.bombard_action_available_at_tile(unit, x, y)
    }

    fn bombard_action_available_for_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        unit.moves_left >= 1 && self.bombard_target_available_for_order(unit, x, y)
    }

    pub(super) fn repeated_bombard_target_remains(&self, unit: &Unit, x: i32, y: i32) -> bool {
        (if unit.owner == HUMAN {
            self.tile(x, y).visible
        } else {
            self.tile(x, y).known_by[unit.owner]
        }) && self.bombard_target_available_for_order(unit, x, y)
    }

    pub(super) fn repeated_precision_bombard_target_remains(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
    ) -> bool {
        (if unit.owner == HUMAN {
            self.tile(x, y).visible
        } else {
            self.tile(x, y).known_by[unit.owner]
        }) && self.can_precision_bomb_tile(unit, x, y)
    }

    pub(crate) fn can_bombard_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.can_begin_bombard(unit)
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
            || !(if unit.owner == HUMAN {
                self.tile(x, y).visible
            } else {
                self.tile(x, y).known_by[unit.owner]
            })
        {
            return false;
        }
        self.bombard_action_available_for_tile(unit, x, y)
    }

    pub(crate) fn bombardable_targets_in_range(&self, id: u32) -> Vec<(i32, i32)> {
        let Some(unit) = self.unit_by_id(id) else {
            return Vec::new();
        };
        if !self.can_begin_bombard(unit) {
            return Vec::new();
        }
        self.map_wrap
            .civ3_radius(unit.x, unit.y, Self::civ3_bombard_range(unit.kind))
            .into_iter()
            .filter_map(|(x, y, _)| self.can_bombard_tile(unit, x, y).then_some((x, y)))
            .collect()
    }

    pub(super) fn start_repeated_bombard(&mut self, id: u32, x: i32, y: i32) -> bool {
        if self.try_ranged_bombard(id, x, y) == MoveResult::Blocked {
            return false;
        }
        let Some(index) = self.unit_index_by_id(id) else {
            return true;
        };
        self.units[index].order = Some(UnitOrder::Bombard { x, y });
        true
    }

    pub(super) fn start_repeated_precision_bombard(&mut self, id: u32, x: i32, y: i32) -> bool {
        if self.try_precision_bombard(id, x, y) == MoveResult::Blocked {
            return false;
        }
        if let Some(index) = self.unit_index_by_id(id) {
            self.units[index].order = Some(UnitOrder::PrecisionBombard { x, y });
        }
        true
    }

    pub(crate) fn precision_bombardable_targets_in_range(&self, id: u32) -> Vec<(i32, i32)> {
        let Some(unit) = self.unit_by_id(id) else {
            return Vec::new();
        };
        if !self.can_precision_bomb(unit) {
            return Vec::new();
        }
        self.map_wrap
            .civ3_radius(unit.x, unit.y, Self::civ3_bombard_range(unit.kind))
            .into_iter()
            .filter_map(|(x, y, _)| self.can_precision_bomb_tile(unit, x, y).then_some((x, y)))
            .collect()
    }

    fn can_precision_bomb_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.can_precision_bomb(unit)
            || (unit.x, unit.y) == (x, y)
            || self.map_wrap.civ3_distance(unit.x, unit.y, x, y)
                > Self::civ3_bombard_range(unit.kind)
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
            || !self.tile(x, y).known_by[unit.owner]
        {
            return false;
        }
        self.city_at(x, y).is_some_and(|city| {
            city.owner != unit.owner
                && self.at_war(unit.owner, city.owner)
                && (city.size > 1
                    || city
                        .buildings
                        .iter()
                        .any(|b| b.data().genus == crate::rules::Genus::Improvement))
        })
    }

    pub(super) fn try_precision_bombard(&mut self, id: u32, x: i32, y: i32) -> MoveResult {
        let Some(attacker) = self.unit_by_id(id).cloned() else {
            return MoveResult::Blocked;
        };
        if attacker.moves_left < 1 || !self.can_precision_bomb_tile(&attacker, x, y) {
            return MoveResult::Blocked;
        }
        self.try_bombard_mode(&attacker, x, y, true)
    }

    pub(super) fn try_ranged_bombard(&mut self, id: u32, x: i32, y: i32) -> MoveResult {
        let Some(attacker) = self.unit_by_id(id).cloned() else {
            return MoveResult::Blocked;
        };
        if !self.can_bombard_tile(&attacker, x, y) {
            return MoveResult::Blocked;
        }
        self.try_bombard(&attacker, x, y)
    }

    /// Freeciv's dai_unit_attack selects Bombard after capture/ransom when
    /// evaluating a reachable attack tile. Civ III bombard ranges exceed the
    /// adjacent C2C3 action range, so scan the unit's legal firing radius
    /// before its normal movement/guard task consumes the turn.
    pub(super) fn run_ai_bombard(&mut self, unit: &Unit) -> bool {
        if !self.players.get(unit.owner).is_some_and(|player| player.ai)
            || !self.can_begin_bombard(unit)
        {
            return false;
        }
        let range = Self::civ3_bombard_range(unit.kind);
        let mut targets: Vec<_> = self
            .map_wrap
            .civ3_radius(unit.x, unit.y, range)
            .into_iter()
            .filter_map(|(x, y, _)| {
                (self.can_bombard_tile(unit, x, y)
                    && !self.ai_bombard_preempted_by_priority_action(unit, x, y))
                .then_some((x, y))
            })
            .collect();
        targets.sort_by(|&(ax, ay), &(bx, by)| {
            self.ai_bombard_tile_value(unit, bx, by)
                .total_cmp(&self.ai_bombard_tile_value(unit, ax, ay))
                .then_with(|| ay.cmp(&by))
                .then_with(|| ax.cmp(&bx))
        });
        let Some((x, y)) = targets.first().copied() else {
            return false;
        };
        if self.ai_bombard_tile_value(unit, x, y) <= 0.0 {
            return false;
        }
        if self.can_precision_bomb(unit)
            && self.city_at(x, y).is_some()
            && self.city_at(x, y).is_some_and(|city| {
                city.buildings
                    .iter()
                    .any(|b| b.data().genus == crate::rules::Genus::Improvement)
            })
        {
            return matches!(
                self.try_precision_bombard(unit.id, x, y),
                MoveResult::Fought
            );
        }
        matches!(self.try_ranged_bombard(unit.id, x, y), MoveResult::Fought)
    }

    /// Move conventional bombarders toward a legal firing position when no
    /// current shot is available. Re-evaluate the visible targets each turn;
    /// this deliberately does not turn an aircraft into a ground-style
    /// artillery unit.
    pub(super) fn run_ai_bombard_approach(&mut self, unit: &Unit) -> bool {
        if !self.players.get(unit.owner).is_some_and(|player| player.ai)
            || unit.kind.fuel_capacity() > 0
            || !self.can_begin_bombard(unit)
        {
            return false;
        }
        let range = Self::civ3_bombard_range(unit.kind);
        let mut target_tiles = std::collections::BTreeSet::new();
        for city in self.cities.iter().filter(|city| {
            city.owner != unit.owner
                && self.at_war(unit.owner, city.owner)
                && self.tile(city.x, city.y).known_by[unit.owner]
        }) {
            target_tiles.insert((city.x, city.y));
        }
        for target in self.units.iter().filter(|target| {
            target.owner != unit.owner
                && self.at_war(unit.owner, target.owner)
                && target.transport.is_none()
                && self.unit_visible_to(unit.owner, target)
        }) {
            target_tiles.insert((target.x, target.y));
        }
        for (index, _) in self.tiles.iter().enumerate().filter(|(_, tile)| {
            tile.owner
                .is_some_and(|owner| owner != unit.owner && self.at_war(unit.owner, owner))
                && tile.known_by[unit.owner]
        }) {
            let (x, y) = (
                index as i32 % self.map_wrap.width(),
                index as i32 / self.map_wrap.width(),
            );
            if !bombardable_terrain_improvements(self, x, y).is_empty() {
                target_tiles.insert((x, y));
            }
        }

        type ScoredFiringPosition = (f64, i32, Vec<(i32, i32)>);
        let mut best: Option<ScoredFiringPosition> = None;
        for (tx, ty) in target_tiles {
            for (px, py, distance) in self.map_wrap.civ3_radius(tx, ty, range) {
                if distance == 0
                    || !self.tile(px, py).known_by[unit.owner]
                    || self
                        .city_at(px, py)
                        .is_some_and(|city| city.owner != unit.owner)
                    || self
                        .tile_units_ordered(px, py)
                        .any(|other| other.id != unit.id && self.at_war(unit.owner, other.owner))
                {
                    continue;
                }
                let mut firing_unit = unit.clone();
                (firing_unit.x, firing_unit.y) = (px, py);
                if !self.can_bombard_tile(&firing_unit, tx, ty) {
                    continue;
                }
                if self.ai_bombard_preempted_by_priority_action(&firing_unit, tx, ty) {
                    continue;
                }
                let value = self.ai_bombard_tile_value(&firing_unit, tx, ty);
                if value <= 0.0 {
                    continue;
                }
                let Some((path, cost)) =
                    self.ai_goto_path(unit, (px, py), super::normal_paths::GotoTask::Attack)
                else {
                    continue;
                };
                if path.len() <= 1 {
                    continue;
                }
                let score = value / (1.0 + f64::from(cost) / f64::from(self.unit_move_rate(unit)));
                if best.as_ref().is_none_or(|(best_score, best_cost, _)| {
                    score > *best_score || (score == *best_score && cost < *best_cost)
                }) {
                    best = Some((score, cost, path));
                }
            }
        }
        best.is_some_and(|(_, _, path)| self.follow_source_ai_path(unit.id, path).moved)
    }

    fn choose_city_bombard_target(
        &mut self,
        city: &City,
        unit_targets: &[Unit],
    ) -> Option<CityBombardTarget> {
        // Prima's PTW combat chapter describes 25% population, 25% buildings,
        // and 50% garrison selection. Conquests confirms those target classes
        // but not the weights, so keep this source-derived split isolated.
        match self.rand_below(100) {
            0..=24 if city.size > 1 => Some(CityBombardTarget::Population(city.id)),
            25..=49 => {
                let buildings: Vec<_> = city
                    .buildings
                    .iter()
                    .copied()
                    .filter(|building| building.data().genus == crate::rules::Genus::Improvement)
                    .collect();
                (!buildings.is_empty()).then(|| {
                    let index = self.rand_below(buildings.len() as i32) as usize;
                    CityBombardTarget::Building(city.id, buildings[index])
                })
            }
            50..=99 => (!unit_targets.is_empty()).then(|| {
                let index = self.rand_below(unit_targets.len() as i32) as usize;
                CityBombardTarget::Unit(unit_targets[index].clone())
            }),
            _ => None,
        }
    }

    fn choose_precision_city_bombard_target(&mut self, city: &City) -> Option<CityBombardTarget> {
        let buildings: Vec<_> = city
            .buildings
            .iter()
            .copied()
            .filter(|building| building.data().genus == crate::rules::Genus::Improvement)
            .collect();
        if buildings.is_empty() {
            return (city.size > 1).then_some(CityBombardTarget::Population(city.id));
        }
        let index = self.rand_below(buildings.len() as i32) as usize;
        Some(CityBombardTarget::Building(city.id, buildings[index]))
    }

    fn ai_bombard_tile_value(&self, attacker: &Unit, x: i32, y: i32) -> f64 {
        let hostile_city = self
            .city_at(x, y)
            .filter(|city| city.owner != attacker.owner && self.at_war(attacker.owner, city.owner));
        let city_unit_weight = if hostile_city.is_some() { 0.5 } else { 1.0 };
        let attack = self.attack_power_against_with_base_for_action(
            attacker,
            x,
            y,
            attacker.kind.bombard_strength(),
            "Bombard",
        );
        let rate = if crate::sim::combat_rules::selected().damage_reduces_bombard_rate
            && attacker.kind.bombard_rate() > 0
        {
            (attacker.kind.bombard_rate() * attacker.hp / attacker.max_hit_points()).max(1)
        } else {
            attacker.kind.bombard_rate()
        };
        let mut value = 0.0;
        for target in self.tile_units_ordered(x, y).filter(|target| {
            target.owner != attacker.owner
                && self.at_war(attacker.owner, target.owner)
                && target.transport.is_none()
                && self.unit_visible_to(attacker.owner, target)
                && self.combat_target_reachable(attacker.kind, target.kind, x, y)
        }) {
            let defense = self.defense_power_against(target, x, y, attacker.kind);
            let firepower = self.combat_firepower(attacker, target).0;
            if attack + defense <= 0 {
                continue;
            }
            let expected_damage = (f64::from(attack) / f64::from(attack + defense)
                * f64::from(rate * firepower))
            .min(f64::from(target.hp));
            let lethal = attacker.kind.has_lethal_bombard_against(target.kind);
            let lethal_bonus = if lethal && expected_damage >= f64::from(target.hp) {
                f64::from(target.kind.stats().cost) * 0.5
            } else {
                0.0
            };
            value += city_unit_weight
                * ((f64::from(target.kind.stats().cost) + 1.0) * expected_damage
                    / f64::from(target.hp.max(1))
                    + lethal_bonus);
        }
        if let Some(city) = hostile_city {
            let improvements = city
                .buildings
                .iter()
                .filter(|building| building.data().genus == crate::rules::Genus::Improvement)
                .count();
            value += 0.25 * f64::from(city.size.max(1)) + 0.25 * improvements as f64;
        }
        if self
            .tile(x, y)
            .owner
            .is_some_and(|owner| owner != attacker.owner && self.at_war(attacker.owner, owner))
        {
            value += bombardable_terrain_improvements(self, x, y).len() as f64;
        }
        value
    }

    pub(super) fn refuel_site(&self, owner: usize, x: i32, y: i32) -> bool {
        self.cities
            .iter()
            .any(|city| (city.x, city.y) == (x, y) && self.allied(owner, city.owner))
            || (matches!(self.tile(x, y).base, Some(Base::Airstrip | Base::Airbase))
                && self.tile(x, y).known_by[owner])
    }

    fn aboard_refueling_transport(&self, unit: &Unit) -> bool {
        unit.transport.is_some_and(|id| {
            self.units.iter().any(|ship| {
                ship.id == id
                    && self.ruleset_transport_pair_allowed_fail_closed(ship.kind, unit.kind)
                    && self.allied(unit.owner, ship.owner)
            })
        })
    }

    pub(super) fn run_source_air_ferry(&mut self, carrier: &Unit) -> (bool, Vec<u32>) {
        let mut done = Vec::new();
        if carrier.moves_left <= 0 {
            return (true, done);
        }
        if carrier.hp < carrier.max_hit_points()
            && self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (carrier.x, carrier.y))
        {
            return (true, done);
        }
        self.refresh_ferry_boss(carrier.id);
        loop {
            let Some(boss) = self
                .ferry_boss(carrier.id)
                .and_then(|id| self.unit_by_id(id))
                .cloned()
            else {
                return (false, done);
            };
            if self
                .map_wrap
                .civ3_distance(carrier.x, carrier.y, boss.x, boss.y)
                > 1
            {
                // Native temporarily abandons a distant boss and falls back
                // to pickup/exploration if no cargo replaces it. The restored
                // reservation must not activate that remote passenger.
                return (false, done);
            }
            // Land passengers retain their ordinary founder/worker/military
            // dispatch; fueled cargo uses the same manager as a land airbase.
            if boss.kind.fuel_capacity() == 0 {
                return (false, done);
            }
            if boss.moves_left > 0 && !self.ai_guard_targets.contains_key(&boss.id) {
                self.run_ai_strike_aircraft(&boss);
            }
            let Some(ship) = self
                .units
                .iter()
                .find(|unit| unit.id == carrier.id)
                .cloned()
            else {
                return (true, done);
            };
            if self
                .units
                .iter()
                .any(|unit| unit.id == boss.id && (unit.x, unit.y) == (ship.x, ship.y))
            {
                // daiferry.c: the boss remains on the boat, so hold station
                // and mark it done for the owner's remaining unit loop.
                done.push(boss.id);
                return (true, done);
            }
            if ship.moves_left <= 0 {
                return (true, done);
            }
            if self.cargo_count(ship.id) == 0 {
                return (false, done);
            }
            self.release_ai_ferry(boss.id);
            self.refresh_ferry_boss(ship.id);
        }
    }

    pub(super) fn try_bombard(&mut self, attacker: &Unit, x: i32, y: i32) -> MoveResult {
        self.try_bombard_mode(attacker, x, y, false)
    }

    fn try_bombard_mode(&mut self, attacker: &Unit, x: i32, y: i32, precision: bool) -> MoveResult {
        let Some(()) = civ3_bombard_unit(attacker.kind) else {
            return MoveResult::Blocked;
        };
        if !(if precision {
            self.can_precision_bomb_tile(attacker, x, y) && attacker.moves_left > 0
        } else {
            self.bombard_action_available_for_tile(attacker, x, y)
        }) {
            return MoveResult::Blocked;
        }
        if !self.try_air_superiority_interceptions(attacker.id, (x, y)) {
            return MoveResult::Fought;
        }
        let Some(attacker) = self.unit_by_id(attacker.id).cloned() else {
            return MoveResult::Fought;
        };
        let mapped_bombard_strength = attacker.kind.bombard_strength();

        let city = self
            .city_at(x, y)
            .filter(|city| city.owner != attacker.owner && self.at_war(attacker.owner, city.owner))
            .cloned();
        let raw_targets: Vec<_> = self
            .tile_units_ordered(x, y)
            .filter(|target| {
                target.owner != attacker.owner
                    && self.at_war(attacker.owner, target.owner)
                    && target.transport.is_none()
                    && !(attacker.kind.fuel_capacity() > 0 && target.kind.fuel_capacity() > 0)
                    && self.unit_visible_to(attacker.owner, target)
                    && self.combat_target_reachable(attacker.kind, target.kind, x, y)
            })
            .cloned()
            .collect();
        let city_action = city.is_some();
        let city_target = city.as_ref().and_then(|city| {
            if precision {
                self.choose_precision_city_bombard_target(city)
            } else {
                self.choose_city_bombard_target(city, &raw_targets)
            }
        });
        let targets: Vec<_> = if city_action {
            match &city_target {
                Some(CityBombardTarget::Unit(target)) => vec![target.clone()],
                _ => Vec::new(),
            }
        } else {
            raw_targets
        };
        let terrain_targets = bombardable_terrain_improvements(self, x, y);
        let has_unit_targets = !targets.is_empty();
        if !has_unit_targets && !city_action && terrain_targets.is_empty() {
            return MoveResult::Blocked;
        }
        // The preserved Civ III unit record stores bombard strength separately
        // from its ordinary attack value (a Bomber has attack 0 and bombard
        // strength 12). Feed that explicit content mapping through the shared
        // selected-ruleset tired-attack/effect pipeline.
        let attack = self.attack_power_against_with_base_for_action(
            &attacker,
            x,
            y,
            mapped_bombard_strength,
            "Bombard",
        );
        let base_rate = attacker.kind.bombard_rate();
        let rate =
            if crate::sim::combat_rules::selected().damage_reduces_bombard_rate && base_rate > 0 {
                (base_rate * attacker.hp / attacker.max_hit_points()).max(1)
            } else {
                base_rate
            };
        let mut total_damage = 0;
        let mut observed_targets = Vec::new();

        // Freeciv's unit_bombard() walks every unit on the target tile and
        // fires one rate of attacks at each reachable defender. ACTION_BOMBARD
        // is marked non-lethal by the pinned engine's ruleset loader.
        for target in targets {
            let defense = self.defense_power_against(&target, x, y, attacker.kind);
            let (attack_firepower, _) = self.combat_firepower(&attacker, &target);
            let mut hp = target.hp;
            let mut hp_steps = vec![(attacker.hp, hp)];
            let lethal = attacker.kind.has_lethal_bombard_against(target.kind);
            for _ in 0..rate {
                if self.rand_below(attack + defense) < attack {
                    hp = if lethal {
                        (hp - attack_firepower).max(0)
                    } else {
                        (hp - attack_firepower).max(1)
                    };
                    hp_steps.push((attacker.hp, hp));
                    if hp == 0 {
                        break;
                    }
                }
            }
            total_damage += target.hp - hp;
            if hp == 0 {
                self.remove_units_with_cargo(vec![target.id]);
            } else if let Some(defender) = self.units.iter_mut().find(|unit| unit.id == target.id) {
                defender.hp = hp;
            }
            if self.visual_events.enabled {
                self.publish_visual_event(VisualEvent::Combat(Box::new(
                    presentation::CombatVisual {
                        attacker: attacker.clone(),
                        defender: target.clone(),
                        hp: hp_steps,
                        city: self.city_at(x, y).cloned(),
                        bombard_effects: attacker.kind.has_bombard_effects(),
                        territory: (0..self.map_wrap.width() * self.map_wrap.height())
                            .map(|i| {
                                self.displayed_tile_owner(
                                    i % self.map_wrap.width(),
                                    i / self.map_wrap.width(),
                                )
                            })
                            .collect(),
                    },
                )));
            }
            observed_targets.push((target.kind, target.owner));
        }

        if let Some(
            target @ (CityBombardTarget::Population(_) | CityBombardTarget::Building(_, _)),
        ) = city_target
        {
            let Some(city) = city.as_ref() else {
                unreachable!("city bombard target requires its city");
            };
            let defense = city_object_bombard_defense(self, city, &attacker);
            for _ in 0..rate {
                if attack + defense > 0 && self.rand_below(attack + defense) < attack {
                    match target {
                        CityBombardTarget::Population(city_id) => {
                            self.reduce_city_population_from_bombard(city_id, attacker.owner);
                        }
                        CityBombardTarget::Building(city_id, building) => {
                            if let Some(city) =
                                self.cities.iter_mut().find(|city| city.id == city_id)
                            {
                                city.buildings.retain(|&current| current != building);
                            }
                            if attacker.owner == HUMAN || city.owner == HUMAN {
                                self.add_event(format!(
                                    "{}'s {} was destroyed by bombardment.",
                                    city.name,
                                    building.data().name
                                ));
                            }
                        }
                        CityBombardTarget::Unit(_) => unreachable!("unit target handled above"),
                    }
                    break;
                }
            }
        }

        let mut terrain_damage = Vec::new();
        if !has_unit_targets && !city_action && !terrain_targets.is_empty() {
            // Prima's PTW guide gives terrain improvements 16 base defense,
            // one hit point, and the tile's applicable terrain defense bonus.
            // Its priority is railroad first, then roads plus
            // mine/irrigation, then other improvements. Conquests confirmation
            // of these exact priorities is still outstanding.
            for _ in 0..rate {
                let current_targets = bombardable_terrain_improvements(self, x, y);
                let defense = terrain_bombard_defense(self, x, y);
                if current_targets.is_empty() || attack + defense <= 0 {
                    break;
                }
                if self.rand_below(attack + defense) < attack {
                    for target in current_targets {
                        bombard_terrain_improvement(self, x, y, target);
                        terrain_damage.push(target);
                    }
                }
            }
        }

        if let Some(bomber) = self.units.iter_mut().find(|unit| unit.id == attacker.id) {
            bomber.moves_left = 0;
            bomber.moved = true;
            bomber.state = UnitState::Idle;
            bomber.work_left = 0;
        }
        if attacker.owner == HUMAN || observed_targets.iter().any(|(_, owner)| *owner == HUMAN) {
            if !terrain_damage.is_empty() {
                let names = terrain_damage
                    .iter()
                    .map(|target| target.name())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.add_event(format!(
                    "A {} destroyed {names} with bombardment.",
                    attacker.kind.name(),
                ));
            } else if observed_targets.is_empty() {
                if let Some(city) = self.city_at(x, y) {
                    self.add_event(format!(
                        "A {} bombarded {}.",
                        attacker.kind.name(),
                        city.name
                    ));
                }
            } else {
                let names = observed_targets
                    .iter()
                    .map(|(kind, _)| kind.name())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.add_event(format!(
                    "A {} dealt {total_damage} damage to {names}.",
                    attacker.kind.name(),
                ));
            }
        }
        MoveResult::Fought
    }

    #[cfg(test)]
    pub(super) fn restore_aircraft(&mut self) {
        for owner in 0..self.players.len() {
            let ids: Vec<_> = self
                .player_units_ordered(owner)
                .filter(|unit| unit.kind.is_aircraft())
                .map(|unit| unit.id)
                .collect();
            for id in ids {
                self.restore_aircraft_unit(id);
            }
            self.finish_aircraft_fuel_deaths(owner);
        }
    }

    pub(super) fn finish_aircraft_fuel_deaths(&mut self, owner: usize) {
        let dead: Vec<_> = self
            .player_units_ordered(owner)
            .filter(|unit| unit.kind.fuel_capacity() > 0 && unit.fuel == 0)
            .map(|unit| (unit.id, unit.kind))
            .collect();
        for (id, kind) in dead {
            if !self.units.iter().any(|unit| unit.id == id) {
                continue;
            }
            self.remove_units_with_cargo(vec![id]);
            if owner == HUMAN {
                self.add_event(format!("Your {} crashed.", kind.name()));
            }
        }
    }

    pub(super) fn restore_aircraft_unit(&mut self, id: u32) {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return;
        };
        let max_fuel = unit.kind.fuel_capacity();
        let site = self.refuel_site(unit.owner, unit.x, unit.y);
        let refueled = site || self.aboard_refueling_transport(&unit);
        // player_restore_units calls unit_gain_hitpoints before refueling.
        // The same ruleset effects now drive aircraft and other unit recovery.
        let hp = unit.hp + self.unit_hitpoint_gain_at(&unit, unit.x, unit.y);
        if let Some(live) = self.units.iter_mut().find(|actor| actor.id == id) {
            live.hp = hp;
            live.moved = false;
            live.paradropped = false;
            live.air_superiority_shot = false;
            if refueled {
                live.fuel = max_fuel;
            }
        }
        if hp <= 0 {
            self.remove_units_with_cargo(vec![id]);
            if unit.owner == HUMAN {
                self.add_event(format!("Your {} crashed.", unit.kind.name()));
            }
            return;
        }
        if !refueled
            && max_fuel > 0
            && unit.fuel <= 1
            && self.boarding_target(&unit, unit.x, unit.y).is_some()
        {
            self.board_or_deboard(id);
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .fuel = max_fuel;
            return;
        }
        if !refueled
            && max_fuel > 0
            && unit.fuel <= 1
            && unit.moves_left > 0
            && let Some(mut map) =
                super::fuel_paths::FuelPaths::new(self, &unit, self.players[unit.owner].ai)
        {
            let destination = map
                .reachable_within(self, unit.moves_left, false, true)
                .into_iter()
                .find(|&(x, y, _)| {
                    !self
                        .tile_units_ordered(x, y)
                        .any(|other| !self.allied(unit.owner, other.owner))
                        && (self.refuel_site(unit.owner, x, y)
                            || self.boarding_target(&unit, x, y).is_some())
                });
            if let Some((x, y, _)) = destination
                && let Some(path) = map.path(self, x, y)
            {
                self.follow_source_ai_path(id, path);
                if let Some(live) = self.unit_by_id(id).cloned() {
                    if (live.x, live.y) == (x, y)
                        && self.boarding_target(&live, x, y).is_some()
                        && !self.refuel_site(live.owner, x, y)
                    {
                        self.board_or_deboard(id);
                    }
                    if let Some(actor) = self.units.iter_mut().find(|actor| actor.id == id) {
                        actor.state = UnitState::Idle;
                    }
                }
            }
        }

        let Some(index) = self.unit_index_by_id(id) else {
            return;
        };
        let unit = &self.units[index];
        let refueled =
            self.refuel_site(unit.owner, unit.x, unit.y) || self.aboard_refueling_transport(unit);
        let unit = &mut self.units[index];
        unit.fuel = if refueled {
            max_fuel
        } else {
            unit.fuel.saturating_sub(1)
        };
    }
}

#[cfg(test)]
#[path = "aviation/tests.rs"]
mod tests;
