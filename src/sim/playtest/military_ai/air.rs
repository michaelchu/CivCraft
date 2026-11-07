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
        let paratrooper = self.spawn_unit(1, UnitKind::Paratroopers, home.0, home.1);
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
        let para = self.spawn_unit(1, UnitKind::Paratroopers, home.0, home.1);
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
        let raider = self.spawn_unit(1, UnitKind::Paratroopers, home.0, home.1);
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
        let kind = Production::Unit(UnitKind::Paratroopers);
        let locked = !self.can_produce(&self.cities[0], kind);
        self.players[HUMAN].known.push(Tech::COMBINED_ARMS);
        let unlocked = self.can_produce(&self.cities[0], kind);
        let actor = self.spawn_unit(HUMAN, UnitKind::Paratroopers, home.0, home.1);
        let out_of_range = !self.apply(Command::Paradrop(actor, outside.0, outside.1));
        (actor, target, locked && unlocked && out_of_range)
    }

    pub(crate) fn ai_fighter_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.hut = false;
        }
        let home = (10, 24);
        let near = offset_native_by_map(home.0, home.1, 3, 0);
        let far = offset_native_by_map(home.0, home.1, 8, 0);
        self.create_city(1, home.0, home.1);
        self.tile_mut(near.0, near.1).known_by[1] = true;
        self.tile_mut(far.0, far.1).known_by[1] = true;
        let fighter = self.spawn_unit(1, UnitKind::Fighter, home.0, home.1);
        let close = self.spawn_unit(HUMAN, UnitKind::Bomber, near.0, near.1);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(1);
        self.run_ai_player(1);
        let intercepted = self.units.iter().all(|unit| unit.id != close);
        self.restore_aircraft();
        let refueled = self.units.iter().any(|unit| {
            unit.id == fighter
                && (unit.x, unit.y) == home
                && unit.fuel == UnitKind::Fighter.fuel_capacity()
        });
        self.update_activities_and_restore();
        if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == fighter) {
            unit.fuel = 1;
        }
        let distant = self.spawn_unit(HUMAN, UnitKind::Bomber, far.0, far.1);
        self.units.last_mut().unwrap().hp = 1;
        self.run_ai_player(1);
        let skipped = self.units.iter().any(|unit| unit.id == distant)
            && self
                .units
                .iter()
                .any(|unit| unit.id == fighter && (unit.x, unit.y) == home);
        (intercepted, skipped, refueled)
    }

    pub(crate) fn ai_carrier_sortie_playtest(&mut self, reposition: bool) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        self.sign_pact(1, HUMAN, Pact::War);
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.base = None;
            tile.hut = false;
            tile.owner = None;
            tile.known_by[1] = true;
        }
        let deck = (10, 24);
        let target = offset_native_by_map(deck.0, deck.1, if reposition { 11 } else { 2 }, 0);
        self.tile_mut(target.0, target.1).terrain = Terrain::Grassland;
        let carrier = self.spawn_unit(1, UnitKind::Carrier, deck.0, deck.1);
        // Cargo load order appoints the Fighter as passenger-in-charge.
        // The reserve Bomber does not receive an independent cargo dispatch.
        let fighter = self.spawn_unit(1, UnitKind::Fighter, deck.0, deck.1);
        assert!(self.board_specific_ship(fighter, carrier));
        let bomber = self.spawn_unit(1, UnitKind::Bomber, deck.0, deck.1);
        assert!(self.board_specific_ship(bomber, carrier));
        let victim = self.spawn_unit(HUMAN, UnitKind::Cannon, target.0, target.1);
        self.units.last_mut().unwrap().hp = if reposition { 2 } else { 1 };
        self.set_random_seed(1);
        self.run_ai_player(1);
        let station = self
            .units
            .iter()
            .find(|unit| unit.id == carrier)
            .map(|unit| (unit.x, unit.y))
            .unwrap();
        let launched =
            self.units
                .iter()
                .any(|unit| unit.id == fighter && unit.moved)
                && self.units.iter().all(|unit| unit.id != victim)
                && self.units.iter().any(|unit| {
                    unit.id == bomber && unit.transport == Some(carrier) && !unit.moved
                });
        self.restore_aircraft();
        self.update_activities_and_restore();
        self.units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap()
            .fuel = 1;
        self.run_ai_player(1);
        self.restore_aircraft();
        let steady = self
            .units
            .iter()
            .any(|unit| unit.id == carrier && (unit.x, unit.y) == station);
        let recovered = self.units.iter().any(|unit| {
            unit.id == fighter
                && unit.transport == Some(carrier)
                && unit.fuel == UnitKind::Fighter.fuel_capacity()
        });
        let source_mission = if reposition {
            // Native ferries do not reposition to extend an aircraft sortie.
            station == deck && self.units.iter().any(|unit| unit.id == victim) && !launched
        } else {
            launched
        };
        (source_mission, steady, recovered)
    }

    pub(crate) fn ai_bomber_playtest(&mut self) -> (bool, bool, bool) {
        // Retain the existing desktop hook name; this now tests the native
        // Fighter sortie. OneAttack Bomber's strict PF cutoff is tested apart.
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        self.sign_pact(1, HUMAN, Pact::War);
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.hut = false;
            tile.owner = None;
            tile.known_by[1] = true;
        }
        let home = (10, 24);
        let target = offset_native_by_map(home.0, home.1, 2, 0);
        self.create_city(1, home.0, home.1);
        let fighter = self.spawn_unit(1, UnitKind::Fighter, home.0, home.1);
        let victim = self.spawn_unit(HUMAN, UnitKind::Cannon, target.0, target.1);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(1);
        self.run_ai_player(1);
        let struck = self.units.iter().all(|unit| unit.id != victim)
            && self
                .units
                .iter()
                .any(|unit| unit.id == fighter && unit.moved);
        self.restore_aircraft();
        self.update_activities_and_restore();
        self.units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap()
            .fuel = 1;
        self.run_ai_player(1);
        self.restore_aircraft();
        let returned = self.units.iter().any(|unit| {
            unit.id == fighter
                && (unit.x, unit.y) == home
                && unit.fuel == UnitKind::Fighter.fuel_capacity()
        });
        self.cities.clear();
        self.update_activities_and_restore();
        let guarded = self.spawn_unit(HUMAN, UnitKind::Cannon, target.0, target.1);
        self.units.last_mut().unwrap().hp = 1;
        self.run_ai_player(1);
        let skipped = self
            .units
            .iter()
            .any(|unit| unit.id == guarded && unit.hp == 1)
            && self
                .units
                .iter()
                .any(|unit| unit.id == fighter && (unit.x, unit.y) == home && !unit.moved);
        (struck, returned, skipped)
    }

    pub(crate) fn ai_embarked_paradrop_danger_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
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
        let marine = self.spawn_unit(HUMAN, UnitKind::Marines, sea.0, sea.1);
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
        let para = self.spawn_unit(HUMAN, UnitKind::Paratroopers, airbase.0, airbase.1);
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
