//! AI planning for aircraft missions and refueling.

use super::*;

type AircraftAttackPath = (Vec<(i32, i32)>, (i32, i32), f64);
type AircraftBaseRoute = (Vec<(i32, i32)>, (i32, i32));

impl Simulation {
    fn aircraft_production_denominator(&self, plane: &Unit) -> i32 {
        let mut total = 0;
        let mut count = 0;
        for city in &self.cities {
            if city.owner == plane.owner {
                total += self.city_forecast(city).shield_surplus();
                count += 1;
            }
        }
        let average = total / count.max(1);
        let home = plane
            .home_city
            .and_then(|id| self.city_by_id(id))
            .map(|c| self.city_forecast(c).shield_surplus())
            .unwrap_or(1);
        home.max(average).max(1)
    }

    pub(in crate::sim) fn aircraft_target_value(&self, plane: &Unit, x: i32, y: i32) -> f64 {
        let max_output = self.aircraft_production_denominator(plane);
        self.aircraft_target_value_with_output(plane, x, y, max_output)
    }

    fn aircraft_target_value_with_output(
        &self,
        plane: &Unit,
        x: i32,
        y: i32,
        max_output: i32,
    ) -> f64 {
        let (_, war_mask, _) = self.pact_masks(plane.owner);
        let at_war_owner = |other: usize| {
            (other < 64 && ((war_mask >> other) & 1) != 0)
                || (other >= 64 && self.at_war(plane.owner, other))
        };
        if !self.players[plane.owner].ai && !self.tile(x, y).known_by[plane.owner]
            || self.any_unit_at(x, y, |u| !at_war_owner(u.owner))
            || self
                .city_at_cached(x, y)
                .is_some_and(|c| !at_war_owner(c.owner))
        {
            return 0.0;
        }
        if plane.id != 0
            && self
                .city_at_cached(x, y)
                .is_some_and(|city| self.invasion_support(plane.owner, city, plane.id).1 == 0)
        {
            return 0.0;
        }
        let Some(defender) = self
            .best_defender_id(plane, x, y)
            .and_then(|id| self.unit_by_id(id))
        else {
            return 0.0;
        };
        let stats = plane.kind.stats();
        let (win, risk) = if stats.attack == 0 && plane.kind.bombard_strength() > 0 {
            let bombard = f64::from(plane.kind.bombard_strength()) * 10.0;
            let defense = f64::from(
                self.defense_power_against(defender, x, y, plane.kind)
                    .max(1),
            );
            let hit_chance = bombard / (bombard + defense);
            // Bombardment deals non-lethal damage without direct ground retaliation.
            let expected_fraction = (hit_chance * f64::from(plane.kind.bombard_rate().max(1))
                / f64::from(defender.hp.max(1)))
            .min(0.75);
            (expected_fraction, 0.0)
        } else {
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
            let risk = f64::from(stats.cost * stats.hp) / f64::from(plane.hp.max(1));
            (win, risk)
        };
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
        let profit = 17.0 * (f64::from(benefit) * win - risk * (1.0 - win)) - 17.0 + 36.0;
        if profit <= 0.0 {
            return 0.0;
        }
        let eff_attack = stats.attack.max(plane.kind.bombard_strength());
        let balanced = 2 * stats.cost * eff_attack / (eff_attack + stats.defense).max(1);
        profit * (23.0_f64 / 24.0).powi(1 + balanced / max_output)
    }

    /// Count observable enemy fixed-wing air threat pressure against `owner`
    /// without peeking through fog of war: visible hostile aircraft plus
    /// rivals at war (or under a war plan) that launched air raids or air
    /// interceptions against `owner` within the last 12 turns.
    pub(in crate::sim) fn observed_enemy_air_threat(&self, owner: usize) -> usize {
        let relevant_rival = |other: usize| {
            other != owner
                && (self.at_war(owner, other) || self.ai_war_plans.contains_key(&(owner, other)))
        };
        let visible_aircraft = self
            .units
            .iter()
            .filter(|enemy| {
                enemy.kind.is_fixed_wing_aircraft()
                    && relevant_rival(enemy.owner)
                    && self.unit_visible_to(owner, enemy)
            })
            .count();
        let active_raiding_rivals = (0..self.players.len())
            .filter(|&other| {
                relevant_rival(other)
                    && self
                        .ai_air_raid_turn
                        .get(&(owner, other))
                        .is_some_and(|&turn| self.turn - turn <= 12)
            })
            .count();
        visible_aircraft + active_raiding_rivals * 2
    }

