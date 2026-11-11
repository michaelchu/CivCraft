//! Deterministic data for the opt-in UI screenshot gallery; never used in play.
use super::*;

#[cfg(test)]
#[path = "ui_review/tests.rs"]
mod tests;

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
impl Simulation {
    pub(crate) fn ui_review_first_contact(&mut self) {
        self.events.clear();
        self.diplomacy.remove(&(HUMAN, 1));
        self.make_diplomatic_contact(HUMAN, 1);
    }

    pub(crate) fn ui_review_fixture(&mut self) -> (u32, u32) {
        self.units.clear();
        self.cities.clear();
        self.players[HUMAN].known = Tech::all().collect();
        self.players[HUMAN].gold = 12_345_678;
        self.players[HUMAN].researching = Tech::NUCLEAR_FISSION;
        self.players[HUMAN].research_goal = Tech::SUPERCONDUCTORS;
        for tile in &mut self.tiles {
            tile.seen = true;
            tile.known_by.fill(true);
        }
        self.tile_mut(20, 24).terrain = Terrain::Grassland;
        self.create_city(HUMAN, 20, 24);
        let city = self.cities[0].id;
        self.cities[0].name = "Rome on the Northern Coast".into();
        self.cities[0].size = 12;
        self.cities[0].specialists = [2, 2, 1];
        self.cities[0].buildings.extend([
            Building::LIBRARY,
            Building::MARKETPLACE,
            Building::TEMPLE,
        ]);
        self.cities[0].worklist = vec![
            Production::Unit(UnitKind::Worker),
            Production::Building(Building::CITY_WALLS),
            Production::Unit(UnitKind::Marine),
        ];
        for kind in [
            UnitKind::Worker,
            UnitKind::Warrior,
            UnitKind::Marine,
            UnitKind::Rifleman,
            UnitKind::Guerilla,
        ] {
            self.spawn_unit(HUMAN, kind, 20, 24);
        }
        for owner in 1..PLAYER_COUNT {
            let p = (10 + owner as i32 * 2, 32);
            self.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
            self.create_city(owner, p.0, p.1);
            self.sign_pact(HUMAN, owner, Pact::Peace);
        }
        self.ui_review_contacts();
        for x in [16, 20, 24] {
            self.tile_mut(x, 40).terrain = Terrain::Grassland;
            self.create_city(1, x, 40);
        }
        self.world_seed = 18_446_744_073_709_551_615;
        let rival = self.cities_owned_by(1).next().unwrap().id;
        let c = self.cities.iter_mut().find(|c| c.id == rival).unwrap();
        c.name = "Athens of the Eastern Mediterranean".into();
        c.size = 8;
        c.buildings.extend([
            Building::LIBRARY,
            Building::MARKETPLACE,
            Building::TEMPLE,
            Building::CITY_WALLS,
        ]);
        self.reconcile_city_workers();
        self.refresh_visibility();
        for tile in &mut self.tiles {
            tile.known_by[HUMAN] = true;
        }
        self.ui_review_contacts();
        (city, rival)
    }
}

impl Simulation {
    /// Isolated visual fixture for the two sides of a shared national boundary.
    pub(crate) fn ui_review_touching_borders(&mut self) {
        self.units.clear();
        self.cities.clear();
        for (owner, dx, name) in [(HUMAN, -2, "Rome"), (1, 2, "Athens")] {
            let (x, y) = offset_native_by_map(20, 24, dx, 0);
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
            self.cities.last_mut().unwrap().name = name.into();
        }
        for tile in &mut self.tiles {
            tile.owner = None;
            tile.last_seen_owner = None;
        }
        for dy in -4..=4 {
            for dx in -4..=4 {
                let (x, y) = offset_native_by_map(20, 24, dx, dy);
                let tile = self.tile_mut(x, y);
                tile.terrain = Terrain::Grassland;
                tile.river = false;
                tile.owner = Some(if dx <= 0 { HUMAN } else { 1 });
                tile.last_seen_owner = tile.owner;
                tile.seen = true;
                tile.visible = true;
                tile.known_by.fill(true);
            }
        }
    }
}

