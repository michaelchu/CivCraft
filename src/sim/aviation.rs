use super::*;

impl Simulation {
    fn airlift_origin(&self, unit: &Unit) -> Option<&City> {
        if unit.kind.is_naval() || unit.moves_left <= 0 || self.cargo_count(unit.id) != 0 {
            return None;
        }
        self.cities.iter().find(|city| {
            (city.x, city.y) == (unit.x, unit.y)
                && city.owner == unit.owner
                && city.has_building(Building::AIRPORT)
                && !city.airlift_used
        })
    }

    pub(crate) fn can_airlift(&self, unit: &Unit, destination: &City) -> bool {
        destination.owner == unit.owner
            && self
                .airlift_origin(unit)
                .is_some_and(|source| source.id != destination.id)
    }

    pub(super) fn airlift(&mut self, id: u32, destination: u32) -> bool {
        let Some(index) = self.units.iter().position(|unit| unit.id == id) else {
            return false;
        };
        let unit = &self.units[index];
        let Some(source) = self.airlift_origin(unit).map(|city| city.id) else {
            return false;
        };
        let Some(city) = self
            .cities
            .iter()
            .find(|city| city.id == destination && city.owner == unit.owner && city.id != source)
        else {
            return false;
        };
        let (x, y, name) = (city.x, city.y, city.name.clone());
        let unit = &mut self.units[index];
        let owner = unit.owner;
        (unit.x, unit.y) = (x, y);
        unit.transport = None;
        unit.moves_left = 0;
        unit.moved = true;
        unit.state = UnitState::Idle;
        unit.clear_work();
        self.list_order.unit(unit);
        self.record_transport_change(id, None);
        for city in &mut self.cities {
            if city.id == source || city.id == destination {
                city.airlift_used = true;
            }
        }
        self.refresh_diplomatic_contact_at(owner, x, y);
        self.refresh_visibility();
        if owner == HUMAN {
            self.add_event(format!("A unit was airlifted to {name}."));
        }
        true
    }

    pub(crate) fn can_begin_paradrop(&self, unit: &Unit) -> bool {
        unit.kind == UnitKind::Paratroopers
            && unit.moves_left >= MOVE_FRAGMENTS
            && !unit.moved
            && unit.work_left == 0
            && unit.transport.is_none()
            && self.refuel_site(unit.owner, unit.x, unit.y)
    }

    fn paradrop_tile_legal(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !(0..MAP_H).contains(&y) || !(1..=10).contains(&self.map_distance(unit.x, unit.y, x, y))
        {
            return false;
        }
        let target = self.tile(x, y);
        target.known_by[unit.owner]
            && !target.terrain.is_water()
            && !target.owner.is_some_and(|owner| {
                owner != unit.owner && self.pact(unit.owner, owner) == Pact::Peace
            })
            && !self.cities.iter().any(|city| {
                (city.x, city.y) == (x, y)
                    && !self.allied(unit.owner, city.owner)
                    && !self.at_war(unit.owner, city.owner)
            })
    }

    pub(super) fn run_ai_paratrooper(&mut self, unit: &Unit) -> bool {
        if !self.can_begin_paradrop(unit) || unit.hp * 2 < unit.kind.stats().hp {
            return false;
        }
        let owner = unit.owner;
        if self
            .cities
            .iter()
            .any(|city| city.owner == owner && (city.x, city.y) == (unit.x, unit.y))
            && !self.units.iter().any(|guard| {
                guard.id != unit.id
                    && guard.owner == owner
                    && guard.kind.is_military()
                    && guard.transport.is_none()
                    && (guard.x, guard.y) == (unit.x, unit.y)
            })
        {
            return false;
        }

        let empty = |x, y| self.units.iter().all(|other| (other.x, other.y) != (x, y));
        let cities = self
            .cities
            .iter()
            .filter(|city| self.paradrop_tile_legal(unit, city.x, city.y) && empty(city.x, city.y));
        let threatened = cities
            .clone()
            .filter(|city| city.owner == owner)
            .filter_map(|city| {
                let danger = self
                    .city_threats(city)
                    .map(|(_, danger)| danger)
                    .sum::<i32>();
                (danger > 0).then_some((city.size * danger, city))
            })
            .max_by_key(|&(score, city)| (score, std::cmp::Reverse(city.id)));
        if let Some((_, city)) = threatened {
            return self.paradrop(unit.id, city.x, city.y);
        }
        let labels = self.continent_labels();
        let origin = labels[Self::idx(unit.x, unit.y)];
        let conquest = cities
            .filter(|city| self.at_war(owner, city.owner))
            .max_by_key(|city| {
                let overseas = labels[Self::idx(city.x, city.y)] != origin;
                (city.size + i32::from(overseas), std::cmp::Reverse(city.id))
            });
        if let Some(city) = conquest {
            return self.paradrop(unit.id, city.x, city.y);
        }

        let mut best = None;
        let mut best_value = 0.0;
        for (index, _) in self.tiles.iter().enumerate() {
            let (x, y) = Self::xy(index);
            if !empty(x, y)
                || !self.paradrop_tile_legal(unit, x, y)
                || self
                    .cities
                    .iter()
                    .any(|city| (city.x, city.y) == (x, y) && !self.allied(owner, city.owner))
            {
                continue;
            }
            let mut staged = unit.clone();
            (staged.x, staged.y) = (x, y);
            for (tx, ty) in super::terrain_work::neighbors(x, y) {
                if !self.tile(tx, ty).known_by[owner]
                    || self.tile(tx, ty).terrain.is_water()
                    || self.treaty_blocks_move(&staged, tx, ty)
                {
                    continue;
                }
                let Some(defender) = self
                    .best_defender_id(&staged, tx, ty)
                    .and_then(|id| self.units.iter().find(|enemy| enemy.id == id))
                else {
                    continue;
                };
                let (attack_fp, defense_fp) = self.combat_firepower(&staged, defender);
                let chance = combat_win_chance(
                    Self::attack_power(&staged),
                    staged.hp,
                    attack_fp,
                    self.defense_power_against(defender, tx, ty, staged.kind),
                    defender.hp,
                    defense_fp,
                );
                let threatened_hp: i32 = self
                    .combat_stack_losses(defender, &staged)
                    .filter(|unit| unit.transport.is_none())
                    .map(|unit| unit.hp)
                    .sum();
                let value = chance * f64::from(threatened_hp * 100) - f64::from(staged.hp * 100)
                    + f64::from(self.tile(x, y).terrain.defense_bonus() / 10);
                if value > best_value {
                    best_value = value;
                    best = Some((x, y, wrapped_map_vector(x, y, tx, ty)));
                }
            }
        }
        if let Some((x, y, (dx, dy))) = best
            && self.paradrop(unit.id, x, y)
        {
            self.try_move_unit(unit.id, dx, dy);
            return true;
        }
        false
    }

