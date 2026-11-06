//! Deterministic data for the opt-in UI screenshot gallery; never used in play.
use super::*;
impl Simulation {
    pub(crate) fn ui_review_spy_war(&mut self) {
        self.cancel_pact(HUMAN, 1);
    }
    pub(crate) fn ui_review_fixture(&mut self) -> (u32, u32, u32) {
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
        self.cities[0].name = "Alexandria on the Northern Coast".into();
        self.cities[0].size = 12;
        self.cities[0].specialists = [2, 2, 1];
        self.cities[0].buildings.extend([
            Building::LIBRARY,
            Building::MARKETPLACE,
            Building::TEMPLE,
        ]);
        self.cities[0].worklist = vec![
            Production::Unit(UnitKind::Engineer),
            Production::Building(Building::CITY_WALLS),
            Production::Unit(UnitKind::Marines),
        ];
        for kind in [
            UnitKind::Engineer,
            UnitKind::Warrior,
            UnitKind::Marines,
            UnitKind::Riflemen,
            UnitKind::AlpineTroops,
            UnitKind::Partisan,
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
        let rival = self.cities.iter().find(|c| c.owner == 1).unwrap().id;
        let c = self.cities.iter_mut().find(|c| c.id == rival).unwrap();
        c.name = "Constantinople of the Eastern Empire".into();
        c.size = 8;
        c.buildings.extend([
            Building::LIBRARY,
            Building::MARKETPLACE,
            Building::TEMPLE,
            Building::CITY_WALLS,
        ]);
        let p = offset_native_by_map(c.x, c.y, -1, 0);
        self.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
        let spy = self.spawn_unit(HUMAN, UnitKind::Spy, p.0, p.1);
        self.reconcile_city_workers();
        self.refresh_visibility();
        for tile in &mut self.tiles {
            tile.known_by[HUMAN] = true;
        }
        self.ui_review_contacts();
        (city, spy, rival)
    }
}

impl Simulation {
    /// Isolated visual fixture for the two sides of a shared national boundary.
    pub(crate) fn ui_review_touching_borders(&mut self) {
        self.units.clear();
        self.cities.clear();
        for (owner, dx, name) in [(HUMAN, -2, "Crimson City"), (1, 2, "Golden City")] {
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

#[cfg(test)]
mod clone_tests {
    use super::*;
    #[test]
    fn baseline_clones_preserve_rng_and_normal_campaign_results() {
        let initial = Simulation::new(crate::mapgen::generate(42));
        let original = bincode::serialize(&initial).unwrap();
        let mut a = initial.clone();
        let mut b = initial.clone();
        let founder = initial
            .units
            .iter()
            .find(|u| u.owner == HUMAN && u.kind == UnitKind::Settlers)
            .unwrap()
            .id;
        assert!(a.apply(Command::FoundCity(founder)));
        assert!(b.apply(Command::FoundCity(founder)));
        assert!(a.apply(Command::EndTurn));
        assert!(b.apply(Command::EndTurn));
        assert_eq!(
            bincode::serialize(&a).unwrap(),
            bincode::serialize(&b).unwrap()
        );
        for _ in 0..32 {
            assert_eq!(a.rand_below(1_000_000), b.rand_below(1_000_000));
        }
        assert_eq!(bincode::serialize(&initial).unwrap(), original);
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
        }
        for dy in -7..=7 {
            for dx in -7..=7 {
                let (x, y) = offset_native_by_map(20, 24, dx, dy);
                let tile = self.tile_mut(x, y);
                tile.terrain = Terrain::Desert;
                tile.river = false;
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
        let target = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .filter(|&(x, y)| self.tile(x, y).river)
            .min_by_key(|&(x, y)| {
                let diagonal = [(1, 1), (-1, -1)].map(|(dx, dy)| {
                    let p = offset_native_by_map(x, y, dx, dy);
                    self.tile(p.0, p.1)
                });
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
                UnitKind::Engineer
            } else {
                UnitKind::Riflemen
            };
            let id = self.spawn_unit(
                HUMAN,
                kind,
                17 + (i % 3) as i32 * 3,
                21 + (i / 3) as i32 * 5,
            );
            let u = self.units.iter_mut().find(|u| u.id == id).unwrap();
            u.hp = hp.min(kind.stats().hp);
            u.veteran = veteran;
            u.state = state;
            u.work_kind = work;
            u.work_left = i32::from(work.is_some()) * 3;
        }
        for (kind, x, automated) in [
            (UnitKind::Engineer, 17, true),
            (UnitKind::Warrior, 20, true),
            (UnitKind::Riflemen, 23, false),
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
            (1, 22, 21, 5, "Walled European", None, true),
            (2, 17, 27, 9, "Industrial", Some(Tech::RAILROAD), false),
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
                UnitKind::Riflemen
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
