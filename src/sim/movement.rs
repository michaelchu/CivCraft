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

use super::movement_rules::{MovementRules, MovementTile, campaign_move_fragments};
use super::*;
use std::collections::BTreeMap;

type PlayerGotoGroupKey = ((i32, i32), (i32, i32));

mod activities;
mod combat;
mod routes;
mod sentry;
mod transports;
use std::{cmp::Reverse, collections::BinaryHeap, sync::OnceLock};

pub(super) struct WorkerRouteZoneCache {
    city_owners: Vec<Option<usize>>,
    city_indices: Vec<Option<usize>>,
    enemy_zoc: Vec<Option<bool>>,
    allied_unit: Vec<Option<bool>>,
    enemy_zoc_hits: u64,
    allied_unit_hits: u64,
}

#[derive(Clone, Copy)]
pub(super) struct StepMovementProfile {
    class: super::unit_catalog::UnitClass,
    ignores_terrain: bool,
    coast_strict: bool,
    can_occupy_city: bool,
}

impl StepMovementProfile {
    fn new(kind: UnitKind, class: super::unit_catalog::UnitClass, can_occupy_city: bool) -> Self {
        Self {
            class,
            ignores_terrain: super::action_rules::unit_type_flag(kind, "IgTer")
                .expect("campaign C2C3 unit-type IgTer binding"),
            coast_strict: super::action_rules::unit_type_flag(kind, "CoastStrict")
                .expect("campaign C2C3 unit-type CoastStrict binding"),
            can_occupy_city,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct WorkerRouteMovementProfile {
    step: StepMovementProfile,
    ignores_zoc: bool,
    pub(super) unknown_tile_cost: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PathZone {
    Unrestricted,
    AlliedStack,
    Hostile,
}

impl PathZone {
    pub(super) fn blocks_entry_from(self, source: Self) -> bool {
        self == Self::Hostile && source != Self::Unrestricted
    }
}

impl WorkerRouteZoneCache {
    pub(super) fn new(sim: &Simulation) -> Self {
        let mut city_owners = vec![None; sim.tiles.len()];
        let mut city_indices = vec![None; sim.tiles.len()];
        for (city_index, city) in sim.cities.iter().enumerate() {
            let tile = sim.map_wrap.index(city.x, city.y);
            city_owners[tile] = Some(city.owner);
            city_indices[tile] = Some(city_index);
        }
        Self {
            city_owners,
            city_indices,
            enemy_zoc: vec![None; sim.tiles.len()],
            allied_unit: vec![None; sim.tiles.len()],
            enemy_zoc_hits: 0,
            allied_unit_hits: 0,
        }
    }

    pub(super) fn cache_hits(&self) -> (u64, u64) {
        (self.enemy_zoc_hits, self.allied_unit_hits)
    }

    pub(super) fn city_at<'a>(&self, sim: &'a Simulation, x: i32, y: i32) -> Option<&'a City> {
        self.city_indices[sim.map_wrap.index(x, y)].and_then(|index| sim.cities.get(index))
    }

    pub(super) fn foreign_city_at(&self, sim: &Simulation, owner: usize, x: i32, y: i32) -> bool {
        self.city_owners[sim.map_wrap.index(x, y)]
            .is_some_and(|city_owner| !sim.military_access(owner, city_owner))
    }
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(super) struct RouteTile {
    pub(super) terrain: Terrain,
    pub(super) road: bool,
    pub(super) railroad: bool,
    pub(super) river: bool,
    pub(super) city_owner: Option<usize>,
}

#[cfg(test)]
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
        self.trace
            .unit_removed(self.turn, victim, lifecycle::UnitRemoval::Transferred);
        self.discard_unit_ai(victim.id);
        self.spawn_unit_with_origin(
            owner,
            victim.kind,
            victim.x,
            victim.y,
            lifecycle::UnitOrigin::Transferred {
                previous_owner: victim.owner,
                previous_unit_id: victim.id,
            },
        );
        let gained = self.units.last_mut().unwrap();
        gained.hp = victim.hp;
        gained.nationality = victim.nationality;
        gained.fuel = victim.fuel;
        gained.veteran = victim.veteran;
        gained.veteran_level = victim.veteran_level;
        gained.paradropped = victim.paradropped;
        gained.moves_left = victim.moves_left;
        gained.home_city = home;
        gained.home_order = gained.id;
        gained.id
    }

    pub(crate) fn unit_move_rate(&self, unit: &Unit) -> i32 {
        let fragments = campaign_move_fragments();
        if unit.kind == UnitKind::Army {
            return self.army_move_rate(unit);
        }
        if unit.kind.is_fixed_wing_aircraft() {
            // Civ III fixed-wing aircraft have a one-mission-per-turn action
            // budget; mission radius is governed by operational_range/rebase_range.
            return fragments;
        }
        if unit.kind.conquests_rule().unit_type == 2 {
            // Missiles and helicopters execute within their source range.
            return unit.kind.operational_range().max(1) * fragments;
        }
        let (_, class) = MovementRules::campaign_unit_movement_data(unit.kind);
        let base = unit.kind.stats().moves * fragments + unit.veteran_move_bonus();
        let mut rate = if class.damage_slows {
            base * unit.hp / unit.max_hit_points()
        } else {
            base
        };
        let context = super::action_rules::SimulationRequirementContext::for_unit(self, unit);
        let effect_bonus = context.effect(EffectKind::MoveBonus);
        rate += i32::try_from(effect_bonus).expect("campaign C2C3 movement bonus fits in i32")
            * fragments;
        let minimum = (class.min_speed * fragments).min(base);
        rate.max(minimum)
    }

    pub(super) fn live_route_tile(&self, x: i32, y: i32) -> RouteTile {
        let mut tile = RouteTile::from(self.tile(x, y));
        tile.city_owner = self.city_at(x, y).map(|city| city.owner);
        tile
    }

    pub(super) fn observed_route_tile(&self, x: i32, y: i32) -> Option<RouteTile> {
        self.route_tile_for(HUMAN, x, y)
    }

    fn route_tile_for(&self, observer: usize, x: i32, y: i32) -> Option<RouteTile> {
        let mut route = RouteTile::from(self.known_tile_for(observer, x, y)?);
        route.city_owner = self.observed_city_owner(observer, x, y);
        Some(route)
    }

    pub(super) fn observed_move_cost_with_profile(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
        dest: Option<RouteTile>,
        profile: &StepMovementProfile,
    ) -> Option<i32> {
        let kind = unit.kind;
        if kind.is_aircraft() {
            return Some(campaign_move_fragments());
        }
        let class = profile.class;
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
        let destination_native = self.cached_movement_native_to_class(to, class);
        if destination_known
            && destination.city_owner.is_none()
            && profile.coast_strict
            && !self.safe_ocean_at(x, y, Some(HUMAN))
        {
            return None;
        }
        if destination_known && kind.is_naval() {
            let native = destination_native;
            let port = destination
                .city_owner
                .is_some_and(|owner| self.military_access(owner, unit.owner))
                && self.map_wrap.eight_way_neighbors(x, y).any(|(nx, ny)| {
                    self.observed_route_tile(nx, ny).is_none_or(|tile| {
                        self.cached_movement_native_to_class(
                            MovementTile::new(
                                tile.terrain,
                                tile.road,
                                tile.railroad,
                                tile.river,
                                self.render_tile(nx, ny).base,
                            ),
                            class,
                        )
                    })
                });
            if !native && !port {
                return None;
            }
        } else if destination_known {
            let city_access = destination.city_owner.is_some_and(|owner| {
                self.military_access(owner, unit.owner)
                    || self.at_war(owner, unit.owner) && profile.can_occupy_city
            });
            if !destination_native && !city_access {
                return None;
            }
        }
        let restricted = [(unit.x, unit.y), (x, y)].into_iter().any(|(tx, ty)| {
            self.observed_route_tile(tx, ty).is_some()
                && self
                    .render_tile(tx, ty)
                    .owner
                    .is_some_and(|owner| self.at_war(unit.owner, owner))
        });
        Some(self.caches.movement().step_cost(
            from,
            to,
            class,
            self.movement_is_cardinal((unit.x, unit.y), (x, y)),
            restricted,
            profile.ignores_terrain,
        ))
    }

    pub(super) fn movement_cost_for(&self, unit: &Unit, nx: i32, ny: i32) -> Option<i32> {
        self.movement_cost_for_with_city_cache(unit, nx, ny, None, None)
    }

    pub(super) fn step_movement_profile(&self, kind: UnitKind) -> StepMovementProfile {
        let (class, data) = MovementRules::campaign_unit_movement_data(kind);
        StepMovementProfile::new(kind, *class, data.can_occupy_city)
    }

    pub(super) fn movement_cost_with_profile(
        &self,
        unit: &Unit,
        nx: i32,
        ny: i32,
        profile: &StepMovementProfile,
    ) -> Option<i32> {
        self.movement_cost_for_with_city_cache(unit, nx, ny, None, Some(profile))
    }

    pub(super) fn raw_move_cost_with_profile(
        &self,
        unit: &Unit,
        nx: i32,
        ny: i32,
        profile: &StepMovementProfile,
    ) -> i32 {
        self.raw_move_cost_for_class(unit, nx, ny, profile.class, profile.ignores_terrain)
    }

    pub(super) fn movement_cost_for_worker_route(
        &self,
        unit: &Unit,
        nx: i32,
        ny: i32,
        cache: &WorkerRouteZoneCache,
        profile: &WorkerRouteMovementProfile,
    ) -> Option<i32> {
        self.movement_cost_for_with_city_cache(unit, nx, ny, Some(cache), Some(&profile.step))
    }

    fn movement_cost_for_with_city_cache(
        &self,
        unit: &Unit,
        nx: i32,
        ny: i32,
        city_cache: Option<&WorkerRouteZoneCache>,
        profile: Option<&StepMovementProfile>,
    ) -> Option<i32> {
        if unit.kind.is_aircraft() {
            return Some(campaign_move_fragments());
        }
        let class = profile.map_or_else(
            || MovementRules::campaign_unit_class(unit.kind),
            |profile| profile.class,
        );
        let ignores_terrain = profile.map_or_else(
            || {
                super::action_rules::unit_type_flag(unit.kind, "IgTer")
                    .expect("campaign C2C3 unit-type IgTer binding")
            },
            |profile| profile.ignores_terrain,
        );
        self.movement_destination_accessible(unit, nx, ny, class, city_cache, profile)
            .then(|| self.raw_move_cost_for_class(unit, nx, ny, class, ignores_terrain))
    }

    fn movement_destination_accessible(
        &self,
        unit: &Unit,
        nx: i32,
        ny: i32,
        class: super::unit_catalog::UnitClass,
        city_cache: Option<&WorkerRouteZoneCache>,
        profile: Option<&StepMovementProfile>,
    ) -> bool {
        let coast_strict = profile.map_or_else(
            || {
                super::action_rules::unit_type_flag(unit.kind, "CoastStrict")
                    .expect("campaign C2C3 unit-type CoastStrict binding")
            },
            |profile| profile.coast_strict,
        );
        let dest = self.tile(nx, ny);
        let destination_native =
            self.cached_movement_native_to_class(MovementTile::from(dest), class);
        if destination_native && !coast_strict {
            return true;
        }
        let city =
            city_cache.map_or_else(|| self.city_at(nx, ny), |cache| cache.city_at(self, nx, ny));
        if city.is_none() && coast_strict && !self.safe_ocean_at(nx, ny, None) {
            return false;
        }
        if unit.kind.is_naval() {
            return destination_native
                || city.is_some_and(|city| {
                    self.military_access(city.owner, unit.owner)
                        && self.naval_city_access(city, unit.kind)
                });
        }
        let can_occupy_city = profile.map_or_else(
            || {
                MovementRules::campaign_unit_movement_data(unit.kind)
                    .1
                    .can_occupy_city
            },
            |profile| profile.can_occupy_city,
        );
        let city_access = city.is_some_and(|city| {
            self.military_access(city.owner, unit.owner)
                || self.at_war(city.owner, unit.owner) && can_occupy_city
        });
        destination_native || city_access
    }

    pub(super) fn worker_route_movement_profile(
        &self,
        kind: UnitKind,
    ) -> WorkerRouteMovementProfile {
        let rules = MovementRules::campaign();
        let class = MovementRules::campaign_unit_class(kind);
        let class_data = MovementRules::campaign_class_data(class);
        WorkerRouteMovementProfile {
            step: StepMovementProfile::new(kind, class, class_data.can_occupy_city),
            ignores_zoc: Self::unit_ignores_zoc(kind),
            unknown_tile_cost: rules
                .unknown_tile_move_cost(kind)
                .expect("selected C2C3 unknown-tile movement cost"),
        }
    }

    pub(super) fn movement_is_cardinal(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        super::topology::CIV3_DIRS
            .iter()
            .enumerate()
            .any(|(direction, _)| {
                !matches!(direction, 2 | 6)
                    && self.map_wrap.eight_way_neighbor(from.0, from.1, direction) == Some(to)
            })
    }

    /// Freeciv `is_safe_ocean`: an adjacent known tile must not carry the
    /// selected C2C3 UnsafeCoast terrain flag. `observer=None` uses live map
    /// state; observer-scoped checks use that player's stored observation.
    pub(super) fn safe_ocean_at(&self, x: i32, y: i32, observer: Option<usize>) -> bool {
        let rules = MovementRules::campaign();
        self.map_wrap
            .eight_way_neighbors(x, y)
            .filter(|&(nx, ny)| self.movement_is_cardinal((x, y), (nx, ny)))
            .any(|(nx, ny)| {
                let tile = match observer {
                    Some(owner) => self.known_tile_for(owner, nx, ny),
                    None => Some(self.tile(nx, ny)),
                };
                tile.is_some_and(|tile| {
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
        MovementRules::campaign_unit_movement_data(kind)
            .1
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

    pub(super) fn enemy_zoc_controls_tile(
        &self,
        owner: usize,
        x: i32,
        y: i32,
        observed: bool,
    ) -> bool {
        if self.advisor_cache.active() {
            let index = self.map_wrap.index(x, y);
            let visibility = usize::from(observed);
            if let Some(controlled) = self
                .advisor_cache
                .memos()
                .zone_control(owner, visibility, index)
            {
                self.trace
                    .count_operation("movement.route_zoc_cache_hits", 1);
                return controlled;
            }
            self.trace
                .count_operation("movement.route_zoc_cache_misses", 1);
            let controlled = self.enemy_zoc_controls_tile_uncached(owner, x, y, observed);
            self.advisor_cache.memos().remember_zone_control(
                owner,
                visibility,
                index,
                self.tiles.len(),
                controlled,
            );
            return controlled;
        }
        self.enemy_zoc_controls_tile_uncached(owner, x, y, observed)
    }

    fn enemy_zoc_controls_tile_uncached(
        &self,
        owner: usize,
        x: i32,
        y: i32,
        observed: bool,
    ) -> bool {
        std::iter::once((x, y))
            .chain(self.map_wrap.eight_way_neighbors(x, y))
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
                    .is_some_and(|city_owner| !self.military_access(owner, city_owner))
                {
                    if observed {
                        // Freeciv exposes an occupied bit for visible enemy
                        // cities and assumes fogged cities are occupied. The
                        // app does not persist that bit, so recover only the
                        // boolean from a live stack when the city is visible.
                        return !self.tile(tx, ty).visible || self.any_unit_at(tx, ty, |_| true);
                    }
                    return self.any_unit_at(tx, ty, |_| true);
                }

                // Freeciv can also source ZOC from extras and their owners.
                // CivCraft's tile model has no generic extra-owner or ZOC
                // extra collection, so those ruleset sources are not yet
                // representable here.
                self.any_unit_at(tx, ty, |unit| {
                    (!observed || self.unit_visible_to_human(unit))
                        && !self.military_access(owner, unit.owner)
                        && unit.transport.is_none()
                        && Self::unit_class_uses_zoc(unit.kind)
                        && !super::action_rules::unit_type_flag(unit.kind, "HasNoZOC")
                            .expect("campaign C2C3 unit-type HasNoZOC binding")
                })
            })
    }

    pub(super) fn terrain_has_no_zoc(terrain: Terrain) -> bool {
        const TERRAIN_COUNT: usize = Terrain::GrasslandShield as usize + 1;
        static NO_ZOC: [OnceLock<bool>; TERRAIN_COUNT] = [const { OnceLock::new() }; TERRAIN_COUNT];
        *NO_ZOC[terrain as usize].get_or_init(|| {
            match MovementRules::campaign()
                .terrain_has_flag(terrain, super::movement_rules::TerrainFlag::NoZoc)
            {
                Ok(flag) => flag,
                // Volcano has no C2C3 terrain record. Its movement-cost mapping
                // does not imply any other terrain flags.
                Err(super::movement_rules::MovementRulesError::UnsupportedTerrain(_)) => false,
                Err(error) => panic!("campaign C2C3 terrain NoZoc data is invalid: {error:?}"),
            }
        })
    }

    fn unit_class_uses_zoc(kind: UnitKind) -> bool {
        MovementRules::campaign_unit_movement_data(kind).1.zoc
    }

    fn allied_unit_at(&self, owner: usize, x: i32, y: i32, observed: bool) -> bool {
        self.any_unit_at(x, y, |other| {
            (!observed || self.unit_visible_to_human(other))
                && self.military_access(owner, other.owner)
        })
    }

    pub(super) fn unit_ignores_zoc(kind: UnitKind) -> bool {
        !Self::unit_class_uses_zoc(kind)
            || super::action_rules::unit_type_flag(kind, "IgZOC")
                .expect("campaign C2C3 unit-type IgZOC binding")
    }

    pub(super) fn live_path_zone(&self, owner: usize, x: i32, y: i32) -> PathZone {
        if self.city_at(x, y).is_some()
            || Self::terrain_has_no_zoc(self.tile(x, y).terrain)
            || !self.enemy_zoc_controls_tile(owner, x, y, false)
        {
            PathZone::Unrestricted
        } else if self.allied_unit_at(owner, x, y, false) {
            PathZone::AlliedStack
        } else {
            PathZone::Hostile
        }
    }

    fn zone_blocks_step_with_observation(
        &self,
        unit: &Unit,
        from: (i32, i32),
        to: (i32, i32),
        observed: bool,
    ) -> bool {
        if Self::unit_ignores_zoc(unit.kind) {
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

    fn enemy_zoc_controls_tile_for_worker_route(
        &self,
        owner: usize,
        position: (i32, i32),
        cache: &mut WorkerRouteZoneCache,
    ) -> bool {
        let index = self.map_wrap.index(position.0, position.1);
        if let Some(controlled) = cache.enemy_zoc[index] {
            cache.enemy_zoc_hits += 1;
            return controlled;
        }
        let controlled = self.enemy_zoc_controls_tile(owner, position.0, position.1, false);
        cache.enemy_zoc[index] = Some(controlled);
        controlled
    }

    #[cfg(test)]
    pub(super) fn zone_blocks_worker_route_step(
        &self,
        unit: &Unit,
        from: (i32, i32),
        to: (i32, i32),
        cache: &mut WorkerRouteZoneCache,
    ) -> bool {
        let profile = self.worker_route_movement_profile(unit.kind);
        self.zone_blocks_worker_route_step_with_profile(unit, from, to, cache, &profile)
    }

    pub(super) fn zone_blocks_worker_route_step_with_profile(
        &self,
        unit: &Unit,
        from: (i32, i32),
        to: (i32, i32),
        cache: &mut WorkerRouteZoneCache,
        profile: &WorkerRouteMovementProfile,
    ) -> bool {
        if profile.ignores_zoc {
            return false;
        }
        let destination = self.map_wrap.index(to.0, to.1);
        let allied_unit = match cache.allied_unit[destination] {
            Some(value) => {
                cache.allied_unit_hits += 1;
                value
            }
            None => {
                let value = self.allied_unit_at(unit.owner, to.0, to.1, false);
                cache.allied_unit[destination] = Some(value);
                value
            }
        };
        if allied_unit {
            return false;
        }
        let source = self.map_wrap.index(from.0, from.1);
        if cache.city_owners[source].is_some()
            || cache.city_owners[destination].is_some()
            || Self::terrain_has_no_zoc(self.tile(from.0, from.1).terrain)
            || Self::terrain_has_no_zoc(self.tile(to.0, to.1).terrain)
        {
            return false;
        }
        self.enemy_zoc_controls_tile_for_worker_route(unit.owner, from, cache)
            && self.enemy_zoc_controls_tile_for_worker_route(unit.owner, to, cache)
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
        let cost = super::action_rules::SimulationRequirementContext::for_unit(self, actor)
            .with_action(action)
            .effect(EffectKind::ActionSuccessActorMoveCost);
        let cost = i32::try_from(cost).expect("campaign C2C3 action movement cost fits in i32");
        if let Some(live_actor) = self.units.iter_mut().find(|unit| unit.id == actor.id) {
            live_actor.moves_left = (live_actor.moves_left - cost).max(0);
        }
        cost
    }

    fn capture_units_unique_preflight(&self, owner: usize, targets: &[Unit]) -> bool {
        let mut unique_types = Vec::new();
        for target in targets {
            if !super::action_rules::unit_type_flag(target.kind, "Unique")
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
            (city.x, city.y) == (unit.x, unit.y) && !self.military_access(unit.owner, city.owner)
        });
        let hostile_unlisted_unit = self.units.iter().any(|other| {
            other.id != id
                && (other.x, other.y) == (unit.x, unit.y)
                && !self.military_access(unit.owner, other.owner)
                && !remaining_capture_ids.contains(&other.id)
        });
        if city_tile || hostile_city || hostile_unlisted_unit {
            self.bounce_unit(id);
        }
    }

    pub(super) fn bounce_unit(&mut self, id: u32) {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return;
        };
        let mut destinations = Vec::new();
        for (x, y) in self
            .map_wrap
            .square_radius_in_source_order(unit.x, unit.y, 2)
        {
            if (x, y) == (unit.x, unit.y) {
                continue;
            }
            if self.unit_can_survive_at_tile(&unit, x, y)
                && self.cities.iter().all(|city| {
                    (city.x, city.y) != (x, y) || self.military_access(unit.owner, city.owner)
                })
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
                    self.bounce_unit(passenger);
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
            && (!self.military_access(owner, unit.owner)
                || crate::sim::action_rules::unit_type_flag(unit.kind, "Flagless")
                    .expect("campaign C2C3 unit-type Flagless binding"))
    }

    pub(super) fn try_capture_units_by_id(&mut self, actor_id: u32, x: i32, y: i32) -> bool {
        let Some(actor) = self.unit_by_id(actor_id).cloned() else {
            return false;
        };
        self.try_capture_units(&actor, x, y)
    }

    fn can_capture_units_at(&self, actor: &Unit, x: i32, y: i32) -> bool {
        if self.map_wrap.step_distance(actor.x, actor.y, x, y) != 1 {
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

    /// Civ III eight-direction grid.
    pub(crate) fn direction_for_unit_offset(
        &self,
        unit_id: u32,
        dx: i32,
        dy: i32,
    ) -> Option<usize> {
        let unit = self.unit_by_id(unit_id)?;
        let destination = self.map_wrap.offset(unit.x, unit.y, dx, dy)?;
        (0..super::topology::CIV3_DIRS.len()).find(|&direction| {
            self.map_wrap.eight_way_neighbor(unit.x, unit.y, direction) == Some(destination)
        })
    }

    /// Moves along Civ III's eight-direction grid.
    #[cfg(test)]
    pub(crate) fn try_move_unit_by_offset(&mut self, unit_id: u32, dx: i32, dy: i32) -> MoveResult {
        let Some(direction) = self.direction_for_unit_offset(unit_id, dx, dy) else {
            return MoveResult::Blocked;
        };
        self.try_move_unit(unit_id, direction)
    }

    /// Moves along Civ III's eight-direction grid.
    pub(in crate::sim) fn try_move_unit(&mut self, unit_id: u32, direction: usize) -> MoveResult {
        let Some(unit) = self.unit_by_id(unit_id).cloned() else {
            return MoveResult::Blocked;
        };
        let Some((nx, ny)) = self.map_wrap.eight_way_neighbor(unit.x, unit.y, direction) else {
            return MoveResult::Blocked;
        };
        let Some(facing) = self.map_wrap.flic_direction_index(direction) else {
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
        let profiler = self.trace.profiler();
        let Some(index) = self.unit_index_by_id(unit_id) else {
            return MoveResult::Blocked;
        };
        let unit = self.units[index].clone();
        if unit.kind.is_nuclear_weapon() || unit.kind.is_fixed_wing_aircraft() {
            return MoveResult::Blocked;
        }
        let action_available = {
            let _profile = profiler.scope("movement.execute.action_availability");
            if unit.transport.is_some() {
                self.transport_disembark_action_for_unit_fail_closed(&unit, (nx, ny))
                    .is_some()
            } else {
                self.action_available_for_unit(&unit, "Unit Move", (nx, ny))
                    .is_ok_and(|available| available)
            }
        };
        if !action_available {
            return MoveResult::Blocked;
        }
        if unit.moves_left <= 0 || unit.work_left > 0 || unit.state == UnitState::Sentry {
            return MoveResult::Blocked;
        }
        let _profile = profiler.scope("movement.execute.blocking_checks");
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
        drop(_profile);
        let _profile = profiler.scope("movement.execute.cost_and_defender");
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
                MovementRules::campaign().move_fragments()
            }
            None => return MoveResult::Blocked,
        };
        let defender = self.best_defender_id(&unit, nx, ny);
        drop(_profile);
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
            if self.unit_attack_strength(&unit) <= 0
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
            self.try_defensive_bombard(unit_id, nx, ny);
            self.resolve_combat(unit_id, defender_id, nx, ny, facing);
            return MoveResult::Fought;
        }

        if self.units.iter().any(|other| {
            (other.x, other.y) == (nx, ny)
                && self.at_war(unit.owner, other.owner)
                && !other.kind.is_fixed_wing_aircraft()
        }) {
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

        // A unit fortified in a Fortress can bombard an enemy passing within
        // one tile, even when the destination tile itself is empty.
        self.try_defensive_bombard(unit_id, nx, ny);
        let carrier = unit
            .kind
            .is_aircraft()
            .then(|| self.boarding_target(&unit, nx, ny))
            .flatten();
        let _profile = profiler.scope("movement.execute.enter_tile");
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
        // An AI-controlled HUMAN slot was already refreshed by enter_tile.
        if unit.owner == HUMAN && !self.players[HUMAN].ai {
            self.refresh_visibility();
        }
        MoveResult::Moved
    }

    /// Moves refresh only the tiles around the mover unless an interactive
    /// player needs the immediate full refresh. An AI-controlled HUMAN slot,
    /// as in headless runs, behaves like any other AI civilization.
    fn moves_use_scoped_visibility(&self, owner: usize) -> bool {
        owner != HUMAN || self.players[HUMAN].ai
    }

    pub(super) fn enter_tile(
        &mut self,
        unit_id: u32,
        nx: i32,
        ny: i32,
        cost: i32,
        facing: Option<u8>,
    ) -> bool {
        self.enter_tile_with_conquest_action(unit_id, nx, ny, cost, facing, None)
    }

    pub(super) fn enter_tile_with_conquest_action(
        &mut self,
        unit_id: u32,
        nx: i32,
        ny: i32,
        cost: i32,
        facing: Option<u8>,
        conquest_action: Option<&'static str>,
    ) -> bool {
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("movement.execute.enter_tile.city_action");
        let Some(unit_index) = self.unit_index_by_id(unit_id) else {
            return false;
        };
        let moving_unit = self.units[unit_index].clone();
        let old_position = (moving_unit.x, moving_unit.y);
        let owner = moving_unit.owner;
        let entering_city = self
            .cities
            .iter()
            .find(|city| (city.x, city.y) == (nx, ny) && self.at_war(city.owner, moving_unit.owner))
            .map(|city| {
                conquest_action.or_else(|| self.city_conquest_action_for_unit(&moving_unit, city))
            });
        if entering_city.is_some_and(|action| action.is_none()) {
            return false;
        }
        drop(_profile);
        let _profile = profiler.scope("movement.execute.enter_tile.unit_update");
        if self.moves_use_scoped_visibility(moving_unit.owner) {
            self.record_visibility_render_scope(&moving_unit, old_position, (nx, ny));
        }
        if moving_unit.transport.is_some() {
            self.release_ai_ferry(unit_id);
        }
        let mut visual_mover = self.visual_events.enabled.then(|| moving_unit.clone());
        if let (Some(unit), Some(facing)) = (&mut visual_mover, facing) {
            unit.facing = facing;
        }
        let mover = &mut self.units[unit_index];
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
        self.record_transport_change(unit_id, None);
        drop(_profile);
        let _profile = profiler.scope("movement.execute.enter_tile.cargo");
        self.move_cargo(unit_id, nx, ny);
        drop(_profile);
        // Native movement updates the moving player's vision immediately,
        // including AI discovery, before contact callbacks and hut outcomes.
        if self.visibility_refresh.deferred() {
            self.visibility_refresh.request_refresh();
        } else if self.moves_use_scoped_visibility(moving_unit.owner) {
            self.visibility_refresh.request_refresh();
            self.flush_deferred_visibility();
        } else {
            let _profile = self
                .trace
                .profiler()
                .scope("movement.execute.refresh_visibility");
            self.refresh_visibility();
        }
        let _profile = profiler.scope("movement.execute.enter_tile.sentries_and_visuals");
        self.wake_neighbor_sentries(unit_id);
        if let Some(unit) = visual_mover {
            let from = (unit.x, unit.y);
            self.publish_visual_event(VisualEvent::Move {
                unit,
                from,
                to: (nx, ny),
            });
        }
        drop(_profile);
        let _profile = profiler.scope("movement.execute.enter_tile.capture");
        if entering_city.is_some_and(|action| action.is_some()) {
            self.capture_city_if_needed(unit_id, nx, ny);
        }
        // Entering an undefended airfield or conquered city destroys any
        // remaining hostile fixed-wing aircraft stationed on the ground there.
        let overrun_aircraft: Vec<_> = self
            .units
            .iter()
            .filter(|other| {
                (other.x, other.y) == (nx, ny)
                    && other.transport.is_none()
                    && other.kind.is_fixed_wing_aircraft()
                    && self.at_war(owner, other.owner)
            })
            .map(|other| other.id)
            .collect();
        if !overrun_aircraft.is_empty() {
            let count = overrun_aircraft.len() as i32;
            self.remove_units_with_cargo_because(overrun_aircraft, lifecycle::UnitRemoval::Combat);
            self.nation_scores[owner].units_killed += count;
        }
        drop(_profile);
        let _profile = profiler.scope("movement.execute.enter_tile.diplomacy");
        self.refresh_diplomatic_contact_at(owner, nx, ny);
        drop(_profile);
        let _profile = profiler.scope("movement.execute.enter_tile.village");
        self.enter_village(unit_id, nx, ny);
        drop(_profile);
        let _profile = profiler.scope("movement.execute.enter_tile.guard_follow");
        self.ai_guard_follow_step(unit_id, old_position);
        true
    }

    pub(super) fn issue_player_goto(&mut self, id: u32, x: i32, y: i32) -> bool {
        if !(0..self.map_wrap.width()).contains(&x) || !(0..self.map_wrap.height()).contains(&y) {
            return false;
        }
        let Some(index) = self.unit_index_by_id(id) else {
            return false;
        };
        if self.units[index].kind.is_fixed_wing_aircraft() {
            return self.rebase_aircraft(id, x, y);
        }
        let unit = &self.units[index];
        let path = self.plan_route(unit, x, y, true);
        if path.is_empty() && (unit.x, unit.y) != (x, y) {
            return false;
        }
        let unit = &mut self.units[index];
        unit.automated = false;
        unit.order = None;
        unit.clear_work();
        self.ai_worker_tasks.remove(&id);
        self.follow_player_goto(id, (x, y), path);
        true
    }

    pub(super) fn issue_player_goto_group(&mut self, ids: Vec<u32>, x: i32, y: i32) -> bool {
        if !(0..self.map_wrap.width()).contains(&x) || !(0..self.map_wrap.height()).contains(&y) {
            return false;
        }
        let mut valid = Vec::new();
        for id in ids {
            if valid.contains(&id) {
                continue;
            }
            let Some(unit) = self.unit_by_id(id).filter(|unit| unit.owner == HUMAN) else {
                continue;
            };
            if unit.kind.is_fixed_wing_aircraft() {
                if self.can_rebase_to(unit, x, y) {
                    valid.push(id);
                }
            } else if (unit.x, unit.y) == (x, y) || !self.plan_route(unit, x, y, true).is_empty() {
                valid.push(id);
            }
        }
        let Some(shared_moves) = valid
            .iter()
            .filter_map(|id| self.unit_by_id(*id).map(|unit| unit.moves_left))
            .min()
        else {
            return false;
        };
        let mut succeeded = false;
        for id in valid {
            let Some(index) = self.unit_index_by_id(id) else {
                continue;
            };
            let original_moves = self.units[index].moves_left;
            self.units[index].moves_left = shared_moves;
            if self.issue_player_goto(id, x, y) {
                succeeded = true;
                if let Some(index) = self.unit_index_by_id(id) {
                    let spent = (shared_moves - self.units[index].moves_left).max(0);
                    self.units[index].moves_left = (original_moves - spent).max(0);
                }
            } else if let Some(index) = self.unit_index_by_id(id) {
                self.units[index].moves_left = original_moves;
            }
        }
        succeeded
    }

    pub(super) fn player_goto(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.unit_by_id(id) else {
            return false;
        };
        let path = self.plan_route(unit, x, y, true);
        if path.is_empty() && (unit.x, unit.y) != (x, y) {
            return false;
        }
        self.follow_player_goto(id, (x, y), path)
    }

    fn follow_player_goto(
        &mut self,
        id: u32,
        destination: (i32, i32),
        path: Vec<(i32, i32)>,
    ) -> bool {
        if path.is_empty() {
            self.unit_goto_tiles.remove(&id);
            return true;
        }
        self.unit_goto_tiles.insert(id, destination);
        let moved = self.follow_grid_path(id, path);
        if self
            .unit_by_id(id)
            .is_some_and(|unit| (unit.x, unit.y) == destination)
        {
            self.unit_goto_tiles.remove(&id);
        }
        moved
    }

    pub(super) fn can_start_connect(&self, unit: &Unit) -> bool {
        unit.kind.stats().can_work
            && unit.order.is_none()
            && unit.transport.is_none()
            && unit.work_left == 0
    }

    pub(super) fn can_start_patrol(&self, unit: &Unit) -> bool {
        unit.kind.is_military()
            && !unit.kind.is_fixed_wing_aircraft()
            && unit.order.is_none()
            && unit.transport.is_none()
    }

    fn movement_order_path(&self, unit: &Unit, x: i32, y: i32) -> Option<Vec<(i32, i32)>> {
        if (unit.x, unit.y) == (x, y)
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
        {
            return None;
        }
        let path = self.plan_route(unit, x, y, true);
        (!path.is_empty()).then_some(path)
    }

    pub(super) fn start_connect_order(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(index) = self.unit_index_by_id(id) else {
            return false;
        };
        let unit = &self.units[index];
        if !self.can_start_connect(unit) || self.movement_order_path(unit, x, y).is_none() {
            return false;
        }
        self.units[index].order = Some(UnitOrder::Connect { x, y });
        self.unit_goto_tiles.remove(&id);
        true
    }

    pub(super) fn start_patrol_order(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(index) = self.unit_index_by_id(id) else {
            return false;
        };
        let unit = &self.units[index];
        if !self.can_start_patrol(unit) {
            return false;
        }
        let Some(path) = self.movement_order_path(unit, x, y) else {
            return false;
        };
        let origin = (unit.x, unit.y);
        self.units[index].order = Some(UnitOrder::Patrol {
            origin,
            destination: (x, y),
            returning: false,
        });
        self.follow_player_goto(id, (x, y), path);
        if self
            .unit_by_id(id)
            .is_some_and(|unit| (unit.x, unit.y) == (x, y))
            && let Some(UnitOrder::Patrol { returning, .. }) = self
                .unit_index_by_id(id)
                .and_then(|index| self.units[index].order.as_mut())
        {
            *returning = true;
        }
        true
    }

    pub(super) fn cancel_unit_order(&mut self, id: u32) -> bool {
        let Some(index) = self.unit_index_by_id(id) else {
            return false;
        };
        let had_order = self.units[index].order.take().is_some();
        self.unit_goto_tiles.remove(&id);
        had_order
    }

    fn continue_unit_orders(&mut self) {
        let orders: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.owner == HUMAN && unit.order.is_some())
            .map(|unit| (unit.id, unit.order.unwrap()))
            .collect();
        for (id, order) in orders {
            match order {
                UnitOrder::AirSuperiority { x, y } => {
                    if self.unit_by_id(id).is_none_or(|unit| {
                        (unit.x, unit.y) != (x, y) || !self.can_set_air_superiority(unit)
                    }) {
                        self.cancel_unit_order(id);
                    }
                }
                UnitOrder::Patrol {
                    origin,
                    destination,
                    returning,
                } => {
                    let target = if returning { origin } else { destination };
                    let Some(unit) = self.unit_by_id(id) else {
                        continue;
                    };
                    if (unit.x, unit.y) == target {
                        if let Some(index) = self.unit_index_by_id(id)
                            && let Some(UnitOrder::Patrol { returning, .. }) =
                                self.units[index].order.as_mut()
                        {
                            *returning = !*returning;
                        }
                    } else {
                        self.player_goto(id, target.0, target.1);
                        if self
                            .unit_by_id(id)
                            .is_some_and(|unit| (unit.x, unit.y) == target)
                            && let Some(index) = self.unit_index_by_id(id)
                            && let Some(UnitOrder::Patrol { returning, .. }) =
                                self.units[index].order.as_mut()
                        {
                            *returning = !*returning;
                        }
                    }
                }
                UnitOrder::Bombard { x, y } => {
                    let Some(unit) = self.unit_by_id(id).cloned() else {
                        continue;
                    };
                    if !self.can_bombard_tile(&unit, x, y)
                        || self.try_ranged_bombard(id, x, y) == MoveResult::Blocked
                        || self
                            .unit_by_id(id)
                            .is_none_or(|unit| !self.repeated_bombard_target_remains(unit, x, y))
                    {
                        self.cancel_unit_order(id);
                    }
                }
                UnitOrder::PrecisionBombard { x, y } => {
                    let Some(unit) = self.unit_by_id(id).cloned() else {
                        continue;
                    };
                    if !self.repeated_precision_bombard_target_remains(&unit, x, y)
                        || self.try_precision_bombard(id, x, y) == MoveResult::Blocked
                        || self.unit_by_id(id).is_none_or(|unit| {
                            !self.repeated_precision_bombard_target_remains(unit, x, y)
                        })
                    {
                        self.cancel_unit_order(id);
                    }
                }
                UnitOrder::Connect { x, y } => {
                    let Some(unit) = self.unit_by_id(id).cloned() else {
                        continue;
                    };
                    if (unit.x, unit.y) == (x, y) {
                        let tile_is_city = self
                            .cities
                            .iter()
                            .any(|city| (city.x, city.y) == (unit.x, unit.y));
                        if !self.tile(unit.x, unit.y).road && !tile_is_city {
                            if !self.begin_work(id, WorkKind::Road)
                                && let Some(index) = self.unit_index_by_id(id)
                            {
                                self.units[index].order = None;
                            }
                        } else if let Some(index) = self.unit_index_by_id(id) {
                            self.units[index].order = None;
                        }
                        continue;
                    }
                    if unit.work_left > 0 {
                        continue;
                    }
                    let tile_is_city = self
                        .cities
                        .iter()
                        .any(|city| (city.x, city.y) == (unit.x, unit.y));
                    if !self.tile(unit.x, unit.y).road && !tile_is_city {
                        if !self.begin_work(id, WorkKind::Road)
                            && let Some(index) = self.unit_index_by_id(id)
                        {
                            self.units[index].order = None;
                        }
                        continue;
                    }
                    let path = self.plan_route(&unit, x, y, true);
                    if path.is_empty() {
                        if let Some(index) = self.unit_index_by_id(id) {
                            self.units[index].order = None;
                        }
                        continue;
                    }
                    for (dx, dy) in path {
                        let Some(direction) = CIV3_DIRS.iter().position(|&step| step == (dx, dy))
                        else {
                            break;
                        };
                        if self.try_move_unit(id, direction) == MoveResult::Blocked {
                            break;
                        }
                        let Some(unit) = self.unit_by_id(id) else {
                            break;
                        };
                        let tile_is_city = self
                            .cities
                            .iter()
                            .any(|city| (city.x, city.y) == (unit.x, unit.y));
                        if !self.tile(unit.x, unit.y).road && !tile_is_city {
                            if !self.begin_work(id, WorkKind::Road)
                                && let Some(index) = self.unit_index_by_id(id)
                            {
                                self.units[index].order = None;
                            }
                            break;
                        }
                        if unit.moves_left <= 0 {
                            break;
                        }
                    }
                }
            }
        }
    }

    pub(super) fn continue_player_gotos(&mut self) {
        let mut groups: BTreeMap<PlayerGotoGroupKey, Vec<u32>> = BTreeMap::new();
        for (&id, &goal) in &self.unit_goto_tiles {
            let Some(unit) = self.unit_by_id(id).filter(|unit| unit.owner == HUMAN) else {
                continue;
            };
            let position = (unit.x, unit.y);
            if position == goal {
                continue;
            }
            groups.entry((position, goal)).or_default().push(id);
        }
        for ((_, (x, y)), ids) in groups {
            let shared_moves = ids
                .iter()
                .filter_map(|id| self.unit_by_id(*id).map(|unit| unit.moves_left))
                .min()
                .unwrap_or(0);
            for id in ids {
                let Some(index) = self.unit_index_by_id(id) else {
                    continue;
                };
                let original_moves = self.units[index].moves_left;
                let original_position = (self.units[index].x, self.units[index].y);
                self.units[index].moves_left = shared_moves;
                if let Some(unit) = self.unit_by_id(id) {
                    let path = self.plan_route(unit, x, y, true);
                    if !path.is_empty() {
                        self.follow_grid_path(id, path);
                    }
                }
                if let Some(index) = self.unit_index_by_id(id) {
                    let current_position = (self.units[index].x, self.units[index].y);
                    if current_position == (x, y) {
                        self.unit_goto_tiles.remove(&id);
                    }
                    if current_position != original_position {
                        let spent = (shared_moves - self.units[index].moves_left).max(0);
                        self.units[index].moves_left = (original_moves - spent).max(0);
                    } else {
                        self.units[index].moves_left = original_moves;
                    }
                }
            }
        }
        let finished: Vec<_> = self
            .unit_goto_tiles
            .iter()
            .filter_map(|(&id, &(x, y))| {
                self.unit_by_id(id)
                    .is_some_and(|unit| (unit.x, unit.y) == (x, y))
                    .then_some(id)
            })
            .collect();
        for id in finished {
            self.unit_goto_tiles.remove(&id);
        }
        self.continue_unit_orders();
    }

    /// Follows a path over Civ III's eight-direction grid.
    pub(super) fn follow_grid_path(
        &mut self,
        unit_id: u32,
        path: impl IntoIterator<Item = (i32, i32)>,
    ) -> bool {
        let defer_visibility = self
            .unit_by_id(unit_id)
            .is_some_and(|unit| unit.owner != HUMAN);
        let visibility_batch = self.visibility_refresh.begin_batch(defer_visibility);
        let mut moved = false;
        for (dx, dy) in path {
            let Some(direction) = CIV3_DIRS.iter().position(|&step| step == (dx, dy)) else {
                break;
            };
            let result = self.try_move_unit(unit_id, direction);
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
        if self.visibility_refresh.end_batch(visibility_batch) {
            let _profile = self
                .trace
                .profiler()
                .scope("movement.follow_path.flush_visibility");
            self.flush_deferred_visibility();
        }
        moved
    }
}

#[cfg(test)]
#[path = "movement/tests.rs"]
mod tests;