    pub(super) fn paradrop(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == id).cloned() else {
            return false;
        };
        if !self.can_begin_paradrop(&unit) {
            return false;
        }
        let x = x.rem_euclid(MAP_W);
        if !self.paradrop_tile_legal(&unit, x, y) {
            if unit.owner == HUMAN {
                self.add_event("Paratroopers need a known land tile and permission to enter.");
            }
            return false;
        }
        let occupiers: Vec<_> = self
            .units
            .iter()
            .filter(|enemy| (enemy.x, enemy.y) == (x, y) && !self.allied(unit.owner, enemy.owner))
            .collect();
        if !occupiers.is_empty() {
            if unit.owner == HUMAN
                && occupiers
                    .iter()
                    .any(|enemy| self.unit_visible_to_human(enemy))
            {
                self.add_event("Visible enemy units block the landing.");
                return false;
            }
            self.refresh_diplomatic_contact_at(unit.owner, x, y);
            if unit.owner == HUMAN {
                if let Some(paratroopers) = self.units.iter_mut().find(|unit| unit.id == id) {
                    (paratroopers.x, paratroopers.y) = (x, y);
                    self.list_order.unit(paratroopers);
                }
                self.refresh_visibility();
                self.add_event("Paratroopers were lost landing on enemy units.");
            }
            self.remove_units_with_cargo(vec![id]);
        } else {
            // The drop itself costs no movement, but enter_tile records the action.
            self.enter_tile(id, x, y, 0);
        }
        self.refresh_visibility();
        self.check_game_over();
        true
    }

    pub(super) fn refuel_site(&self, owner: usize, x: i32, y: i32) -> bool {
        self.cities
            .iter()
            .any(|city| (city.x, city.y) == (x, y) && self.allied(owner, city.owner))
            || (matches!(self.tile(x, y).base, Some(Base::Airstrip | Base::Airbase))
                && self.tile(x, y).known_by[owner])
    }

    fn aboard_refueling_transport(&self, unit: &Unit) -> bool {
        unit.transport.is_some_and(|id| {
            self.units.iter().any(|ship| {
                ship.id == id
                    && ship.kind.can_carry(unit.kind)
                    && self.allied(unit.owner, ship.owner)
            })
        })
    }

    pub(super) fn run_source_air_ferry(&mut self, carrier: &Unit) -> (bool, Vec<u32>) {
        let mut done = Vec::new();
        if carrier.moves_left <= 0 {
            return (true, done);
        }
        if carrier.hp < carrier.kind.stats().hp
            && self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (carrier.x, carrier.y))
        {
            return (true, done);
        }
        self.refresh_ferry_boss(carrier.id);
        loop {
            let Some(boss) = self
                .ferry_boss(carrier.id)
                .and_then(|id| self.units.iter().find(|unit| unit.id == id))
                .cloned()
            else {
                return (false, done);
            };
            if self.map_distance(carrier.x, carrier.y, boss.x, boss.y) > 1 {
                // Native temporarily abandons a distant boss and falls back
                // to pickup/exploration if no cargo replaces it. The restored
                // reservation must not activate that remote passenger.
                return (false, done);
            }
            // Land passengers retain their ordinary founder/worker/military
            // dispatch; fueled cargo uses the same manager as a land airbase.
            if boss.kind.fuel_capacity() == 0 {
                return (false, done);
            }
            if boss.moves_left > 0 && !self.ai_guard_targets.contains_key(&boss.id) {
                self.run_ai_strike_aircraft(&boss);
            }
            let Some(ship) = self
                .units
                .iter()
                .find(|unit| unit.id == carrier.id)
                .cloned()
            else {
                return (true, done);
            };
            if self
                .units
                .iter()
                .any(|unit| unit.id == boss.id && (unit.x, unit.y) == (ship.x, ship.y))
            {
                // daiferry.c: the boss remains on the boat, so hold station
                // and mark it done for the owner's remaining unit loop.
                done.push(boss.id);
                return (true, done);
            }
            if ship.moves_left <= 0 {
                return (true, done);
            }
            if self.cargo_count(ship.id) == 0 {
                return (false, done);
            }
            self.release_ai_ferry(boss.id);
            self.refresh_ferry_boss(ship.id);
        }
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
            Tech::NUCLEAR_FISSION,
            Tech::ROCKETRY,
        ]);
        self.completed_wonders.push(Building::MANHATTAN_PROJECT);
        let target = offset_native_by_map(x, y, 2, 0);
        self.tile_mut(target.0, target.1).terrain = Terrain::Ocean;
        let fighter = self.spawn_unit(1, UnitKind::Fighter, x, y);
        let victim = self.spawn_unit(HUMAN, UnitKind::Battleship, target.0, target.1);
        self.units.last_mut().unwrap().hp = 1;
        self.set_random_seed(4);
        self.refresh_visibility();
        (fighter, victim)
    }

    pub(crate) fn ai_nuclear_advisor_playtest_setup(&mut self) -> (u32, u32) {
        let (fighter, target) = self.ai_aircraft_advisor_playtest_setup();
        self.units.retain(|u| u.id != fighter);
        let victim = self.units.iter().find(|u| u.id == target).unwrap().clone();
        for _ in 0..4 {
            self.spawn_unit(HUMAN, UnitKind::Battleship, victim.x, victim.y);
        }
        let city = self.cities[0].clone();
        let missile = self.spawn_unit(1, UnitKind::Nuclear, city.x, city.y);
        (missile, target)
    }

    // daiair.c: find_something_to_bomb, dai_evaluate_tile_for_air_attack.
    // All legally attackable enemy stacks compete, not just aircraft for fighters.
    pub(super) fn aircraft_target_value(&self, plane: &Unit, x: i32, y: i32) -> f64 {
        if !self.players[plane.owner].ai && !self.tile(x, y).known_by[plane.owner]
            || self
                .units
                .iter()
                .any(|u| (u.x, u.y) == (x, y) && !self.at_war(plane.owner, u.owner))
            || self
                .cities
                .iter()
                .any(|c| (c.x, c.y) == (x, y) && !self.at_war(plane.owner, c.owner))
        {
            return 0.0;
        }
        if plane.id != 0
            && self
                .cities
                .iter()
                .find(|c| (c.x, c.y) == (x, y))
                .is_some_and(|city| self.invasion_support(plane.owner, city, plane.id).1 == 0)
        {
            return 0.0;
        }
        let Some(defender) = self
            .best_defender_id(plane, x, y)
            .and_then(|id| self.units.iter().find(|u| u.id == id))
        else {
            return 0.0;
        };
        let stats = plane.kind.stats();
        let (firepower, retaliation) = self.combat_firepower(plane, defender);
        let win = (100.0
            * combat_win_chance(
                Self::attack_power(plane),
                plane.hp,
                firepower,
                self.defense_power_against(defender, x, y, plane.kind),
                defender.hp,
                retaliation,
            ))
        .floor()
            / 100.0;
        let mut benefit: i32 = self
            .combat_stack_losses(defender, plane)
            .map(|u| u.kind.stats().cost)
            .sum();
        if benefit == 0 {
            return 0.0;
        }
        if plane.kind.is_missile() {
            benefit -= stats.cost;
        }
        let risk = f64::from(stats.cost * stats.hp) / f64::from(plane.hp.max(1));
        let profit = 17.0 * (f64::from(benefit) * win - risk * (1.0 - win)) - 17.0 + 36.0;
        let outputs: Vec<_> = self
            .cities
            .iter()
            .filter(|c| c.owner == plane.owner)
            .map(|c| self.city_forecast(c).shield_surplus())
            .collect();
        let average = outputs.iter().sum::<i32>() / outputs.len().max(1) as i32;
        let home = self
            .cities
            .iter()
            .find(|c| Some(c.id) == plane.home_city)
            .map(|c| self.city_forecast(c).shield_surplus())
            .unwrap_or(1);
        let balanced = 2 * stats.cost * stats.attack / (stats.attack + stats.defense).max(1);
        profit.max(0.0) * (23.0_f64 / 24.0).powi(1 + balanced / home.max(average).max(1))
    }

    pub(super) fn aircraft_production_choice(&self, city: &City) -> Option<(UnitKind, f64)> {
        let mut best = None;
        let mut best_value = 0.0;
        for kind in UnitKind::ALL_NATIVE
            .into_iter()
            .filter(|kind| kind.fuel_capacity() > 0)
        {
            // daiair.c rejects unit_type_is_losing_hp even for fuelled Air.
            // This ruleset supplies no global Unit_Recover effect; Medicine
            // is Health_Pct, so the Air/Helicopter class loss remains active.
            if !self.unit_buildable(city, kind) || kind.hp_loss_percent() > 0 {
                continue;
            }
            let mut plane = self.make_unit(0, city.owner, kind, city.x, city.y);
            plane.home_city = Some(city.id);
            plane.veteran = self.trained_rank(city, kind);
            if let Some((_, _, value)) = self.aircraft_best_target(&plane)
                && value > best_value
            {
                best_value = value;
                best = Some((kind, value));
            }
        }
        best
    }

    fn aircraft_attack_path(&self, plane: &Unit) -> Option<(Vec<(i32, i32)>, (i32, i32), f64)> {
        let mut map = super::fuel_paths::FuelPaths::new(self, plane, true)?;
        let mut best = None;
        let mut best_value = 0.0;
        for (x, y, _) in map.reachable_within(self, plane.moves_left, true, false) {
            if !self
                .units
                .iter()
                .any(|unit| (unit.x, unit.y) == (x, y) && self.at_war(plane.owner, unit.owner))
            {
                continue;
            }
            let value = self.aircraft_target_value(plane, x, y);
            if value > best_value {
                best_value = value;
                best = Some((x, y));
            }
        }
        let target = best?;
        Some((map.path(self, target.0, target.1)?, target, best_value))
    }

    fn aircraft_best_target(&self, plane: &Unit) -> Option<(i32, i32, f64)> {
        if plane.kind.fuel_capacity() > 0 {
            return self
                .aircraft_attack_path(plane)
                .map(|(_, (x, y), value)| (x, y, value));
        }
        self.units
            .iter()
            .filter(|u| self.at_war(plane.owner, u.owner) && u.transport.is_none())
            .filter_map(|u| {
                self.route_cost_within(plane, u.x, u.y, plane.moves_left)?;
                let value = self.aircraft_target_value(plane, u.x, u.y);
                (value > 0.0).then_some((u.x, u.y, value))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
    }

    #[cfg(test)]
    pub(super) fn run_ai_nuclear_aircraft(&mut self, plane: &Unit) {
        // dai_manage_unit dispatches all fueled non-escort actors through
        // dai_manage_airunit. Hunter-controlled launches are separate.
        self.run_ai_strike_aircraft(plane);
    }

    fn source_unit_can_strike(&self, attacker: &Unit, defender: &Unit) -> bool {
        if (attacker.x, attacker.y) == (defender.x, defender.y) {
            return false;
        }
        let cost = if let Some(mut map) = super::fuel_paths::FuelPaths::new(self, attacker, true) {
            map.movement_cost(self, defender.x, defender.y)
        } else {
            super::normal_paths::NormalPaths::enemy_map(self, attacker)
                .movement_cost(self, defender.x, defender.y)
        };
        cost.is_some_and(|cost| cost <= attacker.moves_left)
    }

    pub(super) fn source_hunter_launch(&mut self, hunter_id: u32, primary: u32) {
        let Some(hunter) = self.units.iter().find(|unit| unit.id == hunter_id).cloned() else {
            return;
        };
        if !self.units.iter().any(|unit| unit.id == primary) {
            return;
        }
        let missiles: Vec<_> = self
            .cargo_units_ordered(hunter_id)
            .filter(|unit| unit.owner == hunter.owner && unit.kind.is_missile())
            .map(|unit| unit.id)
            .collect();
        for id in missiles {
            let Some(missile) = self.units.iter().find(|unit| unit.id == id).cloned() else {
                continue;
            };
            // Literal daihunter.c: the map describes the HUNTER and its raw
            // move cost is compared with missile MP divided by SINGLE_MOVE.
            let mut map = super::normal_paths::NormalPaths::enemy_map(self, &hunter);
            let mut target = None;
            for (x, y) in map.reachable_within(self, missile.moves_left / MOVE_FRAGMENTS) {
                if self.cities.iter().any(|city| (city.x, city.y) == (x, y))
                    || self.best_defender_id(&hunter, x, y).is_none()
                {
                    continue;
                }
                for victim in self.tile_units_ordered(x, y) {
                    if !self.at_war(hunter.owner, victim.owner) {
                        continue;
                    }
                    if victim.id == primary
                        || victim.kind.stats().attack > hunter.kind.stats().defense
                            && self.source_unit_can_strike(victim, &hunter)
                    {
                        target = Some((x, y));
                        break;
                    }
                }
                if target.is_some() {
                    break;
                }
            }
            if let Some(target) = target {
                // Native deboard passes the hunter's id as actor, targeting
                // itself. It is illegal for the supported naval hunters and
                // does not unload the missile; normal GOTO performs departure.
                self.unit_goto_tiles.insert(id, target);
                let result = self.ai_goto_unit(id, target, super::normal_paths::GotoTask::None);
                if result.alive {
                    let live = self
                        .units
                        .iter()
                        .find(|unit| unit.id == id)
                        .unwrap()
                        .clone();
                    if let Some(primary) = self.units.iter().find(|unit| unit.id == primary)
                        && self.map_distance(live.x, live.y, primary.x, primary.y) == 1
                    {
                        let step = wrapped_map_vector(live.x, live.y, primary.x, primary.y);
                        self.try_move_unit(id, step.0, step.1);
                    }
                }
                if !self.units.iter().any(|unit| unit.id == primary) {
                    return;
                }
            }
        }
    }

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
                tile.terrain = Terrain::Ocean;
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

    fn aircraft_repair_gain(&self, plane: &Unit, x: i32, y: i32) -> i32 {
        let percent = if let Some(city) = self.cities.iter().find(|c| (c.x, c.y) == (x, y)) {
            if city.has_building(Building::AIRPORT) {
                100
            } else {
                33
            }
        } else if self.tile(x, y).base == Some(Base::Airbase) {
            34
        } else if self.refuel_site(plane.owner, x, y) {
            10
        } else {
            0
        };
        percent_of(plane.kind.stats().hp, percent)
    }

    fn aircraft_regen_turns(&self, plane: &Unit, x: i32, y: i32) -> i32 {
        let lost = plane.kind.stats().hp - plane.hp;
        if lost <= 0 {
            return 0;
        }
        let gain = self.aircraft_repair_gain(plane, x, y);
        if gain <= 0 { i32::MAX } else { 1 + lost / gain }
    }

    fn aircraft_city_needs_defender(&self, plane: &Unit, x: i32, y: i32, current: bool) -> bool {
        let Some(city) = self.cities.iter().find(|c| (c.x, c.y) == (x, y)) else {
            return false;
        };
        // daimilitary.c:grave_danger counts occupiers arriving within one
        // turn (including transports carrying occupation-capable units).
        let danger = self.city_grave_danger(city);
        let defenders = self
            .units
            .iter()
            .filter(|u| (u.x, u.y) == (x, y) && (!current || u.id != plane.id))
            .count();
        danger > defenders * 2
    }

    fn aircraft_refuel_route(
        &self,
        plane: &Unit,
        bases: &[(i32, i32)],
    ) -> Option<(Vec<(i32, i32)>, (i32, i32))> {
        let mut map = super::fuel_paths::FuelPaths::new(self, plane, true)?;
        let mut best = None;
        let mut best_regen = i32::MAX;
        for (x, y, _) in map.reachable_within(self, plane.moves_left, false, true) {
            if !bases.contains(&(x, y)) {
                continue;
            }
            let turns = self.aircraft_regen_turns(plane, x, y);
            if plane.hp >= plane.kind.stats().hp || turns <= 0 {
                best = Some((x, y));
                break;
            }
            if turns < best_regen {
                best_regen = turns;
                best = Some((x, y));
            }
        }
        let destination = best?;
        Some((map.path(self, destination.0, destination.1)?, destination))
    }

    fn strategic_airbase(
        &self,
        plane: &Unit,
        bases: &[(i32, i32)],
    ) -> Option<(Vec<(i32, i32)>, (i32, i32))> {
        let lost = plane.kind.stats().hp - plane.hp;
        let gain = self.aircraft_repair_gain(plane, plane.x, plane.y);
        let mut minimum_regen = i32::MAX;
        if lost > 0 && gain > 0 {
            let remaining = lost - if plane.moved { 0 } else { gain };
            if remaining <= 0 {
                return None;
            }
            minimum_regen = 1 + remaining / gain;
        }
        let mut defend = self.aircraft_city_needs_defender(plane, plane.x, plane.y, true);
        if defend && (lost <= 0 || minimum_regen <= 1) {
            return None;
        }
        let mut virtual_plane = plane.clone();
        virtual_plane.id = 0;
        virtual_plane.moves_left = self.unit_move_rate(&plane);
        let mut worth = if defend {
            0.0
        } else {
            self.aircraft_best_target(&virtual_plane)
                .map_or(0.0, |t| t.2)
        };
        let mut map = super::fuel_paths::FuelPaths::new(self, plane, true)?;
        let routes: Vec<_> = map
            .reachable_within(self, plane.moves_left, true, false)
            .into_iter()
            .filter(|&(x, y, _)| bases.contains(&(x, y)))
            .filter_map(|(x, y, cost)| Some((cost, x, y, map.path(self, x, y)?)))
            .collect();
        let mut best = None;
        for (_, x, y, path) in routes {
            let mut faster = false;
            if lost > 0 {
                let turns = self.aircraft_regen_turns(plane, x, y);
                if turns > minimum_regen {
                    continue;
                }
                if turns < minimum_regen {
                    minimum_regen = turns;
                    best = Some((path.clone(), (x, y)));
                    worth = 0.0;
                    faster = true;
                }
            }
            if self.aircraft_city_needs_defender(plane, x, y, false) {
                if lost <= 0 {
                    return Some((path, (x, y)));
                }
                if !defend {
                    best = Some((path, (x, y)));
                    defend = true;
                }
                continue;
            } else if defend {
                if faster {
                    defend = false;
                } else {
                    continue;
                }
            }
            (virtual_plane.x, virtual_plane.y) = (x, y);
            let value = self
                .aircraft_best_target(&virtual_plane)
                .map_or(0.0, |t| t.2);
            if value > worth {
                worth = value;
                best = Some((path, (x, y)));
            }
        }
        best
    }

    pub(super) fn run_ai_strike_aircraft(&mut self, aircraft: &Unit) {
        let position = (aircraft.x, aircraft.y);
        let mut bases = Vec::new();
        for (index, _) in self.tiles.iter().enumerate() {
            let (x, y) = Self::xy(index);
            if self.refuel_site(aircraft.owner, x, y)
                && !self.units.iter().any(|unit| {
                    (unit.x, unit.y) == (x, y) && !self.allied(aircraft.owner, unit.owner)
                })
            {
                bases.push((x, y));
            }
        }
        let decks: Vec<_> = self
            .units
            .iter()
            .filter(|ship| {
                ship.kind.can_carry(aircraft.kind)
                    && self.allied(aircraft.owner, ship.owner)
                    && (aircraft.transport == Some(ship.id)
                        || self.cargo_count(ship.id) < ship.kind.transport_capacity())
            })
            .map(|ship| ((ship.x, ship.y), ship.id))
            .collect();
        for &(tile, _) in &decks {
            if !bases.contains(&tile) {
                bases.push(tile);
            }
        }
        let on_base = self.refuel_site(aircraft.owner, position.0, position.1)
            || self.aboard_refueling_transport(aircraft);
        if on_base && aircraft.fuel < aircraft.kind.fuel_capacity() {
            return;
        }
        let before = aircraft.moves_left;
        let route = if on_base {
            self.aircraft_attack_path(aircraft)
                .map(|(path, target, _)| (path, target))
                .or_else(|| self.strategic_airbase(aircraft, &bases))
        } else {
            let old_goal = self
                .unit_goto_tiles
                .get(&aircraft.id)
                .copied()
                .filter(|goal| *goal != position && bases.contains(goal));
            old_goal
                .and_then(|goal| {
                    let mut map = super::fuel_paths::FuelPaths::new(self, aircraft, true)?;
                    Some((map.path(self, goal.0, goal.1)?, goal))
                })
                .or_else(|| self.aircraft_refuel_route(aircraft, &bases))
        };
        if let Some((path, _destination)) = route {
            self.follow_source_ai_path(aircraft.id, path);
            if let Some(live) = self
                .units
                .iter()
                .find(|unit| unit.id == aircraft.id)
                .cloned()
                && live.moves_left > 0
                && live.moves_left != before
            {
                self.run_ai_strike_aircraft(&live);
            }
        }
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
        let target = self.units.iter().find(|unit| unit.id == victim).unwrap();
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

    pub(super) fn try_bombard(&mut self, attacker: &Unit, x: i32, y: i32) -> MoveResult {
        if self
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (x, y) && !self.at_war(attacker.owner, city.owner))
            || self
                .units
                .iter()
                .any(|unit| (unit.x, unit.y) == (x, y) && !self.at_war(attacker.owner, unit.owner))
        {
            return MoveResult::Blocked;
        }
        let targets: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                (unit.x, unit.y) == (x, y) && !unit.kind.is_aircraft() && unit.transport.is_none()
            })
            .cloned()
            .collect();
        if targets.is_empty() {
            return MoveResult::Blocked;
        }
        let attack = Self::attack_power(attacker);
        let mut damage = 0;
        for target in &targets {
            let defense = self.defense_power_against(target, x, y, attacker.kind);
            let hits = (0..attacker.kind.bombard_rate())
                .filter(|_| self.rand_below(attack + defense) >= defense)
                .count() as i32;
            let (firepower, _) = self.combat_firepower(attacker, target);
            let defender = self
                .units
                .iter_mut()
                .find(|unit| unit.id == target.id)
                .unwrap();
            let hp = (defender.hp - hits * firepower).max(1);
            damage += defender.hp - hp;
            defender.hp = hp;
        }
        let bomber = self
            .units
            .iter_mut()
            .find(|unit| unit.id == attacker.id)
            .unwrap();
        bomber.moves_left = 0;
        bomber.moved = true;
        bomber.state = UnitState::Idle;
        if attacker.owner == HUMAN || targets.iter().any(|unit| unit.owner == HUMAN) {
            self.add_event(format!(
                "A {} dealt {damage} damage to {} units.",
                attacker.kind.name(),
                targets.len()
            ));
        }
        MoveResult::Fought
    }

    pub(super) fn restore_aircraft(&mut self) {
        for owner in 0..self.players.len() {
            let ids: Vec<_> = self
                .player_units_ordered(owner)
                .filter(|unit| unit.kind.is_aircraft())
                .map(|unit| unit.id)
                .collect();
            for id in ids {
                self.restore_aircraft_unit(id);
            }
            self.finish_aircraft_fuel_deaths(owner);
        }
    }

    pub(super) fn finish_aircraft_fuel_deaths(&mut self, owner: usize) {
        let dead: Vec<_> = self
            .player_units_ordered(owner)
            .filter(|unit| unit.kind.fuel_capacity() > 0 && unit.fuel == 0)
            .map(|unit| (unit.id, unit.kind))
            .collect();
        for (id, kind) in dead {
            if !self.units.iter().any(|unit| unit.id == id) {
                continue;
            }
            self.remove_units_with_cargo(vec![id]);
            if owner == HUMAN {
                self.add_event(format!("Your {} crashed.", kind.name()));
            }
        }
    }

    pub(super) fn restore_aircraft_unit(&mut self, id: u32) {
        let unit = self.units.iter().find(|unit| unit.id == id).cloned();
        let Some(mut unit) = unit else { return };
        let max_fuel = unit.kind.fuel_capacity();
        let max_hp = unit.kind.stats().hp;
        let airborne_loss = if unit.kind.is_missile() {
            0
        } else {
            percent_of(max_hp, 10)
        };
        let site = self.refuel_site(unit.owner, unit.x, unit.y);
        if site || self.aboard_refueling_transport(&unit) {
            let gain = self.aircraft_repair_gain(&unit, unit.x, unit.y);
            let bomber = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
            bomber.fuel = max_fuel;
            bomber.moved = false;
            if !unit.moved {
                bomber.hp = (bomber.hp + gain).min(max_hp);
            }
            return;
        }
        // player_restore_units restores hit points (including airborne
        // loss), kills dead actors, then attempts last-fuel rescue.
        unit.hp -= airborne_loss;
        self.units
            .iter_mut()
            .find(|actor| actor.id == id)
            .unwrap()
            .hp = unit.hp;
        self.units
            .iter_mut()
            .find(|actor| actor.id == id)
            .unwrap()
            .moved = false;
        if unit.hp <= 0 {
            self.remove_units_with_cargo(vec![id]);
            if unit.owner == HUMAN {
                self.add_event(format!("Your {} crashed.", unit.kind.name()));
            }
            return;
        }
        if max_fuel > 0 && unit.fuel <= 1 && self.boarding_target(&unit, unit.x, unit.y).is_some() {
            self.board_or_deboard(id);
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .fuel = max_fuel;
            return;
        }
        if max_fuel > 0 && unit.fuel <= 1 && unit.moves_left > 0 {
            if let Some(mut map) =
                super::fuel_paths::FuelPaths::new(self, &unit, self.players[unit.owner].ai)
            {
                let destination = map
                    .reachable_within(self, unit.moves_left, false, true)
                    .into_iter()
                    .find(|&(x, y, _)| {
                        !self
                            .tile_units_ordered(x, y)
                            .any(|other| !self.allied(unit.owner, other.owner))
                            && (self.refuel_site(unit.owner, x, y)
                                || self.boarding_target(&unit, x, y).is_some())
                    });
                if let Some((x, y, _)) = destination
                    && let Some(path) = map.path(self, x, y)
                {
                    self.follow_source_ai_path(id, path);
                    if let Some(live) = self.units.iter().find(|actor| actor.id == id).cloned() {
                        if (live.x, live.y) == (x, y)
                            && self.boarding_target(&live, x, y).is_some()
                            && !self.refuel_site(live.owner, x, y)
                        {
                            self.board_or_deboard(id);
                        }
                        if let Some(actor) = self.units.iter_mut().find(|actor| actor.id == id) {
                            actor.state = UnitState::Idle;
                        }
                    }
                }
            }
        }

        let Some(index) = self.units.iter().position(|unit| unit.id == id) else {
            return;
        };
        let unit = &self.units[index];
        let refueled =
            self.refuel_site(unit.owner, unit.x, unit.y) || self.aboard_refueling_transport(unit);
        let unit = &mut self.units[index];
        unit.fuel = if refueled {
            max_fuel
        } else {
            unit.fuel.saturating_sub(1)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submarines_refuel_missiles_and_exclude_other_cargo() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (submarine, missile, _, doubled) = game.submarine_playtest_setup();
        assert!(doubled);
        assert_eq!(
            UnitKind::Submarine.required_tech().unwrap().name(),
            "Combustion"
        );
        assert!(!UnitKind::Submarine.can_carry(UnitKind::Fighter));
        assert!(!UnitKind::Submarine.can_carry(UnitKind::Warrior));
        assert!(UnitKind::Submarine.can_carry(UnitKind::Nuclear));
        assert!(game.board_or_deboard(missile));
        game.restore_aircraft();
        assert!(
            game.units.iter().any(|unit| unit.id == missile
                && unit.transport == Some(submarine)
                && unit.fuel == 1)
        );
    }

    #[test]
    fn airlift_uses_airport_capacity_and_resets_next_turn() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let (first, second, destination) = game.airlift_playtest_setup();
        assert!(game.apply(Command::Airlift(first, destination)));
        assert!(!game.apply(Command::Airlift(second, destination)));
        assert!(game.apply(Command::EndTurn));
        assert!(game.apply(Command::Airlift(second, destination)));
    }

    #[test]
    fn cruise_missiles_get_city_firepower_and_face_sdi_instead_of_sam() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (bomber, defender) = game.sam_battery_playtest_setup(true);
        let attacker = game
            .units
            .iter_mut()
            .find(|unit| unit.id == bomber)
            .unwrap();
        attacker.kind = UnitKind::CruiseMissile;
        let attacker = attacker.clone();
        let target = game
            .units
            .iter()
            .find(|unit| unit.id == defender)
            .unwrap()
            .clone();
        let defense = |game: &Simulation| {
            game.defense_power_against(&target, target.x, target.y, attacker.kind)
        };
        assert_eq!(defense(&game), 45);
        game.cities
            .last_mut()
            .unwrap()
            .buildings
            .push(Building::SDI_DEFENSE);
        assert_eq!(defense(&game), 90);
        assert_eq!(game.combat_firepower(&attacker, &target), (2, 1));
        assert_eq!(attacker.kind.fuel_capacity(), 1);
    }

    #[test]
    fn aegis_cruisers_defend_against_air_attackers_and_sail_without_cargo() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (_, id) = game.aegis_playtest_setup();
        let cruiser = game.units.iter().find(|unit| unit.id == id).unwrap();
        let (x, y) = (cruiser.x, cruiser.y);
        let defense = |attacker| game.defense_power_against(cruiser, x, y, attacker);
        assert_eq!(defense(UnitKind::Caravel), 80);
        assert_eq!(defense(UnitKind::Fighter), 400);
        assert_eq!(defense(UnitKind::Helicopter), 400);
        assert_eq!(cruiser.kind.transport_capacity(), 0);
        assert_eq!(cruiser.kind.required_tech(), Some(Tech::ROCKETRY));
        assert!(!UnitKind::Transport.can_carry(cruiser.kind));
        assert_eq!(game.movement_cost_for(cruiser, x, y), Some(MOVE_FRAGMENTS));
        let (land_x, land_y) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(land_x, land_y).terrain = Terrain::Grassland;
        let cruiser = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert_eq!(game.movement_cost_for(cruiser, land_x, land_y), None);
    }

    #[test]
    fn city_defenses_use_the_attacking_units_class() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (_, defender_id) = game.sam_battery_playtest_setup(true);
        let defender = game
            .units
            .iter()
            .find(|unit| unit.id == defender_id)
            .unwrap()
            .clone();
        let city = game.cities.last_mut().unwrap();
        city.buildings.extend([
            Building::COASTAL_DEFENSE,
            Building::CITY_WALLS,
            Building::GREAT_WALL,
        ]);

        let defense = |game: &Simulation, attacker| {
            game.defense_power_against(&defender, defender.x, defender.y, attacker)
        };
        assert_eq!(defense(&game, UnitKind::Bomber), 90);
        assert_eq!(defense(&game, UnitKind::Helicopter), 90);
        assert_eq!(defense(&game, UnitKind::Caravel), 90);
        assert_eq!(defense(&game, UnitKind::Musketeers), 135);
        game.players[defender.owner].known.push(Tech::MACHINE_TOOLS);
        assert_eq!(defense(&game, UnitKind::Musketeers), 112);
    }

    #[test]
    fn helicopters_carry_field_capable_troops_and_crash_only_when_health_runs_out() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        for (tx, ty) in [(x, y), (nx, ny)] {
            let tile = game.tile_mut(tx, ty);
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.hut = false;
        }
        let helicopter = game.spawn_unit(HUMAN, UnitKind::Helicopter, x, y);
        let musketeer = game.spawn_unit(HUMAN, UnitKind::Musketeers, x, y);
        let partisan = game.spawn_unit(HUMAN, UnitKind::Partisan, x, y);
        assert!(!game.board_or_deboard(musketeer));
        assert!(game.board_or_deboard(partisan));
        assert!(game.apply(Command::Move(helicopter, 1, 0)));
        assert!(game.board_or_deboard(partisan));
        assert!(game.units.iter().any(|unit| unit.id == partisan
            && (unit.x, unit.y) == (nx, ny)
            && unit.transport.is_none()));
        assert!(game.board_or_deboard(partisan));
        for _ in 0..9 {
            game.restore_aircraft();
        }
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == helicopter && unit.hp == 3)
        );
        game.restore_aircraft();
        assert!(!game.units.iter().any(|unit| unit.id == helicopter));
        assert!(game.units.iter().any(|unit| unit.id == partisan
            && (unit.x, unit.y) == (nx, ny)
            && unit.transport.is_none()));
    }

    #[test]
    fn bombardment_is_nonlethal_and_does_not_move_the_bomber() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (bomber, targets) = game.bombard_playtest_setup();
        let first = game
            .units
            .iter_mut()
            .find(|unit| unit.id == targets[0])
            .unwrap();
        first.hp = 2;
        assert!(game.apply(Command::Move(bomber, 1, 0)));
        assert_eq!(
            game.units
                .iter()
                .find(|unit| unit.id == targets[0])
                .unwrap()
                .hp,
            1
        );
        let aircraft = game.units.iter().find(|unit| unit.id == bomber).unwrap();
        assert_eq!(
            (aircraft.x, aircraft.y, aircraft.hp, aircraft.moves_left),
            (10, 20, 30, 0)
        );
    }

    #[test]
    fn aircraft_can_return_to_airstrips_or_crash_when_out_of_fuel() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, -1, 0);
        game.tile_mut(nx, ny).terrain = Terrain::Grassland;
        game.tile_mut(nx, ny).base = Some(Base::Airstrip);
        let first = game.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
        let second = game.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
        for unit in game
            .units
            .iter_mut()
            .filter(|unit| [first, second].contains(&unit.id))
        {
            unit.fuel = 1;
            if unit.id == second {
                unit.moves_left = 0;
            }
        }
        game.refresh_visibility(); // Newly placed human aircraft see the nearby airstrip.
        game.restore_aircraft();
        let saved = game.units.iter().find(|unit| unit.id == first).unwrap();
        assert_eq!((saved.x, saved.y, saved.fuel), (nx, ny, 3));
        assert!(!game.units.iter().any(|unit| unit.id == second));
    }
}

