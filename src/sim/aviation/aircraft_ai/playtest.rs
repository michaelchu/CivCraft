//! Focused gameplay scenarios for the parent subsystem.

use super::*;

impl super::Simulation {
    pub(crate) fn ai_source_hunter_launch_playtest(&mut self) -> (bool, bool, bool) {
        let mut outcomes = [false; 3];
        for (case, (distance, kind)) in [
            (2, UnitKind::CruiseMissile),
            (3, UnitKind::CruiseMissile),
            (2, UnitKind::Nuclear),
        ]
        .into_iter()
        .enumerate()
        {
            self.units.clear();
            self.cities.clear();
            self.prune_unit_ai_state();
            self.sign_pact(1, HUMAN, Pact::War);
            self.players[1]
                .known
                .extend([Tech::ADVANCED_FLIGHT, Tech::ROCKETRY]);
            for tile in &mut self.tiles {
                tile.terrain = Terrain::Sea;
                tile.base = None;
                tile.owner = None;
                tile.hut = false;
                tile.known_by.fill(true);
            }
            let boat = self.spawn_unit(1, UnitKind::Submarine, 10, 20);
            let missile = self.spawn_unit(1, kind, 10, 20);
            let loaded = self.board_specific_ship(missile, boat);
            let point = offset_native_by_map(10, 20, distance, 0);
            let victim = self.spawn_unit(HUMAN, UnitKind::Caravel, point.0, point.1);
            self.units.last_mut().unwrap().hp = 1;
            self.set_random_seed(4);
            self.source_hunter_launch(boat, victim);
            outcomes[case] = loaded
                && if case == 1 {
                    self.units.iter().any(|unit| unit.id == victim)
                        && self
                            .units
                            .iter()
                            .any(|unit| unit.id == missile && unit.transport == Some(boat))
                } else {
                    self.units
                        .iter()
                        .all(|unit| unit.id != victim && unit.id != missile)
                };
        }
        (outcomes[0], outcomes[1], outcomes[2])
    }

    pub(crate) fn ai_fuel_mission_playtest(&mut self) -> bool {
        let (id, victim) = self.ai_aircraft_advisor_playtest_setup();
        let home = (self.cities[0].x, self.cities[0].y);
        let fighter = self
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        self.run_ai_strike_aircraft(&fighter);
        let sortie = self.units.iter().all(|unit| unit.id != victim)
            && self
                .units
                .iter()
                .any(|unit| unit.id == id && (unit.x, unit.y) == home);
        let (id, victim) = self.ai_aircraft_advisor_playtest_setup();
        let plane = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
        plane.kind = UnitKind::Bomber;
        plane.hp = plane.kind.stats().hp;
        plane.moves_left = plane.kind.stats().moves * MOVE_FRAGMENTS;
        plane.fuel = plane.kind.fuel_capacity();
        let bomber = plane.clone();
        let target = self.unit_by_id(victim).unwrap();
        let target_point = (target.x, target.y);
        let value = self.aircraft_target_value(&bomber, target.x, target.y);
        let mut map = super::fuel_paths::FuelPaths::new(self, &bomber, true).unwrap();
        let strict_bound = value > 0.0
            && map.movement_cost(self, target_point.0, target_point.1) == Some(bomber.moves_left)
            && self.aircraft_attack_path(&bomber).is_none();
        self.run_ai_strike_aircraft(&bomber);
        let stayed = self.units.iter().any(|unit| unit.id == victim)
            && self.units.iter().any(|unit| {
                unit.id == id && (unit.x, unit.y) == home && unit.moves_left == bomber.moves_left
            });
        self.units.retain(|unit| unit.id == id);
        self.prune_unit_ai_state();
        let start = offset_native_by_map(home.0, home.1, 1, 0);
        let target = offset_native_by_map(home.0, home.1, 4, 0);
        let actor = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
        actor.kind = UnitKind::Fighter;
        actor.hp = actor.kind.stats().hp;
        actor.fuel = 1;
        actor.moves_left = 3 * MOVE_FRAGMENTS;
        (actor.x, actor.y) = start;
        let actor = actor.clone();
        let mut map = super::fuel_paths::FuelPaths::new(self, &actor, true).unwrap();
        let waited = if let Some(path) = map.path(self, target.0, target.1) {
            let includes_wait = path.contains(&(0, 0));
            self.follow_source_ai_path(id, path);
            includes_wait
                && self.units.iter().any(|unit| {
                    unit.id == id
                        && (unit.x, unit.y) == home
                        && unit.moves_left == 2 * MOVE_FRAGMENTS
                })
        } else {
            false
        };
        sortie && strict_bound && stayed && waited
    }
}
