use super::super::guards::GuardState;
use super::super::*;

impl Simulation {
    pub(crate) fn ai_counterattack_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        let (x, y) = (10, 24);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(1, x, y);
        let marine = self.spawn_unit(1, UnitKind::Marines, x, y);
        let cannon = self.spawn_unit(HUMAN, UnitKind::Cannon, nx, ny);
        self.units.last_mut().unwrap().hp = 1;
        self.run_ai_player(1);
        let held = self.units.iter().any(|unit| unit.id == cannon)
            && self
                .units
                .iter()
                .any(|unit| unit.id == marine && (unit.x, unit.y) == (x, y));
        self.update_activities_and_restore();
        let guard = self.spawn_unit(1, UnitKind::Warrior, x, y);
        self.units.last_mut().unwrap().moves_left = 0;
        self.set_random_seed(1);
        self.run_ai_player(1);
        let attack = self.units.iter().all(|unit| unit.id != cannon)
            && self
                .units
                .iter()
                .any(|unit| unit.id == guard && (unit.x, unit.y) == (x, y));

        self.units.clear();
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        let enemy = self.spawn_unit(HUMAN, UnitKind::Marines, nx, ny);
        self.run_ai_player(1);
        let refused = self
            .units
            .iter()
            .any(|unit| unit.id == enemy && unit.hp == 20)
            && self
                .units
                .iter()
                .filter(|unit| unit.owner == 1 && (unit.x, unit.y) == (x, y))
                .count()
                == 2;
        (held, attack, refused)
    }

    pub(crate) fn ai_road_reinforcement_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.road = false;
            tile.railroad = false;
            tile.known_by[1] = true;
        }
        let home = (10, 24);
        let point = |step| offset_native_by_map(home.0, home.1, step, 0);
        let front = point(3);
        let enemy = point(4);
        self.create_city(1, home.0, home.1);
        self.create_city(1, front.0, front.1);
        let marine = self.spawn_unit(1, UnitKind::Marines, home.0, home.1);
        let guard = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        self.spawn_unit(1, UnitKind::Warrior, front.0, front.1);
        self.spawn_unit(HUMAN, UnitKind::Cannon, enemy.0, enemy.1);
        let too_far = !self.reinforce_ai_cities(1).contains(&marine);
        for step in 0..=3 {
            let (x, y) = point(step);
            self.tile_mut(x, y).road = true;
        }
        let dispatched = self.reinforce_ai_cities(1).contains(&marine);
        too_far
            && dispatched
            && self
                .units
                .iter()
                .any(|unit| unit.id == marine && (unit.x, unit.y) == front)
            && self
                .units
                .iter()
                .any(|unit| unit.id == guard && (unit.x, unit.y) == home)
    }

    pub(crate) fn ai_reinforcements_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.road = false;
        }
        let home = (10, 24);
        let front = offset_native_by_map(home.0, home.1, 2, 0);
        let enemy = offset_native_by_map(home.0, home.1, 4, 0);
        self.create_city(1, home.0, home.1);
        self.create_city(1, front.0, front.1);
        let guard = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let marine = self.spawn_unit(1, UnitKind::Marines, home.0, home.1);
        self.spawn_unit(1, UnitKind::Warrior, front.0, front.1);
        self.spawn_unit(HUMAN, UnitKind::Cannon, enemy.0, enemy.1);
        let position = |game: &Self, id| {
            game.units
                .iter()
                .find(|unit| unit.id == id)
                .map(|unit| (unit.x, unit.y))
        };
        self.run_ai_player(1);
        let dispatched = position(self, marine).is_some_and(|tile| tile != home && tile != front);
        for _ in 0..2 {
            self.update_activities_and_restore();
            self.run_ai_player(1);
        }
        (
            dispatched,
            position(self, marine) == Some(front),
            position(self, guard) == Some(home),
        )
    }

    pub(crate) fn ai_fortifications_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.base = None;
            tile.hut = false;
        }
        let (x, y) = (10, 24);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let (tx, ty) = offset_native_by_map(x, y, 2, 0);
        self.create_city(1, x, y);
        self.players[1]
            .known
            .extend([Tech::GUNPOWDER, Tech::MASONRY]);
        let city = self.cities[0].id;
        let immediate = self.spawn_unit(HUMAN, UnitKind::Cannon, nx, ny);
        let empty = self.choose_ai_production(1, city) == Production::Unit(UnitKind::Musketeers);
        self.units.retain(|unit| unit.id != immediate);
        let cannon = self.spawn_unit(HUMAN, UnitKind::Cannon, tx, ty);
        let guard = self.spawn_unit(1, UnitKind::Musketeers, x, y);
        self.spawn_unit(1, UnitKind::Musketeers, x, y);
        self.cities[0].production = Production::Building(Building::GRANARY);
        // A two-turn occupier has urgency1 (not the adjacent grave-danger11).
        // Two defenders now exceed urgency, so the native defense advisor
        // considers walls instead of another unit. Routine military/training
        // and domestic competitors are intentionally a separate decision.
        let walls = self.city_defense_production(&self.cities[0])
            == Some(Production::Building(Building::CITY_WALLS));
        let defender = self.unit_by_id(guard).unwrap();
        let land_bonus =
            self.defense_power_with_building(
                defender,
                x,
                y,
                UnitKind::Cannon,
                Some(Building::CITY_WALLS),
            ) > self.defense_power_with_building(defender, x, y, UnitKind::Cannon, None);
        self.units.retain(|unit| unit.id != cannon);
        self.tile_mut(nx, ny).terrain = Terrain::Ocean;
        self.spawn_unit(HUMAN, UnitKind::Battleship, nx, ny);
        let defender = self.unit_by_id(guard).unwrap();
        let ordinary = self.defense_power_with_building(defender, x, y, UnitKind::Battleship, None);
        // Native dai_find_source_building(NULL) ignores class requirements:
        // its first positive provider can still be City Walls for sea danger.
        // Preserve that advisor quirk while checking the real combat effects.
        let naval = self.city_threats(&self.cities[0]).next().is_some()
            && self.city_defense_production(&self.cities[0])
                == Some(Production::Building(Building::CITY_WALLS))
            && self.defense_power_with_building(
                defender,
                x,
                y,
                UnitKind::Battleship,
                Some(Building::CITY_WALLS),
            ) == ordinary
            && self.defense_power_with_building(
                defender,
                x,
                y,
                UnitKind::Battleship,
                Some(Building::COASTAL_DEFENSE),
            ) > ordinary;
        (empty, walls && land_bonus, naval)
    }

    pub(crate) fn ai_city_defense_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        let (x, y) = (10, 24);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(1, x, y);
        self.players[1]
            .known
            .extend([Tech::GUNPOWDER, Tech::AMPHIBIOUS_WARFARE]);
        // The literal source comparison permits a higher-desire later unit
        // over stock+40; the stock limit is not a hard affordability filter.
        let budget = self.preferred_city_defender(&self.cities[0]) == UnitKind::Marines;
        self.cities[0].shields = 20;
        let stocked = self.preferred_city_defender(&self.cities[0]) == UnitKind::Marines;
        self.cities[0].shields = 0;
        // Isolate the defender recruitment choice from the native veteran
        // training-building stage, which can replace a sub-emergency bid.
        self.cities[0].buildings.push(Building::BARRACKS_II);
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        self.cities[0].production = Production::Building(Building::GRANARY);
        self.prepare_ai_city_defenses(1);
        let quiet = self.cities[0].production == Production::Building(Building::GRANARY);
        self.spawn_unit(HUMAN, UnitKind::Cannon, nx, ny);
        self.prepare_ai_city_defenses(1);
        // The complete native upgrade chain makes Musketeers obsolete once
        // Marines are available, even if intermediate Riflemen tech is absent.
        let responded = self.cities[0].production == Production::Unit(UnitKind::Marines);
        (budget && stocked, quiet, responded)
    }

    pub(crate) fn ai_formal_guard_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        self.ai_guard_targets.clear();
        self.ai_unit_guards.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
            tile.base = None;
        }
        let charge = self.spawn_unit(1, UnitKind::Catapult, 10, 20);
        let guard = self.spawn_unit(1, UnitKind::Musketeers, 10, 20);
        self.request_ai_guard(charge);
        let selected =
            self.run_ai_guard(guard) && self.ai_guard_state(charge) == GuardState::Assigned(guard);
        self.units
            .iter_mut()
            .find(|u| u.id == guard)
            .unwrap()
            .moves_left = 0;
        let waited = self.try_move_unit(charge, 1, 0) == MoveResult::Blocked;
        self.units
            .iter_mut()
            .find(|u| u.id == guard)
            .unwrap()
            .moves_left = 6;
        self.try_move_unit(charge, 1, 0);
        let actor = self.unit_by_id(charge).unwrap().clone();
        let followed = self
            .units
            .iter()
            .any(|u| u.id == guard && (u.x, u.y) == (actor.x, actor.y));
        self.units
            .iter_mut()
            .find(|u| u.id == charge)
            .unwrap()
            .moves_left = 6;
        let target = offset_native_by_map(actor.x, actor.y, 1, 0);
        let enemy = self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(1);
        let attack = self.try_move_unit(charge, 1, 0) == MoveResult::Fought
            && self.units.iter().all(|u| u.id != enemy);
        (selected, waited && followed, attack)
    }
}
