//! Aircraft, fuel, airlift, and paradrop rules.

use super::*;

mod aircraft_ai;
mod paradrop;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectedBombardUnit {
    can_bombard: bool,
    kills_citizens: bool,
    attack: i32,
    bombard_rate: i32,
}

fn selected_unit_class_flag(kind: UnitKind, flag: &str) -> Option<bool> {
    let section = crate::sim::action_rules::freeciv_unit_type_section(kind)?;
    let rules = crate::rules::freeciv::RulesetData::campaign_default();
    let mut units = rules.content.iter().filter(|record| {
        record.kind == crate::rules::freeciv::ContentKind::Unit
            && record.section.eq_ignore_ascii_case(&section)
    });
    let unit = units.next()?;
    if units.next().is_some() {
        return None;
    }
    let class_name = unit.string_field("class").ok()??;
    let mut classes = rules.rule_records.iter().filter(|record| {
        record.namespace.eq_ignore_ascii_case("unit_class")
            && record.rule_name.eq_ignore_ascii_case(class_name)
    });
    let class = classes.next()?;
    if classes.next().is_some() {
        return None;
    }
    let flags = class.string_list_field("flags").ok()??;
    Some(
        flags
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(flag)),
    )
}

fn selected_bombard_unit(kind: UnitKind) -> Option<SelectedBombardUnit> {
    let section = crate::sim::action_rules::freeciv_unit_type_section(kind)?;
    let rules = crate::rules::freeciv::RulesetData::campaign_default();
    let mut records = rules.content.iter().filter(|record| {
        record.kind == crate::rules::freeciv::ContentKind::Unit
            && record.section.eq_ignore_ascii_case(&section)
    });
    let record = records.next()?;
    if records.next().is_some() {
        return None;
    }

    let flags = record.string_list_field("flags").ok()??;
    let attack = i32::try_from(record.integer_field("attack").ok()??).ok()?;
    let bombard_rate = match record.integer_field("bombard_rate").ok()? {
        Some(rate) => i32::try_from(rate).ok()?,
        None => 0,
    };
    if attack < 0 || bombard_rate < 0 {
        return None;
    }

    Some(SelectedBombardUnit {
        can_bombard: flags
            .iter()
            .any(|flag| flag.eq_ignore_ascii_case("Bombarder")),
        kills_citizens: selected_unit_class_flag(kind, "KillCitizen")?,
        attack,
        bombard_rate,
    })
}

fn selected_bombard_max_range() -> Option<i32> {
    let rules = crate::rules::freeciv::RulesetData::campaign_default();
    let mut parameters = rules.parameters.iter().filter(|parameter| {
        parameter.file.eq_ignore_ascii_case("actions.ruleset")
            && parameter.section.eq_ignore_ascii_case("actions")
            && parameter.key.eq_ignore_ascii_case("bombard_max_range")
    });
    let parameter = parameters.next()?;
    if parameters.next().is_some() {
        return None;
    }
    let range = i32::try_from(parameter.integer_value().ok()??).ok()?;
    (range >= 0).then_some(range)
}

fn selected_bombard_blocking_actions() -> Option<Vec<String>> {
    let rules = crate::rules::freeciv::RulesetData::campaign_default();
    let mut parameters = rules.parameters.iter().filter(|parameter| {
        parameter.file.eq_ignore_ascii_case("actions.ruleset")
            && parameter.section.eq_ignore_ascii_case("actions")
            && parameter.key.eq_ignore_ascii_case("bombard_blocked_by")
    });
    let parameter = parameters.next()?;
    if parameters.next().is_some() {
        return None;
    }
    let actions = parameter.string_list_value().ok()??;
    (!actions.is_empty()).then_some(actions)
}

impl Simulation {
    /// Whether this Civ III identity is explicitly mapped to a selected C2C3
    /// Bombarder with a valid Civ III bombard-strength content value.
    pub(super) fn has_selected_bombard_capability(&self, kind: UnitKind) -> bool {
        selected_bombard_unit(kind).is_some_and(|rules| {
            rules.can_bombard
                && rules.attack > 0
                && rules.bombard_rate > 0
                && kind.bombard_strength() > 0
        })
    }

