//! Last observations are campaign state, not a second simulation.
//! Native server/maphand.c copies the giver's private tile, while
//! server/citytools.c::update_dumb_city retains public city graphics until seen again.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ObservedCity {
    pub(crate) city: City,
    pub(crate) art_cell: (i32, i32),
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
    /// Knowledge flags remain current; unexplored callers must still honor `seen`.
    pub(crate) fn render_tile(&self, x: i32, y: i32) -> &Tile {
        let tile = self.tile(x, y);
        if tile.visible || !tile.seen {
            tile
        } else {
            self.observed_map
                .tiles
                .get(&(HUMAN, Self::idx(x, y)))
                .map_or(tile, |(_, remembered)| remembered)
        }
    }

    pub(crate) fn observed_city(&self, city: &City) -> ObservedCity {
        ObservedCity {
            city: city.clone(),
            art_cell: crate::rules::city_art::city_art_cell(
                crate::rules::city_art::initial_style(city.owner),
                city.size,
                |tech| self.has_tech(city.owner, tech),
                |building| city.buildings.contains(&building),
            ),
            occupied: self.units.iter().any(|u| (u.x, u.y) == (city.x, city.y)),
            disorder: self.city_forecast(city).disorder,
            remembered: false,
        }
    }

    pub(crate) fn render_cities(&self) -> Vec<ObservedCity> {
        let reveal = self.reveals_cities(HUMAN);
        let mut cities: BTreeMap<u32, ObservedCity> = self
            .observed_map
            .cities
            .iter()
            .filter(|((owner, _), (_, c))| {
                *owner == HUMAN
                    && self.tile(c.city.x, c.city.y).seen
                    && !self.tile(c.city.x, c.city.y).visible
            })
            .map(|((_, id), (_, c))| (*id, c.clone()))
            .collect();
        for city in &self.cities {
            if city.owner == HUMAN
                || self.tile(city.x, city.y).visible
                || (reveal && self.tile(city.x, city.y).seen)
            {
                cities.insert(city.id, self.observed_city(city));
            }
        }
        cities.into_values().collect()
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
                *viewer != owner || !visible[Self::idx(c.city.x, c.city.y)]
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
                if visible[Self::idx(city.x, city.y)] || self.reveals_cities(owner) {
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
                        *viewer == giver && Self::idx(observed.city.x, observed.city.y) == index
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
            *viewer != receiver || !transferred.contains(&Self::idx(city.city.x, city.city.y))
        });
        let cities: Vec<_> = self
            .observed_map
            .cities
            .iter()
            .filter(|((owner, _), (_, c))| {
                *owner == giver && transferred.contains(&Self::idx(c.city.x, c.city.y))
            })
            .map(|((_, id), entry)| (*id, entry.clone()))
            .collect();
        for (id, city) in cities {
            self.observed_map.cities.insert((receiver, id), city);
        }
    }

    /// Pre-V14 files have no visual snapshots. Preserve all recorded route
    /// facts; reconstruct the remaining artwork once, never on each redraw.
    pub(super) fn migrate_render_memory(&mut self) {
        for (index, tile) in self.tiles.iter().enumerate() {
            for owner in 0..self.players.len() {
                if !tile.known_by[owner] || owner == BARBARIANS {
                    continue;
                }
                let mut observed = tile.clone();
                observed.seen = true;
                observed.visible = false;
                observed.stealth_visible = false;
                if owner == HUMAN
                    && !tile.visible
                    && let Some(Some(route)) = self.route_memory.get(index)
                {
                    observed.terrain = route.terrain;
                    observed.road = route.road;
                    observed.railroad = route.railroad;
                    observed.river = route.river;
                }
                self.observed_map
                    .tiles
                    .insert((owner, index), (0, observed));
            }
        }
        for city in &self.cities {
            for owner in 0..self.players.len() {
                if self.tile(city.x, city.y).known_by[owner] && owner != BARBARIANS {
                    let mut observed = self.observed_city(city);
                    observed.remembered = true;
                    self.observed_map
                        .cities
                        .insert((owner, city.id), (0, observed));
                }
            }
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
                        && (0..MAP_W).contains(&city.x)
                        && (0..MAP_H).contains(&city.y)
                        && city.size > 0
                        && *revision <= self.observed_map.revision
                        && observed.remembered
                        && (0..20).contains(&observed.art_cell.0)
                        && (0..10).contains(&observed.art_cell.1)
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
        unit.y = 48;
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
mod tests {
    use super::*;

    #[test]
    fn fog_preserves_terrain_extras_and_city_graphics_until_reobserved() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.ui_review_observed_map();
        assert!(game.ui_review_observed_map_valid(false));
        let remembered = game
            .render_cities()
            .into_iter()
            .find(|c| c.city.x == 20 && c.city.y == 24)
            .unwrap();
        assert!(remembered.art_cell.1 >= 5, "remembered city retains walls");
        assert_eq!(
            remembered.art_cell.0, 2,
            "remembered Greek city retains classical artwork"
        );
        assert_eq!(game.render_tile(19, 23).terrain, Terrain::Forest);
        assert!(!game.render_tile(20, 24).visible);
        let observed_random = game.random_snapshot();
        game.ui_review_reobserve_map();
        assert!(game.ui_review_observed_map_valid(true));
        let current = game
            .render_cities()
            .into_iter()
            .find(|c| c.city.x == 20 && c.city.y == 24)
            .unwrap();
        assert!(current.art_cell.1 < 5);
        assert_eq!(current.art_cell.0, 8);
        assert_eq!(
            observed_random,
            game.random_snapshot(),
            "observation consumes no random draws"
        );
    }

    #[test]
    fn destroyed_or_captured_cities_remain_at_last_observed_state() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.ui_review_observed_map();
        let id = game.cities[0].id;
        game.cities[0].owner = 2;
        game.refresh_visibility();
        assert_eq!(
            game.render_cities()
                .iter()
                .find(|c| c.city.id == id)
                .unwrap()
                .city
                .owner,
            1
        );
        game.cities.clear();
        game.refresh_visibility();
        assert!(game.render_cities().iter().any(|c| c.city.id == id));
        game.ui_review_reobserve_map();
        assert!(!game.render_cities().iter().any(|c| c.city.id == id));
    }

    #[test]
    fn sea_map_copies_the_sellers_visual_snapshot_not_live_world() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.map_snapshot_playtest());
        assert_eq!(game.render_tile(23, 36).terrain, Terrain::Ocean);
        assert_eq!(game.tile(23, 36).terrain, Terrain::Grassland);
    }

    #[test]
    fn improvements_and_occupancy_are_frozen_outside_sight() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.ui_review_observed_map();
        game.ui_review_reobserve_map();
        let guard = game.spawn_unit(1, UnitKind::Warrior, 20, 24);
        game.refresh_visibility();
        if let Some(unit) = game.units.iter_mut().find(|u| u.owner == HUMAN) {
            unit.x = 4;
            unit.y = 48;
        }
        game.refresh_visibility();
        game.units.retain(|u| u.id != guard);
        let tile = game.tile_mut(20, 24);
        tile.pollution = true;
        tile.fallout = true;
        tile.hut = true;
        tile.irrigation = false;
        tile.mine = true;
        tile.resource = Some(Resource::Oasis);
        game.refresh_visibility();
        let tile = game.render_tile(20, 24);
        assert!(!tile.pollution && !tile.fallout && !tile.hut && !tile.mine);
        assert!(tile.irrigation);
        assert!(tile.resource.is_none());
        assert!(
            game.render_cities()
                .iter()
                .find(|c| c.city.x == 20 && c.city.y == 24)
                .unwrap()
                .occupied
        );
        game.ui_review_reobserve_map();
        assert!(game.render_tile(20, 24).pollution);
        assert!(
            !game
                .render_cities()
                .iter()
                .find(|c| c.city.x == 20 && c.city.y == 24)
                .unwrap()
                .occupied
        );
    }

    #[test]
    fn an_older_trade_map_does_not_replace_a_newer_observation() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.observed_map = Default::default();
        for tile in &mut game.tiles {
            tile.seen = false;
            tile.known_by.fill(false);
        }
        game.tile_mut(23, 36).terrain = Terrain::Ocean;
        game.spawn_unit(1, UnitKind::Trireme, 23, 36);
        game.refresh_visibility();
        game.units.clear();
        game.tile_mut(23, 36).terrain = Terrain::Grassland;
        game.spawn_unit(HUMAN, UnitKind::Explorer, 23, 36);
        game.refresh_visibility();
        game.units.clear();
        game.refresh_visibility();
        game.transfer_treaty_map(1, HUMAN, TreatyMap::World);
        assert_eq!(game.render_tile(23, 36).terrain, Terrain::Grassland);
    }

    #[test]
    fn newer_trade_maps_update_existing_visual_and_movement_knowledge_together() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.observed_map = Default::default();
        for tile in &mut game.tiles {
            tile.seen = false;
            tile.known_by.fill(false);
        }
        let (x, y) = (23, 36);
        let index = Simulation::idx(x, y);
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.road = false;
        tile.railroad = false;
        tile.owner = None;
        // Both receivers already know an older, unroaded land tile.
        game.spawn_unit(HUMAN, UnitKind::Explorer, x, y);
        game.spawn_unit(2, UnitKind::Explorer, x, y);
        game.refresh_visibility();
        game.units.clear();
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Hills;
        tile.road = true;
        tile.railroad = true;
        tile.owner = Some(1);
        game.spawn_unit(1, UnitKind::Explorer, x, y);
        game.refresh_visibility();
        game.units.clear();
        // The world changes again after the seller leaves. Neither recipient
        // may receive this unobserved ocean or lose the recorded roads.
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Ocean;
        tile.road = false;
        tile.railroad = false;
        tile.owner = None;
        game.refresh_visibility();
        assert!(game.tile(x, y).seen && !game.tile(x, y).visible);
        assert!(!game.observed_route_tile(x, y).unwrap().road);
        let random = game.random_snapshot();
        game.transfer_treaty_map(1, HUMAN, TreatyMap::World);
        game.transfer_treaty_map(1, 2, TreatyMap::World);
        let rendered = game.render_tile(x, y);
        assert_eq!(rendered.terrain, Terrain::Hills);
        assert!(rendered.road && rendered.railroad);
        assert_eq!(game.displayed_tile_owner(x, y), Some(1));
        let route = game.observed_route_tile(x, y).unwrap();
        assert_eq!(route.terrain, Terrain::Hills);
        assert!(route.road && route.railroad && !route.is_water());
        let ai = game.ai_map_memory.get(&(2, index)).unwrap();
        assert_eq!(ai.route.terrain, Terrain::Hills);
        assert!(ai.route.road && ai.route.railroad && !ai.ocean);
        assert_eq!(ai.owner, Some(1));
        assert_eq!(random, game.random_snapshot());
    }

    #[test]
    fn observed_map_round_trips_and_rejects_corrupt_owners() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.ui_review_observed_map();
        let path =
            std::env::temp_dir().join(format!("civcraft-observed-map-{}.save", std::process::id()));
        game.save_campaign(&path).unwrap();
        let mut loaded = Simulation::load_campaign(&path).unwrap();
        assert!(loaded.ui_review_observed_map_valid(false));
        assert_eq!(loaded.random_snapshot(), game.random_snapshot());
        loaded
            .observed_map
            .tiles
            .insert((usize::MAX, 0), (0, loaded.tiles[0].clone()));
        assert!(!loaded.valid_render_memory());
        std::fs::remove_file(path).unwrap();
    }
}
