//! Automated gameplay scenarios covering aviation behavior.

use super::*;

impl Simulation {
    pub(crate) fn nuclear_playtest_setup(&mut self, intercepted: bool) -> (u32, u32, u32) {
        // Civ III's normal opening has a Settler and Worker, not a founded
        // city. Stage both ICBM launch and target cities explicitly so this
        // incident scenario does not depend on a legacy Freeciv opening.
        self.units.clear();
        self.cities.clear();
        let (sx, sy) = (10, 20);
        let target = offset_native_by_map(sx, sy, 7, 0);
        for (x, y) in [(sx, sy), target] {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.resource = None;
            tile.hut = false;
            tile.base = None;
        }
        self.create_city(HUMAN, sx, sy);
        self.create_city(1, target.0, target.1);
        let city = self.cities.last_mut().unwrap();
        city.size = 4;
        let city_id = city.id;
        if intercepted {
            city.buildings.push(Building::SDI_DEFENSE);
        }
        self.tile_mut(target.0, target.1).known_by[HUMAN] = true;
        self.sign_pact(HUMAN, 1, Pact::War);
        self.completed_wonders.push(Building::MANHATTAN_PROJECT);
        let missile = self.spawn_unit(HUMAN, UnitKind::ICBM, sx, sy);
        let defender = self.spawn_unit(1, UnitKind::Musketman, target.0, target.1);
        // The nuclear interception roll uses the simulation RNG. These seeds
        // put the first xorshift draw on the desired side of the 75% threshold.
        self.native_random = None;
        self.rng = if intercepted { 2 } else { 3 };
        (missile, defender, city_id)
    }

    pub(crate) fn cruise_missile_playtest_setup(&mut self) -> (u32, u32, u32) {
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        for (tx, ty) in [(x, y), (nx, ny)] {
            let tile = self.tile_mut(tx, ty);
            tile.terrain = Terrain::Sea;
            tile.hut = false;
            tile.base = None;
        }
        let carrier = self.spawn_unit(HUMAN, UnitKind::Carrier, x, y);
        let missile = self.spawn_unit(HUMAN, UnitKind::CruiseMissile, x, y);
        let target = self.spawn_unit(1, UnitKind::Transport, nx, ny);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(1);
        (carrier, missile, target)
    }

    pub(crate) fn aegis_playtest_setup(&mut self) -> (u32, u32) {
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        for (tx, ty) in [(x, y), (nx, ny)] {
            let tile = self.tile_mut(tx, ty);
            tile.terrain = Terrain::Sea;
            tile.hut = false;
            tile.base = None;
        }
        let attacker = self.spawn_unit(HUMAN, UnitKind::Helicopter, x, y);
        let cruiser = self.spawn_unit(1, UnitKind::AegisCruiser, nx, ny);
        self.units
            .iter_mut()
            .filter(|unit| [attacker, cruiser].contains(&unit.id))
            .for_each(|unit| unit.hp = 1);
        // This seed makes the first round hinge on the anti-air bonus.
        self.set_random_seed(19);
        (attacker, cruiser)
    }

    pub(crate) fn sam_battery_playtest_setup(&mut self, protected: bool) -> (u32, u32) {
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let tile = self.tile_mut(nx, ny);
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.river = false;
        tile.river_edges = 0;
        self.create_city(1, nx, ny);
        if protected {
            self.cities
                .last_mut()
                .unwrap()
                .buildings
                .push(Building::SAM_BATTERY);
        }
        let bomber = self.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
        // Use a durable source roster unit so the unprotected combat roll
        // cannot remove the defender and hide the SAM damage comparison.
        let defender = self.spawn_unit(1, UnitKind::MechInfantry, nx, ny);
        self.set_random_seed(91); // All three native rolls distinguish SAM defense.
        (bomber, defender)
    }

    pub(crate) fn helicopter_playtest_setup(&mut self) -> (u32, u32) {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = (city.x, city.y);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.tile_mut(nx, ny).base = None;
        self.tile_mut(nx, ny).hut = false;
        self.players[HUMAN].known.push(Tech::COMBINED_ARMS);
        let helicopter = self.spawn_unit(HUMAN, UnitKind::Helicopter, x, y);
        let passenger = self.spawn_unit(HUMAN, UnitKind::Musketman, x, y);
        (helicopter, passenger)
    }

