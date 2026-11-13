//! Paratrooper launch and landing rules.

use super::*;

#[derive(Clone, Copy)]
enum SelectedActionRange {
    Unlimited,
    Limited(i32),
}

fn selected_unit_record(kind: UnitKind) -> Option<&'static crate::rules::freeciv::ContentRecord> {
    let section = crate::sim::action_rules::freeciv_unit_type_section(kind)?;
    let rules = crate::rules::freeciv::RulesetData::campaign_default();
    let mut records = rules.content.iter().filter(|record| {
        record.kind == crate::rules::freeciv::ContentKind::Unit
            && record.section.eq_ignore_ascii_case(&section)
    });
    let record = records.next()?;
    records.next().is_none().then_some(record)
}

fn selected_paratroopers_range(kind: UnitKind) -> Option<i32> {
    let range = i32::try_from(
        selected_unit_record(kind)?
            .integer_field("paratroopers_range")
            .ok()??,
    )
    .ok()?;
    (range >= 0).then_some(range)
}

fn selected_airlift_max_range() -> Option<SelectedActionRange> {
    let rules = crate::rules::freeciv::RulesetData::campaign_default();
    let mut parameters = rules.parameters.iter().filter(|parameter| {
        parameter.file.eq_ignore_ascii_case("actions.ruleset")
            && parameter.section.eq_ignore_ascii_case("actions")
            && parameter.key.eq_ignore_ascii_case("airlift_max_range")
    });
    let parameter = parameters.next()?;
    if parameters.next().is_some() {
        return None;
    }
    if let Ok(Some(value)) = parameter.string_value() {
        return if value.eq_ignore_ascii_case("unlimited") {
            Some(SelectedActionRange::Unlimited)
        } else {
            let range = value.parse::<i32>().ok()?;
            (range >= 0).then_some(SelectedActionRange::Limited(range))
        };
    }
    let range = i32::try_from(parameter.integer_value().ok()??).ok()?;
    (range >= 0).then_some(SelectedActionRange::Limited(range))
}

fn selected_airlift_capacity_always_enabled(source: bool) -> Option<bool> {
    let key = if source {
        "airlift_from_always_enabled"
    } else {
        "airlift_to_always_enabled"
    };
    let rules = crate::rules::freeciv::RulesetData::campaign_default();
    let mut parameters = rules.parameters.iter().filter(|parameter| {
        parameter.file.eq_ignore_ascii_case("game.ruleset")
            && parameter.key.eq_ignore_ascii_case(key)
    });
    let parameter = parameters.next()?;
    if parameters.next().is_some() {
        return None;
    }
    parameter.boolean_value().ok()?
}

impl Simulation {
    pub(crate) fn known_target_tile(&self, owner: usize, x: i32, y: i32) -> Option<Tile> {
        let index = self.map_wrap.index(x, y);
        let live = self.tile(x, y);
        if !live.known_by[owner] {
            return None;
        }
        if owner == HUMAN {
            if live.visible {
                return Some(live.clone());
            }
            if live.seen {
                return self.remembered_tile(owner, x, y).cloned();
            }
            return None;
        }

        let observation = self.ai_map_memory.get(&(owner, index))?;
        let mut tile = live.clone();
        tile.terrain = observation.route.terrain;
        tile.road = observation.route.road;
        tile.railroad = observation.route.railroad;
        tile.river = observation.route.river;
        tile.base = None;
        tile.owner = observation.owner;
        Some(tile)
    }

    pub(crate) fn known_target_is_city(&self, owner: usize, x: i32, y: i32) -> bool {
        if owner == HUMAN {
            self.render_cities()
                .iter()
                .any(|city| (city.city.x, city.city.y) == (x, y))
        } else {
            self.ai_map_memory
                .get(&(owner, self.map_wrap.index(x, y)))
                .is_some_and(|observation| observation.route.city_owner.is_some())
        }
    }

