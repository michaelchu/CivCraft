//! Unit action, selection, and context projections.

use super::*;

impl Simulation {
    pub(crate) fn unit_action_view(&self, id: u32) -> Option<UnitActionView> {
        let unit = self.unit_by_id(id)?;
        let owner_gold = self.players[unit.owner].gold;
        let stats = unit.kind.stats();
        let tile = self.tile(unit.x, unit.y);
        let work_left = unit.work_left;
        let cities = self
            .cities
            .iter()
            .map(|city| UnitCityActionView {
                id: city.id,
                name: city.name.clone(),
                can_airlift: self.can_airlift(unit, city),
                can_join: self.can_join_city(unit, city),
                can_help_wonder: self.can_help_wonder(unit, city)
                    && self.tile(city.x, city.y).visible,
            })
            .collect();
        let upgrade = self
            .paid_upgrade(unit)
            .map(|(target, price)| (target, price, owner_gold >= price));
        let road_action = if stats.can_work && unit.transport.is_none() {
            if tile.road && self.has_tech(unit.owner, Tech::STEAM_POWER) {
                Some((
                    "Build railroad [R]".to_owned(),
                    self.can_railroad_at(unit.owner, unit.x, unit.y),
                ))
            } else {
                Some((
                    "Build road [R]".to_owned(),
                    !tile.road
                        && tile.terrain.road_time() > 0
                        && (!tile.river || self.has_tech(unit.owner, Tech::ENGINEERING)),
                ))
            }
        } else {
            None
        };
        let irrigation_action =
            if stats.can_work && unit.transport.is_none() && !unit.kind.is_naval() {
                if tile.irrigation {
                    Some((
                        "Build farmland [I]".to_owned(),
                        self.can_farm_at(unit.owner, unit.x, unit.y),
                    ))
                } else {
                    Some((
                        "Irrigate [I]".to_owned(),
                        self.can_irrigate_at(unit.owner, unit.x, unit.y),
                    ))
                }
            } else {
                None
            };
        let mine_action = if let Some(kind) = self.oil_work(unit) {
            Some((format!("Build {} [M]", kind.name()), work_left == 0))
        } else if stats.can_work && unit.transport.is_none() && !unit.kind.is_naval() {
            Some((
                "Mine [M]".to_owned(),
                !tile.mine && tile.terrain.mine_time() > 0,
            ))
        } else {
            None
        };
        let terrain_work = [WorkKind::Cultivate, WorkKind::Plant]
            .into_iter()
            .filter(|&kind| self.terrain_work(unit, kind).is_some())
            .collect();
        let clean_targets = if unit.kind.can_clean() && unit.transport.is_none() {
            [
                Contamination::Pollution,
                Contamination::Fallout,
                Contamination::Craters,
            ]
            .into_iter()
            .filter(|&target| tile.has_contamination(target))
            .map(|target| (target, target.name().to_owned()))
            .collect()
        } else {
            Vec::new()
        };
        let base_work = [Base::Fort, Base::Fortress, Base::Airstrip, Base::Airbase]
            .into_iter()
            .filter(|&base| self.base_work_time(unit, base).is_some())
            .map(|base| (base, base.name().to_owned()))
            .collect();
        let base_target = tile.base.map(PillageTarget::Base);
        let pillage_targets = PillageTarget::ALL
            .into_iter()
            .chain(base_target)
            .filter(|&target| self.can_pillage(unit, target))
            .map(|target| (target, target.name().to_owned()))
            .collect();
        let disband_cities = self
            .cities
            .iter()
            .filter(|city| self.can_disband(unit, Some(city)))
            .map(|city| (city.id, city.name.clone()))
            .collect();
        Some(UnitActionView {
            unit_name: unit.kind.name(),
            owner: unit.owner,
            x: unit.x,
            y: unit.y,
            moves_left: unit.moves_left,
            work_left,
            upgrade,
            air_superiority: self.can_assign_air_superiority(unit),
            rebase: self.can_begin_rebase(unit),
            paradrop: self.can_begin_paradrop(unit),
            bombard: self.can_begin_bombard(unit),
            cities,
            nuclear_weapon: unit.kind.is_nuclear_weapon(),
            nuclear_strike: self.can_begin_nuclear_strike(unit),
            transport_action: self.transport_action(unit).map(str::to_owned),
            found_city: (stats.can_found && unit.transport.is_none())
                .then(|| self.can_found_at(unit.x, unit.y)),
            road_action,
            road_work: if tile.road {
                WorkKind::Railroad
            } else {
                WorkKind::Road
            },
            irrigation_action,
            irrigation_work: if tile.irrigation {
                WorkKind::Farmland
            } else {
                WorkKind::Irrigation
            },
            mine_action,
            mine_work: self.oil_work(unit).unwrap_or(WorkKind::Mine),
            terrain_work,
            clean_targets,
            base_work,
            pillage_targets,
            fortify: unit.kind.is_military() && unit.transport.is_none(),
            set_home_city: self.home_city_target(unit).is_some(),
            automate: unit.automated
                || (unit.kind.fuel_capacity() == 0 && unit.kind != UnitKind::Helicopter),
            can_disband: self.can_disband(unit, None),
            disband_cities,
        })
    }

