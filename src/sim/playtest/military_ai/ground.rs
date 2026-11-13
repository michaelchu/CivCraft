//! Implements ground behavior within the military ai subsystem.

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
        self.sign_pact(1, HUMAN, Pact::War);
        let marine = self.spawn_unit(1, UnitKind::Marine, x, y);
        let bad_odds = self.spawn_unit(HUMAN, UnitKind::ModernArmor, nx, ny);
        self.run_ai_player(1);
        let held = self.units.iter().any(|unit| unit.id == bad_odds)
            && self
                .units
                .iter()
                .any(|unit| unit.id == marine && (unit.x, unit.y) == (x, y));
        self.update_activities_and_restore();
        self.units.retain(|unit| unit.id != bad_odds);
        let cannon = self.spawn_unit(HUMAN, UnitKind::Cannon, nx, ny);
        self.units.last_mut().unwrap().hp = 1;
        let guard = self.spawn_unit(1, UnitKind::Warrior, x, y);
        self.units.last_mut().unwrap().moves_left = 0;
        // The original Marine is reserved as the city defender; a second
        // movable Marine tests whether a spare can counterattack.
        self.spawn_unit(1, UnitKind::Marine, x, y);
        self.set_random_seed(1);
        self.run_ai_player(1);
        let attack = self.units.iter().all(|unit| unit.id != cannon)
            && self
                .units
                .iter()
                .any(|unit| unit.id == guard && (unit.x, unit.y) == (x, y));

        self.units.clear();
        self.sign_pact(1, HUMAN, Pact::War);
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        let enemy = self.spawn_unit(HUMAN, UnitKind::Marine, nx, ny);
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
        // Keep the home city outside the cavalry's grave-danger horizon.
        // Freeciv retains a home defender when source and front cities are
        // equally urgent; this enemy still reaches the front city first.
        let enemy = point(7);
        self.create_city(1, home.0, home.1);
        self.create_city(1, front.0, front.1);
        self.sign_pact(1, HUMAN, Pact::War);
        let marine = self.spawn_unit(1, UnitKind::Marine, home.0, home.1);
        let guard = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        self.spawn_unit(1, UnitKind::Warrior, front.0, front.1);
        // Keep an attack-capable enemy cavalry unit outside the front city.
        self.spawn_unit(HUMAN, UnitKind::Cavalry, enemy.0, enemy.1);
        let no_near_route = std::collections::HashSet::new();
        let too_far = !self
            .reinforce_ai_cities(1, &no_near_route)
            .contains(&marine);
        for step in 0..=3 {
            let (x, y) = point(step);
            self.tile_mut(x, y).road = true;
        }
        let dispatched = self
            .reinforce_ai_cities(1, &no_near_route)
            .contains(&marine);
        let position = self
            .units
            .iter()
            .find(|unit| unit.id == marine)
            .map(|unit| (unit.x, unit.y));
        let guarded = self
            .units
            .iter()
            .any(|unit| unit.id == guard && (unit.x, unit.y) == home);
        // The C2C3 road move cost is two fragments per tile. A Marine cannot
        // cross three tiles in one movement allotment, so assert dispatch,
        // route retention, and progress instead of same-turn arrival.
        too_far
            && dispatched
            && position.is_some_and(|position| position != home)
            && self.unit_goto_tiles.get(&marine) == Some(&front)
            && guarded
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
        self.sign_pact(1, HUMAN, Pact::War);
        let guard = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let marine = self.spawn_unit(1, UnitKind::Marine, home.0, home.1);
        self.units
            .iter_mut()
            .find(|unit| unit.id == marine)
            .unwrap()
            .moves_left = UnitKind::Marine.stats().moves * MOVE_FRAGMENTS;
        self.spawn_unit(1, UnitKind::Warrior, front.0, front.1);
        // A mobile attacker at war creates a real incoming city threat.
        let horseman = self.spawn_unit(HUMAN, UnitKind::Horseman, enemy.0, enemy.1);
        self.units
            .iter_mut()
            .find(|unit| unit.id == horseman)
            .unwrap()
            .moves_left = UnitKind::Horseman.stats().moves * MOVE_FRAGMENTS;
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
        // Musketmen require connected Saltpeter in the Conquests roster.
        self.tile_mut(x, y).road = true;
        let saltpeter = self.tile_mut(nx, ny);
        saltpeter.terrain = Terrain::Hills;
        saltpeter.resource = Some(Resource::Saltpeter);
        saltpeter.road = true;
        saltpeter.owner = Some(1);
        let immediate = self.spawn_unit(HUMAN, UnitKind::Cannon, nx, ny);
        let empty = self.unit_buildable(&self.cities[0], UnitKind::Musketman);
        self.units.retain(|unit| unit.id != immediate);
        let cannon = self.spawn_unit(HUMAN, UnitKind::Cannon, tx, ty);
        let guard = self.spawn_unit(1, UnitKind::Musketman, x, y);
        self.spawn_unit(1, UnitKind::Musketman, x, y);
        self.cities[0].production = Production::Building(Building::GRANARY);
        // Inspect combat effects directly; city-wide production priority is
        // not a source-defined roster rule.
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
        self.tile_mut(nx, ny).terrain = Terrain::Sea;
        self.spawn_unit(HUMAN, UnitKind::Battleship, nx, ny);
        let defender = self.unit_by_id(guard).unwrap();
        let ordinary = self.defense_power_with_building(defender, x, y, UnitKind::Battleship, None);
        let naval = self.defense_power_with_building(
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
        (empty, land_bonus, naval)
    }

    pub(crate) fn ai_city_defense_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        let (x, y) = (10, 24);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(x, y).river = false;
        for (adjacent_x, adjacent_y) in self.map_wrap.civ3_neighbors(x, y) {
            let tile = self.tile_mut(adjacent_x, adjacent_y);
            tile.terrain = Terrain::Grassland;
            tile.river = false;
        }
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(1, x, y);
        self.players[1].known.extend([
            Tech::BRONZE_WORKING,
            Tech::GUNPOWDER,
            Tech::AMPHIBIOUS_WARFARE,
        ]);
        let saltpeter = offset_native_by_map(x, y, -1, 0);
        let resource = self.tile_mut(saltpeter.0, saltpeter.1);
        resource.terrain = Terrain::Desert;
        resource.owner = Some(1);
        resource.resource = Some(Resource::Saltpeter);
        resource.road = true;
        let source_defender = |game: &Simulation| {
            let kind = game.preferred_city_defender(&game.cities[0]);
            super::super::action_rules::freeciv_unit_has_role(kind, "DefendGood")
                || super::super::action_rules::freeciv_unit_has_role(kind, "DefendOk")
        };
        let budget = source_defender(self);
        self.cities[0].shields = 20;
        let stocked = source_defender(self);
        self.cities[0].shields = 0;
        // Isolate the defender recruitment choice from the native veteran
        // training-building stage, which can replace a sub-emergency bid.
        self.cities[0].buildings.push(Building::BARRACKS_II);
        self.spawn_unit(1, UnitKind::Warrior, x, y);
        self.cities[0].production = Production::Building(Building::GRANARY);
        let no_emergency_bid = self.native_city_defense_choice(&self.cities[0]).0.is_none();
        self.prepare_ai_city_defenses(1);
        let quiet = no_emergency_bid
            && !matches!(self.cities[0].production, Production::Unit(kind)
                if super::super::action_rules::freeciv_unit_has_role(kind, "DefendGood")
                    || super::super::action_rules::freeciv_unit_has_role(kind, "DefendOk"));
        self.sign_pact(1, HUMAN, Pact::War);
        self.spawn_unit(HUMAN, UnitKind::Warrior, nx, ny);
        self.prepare_ai_city_defenses(1);
        let responded = matches!(self.cities[0].production, Production::Unit(kind)
            if super::super::action_rules::freeciv_unit_has_role(kind, "DefendGood")
                || super::super::action_rules::freeciv_unit_has_role(kind, "DefendOk"));
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
        // Use a charge with lower defense and higher attack than the Musketman
        // guard, matching Freeciv's bodyguard compatibility requirements.
        let charge = self.spawn_unit(1, UnitKind::Longbowman, 10, 20);
        let guard = self.spawn_unit(1, UnitKind::Musketman, 10, 20);
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
        self.sign_pact(1, HUMAN, Pact::War);
        let enemy = self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(1);
        let attack = self.try_move_unit(charge, 1, 0) == MoveResult::Fought
            && self.units.iter().all(|u| u.id != enemy);
        (selected, waited && followed, attack)
    }
}

#[cfg(test)]
mod focused_p13_tests {
    use super::*;

    #[test]
    fn reserved_defender_can_reinforce_a_more_urgent_city() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (dispatched, arrived, home_guarded) = sim.ai_reinforcements_playtest();
        assert!(
            dispatched,
            "reserved Marine was not dispatched: {dispatched}, {arrived}, {home_guarded}"
        );
        assert!(arrived, "Marine did not reach the threatened city");
        assert!(home_guarded, "source city lost its Warrior");
    }
}