    /// Estimate the observable air-raid danger to `city` for counter-air
    /// production (`SAM_BATTERY`, `MobileSAM`/`Flak`, and CAP fighters).
    pub(in crate::sim) fn city_air_threat_score(&self, city: &City) -> f64 {
        let global_threat = self.observed_enemy_air_threat(city.owner);
        if global_threat == 0 {
            return 0.0;
        }
        let mut score = 0.0_f64;
        if let Some(&bombed_turn) = self.ai_city_bombed_turn.get(&city.id) {
            let age = (self.turn - bombed_turn).max(0);
            if age <= 10 {
                score += 4.0 - 0.2 * f64::from(age);
            }
        }
        for enemy in &self.units {
            if !enemy.kind.is_fixed_wing_aircraft()
                || enemy.owner == city.owner
                || !(self.at_war(city.owner, enemy.owner)
                    || self.ai_war_plans.contains_key(&(city.owner, enemy.owner)))
                || !self.unit_visible_to(city.owner, enemy)
            {
                continue;
            }
            let dist = self
                .map_wrap
                .step_distance(city.x, city.y, enemy.x, enemy.y);
            if dist <= enemy.kind.operational_range() {
                score += 2.0;
            } else if dist <= enemy.kind.rebase_range() {
                score += 0.75;
            }
        }
        let nearby_raiding_bases = self
            .cities
            .iter()
            .filter(|other| {
                other.owner != city.owner
                    && self.at_war(city.owner, other.owner)
                    && self
                        .ai_air_raid_turn
                        .get(&(city.owner, other.owner))
                        .is_some_and(|&turn| self.turn - turn <= 12)
                    && self.tile(other.x, other.y).known_by[city.owner]
                    && self
                        .map_wrap
                        .step_distance(city.x, city.y, other.x, other.y)
                        <= 16
            })
            .count();
        score += (nearby_raiding_bases.min(2) as f64) * 1.75;
        score
    }

    /// Evaluate Civ III counter-air defense production (`SAM_BATTERY` first,
    /// then `MobileSAM` / `Flak` tile AA) when `city` faces an observable
    /// enemy air threat. Returns `None` when no enemy air threat is observed.
    pub(in crate::sim) fn air_defense_production_choice(
        &self,
        city: &City,
    ) -> Option<super::advisors::AiBuildChoice> {
        let threat = self.city_air_threat_score(city);
        if threat <= 0.0 {
            return None;
        }
        let improvements = city
            .buildings
            .iter()
            .filter(|b| b.data().genus == crate::rules::Genus::Improvement)
            .count() as f64;
        let city_importance =
            1.0 + 0.1 * f64::from(city.size.min(16)) + 0.15 * improvements.min(6.0);
        let mut best: Option<super::advisors::AiBuildChoice> = None;

        // 1. City SAM Battery (50% fire chance, 8/(8+D) shootdown, aborts run).
        if !city.has_building(crate::rules::Building::SAM_BATTERY)
            && self
                .building_blocker(city, crate::rules::Building::SAM_BATTERY)
                .is_none()
        {
            let builder_factor = super::ai_traits::builder_building_want_factor();
            let raw_want = (110.0 + threat * 42.0) * city_importance;
            let want = (raw_want * f64::from(builder_factor)).min(480.0);
            best = Some(super::advisors::AiBuildChoice {
                production: Production::Building(crate::rules::Building::SAM_BATTERY),
                want,
            });
        }

        // 2. Tile AA units (Mobile SAM / Flak) when SAM Battery is already
        //    built or unavailable and the city has fewer than 2 AA units.
        let tile_aa_units = self
            .tile_units_ordered(city.x, city.y)
            .filter(|u| u.owner == city.owner && u.kind.air_defense() > 0)
            .count();
        if tile_aa_units < 2 {
            let best_aa_unit = [UnitKind::MobileSAM, UnitKind::Flak]
                .into_iter()
                .find(|&kind| self.unit_buildable(city, kind));
            if let Some(kind) = best_aa_unit {
                let has_sam = city.has_building(crate::rules::Building::SAM_BATTERY);
                let sam_buildable = !has_sam
                    && self
                        .building_blocker(city, crate::rules::Building::SAM_BATTERY)
                        .is_none();
                let aa_scale = if sam_buildable { 0.7 } else { 1.0 };
                let want = ((80.0 + threat * f64::from(kind.air_defense()) * 14.0)
                    * aa_scale
                    * if tile_aa_units == 0 { 1.0 } else { 0.65 })
                .min(340.0);
                if best.is_none_or(|old| want > old.want) {
                    best = Some(super::advisors::AiBuildChoice {
                        production: Production::Unit(kind),
                        want,
                    });
                }
            }
        }
        best
    }

    pub(in crate::sim) fn aircraft_production_choice(
        &self,
        city: &City,
    ) -> Option<(UnitKind, f64)> {
        let (_, war_mask, _) = self.pact_masks(city.owner);
        let has_war_or_plan = war_mask != 0
            || (0..self.players.len())
                .any(|other| other != city.owner && self.at_war(city.owner, other))
            || self
                .ai_war_plans
                .keys()
                .any(|&(planner, _)| planner == city.owner);
        if !has_war_or_plan {
            return None;
        }
        let mut best = None;
        let mut best_value = 0.0;
        let mut max_output: Option<i32> = None;
        for kind in UnitKind::all().filter(|kind| kind.fuel_capacity() > 0) {
            // Fuelled aircraft refuel at cities/airbases every turn, so do not
            // reject them for the Air class's field hp_loss_percent.
            if !self.unit_buildable(city, kind) {
                continue;
            }
            let mut plane = self.make_unit(0, city.owner, kind, city.x, city.y);
            plane.home_city = Some(city.id);
            plane.veteran = self.trained_rank(city, kind);
            plane.hp = plane.max_hit_points();
            let output =
                *max_output.get_or_insert_with(|| self.aircraft_production_denominator(&plane));
            let value = if kind.is_fixed_wing_aircraft() {
                self.aircraft_production_value(city, &plane, output)
            } else {
                self.aircraft_attack_path_with_output(&plane, output)
                    .map_or(0.0, |(_, _, v)| v)
            };
            if value > best_value {
                best_value = value;
                best = Some((kind, value));
            }
        }
        best
    }