impl Simulation {
    /// Controlled artwork cases: diagonal desert course, bends, forks and a
    /// lake receiving rivers from each of the six isohex cardinal directions.
    pub(crate) fn ui_review_rivers(&mut self, fogged: bool) {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.owner = None;
            tile.last_seen_owner = None;
            tile.river = false;
            tile.river_edges = 0;
        }
        for dy in -7..=7 {
            for dx in -7..=7 {
                let (x, y) = offset_native_by_map(20, 24, dx, dy);
                let tile = self.tile_mut(x, y);
                tile.terrain = Terrain::Desert;
                tile.river = false;
                tile.river_edges = 0;
                tile.resource = None;
                tile.hut = false;
                tile.road = false;
                tile.railroad = false;
                tile.irrigation = false;
                tile.farmland = false;
                tile.mine = false;
                tile.oil_well = false;
                tile.oil_platform = false;
                tile.pollution = false;
                tile.fallout = false;
                tile.base = None;
                tile.seen = true;
                tile.visible = true;
                tile.known_by.fill(true);
            }
        }
        let mut rivers = vec![
            (-4, -4),
            (-3, -3),
            (-2, -2),
            (2, -3),
            (2, -2),
            (3, -1),
            (3, 0),
            (3, 4),
            (3, 3),
            (4, 4),
            (2, 4),
        ];
        rivers.extend(ISOHEX_DIRS.map(|(dx, dy)| (-3 + dx, 2 + dy)));
        for (dx, dy) in rivers {
            let (x, y) = offset_native_by_map(20, 24, dx, dy);
            self.tile_mut(x, y).river = true;
        }
        for (dx, dy) in [(-1, -1), (-3, 2), (4, 0), (5, 4)] {
            let (x, y) = offset_native_by_map(20, 24, dx, dy);
            self.tile_mut(x, y).terrain = Terrain::Lake;
        }
        super::topology::rebuild_civ3_river_edges(self.map_wrap, &mut self.tiles);
        if fogged {
            let (x, y) = offset_native_by_map(20, 24, -1, -1);
            let tile = self.tile_mut(x, y);
            tile.seen = false;
            tile.visible = false;
            tile.known_by[HUMAN] = false;
            let (x, y) = offset_native_by_map(20, 24, 3, -1);
            self.tile_mut(x, y).visible = false;
        }
        self.events.clear();
        self.add_event(
            "River artwork fixture: diagonal desert course, bends, forks and six lake outlets.",
        );
    }
    /// Reveals the generated map for visual inspection without changing any
    /// terrain, river flags, extras, RNG or starting positions.
    pub(crate) fn ui_review_generated_river_focus(&mut self) -> Option<(i32, i32)> {
        let target = (0..self.map_wrap.height())
            .flat_map(|y| (0..self.map_wrap.width()).map(move |x| (x, y)))
            .filter(|&(x, y)| self.tile(x, y).river)
            .min_by_key(|&(x, y)| {
                let diagonal: Vec<_> = [(1, 1), (-1, -1)]
                    .into_iter()
                    .filter_map(|(dx, dy)| {
                        self.map_wrap
                            .offset(x, y, dx, dy)
                            .map(|p| self.tile(p.0, p.1))
                    })
                    .collect();
                let lake = diagonal.iter().any(|t| t.terrain == Terrain::Lake);
                let link = diagonal.iter().any(|t| t.terrain.is_water() || t.river);
                (
                    !link,
                    self.tile(x, y).terrain != Terrain::Desert,
                    !lake,
                    y,
                    x,
                )
            });
        for tile in &mut self.tiles {
            tile.seen = true;
            tile.visible = true;
            tile.known_by[HUMAN] = true;
            tile.last_seen_owner = tile.owner;
        }
        target
    }
}

