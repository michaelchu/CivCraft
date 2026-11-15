//! Last observations are campaign state, not a second simulation.
//! Native server/maphand.c copies the giver's private tile, while
//! server/citytools.c::update_dumb_city retains public city graphics until seen again.
use super::*;
use crate::rules::TechEra;
use crate::rules::freeciv::{
    ContentKind, EffectContext, Requirement, RequirementContext, RequirementEvaluationError,
    RequirementRange, RequirementValue, RulesetData, effect_value,
};
#[path = "observed_map/unit_sight_bindings.rs"]
mod unit_sight_bindings;
use std::collections::BTreeMap;
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
        let base = ruleset_unit_vision_radius_sq(ruleset, kind);
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
        ruleset_unit_vision_layer(RulesetData::campaign_default(), kind)
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
        let mut observations = vec![None; self.tiles.len()];
        let mut render_seen = vec![vec![false; self.tiles.len()]; self.players.len()];
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
        let city_reveal_owners: Vec<_> = (0..self.players.len())
            .filter(|&owner| self.reveals_cities(owner))
            .collect();
        for owner in city_reveal_owners {
            for city in &self.cities {
                let tile = &mut self.tiles[self.map_wrap.index(city.x, city.y)];
                tile.known_by[owner] = true;
                tile.seen |= owner == HUMAN;
            }
        }
        self.reconcile_city_workers();
        self.remember_visible_borders();
        self.remember_route_tiles(false);
        self.remember_visible_rivals();
        self.remember_render_map(&render_seen);
        self.share_render_memory();
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
        let (_, _, trade) = self.city_yields(city);
        let luxury = self.city_trade(city, trade)[2];
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
            civ3_era: Tech::all()
                .filter(|&tech| self.has_tech(city.owner, tech))
                .map(|tech| match tech.era() {
                    TechEra::Ancient => 0,
                    TechEra::MiddleAges => 1,
                    TechEra::Industrial => 2,
                    TechEra::Modern => 3,
                })
                .max()
                .unwrap_or(0),
            occupied,
            disorder: self.city_in_disorder(city, luxury),
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
        self.observed_map.revision = self.observed_map.revision.saturating_add(1);
        let revision = self.observed_map.revision;
        let indices = scope
            .map(|scope| scope.to_vec())
            .unwrap_or_else(|| (0..self.tiles.len()).collect());
        for (owner, visible) in visible.iter().enumerate() {
            if owner == BARBARIANS {
                continue;
            }
            // Looking at a now-empty city tile removes only that observer's memory.
            self.observed_map.cities.retain(|(viewer, _), (_, c)| {
                let index = self.map_wrap.index(c.city.x, c.city.y);
                *viewer != owner || !indices.contains(&index) || !visible[index]
            });
            for &index in &indices {
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
            for city in &self.cities {
                let index = self.map_wrap.index(city.x, city.y);
                if indices.contains(&index) && (visible[index] || self.reveals_cities(owner)) {
                    // City visibility and unit visibility are independent.
                    // A map-show operation can reveal a city site while its
                    // garrison remains hidden.
                    let occupied = self.has_main_sight_at(owner, city.x, city.y)
                        && self
                            .units
                            .iter()
                            .any(|unit| (unit.x, unit.y) == (city.x, city.y));
                    let mut observed = self.observed_city_with_occupancy(city, occupied);
                    observed.remembered = true;
                    self.observed_map
                        .cities
                        .insert((owner, city.id), (revision, observed));
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
        let radius_sq = ruleset_unit_vision_radius_sq(RulesetData::campaign_default(), unit_kind);
        let radius = (radius_sq.max(0) as f64).sqrt().ceil() as i32;
        let indices = self
            .map_wrap
            .civ3_radius(x, y, radius)
            .into_iter()
            .filter(|(_, _, distance)| distance * distance <= radius_sq.max(0))
            .map(|(tx, ty, _)| self.map_wrap.index(tx, ty))
            .collect::<Vec<_>>();
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

fn ruleset_unit_vision_radius_sq(ruleset: &RulesetData, kind: UnitKind) -> i32 {
    let conquests_id = kind
        .conquests_id()
        .expect("campaign unit must have a Conquests identity");
    let binding = unit_sight_binding_for(conquests_id)
        .unwrap_or_else(|| panic!("missing sight-only binding for Civ III unit {conquests_id}"));
    let section = match binding {
        SightBinding::Exact(section) | SightBinding::SightAlias(section) => section,
        SightBinding::Unresolved => return 2,
    };
    let record = ruleset
        .content
        .iter()
        .find(|record| record.kind == ContentKind::Unit && record.section == section)
        .unwrap_or_else(|| {
            panic!(
                "sight binding for Civ III unit {conquests_id} references missing C2C3 unit {section}"
            )
        });
    let Some(value) = record
        .integer_field("vision_radius_sq")
        .unwrap_or_else(|error| panic!("invalid vision radius for {}: {error}", kind.name()))
    else {
        panic!("C2C3 unit {section} has no vision_radius_sq")
    };
    i32::try_from(value).unwrap_or_else(|_| panic!("invalid vision_radius_sq for {}", kind.name()))
}

fn ruleset_unit_vision_layer(ruleset: &RulesetData, kind: UnitKind) -> UnitVisionLayer {
    let conquests_id = kind
        .conquests_id()
        .expect("campaign unit must have a Conquests identity");
    let binding = unit_sight_binding_for(conquests_id)
        .unwrap_or_else(|| panic!("missing sight-only binding for Civ III unit {conquests_id}"));
    let section = match binding {
        SightBinding::Exact(section) | SightBinding::SightAlias(section) => section,
        // Preserve the Civ III visibility behavior for identities whose C2C3
        // counterpart is intentionally unresolved.
        SightBinding::Unresolved => {
            return if kind.uses_stealth_vision() {
                UnitVisionLayer::Stealth
            } else {
                UnitVisionLayer::Main
            };
        }
    };
    let record = ruleset
        .content
        .iter()
        .find(|record| record.kind == ContentKind::Unit && record.section == section)
        .unwrap_or_else(|| {
            panic!(
                "sight binding for Civ III unit {conquests_id} references missing C2C3 unit {section}"
            )
        });
    // Freeciv's native ruleset loader defaults omitted vision_layer to Main.
    let layer = record
        .string_field("vision_layer")
        .unwrap_or_else(|error| panic!("invalid vision layer for {}: {error}", kind.name()))
        .unwrap_or("Main");
    match layer.to_ascii_lowercase().as_str() {
        "main" => UnitVisionLayer::Main,
        "stealth" => UnitVisionLayer::Stealth,
        "subsurface" => UnitVisionLayer::Subsurface,
        _ => panic!("invalid vision_layer {layer:?} for C2C3 unit {section}"),
    }
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