    /// Score a buildable Civ III fixed-wing aircraft by its bombard mission value
    /// from this city (or reachable forward bases within rebase range) plus CAP
    /// interception value when an observable enemy air threat exists.
    ///
    /// Valuation for Civ III Conquests fixed-wing aircraft.
    fn aircraft_production_value(&self, city: &City, plane: &Unit, max_output: i32) -> f64 {
        let stats = plane.kind.stats();
        let own_cities = self
            .cities
            .iter()
            .filter(|c| c.owner == city.owner)
            .count()
            .max(1);
        let own_fighters = self
            .units
            .iter()
            .filter(|u| u.owner == city.owner && is_air_superiority_unit(u.kind))
            .count();
        let own_bombers = self
            .units
            .iter()
            .filter(|u| {
                u.owner == city.owner
                    && u.kind.is_fixed_wing_aircraft()
                    && !is_air_superiority_unit(u.kind)
            })
            .count();

        let best_local = self.best_bombard_value_from(plane, city.x, city.y);
        let best_forward = if best_local > 0.0 {
            best_local
        } else {
            let rebase_limit = plane.kind.rebase_range();
            self.cities
                .iter()
                .filter(|other| {
                    other.id != city.id
                        && self.allied(city.owner, other.owner)
                        && self
                            .map_wrap
                            .step_distance(city.x, city.y, other.x, other.y)
                            <= rebase_limit
                })
                .map(|other| 0.75 * self.best_bombard_value_from(plane, other.x, other.y))
                .fold(0.0_f64, f64::max)
        };

        // Also allow pre-war buildup against planned war targets' known cities.
        let war_plan_score = if best_forward <= 0.0 {
            let max_reach = plane.kind.operational_range() + plane.kind.rebase_range();
            let has_planned_target = self.cities.iter().any(|other| {
                other.owner != city.owner
                    && self.ai_war_plans.contains_key(&(city.owner, other.owner))
                    && self.tile(other.x, other.y).known_by[city.owner]
                    && self
                        .map_wrap
                        .step_distance(city.x, city.y, other.x, other.y)
                        <= max_reach
            });
            if has_planned_target {
                f64::from(plane.kind.bombard_strength() * plane.kind.bombard_rate()) * 0.25
            } else {
                0.0
            }
        } else {
            0.0
        };

        let is_fighter = is_air_superiority_unit(plane.kind);
        let strike_cap = (own_cities * 2).max(2);
        let bomber_diminishing = if !is_fighter {
            if own_bombers >= strike_cap {
                0.25
            } else {
                1.0 / (1.0 + 0.18 * own_bombers as f64)
            }
        } else {
            1.0 / (1.0 + 0.22 * own_fighters as f64)
        };

        let mut raw_score = (best_forward + war_plan_score) * 28.0 * bomber_diminishing;
        if plane.kind.is_stealth_aircraft() && raw_score > 0.0 {
            raw_score *= 1.2;
        }

        if is_fighter {
            let observed_threat = self.observed_enemy_air_threat(city.owner);
            let desired_fighters = (observed_threat + 1).min(own_cities * 2);
            if observed_threat > 0 && own_fighters < desired_fighters {
                let cap_need = (desired_fighters - own_fighters).min(4) as f64;
                let cap_bonus = (120.0 + cap_need * f64::from(stats.attack.max(1)) * 22.0)
                    * if city.has_building(crate::rules::Building::SAM_BATTERY) {
                        0.85
                    } else {
                        1.0
                    };
                raw_score += cap_bonus;
            }
        }

        if raw_score <= 0.0 {
            return 0.0;
        }
        let eff_attack = stats.attack.max(plane.kind.bombard_strength());
        let balanced = 2 * stats.cost * eff_attack / (eff_attack + stats.defense).max(1);
        raw_score * (23.0_f64 / 24.0).powi(1 + balanced / max_output)
    }

    fn best_bombard_value_from(&self, plane: &Unit, bx: i32, by: i32) -> f64 {
        let range = Self::fixed_wing_bombard_range(plane.kind);
        if range <= 0 {
            return 0.0;
        }
        let mut staged = plane.clone();
        staged.x = bx;
        staged.y = by;
        staged.moves_left = self.unit_move_rate(plane);
        let (_, war_mask, _) = self.pact_masks(plane.owner);
        let mut best = 0.0_f64;
        for (x, y, _) in self.map_wrap.tiles_within_steps(bx, by, range) {
            let hostile_city = self.city_at_cached(x, y).is_some_and(|c| {
                c.owner != plane.owner
                    && ((c.owner < 64 && ((war_mask >> c.owner) & 1) != 0)
                        || (c.owner >= 64 && self.at_war(plane.owner, c.owner)))
            });
            let hostile_unit = self.any_unit_at(x, y, |u| {
                u.owner != plane.owner
                    && ((u.owner < 64 && ((war_mask >> u.owner) & 1) != 0)
                        || (u.owner >= 64 && self.at_war(plane.owner, u.owner)))
            });
            if !hostile_city && !hostile_unit {
                continue;
            }
            if !self.can_bombard_tile(&staged, x, y) {
                continue;
            }
            best = best.max(self.ai_bombard_tile_value(&staged, x, y));
        }
        best
    }

