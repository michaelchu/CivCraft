//! Automated scenarios for AI gameplay behavior.

use super::super::exploration::ExploreResult;
use super::*;

impl Simulation {
    pub(crate) fn ai_persistent_mission_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.known_by[1] = true;
        }
        let origin = (10, 24);
        let far = offset_native_by_map(origin.0, origin.1, 7, 0);
        let close = offset_native_by_map(origin.0, origin.1, 2, -2);
        self.create_city(1, origin.0, origin.1);
        self.create_city(HUMAN, far.0, far.1);
        let first = self.cities[1].id;
        let agent = self.spawn_unit(1, UnitKind::Diplomat, origin.0, origin.1);
        self.run_ai_player(1);
        let assigned = self.agent_city_targets.get(&agent) == Some(&first);
        self.create_city(HUMAN, close.0, close.1);
        self.update_activities_and_restore();
        self.run_ai_player(1);
        let retained = self.agent_city_targets.get(&agent) == Some(&first);
        self.cities.retain(|city| city.id != first);
        self.update_activities_and_restore();
        self.run_ai_player(1);
        let alternative = self.cities.last().unwrap().id;
        assigned
            && retained
            && (self.agent_city_targets.get(&agent) == Some(&alternative)
                || self.has_embassy(1, HUMAN))
    }

    pub(crate) fn ai_offensive_recruitment_playtest(&mut self) -> bool {
        self.units.clear();
        let home = (10, 24);
        let target = offset_native_by_map(home.0, home.1, 2, 0);
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
        }
        self.create_city(1, home.0, home.1);
        self.create_city(HUMAN, target.0, target.1);
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let id = self.cities[0].id;
        self.players[1].known = vec![Tech::ALPHABET];
        self.players[1].researching = Tech::GUNPOWDER;
        self.players[HUMAN].known = vec![Tech::ALPHABET];
        let unprofitable = self.offensive_agent_production(&self.cities[0]).is_none();
        self.players[HUMAN].known.push(Tech::POTTERY);
        let recruited = self.choose_ai_production(1, id) == Production::Unit(UnitKind::Diplomat);
        let second_home = offset_native_by_map(home.0, home.1, 6, 0);
        let second_target = offset_native_by_map(home.0, home.1, 8, 0);
        self.create_city(1, second_home.0, second_home.1);
        self.cities[0].production = Production::Unit(UnitKind::Diplomat);
        let queued_blocks_duplicate = self.offensive_agent_production(&self.cities[2]).is_none();
        self.cities[0].production = Production::Unit(UnitKind::Warrior);
        self.spawn_unit(1, UnitKind::Diplomat, home.0, home.1);
        let active_blocks_duplicate = self.offensive_agent_production(&self.cities[0]).is_none();
        self.create_city(HUMAN, second_target.0, second_target.1);
        let distinct_target =
            self.offensive_agent_production(&self.cities[2]) == Some(UnitKind::Diplomat);
        unprofitable
            && recruited
            && queued_blocks_duplicate
            && active_blocks_duplicate
            && distinct_target
    }

    pub(crate) fn ai_city_sabotage_playtest(&mut self) -> bool {
        self.units.clear();
        let target = (10, 24);
        let approach = offset_native_by_map(target.0, target.1, -1, 0);
        for (x, y) in [target, approach] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        self.create_city(HUMAN, target.0, target.1);
        self.create_city(1, approach.0, approach.1);
        self.players[HUMAN].known = vec![Tech::ALPHABET];
        self.players[1].known = vec![Tech::ALPHABET];
        let id = self.cities[0].id;
        let envoy = self.spawn_unit(1, UnitKind::Diplomat, approach.0, approach.1);
        if !self.establish_embassy(envoy, id) {
            return false;
        }
        self.cities[0].shields = 10;
        self.spawn_unit(1, UnitKind::Diplomat, approach.0, approach.1);
        let agent = self.units.last().unwrap().clone();
        self.sign_pact(1, HUMAN, Pact::Peace);
        let abstained = !self.ai_city_espionage(&agent) && self.cities[0].shields == 10;
        self.sign_pact(1, HUMAN, Pact::War);
        self.set_random_seed(1);
        self.run_ai_player(1);
        abstained && self.cities[0].shields == 0 && self.cities[0].has_building(Building::PALACE)
    }

    pub(crate) fn ai_incite_revolt_playtest(&mut self) -> bool {
        self.units.clear();
        let target = (10, 24);
        let home = offset_native_by_map(target.0, target.1, -1, 0);
        let capital = offset_native_by_map(target.0, target.1, 6, 0);
        for (x, y) in [target, home, capital] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        self.create_city(HUMAN, capital.0, capital.1);
        self.create_city(HUMAN, target.0, target.1);
        self.create_city(1, home.0, home.1);
        self.players[HUMAN].known = vec![Tech::ALPHABET];
        self.players[1].known = vec![Tech::ALPHABET];
        let agent = self.spawn_unit(1, UnitKind::Diplomat, home.0, home.1);
        let target_id = self.cities[1].id;
        if !self.establish_embassy(agent, target_id) {
            return false;
        }
        self.spawn_unit(1, UnitKind::Diplomat, home.0, home.1);
        self.refresh_visibility();
        let diplomat = self.units.last().unwrap().clone();
        let Some(price) = self.incite_price(&diplomat, &self.cities[1]) else {
            return false;
        };
        self.players[1].gold = price - 1;
        let declined = !self.ai_city_espionage(&diplomat) && self.cities[1].owner == HUMAN;
        self.players[1].gold = price;
        self.set_random_seed(1);
        self.run_ai_player(1);
        declined
            && self.cities[1].owner == 1
            && self.cities[0].owner == HUMAN
            && self.players[1].gold == 0
    }

    pub(crate) fn ai_city_mission_playtest(&mut self) -> bool {
        self.units.clear();
        let target = (10, 24);
        let start = offset_native_by_map(target.0, target.1, -2, 0);
        let approach = offset_native_by_map(target.0, target.1, -1, 0);
        for (x, y) in [target, start, approach] {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        self.create_city(HUMAN, target.0, target.1);
        self.create_city(1, start.0, start.1);
        self.players[HUMAN].known = vec![Tech::POTTERY];
        self.players[1].known = vec![Tech::ALPHABET];
        self.spawn_unit(1, UnitKind::Diplomat, start.0, start.1);
        self.refresh_visibility();
        self.run_ai_player(1);
        let embassy = self.has_embassy(1, HUMAN) && !self.has_embassy(HUMAN, 1);
        self.spawn_unit(1, UnitKind::Diplomat, start.0, start.1);
        self.set_random_seed(1);
        self.run_ai_player(1);
        embassy && self.has_tech(1, Tech::POTTERY) && self.cities[0].technology_thefts == 1
    }

    pub(crate) fn ai_counterintelligence_playtest(&mut self) -> bool {
        self.units.clear();
        let city = (10, 24);
        let friendly = offset_native_by_map(city.0, city.1, -1, 0);
        let enemy = offset_native_by_map(city.0, city.1, 1, 0);
        for (x, y) in [city, friendly, enemy] {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        self.create_city(1, city.0, city.1);
        self.players[1].known.retain(|&tech| tech != Tech::ALPHABET);
        self.players[1].research_goal = Tech::POTTERY;
        self.spawn_unit(1, UnitKind::Warrior, city.0, city.1);
        self.spawn_unit(HUMAN, UnitKind::Diplomat, enemy.0, enemy.1);
        self.refresh_visibility();
        self.ai_tech_wants.insert(1, self.advisor_tech_wants(1));
        let researched = self.counterintelligence_tech_want(1) == 3000
            && self.next_ai_research_target(1) == Tech::ALPHABET;
        self.players[1]
            .known
            .extend([Tech::ALPHABET, Tech::ESPIONAGE]);
        self.prepare_ai_city_defenses(1);
        let recruited = self.cities[0].production == Production::Unit(UnitKind::Spy);

        let guard = self.spawn_unit(1, UnitKind::Diplomat, friendly.0, friendly.1);
        let protects = |game: &Self| {
            game.units
                .iter()
                .any(|unit| unit.id == guard && (unit.x, unit.y) == city)
        };
        self.run_ai_player(1);
        let arrived = protects(self);
        self.update_activities_and_restore();
        self.run_ai_player(1);
        researched
            && recruited
            && arrived
            && protects(self)
            && self
                .counterintelligence_production(&self.cities[0])
                .is_none()
    }

    pub(crate) fn ai_bribery_playtest(&mut self) -> bool {
        self.units.clear();
        let start = (10, 24);
        let adjacent = offset_native_by_map(start.0, start.1, 1, 0);
        for (x, y) in [start, adjacent] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.tile_mut(x, y).hut = false;
        }
        self.create_city(1, start.0, start.1);
        let home = self.cities[0].id;
        let actor = self.spawn_unit(1, UnitKind::Diplomat, start.0, start.1);
        self.units.last_mut().unwrap().home_city = Some(home);
        let diplomat = self.units.last().unwrap().clone();
        let target = self.spawn_unit(HUMAN, UnitKind::Worker, adjacent.0, adjacent.1);
        self.refresh_visibility();
        self.ai_purchase_reserve.insert(1, 100);
        let Some(price) = self.bribe_price(&diplomat, self.units.last().unwrap()) else {
            return false;
        };
        self.players[1].gold = price + 99;
        let declined = !self.ai_bribe_nearby(&diplomat) && self.players[1].gold == price + 99;

        let victim = self.units.last_mut().unwrap();
        victim.kind = UnitKind::Horseman;
        victim.hp = UnitKind::Horseman.stats().hp;
        let Some(price) = self.bribe_price(&diplomat, self.units.last().unwrap()) else {
            return false;
        };
        self.players[1].gold = price;
        self.ai_bribe_nearby(&diplomat);
        declined
            && self.players[1].gold == 0
            && self.units.iter().all(|unit| unit.id != target)
            && self.units.iter().any(|unit| {
                unit.owner == 1 && unit.kind == UnitKind::Horseman && unit.home_city == Some(home)
            })
            && self
                .units
                .iter()
                .any(|unit| unit.id == actor && (unit.x, unit.y) == adjacent)
    }

    pub(crate) fn ai_reserve_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_war_plans.clear();
        self.ai_purchase_reserve.clear();
        self.ai_government_cache.clear();
        self.players[1].government = Government::Despotism;
        self.players[1].known = vec![Tech::EXPLOSIVES];
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
        for _ in 0..5 {
            let id = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
            let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
            unit.home_city = Some(city);
            unit.home_order = id;
        }
        self.spawn_unit(1, UnitKind::Worker, home.0, home.1);
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

    pub(crate) fn ai_civilian_upgrade_playtest(&mut self) -> bool {
        self.units.clear();
        let home = (10, 24);
        let field = offset_native_by_map(home.0, home.1, 1, 0);
        self.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
        self.tile_mut(field.0, field.1).terrain = Terrain::Grassland;
        self.create_city(1, home.0, home.1);
        self.players[1]
            .known
            .extend([Tech::EXPLOSIVES, Tech::GUNPOWDER]);
        let worker = self.spawn_unit(1, UnitKind::Worker, home.0, home.1);
        let field_worker = self.spawn_unit(1, UnitKind::Worker, field.0, field.1);
        let guard = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let rival = self.spawn_unit(HUMAN, UnitKind::Explorer, field.0, field.1);
        let kind = |game: &Self, id| game.unit_by_id(id).unwrap().kind;
        let Some((_, price)) = self.paid_upgrade(self.unit_by_id(worker).unwrap()) else {
            return false;
        };
        self.refresh_visibility();
        self.ai_war_plans.insert(
            (1, HUMAN),
            super::diplomacy::WarPlan {
                countdown: 0,
                reason: super::diplomacy::WarReason::Hatred,
            },
        );
        self.players[1].gold = price + 3;
        self.spend_ai_defense_gold(1);
        let wartime = kind(self, worker) == UnitKind::Worker && self.players[1].gold == price + 3;

        self.sign_pact(1, HUMAN, Pact::Peace);
        self.ai_war_plans.remove(&(1, HUMAN));
        self.units.retain(|unit| unit.id != rival);
        self.players[1].gold = price + 2;
        self.spend_ai_defense_gold(1);
        let reserve = kind(self, worker) == UnitKind::Worker && self.players[1].gold == price + 2;

        let replenished = self.ai_gold_reserve(1) + 1;
        self.players[1].gold = price + replenished;
        self.spend_ai_defense_gold(1);
        wartime
            && reserve
            && self.players[1].gold == replenished
            && kind(self, worker) == UnitKind::Engineer
            && kind(self, field_worker) == UnitKind::Worker
            && kind(self, guard) == UnitKind::Warrior
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
        let guard = self
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && (unit.x, unit.y) == (target.x, target.y))
            .unwrap()
            .clone();
        let actor = self.unit_by_id(infantry).unwrap();
        let distant =
            self.map_distance(actor.x, actor.y, target.x, target.y) > actor.kind.stats().moves;
        self.run_ai_player(1);
        distant
            && self.ai_ground_targets.get(&infantry) == Some(&city)
            && self
                .units
                .iter()
                .any(|unit| unit.id == bomber && unit.moved)
            && self
                .units
                .iter()
                .find(|unit| unit.id == guard.id)
                .is_none_or(|unit| unit.hp < guard.hp)
    }

    pub(crate) fn ai_aircraft_advisor_playtest(&mut self) -> bool {
        let (fighter, victim) = self.ai_aircraft_advisor_playtest_setup();
        let plane = self.unit_by_id(fighter).unwrap();
        let target = self.unit_by_id(victim).unwrap();
        let positive_tactical_value = self.aircraft_target_value(plane, target.x, target.y) > 0.0;
        let production_gated = self.aircraft_production_choice(&self.cities[0]).is_none();
        self.run_ai_player(1);
        positive_tactical_value
            && production_gated
            && !self.units.iter().any(|unit| unit.id == victim)
            && self
                .units
                .iter()
                .any(|unit| unit.id == fighter && unit.owner == 1)
    }

    pub(crate) fn ai_nuclear_advisor_playtest(&mut self) -> bool {
        let (missile, victim) = self.ai_nuclear_advisor_playtest_setup();
        let initial_moves = self
            .units
            .iter()
            .find(|unit| unit.id == missile)
            .unwrap()
            .moves_left;
        self.run_ai_player(1);
        self.units
            .iter()
            .any(|unit| unit.id == missile && unit.moves_left == initial_moves)
            && self.units.iter().any(|unit| unit.id == victim)
    }
    pub(crate) fn ai_merchant_road_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.hut = false;
        }
        let origin = (10, 24);
        let point = |step| offset_native_by_map(origin.0, origin.1, step, 0);
        let (x, y) = point(3);
        self.tile_mut(x, y).river = true;
        self.tile_mut(origin.0, origin.1).river = true;
        self.create_city(HUMAN, x, y);
        self.create_city(1, origin.0, origin.1);
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        let city = self.cities[0].id;
        let home = self.cities[1].id;
        let id = self.spawn_unit(1, UnitKind::Caravan, origin.0, origin.1);
        self.units.last_mut().unwrap().home_city = Some(home);
        let caravan = self.units.last().unwrap().clone();
        self.run_ai_merchant(&caravan);
        let slow = !self.merchant_playtest_route_exists(home, city);
        let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) else {
            return false;
        };
        *unit = caravan.clone();
        for step in 0..=2 {
            let (x, y) = point(step);
            self.tile_mut(x, y).road = true;
        }
        self.run_ai_merchant(&caravan);
        slow && self.merchant_playtest_route_exists(home, city)
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
                    Terrain::Ocean
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
        for kind in [
            UnitKind::Fighter,
            UnitKind::Bomber,
            UnitKind::StealthFighter,
            UnitKind::StealthBomber,
            UnitKind::CruiseMissile,
            UnitKind::Nuclear,
            UnitKind::Helicopter,
        ] {
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
                    Terrain::Ocean
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
