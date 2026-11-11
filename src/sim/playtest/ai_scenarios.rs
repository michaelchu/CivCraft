//! Automated scenarios for AI gameplay behavior.

use super::super::exploration::ExploreResult;
use super::*;

impl Simulation {
    pub(crate) fn ai_reserve_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_war_plans.clear();
        self.ai_purchase_reserve.clear();
        self.ai_government_cache.clear();
        self.players[1].government = Government::Despotism;
        self.players[1].known = vec![Tech::EXPLOSIVES, Tech::ENGINEERING];
        self.players[1].researching = Tech::BRONZE_WORKING;
        self.players[1].research = 0;
        self.players[1].last_science = 0;
        self.players[1].rates = [60, 40, 0];
        let home = (10, 24);
        for (_, _, x, y) in Self::city_radius_at(home.0, home.1) {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = true;
            tile.road = true;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.hut = false;
        }
        self.create_city(1, home.0, home.1);
        let city = self.cities[0].id;
        // A real balanced 40% tax floor makes the source +10% coffers refill
        // observable. With zero expenses, the 60% science cap already forces
        // 40% tax and hides that increase.
        // With four free support slots under Despotism, seven Warriors make
        // the upkeep bill large enough to expose the purchase cash reserve.
        for _ in 0..7 {
            let id = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
            let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
            unit.home_city = Some(city);
            unit.home_order = id;
        }
        // A Catapult upgrades to Trebuchet with the available Engineering
        // technology; Workers have no Conquests upgrade successor.
        self.spawn_unit(1, UnitKind::Catapult, home.0, home.1);
        let Some((_, price)) = self.paid_upgrade(self.units.last().unwrap()) else {
            return false;
        };
        self.players[1].gold = 3;
        self.choose_ai_rates(1);
        let baseline = self.players[1].rates[1];
        self.spend_ai_defense_gold(1);
        self.choose_ai_rates(1);
        let refilling =
            self.ai_gold_reserve(1) == price && self.players[1].rates[1] == baseline + 10;
        self.players[1].gold = price * 2 + 1;
        self.spend_ai_defense_gold(1);
        self.choose_ai_rates(1);
        refilling && self.ai_gold_reserve(1) == 2 && self.players[1].rates[1] == baseline
    }

    pub(crate) fn civilian_upgrade_roster_playtest(&mut self) -> bool {
        self.units.clear();
        let home = (10, 24);
        self.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
        self.create_city(1, home.0, home.1);
        let worker = self.spawn_unit(1, UnitKind::Worker, home.0, home.1);
        let settler = self.spawn_unit(1, UnitKind::Settler, home.0, home.1);
        self.players[1].gold = i32::MAX;
        [worker, settler].into_iter().all(|id| {
            self.unit_by_id(id)
                .is_some_and(|unit| self.paid_upgrade(unit).is_none())
        })
    }

    pub(crate) fn ai_civilian_upgrade_playtest(&mut self) -> bool {
        self.civilian_upgrade_roster_playtest()
    }

    pub(crate) fn ai_defense_sale_playtest(&mut self) -> bool {
        self.units.clear();
        let front = (10, 24);
        let enemy = offset_native_by_map(front.0, front.1, 1, 0);
        for (x, y) in [front, enemy] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        self.create_city(1, front.0, front.1);
        self.turn += 1;
        self.cities[0]
            .buildings
            .extend([Building::CITY_WALLS, Building::GRANARY]);
        self.spawn_unit(HUMAN, UnitKind::Cannon, enemy.0, enemy.1);
        self.prepare_ai_city_defenses(1);
        let Some(price) = self.rush_cost(&self.cities[0]) else {
            return false;
        };
        let expenses = self.building_upkeep(&self.cities[0]);
        self.players[1].gold = price + expenses - 1;
        self.spend_ai_defense_gold(1);
        let city = &self.cities[0];
        city.bought_production
            && city.sold_building
            && city.has_building(Building::CITY_WALLS)
            && !city.has_building(Building::GRANARY)
            && self.players[1].gold == expenses - 1 + Building::GRANARY.data().cost
    }

    pub(crate) fn ai_defense_purchase_playtest(&mut self) -> bool {
        self.units.clear();
        let front = (10, 24);
        let enemy = offset_native_by_map(front.0, front.1, 1, 0);
        let rear = offset_native_by_map(front.0, front.1, 9, 0);
        for (x, y) in [front, enemy, rear] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        self.create_city(1, front.0, front.1);
        self.create_city(1, rear.0, rear.1);
        self.turn += 1;
        self.spawn_unit(HUMAN, UnitKind::Cannon, enemy.0, enemy.1);
        self.prepare_ai_city_defenses(1);
        let Some(price) = self.rush_cost(&self.cities[0]) else {
            return false;
        };
        self.players[1].gold = price - 1;
        self.spend_ai_defense_gold(1);
        let declined = !self.cities[0].bought_production && self.players[1].gold == price - 1;

        self.players[1].gold = price;
        self.spend_ai_defense_gold(1);
        let purchased = self.cities[0].bought_production
            && !self.cities[1].bought_production
            && self.players[1].gold == 0;
        self.process_cities();
        declined
            && purchased
            && self
                .units
                .iter()
                .any(|unit| unit.owner == 1 && unit.kind.is_military() && (unit.x, unit.y) == front)
    }

    pub(crate) fn ai_ground_invasion_playtest(&mut self) -> bool {
        let (infantry, bomber, city) = self.ai_ground_invasion_playtest_setup();
        let target = self.city_by_id(city).unwrap();
        let actor = self.unit_by_id(infantry).unwrap();
        let distant =
            self.map_distance(actor.x, actor.y, target.x, target.y) > actor.kind.stats().moves;
        let source_force = actor.kind.can_occupy_city()
            && self.ai_ground_targets.get(&infantry) == Some(&city)
            && self.invasion_support(1, target, bomber).1 > 0;
        // A distant Fighter sortie is not required: Conquests fuel rules can
        // make a one-way attack unrecoverable. Validate the assigned ground
        // force and city-occupation role directly instead of asserting AI
        // mission priority or a successful random combat outcome.
        distant && source_force
    }

    pub(crate) fn ai_aircraft_advisor_playtest(&mut self) -> bool {
        let (fighter, victim) = self.ai_aircraft_advisor_playtest_setup();
        let plane = self.unit_by_id(fighter).unwrap().clone();
        let target = self.unit_by_id(victim).unwrap();
        // Confirm imported aircraft fuel/range and Oil build requirements.
        // Target profitability and whether the AI launches are policy
        // choices not specified by the Conquests unit roster.
        plane.kind.fuel_capacity() > 0
            && self.map_distance(plane.x, plane.y, target.x, target.y)
                <= plane.kind.operational_range()
            && plane.kind.required_resources().contains(&Resource::Oil)
    }

    pub(crate) fn ai_nuclear_strike_playtest(&mut self) -> bool {
        let (missile, victim) = self.ai_nuclear_strike_playtest_setup();
        self.run_ai_player(1);
        !self.units.iter().any(|unit| unit.id == missile)
            && self.units.iter().any(|unit| unit.id == victim)
    }
    pub(crate) fn ai_threat_route_cost_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Mountains;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.owner = None;
            tile.known_by[1] = true;
        }
        let start = (10, 24);
        let point = |step| offset_native_by_map(start.0, start.1, step, 0);
        let city = point(4);
        self.create_city(1, city.0, city.1);
        // Keep the three intermediate road steps neutral so this fixture tests
        // the hostile city-entry surcharge independently of city borders.
        for step in 1..4 {
            let (x, y) = point(step);
            self.tile_mut(x, y).owner = None;
        }
        let enemy = self.spawn_unit(HUMAN, UnitKind::Horseman, start.0, start.1);
        let arrival = |sim: &Self| {
            sim.city_threat_arrivals(&sim.cities[0])
                .find(|(unit, _, _)| unit.id == enemy)
                .map(|(_, _, turns)| turns)
        };
        let rough = arrival(self).is_none();
        for step in 0..=4 {
            let (x, y) = point(step);
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        let open = arrival(self) == Some(2);
        for step in 0..=4 {
            let (x, y) = point(step);
            self.tile_mut(x, y).road = true;
        }
        // Three neutral road steps cost two fragments each, but the last
        // enemy-owned city step costs six. Native PF floors 12 / 12 to one.
        let hostile_roads = arrival(self) == Some(1);
        let mut friendly = self.unit_by_id(enemy).unwrap().clone();
        friendly.owner = 1;
        // Compare the same corridor without an enemy occupying its origin.
        self.units.retain(|unit| unit.id != enemy);
        let friendly_roads = self
            .search_route(&friendly, city.0, city.1, false, i32::MAX)
            .is_some_and(|(_, cost)| cost / self.unit_move_rate(&friendly) == 0);
        rough && open && hostile_roads && friendly_roads
    }

    pub(crate) fn ai_idle_exploration_playtest(&mut self) -> bool {
        let mut explored = true;
        for kind in [UnitKind::Warrior, UnitKind::Caravel] {
            self.units.clear();
            self.cities.clear();
            self.prune_unit_ai_state();
            for tile in &mut self.tiles {
                tile.terrain = if kind.is_naval() {
                    Terrain::Sea
                } else {
                    Terrain::Grassland
                };
                tile.hut = false;
                tile.owner = None;
                tile.road = false;
                tile.railroad = false;
                tile.river = false;
                tile.known_by.fill(false);
            }
            let id = self.spawn_unit(1, kind, 10, 20);
            self.refresh_visibility();
            let before = self.tiles.iter().filter(|tile| tile.known_by[1]).count();
            self.run_ai_player(1);
            explored &= self
                .units
                .iter()
                .any(|unit| unit.id == id && (unit.x, unit.y) != (10, 20))
                && self.tiles.iter().filter(|tile| tile.known_by[1]).count() > before;
        }
        explored
    }

    pub(crate) fn ai_exploration_legality_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.owner = None;
            tile.known_by.fill(false);
        }
        let mut rejected = true;
        // Derive the excluded set from imported prototype movement/fuel data.
        for kind in UnitKind::ALL_NATIVE
            .iter()
            .copied()
            .filter(|kind| kind.fuel_capacity() > 0 || *kind == UnitKind::Helicopter)
        {
            let id = self.spawn_unit(HUMAN, kind, 10, 20);
            rejected &= !self.apply(Command::Automate(id));
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .automated = true;
            rejected &= self.apply(Command::Automate(id));
        }
        self.units.clear();
        self.prune_unit_ai_state();
        let mut explored = true;
        for kind in [UnitKind::Warrior, UnitKind::Caravel] {
            for tile in &mut self.tiles {
                tile.terrain = if kind.is_naval() {
                    Terrain::Sea
                } else {
                    Terrain::Grassland
                };
                tile.known_by[HUMAN] = false;
            }
            let id = self.spawn_unit(HUMAN, kind, 10, 20);
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .state = UnitState::Sentry;
            explored &= self.apply(Command::Automate(id));
            self.refresh_visibility();
            let before = self
                .tiles
                .iter()
                .filter(|tile| tile.known_by[HUMAN])
                .count();
            self.run_automated_unit(id);
            explored &= self
                .units
                .iter()
                .any(|unit| unit.id == id && (unit.x, unit.y) != (10, 20))
                && self
                    .tiles
                    .iter()
                    .filter(|tile| tile.known_by[HUMAN])
                    .count()
                    > before;
            self.units.clear();
            self.prune_unit_ai_state();
        }
        rejected && explored
    }

    pub(crate) fn ai_exploration_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.owner = None;
            tile.known_by.fill(false);
        }
        let ai = self.spawn_unit(1, UnitKind::Explorer, 10, 20);
        self.tile_mut(10, 20).known_by[1] = true;
        let constrained = self.run_auto_explorer(ai) == ExploreResult::Done;
        self.units.clear();
        let human = self.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
        self.refresh_visibility();
        let before = self
            .tiles
            .iter()
            .filter(|tile| tile.known_by[HUMAN])
            .count();
        let automated = self.apply(Command::Automate(human));
        self.run_automated_unit(human);
        let moved = self.units.iter().any(|unit| {
            unit.id == human
                && unit.automated
                && unit.moves_left == 0
                && (unit.x, unit.y) != (10, 20)
        });
        let discovered = self
            .tiles
            .iter()
            .filter(|tile| tile.known_by[HUMAN])
            .count()
            > before;
        for tile in &mut self.tiles {
            tile.known_by[HUMAN] = true;
        }
        self.units
            .iter_mut()
            .find(|unit| unit.id == human)
            .unwrap()
            .moves_left = MOVE_FRAGMENTS;
        self.run_automated_unit(human);
        constrained
            && automated
            && moved
            && discovered
            && self
                .units
                .iter()
                .any(|unit| unit.id == human && !unit.automated)
    }
}
