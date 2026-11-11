//! Automated gameplay scenarios covering naval trade behavior.

use super::*;

impl Simulation {
    pub(crate) fn hunter_production_playtest_setup(&mut self) -> (u32, u32) {
        let (x, y) = (10, 20);
        let (tx, ty) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(tx, ty).terrain = Terrain::Sea;
        self.tile_mut(tx, ty).known_by[1] = true;
        self.tile_mut(tx, ty).hut = false;
        // Cruise Missile requires connected Aluminum in the Conquests roster.
        let (ax, ay) = offset_native_by_map(x, y, -1, 0);
        let aluminum = self.tile_mut(ax, ay);
        aluminum.terrain = Terrain::Hills;
        aluminum.owner = Some(1);
        aluminum.resource = Some(Resource::Aluminum);
        aluminum.road = true;
        let (ux, uy) = offset_native_by_map(x, y, -2, 0);
        let uranium = self.tile_mut(ux, uy);
        uranium.terrain = Terrain::Mountains;
        uranium.owner = Some(1);
        uranium.resource = Some(Resource::Uranium);
        uranium.road = true;
        self.create_city(1, x, y);
        let city = self.cities.last().unwrap().id;
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        let target = self.spawn_unit(HUMAN, UnitKind::Battleship, tx, ty);
        self.units.last_mut().unwrap().hp = 1;
        self.previous_naval_positions
            .insert(target, offset_native_by_map(x, y, 5, 0));
        self.players[1]
            .known
            .extend([Tech::COMBUSTION, Tech::ROCKETRY]);
        let production = self.choose_ai_production(1, city);
        let port = self.cities.last_mut().unwrap();
        port.production = production;
        port.shields = 120;
        (city, target)
    }

    pub(crate) fn ai_submarine_playtest_setup(&mut self) -> (u32, u32, u32) {
        let (submarine, missile, target, _) = self.submarine_playtest_setup();
        for unit in &mut self.units {
            if unit.id == submarine || unit.id == missile {
                unit.owner = 1;
            }
            if unit.id == target {
                unit.owner = HUMAN;
                unit.kind = UnitKind::Carrier;
            }
            if unit.id == missile {
                unit.transport = Some(submarine);
            }
        }
        let (x, y) = self
            .units
            .iter()
            .find(|unit| unit.id == target)
            .map(|unit| (unit.x, unit.y))
            .unwrap();
        self.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
        self.units.last_mut().unwrap().transport = Some(target);
        (submarine, missile, target)
    }

    pub(crate) fn submarine_playtest_setup(&mut self) -> (u32, u32, u32, bool) {
        let (submarine, missile, target) = self.cruise_missile_playtest_setup();
        let ship = self
            .units
            .iter_mut()
            .find(|unit| unit.id == submarine)
            .unwrap();
        ship.kind = UnitKind::Submarine;
        ship.hp = UnitKind::Submarine.stats().hp;
        let mut destroyer = self
            .units
            .iter()
            .find(|unit| unit.id == target)
            .unwrap()
            .clone();
        destroyer.kind = UnitKind::Destroyer;
        let defense =
            |attacker| self.defense_power_against(&destroyer, destroyer.x, destroyer.y, attacker);
        let doubled = defense(UnitKind::Submarine) == 2 * defense(UnitKind::Cruiser);
        (submarine, missile, target, doubled)
    }