#[cfg(test)]
mod fighter_tests {
    use super::*;

    #[test]
    fn fighter_can_attack_an_airborne_bomber_and_refuels_to_two_turns() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (fighter, bomber) = game.fighter_playtest_setup();
        let warrior = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        assert!(!game.apply(Command::Move(warrior, 1, 0)));
        assert!(game.apply(Command::Move(fighter, 1, 0)));
        assert!(!game.units.iter().any(|unit| unit.id == bomber));
        let aircraft = game
            .units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap();
        assert_eq!(aircraft.fuel, 2);
        aircraft.fuel = 1;
        game.tile_mut(10, 20).base = Some(Base::Airbase);
        game.restore_aircraft();
        let aircraft = game.units.iter().find(|unit| unit.id == fighter).unwrap();
        assert_eq!(aircraft.fuel, 2);
    }
}

#[cfg(test)]
mod carrier_tests {
    use super::*;

    #[test]
    fn carrier_production_and_emergency_landing_follow_aircraft_cargo_rules() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (carrier, fighter) = game.carrier_playtest_setup();
        let port = Production::Unit(UnitKind::Carrier);
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.create_city(HUMAN, x, y);
        assert!(!game.can_produce(&game.cities[0], port));
        game.players[HUMAN].known.push(Tech::ADVANCED_FLIGHT);
        assert!(game.can_produce(&game.cities[0], port));
        game.cities.clear();
        assert!(UnitKind::Carrier.can_carry(UnitKind::Warrior));
        assert!(!UnitKind::Carrier.can_carry(UnitKind::Cannon));
        assert!(!UnitKind::Carrier.can_carry(UnitKind::Worker));
        assert!(!UnitKind::Transport.can_carry(UnitKind::Fighter));
        game.restore_aircraft();
        let saved = game.units.iter().find(|unit| unit.id == fighter).unwrap();
        assert_eq!((saved.transport, saved.fuel), (Some(carrier), 2));
    }
}

