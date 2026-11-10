//! Last observations are campaign state, not a second simulation.
//! Native server/maphand.c copies the giver's private tile, while
//! server/citytools.c::update_dumb_city retains public city graphics until seen again.
use super::*;
use crate::rules::TechEra;
use std::collections::BTreeMap;

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
        self.has_active_wonder(owner, Building::INTERNET)
    }

    pub(in crate::sim) fn refresh_visibility(&mut self) {
        for tile in &mut self.tiles {
            tile.visible = false;
            tile.stealth_visible = false;
        }
        for owner in 0..self.players.len() {
            if self.has_tech(owner, Tech::SATELLITES) {
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
                let vision = match self.tile(u.x, u.y).base {
                    Some(Base::Fortress) if self.has_tech(u.owner, Tech::ASTRONOMY) => 8,
                    Some(Base::Airbase) => 8,
                    _ => 0,
                };
                let radius = u.kind.stats().vision_sq + vision;
                (u.owner, u.x, u.y, radius, radius.clamp(0, 2))
            })
            .chain(self.cities.iter().map(|c| (c.owner, c.x, c.y, 8, 2)))
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
                if distance * distance > radius_sq {
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
                    self.tiles[idx].stealth_visible |= distance * distance <= stealth_sq;
                    self.tiles[idx].seen = true;
                }
            }
        }
        let internet_owners: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.has_building(Building::INTERNET))
            .map(|city| city.owner)
            .collect();
        for owner in internet_owners {
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
        self.observed_map.revision = self.observed_map.revision.saturating_add(1);
        let revision = self.observed_map.revision;
        for (owner, visible) in visible.iter().enumerate() {
            if owner == BARBARIANS {
                continue;
            }
            // Looking at a now-empty city tile removes only that observer's memory.
            self.observed_map.cities.retain(|(viewer, _), (_, c)| {
                *viewer != owner || !visible[self.map_wrap.index(c.city.x, c.city.y)]
            });
            for (index, &seen) in visible.iter().enumerate() {
                if seen {
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
                if visible[self.map_wrap.index(city.x, city.y)] || self.reveals_cities(owner) {
                    let mut observed = self.observed_city(city);
                    observed.remembered = true;
                    self.observed_map
                        .cities
                        .insert((owner, city.id), (revision, observed));
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

impl Simulation {
    /// Gallery scenario: a city and its road are remembered while changes to
    /// the live tile and city remain hidden until the scout returns.
    pub(crate) fn ui_review_observed_map(&mut self) -> (u32, (i32, i32)) {
        self.units.clear();
        self.cities.clear();
        self.observed_map = Default::default();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.hut = false;
            tile.resource = None;
            tile.river = false;
            tile.base = None;
            tile.owner = None;
            tile.seen = false;
            tile.visible = false;
            tile.known_by.fill(false);
        }
        self.players[1].known.clear();
        self.create_city(1, 20, 24);
        let c = self.cities.last_mut().unwrap();
        c.name = "Remembered Athens".into();
        c.size = 5;
        c.buildings.push(Building::CITY_WALLS);
        let scout = self.spawn_unit(HUMAN, UnitKind::Explorer, 19, 24);
        self.tile_mut(20, 24).road = true;
        self.tile_mut(19, 23).terrain = Terrain::Forest;
        self.refresh_visibility();
        let unit = self.units.iter_mut().find(|u| u.id == scout).unwrap();
        unit.x = 4;
        unit.y = 24;
        self.refresh_visibility();
        let c = self.cities.last_mut().unwrap();
        c.name = "Rebuilt Athens".into();
        c.size = 13;
        c.buildings.retain(|b| *b != Building::CITY_WALLS);
        self.players[1].known.push(Tech::ROCKETRY);
        self.tile_mut(20, 24).road = false;
        self.tile_mut(20, 24).railroad = false;
        self.tile_mut(20, 24).terrain = Terrain::Desert;
        self.tile_mut(20, 24).irrigation = true;
        self.tile_mut(19, 23).terrain = Terrain::Hills;
        self.refresh_visibility();
        (scout, (20, 24))
    }

    pub(crate) fn ui_review_reobserve_map(&mut self) {
        let unit = self.units.iter_mut().find(|u| u.owner == HUMAN).unwrap();
        unit.x = 19;
        unit.y = 24;
        self.refresh_visibility();
    }

    pub(crate) fn ui_review_observed_map_valid(&self, reobserved: bool) -> bool {
        let city = self
            .render_cities()
            .into_iter()
            .find(|c| c.city.x == 20 && c.city.y == 24);
        if reobserved {
            self.render_tile(20, 24).terrain == Terrain::Desert
                && !self.render_tile(20, 24).road
                && city.is_some_and(|c| {
                    c.city.name == "Rebuilt Athens" && c.city.size == 13 && !c.remembered
                })
        } else {
            self.render_tile(20, 24).terrain == Terrain::Grassland
                && self.render_tile(20, 24).road
                && city.is_some_and(|c| {
                    c.city.name == "Remembered Athens" && c.city.size == 5 && c.remembered
                })
        }
    }
}

#[cfg(test)]
#[path = "observed_map/tests.rs"]
mod tests;
