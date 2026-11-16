//! Last observations are campaign state, not a second simulation.
//! Native server/maphand.c copies the giver's private tile, while
//! server/citytools.c::update_dumb_city retains public city graphics until seen again.
use super::*;
use crate::rules::TechEra;
use crate::rules::freeciv::{
    EffectContext, Requirement, RequirementContext, RequirementEvaluationError, RequirementRange,
    RequirementValue, RulesetData, effect_value,
};
use std::collections::BTreeMap;
#[cfg(test)]
#[path = "observed_map/unit_sight_bindings.rs"]
mod unit_sight_bindings;
#[cfg(test)]
use unit_sight_bindings::{SightBinding, binding_for as unit_sight_binding_for};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sim) enum UnitVisionLayer {
    Main,
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

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct ObservedMap {
    revision: u64,
    tiles: BTreeMap<(usize, usize), (u64, Tile)>,
    cities: BTreeMap<(usize, u32), (u64, ObservedCity)>,
}

impl Simulation {
    pub(crate) fn reveals_cities(&self, owner: usize) -> bool {
        self.effect_value_for_player(owner, "Reveal_Cities") > 0
    }

    pub(crate) fn reveals_map(&self, owner: usize) -> bool {
        self.effect_value_for_player(owner, "Reveal_Map") > 0
    }

    pub(crate) fn unit_vision_radius_sq_for(
        &self,
        owner: usize,
        kind: UnitKind,
        x: i32,
        y: i32,
    ) -> i32 {
        let ruleset = RulesetData::campaign_default();
        let base = native_unit_vision_radius_sq(kind);
        let context = VisionRequirementContext {
            simulation: self,
            owner,
            unit: Some(kind),
            tile: Some((x, y)),
        };
        let bonus = effect_value(&ruleset.effects, "Unit_Vision_Radius_Sq", &context)
            .unwrap_or_else(|error| panic!("unit vision effect evaluation failed: {error:?}"));
        i32::try_from(i64::from(base) + bonus)
            .unwrap_or_else(|_| panic!("unit vision radius is outside i32 range"))
    }

    pub(in crate::sim) fn unit_vision_layer_for(&self, kind: UnitKind) -> UnitVisionLayer {
        native_unit_vision_layer(kind)
    }

    pub(crate) fn city_vision_radius_sq(&self, owner: usize) -> i32 {
        let ruleset = RulesetData::campaign_default();
        let context = VisionRequirementContext {
            simulation: self,
            owner,
            unit: None,
            tile: None,
        };
        i32::try_from(
            effect_value(&ruleset.effects, "City_Vision_Radius_Sq", &context)
                .unwrap_or_else(|error| panic!("city vision effect evaluation failed: {error:?}")),
        )
        .unwrap_or_else(|_| panic!("city vision radius is outside i32 range"))
    }

    fn effect_value_for_player(&self, owner: usize, kind: &str) -> i64 {
        let ruleset = RulesetData::campaign_default();
        let context = VisionRequirementContext {
            simulation: self,
            owner,
            unit: None,
            tile: None,
        };
        effect_value(&ruleset.effects, kind, &context)
            .unwrap_or_else(|error| panic!("{kind} effect evaluation failed: {error:?}"))
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
                    tile.known_by[owner] = true;
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
        let _profile = profiler.scope("visibility.refresh.knowledge_sharing");
        let sharing: Vec<_> = recipients
            .iter()
            .enumerate()
            .filter(|(_, targets)| targets.len() > 1)
            .collect();
        if !sharing.is_empty() {
            for tile in &mut self.tiles {
                let known = tile.known_by;
                for &(owner, targets) in &sharing {
                    if known[owner] {
                        for &target in targets {
                            tile.known_by[target] = true;
                        }
                    }
                }
                tile.seen |= tile.known_by[HUMAN];
            }
        }
        drop(_profile);
        let mut observations = vec![None; self.tiles.len()];
        let mut render_seen = vec![vec![false; self.tiles.len()]; self.players.len()];
        let _profile = profiler.scope("visibility.refresh.revealers");
        for (owner, x, y, radius_sq, stealth_sq) in revealers {
            let radius = (radius_sq as f32).sqrt().ceil() as i32;
            for (visible_x, visible_y, distance) in self.map_wrap.civ3_radius(x, y, radius) {
                if !self.within_vision_distance_sq(distance, radius_sq) {
                    continue;
                }
                let idx = self.map_wrap.index(visible_x, visible_y);
                if owner != BARBARIANS {
                    for &target in &recipients[owner] {
                        self.tiles[idx].known_by[target] = true;
                        render_seen[target][idx] = true;
                        if target != HUMAN && target != BARBARIANS {
                            let observation = *observations[idx]
                                .get_or_insert_with(|| self.live_map_observation(idx));
                            self.ai_map_memory.insert((target, idx), observation);
                        }
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
                tile.known_by[owner] = true;
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
        let _profile = profiler.scope("visibility.refresh.remember_routes");
        self.remember_route_tiles(false);
        drop(_profile);
        let _profile = profiler.scope("visibility.refresh.remember_rivals");
        self.remember_visible_rivals();
        drop(_profile);
        let _profile = profiler.scope("visibility.refresh.remember_render_map");
        let render_scope = self.visibility_render_scope.take();
        self.remember_render_positions_with_sight(&render_seen, render_scope.as_deref(), true);
        drop(_profile);
        let _profile = profiler.scope("visibility.refresh.share_render_memory");
        self.share_render_memory();
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
            let radius = (radius_sq as f32).sqrt().ceil() as i32;
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

    /// Knowledge flags remain current; unexplored callers must still honor `seen`.
    pub(crate) fn render_tile(&self, x: i32, y: i32) -> &Tile {
        let tile = self.tile(x, y);
        if tile.visible || !tile.seen {
            tile
        } else {
            self.observed_map
                .tiles
                .get(&(HUMAN, self.map_wrap.index(x, y)))
                .map_or(tile, |(_, remembered)| remembered)
        }
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
        for (owner, visible) in visible.iter().enumerate() {
            if owner == BARBARIANS {
                continue;
            }
            // Looking at a now-empty city tile removes only that observer's memory.
            let _prune_profile = self
                .trace
                .profiler()
                .scope("visibility.render_memory.city_prune");
            self.observed_map.cities.retain(|(viewer, _), (_, c)| {
                let index = self.map_wrap.index(c.city.x, c.city.y);
                *viewer != owner
                    || scope.is_some_and(|indices| indices.binary_search(&index).is_err())
                    || !visible[index]
            });
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
                    self.observed_map
                        .cities
                        .insert((owner, city.id), (revision, observed));
                    drop(_store_profile);
                }
            }
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
        if owner >= self.players.len() || owner == BARBARIANS {
            return;
        }
        // Freeciv map_show_circle receives the unit type's base
        // vision_radius_sq, not the effect-adjusted sight used for normal
        // vision updates.
        let radius_sq = native_unit_vision_radius_sq(unit_kind);
        let radius = (radius_sq.max(0) as f64).sqrt().ceil() as i32;
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
                tile.known_by[recipient] = true;
                if recipient == HUMAN {
                    tile.seen = true;
                }
                remembered[recipient][index] = true;
            }
        }
        self.remember_render_positions(&remembered, Some(&indices));
        for &index in &indices {
            for &recipient in &recipients {
                if !remembered[recipient][index] {
                    continue;
                }
                let owner = self.tiles[index].owner;
                let route = movement::RouteTile::from(&self.tiles[index]);
                if recipient == HUMAN {
                    self.tiles[index].last_seen_owner = owner;
                    self.route_memory[index] = Some(route);
                } else {
                    self.ai_map_memory
                        .insert((recipient, index), self.live_map_observation(index));
                }
            }
        }
    }

    pub(super) fn share_render_memory(&mut self) {
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
            self.transfer_render_map(giver, receiver, TreatyMap::World);
        }
        // Map-reveal effects can grant knowledge without direct sight. Capture
        // those newly known tiles once; shared historical snapshots take priority.
        for (index, tile) in self.tiles.iter().enumerate() {
            for owner in 0..self.players.len() {
                if owner == BARBARIANS
                    || !tile.known_by[owner]
                    || self.observed_map.tiles.contains_key(&(owner, index))
                {
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
    }

    pub(super) fn transfer_render_map(&mut self, giver: usize, receiver: usize, map: TreatyMap) {
        let tiles: Vec<_> = self
            .observed_map
            .tiles
            .iter()
            .filter(|((owner, _), (_, tile))| {
                *owner == giver && (map != TreatyMap::Sea || tile.terrain.is_water())
            })
            .map(|((_, index), entry)| (*index, entry.clone()))
            .collect();
        let mut transferred = std::collections::BTreeSet::new();
        for (index, entry) in tiles {
            if receiver == HUMAN && self.tiles[index].visible {
                continue;
            }
            if self
                .observed_map
                .tiles
                .get(&(receiver, index))
                .is_none_or(|old| old.0 < entry.0)
            {
                // Rendering and movement must consume the same dated private
                // tile. Never reconstruct the recipient's route from live terrain.
                let mut route = movement::RouteTile::from(&entry.1);
                route.city_owner = self
                    .observed_map
                    .cities
                    .iter()
                    .find(|((viewer, _), (_, observed))| {
                        *viewer == giver
                            && self.map_wrap.index(observed.city.x, observed.city.y) == index
                    })
                    .map(|(_, (_, observed))| observed.city.owner);
                let observation = MapObservation {
                    route,
                    owner: entry.1.owner,
                    ocean: entry.1.terrain.is_water(),
                };
                if receiver == HUMAN {
                    self.tiles[index].last_seen_owner = observation.owner;
                    self.route_memory[index] = Some(route);
                } else if receiver != BARBARIANS {
                    self.ai_map_memory.insert((receiver, index), observation);
                }
                self.observed_map.tiles.insert((receiver, index), entry);
                transferred.insert(index);
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
            kind: requirement.kind.clone(),
            name: requirement.name.clone(),
        };
        let name = match &requirement.name {
            RequirementValue::String(name) => name.as_str(),
            _ => return Err(unsupported()),
        };
        match requirement.kind.to_ascii_lowercase().as_str() {
            "tech" if requirement.range == RequirementRange::Player => {
                let tech = Tech::all()
                    .find(|tech| tech.name().eq_ignore_ascii_case(name))
                    .ok_or_else(unsupported)?;
                Ok(self.simulation.has_tech(self.owner, tech))
            }
            "building" if requirement.range == RequirementRange::Player => {
                let building = Building::all()
                    .find(|building| building.data().name.eq_ignore_ascii_case(name))
                    .ok_or_else(unsupported)?;
                Ok(self.simulation.has_active_wonder(self.owner, building))
            }
            "extra" if requirement.range == RequirementRange::Tile => {
                let (x, y) = self.tile.ok_or_else(unsupported)?;
                Ok(match name.to_ascii_lowercase().as_str() {
                    "fortress" => self.simulation.tile(x, y).base == Some(Base::Fortress),
                    "airbase" => self.simulation.tile(x, y).base == Some(Base::Airbase),
                    _ => return Err(unsupported()),
                })
            }
            "terrain" if requirement.range == RequirementRange::Tile => {
                let (x, y) = self.tile.ok_or_else(unsupported)?;
                Ok(name.eq_ignore_ascii_case(self.simulation.tile(x, y).terrain.name()))
            }
            "unitclassflag" if requirement.range == RequirementRange::Local => {
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
    super::action_rules::native_unit_economics_binding(kind)
        .map(|binding| binding.vision_radius_sq)
        .unwrap_or(2)
}

fn native_unit_vision_layer(kind: UnitKind) -> UnitVisionLayer {
    let layer = super::action_rules::native_unit_economics_binding(kind)
        .map_or("Main", |binding| binding.vision_layer.as_str());
    match layer.to_ascii_lowercase().as_str() {
        "main" => UnitVisionLayer::Main,
        "stealth" => UnitVisionLayer::Stealth,
        "subsurface" => UnitVisionLayer::Subsurface,
        _ => panic!("invalid native vision_layer {layer:?} for {}", kind.name()),
    }
}

#[cfg(test)]
fn ruleset_unit_vision_radius_sq(_ruleset: &RulesetData, kind: UnitKind) -> i32 {
    native_unit_vision_radius_sq(kind)
}

#[cfg(test)]
fn ruleset_unit_vision_layer(_ruleset: &RulesetData, kind: UnitKind) -> UnitVisionLayer {
    native_unit_vision_layer(kind)
}

#[allow(dead_code)] // Explicit unmapped records are checked against the selected ruleset in tests.
struct FreecivUnitVisionGap {
    freeciv_section: &'static str,
    freeciv_radius_sq: i32,
    civ3_catalog_id: Option<&'static str>,
    civ3_source: Option<&'static str>,
    fallback_radius_sq: i32,
    note: &'static str,
}

/// Freeciv C2C3 unit records whose vision has no direct Civ III unit identity.
/// Similar roles remain explicit candidates, never automatic substitutions.
#[allow(dead_code)] // Keeps Freeciv-only unit records and candidate IDs explicit for audits.
const UNMAPPED_FREECIV_UNIT_VISION: &[FreecivUnitVisionGap] = &[
    FreecivUnitVisionGap {
        freeciv_section: "unit_partisan",
        freeciv_radius_sq: 6,
        civ3_catalog_id: Some("guerilla"),
        civ3_source: Some("Conquests/conquests.biq#PRTO[75]"),
        fallback_radius_sq: 2,
        note: "Civ III Guerilla exists, but no Partisan identity crosswalk is established.",
    },
    FreecivUnitVisionGap {
        freeciv_section: "unit_spy",
        freeciv_radius_sq: 8,
        civ3_catalog_id: None,
        civ3_source: None,
        fallback_radius_sq: 2,
        note: "Civ III Conquests has no Spy unit identity; espionage uses city actions.",
    },
    FreecivUnitVisionGap {
        freeciv_section: "unit_awacs",
        freeciv_radius_sq: 26,
        civ3_catalog_id: Some("f_15"),
        civ3_source: Some("Conquests/conquests.biq#PRTO[64]"),
        fallback_radius_sq: 2,
        note: "Civ III F-15 is a Jet Fighter, not an AWACS unit identity.",
    },
];

#[cfg(test)]
#[path = "observed_map/tests.rs"]
mod tests;