    pub(crate) fn stealth_aircraft_playtest_setup(&mut self) -> (u32, u32) {
        if self.cities_owned_by(HUMAN).next().is_none() {
            self.tile_mut(10, 20).terrain = Terrain::Grassland;
            self.create_city(HUMAN, 10, 20);
        }
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = (city.x, city.y);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.players[HUMAN].known.extend([
            Tech::ADVANCED_FLIGHT,
            Tech::STEALTH,
            Tech::ROCKETRY,
            Tech::REFINING,
        ]);
        for (dx, terrain, resource) in [
            (1, Terrain::Desert, Resource::Oil),
            (-1, Terrain::Hills, Resource::Aluminum),
        ] {
            let (rx, ry) = self.map_wrap.offset(x, y, dx, 0).unwrap();
            let tile = self.tile_mut(rx, ry);
            tile.terrain = terrain;
            tile.resource = Some(resource);
            tile.owner = Some(HUMAN);
            tile.road = true;
        }
        self.players[HUMAN].gold = 1000;
        let bomber = self.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
        let target = self.spawn_unit(1, UnitKind::Worker, nx, ny);
        (bomber, target)
    }

    pub(crate) fn carrier_playtest_setup(&mut self) -> (u32, u32) {
        let (x, y) = (10, 20);
        let deck = offset_native_by_map(x, y, 1, 0);
        let sailing = offset_native_by_map(x, y, 2, 0);
        let launch = offset_native_by_map(x, y, 3, 0);
        for (tx, ty) in [(x, y), deck, sailing, launch] {
            self.tile_mut(tx, ty).terrain = Terrain::Sea;
            self.tile_mut(tx, ty).hut = false;
        }
        let carrier = self.spawn_unit(HUMAN, UnitKind::Carrier, deck.0, deck.1);
        let fighter = self.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
        self.units.last_mut().unwrap().fuel = 1;
        (carrier, fighter)
    }

    pub(crate) fn fighter_playtest_setup(&mut self) -> (u32, u32) {
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        let fighter = self.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
        let bomber = self.spawn_unit(1, UnitKind::Bomber, nx, ny);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(4);
        (fighter, bomber)
    }

    pub(crate) fn bombard_playtest_setup(&mut self) -> (u32, [u32; 2]) {
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Sea;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        let bomber = self.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
        let targets = [
            self.spawn_unit(1, UnitKind::Worker, nx, ny),
            self.spawn_unit(1, UnitKind::Spearman, nx, ny),
        ];
        (bomber, targets)
    }

    pub(crate) fn airlift_playtest_setup(&mut self) -> (u32, u32, u32) {
        let source = self
            .cities
            .iter_mut()
            .find(|city| city.owner == HUMAN)
            .unwrap();
        source.buildings.push(Building::AIRPORT);
        let (x, y) = (source.x, source.y);
        let (nx, ny) = offset_native_by_map(x, y, 5, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(HUMAN, nx, ny);
        let destination = self.cities.last().unwrap().id;
        let first = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        let second = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        (first, second, destination)
    }

    pub(crate) fn ai_aircraft_advisor_playtest_setup(&mut self) -> (u32, u32) {
        self.units.clear();
        self.cities.clear();
        let (x, y) = (10, 24);
        self.sign_pact(1, HUMAN, Pact::War);
        for tile in &mut self.tiles {
            tile.known_by[1] = true;
            tile.hut = false;
        }
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.create_city(1, x, y);
        self.cities[0].size = 5;
        self.players[1].known.extend([
            Tech::FLIGHT,
            Tech::ADVANCED_FLIGHT,
            Tech::SPACE_FLIGHT,
            Tech::ROCKETRY,
        ]);
        self.completed_wonders.push(Building::MANHATTAN_PROJECT);
        let target = offset_native_by_map(x, y, 2, 0);
        self.tile_mut(target.0, target.1).terrain = Terrain::Sea;
        let fighter = self.spawn_unit(1, UnitKind::Fighter, x, y);
        self.units.last_mut().unwrap().moves_left =
            UnitKind::Fighter.stats().moves * MOVE_FRAGMENTS;
        let victim = self.spawn_unit(HUMAN, UnitKind::Battleship, target.0, target.1);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(4);
        self.refresh_visibility();
        (fighter, victim)
    }

    pub(crate) fn ai_nuclear_strike_playtest_setup(&mut self) -> (u32, u32) {
        let (fighter, target) = self.ai_aircraft_advisor_playtest_setup();
        self.units.retain(|u| u.id != fighter);
        let victim = self.unit_by_id(target).unwrap().clone();
        for _ in 0..4 {
            self.spawn_unit(HUMAN, UnitKind::Battleship, victim.x, victim.y);
        }
        let city = self.cities[0].clone();
        let missile = self.spawn_unit(1, UnitKind::TacticalNuke, city.x, city.y);
        (missile, target)
    }
}
