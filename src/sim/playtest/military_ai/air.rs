//! Implements air behavior within the military ai subsystem.

use super::super::*;

impl Simulation {
    pub(crate) fn ai_paradrop_stack_playtest(&mut self) -> (bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by[1] = true;
            tile.hut = false;
            tile.base = None;
        }
        let home = (10, 24);
        let target = offset_native_by_map(home.0, home.1, 6, 0);
        self.create_city(1, home.0, home.1);
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let paratrooper = self.spawn_unit(1, UnitKind::Paratrooper, home.0, home.1);
        let unit = self.units.last().unwrap().clone();
        self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
        let declined = !self.run_ai_paratrooper(&unit);
        for _ in 0..2 {
            self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
        }
        self.set_random_seed(1);
        let struck = self.run_ai_paratrooper(&unit)
            && self.units.iter().all(|unit| unit.owner != HUMAN)
            && self.units.iter().any(|unit| {
                unit.id == paratrooper && (unit.x, unit.y) == target && unit.moves_left == 0
            });
        (declined, struck)
    }

    pub(crate) fn ai_paradrop_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        let home = (10, 24);
        let front = offset_native_by_map(home.0, home.1, 6, 0);
        let enemy = offset_native_by_map(home.0, home.1, -6, 0);
        let threat = offset_native_by_map(front.0, front.1, 1, 0);
        for (x, y) in [home, front, enemy, threat] {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.known_by[1] = true;
        }
        self.create_city(1, home.0, home.1);
        self.create_city(1, front.0, front.1);
        self.create_city(HUMAN, enemy.0, enemy.1);
        self.spawn_unit(HUMAN, UnitKind::Cannon, threat.0, threat.1);
        let guard = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let para = self.spawn_unit(1, UnitKind::Paratrooper, home.0, home.1);
        self.run_ai_player(1);
        let defended = self
            .units
            .iter()
            .any(|unit| unit.id == para && (unit.x, unit.y) == front)
            && self
                .units
                .iter()
                .any(|unit| unit.id == guard && (unit.x, unit.y) == home)
            && self
                .cities
                .iter()
                .any(|city| city.owner == HUMAN && (city.x, city.y) == enemy);
        let defender = self.spawn_unit(HUMAN, UnitKind::Warrior, enemy.0, enemy.1);
        let raider = self.spawn_unit(1, UnitKind::Paratrooper, home.0, home.1);
        let raider_unit = self.units.last().unwrap().clone();
        let refused = !self.run_ai_paratrooper(&raider_unit);
        self.units.retain(|unit| unit.id != defender);
        let captured = self.run_ai_paratrooper(&raider_unit)
            && self
                .units
                .iter()
                .any(|unit| unit.id == raider && (unit.x, unit.y) == enemy)
            && self
                .cities
                .iter()
                .any(|city| city.owner == 1 && (city.x, city.y) == enemy);
        (defended, refused, captured)
    }

    pub(crate) fn paradrop_playtest_setup(&mut self) -> (u32, (i32, i32), bool) {
        self.units.clear();
        let home = (10, 24);
        let target = offset_native_by_map(home.0, home.1, 6, 0);
        let outside = offset_native_by_map(home.0, home.1, 11, 0);
        for (x, y) in [home, target, outside] {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.known_by[HUMAN] = true;
            tile.hut = false;
        }
        self.create_city(HUMAN, home.0, home.1);
        self.create_city(1, target.0, target.1);
        let kind = Production::Unit(UnitKind::Paratrooper);
        let locked = !self.can_produce(&self.cities[0], kind);
        self.players[HUMAN].known.push(Tech::COMBINED_ARMS);
        let unlocked = self.can_produce(&self.cities[0], kind);
        let actor = self.spawn_unit(HUMAN, UnitKind::Paratrooper, home.0, home.1);
        let out_of_range = !self.apply(Command::Paradrop(actor, outside.0, outside.1));
        (actor, target, locked && unlocked && out_of_range)
    }

    pub(crate) fn ai_fighter_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.sign_pact(HUMAN, 1, Pact::War);
        self.sign_pact(HUMAN, BARBARIANS, Pact::War);
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.hut = false;
            tile.known_by[1] = true;
        }
        let home = (10, 24);
        let near = offset_native_by_map(home.0, home.1, 2, 0);
        let far = offset_native_by_map(home.0, home.1, 8, 0);
        self.create_city(1, home.0, home.1);
        self.tile_mut(near.0, near.1).known_by[1] = true;
        self.tile_mut(far.0, far.1).known_by[1] = true;
        let fighter = self.spawn_unit(1, UnitKind::Fighter, home.0, home.1);
        self.units.last_mut().unwrap().moves_left = UnitKind::Fighter.stats().moves * MOVE_FRAGMENTS;
        let close = self.spawn_unit(HUMAN, UnitKind::Bomber, near.0, near.1);
        self.units.last_mut().unwrap().hp = 1;
        self.units.last_mut().unwrap().moves_left = UnitKind::Bomber.stats().moves * MOVE_FRAGMENTS;
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        self.refresh_visibility();
        self.run_ai_player(1);
        let assigned = self
            .unit_by_id(fighter)
            .is_some_and(|unit| unit.state == UnitState::AirSuperiority);
        self.native_random = None;
        self.rng = 2; // 22% interception roll, below the stock 50% chance.
        let bomber = self.unit_by_id(close).unwrap().clone();
        self.try_bombard(&bomber, home.0, home.1);
        let intercepted = assigned && self.units.iter().all(|unit| unit.id != close);
        self.restore_aircraft();
        let refueled = self.units.iter().any(|unit| {
            unit.id == fighter
                && (unit.x, unit.y) == home
                && unit.fuel == UnitKind::Fighter.fuel_capacity()
        });
        self.update_activities_and_restore();
        if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == fighter) {
            unit.fuel = 1;
            unit.state = UnitState::Idle;
        }
        let distant = self.spawn_unit(HUMAN, UnitKind::Bomber, far.0, far.1);
        self.units.last_mut().unwrap().hp = 1;
        let plane = self.unit_by_id(fighter).unwrap();
        let target = self.unit_by_id(distant).unwrap();
        let skipped = self.map_distance(home.0, home.1, target.x, target.y)
            > plane.kind.operational_range()
            && self
                .route_cost_within(plane, target.x, target.y, plane.moves_left)
                .is_none();
        (intercepted, skipped, refueled)
    }

    pub(crate) fn ai_embarked_paradrop_danger_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.players[1].known = vec![Tech::BRONZE_WORKING];
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Sea;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.base = None;
            tile.hut = false;
            tile.owner = None;
            tile.known_by[1] = false;
        }
        let home = (10, 24);
        self.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
        self.create_city(1, home.0, home.1);
        self.sign_pact(1, HUMAN, Pact::War);
        let sea = offset_native_by_map(home.0, home.1, -4, 0);
        let ferry = self.spawn_unit(HUMAN, UnitKind::Transport, sea.0, sea.1);
        let infantry = self.spawn_unit(HUMAN, UnitKind::Warrior, sea.0, sea.1);
        let marine = self.spawn_unit(HUMAN, UnitKind::Marine, sea.0, sea.1);
        if !self.board_specific_ship(infantry, ferry) || !self.board_specific_ship(marine, ferry) {
            return false;
        }
        self.tile_mut(sea.0, sea.1).known_by[1] = false;
        let city = self.cities[0].clone();
        let arrival = |sim: &Self, id| {
            sim.city_threat_arrivals(&city)
                .find(|(u, _, _)| u.id == id)
                .map(|(_, _, turn)| turn)
        };
        let transported = arrival(self, infantry) == Some(1)
            && arrival(self, marine) == Some(0)
            && arrival(self, ferry) == Some(0)
            && self.city_defense_production(&city).is_some();
        self.remove_units_with_cargo(vec![ferry]);
        let airbase = offset_native_by_map(home.0, home.1, 9, 0);
        self.tile_mut(airbase.0, airbase.1).terrain = Terrain::Grassland;
        self.tile_mut(airbase.0, airbase.1).base = Some(Base::Airbase);
        self.tile_mut(airbase.0, airbase.1).owner = Some(HUMAN);
        let para = self.spawn_unit(HUMAN, UnitKind::Paratrooper, airbase.0, airbase.1);
        self.tile_mut(airbase.0, airbase.1).known_by[1] = false;
        let airborne =
            arrival(self, para) == Some(0) && self.city_defense_production(&city).is_some();
        let before = self.unit_by_id(para).map(|u| (u.x, u.y));
        self.sign_pact(1, HUMAN, Pact::Peace);
        self.ai_war_plans.insert(
            (1, HUMAN),
            diplomacy::WarPlan {
                countdown: 3,
                reason: diplomacy::WarReason::Spoils,
            },
        );
        let anticipated = arrival(self, para) == Some(0);
        let after = self.unit_by_id(para).map(|u| (u.x, u.y));
        transported && airborne && anticipated && before == after
    }
}