    fn bombard_blocked_by_action(
        &self,
        actor: &Unit,
        x: i32,
        y: i32,
        blocked_by: &[String],
    ) -> bool {
        self.tile_units_ordered(x, y).any(|target| {
            blocked_by.iter().any(|action| {
                self.action_available_for_unit_against_unit(actor, action, target)
                    .unwrap_or(true)
            })
        })
    }

    pub(crate) fn can_begin_bombard(&self, unit: &Unit) -> bool {
        unit.owner == HUMAN
            && self.has_selected_bombard_capability(unit.kind)
            && unit.transport.is_none()
            && unit.moves_left > 0
    }

    fn bombard_action_available_at_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        let city_action = self.city_at(x, y).is_some_and(|city| {
            self.action_available_for_unit_against_city(unit, "Bombard", city)
                .unwrap_or(false)
        });
        let unit_action = self.tile_units_ordered(x, y).any(|target| {
            self.combat_target_reachable(unit.kind, target.kind, x, y)
                && self
                    .action_available_for_unit_against_unit(unit, "Bombard", target)
                    .unwrap_or(false)
        });
        city_action || unit_action
    }

    fn bombard_action_available_for_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.has_selected_bombard_capability(unit.kind)
            || unit.transport.is_some()
            || unit.moves_left < 1
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
            || (x, y) == (unit.x, unit.y)
            || selected_bombard_max_range()
                .is_none_or(|range| self.map_wrap.civ3_distance(unit.x, unit.y, x, y) > range)
        {
            return false;
        }
        let Some(blocked_by) = selected_bombard_blocking_actions() else {
            return false;
        };
        !self.bombard_blocked_by_action(unit, x, y, &blocked_by)
            && self.bombard_action_available_at_tile(unit, x, y)
    }

    pub(crate) fn can_bombard_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.can_begin_bombard(unit)
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
            || !self.tile(x, y).visible
        {
            return false;
        }
        self.bombard_action_available_for_tile(unit, x, y)
    }

    pub(super) fn try_ranged_bombard(&mut self, id: u32, x: i32, y: i32) -> MoveResult {
        let Some(attacker) = self.unit_by_id(id).cloned() else {
            return MoveResult::Blocked;
        };
        if !self.can_bombard_tile(&attacker, x, y) {
            return MoveResult::Blocked;
        }
        self.try_bombard(&attacker, x, y)
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
        if carrier.hp < carrier.max_hit_points()
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
                .and_then(|id| self.unit_by_id(id))
                .cloned()
            else {
                return (false, done);
            };
            if self
                .map_wrap
                .civ3_distance(carrier.x, carrier.y, boss.x, boss.y)
                > 1
            {
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

    pub(super) fn try_bombard(&mut self, attacker: &Unit, x: i32, y: i32) -> MoveResult {
        let Some(actor_rules) = selected_bombard_unit(attacker.kind) else {
            return MoveResult::Blocked;
        };
        if !self.bombard_action_available_for_tile(attacker, x, y) {
            return MoveResult::Blocked;
        }
        let mapped_bombard_strength = attacker.kind.bombard_strength();

        let targets: Vec<_> = self
            .tile_units_ordered(x, y)
            .filter(|target| {
                self.combat_target_reachable(attacker.kind, target.kind, x, y)
                    && self
                        .action_available_for_unit_against_unit(attacker, "Bombard", target)
                        .unwrap_or(false)
            })
            .cloned()
            .collect();
        let city_action = self.city_at(x, y).is_some_and(|city| {
            self.action_available_for_unit_against_city(attacker, "Bombard", city)
                .unwrap_or(false)
        });
        if targets.is_empty() && !city_action {
            return MoveResult::Blocked;
        }
        // The preserved Civ III unit record stores bombard strength separately
        // from its ordinary attack value (a Bomber has attack 0 and bombard
        // strength 12). Feed that explicit content mapping through the shared
        // selected-ruleset tired-attack/effect pipeline.
        let attack = self.attack_power_against_with_base_for_action(
            attacker,
            x,
            y,
            mapped_bombard_strength,
            "Bombard",
        );
        let base_rate = actor_rules.bombard_rate;
        let rate = if crate::sim::combat_rules::selected().damage_reduces_bombard_rate {
            (base_rate * attacker.hp / attacker.max_hit_points()).max(1)
        } else {
            base_rate
        };
        let mut total_damage = 0;
        let mut observed_targets = Vec::new();

        // Freeciv's unit_bombard() walks every unit on the target tile and
        // fires one rate of attacks at each reachable defender. ACTION_BOMBARD
        // is marked non-lethal by the pinned engine's ruleset loader.
        for target in targets {
            let defense = self.defense_power_against(&target, x, y, attacker.kind);
            let (attack_firepower, _) = self.combat_firepower(attacker, &target);
            let mut hp = target.hp;
            let mut hp_steps = vec![(attacker.hp, hp)];
            for _ in 0..rate {
                if self.rand_below(attack + defense) < attack {
                    hp = (hp - attack_firepower).max(1);
                    hp_steps.push((attacker.hp, hp));
                }
            }
            total_damage += target.hp - hp;
            if let Some(defender) = self.units.iter_mut().find(|unit| unit.id == target.id) {
                defender.hp = hp;
            }
            if self.visual_events.enabled {
                self.publish_visual_event(VisualEvent::Combat(Box::new(
                    presentation::CombatVisual {
                        attacker: attacker.clone(),
                        defender: target.clone(),
                        hp: hp_steps,
                        city: self.city_at(x, y).cloned(),
                        bombard_effects: true,
                        territory: (0..self.map_wrap.width() * self.map_wrap.height())
                            .map(|i| {
                                self.displayed_tile_owner(
                                    i % self.map_wrap.width(),
                                    i / self.map_wrap.width(),
                                )
                            })
                            .collect(),
                    },
                )));
            }
            observed_targets.push((target.kind, target.owner));
        }

        // Freeciv calls unit_attack_civilian_casualties once after all unit
        // targets have been processed, including bombardments on empty city
        // tiles. C2C3's selected unit class and city-effect adapter determine
        // whether a citizen can be lost (Big Land KillCitizen, walls, and
        // Great Wall protection).
        if actor_rules.kills_citizens && self.city_at(x, y).is_some() {
            self.apply_bombard_population_loss(attacker, x, y);
        }

        if let Some(bomber) = self.units.iter_mut().find(|unit| unit.id == attacker.id) {
            bomber.moves_left = 0;
            bomber.moved = true;
            bomber.state = UnitState::Idle;
            bomber.work_left = 0;
        }
        if attacker.owner == HUMAN || observed_targets.iter().any(|(_, owner)| *owner == HUMAN) {
            if observed_targets.is_empty() {
                if let Some(city) = self.city_at(x, y) {
                    self.add_event(format!(
                        "A {} bombarded {}.",
                        attacker.kind.name(),
                        city.name
                    ));
                }
            } else {
                let names = observed_targets
                    .iter()
                    .map(|(kind, _)| kind.name())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.add_event(format!(
                    "A {} dealt {total_damage} damage to {names}.",
                    attacker.kind.name(),
                ));
            }
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
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return;
        };
        let max_fuel = unit.kind.fuel_capacity();
        let site = self.refuel_site(unit.owner, unit.x, unit.y);
        let refueled = site || self.aboard_refueling_transport(&unit);
        // player_restore_units calls unit_gain_hitpoints before refueling.
        // The same ruleset effects now drive aircraft and other unit recovery.
        let hp = unit.hp + self.unit_hitpoint_gain_at(&unit, unit.x, unit.y);
        if let Some(live) = self.units.iter_mut().find(|actor| actor.id == id) {
            live.hp = hp;
            live.moved = false;
            live.paradropped = false;
            if refueled {
                live.fuel = max_fuel;
            }
        }
        if hp <= 0 {
            self.remove_units_with_cargo(vec![id]);
            if unit.owner == HUMAN {
                self.add_event(format!("Your {} crashed.", unit.kind.name()));
            }
            return;
        }
        if !refueled
            && max_fuel > 0
            && unit.fuel <= 1
            && self.boarding_target(&unit, unit.x, unit.y).is_some()
        {
            self.board_or_deboard(id);
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .fuel = max_fuel;
            return;
        }
        if !refueled
            && max_fuel > 0
            && unit.fuel <= 1
            && unit.moves_left > 0
            && let Some(mut map) =
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
                if let Some(live) = self.unit_by_id(id).cloned() {
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

        let Some(index) = self.unit_index_by_id(id) else {
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
#[path = "aviation/tests.rs"]
mod tests;
