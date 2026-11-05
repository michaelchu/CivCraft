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