    pub(crate) fn surface_warship_playtest(&mut self) -> (bool, bool, bool) {
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let (x, y) = (city.x, city.y);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let (tx, ty) = offset_native_by_map(nx, ny, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Sea;
        self.tile_mut(tx, ty).terrain = Terrain::Grassland;
        self.tile_mut(tx, ty).base = None;
        self.players[HUMAN].known.extend([
            Tech::NAVIGATION,
            Tech::STEAM_ENGINE,
            Tech::ENGINEERING,
            Tech::STEEL,
            Tech::MASS_PRODUCTION,
        ]);
        let city = self.cities_owned_by(HUMAN).next().unwrap();
        let production = self.can_produce(city, Production::Unit(UnitKind::Destroyer))
            && self.can_produce(city, Production::Unit(UnitKind::Cruiser))
            && self.can_produce(city, Production::Unit(UnitKind::Battleship))
            && !self.can_produce(city, Production::Unit(UnitKind::Frigate))
            && !self.can_produce(city, Production::Unit(UnitKind::Ironclad));
        let frigate = self.spawn_unit(HUMAN, UnitKind::Frigate, x, y);
        let upgrade = self
            .units
            .iter()
            .find(|unit| unit.id == frigate)
            .and_then(|unit| self.paid_upgrade(unit))
            .is_some_and(|(kind, _)| kind == UnitKind::Destroyer);
        let battleship = self.spawn_unit(HUMAN, UnitKind::Battleship, nx, ny);
        let defender = self.spawn_unit(1, UnitKind::Worker, tx, ty);
        let shore_attack = self.apply(Command::Move(battleship, 1, 0))
            && self.units.iter().all(|unit| unit.id != defender)
            && self
                .units
                .iter()
                .any(|unit| unit.id == battleship && (unit.x, unit.y) == (nx, ny));
        (production, upgrade, shore_attack)
    }

    pub(crate) fn trireme_playtest_setup(&mut self) -> (u32, u32) {
        let (x, y) = (10, 20);
        for step in 0..=2 {
            let (nx, ny) = offset_native_by_map(x, y, step, 0);
            let tile = self.tile_mut(nx, ny);
            tile.terrain = if step == 2 {
                Terrain::Ocean
            } else {
                Terrain::Grassland
            };
            tile.river = step != 2;
            tile.hut = false;
            tile.owner = None;
        }
        let ship = self.spawn_unit(HUMAN, UnitKind::Galley, x, y);
        let cargo = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        (ship, cargo)
    }

    pub(crate) fn naval_bonus_playtest(&mut self) -> (i32, i32, i32, usize) {
        let city = self
            .cities
            .iter_mut()
            .find(|city| city.owner == HUMAN)
            .unwrap();
        city.buildings.extend([
            Building::LIGHTHOUSE,
            Building::MAGELLANS_EXPEDITION,
            Building::PORT_FACILITY,
        ]);
        let (x, y) = (city.x, city.y);
        let rank = self.trained_rank(
            self.cities_owned_by(HUMAN).next().unwrap(),
            UnitKind::AegisCruiser,
        );
        self.players[HUMAN].known.push(Tech::NUCLEAR_POWER);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Sea;
        let docked = self.spawn_unit(HUMAN, UnitKind::AegisCruiser, x, y);
        self.units.last_mut().unwrap().hp = 15;
        let at_sea = self.spawn_unit(HUMAN, UnitKind::AegisCruiser, nx, ny);
        let ship = self.units.last_mut().unwrap();
        ship.hp = 15;
        ship.moved = true;
        self.update_activities_and_restore();
        let docked = self.unit_by_id(docked).unwrap();
        let at_sea = self.unit_by_id(at_sea).unwrap();
        (docked.hp, docked.moves_left, at_sea.moves_left, rank)
    }

    pub(crate) fn ai_ferry_registry_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_ferries = Default::default();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Sea;
            tile.hut = false;
        }
        let ship = self.spawn_unit(1, UnitKind::Transport, 10, 24);
        let first = self.spawn_unit(1, UnitKind::Settler, 10, 24);
        let second = self.spawn_unit(1, UnitKind::Warrior, 10, 24);
        self.request_ai_ferry(first);
        let available = self.ferry_available_count(1) == 1;
        let assigned = self.assign_ai_ferry(ship, first)
            && self.ferry_available_count(1) == 0
            && !self.ferry_can_take(ship, second)
            && self.cargo_count(ship) == 0;
        self.board_specific_ship(first, ship);
        self.board_specific_ship(second, ship);
        let kept_boss = self.ferry_boss(ship) == Some(first);
        self.release_ai_ferry(first);
        let released = self.ferry_available_count(1) == 1 && self.cargo_count(ship) == 2;
        // A released boat can appoint carried cargo rather than remain idle.
        self.refresh_ferry_boss(ship);
        let appointed = self.ferry_boss(ship).is_some() && self.ferry_available_count(1) == 0;
        let boss = self.ferry_boss(ship).unwrap();
        self.remove_units_with_cargo(vec![boss]);
        let death_released = self.ferry_available_count(1) == 1;
        available
            && assigned
            && kept_boss
            && released
            && appointed
            && death_released
            && self.ai_ferries.valid(&self.units)
    }
}