#[cfg(test)]
mod stealth_tests {
    use super::*;

    #[test]
    fn desktop_airbase_sortie_fixture_uses_fighter_and_preserves_safe_return() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_bomber_playtest(), (true, true, true));
    }

    fn missile_hunt(distance: i32, kind: UnitKind) -> (Simulation, u32, u32, u32) {
        loaded_hunt(distance, kind, UnitKind::Submarine)
    }

    fn loaded_hunt(
        distance: i32,
        kind: UnitKind,
        platform: UnitKind,
    ) -> (Simulation, u32, u32, u32) {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.prune_unit_ai_state();
        game.sign_pact(1, HUMAN, Pact::War);
        game.players[1]
            .known
            .extend([Tech::ADVANCED_FLIGHT, Tech::ROCKETRY]);
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.base = None;
            tile.owner = None;
            tile.hut = false;
            tile.known_by.fill(true);
        }
        let boat = game.spawn_unit(1, platform, 10, 20);
        let missile = game.spawn_unit(1, kind, 10, 20);
        assert!(game.board_specific_ship(missile, boat));
        let target = offset_native_by_map(10, 20, distance, 0);
        let victim = game.spawn_unit(HUMAN, UnitKind::Caravel, target.0, target.1);
        game.units.last_mut().unwrap().hp = 1;
        game.set_random_seed(4);
        (game, boat, missile, victim)
    }

    #[test]
    fn hunter_launch_preserves_transport_pf_and_divided_missile_budget() {
        let (mut game, boat, missile, victim) = missile_hunt(3, UnitKind::CruiseMissile);
        game.source_hunter_launch(boat, victim);
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == missile && unit.transport == Some(boat))
        );
        let (mut game, boat, missile, victim) = missile_hunt(2, UnitKind::CruiseMissile);
        game.source_hunter_launch(boat, victim);
        assert!(
            game.units
                .iter()
                .all(|unit| unit.id != victim && unit.id != missile)
        );
    }

    #[test]
    fn desktop_native_hunter_launch_fixture_exercises_range_and_nuclear_cargo() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_source_hunter_launch_playtest(), (true, true, true));
    }

    #[test]
    fn carrier_bomber_boss_holds_fighter_reserve_instead_of_independent_cargo_dispatch() {
        let (mut game, boat, bomber, victim) = loaded_hunt(2, UnitKind::Bomber, UnitKind::Carrier);
        let fighter = game.spawn_unit(1, UnitKind::Fighter, 10, 20);
        assert!(game.board_specific_ship(fighter, boat));
        game.run_ai_player(1);
        assert_eq!(game.ferry_boss(boat), Some(bomber));
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && !unit.moved && unit.transport == Some(boat))
        );
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == boat && (unit.x, unit.y) == (10, 20))
        );
    }

    #[test]
    fn carrier_without_moves_cannot_activate_its_aircraft_boss() {
        let (mut game, boat, fighter, victim) =
            loaded_hunt(2, UnitKind::Fighter, UnitKind::Carrier);
        game.units
            .iter_mut()
            .find(|unit| unit.id == boat)
            .unwrap()
            .moves_left = 0;
        game.run_ai_player(1);
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && !unit.moved)
        );
    }

    #[test]
    fn carrier_does_not_activate_a_distant_reserved_aircraft_without_cargo() {
        let (mut game, boat, fighter, victim) =
            loaded_hunt(2, UnitKind::Fighter, UnitKind::Carrier);
        assert!(game.board_or_deboard(fighter));
        let remote = offset_native_by_map(10, 20, 5, 0);
        game.tile_mut(remote.0, remote.1).terrain = Terrain::Grassland;
        game.tile_mut(remote.0, remote.1).base = Some(Base::Airstrip);
        let actor = game
            .units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap();
        (actor.x, actor.y) = remote;
        let actor = actor.clone();
        let target = offset_native_by_map(remote.0, remote.1, 2, 0);
        let enemy = game
            .units
            .iter_mut()
            .find(|unit| unit.id == victim)
            .unwrap();
        (enemy.x, enemy.y) = target;
        assert!(game.aircraft_attack_path(&actor).is_some());
        game.release_ai_ferry(fighter);
        assert!(game.assign_ai_ferry(boat, fighter));
        let ship = game
            .units
            .iter()
            .find(|unit| unit.id == boat)
            .unwrap()
            .clone();
        let (handled, done) = game.run_source_air_ferry(&ship);
        assert!(!handled && done.is_empty());
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && unit.moves_left == actor.moves_left)
        );
    }

    #[test]
    fn empty_air_refuel_path_does_not_embark_until_emergency_rescue() {
        let (mut game, boat, fighter, victim) =
            loaded_hunt(2, UnitKind::Fighter, UnitKind::Carrier);
        assert!(game.board_or_deboard(fighter));
        let actor = game
            .units
            .iter()
            .find(|unit| unit.id == fighter)
            .unwrap()
            .clone();
        assert_eq!(actor.fuel, 2);
        game.run_ai_strike_aircraft(&actor);
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(game.units.iter().any(|unit| unit.id == fighter
            && unit.transport.is_none()
            && unit.moves_left == actor.moves_left));
        game.restore_aircraft();
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && unit.transport.is_none() && unit.fuel == 1)
        );
        game.restore_aircraft();
        assert!(game.units.iter().any(|unit| unit.id == fighter
            && unit.transport == Some(boat)
            && unit.fuel == 2
            && unit.hp == actor.hp - 2 * percent_of(actor.kind.stats().hp, 10)));
    }

    #[test]
    fn fuel_free_helicopter_takes_airborne_loss_without_emergency_embark() {
        let (mut game, _boat, helicopter, _victim) =
            loaded_hunt(2, UnitKind::Helicopter, UnitKind::Carrier);
        assert!(game.board_or_deboard(helicopter));
        let before = game
            .units
            .iter()
            .find(|unit| unit.id == helicopter)
            .unwrap()
            .hp;
        game.restore_aircraft();
        assert!(game.units.iter().any(|unit| unit.id == helicopter
            && unit.transport.is_none()
            && unit.fuel == 0
            && unit.hp == before - percent_of(UnitKind::Helicopter.stats().hp, 10)));
    }

    #[test]
    fn hunter_launch_supports_nuclear_cargo_and_cargo_append_order() {
        let (mut game, boat, missile, victim) = missile_hunt(2, UnitKind::CruiseMissile);
        let nuclear = game.spawn_unit(1, UnitKind::Nuclear, 10, 20);
        assert!(game.board_specific_ship(nuclear, boat));
        game.source_hunter_launch(boat, victim);
        assert!(
            game.units
                .iter()
                .all(|unit| unit.id != victim && unit.id != missile)
        );
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == nuclear && unit.transport == Some(boat))
        );
        let (mut game, boat, missile, victim) = missile_hunt(2, UnitKind::Nuclear);
        game.source_hunter_launch(boat, victim);
        assert!(
            game.units
                .iter()
                .all(|unit| unit.id != victim && unit.id != missile)
        );
    }

    #[test]
    fn immobile_auxiliary_threat_does_not_preempt_primary_hunter_target() {
        let (mut game, boat, missile, victim) = missile_hunt(2, UnitKind::CruiseMissile);
        let point = offset_native_by_map(10, 20, 0, 1);
        let threat = game.spawn_unit(HUMAN, UnitKind::Battleship, point.0, point.1);
        game.units.last_mut().unwrap().moves_left = 0;
        game.source_hunter_launch(boat, victim);
        assert!(game.units.iter().any(|unit| unit.id == threat));
        assert!(
            game.units
                .iter()
                .all(|unit| unit.id != victim && unit.id != missile)
        );
    }

    #[test]
    fn desktop_carrier_sortie_fixture_uses_fighter_and_recovers_aboard() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_carrier_sortie_playtest(false), (true, true, true));
    }

    #[test]
    fn desktop_carrier_does_not_reposition_to_extend_its_aircraft_sortie() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_carrier_sortie_playtest(true), (true, true, true));
    }

    #[test]
    fn fuel_mission_fixture_exercises_sortie_wait_and_literal_oneattack_limit() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_fuel_mission_playtest());
    }

    #[test]
    fn default_ai_air_target_pf_is_omniscient_without_map_handicap() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let plane = game
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        let target = game.units.iter().find(|unit| unit.id == victim).unwrap();
        let point = (target.x, target.y);
        game.tile_mut(point.0, point.1).known_by[plane.owner] = false;
        assert_eq!(
            game.aircraft_attack_path(&plane).map(|path| path.1),
            Some(point)
        );
    }

    #[test]
    fn known_refuel_extras_work_on_enemy_owned_tiles() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        let point = (10, 20);
        game.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
        game.tile_mut(point.0, point.1).base = Some(Base::Airstrip);
        game.tile_mut(point.0, point.1).owner = Some(HUMAN);
        game.tile_mut(point.0, point.1).known_by[1] = true;
        game.sign_pact(1, HUMAN, Pact::War);
        let id = game.spawn_unit(1, UnitKind::Fighter, point.0, point.1);
        game.units
            .iter_mut()
            .find(|unit| unit.id == id)
            .unwrap()
            .fuel = 1;
        assert!(game.refuel_site(1, point.0, point.1));
        game.restore_aircraft();
        assert_eq!(
            game.units.iter().find(|unit| unit.id == id).unwrap().fuel,
            2
        );
        game.tile_mut(point.0, point.1).known_by[1] = false;
        assert!(!game.refuel_site(1, point.0, point.1));
    }

    #[test]
    fn stealth_aircraft_replace_older_models_and_need_close_detection() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (explorer, hidden) = game.stealth_playtest_setup();
        game.units.last_mut().unwrap().kind = UnitKind::StealthFighter;
        assert!(!game.unit_visible_to_human(game.units.last().unwrap()));
        assert!(game.apply(Command::Move(explorer, 1, 0)));
        let aircraft = game.units.iter().find(|unit| unit.id == hidden).unwrap();
        assert!(game.unit_visible_to_human(aircraft));

        game.create_city(HUMAN, 10, 20);
        let city = &game.cities[0];
        game.players[HUMAN]
            .known
            .extend([Tech::FLIGHT, Tech::ADVANCED_FLIGHT, Tech::STEALTH]);
        for (old, new) in [
            (UnitKind::Fighter, UnitKind::StealthFighter),
            (UnitKind::Bomber, UnitKind::StealthBomber),
        ] {
            assert!(!game.can_produce(city, Production::Unit(old)));
            assert!(game.can_produce(city, Production::Unit(new)));
            assert_eq!(game.production_successor(city, old), Some(new));
            assert_eq!(new.fuel_capacity(), old.fuel_capacity());
        }
    }
    #[test]
    fn existing_aircraft_attack_profitable_targets_but_hp_loss_blocks_offensive_builds() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let plane = game.units.iter().find(|u| u.id == id).unwrap().clone();
        let target = game.units.iter().find(|u| u.id == victim).unwrap().clone();
        assert!(game.unit_buildable(&game.cities[0], UnitKind::Fighter));
        assert_eq!(UnitKind::Fighter.hp_loss_percent(), 10);
        assert!(game.aircraft_target_value(&plane, target.x, target.y) > 0.0);
        assert!(game.aircraft_production_choice(&game.cities[0]).is_none());
        game.players[1].known.push(Tech::MEDICINE);
        game.cities[0].buildings.push(Building::AIRPORT);
        assert!(game.aircraft_production_choice(&game.cities[0]).is_none());
        game.run_ai_strike_aircraft(&plane);
        assert!(!game.units.iter().any(|u| u.id == victim));
        game.players[1].known.clear();
        assert!(game.aircraft_production_choice(&game.cities[0]).is_none());
    }

    #[test]
    fn airborne_plane_refuels_before_attacking_and_waits_for_full_fuel() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let home = (game.cities[0].x, game.cities[0].y);
        let open = offset_native_by_map(home.0, home.1, 1, 0);
        let plane = game.units.iter_mut().find(|u| u.id == id).unwrap();
        (plane.x, plane.y) = open;
        plane.fuel = 1;
        let snapshot = plane.clone();
        game.run_ai_strike_aircraft(&snapshot);
        let plane = game.units.iter().find(|u| u.id == id).unwrap().clone();
        assert_eq!((plane.x, plane.y), home);
        assert!(game.units.iter().any(|u| u.id == victim));
        game.run_ai_strike_aircraft(&plane);
        assert!(game.units.iter().any(|u| u.id == victim));
    }

    #[test]
    fn standalone_nuclear_uses_native_air_manager_strict_attack_cutoff() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (fighter, victim) = game.ai_aircraft_advisor_playtest_setup();
        game.units.retain(|u| u.id != fighter);
        let city = game.cities[0].clone();
        let target = game.units.iter().find(|u| u.id == victim).unwrap().clone();
        let id = game.spawn_unit(1, UnitKind::Nuclear, city.x, city.y);
        let plane = game.units.iter().find(|u| u.id == id).unwrap().clone();
        game.units.iter_mut().find(|u| u.id == victim).unwrap().kind = UnitKind::Warrior;
        assert!(game.aircraft_best_target(&plane).is_none());
        for _ in 0..4 {
            game.spawn_unit(HUMAN, UnitKind::Battleship, target.x, target.y);
        }
        assert!(game.aircraft_target_value(&plane, target.x, target.y) > 0.0);
        let mut map = super::super::fuel_paths::FuelPaths::new(&game, &plane, true).unwrap();
        assert_eq!(
            map.movement_cost(&game, target.x, target.y),
            Some(plane.moves_left)
        );
        assert!(game.aircraft_best_target(&plane).is_none());
        let mut short_range = plane.clone();
        short_range.moves_left = MOVE_FRAGMENTS;
        assert!(game.aircraft_best_target(&short_range).is_none());
        game.run_ai_nuclear_aircraft(&plane);
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == id && unit.moves_left == plane.moves_left)
        );
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(game.ai_nuclear_advisor_playtest());
    }
    #[test]
    fn aircraft_rebases_toward_targets_outside_its_current_sortie() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let plane = game.units.iter().find(|u| u.id == id).unwrap().clone();
        let base = offset_native_by_map(plane.x, plane.y, 6, 0);
        let target = offset_native_by_map(plane.x, plane.y, 12, 0);
        game.tile_mut(base.0, base.1).terrain = Terrain::Grassland;
        game.tile_mut(base.0, base.1).base = Some(Base::Airbase);
        game.tile_mut(base.0, base.1).owner = Some(1);
        let enemy = game.units.iter_mut().find(|u| u.id == victim).unwrap();
        (enemy.x, enemy.y) = target;
        game.tile_mut(target.0, target.1).terrain = Terrain::Ocean;
        assert!(game.aircraft_best_target(&plane).is_none());
        game.run_ai_strike_aircraft(&plane);
        let plane = game.units.iter().find(|u| u.id == id).unwrap();
        assert_eq!((plane.x, plane.y), base);
        assert!(game.units.iter().any(|u| u.id == victim));
    }
    #[test]
    fn city_air_raids_need_occupiers_but_virtual_production_can_plan_ahead() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let plane = game.units.iter().find(|u| u.id == id).unwrap().clone();
        let enemy = game.units.iter_mut().find(|u| u.id == victim).unwrap();
        enemy.kind = UnitKind::Warrior;
        let target = (enemy.x, enemy.y);
        game.tile_mut(target.0, target.1).terrain = Terrain::Grassland;
        game.create_city(HUMAN, target.0, target.1);
        assert_eq!(game.aircraft_target_value(&plane, target.0, target.1), 0.0);
        let mut virtual_plane = plane.clone();
        virtual_plane.id = 0;
        assert!(game.aircraft_target_value(&virtual_plane, target.0, target.1) > 0.0);
        let occupier = game.spawn_unit(1, UnitKind::Warrior, plane.x, plane.y);
        // Nearby troops alone do not support bombing a defended city.
        assert_eq!(game.aircraft_target_value(&plane, target.0, target.1), 0.0);
        let city = game
            .cities
            .iter()
            .find(|c| (c.x, c.y) == target)
            .unwrap()
            .id;
        game.ai_ground_targets.insert(occupier, city);
        assert!(game.aircraft_target_value(&plane, target.0, target.1) > 0.0);
    }
    #[test]
    fn damaged_aircraft_prefers_fast_repair_but_finishes_imminent_home_repair() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        game.units.retain(|u| u.id != victim);
        let home = (game.cities[0].x, game.cities[0].y);
        let airport = offset_native_by_map(home.0, home.1, 4, 0);
        game.tile_mut(airport.0, airport.1).terrain = Terrain::Grassland;
        game.create_city(1, airport.0, airport.1);
        game.cities
            .last_mut()
            .unwrap()
            .buildings
            .push(Building::AIRPORT);
        let plane = game.units.iter_mut().find(|u| u.id == id).unwrap();
        plane.hp = 5;
        plane.moved = true;
        let plane = plane.clone();
        assert!(
            game.aircraft_regen_turns(&plane, airport.0, airport.1)
                < game.aircraft_regen_turns(&plane, home.0, home.1)
        );
        game.run_ai_strike_aircraft(&plane);
        let plane = game.units.iter_mut().find(|u| u.id == id).unwrap();
        assert_eq!((plane.x, plane.y), airport);
        (plane.x, plane.y) = home;
        plane.hp = plane.kind.stats().hp - 1;
        plane.moved = false;
        plane.moves_left = plane.kind.stats().moves * MOVE_FRAGMENTS;
        let plane = plane.clone();
        assert!(game.strategic_airbase(&plane, &[home, airport]).is_none());
    }

    #[test]
    fn airborne_damaged_plane_selects_repair_time_before_distance() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        game.units.retain(|u| u.id != victim);
        let home = (game.cities[0].x, game.cities[0].y);
        let airport = offset_native_by_map(home.0, home.1, 4, 0);
        game.tile_mut(airport.0, airport.1).terrain = Terrain::Grassland;
        game.create_city(1, airport.0, airport.1);
        game.cities
            .last_mut()
            .unwrap()
            .buildings
            .push(Building::AIRPORT);
        let plane = game.units.iter_mut().find(|u| u.id == id).unwrap();
        (plane.x, plane.y) = offset_native_by_map(home.0, home.1, 1, 0);
        plane.hp = 5;
        let plane = plane.clone();
        game.run_ai_strike_aircraft(&plane);
        let plane = game.units.iter().find(|u| u.id == id).unwrap();
        assert_eq!((plane.x, plane.y), airport);
    }

    #[test]
    fn strategic_airbase_reinforces_grave_danger_and_holds_its_own_city() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        game.units.retain(|u| u.id != victim);
        let plane = game.units.iter().find(|u| u.id == id).unwrap().clone();
        let home = (plane.x, plane.y);
        let front = offset_native_by_map(home.0, home.1, 5, 0);
        let attack = offset_native_by_map(front.0, front.1, 1, 0);
        game.tile_mut(front.0, front.1).terrain = Terrain::Grassland;
        game.tile_mut(attack.0, attack.1).terrain = Terrain::Grassland;
        game.create_city(1, front.0, front.1);
        for _ in 0..3 {
            game.spawn_unit(HUMAN, UnitKind::Warrior, attack.0, attack.1);
        }
        assert!(game.aircraft_city_needs_defender(&plane, front.0, front.1, false));
        assert_eq!(
            game.strategic_airbase(&plane, &[home, front]).map(|r| r.1),
            Some(front)
        );
        let mut defender = plane.clone();
        (defender.x, defender.y) = front;
        assert!(game.strategic_airbase(&defender, &[home, front]).is_none());
        for _ in 0..2 {
            game.spawn_unit(1, UnitKind::Warrior, front.0, front.1);
        }
        assert!(!game.aircraft_city_needs_defender(&plane, front.0, front.1, false));
    }
    #[test]
    fn airlift_refreshes_contact_but_stationary_bombardment_does_not() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        let source = (10, 20);
        let destination = offset_native_by_map(source.0, source.1, 6, 0);
        game.create_city(HUMAN, source.0, source.1);
        game.create_city(HUMAN, destination.0, destination.1);
        for city in &mut game.cities {
            city.buildings.push(Building::AIRPORT);
        }
        let arrival_city = game.cities[1].id;
        let visitor = game.spawn_unit(HUMAN, UnitKind::Warrior, source.0, source.1);
        let target = offset_native_by_map(destination.0, destination.1, 1, 0);
        game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
        game.turn += 30;
        game.diplomatic_contact_until.clear();
        assert!(game.airlift(visitor, arrival_city));
        assert!(
            game.diplomatic_contact_until
                .get(&(HUMAN, 1))
                .is_some_and(|until| *until > game.turn)
        );
        let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, destination.0, destination.1);
        game.diplomatic_contact_until.clear();
        let aircraft = game.units.iter().find(|u| u.id == bomber).unwrap().clone();
        assert_eq!(
            game.try_bombard(&aircraft, target.0, target.1),
            MoveResult::Fought
        );
        assert!(game.diplomatic_contact_until.is_empty());
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.diplomatic_contact_until.clear();
        assert_eq!(
            game.try_bombard(&aircraft, target.0, target.1),
            MoveResult::Blocked
        );
        assert!(game.diplomatic_contact_until.is_empty());
    }

    #[test]
    fn lost_paradrop_establishes_contact_at_the_actual_landing() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        let origin = (10, 20);
        let landing = offset_native_by_map(origin.0, origin.1, 5, 0);
        game.create_city(HUMAN, origin.0, origin.1);
        let troop = game.spawn_unit(HUMAN, UnitKind::Paratroopers, origin.0, origin.1);
        game.spawn_unit(1, UnitKind::Warrior, landing.0, landing.1);
        game.tile_mut(landing.0, landing.1).known_by[HUMAN] = true;
        game.turn += 30;
        game.diplomatic_contact_until.clear();
        assert!(game.paradrop(troop, landing.0, landing.1));
        assert!(!game.units.iter().any(|u| u.id == troop));
        assert!(
            game.diplomatic_contact_until
                .get(&(HUMAN, 1))
                .is_some_and(|until| *until > game.turn)
        );
    }
}