impl Simulation {
    pub(crate) fn ui_review_unit_status(&mut self) {
        self.units.clear();
        self.cities.clear();
        for y in 18..=33 {
            for x in 15..=25 {
                let tile = self.tile_mut(x, y);
                tile.terrain = Terrain::Grassland;
                tile.resource = None;
                tile.hut = false;
                tile.base = None;
                tile.owner = None;
                tile.seen = true;
                tile.visible = true;
            }
        }
        for (i, (hp, veteran, state, work)) in [
            (20, 0, UnitState::Idle, None),
            (10, 1, UnitState::Fortified, None),
            (2, 2, UnitState::Sentry, None),
            (20, 0, UnitState::Idle, Some(WorkKind::Road)),
            (20, 0, UnitState::Idle, Some(WorkKind::Mine)),
            (20, 0, UnitState::Idle, Some(WorkKind::Irrigation)),
        ]
        .into_iter()
        .enumerate()
        {
            let kind = if work.is_some() {
                UnitKind::Worker
            } else {
                UnitKind::Rifleman
            };
            let id = self.spawn_unit(
                HUMAN,
                kind,
                17 + (i % 3) as i32 * 3,
                21 + (i / 3) as i32 * 5,
            );
            let u = self.units.iter_mut().find(|u| u.id == id).unwrap();
            u.veteran = veteran;
            u.hp = hp.min(u.max_hit_points());
            u.state = state;
            u.work_kind = work;
            u.work_left = i32::from(work.is_some()) * 3;
        }
        for (kind, x, automated) in [
            (UnitKind::Worker, 17, true),
            (UnitKind::Warrior, 20, true),
            (UnitKind::Rifleman, 23, false),
        ] {
            let id = self.spawn_unit(HUMAN, kind, x, 31);
            let unit = self.units.iter_mut().find(|u| u.id == id).unwrap();
            unit.automated = automated;
            if !automated {
                unit.state = UnitState::Fortifying;
                unit.veteran = 1;
            }
            if !automated {
                self.spawn_unit(HUMAN, UnitKind::Warrior, x, 31);
            }
        }
    }
}

impl Simulation {
    /// Four cultural sheets currently represented by the playable roster,
    /// with one city in each supported population band per style.
    pub(crate) fn ui_review_civ3_city_sprites(&mut self) {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base_terrain = TerrainBase::Grassland;
            tile.resource = None;
            tile.hut = false;
            tile.base = None;
            tile.owner = None;
            tile.seen = true;
            tile.visible = true;
        }
        let styles = [
            (0, "Roman"),
            (1, "European"),
            (2, "Middle Eastern"),
            (4, "Asian"),
        ];
        for (row, (owner, style)) in styles.into_iter().enumerate() {
            self.players[owner].known.clear();
            for (col, (size, band)) in [(3, "Small"), (9, "Medium"), (15, "Large")]
                .into_iter()
                .enumerate()
            {
                let x = 15 + col as i32 * 5;
                let y = 17 + row as i32 * 6;
                self.create_city(owner, x, y);
                let city = self.cities.last_mut().unwrap();
                city.size = size;
                city.name = format!("{style} {band}");
            }
        }
        self.reconcile_city_workers();
        self.refresh_visibility();
        for tile in &mut self.tiles {
            tile.seen = true;
            tile.visible = true;
        }
    }

    pub(crate) fn ui_review_city_appearance(&mut self) {
        self.units.clear();
        self.cities.clear();
        for y in 18..=29 {
            for x in 15..=25 {
                let tile = self.tile_mut(x, y);
                tile.terrain = Terrain::Grassland;
                tile.resource = None;
                tile.hut = false;
                tile.base = None;
                tile.owner = None;
                tile.seen = true;
                tile.visible = true;
            }
        }
        for (owner, x, y, size, name, tech, walls) in [
            (0, 17, 21, 4, "Classical", None, false),
            (1, 22, 21, 5, "Walled Classical", None, true),
            (2, 17, 27, 9, "Industrial", Some(Tech::STEAM_POWER), false),
            (
                3,
                22,
                27,
                17,
                "Modern disorder",
                Some(Tech::SUPERCONDUCTORS),
                true,
            ),
        ] {
            self.players[owner].known.clear();
            if let Some(tech) = tech {
                self.players[owner].known.push(tech);
            }
            self.create_city(owner, x, y);
            let c = self.cities.last_mut().unwrap();
            c.size = size;
            c.name = name.into();
            if walls {
                c.buildings.push(Building::CITY_WALLS);
            }
            if owner != 3 {
                c.buildings.push(Building::SHAKESPEARES_THEATER);
            }
            self.spawn_unit(owner, UnitKind::Warrior, x, y);
        }
        self.reconcile_city_workers();
        self.refresh_visibility();
        for tile in &mut self.tiles {
            tile.seen = true;
            tile.visible = true;
        }
    }
}

