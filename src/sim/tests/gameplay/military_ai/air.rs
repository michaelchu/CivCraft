//! Unit checks and fixtures for private simulation behavior.

use super::super::*;

use crate::sim::fixtures::{capture_known_tiles, give_unit_full_movement};

impl Simulation {
    pub(crate) fn ai_paradrop_stack_test(&mut self) -> (bool, bool) {
        self.units.clear();
        self.sign_pact(1, HUMAN, Pact::War);
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by.set(1, true);
            tile.hut = false;
            tile.base = None;
        }
        let home = (10, 24);
        let target = offset_native_by_map(home.0, home.1, 6, 0);
        self.players[1].known.push(Tech::COMBINED_ARMS);
        self.create_city(1, home.0, home.1);
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let paratrooper = self.spawn_unit(1, UnitKind::Paratrooper, home.0, home.1);
        give_unit_full_movement(self, paratrooper, UnitKind::Paratrooper);
        let unit = self.units.last().unwrap().clone();
        self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
        self.refresh_visibility();
        capture_known_tiles(self, 1);
        let declined = !self.run_ai_paratrooper(&unit);
        self.units.retain(|unit| unit.id != paratrooper);
        let paratrooper = self.spawn_unit(1, UnitKind::Paratrooper, home.0, home.1);
        give_unit_full_movement(self, paratrooper, UnitKind::Paratrooper);
        let unit = self.unit_by_id(paratrooper).unwrap().clone();
        for _ in 0..2 {
            self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
        }
        self.refresh_visibility();
        capture_known_tiles(self, 1);
        self.set_random_seed(1);
        let struck = self.run_ai_paratrooper(&unit)
            && self.unit_by_id(paratrooper).is_some_and(|unit| {
                unit.paradropped
                    && (unit.x, unit.y) != home
                    && self.map_distance(unit.x, unit.y, target.0, target.1) <= 1
            });
        (declined, struck)
    }

    pub(crate) fn paradrop_fixture(&mut self) -> (u32, (i32, i32), bool) {
        self.units.clear();
        self.sign_pact(HUMAN, 1, Pact::War);
        let home = (10, 24);
        let target = offset_native_by_map(home.0, home.1, 6, 0);
        let outside = offset_native_by_map(home.0, home.1, 11, 0);
        for (x, y) in [home, target, outside] {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.known_by.set(HUMAN, true);
            tile.seen = true;
            tile.hut = false;
        }
        let oil = offset_native_by_map(home.0, home.1, 1, 0);
        let rubber = offset_native_by_map(home.0, home.1, -1, 0);
        self.tile_mut(home.0, home.1).road = true;
        for (position, resource) in [(oil, Resource::Oil), (rubber, Resource::Rubber)] {
            let tile = self.tile_mut(position.0, position.1);
            tile.resource = Some(resource);
            tile.terrain = if resource == Resource::Oil {
                Terrain::Plains
            } else {
                Terrain::Forest
            };
            tile.owner = Some(HUMAN);
            tile.road = true;
        }
        self.create_city(HUMAN, home.0, home.1);
        self.create_city(1, target.0, target.1);
        self.cities.last_mut().unwrap().size = 2;
        let kind = Production::Unit(UnitKind::Paratrooper);
        let locked = !self.can_produce(&self.cities[0], kind);
        // Unit buildability follows the Civ III catalog gate; C2C3 still
        // supplies the selected Paradrop action and range mechanics.
        self.players[HUMAN].known.push(Tech::ADVANCED_FLIGHT);
        let unlocked = self.can_produce(&self.cities[0], kind);
        let actor = self.spawn_unit(HUMAN, UnitKind::Paratrooper, home.0, home.1);
        give_unit_full_movement(self, actor, UnitKind::Paratrooper);
        self.refresh_visibility();
        let mut remembered = vec![vec![false; self.tiles.len()]; self.players.len()];
        for (x, y) in [home, target, outside] {
            remembered[HUMAN][self.map_wrap.index(x, y)] = true;
        }
        self.remember_render_map(&remembered);
        let out_of_range = !self.apply(Command::Paradrop(actor, outside.0, outside.1));
        (actor, target, locked && unlocked && out_of_range)
    }

    pub(crate) fn ai_fighter_test(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.sign_pact(HUMAN, 1, Pact::War);
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.hut = false;
            tile.known_by.set(1, true);
        }
        let home = (10, 24);
        let near = offset_native_by_map(home.0, home.1, 1, 0);
        let far = offset_native_by_map(home.0, home.1, 8, 0);
        self.create_city(1, home.0, home.1);
        self.tile_mut(near.0, near.1).known_by.set(1, true);
        self.tile_mut(far.0, far.1).known_by.set(1, true);
        let fighter = self.spawn_unit(1, UnitKind::Fighter, home.0, home.1);
        self.units.last_mut().unwrap().moves_left =
            UnitKind::Fighter.stats().moves * MOVE_FRAGMENTS;
        let close = self.spawn_unit(HUMAN, UnitKind::Bomber, near.0, near.1);
        self.units.last_mut().unwrap().hp = 1;
        self.units.last_mut().unwrap().moves_left = UnitKind::Bomber.stats().moves * MOVE_FRAGMENTS;
        self.refresh_visibility();
        self.native_random = None;
        self.rng = 1;
        self.run_ai_player(1);
        // Freeciv has no Air Superiority patrol mission or automatic
        // interception. The AI should leave this fighter on its base; the
        // ruleset target-tile attack path is covered by the aviation combat
        // scenario, where combat is an explicit move into the target tile.
        let explicit_strike = self.unit_by_id(close).is_none()
            && self.unit_by_id(fighter).is_some_and(|plane| {
                plane.moves_left < UnitKind::Fighter.stats().moves * MOVE_FRAGMENTS
            });
        self.update_activities_and_restore();
        let refuel_test = self.spawn_unit(1, UnitKind::Fighter, home.0, home.1);
        self.units
            .iter_mut()
            .find(|unit| unit.id == refuel_test)
            .unwrap()
            .fuel = 1;
        self.restore_aircraft();
        let refueled = self
            .unit_by_id(refuel_test)
            .is_some_and(|unit| unit.fuel == UnitKind::Fighter.fuel_capacity());
        let distant = self.spawn_unit(HUMAN, UnitKind::Bomber, far.0, far.1);
        self.units.last_mut().unwrap().hp = 1;
        let plane = self.unit_by_id(fighter).unwrap();
        let target = self.unit_by_id(distant).unwrap();
        let skipped = self.map_distance(home.0, home.1, target.x, target.y)
            > plane.kind.operational_range()
            && self
                .route_cost_within(plane, target.x, target.y, plane.moves_left)
                .is_none();
        (explicit_strike, skipped, refueled)
    }

    pub(crate) fn ai_embarked_paradrop_danger_test(&mut self) -> bool {
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
            tile.known_by.set(1, false);
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
        self.tile_mut(sea.0, sea.1).known_by.set(1, false);
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
        self.tile_mut(airbase.0, airbase.1).known_by.set(1, false);
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

#[cfg(test)]
mod focused_p13_tests {
    use super::*;

    #[test]
    fn requirements_use_civ3_paratrooper_tech_and_range() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (_, _, passed) = sim.paradrop_fixture();
        assert!(passed);
    }

    #[test]
    fn ai_paradrop_follow_up_matches_move_limited_source_behavior() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (_, struck) = sim.ai_paradrop_stack_test();
        assert!(
            struck,
            "AI did not execute a legal adjacent attack after landing"
        );
    }
}
