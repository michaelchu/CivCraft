//! Paratrooper launch and landing rules.

use super::*;

fn selected_paratroopers_range(kind: UnitKind) -> Option<i32> {
    crate::sim::action_rules::unit_paratroopers_range(kind)
}

/// `airlift_from_always_enabled = FALSE`, `airlift_to_always_enabled = TRUE`:
/// a destination city can always receive an airlift, while a source city
/// needs airlift capacity.
const fn airlift_capacity_always_enabled(source: bool) -> bool {
    !source
}

impl Simulation {
    fn paradrop_tile_is_native(&self, kind: UnitKind, tile: &Tile) -> bool {
        self.pf_native_tile_data(kind, tile)
    }

    fn selected_paradrop_action(
        &self,
        unit: &Unit,
        target: (i32, i32),
        target_state: &Tile,
    ) -> Option<&'static str> {
        ["Paradrop Unit Enter", "Paradrop Unit Enter Conquer"]
            .into_iter()
            .find(|action| {
                self.action_available_for_unit_with_target_state_fail_closed(
                    unit,
                    action,
                    target,
                    target_state,
                )
            })
    }

    fn airlift_capacity(&self, city: &City) -> Option<i64> {
        let context = crate::sim::action_rules::SimulationRequirementContext::for_city_at(
            self,
            city,
            Some((city.x, city.y)),
        );
        crate::rules::engine_rules::effect_value(
            &crate::rules::engine_rules::RulesetData::campaign_default().effects,
            EffectKind::Airlift,
            &context,
        )
        .ok()
    }

    fn airlift_capacity_available(&self, city: &City, source: bool) -> bool {
        airlift_capacity_always_enabled(source)
            || (!city.airlift_used && self.airlift_capacity(city).is_some_and(|value| value > 0))
    }

    fn airlift_origin(&self, unit: &Unit, destination: (i32, i32)) -> Option<&City> {
        // Airlifts have no range limit (`airlift_max_range = "unlimited"`).
        if unit.transport.is_some()
            || !self.pf_native_tile(unit.kind, destination.0, destination.1)
            || !self.action_available_for_unit_fail_closed(unit, "Airlift Unit", destination)
        {
            return None;
        }
        self.cities.iter().find(|city| {
            (city.x, city.y) == (unit.x, unit.y)
                && city.owner == unit.owner
                && self.airlift_capacity_available(city, true)
        })
    }

    pub(crate) fn can_airlift(&self, unit: &Unit, destination: &City) -> bool {
        destination.owner == unit.owner
            && self.airlift_capacity_available(destination, false)
            && self
                .airlift_origin(unit, (destination.x, destination.y))
                .is_some_and(|source| source.id != destination.id)
    }

    pub(in crate::sim) fn airlift(&mut self, id: u32, destination: u32) -> bool {
        let Some(index) = self.unit_index_by_id(id) else {
            return false;
        };
        let unit = &self.units[index];
        let Some(city) = self.cities.iter().find(|city| {
            city.id == destination
                && city.owner == unit.owner
                && self.airlift_capacity_available(city, false)
        }) else {
            return false;
        };
        let destination_xy = (city.x, city.y);
        let Some(source) = self
            .airlift_origin(unit, destination_xy)
            .map(|city| city.id)
        else {
            return false;
        };
        if source == destination {
            return false;
        }
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
        !unit.paradropped
            && unit.work_left == 0
            && unit.transport.is_none()
            && (self.action_available_for_unit_fail_closed(
                unit,
                "Paradrop Unit Enter",
                (unit.x, unit.y),
            ) || self.action_available_for_unit_fail_closed(
                unit,
                "Paradrop Unit Enter Conquer",
                (unit.x, unit.y),
            ))
    }

    fn paradrop_tile_legal(&self, unit: &Unit, x: i32, y: i32) -> bool {
        let Some(range) = selected_paratroopers_range(unit.kind) else {
            return false;
        };
        let distance = self.map_distance(unit.x, unit.y, x, y);
        if !(0..self.map_wrap.height()).contains(&y) || distance < 1 || distance > range {
            return false;
        }
        let Some(target_state) = self.known_tile_for(unit.owner, x, y) else {
            return false;
        };
        (self.observed_city_owner(unit.owner, x, y).is_some()
            || self.paradrop_tile_is_native(unit.kind, target_state))
            && self
                .selected_paradrop_action(unit, (x, y), target_state)
                .is_some()
    }

    pub(in crate::sim) fn run_ai_paratrooper(&mut self, unit: &Unit) -> bool {
        // daiparadrop.c first clears immediate threats and opportunistic
        // attacks before considering a jump.
        if !self.ai_military_rampage(unit.id, 1, 99999) {
            return true;
        }
        let Some(unit) = self.unit_by_id(unit.id).cloned() else {
            return true;
        };
        if !self.can_begin_paradrop(&unit) || unit.hp * 2 < unit.max_hit_points() {
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
        let cities = self.cities.iter().filter(|city| {
            self.paradrop_tile_legal(&unit, city.x, city.y) && empty(city.x, city.y)
        });
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
            return self.ai_paradrop_and_rampage(unit.id, city.x, city.y);
        }
        let labels = self.continent_labels();
        let origin = labels[self.map_wrap.index(unit.x, unit.y)];
        let conquest = cities
            .filter(|city| self.at_war(owner, city.owner))
            .max_by_key(|city| {
                let overseas = labels[self.map_wrap.index(city.x, city.y)] != origin;
                (city.size + i32::from(overseas), std::cmp::Reverse(city.id))
            });
        if let Some(city) = conquest {
            return self.ai_paradrop_and_rampage(unit.id, city.x, city.y);
        }

        let mut best = None;
        let mut best_value = 0.0;
        let range = selected_paratroopers_range(unit.kind).unwrap_or(0);
        let mut candidates = self.map_wrap.tiles_within_steps(unit.x, unit.y, range);
        candidates.sort_unstable_by_key(|&(x, y, _)| self.map_wrap.index(x, y));
        for (x, y, _) in candidates {
            if !empty(x, y)
                || !self.paradrop_tile_legal(&unit, x, y)
                || self
                    .cities
                    .iter()
                    .any(|city| (city.x, city.y) == (x, y) && !self.allied(owner, city.owner))
            {
                continue;
            }
            let mut staged = unit.clone();
            (staged.x, staged.y) = (x, y);
            for (dx, dy) in CIV3_DIRS {
                let Some((tx, ty)) = self.map_wrap.eight_way_offset(x, y, dx, dy) else {
                    continue;
                };
                if !self.tile(tx, ty).known_by[owner]
                    || self.tile(tx, ty).terrain.is_water()
                    || self.treaty_blocks_move(&staged, tx, ty)
                {
                    continue;
                }
                let Some(defender) = self
                    .best_defender_id(&staged, tx, ty)
                    .and_then(|id| self.unit_by_id(id))
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
                    best = Some((x, y));
                }
            }
        }
        if let Some((x, y)) = best
            && self.paradrop(unit.id, x, y)
        {
            self.ai_military_rampage(unit.id, 1, 1);
            return true;
        }
        false
    }

    fn ai_paradrop_and_rampage(&mut self, id: u32, x: i32, y: i32) -> bool {
        if self.paradrop(id, x, y) {
            if self.unit_by_id(id).is_some() {
                self.ai_military_rampage(id, 1, 1);
            }
            true
        } else {
            false
        }
    }

    pub(in crate::sim) fn paradrop(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return false;
        };
        if !self.can_begin_paradrop(&unit) {
            return false;
        }
        let x = x.rem_euclid(self.map_wrap.width());
        if !self.paradrop_tile_legal(&unit, x, y) {
            if unit.owner == HUMAN {
                self.add_event("Paratroopers need a known native tile and permission to enter.");
            }
            return false;
        }
        let Some(known_tile) = self.known_tile_for(unit.owner, x, y) else {
            return false;
        };
        let Some(action) = self.selected_paradrop_action(&unit, (x, y), known_tile) else {
            return false;
        };

        // Freeciv first rejects a known non-native player-map tile, then
        // resolves the actual tile. A remembered land tile can have changed
        // to water since it was seen; that attempt is accepted and the unit
        // is lost after the actual-tile check.
        if !self.pf_native_tile(unit.kind, x, y) {
            self.show_map_circle_to_player(unit.owner, x, y, unit.kind);
            if unit.owner == HUMAN {
                self.add_event(format!(
                    "Your {} paradropped into the {} and was lost.",
                    unit.kind.name(),
                    self.tile(x, y).terrain.name()
                ));
            }
            self.remove_units_with_cargo(vec![id]);
            self.refresh_visibility();
            self.check_game_over();
            if self.trace.wants::<military_observation::AirMissionEvent>() {
                self.trace_air_mission(military_observation::AirMissionEvent {
                    owner: unit.owner,
                    unit_id: unit.id,
                    kind: unit.kind.name(),
                    role: military_observation::military_unit_role(unit.kind),
                    mission: "paradrop",
                    from: (unit.x, unit.y),
                    to: Some((x, y)),
                    target_owner: None,
                    succeeded: false,
                });
            }
            return true;
        }

        let city_owner = self.city_at(x, y).map(|city| city.owner);
        let occupiers: Vec<_> = self
            .units
            .iter()
            .filter(|enemy| (enemy.x, enemy.y) == (x, y) && !self.allied(unit.owner, enemy.owner))
            .cloned()
            .collect();
        let hostile_city = city_owner.is_some_and(|owner| !self.allied(unit.owner, owner));
        if (hostile_city && action != "Paradrop Unit Enter Conquer") || !occupiers.is_empty() {
            self.show_map_circle_to_player(unit.owner, x, y, unit.kind);
            let mut main_victim = None;
            let mut secondary_victim = None;
            if let Some(owner) = city_owner.filter(|&owner| !self.allied(unit.owner, owner)) {
                if self.at_war(unit.owner, owner) {
                    secondary_victim = Some(owner);
                } else {
                    main_victim = Some(owner);
                }
            }
            if main_victim.is_none() {
                for enemy in &occupiers {
                    if !self.at_war(unit.owner, enemy.owner) {
                        main_victim = Some(enemy.owner);
                        break;
                    }
                    secondary_victim.get_or_insert(enemy.owner);
                }
            }
            let victim = main_victim.or(secondary_victim);
            self.refresh_diplomatic_contact_at(unit.owner, x, y);
            self.action_caught_casus_belli(&unit, action, victim, (x, y));
            if unit.owner == HUMAN {
                self.add_event("Paratroopers were lost landing on enemy units.");
            }
            self.remove_units_with_cargo(vec![id]);
            self.refresh_visibility();
            self.check_game_over();
            if self.trace.wants::<military_observation::AirMissionEvent>() {
                self.trace_air_mission(military_observation::AirMissionEvent {
                    owner: unit.owner,
                    unit_id: unit.id,
                    kind: unit.kind.name(),
                    role: military_observation::military_unit_role(unit.kind),
                    mission: "paradrop",
                    from: (unit.x, unit.y),
                    to: Some((x, y)),
                    target_owner: victim,
                    succeeded: false,
                });
            }
            return true;
        }

        if let Some(paratroopers) = self.units.iter_mut().find(|unit| unit.id == id) {
            paratroopers.paradropped = true;
        }
        let actual_owner = self.tile(x, y).owner;
        // The drop costs no movement, but Freeciv still runs the movement
        // callback before applying action-success consequences.
        let conquest_action = (action == "Paradrop Unit Enter Conquer").then_some(action);
        if !self.enter_tile_with_conquest_action(id, x, y, 0, None, conquest_action) {
            return false;
        }
        self.action_success_casus_belli(&unit, action, actual_owner, (x, y));
        self.refresh_visibility();
        self.check_game_over();
        if self.trace.wants::<military_observation::AirMissionEvent>() {
            self.trace_air_mission(military_observation::AirMissionEvent {
                owner: unit.owner,
                unit_id: unit.id,
                kind: unit.kind.name(),
                role: military_observation::military_unit_role(unit.kind),
                mission: "paradrop",
                from: (unit.x, unit.y),
                to: Some((x, y)),
                target_owner: city_owner.or(actual_owner),
                succeeded: true,
            });
        }
        true
    }
}