impl Simulation {
    /// Real commands drive both cases; only their initial battle/map is staged.
    pub(crate) fn ui_review_animation_setup(&mut self, combat: bool) -> (u32, (i32, i32)) {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.base = None;
            tile.river = false;
            tile.resource = None;
            tile.hut = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.road = false;
            tile.railroad = false;
            tile.visible = true;
            tile.seen = true;
        }
        let target = offset_native_by_map(20, 24, 1, 0);
        let id = self.spawn_unit(
            HUMAN,
            if combat {
                UnitKind::Rifleman
            } else {
                UnitKind::Warrior
            },
            20,
            24,
        );
        if combat {
            self.create_city(1, target.0, target.1);
            self.cities.last_mut().unwrap().name = "Battlefield".into();
            self.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
            self.cancel_pact(HUMAN, 1);
        }
        // Keep an independent rival settlement so this visual battle does
        // not trigger the game's victory modal before it can be inspected.
        self.create_city(1, 40, 40);
        self.spawn_unit(1, UnitKind::Warrior, 40, 40);
        self.enable_visual_events();
        (id, target)
    }
}

impl Simulation {
    pub(crate) fn ui_review_fog(&mut self) -> u32 {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base_terrain = TerrainBase::Grassland;
            tile.altitude = 0;
            tile.hut = false;
            tile.river = false;
            tile.river_edges = 0;
            tile.resource = None;
            tile.road = false;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.base = None;
            tile.mine = false;
            tile.oil_well = false;
            tile.oil_platform = false;
            tile.seen = false;
            tile.visible = false;
            tile.owner = None;
            tile.border_city = None;
            tile.last_seen_owner = None;
            tile.stealth_visible = false;
        }
        let id = self.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);
        self.refresh_visibility();
        for y in 0..self.map_wrap.height() {
            for x in 0..self.map_wrap.width() {
                let distance = self.map_wrap.civ3_distance(x, y, 20, 24);
                let tile = self.tile_mut(x, y);
                tile.seen = distance <= 6;
                tile.visible = distance <= 3;
                tile.known_by[HUMAN] = tile.seen;
            }
        }
        id
    }
}

impl Simulation {
    /// Staged gallery knowledge is explicit and isolated from campaign sight.
    /// Freeze the staged art for remembered cells as if the fixture had been seen.
    pub(crate) fn ui_review_record_knowledge(&mut self) {
        let visible: Vec<Vec<bool>> = (0..self.players.len())
            .map(|owner| {
                self.tiles
                    .iter()
                    .map(|tile| owner == HUMAN && tile.seen)
                    .collect()
            })
            .collect();
        self.remember_render_map(&visible);
    }

    #[cfg(test)]
    pub(crate) fn ui_review_remember_all_tiles(&mut self) {
        for tile in &mut self.tiles {
            tile.seen = true;
            tile.visible = false;
            tile.known_by[HUMAN] = true;
        }
        self.ui_review_record_knowledge();
    }

