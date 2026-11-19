//! Last observations are campaign state, not a second simulation.
//! Native server/maphand.c copies the giver's private tile, while
//! server/citytools.c::update_dumb_city retains public city graphics until seen again.
use super::*;
use crate::rules::TechEra;
use crate::rules::engine_rules::RequirementKind;
use crate::rules::engine_rules::campaign_effect;
use crate::rules::engine_rules::{
    EffectContext, Requirement, RequirementContext, RequirementEvaluationError, RequirementRange,
    RequirementValue, ResolvedRequirementValue,
};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sim) enum UnitVisionLayer {
    Main,
    #[allow(dead_code)] // Retained engine layer; native stealth aircraft use main vision.
    Stealth,
    Subsurface,
}

impl UnitVisionLayer {
    pub(in crate::sim) fn requires_detection(self) -> bool {
        !matches!(self, Self::Main)
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ObservedCity {
    pub(crate) city: City,
    pub(crate) civ3_style: u8,
    pub(crate) civ3_era: u8,
    pub(crate) occupied: bool,
    pub(crate) disorder: bool,
    pub(crate) remembered: bool,
}

pub(super) type PartialCitySites = BTreeMap<(usize, usize), usize>;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct ObservedMap {
    pub(super) revision: u64,
    pub(super) tiles: BTreeMap<(usize, usize), (u64, Tile)>,
    pub(super) cities: BTreeMap<(usize, u32), (u64, ObservedCity)>,
    #[serde(skip)]
    pub(super) city_owners: BTreeMap<(usize, usize), usize>,
    // A legacy route can identify the owner at a city site without a city
    // record. Keep that observation until this observer sees the tile again.
    #[serde(skip)]
    pub(super) partial_sites: PartialCitySites,
}

impl Simulation {
    pub(crate) fn reveals_cities(&self, owner: usize) -> bool {
        self.effect_value_for_player(owner, EffectKind::RevealCities) > 0
    }

    pub(crate) fn reveals_map(&self, owner: usize) -> bool {
        self.effect_value_for_player(owner, EffectKind::RevealMap) > 0
    }

    pub(crate) fn unit_vision_radius_sq_for(
        &self,
        owner: usize,
        kind: UnitKind,
        x: i32,
        y: i32,
    ) -> i32 {
        let base = native_unit_vision_radius_sq(kind);
        let context = VisionRequirementContext {
            simulation: self,
            owner,
            unit: Some(kind),
            tile: Some((x, y)),
        };
        let bonus = campaign_effect(EffectKind::UnitVisionRadiusSq, &context);
        i32::try_from(i64::from(base) + bonus)
            .unwrap_or_else(|_| panic!("unit vision radius is outside i32 range"))
    }

    pub(in crate::sim) fn unit_vision_layer_for(&self, kind: UnitKind) -> UnitVisionLayer {
        native_unit_vision_layer(kind)
    }

    pub(crate) fn city_vision_radius_sq(&self, owner: usize) -> i32 {
        let context = VisionRequirementContext {
            simulation: self,
            owner,
            unit: None,
            tile: None,
        };
        i32::try_from(campaign_effect(EffectKind::CityVisionRadiusSq, &context))
            .unwrap_or_else(|_| panic!("city vision radius is outside i32 range"))
    }

    fn effect_value_for_player(&self, owner: usize, kind: EffectKind) -> i64 {
        let context = VisionRequirementContext {
            simulation: self,
            owner,
            unit: None,
            tile: None,
        };
        campaign_effect(kind, &context)
    }

    pub(crate) fn has_main_sight_at(&self, observer: usize, x: i32, y: i32) -> bool {
        self.units.iter().any(|unit| {
            self.vision_recipients(unit.owner).contains(&observer)
                && self.within_vision_radius_sq(
                    (unit.x, unit.y),
                    (x, y),
                    self.unit_vision_radius_sq_for(unit.owner, unit.kind, unit.x, unit.y),
                )
        }) || self.cities.iter().any(|city| {
            self.vision_recipients(city.owner).contains(&observer)
                && self.within_vision_radius_sq(
                    (city.x, city.y),
                    (x, y),
                    self.city_vision_radius_sq(city.owner),
                )
        })
    }

    pub(crate) fn unit_is_visible_from_sources(&self, observer: usize, target: &Unit) -> bool {
        let mut visible = false;
        let mut stealth_visible = false;
        let requires_detection = self.unit_vision_layer_for(target.kind).requires_detection();
        for viewer in &self.units {
            if !self.vision_recipients(viewer.owner).contains(&observer) {
                continue;
            }
            let radius_sq =
                self.unit_vision_radius_sq_for(viewer.owner, viewer.kind, viewer.x, viewer.y);
            visible |=
                self.within_vision_radius_sq((viewer.x, viewer.y), (target.x, target.y), radius_sq);
            stealth_visible |= self.within_vision_radius_sq(
                (viewer.x, viewer.y),
                (target.x, target.y),
                radius_sq.clamp(0, 2),
            );
            if visible
                && (self.allied(observer, target.owner) || !requires_detection || stealth_visible)
            {
                return true;
            }
        }
        for city in &self.cities {
            if !self.vision_recipients(city.owner).contains(&observer) {
                continue;
            }
            let radius_sq = self.city_vision_radius_sq(city.owner);
            let distance_visible =
                self.within_vision_radius_sq((city.x, city.y), (target.x, target.y), radius_sq);
            stealth_visible |= self.within_vision_radius_sq(
                (city.x, city.y),
                (target.x, target.y),
                radius_sq.clamp(0, 2),
            );
            if distance_visible {
                visible = true;
                if visible
                    && (self.allied(observer, target.owner)
                        || !requires_detection
                        || stealth_visible)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Use the Civ III grid's shortest step count as Freeciv's hex real
    /// distance, then square it for the ruleset's `radius_sq` values.
    pub(crate) fn within_vision_radius_sq(
        &self,
        from: (i32, i32),
        to: (i32, i32),
        radius_sq: i32,
    ) -> bool {
        let distance = self.map_distance(from.0, from.1, to.0, to.1);
        self.within_vision_distance_sq(distance, radius_sq)
    }

    pub(crate) fn within_vision_distance_sq(&self, distance: i32, radius_sq: i32) -> bool {
        distance * distance <= radius_sq
    }

    pub(in crate::sim) fn refresh_visibility(&mut self) {
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("visibility.refresh.reset");
        for tile in &mut self.tiles {
            tile.visible = false;
            tile.stealth_visible = false;
        }
        for owner in 0..self.players.len() {
            if self.reveals_map(owner) {
                for tile in &mut self.tiles {
                    tile.known_by.set(owner, true);
                    if owner == HUMAN {
                        tile.seen = true;
                    }
                }
            }
        }
        drop(_profile);
        let revealers: Vec<_> = self
            .units
            .iter()
            .map(|u| {
                let radius = self.unit_vision_radius_sq_for(u.owner, u.kind, u.x, u.y);
                (u.owner, u.x, u.y, radius, radius.clamp(0, 2))
            })
            .chain(
                self.cities
                    .iter()
                    .map(|c| (c.owner, c.x, c.y, self.city_vision_radius_sq(c.owner), 2)),
            )
            .collect();
        let recipients: Vec<_> = (0..self.players.len())
            .map(|owner| self.vision_recipients(owner))
            .collect();
        let mut render_seen = vec![vec![false; self.tiles.len()]; self.players.len()];
        let _profile = profiler.scope("visibility.refresh.revealers");
        for (owner, x, y, radius_sq, stealth_sq) in revealers {
            let radius = radius_sq.max(0).isqrt();
            for (visible_x, visible_y, distance) in self.map_wrap.civ3_radius(x, y, radius) {
                if !self.within_vision_distance_sq(distance, radius_sq) {
                    continue;
                }
                let idx = self.map_wrap.index(visible_x, visible_y);
                if owner != BARBARIANS {
                    for &target in &recipients[owner] {
                        self.tiles[idx].known_by.set(target, true);
                        render_seen[target][idx] = true;
                    }
                }
                if recipients[owner].contains(&HUMAN) {
                    self.tiles[idx].visible = true;
                    self.tiles[idx].stealth_visible |=
                        self.within_vision_distance_sq(distance, stealth_sq);
                    self.tiles[idx].seen = true;
                }
            }
        }
        drop(_profile);
        let city_reveal_owners: Vec<_> = (0..self.players.len())
            .filter(|&owner| self.reveals_cities(owner))
            .collect();
        let _profile = profiler.scope("visibility.refresh.city_reveal");
        for owner in city_reveal_owners {
            for city in &self.cities {
                let tile = &mut self.tiles[self.map_wrap.index(city.x, city.y)];
                tile.known_by.set(owner, true);
                tile.seen |= owner == HUMAN;
            }
        }
        drop(_profile);
        let _profile = profiler.scope("visibility.refresh.reconcile_workers");
        self.reconcile_city_workers();
        drop(_profile);
        let _profile = profiler.scope("visibility.refresh.remember_borders");
        self.remember_visible_borders();
        drop(_profile);
        let _profile = profiler.scope("visibility.refresh.remember_render_map");
        let render_scope = self.visibility_render_scope.take();
        self.trace.count_operation(
            if render_scope.is_some() {
                "visibility.refresh.scoped"
            } else {
                "visibility.refresh.unscoped"
            },
            1,
        );
        self.remember_render_positions_with_sight(&render_seen, render_scope.as_deref(), true);
        drop(_profile);
        let _profile = profiler.scope("visibility.refresh.share_render_memory");
        self.share_render_memory(render_scope.as_deref());
    }

    pub(super) fn record_visibility_render_scope(
        &mut self,
        unit: &Unit,
        from: (i32, i32),
        to: (i32, i32),
    ) {
        let tile_count = self.tiles.len();
        if self.visibility_pending_render_tiles.len() != tile_count {
            self.visibility_pending_render_tiles = vec![false; tile_count];
        }
        for (x, y) in [from, to] {
            let radius_sq = self.unit_vision_radius_sq_for(unit.owner, unit.kind, x, y);
            let radius = radius_sq.max(0).isqrt();
            for (tx, ty, distance) in self.map_wrap.civ3_radius(x, y, radius) {
                if self.within_vision_distance_sq(distance, radius_sq) {
                    let index = self.map_wrap.index(tx, ty);
                    self.visibility_pending_render_tiles[index] = true;
                }
            }
        }
    }

    pub(super) fn flush_deferred_visibility(&mut self) {
        if self.visibility_refresh_deferred || !self.visibility_refresh_pending {
            return;
        }
        self.visibility_refresh_pending = false;
        let scope = self
            .visibility_pending_render_tiles
            .iter_mut()
            .enumerate()
            .filter_map(|(index, visible)| {
                if *visible {
                    *visible = false;
                    Some(index)
                } else {
                    None
                }
            })
            .collect();
        self.visibility_render_scope = Some(scope);
        self.refresh_visibility();
        self.visibility_render_scope = None;
    }

    pub(super) fn human_known_tile(&self, x: i32, y: i32) -> Option<&Tile> {
        self.known_tile_for(HUMAN, x, y)
    }

    pub(super) fn known_tile_for(&self, observer: usize, x: i32, y: i32) -> Option<&Tile> {
        let tile = self.tile(x, y);
        if observer == HUMAN && tile.visible {
            return Some(tile);
        }
        let known = if observer == HUMAN {
            tile.seen
        } else {
            tile.known_by[observer]
        };
        known
            .then(|| self.remembered_tile(observer, x, y))
            .flatten()
    }

    /// Render projections keep the knowledge flag separate from their tile data.
    pub(crate) fn render_tile(&self, x: i32, y: i32) -> &Tile {
        self.human_known_tile(x, y)
            .unwrap_or_else(|| self.tile(x, y))
    }

    pub(super) fn observed_city_owner(&self, observer: usize, x: i32, y: i32) -> Option<usize> {
        let index = self.map_wrap.index(x, y);
        if observer == HUMAN && self.tiles[index].visible {
            let (x, y) = self.map_wrap.position(index);
            return self.city_at(x, y).map(|city| city.owner);
        }
        self.observed_map
            .city_owners
            .get(&(observer, index))
            .copied()
    }

    pub(super) fn rebuild_observed_city_index(&mut self) {
        self.observed_map
            .city_owners
            .clone_from(&self.observed_map.partial_sites);
        self.observed_map
            .city_owners
            .extend(
                self.observed_map
                    .cities
                    .iter()
                    .map(|(&(observer, _), (_, observed))| {
                        let city = &observed.city;
                        ((observer, self.map_wrap.index(city.x, city.y)), city.owner)
                    }),
            );
    }

    pub(super) fn import_legacy_routes(&mut self, routes: Vec<Option<movement::RouteTile>>) {
        for (index, route) in routes.into_iter().enumerate() {
            if let Some(route) = route.filter(|_| self.tiles[index].seen) {
                let owner = self.tiles[index].last_seen_owner;
                self.import_partial_tile(HUMAN, index, route, owner);
            }
        }
    }

    /// Older formats could preserve routing information without a full tile.
    /// Build a snapshot using the fields actually known to the observer.
    pub(super) fn import_partial_tile(
        &mut self,
        observer: usize,
        index: usize,
        route: movement::RouteTile,
        owner: Option<usize>,
    ) {
        if let Some(owner) = route.city_owner
            && !self
                .observed_map
                .city_owners
                .contains_key(&(observer, index))
        {
            self.observed_map
                .partial_sites
                .insert((observer, index), owner);
        }
        self.observed_map
            .tiles
            .entry((observer, index))
            .or_insert_with(|| {
                let base_terrain = match route.terrain {
                    Terrain::Coast | Terrain::Lake => TerrainBase::Coast,
                    Terrain::Sea => TerrainBase::Sea,
                    Terrain::Ocean => TerrainBase::Ocean,
                    Terrain::Plains => TerrainBase::Plains,
                    Terrain::Desert | Terrain::FloodPlains => TerrainBase::Desert,
                    Terrain::Tundra => TerrainBase::Tundra,
                    Terrain::Glacier => TerrainBase::Ice,
                    _ => TerrainBase::Grassland,
                };
                let mut snapshot = Tile::from(crate::world::GeneratedTile {
                    terrain: route.terrain,
                    base_terrain,
                    altitude: 0,
                    hut: false,
                    river: route.river,
                    river_edges: 0,
                    resource: None,
                });
                snapshot.seen = true;
                snapshot.known_by.set(observer, true);
                snapshot.owner = owner;
                snapshot.road = route.road;
                snapshot.railroad = route.railroad;
                (0, snapshot)
            });
    }

    #[cfg(test)]
    pub(super) fn remember_visible_human_tiles(&mut self) {
        let visible = self.tiles.iter().map(|tile| tile.visible).collect();
        self.remember_render_map(&[visible]);
    }

    pub(crate) fn remembered_tile(&self, owner: usize, x: i32, y: i32) -> Option<&Tile> {
        self.observed_map
            .tiles
            .get(&(owner, self.map_wrap.index(x, y)))
            .map(|(_, tile)| tile)
    }

    pub(crate) fn observed_city(&self, city: &City) -> ObservedCity {
        self.observed_city_with_occupancy(
            city,
            self.units.iter().any(|u| (u.x, u.y) == (city.x, city.y)),
        )
    }

    fn observed_city_with_occupancy(&self, city: &City, occupied: bool) -> ObservedCity {
        // Appearance needs luxury and mood, but not the forecast's upkeep scans/sorts.
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("observed_city.yields");
        let (_, _, trade) = self.city_yields(city);
        drop(_profile);
        let _profile = profiler.scope("observed_city.trade");
        let luxury = self.city_trade(city, trade)[2];
        drop(_profile);
        let _profile = profiler.scope("observed_city.era");
        let civ3_era = Tech::all()
            .filter(|&tech| self.has_tech(city.owner, tech))
            .map(|tech| match tech.era() {
                TechEra::Ancient => 0,
                TechEra::MiddleAges => 1,
                TechEra::Industrial => 2,
                TechEra::Modern => 3,
            })
            .max()
            .unwrap_or(0);
        drop(_profile);
        let _profile = profiler.scope("observed_city.mood");
        let disorder = self.city_in_disorder(city, luxury);
        drop(_profile);
        ObservedCity {
            city: city.clone(),
            // Civ III supplies five cultural sheets; the default mapping is
            // data on each civilization and can be overridden by its theme.
            civ3_style: self.players[city.owner]
                .civilization_id
                .as_deref()
                .and_then(crate::rules::civilizations::by_id)
                .map(|civilization| civilization.civ3_city_style)
                .unwrap_or(2),
            civ3_era,
            occupied,
            disorder,
            remembered: false,
        }
    }

    // Resolve live versus remembered cities once, without cloning city state or
    // computing appearance. Minimap markers only need these borrowed records.
    fn render_city_sources(&self) -> BTreeMap<u32, (&City, Option<&ObservedCity>)> {
        let reveal = self.reveals_cities(HUMAN);
        let mut cities: BTreeMap<_, _> = self
            .observed_map
            .cities
            .iter()
            .filter(|((owner, _), (_, c))| {
                *owner == HUMAN
                    && self.tile(c.city.x, c.city.y).seen
                    && !self.tile(c.city.x, c.city.y).visible
            })
            .map(|((_, id), (_, c))| (*id, (&c.city, Some(c))))
            .collect();
        for city in &self.cities {
            if city.owner == HUMAN
                || self.tile(city.x, city.y).visible
                || (reveal && self.tile(city.x, city.y).seen)
            {
                cities.insert(city.id, (city, None));
            }
        }
        cities
    }

    pub(crate) fn render_city_markers(&self) -> Vec<&City> {
        self.render_city_sources()
            .into_values()
            .map(|(city, _)| city)
            .collect()
    }

    pub(crate) fn render_cities(&self) -> Vec<ObservedCity> {
        self.render_cities_where(|_| true)
    }

    pub(crate) fn render_cities_where(&self, include: impl Fn(&City) -> bool) -> Vec<ObservedCity> {
        let occupied: std::collections::HashSet<_> =
            self.units.iter().map(|u| (u.x, u.y)).collect();
        self.render_city_sources()
            .into_values()
            .filter(|(city, _)| include(city))
            .map(|(city, remembered)| {
                remembered.cloned().unwrap_or_else(|| {
                    self.observed_city_with_occupancy(city, occupied.contains(&(city.x, city.y)))
                })
            })
            .collect()
    }

    pub(super) fn remember_render_map(&mut self, visible: &[Vec<bool>]) {
        self.remember_render_positions(visible, None);
    }

    fn remember_render_positions(&mut self, visible: &[Vec<bool>], scope: Option<&[usize]>) {
        self.remember_render_positions_with_sight(visible, scope, false);
    }

    fn remember_render_positions_with_sight(
        &mut self,
        visible: &[Vec<bool>],
        scope: Option<&[usize]>,
        visible_is_current_sight: bool,
    ) {
        self.observed_map.revision = self.observed_map.revision.saturating_add(1);
        let revision = self.observed_map.revision;
        let mut occupied = vec![false; self.tiles.len()];
        for unit in &self.units {
            occupied[self.map_wrap.index(unit.x, unit.y)] = true;
        }
        let mut city_snapshots = vec![None; self.cities.len()];
        let mut index_dirty = false;
        for (owner, visible) in visible.iter().enumerate() {
            if owner == BARBARIANS {
                continue;
            }
            // Looking at a now-empty city tile removes only that observer's memory.
            let _prune_profile = self
                .trace
                .profiler()
                .scope("visibility.render_memory.city_prune");
            let indexed_before =
                self.observed_map.cities.len() + self.observed_map.partial_sites.len();
            self.observed_map.cities.retain(|(viewer, _), (_, c)| {
                let index = self.map_wrap.index(c.city.x, c.city.y);
                *viewer != owner
                    || scope.is_some_and(|indices| indices.binary_search(&index).is_err())
                    || !visible[index]
            });
            self.observed_map
                .partial_sites
                .retain(|&(viewer, index), _| {
                    viewer != owner
                        || scope.is_some_and(|indices| indices.binary_search(&index).is_err())
                        || !visible[index]
                });
            index_dirty |= indexed_before
                != self.observed_map.cities.len() + self.observed_map.partial_sites.len();
            drop(_prune_profile);
            let _tile_profile = self
                .trace
                .profiler()
                .scope("visibility.render_memory.tile_snapshots");
            if let Some(indices) = scope {
                for &index in indices {
                    if visible[index] {
                        let mut tile = self.tiles[index].clone();
                        tile.seen = true;
                        tile.visible = false;
                        tile.stealth_visible = false;
                        self.observed_map
                            .tiles
                            .insert((owner, index), (revision, tile));
                    }
                }
            } else {
                for (index, &is_visible) in visible.iter().enumerate() {
                    if is_visible {
                        let mut tile = self.tiles[index].clone();
                        tile.seen = true;
                        tile.visible = false;
                        tile.stealth_visible = false;
                        self.observed_map
                            .tiles
                            .insert((owner, index), (revision, tile));
                    }
                }
            }
            drop(_tile_profile);
            let _city_profile = self
                .trace
                .profiler()
                .scope("visibility.render_memory.city_snapshots");
            for (city_index, city) in self.cities.iter().enumerate() {
                let index = self.map_wrap.index(city.x, city.y);
                if scope.is_none_or(|indices| indices.binary_search(&index).is_ok())
                    && (visible[index] || self.reveals_cities(owner))
                {
                    // City visibility and unit visibility are independent.
                    // A map-show operation can reveal a city site while its
                    // garrison remains hidden.
                    let has_sight = if visible_is_current_sight {
                        visible[index]
                    } else {
                        self.has_main_sight_at(owner, city.x, city.y)
                    };
                    let _appearance_profile = self
                        .trace
                        .profiler()
                        .scope("visibility.render_memory.city_appearance");
                    let base = city_snapshots[city_index]
                        .get_or_insert_with(|| self.observed_city_with_occupancy(city, false));
                    drop(_appearance_profile);
                    let mut observed = base.clone();
                    observed.occupied = has_sight && occupied[index];
                    observed.remembered = true;
                    let _store_profile = self
                        .trace
                        .profiler()
                        .scope("visibility.render_memory.city_store");
                    let replaced = self
                        .observed_map
                        .cities
                        .insert((owner, city.id), (revision, observed));
                    // The owner index only reads a city's position and owner.
                    index_dirty |= replaced.is_none_or(|(_, old)| {
                        (old.city.x, old.city.y, old.city.owner) != (city.x, city.y, city.owner)
                    });
                    drop(_store_profile);
                }
            }
        }
        if index_dirty {
            self.rebuild_observed_city_index();
        }
    }

    /// Reveal a remembered map circle without granting current sight or unit
    /// vision. Freeciv's `server/maphand.c::map_show_circle` updates tile and
    /// city memories while leaving off-screen units hidden.
    pub(in crate::sim) fn show_map_circle_to_player(
        &mut self,
        owner: usize,
        x: i32,
        y: i32,
        unit_kind: UnitKind,
    ) {
        // Freeciv's map_show_circle uses the unit type's base sight radius.
        self.reveal_map_circle(owner, x, y, native_unit_vision_radius_sq(unit_kind));
    }

    pub(in crate::sim) fn reveal_map_circle(
        &mut self,
        owner: usize,
        x: i32,
        y: i32,
        radius_sq: i32,
    ) {
        if owner >= self.players.len() || owner == BARBARIANS {
            return;
        }
        let radius = radius_sq.max(0).isqrt();
        let mut indices = self
            .map_wrap
            .civ3_radius(x, y, radius)
            .into_iter()
            .filter(|(_, _, distance)| distance * distance <= radius_sq.max(0))
            .map(|(tx, ty, _)| self.map_wrap.index(tx, ty))
            .collect::<Vec<_>>();
        // remember_render_positions_with_sight uses binary_search on scoped
        // indices when storing and pruning city memories. civ3_radius returns
        // breadth-first order, not index order.
        indices.sort_unstable();
        let recipients = self.vision_recipients(owner);
        let mut remembered = vec![vec![false; self.tiles.len()]; self.players.len()];
        for &index in &indices {
            let (tx, ty) = self.map_wrap.position(index);
            for &recipient in &recipients {
                if recipient == BARBARIANS || self.has_main_sight_at(recipient, tx, ty) {
                    continue;
                }
                let tile = &mut self.tiles[index];
                tile.known_by.set(recipient, true);
                if recipient == HUMAN {
                    tile.seen = true;
                }
                remembered[recipient][index] = true;
            }
        }
        self.remember_render_positions(&remembered, Some(&indices));
        for &index in &indices {
            if remembered[HUMAN][index] {
                self.tiles[index].last_seen_owner = self.tiles[index].owner;
            }
        }
    }

    /// `scope` (sorted tile indices) limits sharing to tiles this refresh could
    /// have changed; `None` shares everything.
    pub(super) fn share_render_memory(&mut self, scope: Option<&[usize]>) {
        // Map-reveal effects can grant knowledge without direct sight. Capture
        // snapshots for direct reveals before moving any private knowledge.
        let revealers: Vec<_> = (0..self.players.len())
            .filter(|&owner| owner != BARBARIANS && self.reveals_map(owner))
            .collect();
        for (index, tile) in self
            .tiles
            .iter()
            .enumerate()
            .filter(|_| !revealers.is_empty())
        {
            for &owner in &revealers {
                if self.observed_map.tiles.contains_key(&(owner, index)) {
                    continue;
                }
                let mut snapshot = tile.clone();
                snapshot.seen = true;
                snapshot.visible = false;
                snapshot.stealth_visible = false;
                self.observed_map
                    .tiles
                    .insert((owner, index), (self.observed_map.revision, snapshot));
            }
        }
        // Sharing transfers historical knowledge as well as current sight.
        let shares: Vec<_> = (0..self.players.len())
            .flat_map(|giver| {
                self.vision_recipients(giver)
                    .into_iter()
                    .filter(move |receiver| *receiver != giver)
                    .map(move |receiver| (giver, receiver))
            })
            .collect();
        for (giver, receiver) in shares {
            self.transfer_map_observations_in(giver, receiver, TreatyMap::World, |_| true, scope);
        }
    }

    pub(super) fn transfer_map_observations(
        &mut self,
        giver: usize,
        receiver: usize,
        map: TreatyMap,
        include: impl Fn(usize) -> bool,
    ) {
        self.transfer_map_observations_in(giver, receiver, map, include, None);
    }

    /// With a sorted `scope`, only those tiles are examined (in the same
    /// ascending order a full range scan would visit them).
    fn transfer_map_observations_in(
        &mut self,
        giver: usize,
        receiver: usize,
        map: TreatyMap,
        include: impl Fn(usize) -> bool,
        scope: Option<&[usize]>,
    ) {
        if giver == receiver || receiver == BARBARIANS {
            return;
        }
        let wanted = |index: usize, revision: u64| {
            let tile = &self.tiles[index];
            include(index)
                && tile.known_by[giver]
                && (map != TreatyMap::Sea || tile.terrain.is_water())
                && !(receiver == HUMAN && tile.visible)
                && self
                    .observed_map
                    .tiles
                    .get(&(receiver, index))
                    .is_none_or(|old| !tile.known_by[receiver] || old.0 < revision)
        };
        let tiles: Vec<_> = match scope {
            Some(scope) => scope
                .iter()
                .filter_map(|&index| {
                    let entry = self.observed_map.tiles.get(&(giver, index))?;
                    wanted(index, entry.0).then(|| (index, entry.clone()))
                })
                .collect(),
            None => self
                .observed_map
                .tiles
                .range((giver, 0)..=(giver, usize::MAX))
                .filter(|((_, index), (revision, _))| wanted(*index, *revision))
                .map(|((_, index), entry)| (*index, entry.clone()))
                .collect(),
        };

        let mut transferred = std::collections::BTreeSet::new();
        for (index, entry) in tiles {
            let tile = &mut self.tiles[index];
            tile.known_by.set(receiver, true);
            if receiver == HUMAN {
                tile.seen = true;
                tile.last_seen_owner = entry.1.owner;
            }
            self.observed_map.tiles.insert((receiver, index), entry);
            transferred.insert(index);
        }
        if transferred.is_empty() {
            return;
        }
        for &index in &transferred {
            if let Some(&owner) = self.observed_map.partial_sites.get(&(giver, index)) {
                self.observed_map
                    .partial_sites
                    .insert((receiver, index), owner);
            } else {
                self.observed_map.partial_sites.remove(&(receiver, index));
            }
        }
        // A newer observation of an empty tile must also clear a stale city.
        self.observed_map.cities.retain(|(viewer, _), (_, city)| {
            *viewer != receiver
                || !transferred.contains(&self.map_wrap.index(city.city.x, city.city.y))
        });
        let cities: Vec<_> = self
            .observed_map
            .cities
            .iter()
            .filter(|((owner, _), (_, c))| {
                *owner == giver && transferred.contains(&self.map_wrap.index(c.city.x, c.city.y))
            })
            .map(|((_, id), entry)| (*id, entry.clone()))
            .collect();
        for (id, city) in cities {
            self.observed_map.cities.insert((receiver, id), city);
        }
        self.rebuild_observed_city_index();
    }

    pub(super) fn valid_render_memory(&self) -> bool {
        self.observed_map
            .tiles
            .iter()
            .all(|(&(owner, index), (revision, tile))| {
                owner < self.players.len()
                    && index < self.tiles.len()
                    && *revision <= self.observed_map.revision
                    && !tile.visible
                    && !tile.stealth_visible
                    && tile.seen
                    && tile.owner.is_none_or(|owner| owner < self.players.len())
            })
            && self
                .observed_map
                .cities
                .iter()
                .all(|(&(viewer, id), (revision, observed))| {
                    let city = &observed.city;
                    viewer < self.players.len()
                        && city.owner < self.players.len()
                        && id == city.id
                        && id > 0
                        && id < self.next_city_id
                        && (0..self.map_wrap.width()).contains(&city.x)
                        && (0..self.map_wrap.height()).contains(&city.y)
                        && city.size > 0
                        && *revision <= self.observed_map.revision
                        && observed.remembered
                        && observed.civ3_style < 5
                        && observed.civ3_era < 4
                })
    }
}

struct VisionRequirementContext<'a> {
    simulation: &'a Simulation,
    owner: usize,
    unit: Option<UnitKind>,
    tile: Option<(i32, i32)>,
}

impl RequirementContext for VisionRequirementContext<'_> {
    fn is_present(&self, requirement: &Requirement) -> Result<bool, RequirementEvaluationError> {
        let unsupported = || RequirementEvaluationError::Unsupported {
            kind: requirement.kind.name().to_owned(),
            name: requirement.name.clone(),
        };
        match requirement.kind {
            RequirementKind::Tech if requirement.range == RequirementRange::Player => {
                let tech = match &requirement.name {
                    _ if matches!(
                        requirement.resolved,
                        Some(ResolvedRequirementValue::Tech(_))
                    ) =>
                    {
                        match requirement.resolved {
                            Some(ResolvedRequirementValue::Tech(tech)) => tech,
                            _ => unreachable!(),
                        }
                    }
                    RequirementValue::String(name) => Tech::all()
                        .find(|tech| tech.name().eq_ignore_ascii_case(name))
                        .ok_or_else(unsupported)?,
                    _ => return Err(unsupported()),
                };
                Ok(self.simulation.has_tech(self.owner, tech))
            }
            RequirementKind::Building if requirement.range == RequirementRange::Player => {
                let building = match &requirement.name {
                    _ if matches!(
                        requirement.resolved,
                        Some(ResolvedRequirementValue::Building(_))
                    ) =>
                    {
                        match requirement.resolved {
                            Some(ResolvedRequirementValue::Building(building)) => building,
                            _ => unreachable!(),
                        }
                    }
                    RequirementValue::String(name) => Building::all()
                        .find(|building| building.data().name.eq_ignore_ascii_case(name))
                        .ok_or_else(unsupported)?,
                    _ => return Err(unsupported()),
                };
                Ok(self.simulation.has_active_wonder(self.owner, building))
            }
            RequirementKind::Extra if requirement.range == RequirementRange::Tile => {
                let name = match &requirement.name {
                    RequirementValue::String(name) => name.as_str(),
                    _ => return Err(unsupported()),
                };
                let (x, y) = self.tile.ok_or_else(unsupported)?;
                Ok(match name.to_ascii_lowercase().as_str() {
                    "fortress" => self.simulation.tile(x, y).base == Some(Base::Fortress),
                    "airbase" => self.simulation.tile(x, y).base == Some(Base::Airbase),
                    _ => return Err(unsupported()),
                })
            }
            RequirementKind::Terrain if requirement.range == RequirementRange::Tile => {
                let (x, y) = self.tile.ok_or_else(unsupported)?;
                match &requirement.name {
                    _ if matches!(
                        requirement.resolved,
                        Some(ResolvedRequirementValue::Terrain(_))
                    ) =>
                    {
                        match requirement.resolved {
                            Some(ResolvedRequirementValue::Terrain(expected)) => {
                                Ok(expected == self.simulation.tile(x, y).terrain)
                            }
                            _ => unreachable!(),
                        }
                    }
                    RequirementValue::String(name) => {
                        Ok(name.eq_ignore_ascii_case(self.simulation.tile(x, y).terrain.name()))
                    }
                    _ => Err(unsupported()),
                }
            }
            RequirementKind::UnitClassFlag if requirement.range == RequirementRange::Local => {
                let name = match &requirement.name {
                    RequirementValue::String(name) => name.as_str(),
                    _ => return Err(unsupported()),
                };
                let unit = self.unit.ok_or_else(unsupported)?;
                Ok(name.eq_ignore_ascii_case("Ground") && unit.unit_class().is_ground())
            }
            _ => Err(unsupported()),
        }
    }
}

impl EffectContext for VisionRequirementContext<'_> {
    fn multiplier_percent(&self, _multiplier: &str) -> Option<i64> {
        None
    }
}

fn native_unit_vision_radius_sq(kind: UnitKind) -> i32 {
    // Radar sees two tiles in all directions; ordinary native sight sees one.
    if kind.conquests_rule().has_capability("Radar") {
        8
    } else {
        2
    }
}

fn native_unit_vision_layer(kind: UnitKind) -> UnitVisionLayer {
    if kind.conquests_rule().has_capability("Submarine") {
        UnitVisionLayer::Subsurface
    } else {
        // Stealth affects air interception, not surface invisibility.
        UnitVisionLayer::Main
    }
}

#[cfg(test)]
fn ruleset_unit_vision_radius_sq(
    _ruleset: &crate::rules::engine_rules::RulesetData,
    kind: UnitKind,
) -> i32 {
    native_unit_vision_radius_sq(kind)
}

#[cfg(test)]
#[path = "observed_map/tests.rs"]
mod tests;