    fn paradrop_tile_is_native(&self, kind: UnitKind, tile: &Tile) -> bool {
        let rules = crate::sim::movement_rules::MovementRules::campaign();
        let class = rules
            .unit_class_for_kind(kind)
            .expect("campaign C2C3 unit class binding");
        match rules.tile_native_to_class(
            tile.terrain,
            tile.road,
            tile.railroad,
            tile.river,
            tile.base,
            class,
        ) {
            Ok(native) => native,
            Err(crate::sim::movement_rules::MovementRulesError::UnsupportedTerrain(_)) => false,
            Err(error) => panic!("campaign C2C3 native-tile data is invalid: {error:?}"),
        }
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
        crate::rules::freeciv::effect_value(
            &crate::rules::freeciv::RulesetData::campaign_default().effects,
            "Airlift",
            &context,
        )
        .ok()
    }

    fn airlift_capacity_available(&self, city: &City, source: bool) -> bool {
        let Some(always_enabled) = selected_airlift_capacity_always_enabled(source) else {
            return false;
        };
        always_enabled
            || (!city.airlift_used && self.airlift_capacity(city).is_some_and(|value| value > 0))
    }

    fn airlift_origin(&self, unit: &Unit, destination: (i32, i32)) -> Option<&City> {
        let max_range = selected_airlift_max_range()?;
        let distance = self.map_distance(unit.x, unit.y, destination.0, destination.1);
        if unit.transport.is_some()
            || matches!(max_range, SelectedActionRange::Limited(range) if distance > range)
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
        let Some(target_state) = self.known_target_tile(unit.owner, x, y) else {
            return false;
        };
        (self.known_target_is_city(unit.owner, x, y)
            || self.paradrop_tile_is_native(unit.kind, &target_state))
            && self
                .selected_paradrop_action(unit, (x, y), &target_state)
                .is_some()
    }

    pub(in crate::sim) fn run_ai_paratrooper(&mut self, unit: &Unit) -> bool {
        if !self.can_begin_paradrop(unit) || unit.hp * 2 < unit.max_hit_points() {
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
        let origin = labels[self.map_wrap.index(unit.x, unit.y)];
        let conquest = cities
            .filter(|city| self.at_war(owner, city.owner))
            .max_by_key(|city| {
                let overseas = labels[self.map_wrap.index(city.x, city.y)] != origin;
                (city.size + i32::from(overseas), std::cmp::Reverse(city.id))
            });
        if let Some(city) = conquest {
            return self.paradrop(unit.id, city.x, city.y);
        }

        let mut best = None;
        let mut best_value = 0.0;
        for (index, _) in self.tiles.iter().enumerate() {
            let (x, y) = self.map_wrap.position(index);
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
            for (direction, (dx, dy)) in CIV3_DIRS.into_iter().enumerate() {
                let Some((tx, ty)) = self.map_wrap.civ3_offset(x, y, dx, dy) else {
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
                    best = Some((x, y, direction));
                }
            }
        }
        if let Some((x, y, direction)) = best
            && self.paradrop(unit.id, x, y)
        {
            self.try_move_unit_civ3(unit.id, direction);
            return true;
        }
        false
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
        let Some(known_tile) = self.known_target_tile(unit.owner, x, y) else {
            return false;
        };
        let Some(action) = self.selected_paradrop_action(&unit, (x, y), &known_tile) else {
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
            return true;
        }

        if let Some(paratroopers) = self.units.iter_mut().find(|unit| unit.id == id) {
            paratroopers.paradropped = true;
        }
        let actual_owner = self.tile(x, y).owner;
        // The drop costs no movement, but Freeciv still runs the movement
        // callback before applying action-success consequences.
        self.enter_tile(id, x, y, 0, None);
        self.action_success_casus_belli(&unit, action, actual_owner, (x, y));
        self.refresh_visibility();
        self.check_game_over();
        true
    }
}