    /// Controlled combinations through the normal shared world/city renderer.
    pub(crate) fn ui_review_terrain_layers(&mut self, layers: bool) -> Option<u32> {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base_terrain = TerrainBase::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.river_edges = 0;
            tile.mine = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.oil_well = false;
            tile.oil_platform = false;
            tile.hut = false;
            tile.resource = None;
            tile.base = None;
            tile.pollution = false;
            tile.fallout = false;
            tile.owner = None;
            tile.last_seen_owner = None;
            tile.seen = true;
            tile.visible = true;
            tile.known_by.fill(true);
        }
        if layers {
            for dx in -4..=4 {
                let (x, y) = offset_native_by_map(20, 24, dx, 0);
                let tile = self.tile_mut(x, y);
                tile.river = true;
                tile.road = true;
                tile.irrigation = true;
                if dx < 0 {
                    tile.railroad = true;
                }
                if dx == 2 {
                    tile.base = Some(Base::Fortress);
                }
                if dx == -3 {
                    tile.mine = true;
                }
                if dx == -4 {
                    tile.hut = true;
                }
                if dx == -1 {
                    tile.fallout = true;
                }
            }
            self.create_city(HUMAN, 20, 24);
            let city = self.cities.last_mut().unwrap();
            city.name = "River Crossing".into();
            city.buildings.push(Building::CITY_WALLS);
            let id = city.id;
            self.tile_mut(20, 24).pollution = true;
            self.tile_mut(20, 24).irrigation = true;
            self.tile_mut(20, 24).river = true;
            self.tile_mut(20, 24).road = true;
            super::topology::rebuild_civ3_river_edges(self.map_wrap, &mut self.tiles);
            self.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);
            Some(id)
        } else {
            for dy in -4..=4 {
                for dx in -4..=4 {
                    let (x, y) = offset_native_by_map(20, 24, dx, dy);
                    let terrain = match (dx, dy) {
                        (_, -4..=-3) => {
                            if dx < 0 {
                                Terrain::Sea
                            } else {
                                Terrain::Glacier
                            }
                        }
                        (-4..=-2, -2..=0) => {
                            if (dx + dy) % 2 == 0 {
                                Terrain::Forest
                            } else {
                                Terrain::Jungle
                            }
                        }
                        (1..=4, -2..=0) => {
                            if dx % 2 == 0 {
                                Terrain::Mountains
                            } else {
                                Terrain::Hills
                            }
                        }
                        (_, 1..=4) => match dx {
                            -4..=-3 => Terrain::Desert,
                            -2..=-1 => Terrain::Plains,
                            0 => Terrain::Grassland,
                            1..=2 => Terrain::Marsh,
                            _ => Terrain::Tundra,
                        },
                        _ => Terrain::Grassland,
                    };
                    let tile = self.tile_mut(x, y);
                    tile.terrain = terrain;
                    tile.base_terrain = match terrain {
                        Terrain::Glacier => TerrainBase::Ice,
                        Terrain::Coast | Terrain::Lake => TerrainBase::Coast,
                        Terrain::Sea => TerrainBase::Sea,
                        Terrain::Ocean => TerrainBase::Ocean,
                        Terrain::Desert | Terrain::FloodPlains => TerrainBase::Desert,
                        Terrain::Plains | Terrain::Hills | Terrain::Mountains => {
                            TerrainBase::Plains
                        }
                        Terrain::Tundra => TerrainBase::Tundra,
                        Terrain::Grassland | Terrain::GrasslandShield => TerrainBase::Grassland,
                        Terrain::Forest | Terrain::Jungle | Terrain::Marsh | Terrain::Volcano => {
                            TerrainBase::Grassland
                        }
                    };
                }
            }
            super::topology::rebuild_civ3_river_edges(self.map_wrap, &mut self.tiles);
            None
        }
    }

    /// Place every known resource in view for sprite review.
    pub(crate) fn ui_review_resource_shadows(&mut self) {
        use Resource::*;

        self.ui_review_terrain_layers(false);
        let land_resources = [
            (Terrain::Mountains, Aluminum),
            (Terrain::Hills, Coal),
            (Terrain::Grassland, Horses),
            (Terrain::Mountains, Iron),
            (Terrain::Desert, Oil),
            (Terrain::Jungle, Rubber),
            (Terrain::Hills, Saltpeter),
            (Terrain::Mountains, Uranium),
            (Terrain::Jungle, Dyes),
            (Terrain::Forest, Furs),
            (Terrain::Jungle, Gems),
            (Terrain::Desert, Incense),
            (Terrain::Plains, Ivory),
            (Terrain::Forest, Silk),
            (Terrain::Marsh, Spice),
            (Terrain::Grassland, Wine),
            (Terrain::Grassland, Cattle),
            (Terrain::Mountains, Gold),
            (Terrain::Tundra, Game),
            (Terrain::Desert, Oasis),
            (Terrain::Jungle, Sugar),
            (Terrain::Grassland, Tobacco),
            (Terrain::Jungle, TropicalFruit),
            (Terrain::Plains, Wheat),
        ];
        for (index, (terrain, resource)) in land_resources.into_iter().enumerate() {
            let dx = index as i32 % 6 - 3;
            let dy = index as i32 / 6 - 2;
            let (x, y) = offset_native_by_map(20, 24, dx, dy);
            let tile = self.tile_mut(x, y);
            tile.terrain = terrain;
            tile.base_terrain = match terrain {
                Terrain::Glacier => TerrainBase::Ice,
                Terrain::Coast | Terrain::Lake => TerrainBase::Coast,
                Terrain::Sea => TerrainBase::Sea,
                Terrain::Ocean => TerrainBase::Ocean,
                Terrain::Desert | Terrain::FloodPlains => TerrainBase::Desert,
                Terrain::Plains | Terrain::Hills | Terrain::Mountains => TerrainBase::Plains,
                Terrain::Tundra => TerrainBase::Tundra,
                Terrain::Grassland | Terrain::GrasslandShield => TerrainBase::Grassland,
                Terrain::Forest | Terrain::Jungle | Terrain::Marsh | Terrain::Volcano => {
                    TerrainBase::Grassland
                }
            };
            tile.resource = Some(resource);
        }

        // Give the ocean resources a broad, connected coastal sea so the
        // Whales and Fish do not look like they are sitting in a tiny pond.
        for dy in 1..=3 {
            for dx in 3..=6 {
                let (x, y) = offset_native_by_map(20, 24, dx, dy);
                self.tile_mut(x, y).terrain = Terrain::Sea;
            }
        }
        for (dx, resource) in [(3, Fish), (4, Whales)] {
            let (x, y) = offset_native_by_map(20, 24, dx, 2);
            self.tile_mut(x, y).resource = Some(resource);
        }
    }

    pub(crate) fn ui_review_terrain_seams(&mut self) {
        self.ui_review_terrain_layers(false);
        let kinds = [
            Terrain::Coast,
            Terrain::Sea,
            Terrain::Ocean,
            Terrain::Lake,
            Terrain::Glacier,
            Terrain::Desert,
            Terrain::Plains,
            Terrain::Grassland,
            Terrain::GrasslandShield,
            Terrain::Forest,
            Terrain::Jungle,
            Terrain::Hills,
            Terrain::Mountains,
            Terrain::Marsh,
            Terrain::Tundra,
            Terrain::FloodPlains,
            Terrain::Volcano,
        ];
        for tile in 0..self.tiles.len() {
            let (x, y) = (
                tile as i32 % self.map_wrap.width(),
                tile as i32 / self.map_wrap.width(),
            );
            self.tiles[tile].terrain = kinds[((x / 4 + (y / 4) * 3) as usize) % kinds.len()];
        }
    }

    /// A controlled island with evenly spaced Civ III water-depth rings.
    /// This isolates atlas transitions from procedural coastline generation.
    pub(crate) fn ui_review_civ3_water_depths(&mut self) {
        self.ui_review_terrain_layers(false);
        for tile in 0..self.tiles.len() {
            self.tiles[tile].terrain = Terrain::Ocean;
        }
        for dy in -8i32..=8 {
            for dx in -8i32..=8 {
                let distance = crate::sim::topology::isohex_vector_distance(dx, dy);
                let terrain = match distance {
                    0..=3 => Terrain::Grassland,
                    4..=5 => Terrain::Coast,
                    6..=8 => Terrain::Sea,
                    _ => Terrain::Ocean,
                };
                let (x, y) = self
                    .map_wrap
                    .offset(20, 24, dx, dy)
                    .expect("the water-depth bands stay inside the bounded map");
                self.tile_mut(x, y).terrain = terrain;
            }
        }
    }

    pub(crate) fn ui_review_ice_transitions(&mut self) {
        self.ui_review_terrain_layers(false);
        for dy in -8i32..=8 {
            for dx in -8i32..=8 {
                let (x, y) = offset_native_by_map(20, 24, dx, dy);
                let ground = if dx.abs() <= 2 && dy.abs() <= 2 {
                    Terrain::Glacier
                } else if dy < -2 {
                    Terrain::Desert
                } else if dx > 2 {
                    Terrain::Grassland
                } else if dy > 2 {
                    Terrain::Plains
                } else {
                    Terrain::Marsh
                };
                self.tile_mut(x, y).terrain = ground;
            }
        }
    }

    pub(crate) fn ui_review_route_setup(&mut self) -> (u32, (i32, i32)) {
        self.ui_review_terrain_layers(false);
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
        }
        let (x, y) = offset_native_by_map(20, 24, -2, 0);
        let id = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        (id, offset_native_by_map(20, 24, 3, 0))
    }
}

impl Simulation {
    pub(crate) fn ui_review_reveal_all_tiles(&mut self) {
        for tile in &mut self.tiles {
            tile.seen = true;
            tile.visible = true;
            tile.known_by[HUMAN] = true;
        }
        self.ui_review_record_knowledge();
    }
    pub(crate) fn ui_review_wrap_seam(&mut self) -> u32 {
        self.ui_review_terrain_layers(false);
        for (index, tile) in self.tiles.iter_mut().enumerate() {
            let y = index as i32 / self.map_wrap.width();
            tile.terrain = if y == 0 {
                Terrain::Tundra
            } else if y == self.map_wrap.height() - 1 {
                Terrain::Desert
            } else {
                Terrain::Grassland
            };
            tile.visible = true;
            tile.seen = true;
            tile.owner = None;
            tile.last_seen_owner = None;
        }
        self.ui_review_record_knowledge();
        self.spawn_unit(HUMAN, UnitKind::Warrior, 12, 0)
    }
}
