//! Unit movement, routes, activities, combat entry, and transport interactions.
//!
//! This module is the shared rules layer for direct moves and automated goto
//! execution. It validates map adjacency and terrain/ownership constraints,
//! spends movement points, updates unit positions, and invokes combat or
//! boarding logic when a step encounters another object. Child modules isolate
//! activity progress, combat consequences, path execution, and cargo handling.
//!
//! Movement costs and available movement must stay consistent with pathfinding
//! and UI previews. Turn phase ordering can restore or consume movement at
//! different points, so avoid treating `moves_left` as a simple per-frame cap.

use super::movement_rules::{MovementRules, MovementTile};
use super::*;
use crate::rules::freeciv::{RulesetData, effect_value};

mod activities;
mod combat;
mod routes;
mod sentry;
mod transports;
use std::{cmp::Reverse, collections::BinaryHeap, sync::OnceLock};

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
    pub(super) fn transfer_unit(&mut self, victim: &Unit, owner: usize, home: Option<u32>) -> u32 {
        self.refresh_diplomatic_contact_at(owner, victim.x, victim.y);
        self.units.retain(|unit| unit.id != victim.id);
        self.discard_unit_ai(victim.id);
        self.spawn_unit(owner, victim.kind, victim.x, victim.y);
        let gained = self.units.last_mut().unwrap();
        gained.hp = victim.hp;
        gained.nationality = victim.nationality;
        gained.fuel = victim.fuel;
        gained.veteran = victim.veteran;
        gained.c2c3_veteran = victim.c2c3_veteran;
        gained.paradropped = victim.paradropped;
        gained.moves_left = victim.moves_left;
        gained.home_city = home;
        gained.home_order = gained.id;
        gained.id
    }

    pub(crate) fn unit_move_rate(&self, unit: &Unit) -> i32 {
        let movement_rules = super::movement_rules::MovementRules::campaign();
        let fragments = movement_rules
            .move_fragments()
            .expect("campaign C2C3 movement fragment setting");
        let class = movement_rules
            .unit_class_movement_data_for_kind(unit.kind)
            .expect("campaign C2C3 unit-class movement data");
        let base = unit.kind.stats().moves * fragments + unit.veteran_move_bonus();
        let mut rate = if class.damage_slows {
            base * unit.hp / unit.max_hit_points()
        } else {
            base
        };
        let ruleset = RulesetData::campaign_default();
        let context = super::action_rules::SimulationRequirementContext::for_unit(self, unit);
        let effect_bonus = effect_value(&ruleset.effects, "Move_Bonus", &context)
            .expect("campaign C2C3 Move_Bonus requirements are supported");
        rate += i32::try_from(effect_bonus).expect("campaign C2C3 movement bonus fits in i32")
            * fragments;
        let minimum = (class.min_speed * fragments).min(base);
        rate.max(minimum)
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
        let rules = MovementRules::campaign();
        let class = rules
            .unit_class_for_kind(kind)
            .expect("campaign C2C3 unit class binding");
        let fragments = rules
            .move_fragments()
            .expect("campaign C2C3 movement fragment setting");
        if kind.is_aircraft() {
            return Some(fragments);
        }
        let destination_known = dest.is_some();
        let destination = dest.unwrap_or(RouteTile {
            terrain: if kind.is_naval() {
                Terrain::Ocean
            } else {
                Terrain::Mountains
            },
            road: false,
            railroad: false,
            river: false,
            city_owner: None,
        });
        let source = self.observed_route_tile(unit.x, unit.y);
        let to_movement_tile = |tile: RouteTile, x: i32, y: i32, known: bool| {
            MovementTile::new(
                tile.terrain,
                tile.road,
                tile.railroad,
                tile.river,
                known.then(|| self.render_tile(x, y).base).flatten(),
            )
        };
        let from = source.map_or_else(
            || MovementTile::new(Terrain::Mountains, false, false, false, None),
            |tile| to_movement_tile(tile, unit.x, unit.y, true),
        );
        let to = to_movement_tile(destination, x, y, destination_known);
        let destination_native = rules
            .movement_tile_native_to_class(
                destination.terrain,
                destination.road,
                destination.railroad,
                destination.river,
                destination_known
                    .then(|| self.render_tile(x, y).base)
                    .flatten(),
                class,
            )
            .expect("campaign C2C3 movement native_to data");
        if destination_known
            && destination.city_owner.is_none()
            && super::action_rules::freeciv_unit_type_flag(kind, "CoastStrict")
                .expect("campaign C2C3 unit-type CoastStrict binding")
            && !self.safe_ocean_at(x, y, Some(HUMAN))
        {
            return None;
        }
        if destination_known && kind.is_naval() {
            let native = destination_native;
            let port = destination
                .city_owner
                .is_some_and(|owner| self.allied(owner, unit.owner))
                && self.map_wrap.civ3_neighbors(x, y).any(|(nx, ny)| {
                    self.observed_route_tile(nx, ny).is_none_or(|tile| {
                        rules
                            .movement_tile_native_to_class(
                                tile.terrain,
                                tile.road,
                                tile.railroad,
                                tile.river,
                                self.render_tile(nx, ny).base,
                                class,
                            )
                            .expect("campaign C2C3 movement native_to data")
                    })
                });
            if !native && !port {
                return None;
            }
        } else if destination_known {
            let city_access = destination.city_owner.is_some_and(|owner| {
                self.allied(owner, unit.owner)
                    || self.at_war(owner, unit.owner)
                        && rules
                            .unit_class_movement_data_for_kind(kind)
                            .expect("campaign C2C3 unit class data")
                            .can_occupy_city
            });
            if !destination_native && !city_access {
                return None;
            }
        }
        let ignores_terrain = super::action_rules::freeciv_unit_type_flag(kind, "IgTer")
            .expect("campaign C2C3 unit-type IgTer binding");
        let restricted = [(unit.x, unit.y), (x, y)].into_iter().any(|(tx, ty)| {
            self.observed_route_tile(tx, ty).is_some()
                && self
                    .render_tile(tx, ty)
                    .owner
                    .is_some_and(|owner| self.at_war(unit.owner, owner))
        });
        Some(
            rules
                .step_cost(
                    from,
                    to,
                    class,
                    self.movement_is_cardinal((unit.x, unit.y), (x, y)),
                    restricted,
                    ignores_terrain,
                )
                .expect("campaign C2C3 step movement data"),
        )
    }

    pub(super) fn movement_cost_for(&self, unit: &Unit, nx: i32, ny: i32) -> Option<i32> {
        let rules = MovementRules::campaign();
        let fragments = rules
            .move_fragments()
            .expect("campaign C2C3 movement fragment setting");
        if unit.kind.is_aircraft() {
            return Some(fragments);
        }
        let dest = self.tile(nx, ny);
        let source = self.tile(unit.x, unit.y);
        let class = rules
            .unit_class_for_kind(unit.kind)
            .expect("campaign C2C3 unit class binding");
        let source_movement = MovementTile::new(
            source.terrain,
            source.road,
            source.railroad,
            source.river,
            source.base,
        );
        let destination_movement = MovementTile::new(
            dest.terrain,
            dest.road,
            dest.railroad,
            dest.river,
            dest.base,
        );
        let destination_native = rules
            .movement_tile_native_to_class(
                dest.terrain,
                dest.road,
                dest.railroad,
                dest.river,
                dest.base,
                class,
            )
            .expect("campaign C2C3 movement native_to data");
        if self.city_at(nx, ny).is_none()
            && super::action_rules::freeciv_unit_type_flag(unit.kind, "CoastStrict")
                .expect("campaign C2C3 unit-type CoastStrict binding")
            && !self.safe_ocean_at(nx, ny, None)
        {
            return None;
        }
        if unit.kind.is_naval() {
            return (destination_native
                || self.cities.iter().any(|city| {
                    (city.x, city.y) == (nx, ny)
                        && self.allied(city.owner, unit.owner)
                        && self.naval_city_access(city, unit.kind)
                }))
            .then(|| self.ruleset_step_cost(unit, source_movement, destination_movement, nx, ny));
        }
        let city_access = self.cities.iter().any(|city| {
            (city.x, city.y) == (nx, ny)
                && (self.allied(city.owner, unit.owner)
                    || self.at_war(city.owner, unit.owner)
                        && rules
                            .unit_class_movement_data_for_kind(unit.kind)
                            .expect("campaign C2C3 unit class data")
                            .can_occupy_city)
        });
        (destination_native || city_access)
            .then(|| self.ruleset_step_cost(unit, source_movement, destination_movement, nx, ny))
    }

    fn ruleset_step_cost(
        &self,
        unit: &Unit,
        source: MovementTile,
        destination: MovementTile,
        nx: i32,
        ny: i32,
    ) -> i32 {
        let rules = MovementRules::campaign();
        let class = rules
            .unit_class_for_kind(unit.kind)
            .expect("campaign C2C3 unit class binding");
        let ignores_terrain = super::action_rules::freeciv_unit_type_flag(unit.kind, "IgTer")
            .expect("campaign C2C3 unit-type IgTer binding");
        let restricted = [self.tile(unit.x, unit.y).owner, self.tile(nx, ny).owner]
            .into_iter()
            .flatten()
            .any(|owner| self.at_war(unit.owner, owner));
        rules
            .step_cost(
                source,
                destination,
                class,
                self.movement_is_cardinal((unit.x, unit.y), (nx, ny)),
                restricted,
                ignores_terrain,
            )
            .expect("campaign C2C3 step movement data")
    }

    pub(super) fn movement_is_cardinal(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        super::topology::CIV3_DIRS
            .iter()
            .enumerate()
            .any(|(direction, _)| {
                !matches!(direction, 2 | 6)
                    && self.map_wrap.civ3_neighbor(from.0, from.1, direction) == Some(to)
            })
    }

    /// Freeciv `is_safe_ocean`: an adjacent known tile must not carry the
    /// selected C2C3 UnsafeCoast terrain flag. `observer=None` uses live map
    /// state; observer-scoped checks use that player's stored observation.
    pub(super) fn safe_ocean_at(&self, x: i32, y: i32, observer: Option<usize>) -> bool {
        let rules = MovementRules::campaign();
        self.map_wrap
            .civ3_neighbors(x, y)
            .filter(|&(nx, ny)| self.movement_is_cardinal((x, y), (nx, ny)))
            .any(|(nx, ny)| {
                let index = self.map_wrap.index(nx, ny);
                let route = match observer {
                    None => Some(self.live_route_tile(nx, ny)),
                    Some(HUMAN) => self.observed_route_tile(nx, ny),
                    Some(owner) => self
                        .ai_map_memory
                        .get(&(owner, index))
                        .map(|observation| observation.route),
                };
                route.is_some_and(|tile| {
                    match rules.terrain_has_flag(
                        tile.terrain,
                        super::movement_rules::TerrainFlag::UnsafeCoast,
                    ) {
                        Ok(unsafe_coast) => !unsafe_coast,
                        Err(super::movement_rules::MovementRulesError::UnsupportedTerrain(_)) => {
                            false
                        }
                        Err(error) => panic!("invalid selected C2C3 coast data: {error:?}"),
                    }
                })
            })
    }

    fn class_can_occupy_city(&self, kind: UnitKind) -> bool {
        MovementRules::campaign()
            .unit_class_movement_data_for_kind(kind)
            .expect("campaign C2C3 unit class data")
            .can_occupy_city
    }

    pub(super) fn under_foreign_control(
        &self,
        owner: usize,
        x: i32,
        y: i32,
        observed: bool,
    ) -> bool {
        self.enemy_zoc_controls_tile(owner, x, y, observed)
    }

    fn enemy_zoc_controls_tile(&self, owner: usize, x: i32, y: i32, observed: bool) -> bool {
        std::iter::once((x, y))
            .chain(self.map_wrap.civ3_neighbors(x, y))
            .any(|(tx, ty)| {
                let tile = if observed {
                    self.observed_route_tile(tx, ty)
                } else {
                    Some(self.live_route_tile(tx, ty))
                };
                let Some(tile) = tile else {
                    // The client ignores unknown terrain and units under fog.
                    return false;
                };
                if Self::terrain_has_no_zoc(tile.terrain) {
                    return false;
                }

                if tile
                    .city_owner
                    .is_some_and(|city_owner| !self.allied(owner, city_owner))
                {
                    if observed {
                        // Freeciv exposes an occupied bit for visible enemy
                        // cities and assumes fogged cities are occupied. The
                        // app does not persist that bit, so recover only the
                        // boolean from a live stack when the city is visible.
                        return !self.tile(tx, ty).visible
                            || self.units.iter().any(|unit| (unit.x, unit.y) == (tx, ty));
                    }
                    return self.units.iter().any(|unit| (unit.x, unit.y) == (tx, ty));
                }

                // Freeciv can also source ZOC from extras and their owners.
                // CivCraft's tile model has no generic extra-owner or ZOC
                // extra collection, so those ruleset sources are not yet
                // representable here.
                self.units.iter().any(|unit| {
                    (unit.x, unit.y) == (tx, ty)
                        && (!observed || self.unit_visible_to_human(unit))
                        && !self.allied(owner, unit.owner)
                        && unit.transport.is_none()
                        && Self::unit_class_uses_zoc(unit.kind)
                        && !super::action_rules::freeciv_unit_type_flag(unit.kind, "HasNoZOC")
                            .expect("campaign C2C3 unit-type HasNoZOC binding")
                })
            })
    }

    fn terrain_has_no_zoc(terrain: Terrain) -> bool {
        match super::movement_rules::MovementRules::campaign()
            .terrain_has_flag(terrain, super::movement_rules::TerrainFlag::NoZoc)
        {
            Ok(flag) => flag,
            // Volcano has no C2C3 terrain record. Its movement-cost mapping
            // does not imply any other terrain flags.
            Err(super::movement_rules::MovementRulesError::UnsupportedTerrain(_)) => false,
            Err(error) => panic!("campaign C2C3 terrain NoZoc data is invalid: {error:?}"),
        }
    }

    fn unit_class_uses_zoc(kind: UnitKind) -> bool {
        use crate::sim::unit_catalog::UnitClass as C;

        // Keep the ruleset flags authoritative and cache them because route
        // expansion asks this question for every candidate step. Civ III's
        // runtime catalog has no separate Merchant class crosswalk.
        static CLASS_ZOC: OnceLock<[bool; 8]> = OnceLock::new();
        let class_zoc = CLASS_ZOC.get_or_init(|| {
            let rules = super::movement_rules::MovementRules::campaign();
            [
                C::Land,
                C::SmallLand,
                C::BigLand,
                C::Sea,
                C::Trireme,
                C::Air,
                C::Helicopter,
                C::Missile,
            ]
            .map(|class| {
                rules
                    .unit_class_movement_data(class)
                    .expect("C2C3 defines the mapped unit classes")
                    .zoc
            })
        });
        let class_index = match super::movement_rules::MovementRules::campaign()
            .unit_class_for_kind(kind)
            .expect("campaign C2C3 unit class binding")
        {
            C::Land => 0,
            C::SmallLand => 1,
            C::BigLand => 2,
            C::Sea => 3,
            C::Trireme => 4,
            C::Air => 5,
            C::Helicopter => 6,
            C::Missile => 7,
        };
        class_zoc[class_index]
    }

    fn allied_unit_at(&self, owner: usize, x: i32, y: i32, observed: bool) -> bool {
        self.units.iter().any(|other| {
            (other.x, other.y) == (x, y)
                && (!observed || self.unit_visible_to_human(other))
                && self.allied(owner, other.owner)
        })
    }

    fn zone_blocks_step_with_observation(
        &self,
        unit: &Unit,
        from: (i32, i32),
        to: (i32, i32),
        observed: bool,
    ) -> bool {
        if !Self::unit_class_uses_zoc(unit.kind)
            || super::action_rules::freeciv_unit_type_flag(unit.kind, "IgZOC")
                .expect("campaign C2C3 unit-type IgZOC binding")
        {
            return false;
        }
        if self.allied_unit_at(unit.owner, to.0, to.1, observed) {
            return false;
        }

        let route_tile = |position: (i32, i32)| {
            if observed {
                self.observed_route_tile(position.0, position.1)
            } else {
                Some(self.live_route_tile(position.0, position.1))
            }
        };
        let source = route_tile(from);
        let destination = route_tile(to);
        if source.is_some_and(|tile| tile.city_owner.is_some())
            || destination.is_some_and(|tile| tile.city_owner.is_some())
            || source.is_some_and(|tile| Self::terrain_has_no_zoc(tile.terrain))
            || destination.is_some_and(|tile| Self::terrain_has_no_zoc(tile.terrain))
        {
            return false;
        }

        self.enemy_zoc_controls_tile(unit.owner, from.0, from.1, observed)
            && self.enemy_zoc_controls_tile(unit.owner, to.0, to.1, observed)
    }

    pub(super) fn zone_blocks_step(&self, unit: &Unit, from: (i32, i32), to: (i32, i32)) -> bool {
        self.zone_blocks_step_with_observation(unit, from, to, false)
    }

    pub(super) fn zone_blocks_route(
        &self,
        unit: &Unit,
        from: (i32, i32),
        to: (i32, i32),
        observed: bool,
    ) -> bool {
        self.zone_blocks_step_with_observation(unit, from, to, observed)
    }

    fn try_capture_units(&mut self, actor: &Unit, x: i32, y: i32) -> bool {
        if !self.can_capture_units_at(actor, x, y) {
            return false;
        }
        let targets: Vec<_> = self
            .units
            .iter()
            .filter(|unit| (unit.x, unit.y) == (x, y))
            .cloned()
            .collect();
        // Freeciv validates every unit in the stack before transferring any
        // of them. In particular, Unique units cannot duplicate an existing
        // unit owned by the capturer or another Unique unit in the stack.
        if !self.capture_units_unique_preflight(actor.owner, &targets) {
            return false;
        }
        let target_ids: Vec<_> = targets.iter().map(|unit| unit.id).collect();
        let home = if crate::sim::combat_rules::selected().homecaughtunits {
            actor.home_city.filter(|&id| {
                self.cities
                    .iter()
                    .any(|city| city.id == id && city.owner == actor.owner)
            })
        } else {
            None
        };
        let mut captured_count = 0;
        for (index, listed_target) in targets.iter().enumerate() {
            let Some(target) = self.unit_by_id(listed_target.id).cloned() else {
                continue;
            };
            // The source action skips units that already belong to the actor.
            if target.owner == actor.owner {
                continue;
            }
            let captured_id = self.transfer_unit(&target, actor.owner, home);
            if let Some(captured) = self.unit_by_id(captured_id).cloned() {
                captured_count += 1;
                self.publish_visual_event(VisualEvent::Capture { unit: captured });
                self.bounce_captured_unit_if_needed(captured_id, &target_ids[index + 1..]);
            }
        }
        if actor.owner == HUMAN || targets.iter().any(|unit| unit.owner == HUMAN) {
            self.add_event(format!(
                "{} captured {} units.",
                actor.kind.name(),
                captured_count
            ));
        }
        // Action_Success_Actor_Move_Cost is expressed in movement fragments
        // and selected C2C3 charges six fragments for Capture Units.
        self.spend_action_success_move_cost(actor, "Capture Units");
        self.refresh_visibility();
        self.check_game_over();
        true
    }

    fn spend_action_success_move_cost(&mut self, actor: &Unit, action: &str) -> i32 {
        let cost = effect_value(
            &RulesetData::campaign_default().effects,
            "Action_Success_Actor_Move_Cost",
            &super::action_rules::SimulationRequirementContext::for_unit(self, actor)
                .with_action(action),
        )
        .expect("campaign C2C3 action movement-cost requirements are supported");
        let cost = i32::try_from(cost).expect("campaign C2C3 action movement cost fits in i32");
        if let Some(live_actor) = self.units.iter_mut().find(|unit| unit.id == actor.id) {
            live_actor.moves_left = (live_actor.moves_left - cost).max(0);
        }
        cost
    }

    fn capture_units_unique_preflight(&self, owner: usize, targets: &[Unit]) -> bool {
        let mut unique_types = Vec::new();
        for target in targets {
            if !super::action_rules::freeciv_unit_type_flag(target.kind, "Unique")
                .expect("campaign C2C3 unit-type Unique binding")
            {
                continue;
            }
            if self
                .units
                .iter()
                .any(|unit| unit.owner == owner && unit.kind == target.kind)
                || unique_types.contains(&target.kind)
            {
                return false;
            }
            unique_types.push(target.kind);
        }
        true
    }

    fn bounce_captured_unit_if_needed(&mut self, id: u32, remaining_capture_ids: &[u32]) {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return;
        };
        let city_tile = self.city_at(unit.x, unit.y).is_some();
        let hostile_city = self.cities.iter().any(|city| {
            (city.x, city.y) == (unit.x, unit.y) && !self.allied(unit.owner, city.owner)
        });
        let hostile_unlisted_unit = self.units.iter().any(|other| {
            other.id != id
                && (other.x, other.y) == (unit.x, unit.y)
                && !self.allied(unit.owner, other.owner)
                && !remaining_capture_ids.contains(&other.id)
        });
        if city_tile || hostile_city || hostile_unlisted_unit {
            self.bounce_unit_freeciv(id);
        }
    }

    pub(super) fn bounce_unit_freeciv(&mut self, id: u32) {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return;
        };
        let mut destinations = Vec::new();
        for (x, y) in self.map_wrap.freeciv_square_radius(unit.x, unit.y, 2) {
            if (x, y) == (unit.x, unit.y) {
                continue;
            }
            if self.unit_can_survive_at_tile(&unit, x, y)
                && self
                    .cities
                    .iter()
                    .all(|city| (city.x, city.y) != (x, y) || self.allied(unit.owner, city.owner))
                && self.units.iter().all(|other| {
                    (other.x, other.y) != (x, y) || !self.unit_is_nonallied_to(other, unit.owner)
                })
            {
                destinations.push((x, y));
            }
        }
        if destinations.is_empty() {
            let cargo: Vec<_> = self
                .units
                .iter()
                .filter(|other| other.transport == Some(id))
                .map(|other| other.id)
                .collect();
            for passenger in cargo {
                if self.unit_by_id(passenger).is_some() {
                    self.bounce_unit_freeciv(passenger);
                }
            }
            self.remove_units_with_cargo(vec![id]);
            return;
        }
        let (x, y) = destinations[self.rand_below(destinations.len() as i32) as usize];
        if self
            .unit_by_id(id)
            .is_some_and(|unit| unit.transport.is_some())
            && let Some(index) = self.unit_index_by_id(id)
        {
            self.set_transport(index, None);
        }
        if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) {
            (unit.x, unit.y) = (x, y);
            unit.clear_work();
            self.list_order.unit(unit);
        }
        self.move_cargo(id, x, y);
    }

    pub(super) fn unit_is_nonallied_to(&self, unit: &Unit, owner: usize) -> bool {
        unit.owner != owner
            && (!self.allied(owner, unit.owner)
                || crate::sim::action_rules::freeciv_unit_type_flag(unit.kind, "Flagless")
                    .expect("campaign C2C3 unit-type Flagless binding"))
    }

    pub(super) fn try_capture_units_by_id(&mut self, actor_id: u32, x: i32, y: i32) -> bool {
        let Some(actor) = self.unit_by_id(actor_id).cloned() else {
            return false;
        };
        self.try_capture_units(&actor, x, y)
    }

    fn can_capture_units_at(&self, actor: &Unit, x: i32, y: i32) -> bool {
        if self.map_wrap.civ3_distance(actor.x, actor.y, x, y) != 1 {
            return false;
        }
        let targets = self
            .units
            .iter()
            .filter(|unit| (unit.x, unit.y) == (x, y))
            .collect::<Vec<_>>();
        !targets.is_empty()
            && targets.iter().any(|target| {
                self.action_available_for_unit_against_unit(actor, "Capture Units", target)
                    .unwrap_or(false)
            })
    }

    pub(crate) fn can_begin_capture_units(&self, actor: &Unit) -> bool {
        actor.moves_left > 0
            && self.units.iter().any(|target| {
                target.owner != actor.owner
                    && self.tile(target.x, target.y).visible
                    && self.can_capture_units_at(actor, target.x, target.y)
            })
    }

    pub(super) fn try_move_unit(&mut self, unit_id: u32, dx: i32, dy: i32) -> MoveResult {
        if !is_isohex_step(dx, dy) {
            return MoveResult::Blocked;
        }
        let Some(unit) = self.unit_by_id(unit_id).cloned() else {
            return MoveResult::Blocked;
        };
        let Some((nx, ny)) = self.map_wrap.offset(unit.x, unit.y, dx, dy) else {
            return MoveResult::Blocked;
        };
        let facing = self.map_wrap.civ3_flic_direction(dx, dy);
        self.try_move_unit_to(unit_id, nx, ny, facing)
    }

    pub(super) fn try_move_unit_civ3(&mut self, unit_id: u32, direction: usize) -> MoveResult {
        let Some(unit) = self.unit_by_id(unit_id).cloned() else {
            return MoveResult::Blocked;
        };
        let Some((nx, ny)) = self.map_wrap.civ3_neighbor(unit.x, unit.y, direction) else {
            return MoveResult::Blocked;
        };
        let Some(facing) = self.map_wrap.civ3_flic_direction_index(direction) else {
            return MoveResult::Blocked;
        };
        self.try_move_unit_to(unit_id, nx, ny, Some(facing))
    }

    fn try_move_unit_to(
        &mut self,
        unit_id: u32,
        nx: i32,
        ny: i32,
        facing: Option<u8>,
    ) -> MoveResult {
        let Some(index) = self.unit_index_by_id(unit_id) else {
            return MoveResult::Blocked;
        };
        let unit = self.units[index].clone();
        if unit.kind.is_nuclear_weapon() {
            return MoveResult::Blocked;
        }
        let action_available = if unit.transport.is_some() {
            self.transport_disembark_action_for_unit_fail_closed(&unit, (nx, ny))
                .is_some()
        } else {
            self.action_available_for_unit(&unit, "Unit Move", (nx, ny))
                .is_ok_and(|available| available)
        };
        if !action_available {
            return MoveResult::Blocked;
        }
        if unit.moves_left <= 0 || unit.work_left > 0 || unit.state == UnitState::Sentry {
            return MoveResult::Blocked;
        }
        if self.treaty_blocks_move(&unit, nx, ny) {
            if unit.owner == HUMAN {
                self.add_event("A treaty prevents entry to that tile.");
            }
            return MoveResult::Blocked;
        }
        let attack_target = self
            .units
            .iter()
            .any(|other| (other.x, other.y) == (nx, ny) && self.at_war(unit.owner, other.owner));
        if !attack_target && self.zone_blocks_step(&unit, (unit.x, unit.y), (nx, ny)) {
            return MoveResult::Blocked;
        }
        if !self.can_leave_transport(&unit) {
            return MoveResult::Blocked;
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
                MovementRules::campaign()
                    .move_fragments()
                    .expect("campaign C2C3 movement fragment setting")
            }
            None => return MoveResult::Blocked,
        };
        let defender = self.best_defender_id(&unit, nx, ny);
        if let Some(defender_id) = defender {
            let Some(defender) = self.unit_by_id(defender_id).cloned() else {
                return MoveResult::Blocked;
            };
            let action = self.combat_action_for_unit(&unit);
            if !self.action_available_for_unit_against_unit_fail_closed(&unit, action, &defender)
                || !self.physical_attack_stack_allowed(&unit, nx, ny)
            {
                return MoveResult::Blocked;
            }
            if unit.kind.stats().attack <= 0
                || unit.kind.is_nuclear_weapon()
                || (unit.kind == UnitKind::Submarine && !self.tile(nx, ny).terrain.is_water())
                || (unit.transport.is_some()
                    && !(unit.kind.is_missile() || unit.kind.has_marine_attack()))
            {
                return MoveResult::Blocked;
            }
            // Freeciv wakes nearby sentries before resolving the attack, so
            // a defender survives combat in its normal active state.
            self.wake_neighbor_sentries(unit_id);
            self.resolve_combat(unit_id, defender_id, nx, ny, facing);
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
            && (!self.class_can_occupy_city(unit.kind)
                || (unit.transport.is_some()
                    && self.tile(unit.x, unit.y).terrain.is_water()
                    && !unit.kind.has_marine_attack()))
        {
            return MoveResult::Blocked;
        }

        if !self.resolve_zoc_attacks(unit_id, nx, ny) {
            return MoveResult::Fought;
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
        if !self.enter_tile(unit_id, nx, ny, cost, facing) {
            return MoveResult::Blocked;
        }
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

    pub(super) fn enter_tile(
        &mut self,
        unit_id: u32,
        nx: i32,
        ny: i32,
        cost: i32,
        facing: Option<u8>,
    ) -> bool {
        let entering_city = self
            .unit_by_id(unit_id)
            .and_then(|unit| {
                self.cities
                    .iter()
                    .find(|city| {
                        (city.x, city.y) == (nx, ny) && self.at_war(city.owner, unit.owner)
                    })
                    .map(|city| (unit.clone(), city))
            })
            .map(|(unit, city)| self.city_conquest_action_for_unit(&unit, city));
        if entering_city.is_some_and(|action| action.is_none()) {
            return false;
        }
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
        let mut visual_mover = self
            .visual_events
            .enabled
            .then(|| self.unit_by_id(unit_id).cloned())
            .flatten();
        if let (Some(unit), Some(facing)) = (&mut visual_mover, facing) {
            unit.facing = facing;
        }
        if let Some(mover) = self.units.iter_mut().find(|u| u.id == unit_id) {
            mover.x = nx;
            mover.y = ny;
            if let Some(facing) = facing {
                mover.facing = facing;
            }
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
        self.wake_neighbor_sentries(unit_id);
        if let Some(unit) = visual_mover {
            let from = (unit.x, unit.y);
            self.publish_visual_event(VisualEvent::Move {
                unit,
                from,
                to: (nx, ny),
            });
        }
        if entering_city.is_some_and(|action| action.is_some()) {
            self.capture_city_if_needed(unit_id, nx, ny);
        }
        if let Some(owner) = owner {
            self.refresh_diplomatic_contact_at(owner, nx, ny);
        }
        self.enter_village(unit_id, nx, ny);
        if let Some(old_position) = old_position {
            self.ai_guard_follow_step(unit_id, old_position);
        }
        true
    }

    pub(super) fn player_goto(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.unit_by_id(id) else {
            return false;
        };
        let path = self.plan_route(unit, x, y, true);
        if path.is_empty() {
            let at_destination = (unit.x, unit.y) == (x, y);
            if at_destination {
                self.unit_goto_tiles.remove(&id);
            }
            return at_destination;
        }
        self.unit_goto_tiles.insert(id, (x, y));
        let moved = self.follow_civ3_path(id, path);
        if self
            .unit_by_id(id)
            .is_some_and(|unit| (unit.x, unit.y) == (x, y))
        {
            self.unit_goto_tiles.remove(&id);
        }
        moved
    }

    pub(super) fn continue_player_gotos(&mut self) {
        let orders: Vec<_> = self
            .unit_goto_tiles
            .iter()
            .filter_map(|(&id, &goal)| {
                self.unit_by_id(id)
                    .filter(|unit| unit.owner == HUMAN)
                    .map(|_| (id, goal))
            })
            .collect();
        for (id, (x, y)) in orders {
            let Some(unit) = self.unit_by_id(id) else {
                continue;
            };
            if (unit.x, unit.y) == (x, y) {
                self.unit_goto_tiles.remove(&id);
                continue;
            }
            let path = self.plan_route(unit, x, y, true);
            if path.is_empty() {
                continue;
            }
            self.follow_civ3_path(id, path);
            if self
                .unit_by_id(id)
                .is_some_and(|unit| (unit.x, unit.y) == (x, y))
            {
                self.unit_goto_tiles.remove(&id);
            }
        }
    }

    pub(super) fn follow_civ3_path(
        &mut self,
        unit_id: u32,
        path: impl IntoIterator<Item = (i32, i32)>,
    ) -> bool {
        let mut moved = false;
        for (dx, dy) in path {
            let Some(direction) = CIV3_DIRS.iter().position(|&step| step == (dx, dy)) else {
                break;
            };
            let result = self.try_move_unit_civ3(unit_id, direction);
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