    pub(crate) fn human_city_at_view(&self, x: i32, y: i32) -> Option<u32> {
        self.cities
            .iter()
            .find(|city| city.owner == HUMAN && (city.x, city.y) == (x, y))
            .map(|city| city.id)
    }

    pub(crate) fn human_units_at_view(&self, x: i32, y: i32) -> Vec<SelectableUnitView> {
        let mut units = self
            .units
            .iter()
            .rev()
            .filter(|unit| unit.owner == HUMAN && (unit.x, unit.y) == (x, y))
            .map(|unit| SelectableUnitView {
                id: unit.id,
                transported: unit.transport.is_some(),
            })
            .collect::<Vec<_>>();
        units.sort_by_key(|unit| unit.transported);
        units
    }

    pub(crate) fn selectable_units_view(&self) -> Vec<SelectableUnitView> {
        self.units
            .iter()
            .filter(|unit| {
                unit.owner == HUMAN
                    && unit.moves_left > 0
                    && unit.work_left == 0
                    && unit.state != UnitState::Sentry
            })
            .map(|unit| SelectableUnitView {
                id: unit.id,
                transported: unit.transport.is_some(),
            })
            .collect()
    }

    pub(crate) fn unit_position_view(&self, id: u32) -> Option<(i32, i32)> {
        self.unit_by_id(id).map(|unit| (unit.x, unit.y))
    }

    pub(crate) fn newest_city_id_view(&self) -> Option<u32> {
        self.cities.last().map(|city| city.id)
    }

    pub(crate) fn unit_fidget_view(&self, id: u32) -> Option<UnitFidgetView> {
        let unit = self.unit_by_id(id)?;
        Some(UnitFidgetView {
            id: unit.id,
            kind: unit.kind,
            era_index: self.research_era(unit.owner).index(),
            state: unit.state,
            working: unit.work_kind.is_some(),
        })
    }

    /// Projects a unit and adjacent campaign state for the context card.
    /// `unit` may be the presentation playback snapshot for an active animation.
    pub(crate) fn unit_context_view(&self, unit: &Unit) -> UnitContextView {
        let status = if unit.work_left > 0 {
            format!(
                "Building {} - {} turns",
                unit.work_kind
                    .map(|work| work.name())
                    .unwrap_or("improvement"),
                self.project_turns_left(unit)
            )
        } else if let Some(carrier) = unit.transport {
            self.units
                .iter()
                .find(|ship| ship.id == carrier)
                .map_or_else(
                    || "Aboard transport".to_owned(),
                    |ship| format!("Aboard {}", ship.kind.name()),
                )
        } else if unit.kind.transport_capacity() > 0 {
            format!(
                "Cargo {}/{}",
                self.cargo_count(unit.id),
                unit.kind.transport_capacity()
            )
        } else if unit.automated {
            "Automated".to_owned()
        } else {
            match unit.state {
                UnitState::Idle => "Ready".to_owned(),
                UnitState::Fortifying => "Fortifying".to_owned(),
                UnitState::Fortified => "Fortified (+50% defense)".to_owned(),
                UnitState::Sentry => "Sentry".to_owned(),
                UnitState::AirSuperiority => "Air Superiority".to_owned(),
            }
        };
        let status = if unit.kind.is_aircraft() {
            format!(
                "{status} · Fuel {}/{}",
                unit.fuel,
                unit.kind.fuel_capacity()
            )
        } else {
            status
        };
        let status = if unit.veteran > 0 {
            format!("{status} · {}", unit.rank_name())
        } else {
            status
        };
        let player = &self.players[unit.owner];
        let stats = unit.kind.stats();
        UnitContextView {
            kind: unit.kind,
            owner: unit.owner,
            civilization_id: player.civilization_id.clone(),
            era_index: self.research_era(unit.owner).index(),
            owner_name: player.name.clone(),
            terrain_name: self.tile(unit.x, unit.y).terrain.name(),
            status,
            hp: unit.hp,
            max_hp: unit.max_hit_points(),
            moves_left: unit.moves_left,
            moves_allowance: self.unit_move_allowance(unit.owner, unit.kind),
            attack: stats.attack,
            defense: stats.defense,
        }
    }

    pub(crate) fn unit_context_view_for_rendered(
        &self,
        unit: &RenderedUnitView,
    ) -> Option<UnitContextView> {
        unit.context.clone()
    }

    pub(crate) fn unit_context_view_by_id(&self, id: u32) -> Option<UnitContextView> {
        self.unit_by_id(id).map(|unit| self.unit_context_view(unit))
    }
}