    pub(super) fn aircraft_attack_path(&self, plane: &Unit) -> Option<AircraftAttackPath> {
        let max_output = self.aircraft_production_denominator(plane);
        self.aircraft_attack_path_with_output(plane, max_output)
    }

    fn aircraft_attack_path_with_output(
        &self,
        plane: &Unit,
        max_output: i32,
    ) -> Option<AircraftAttackPath> {
        let mut map = super::fuel_paths::FuelPaths::new(self, plane, true)?;
        let (_, war_mask, _) = self.pact_masks(plane.owner);
        let mut best = None;
        let mut best_value = 0.0;
        for (x, y, _) in map.reachable_within(self, plane.moves_left, true, false) {
            if !self.any_unit_at(x, y, |unit| {
                (unit.owner < 64 && ((war_mask >> unit.owner) & 1) != 0)
                    || (unit.owner >= 64 && self.at_war(plane.owner, unit.owner))
            }) {
                continue;
            }
            let value = self.aircraft_target_value_with_output(plane, x, y, max_output);
            if value > best_value {
                best_value = value;
                best = Some((x, y));
            }
        }
        let target = best?;
        Some((map.path(self, target.0, target.1)?, target, best_value))
    }

    pub(super) fn aircraft_best_target(&self, plane: &Unit) -> Option<(i32, i32, f64)> {
        if plane.kind.fuel_capacity() > 0 {
            return self
                .aircraft_attack_path(plane)
                .map(|(_, (x, y), value)| (x, y, value));
        }
        let targets: Vec<_> = self
            .units
            .iter()
            .filter(|u| self.at_war(plane.owner, u.owner) && u.transport.is_none())
            .collect();
        let destinations: Vec<_> = targets.iter().map(|u| (u.x, u.y)).collect();
        let route_costs = self.route_costs_to_many_within(plane, &destinations, plane.moves_left);
        targets
            .into_iter()
            .zip(route_costs)
            .filter_map(|(u, cost)| {
                cost?;
                let value = self.aircraft_target_value(plane, u.x, u.y);
                (value > 0.0).then_some((u.x, u.y, value))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
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

    pub(in crate::sim) fn source_hunter_launch(&mut self, hunter_id: u32, primary: u32) {
        let Some(hunter) = self.unit_by_id(hunter_id).cloned() else {
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
            let Some(missile) = self.unit_by_id(id).cloned() else {
                continue;
            };
            // Literal daihunter.c: the map describes the HUNTER and its raw
            // move cost is compared with missile MP divided by SINGLE_MOVE.
            let mut map = super::normal_paths::NormalPaths::enemy_map(self, &hunter);
            let mut target = None;
            // NormalPaths stores costs in movement fragments. Freeciv's
            // `missile->moves_left / SINGLE_MOVE` is therefore already the
            // corresponding fragment budget in CivCraft; do not divide a
            // second time before passing it to reachable_within().
            for (x, y, _) in map.reachable_within(self, missile.moves_left) {
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
                let target_owner = self
                    .tile_units_ordered(target.0, target.1)
                    .find(|unit| self.at_war(hunter.owner, unit.owner))
                    .map(|unit| unit.owner);
                let origin = (missile.x, missile.y);
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
                    if let Some(primary) = self.unit_by_id(primary)
                        && self
                            .map_wrap
                            .step_distance(live.x, live.y, primary.x, primary.y)
                            == 1
                        && let Some(direction) = EIGHT_WAY_DIRS.iter().position(|&(dx, dy)| {
                            self.map_wrap.eight_way_offset(live.x, live.y, dx, dy)
                                == Some((primary.x, primary.y))
                        })
                    {
                        self.try_move_unit(id, direction);
                    }
                }
                if self.trace.wants::<military_observation::AirMissionEvent>() {
                    let spent_or_moved = self.unit_by_id(id).is_none_or(|unit| {
                        (unit.x, unit.y) != origin || unit.moves_left < missile.moves_left
                    });
                    self.trace_air_mission(military_observation::AirMissionEvent {
                        owner: missile.owner,
                        unit_id: id,
                        kind: missile.kind.name(),
                        role: military_observation::military_unit_role(missile.kind),
                        mission: "missile_launch",
                        from: origin,
                        to: Some(target),
                        target_owner,
                        succeeded: spent_or_moved,
                    });
                }
                if !self.units.iter().any(|unit| unit.id == primary) {
                    return;
                }
            }
        }
    }

    pub(super) fn aircraft_repair_gain(&self, plane: &Unit, x: i32, y: i32) -> i32 {
        self.stationary_unit_hp_recovery_at(plane, x, y)
    }

    pub(super) fn aircraft_regen_turns(&self, plane: &Unit, x: i32, y: i32) -> i32 {
        let lost = plane.max_hit_points() - plane.hp;
        if lost <= 0 {
            return 0;
        }
        let gain = self.aircraft_repair_gain(plane, x, y);
        if gain <= 0 { i32::MAX } else { 1 + lost / gain }
    }

    pub(super) fn aircraft_city_needs_defender(
        &self,
        plane: &Unit,
        x: i32,
        y: i32,
        current: bool,
    ) -> bool {
        let Some(city) = self.city_at(x, y) else {
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

    pub(super) fn aircraft_refuel_route(
        &self,
        plane: &Unit,
        bases: &[(i32, i32)],
    ) -> Option<AircraftBaseRoute> {
        let mut map = super::fuel_paths::FuelPaths::new(self, plane, true)?;
        let mut best = None;
        let mut best_regen = i32::MAX;
        for (x, y, _) in map.reachable_within(self, plane.moves_left, false, true) {
            if !bases.contains(&(x, y)) {
                continue;
            }
            let turns = self.aircraft_regen_turns(plane, x, y);
            if plane.hp >= plane.max_hit_points() || turns <= 0 {
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

    /// Rebase a strike aircraft to a refueling site from which it can bombard
    /// a valuable hostile target, then use the normal bombard selector there.
    pub(super) fn aircraft_bombard_base_route(
        &self,
        plane: &Unit,
        bases: &[(i32, i32)],
    ) -> Option<AircraftBaseRoute> {
        let mut map = super::fuel_paths::FuelPaths::new(self, plane, true)?;
        let mut best: Option<(f64, i32, (i32, i32))> = None;
        for &(x, y) in bases {
            let Some(cost) = map.movement_cost(self, x, y) else {
                continue;
            };
            let carrier = self.units.iter().find(|ship| {
                (ship.x, ship.y) == (x, y)
                    && self.allied(plane.owner, ship.owner)
                    && self.ruleset_transport_pair_allowed_fail_closed(ship.kind, plane.kind)
            });
            let mut staged = plane.clone();
            staged.x = x;
            staged.y = y;
            staged.transport = carrier.map(|ship| ship.id);
            staged.moves_left = self.unit_move_rate(plane);
            let value = self
                .map_wrap
                .tiles_within_steps(x, y, Self::fixed_wing_bombard_range(plane.kind))
                .into_iter()
                .filter(|&(tx, ty, _)| self.can_bombard_tile(&staged, tx, ty))
                .map(|(tx, ty, _)| self.ai_bombard_tile_value(&staged, tx, ty))
                .fold(0.0_f64, f64::max);
            if value <= 0.0 {
                continue;
            }
            let score = value / (1.0 + f64::from(cost) / f64::from(self.unit_move_rate(plane)));
            if best
                .is_none_or(|(old, old_cost, _)| score > old || (score == old && cost < old_cost))
            {
                best = Some((score, cost, (x, y)));
            }
        }
        let (_, _, destination) = best?;
        Some((map.path(self, destination.0, destination.1)?, destination))
    }

    pub(super) fn strategic_airbase(
        &self,
        plane: &Unit,
        bases: &[(i32, i32)],
    ) -> Option<AircraftBaseRoute> {
        let lost = plane.max_hit_points() - plane.hp;
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
        virtual_plane.moves_left = self.unit_move_rate(plane);
        let mut worth = if defend {
            0.0
        } else {
            self.aircraft_best_target(&virtual_plane)
                .map_or(0.0, |t| t.2)
        };
        let mut map = super::fuel_paths::FuelPaths::new(self, plane, true)?;
        let mut best = None;
        for (x, y, _) in map.reachable_within(self, plane.moves_left, true, false) {
            if !bases.contains(&(x, y)) {
                continue;
            }
            let mut faster = false;
            if lost > 0 {
                let turns = self.aircraft_regen_turns(plane, x, y);
                if turns > minimum_regen {
                    continue;
                }
                if turns < minimum_regen {
                    minimum_regen = turns;
                    best = Some((x, y));
                    worth = 0.0;
                    faster = true;
                }
            }
            if self.aircraft_city_needs_defender(plane, x, y, false) {
                if lost <= 0 {
                    best = Some((x, y));
                    break;
                }
                if !defend {
                    best = Some((x, y));
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
                best = Some((x, y));
            }
        }
        let (x, y) = best?;
        Some((map.path(self, x, y)?, (x, y)))
    }

    pub(in crate::sim) fn run_ai_strike_aircraft(&mut self, aircraft: &Unit) {
        if aircraft.kind.is_fixed_wing_aircraft() {
            self.run_ai_aircraft(aircraft);
            return;
        }
        if matches!(aircraft.order, Some(UnitOrder::AirSuperiority { .. })) {
            return;
        }
        let position = (aircraft.x, aircraft.y);
        let on_base = self.refuel_site(aircraft.owner, position.0, position.1)
            || self.aboard_refueling_transport(aircraft);
        if on_base && self.run_ai_bombard(aircraft) {
            if self.trace.wants::<military_observation::AirMissionEvent>() {
                self.trace_air_mission(military_observation::AirMissionEvent {
                    owner: aircraft.owner,
                    unit_id: aircraft.id,
                    kind: aircraft.kind.name(),
                    role: military_observation::military_unit_role(aircraft.kind),
                    mission: "bombard",
                    from: position,
                    to: None,
                    target_owner: None,
                    succeeded: true,
                });
            }
            return;
        }
        if on_base && aircraft.fuel < aircraft.kind.fuel_capacity() {
            if self.trace.wants::<military_observation::AirMissionEvent>() {
                self.trace_air_mission(military_observation::AirMissionEvent {
                    owner: aircraft.owner,
                    unit_id: aircraft.id,
                    kind: aircraft.kind.name(),
                    role: military_observation::military_unit_role(aircraft.kind),
                    mission: "refuel_wait",
                    from: position,
                    to: None,
                    target_owner: None,
                    succeeded: true,
                });
            }
            return;
        }
        let attack = on_base
            .then(|| self.aircraft_attack_path(aircraft))
            .flatten();
        if on_base && attack.is_none() && self.ai_assign_air_superiority(aircraft) {
            if self.trace.wants::<military_observation::AirMissionEvent>() {
                self.trace_air_mission(military_observation::AirMissionEvent {
                    owner: aircraft.owner,
                    unit_id: aircraft.id,
                    kind: aircraft.kind.name(),
                    role: military_observation::military_unit_role(aircraft.kind),
                    mission: "air_superiority",
                    from: position,
                    to: Some(position),
                    target_owner: None,
                    succeeded: true,
                });
            }
            return;
        }
        let static_sites = self.planning.airfields(&self.tiles, self.map_wrap);
        let mut bases: Vec<_> = self
            .cities
            .iter()
            .filter(|c| self.allied(aircraft.owner, c.owner))
            .map(|c| (c.x, c.y))
            .chain(static_sites.iter().copied().filter(|&(x, y)| {
                let tile = self.tile(x, y);
                tile.known_by[aircraft.owner]
                    && matches!(tile.base, Some(Base::Airstrip | Base::Airbase))
            }))
            .filter(|&(x, y)| !self.any_unit_at(x, y, |u| !self.allied(aircraft.owner, u.owner)))
            .collect();
        drop(static_sites);
        bases.sort_unstable_by_key(|&(x, y)| self.map_wrap.index(x, y));
        bases.dedup();
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
        let before = aircraft.moves_left;
        let (mission_kind, route) = if on_base {
            if let Some((path, target, _)) = attack {
                ("air_strike", Some((path, target)))
            } else if let Some(route) = self.aircraft_bombard_base_route(aircraft, &bases) {
                ("rebase_bombard", Some(route))
            } else if let Some(route) = self.strategic_airbase(aircraft, &bases) {
                ("rebase_strategic", Some(route))
            } else {
                ("idle_on_base", None)
            }
        } else {
            let old_goal = self
                .unit_goto_tiles
                .get(&aircraft.id)
                .copied()
                .filter(|goal| *goal != position && bases.contains(goal));
            let route = old_goal
                .and_then(|goal| {
                    let mut map = super::fuel_paths::FuelPaths::new(self, aircraft, true)?;
                    Some((map.path(self, goal.0, goal.1)?, goal))
                })
                .or_else(|| self.aircraft_refuel_route(aircraft, &bases));
            ("refuel_return", route)
        };
        if let Some((path, destination)) = route {
            let target_owner = (mission_kind == "air_strike")
                .then(|| {
                    self.tile_units_ordered(destination.0, destination.1)
                        .find(|u| self.at_war(aircraft.owner, u.owner))
                        .map(|u| u.owner)
                })
                .flatten();
            if bases.contains(&destination) {
                self.unit_goto_tiles.insert(aircraft.id, destination);
            }
            let movement = self.follow_source_ai_path(aircraft.id, path);
            if movement.at_goal {
                self.unit_goto_tiles.remove(&aircraft.id);
            }
            if self.trace.wants::<military_observation::AirMissionEvent>() {
                self.trace_air_mission(military_observation::AirMissionEvent {
                    owner: aircraft.owner,
                    unit_id: aircraft.id,
                    kind: aircraft.kind.name(),
                    role: military_observation::military_unit_role(aircraft.kind),
                    mission: mission_kind,
                    from: position,
                    to: Some(destination),
                    target_owner,
                    succeeded: movement.at_goal
                        || self
                            .unit_by_id(aircraft.id)
                            .is_none_or(|live| live.moves_left != before),
                });
            }
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
        } else if self.trace.wants::<military_observation::AirMissionEvent>() {
            self.trace_air_mission(military_observation::AirMissionEvent {
                owner: aircraft.owner,
                unit_id: aircraft.id,
                kind: aircraft.kind.name(),
                role: military_observation::military_unit_role(aircraft.kind),
                mission: mission_kind,
                from: position,
                to: None,
                target_owner: None,
                succeeded: false,
            });
        }
    }

    /// Drives Civ III Conquests fixed-wing aircraft missions.
    fn run_ai_aircraft(&mut self, aircraft: &Unit) {
        let position = (aircraft.x, aircraft.y);
        let on_base = self.aircraft_on_base(aircraft);
        let at_war_any = (0..self.players.len())
            .any(|other| other != aircraft.owner && self.at_war(aircraft.owner, other));

        if !at_war_any {
            if matches!(aircraft.order, Some(UnitOrder::AirSuperiority { .. })) {
                self.cancel_unit_order(aircraft.id);
            }
            if on_base && aircraft.moves_left > 0 && aircraft.hp < aircraft.max_hit_points() {
                self.ai_rebase_aircraft(aircraft, false);
            }
            return;
        }

        if matches!(aircraft.order, Some(UnitOrder::AirSuperiority { .. })) {
            if on_base && self.ai_base_needs_cap(aircraft.owner, position) {
                return;
            }
            self.cancel_unit_order(aircraft.id);
        }

        if !on_base || aircraft.moves_left <= 0 {
            return;
        }

        // 1. Assign Air Superiority (CAP) on fighters defending threatened bases
        //    when enemies at war possess fixed-wing aircraft.
        if self.ai_assign_air_superiority(aircraft) {
            if self.trace.wants::<military_observation::AirMissionEvent>() {
                self.trace_air_mission(military_observation::AirMissionEvent {
                    owner: aircraft.owner,
                    unit_id: aircraft.id,
                    kind: aircraft.kind.name(),
                    role: military_observation::military_unit_role(aircraft.kind),
                    mission: "air_superiority",
                    from: position,
                    to: Some(position),
                    target_owner: None,
                    succeeded: true,
                });
            }
            return;
        }

        // 2. Bombard / Precision Bomb the best enemy target in operational range.
        if self.run_ai_bombard(aircraft) {
            if self.trace.wants::<military_observation::AirMissionEvent>() {
                self.trace_air_mission(military_observation::AirMissionEvent {
                    owner: aircraft.owner,
                    unit_id: aircraft.id,
                    kind: aircraft.kind.name(),
                    role: military_observation::military_unit_role(aircraft.kind),
                    mission: "bombard",
                    from: position,
                    to: None,
                    target_owner: None,
                    succeeded: true,
                });
            }
            return;
        }

        // 3. Rebase (within 3 * operational_range) to a forward base that puts
        //    valuable enemy targets in range, needs CAP, or repairs faster.
        if self.ai_rebase_aircraft(aircraft, true) {
            return;
        }

        // 4. Recon unexplored or fogged enemy territory within operational range.
        if self.ai_recon_aircraft(aircraft) {
            return;
        }

        if self.trace.wants::<military_observation::AirMissionEvent>() {
            self.trace_air_mission(military_observation::AirMissionEvent {
                owner: aircraft.owner,
                unit_id: aircraft.id,
                kind: aircraft.kind.name(),
                role: military_observation::military_unit_role(aircraft.kind),
                mission: "idle_on_base",
                from: position,
                to: None,
                target_owner: None,
                succeeded: false,
            });
        }
    }

    fn ai_base_needs_cap(&self, owner: usize, position: (i32, i32)) -> bool {
        if self.observed_enemy_air_threat(owner) == 0 {
            return false;
        }
        let recently_bombed = self.city_at(position.0, position.1).is_some_and(|city| {
            self.ai_city_bombed_turn
                .get(&city.id)
                .is_some_and(|&turn| self.turn - turn <= 10)
        });
        // Threat radius covers Bomber/Stealth Bomber operational range (10-16).
        let near_enemy_city = self.cities.iter().any(|city| {
            city.owner != owner
                && self.at_war(owner, city.owner)
                && self.tile(city.x, city.y).known_by[owner]
                && self
                    .map_wrap
                    .step_distance(position.0, position.1, city.x, city.y)
                    <= 16
        });
        let near_visible_enemy_air = self.units.iter().any(|enemy| {
            enemy.kind.is_fixed_wing_aircraft()
                && self.at_war(owner, enemy.owner)
                && self.unit_visible_to(owner, enemy)
                && self
                    .map_wrap
                    .step_distance(position.0, position.1, enemy.x, enemy.y)
                    <= enemy.kind.operational_range()
        });
        recently_bombed || near_enemy_city || near_visible_enemy_air
    }

    /// Civ III Conquests rebase mission.
    fn ai_rebase_aircraft(&mut self, aircraft: &Unit, at_war: bool) -> bool {
        let bases = self.rebaseable_bases_in_range(aircraft.id);
        if bases.is_empty() {
            return false;
        }
        let current_regen = self.aircraft_regen_turns(aircraft, aircraft.x, aircraft.y);
        let damaged = aircraft.hp < aircraft.max_hit_points();
        if damaged && current_regen <= 1 {
            return false;
        }
        let is_fighter = is_air_superiority_unit(aircraft.kind);
        let current_value = if at_war {
            self.best_bombard_value_from(aircraft, aircraft.x, aircraft.y)
        } else {
            0.0
        };
        let nearest_enemy_city = |x: i32, y: i32| {
            self.cities
                .iter()
                .filter(|c| {
                    c.owner != aircraft.owner
                        && self.at_war(aircraft.owner, c.owner)
                        && self.tile(c.x, c.y).known_by[aircraft.owner]
                })
                .map(|c| self.map_wrap.step_distance(x, y, c.x, c.y))
                .min()
        };
        let current_enemy_dist = nearest_enemy_city(aircraft.x, aircraft.y);

        let mut best: Option<(f64, i32, (i32, i32))> = None;
        for (bx, by) in bases {
            let mut score = 0.0;
            if damaged {
                let turns = self.aircraft_regen_turns(aircraft, bx, by);
                if turns < current_regen {
                    score += f64::from((current_regen - turns).min(5)) * 10.0;
                }
            }
            if at_war {
                let bombard_val = self.best_bombard_value_from(aircraft, bx, by);
                if (current_value <= 0.0 && bombard_val > 0.0) || bombard_val > current_value + 0.5
                {
                    score += bombard_val;
                }
                if is_fighter && self.ai_base_needs_cap(aircraft.owner, (bx, by)) {
                    let cap_there = self
                        .tile_units_ordered(bx, by)
                        .filter(|u| {
                            u.owner == aircraft.owner
                                && matches!(u.order, Some(UnitOrder::AirSuperiority { .. }))
                        })
                        .count();
                    if cap_there == 0 {
                        score += 12.0;
                    }
                }
                if score <= 0.0
                    && current_value <= 0.0
                    && let (Some(cur_d), Some(new_d)) =
                        (current_enemy_dist, nearest_enemy_city(bx, by))
                    && new_d + 2 < cur_d
                {
                    score += f64::from(cur_d - new_d) * 0.5;
                }
            }
            if score <= 0.0 {
                continue;
            }
            let dist = self.map_wrap.step_distance(aircraft.x, aircraft.y, bx, by);
            if best.is_none_or(|(old_score, old_dist, _)| {
                score > old_score || (score == old_score && dist < old_dist)
            }) {
                best = Some((score, dist, (bx, by)));
            }
        }
        let Some((_, _, (bx, by))) = best else {
            return false;
        };
        self.rebase_aircraft(aircraft.id, bx, by)
    }

    /// Civ III Conquests recon mission.
    fn ai_recon_aircraft(&mut self, aircraft: &Unit) -> bool {
        if !self.can_begin_recon(aircraft) {
            return false;
        }
        let range = aircraft.kind.operational_range();
        let enemy_cities: Vec<(i32, i32)> = self
            .cities
            .iter()
            .filter(|c| {
                c.owner != aircraft.owner
                    && self.at_war(aircraft.owner, c.owner)
                    && self.tile(c.x, c.y).known_by[aircraft.owner]
            })
            .map(|c| (c.x, c.y))
            .collect();
        let mut best: Option<(i32, i32, (i32, i32))> = None;
        for (x, y, dist) in self
            .map_wrap
            .tiles_within_steps(aircraft.x, aircraft.y, range)
        {
            if dist == 0 {
                continue;
            }
            let mut score = 0_i32;
            for (tx, ty, _) in self.map_wrap.tiles_within_steps(x, y, 1) {
                let tile = self.tile(tx, ty);
                if !tile.known_by[aircraft.owner] {
                    score += 3;
                } else if !self.has_main_sight_at(aircraft.owner, tx, ty) {
                    let enemy_owned = tile.owner.is_some_and(|owner| {
                        owner != aircraft.owner && self.at_war(aircraft.owner, owner)
                    });
                    let near_enemy_city = enemy_cities
                        .iter()
                        .any(|&(cx, cy)| self.map_wrap.step_distance(tx, ty, cx, cy) <= 3);
                    if enemy_owned || near_enemy_city {
                        score += 2;
                    }
                }
            }
            if score == 0 {
                continue;
            }
            if best.is_none_or(|(old_score, old_dist, _)| {
                score > old_score || (score == old_score && dist < old_dist)
            }) {
                best = Some((score, dist, (x, y)));
            }
        }
        let Some((_, _, (rx, ry))) = best else {
            return false;
        };
        self.recon_aircraft(aircraft.id, rx, ry)
    }

    fn ai_assign_air_superiority(&mut self, aircraft: &Unit) -> bool {
        if !self.players[aircraft.owner].ai || !self.can_set_air_superiority(aircraft) {
            return false;
        }
        if !self.ai_base_needs_cap(aircraft.owner, (aircraft.x, aircraft.y)) {
            return false;
        }
        // Keep roughly half of fighters at a threatened base on Air Superiority
        // (at least 1), leaving others free for bombard/recon/rebase sorties.
        let total_fighters = self
            .tile_units_ordered(aircraft.x, aircraft.y)
            .filter(|u| u.owner == aircraft.owner && is_air_superiority_unit(u.kind))
            .count();
        let cap_fighters = self
            .tile_units_ordered(aircraft.x, aircraft.y)
            .filter(|u| {
                u.owner == aircraft.owner
                    && is_air_superiority_unit(u.kind)
                    && matches!(u.order, Some(UnitOrder::AirSuperiority { .. }))
            })
            .count();
        let desired_cap = total_fighters.div_ceil(2).max(1);
        cap_fighters < desired_cap && self.start_air_superiority(aircraft.id)
    }
}
