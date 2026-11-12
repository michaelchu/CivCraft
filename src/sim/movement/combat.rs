//! Combat outcome calculation and the state changes caused by attacks.

use super::*;

impl Simulation {
    fn try_enslave_defeated_unit(&mut self, attacker: &Unit, defender: &Unit, x: i32, y: i32) {
        let result_index = attacker
            .kind
            .conquests_rule()
            .enslave_result_prototype_index;
        if result_index < 0 {
            return;
        }
        let Some(result_rule) = crate::sim::civ3_unit_rules::get_by_prototype_index(result_index)
        else {
            return;
        };
        let Some(result_kind) = UnitKind::from_conquests_id(result_rule.id) else {
            return;
        };
        if self.rand_below(3) != 0 {
            return;
        }

        let created_id = self.spawn_unit(attacker.owner, result_kind, x, y);
        let created = {
            let created = self
                .units
                .iter_mut()
                .find(|unit| unit.id == created_id)
                .expect("new enslaved unit exists");
            // The new unit serves the victor while retaining the defeated
            // unit's nationality, matching Civ III's captured-unit rule.
            created.nationality = defender.nationality;
            created.clone()
        };
        self.publish_visual_event(VisualEvent::Capture { unit: created });
        if attacker.owner == HUMAN {
            self.add_event(format!(
                "Your {} enslaved the enemy, creating a new {}!",
                attacker.kind.name(),
                result_kind.name()
            ));
        }
    }

    pub(crate) fn attack_power(unit: &Unit) -> i32 {
        let stats = unit.kind.stats();
        let mut power = percent_of(stats.attack * POWER_FACTOR, unit.veteran_power());
        if unit.moves_left < MOVE_FRAGMENTS {
            power = power * unit.moves_left.max(0) / MOVE_FRAGMENTS;
        }
        power
    }

    #[cfg(test)]
    pub(crate) fn defense_power(&self, unit: &Unit, x: i32, y: i32) -> i32 {
        self.defense_power_against(unit, x, y, unit.kind)
    }

    pub(in crate::sim) fn defense_power_against(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
        attacker: UnitKind,
    ) -> i32 {
        self.defense_power_with_building(unit, x, y, attacker, None)
    }

    pub(in crate::sim) fn defense_power_with_building(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
        attacker: UnitKind,
        building: Option<Building>,
    ) -> i32 {
        // Freeciv applies these as separate integer-multiplication stages:
        // terrain in get_defense_power, then Defend_Bonus, then
        // Fortify_Defense_Bonus in defense_multiplication.
        let mut power = percent_of(
            unit.kind.stats().defense * POWER_FACTOR,
            unit.veteran_power(),
        );
        if unit.kind.has_terrain_defense() {
            power = apply_percent_bonus(power, self.tile(x, y).terrain.defense_bonus());
        }

        let city = self.city_at(x, y);
        let base = self.tile(x, y).base;
        let aerial_attack = attacker.is_aircraft();
        let naval_attack = attacker.is_naval();
        if unit.kind == UnitKind::AegisCruiser && aerial_attack && !attacker.is_nuclear_weapon() {
            power *= 5;
        }
        if unit.kind == UnitKind::Destroyer && attacker == UnitKind::Submarine {
            power *= 2;
        }
        let ground_attack = !aerial_attack && !naval_attack;
        let mut defend_bonus = if aerial_attack {
            match base {
                Some(Base::Airstrip) => 50,
                Some(Base::Airbase) => 100,
                Some(Base::Fortress) => 50,
                _ => 0,
            }
        } else {
            base.map_or(0, Base::defense_bonus)
        };
        if let Some(city) = city {
            let has = |candidate| building == Some(candidate) || city.has_building(candidate);
            if ground_attack {
                defend_bonus += CITY_DEFEND_BONUS;
                if has(Building::CITY_WALLS) {
                    defend_bonus += 100;
                }
                if self.has_active_wonder(unit.owner, Building::GREAT_WALL) {
                    defend_bonus += 50;
                }
            }
            if !aerial_attack && city.size >= LARGE_CITY_SIZE {
                defend_bonus += LARGE_CITY_DEFEND_BONUS;
            }
            if naval_attack && has(Building::COASTAL_DEFENSE) {
                defend_bonus += 100;
            }
            if aerial_attack && has(Building::SAM_BATTERY) {
                defend_bonus += 100;
            }
        }
        power = apply_percent_bonus(power, defend_bonus);
        // combat.c:defense_multiplication applies the extra attribute after
        // defense effects and before fortification. River is native only
        // to its configured classes; it is separate from terrain defense.
        power += power * self.pf_extra_class_defense(unit.kind, x, y) / 100;

        // Civ2Civ3 gives CanFortify units without the Cant_Fortify type flag
        // the same 50% city stage even while they are not explicitly fortified.
        let fortify_bonus =
            if unit.state == UnitState::Fortified || (city.is_some() && unit.kind.can_fortify()) {
                FORTIFY_DEFENSE_BONUS
            } else {
                0
            };
        apply_percent_bonus(power, fortify_bonus)
    }

    pub(in crate::sim) fn combat_firepower(&self, attacker: &Unit, defender: &Unit) -> (i32, i32) {
        let mut attack = attacker.kind.stats().firepower;
        let defense = defender.kind.stats().firepower;
        let in_city = self
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (defender.x, defender.y));
        if in_city && attacker.kind.is_city_buster() {
            attack *= 2;
        }
        if in_city && defender.kind.is_naval() {
            (attack * 2, 1)
        } else {
            (attack, defense)
        }
    }

    pub(in crate::sim) fn defender_rank(
        &self,
        attacker: &Unit,
        defender: &Unit,
        x: i32,
        y: i32,
    ) -> (i32, i32, i32) {
        let attack_power = Self::attack_power(attacker);
        let defense_power = self.defense_power_against(defender, x, y, attacker.kind);
        let defender_stats = defender.kind.stats();
        let (attack_firepower, defense_firepower) = self.combat_firepower(attacker, defender);
        let attacker_win = combat_win_chance(
            attack_power,
            attacker.hp,
            attack_firepower,
            defense_power,
            defender.hp,
            defense_firepower,
        );

        // get_defender truncates the defender's win chance to five decimal
        // places, then prefers the cheaper unit and finally the higher
        // defense rating when those values tie.
        let defender_value = (100_000.0 * (1.0 - attacker_win)) as i32;
        let rounds_survived = (defender.hp + attack_firepower - 1) / attack_firepower;
        let defense_rating = defense_power * rounds_survived * defense_firepower;
        (defender_value, -defender_stats.cost, defense_rating)
    }

    pub(in crate::sim) fn best_defender_id(&self, attacker: &Unit, x: i32, y: i32) -> Option<u32> {
        let mut best = None;
        let mut best_rank = None;
        let attacker_rule = attacker.kind.conquests_rule();
        for defender in self.tile_units_ordered(x, y).filter(|unit| {
            self.at_war(unit.owner, attacker.owner)
                && unit.transport.is_none()
                && attacker_rule.can_stealth_target(unit.kind.conquests_rule())
                && (!unit.kind.is_aircraft()
                    || attacker.kind.can_attack_aircraft()
                    || (!attacker.kind.is_aircraft() && self.refuel_site(unit.owner, x, y)))
        }) {
            let rank = self.defender_rank(attacker, defender, x, y);
            if best_rank.is_none() || best_rank.is_some_and(|current| rank > current) {
                best = Some(defender.id);
                best_rank = Some(rank);
            }
        }
        best
    }

    pub(in crate::sim) fn stack_vulnerable(&self, x: i32, y: i32) -> bool {
        // combat.c:is_stack_vulnerable, default killstack=TRUE. None of the
        // supplied Civ2Civ3 extras carry NoStackDeath, including bases.
        !self.cities.iter().any(|city| (city.x, city.y) == (x, y))
    }

    pub(in crate::sim) fn combat_stack_losses<'a>(
        &'a self,
        loser: &'a Unit,
        _winner: &'a Unit,
    ) -> impl Iterator<Item = &'a Unit> {
        // Civ III's manual describes the strongest unit on a square as the
        // defender; defeating it does not eliminate the other units stacked
        // there. Cargo removal is handled separately when the defeated unit
        // itself is a transport.
        self.units.iter().filter(move |unit| unit.id == loser.id)
    }

    pub(in crate::sim) fn resolve_zoc_attacks(&mut self, mover_id: u32, x: i32, y: i32) -> bool {
        let Some(mover) = self.unit_by_id(mover_id).cloned() else {
            return false;
        };
        if mover.kind.ignores_zoc() {
            return true;
        }
        let attackers = self
            .units
            .iter()
            .filter(|unit| {
                !self.allied(unit.owner, mover.owner)
                    && self.at_war(unit.owner, mover.owner)
                    && unit.transport.is_none()
                    && !self.tile(unit.x, unit.y).terrain.is_water()
                    && unit.kind.has_zone_of_control()
                    && !unit.kind.has_no_zoc()
                    && self.map_wrap.civ3_distance(x, y, unit.x, unit.y) == 1
            })
            .map(|unit| unit.id)
            .collect::<Vec<_>>();
        for attacker_id in attackers {
            let (Some(attacker), Some(defender)) = (
                self.unit_by_id(attacker_id).cloned(),
                self.unit_by_id(mover_id).cloned(),
            ) else {
                break;
            };
            let attack_power = Self::attack_power(&attacker);
            let defense_power = self.defense_power_against(&defender, x, y, attacker.kind);
            if attack_power > 0
                && defense_power > 0
                && self.rand_below(attack_power + defense_power) < attack_power
            {
                let (attack_firepower, _) = self.combat_firepower(&attacker, &defender);
                if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == mover_id) {
                    unit.hp -= attack_firepower;
                    if unit.hp <= 0 {
                        let before = self.units.len();
                        self.remove_units_with_cargo(vec![mover_id]);
                        self.nation_scores[attacker.owner].units_killed +=
                            (before - self.units.len()) as i32;
                        return false;
                    }
                }
            }
        }
        self.unit_by_id(mover_id).is_some()
    }

    fn remove_combat_losses(&mut self, loser: &Unit, winner: &Unit) {
        let losses = self
            .combat_stack_losses(loser, winner)
            .map(|unit| unit.id)
            .collect();
        let before = self.units.len();
        self.remove_units_with_cargo(losses);
        self.nation_scores[winner.owner].units_killed += (before - self.units.len()) as i32;
    }

    pub(in crate::sim) fn resolve_combat(
        &mut self,
        attacker_id: u32,
        defender_id: u32,
        nx: i32,
        ny: i32,
        facing: Option<u8>,
    ) {
        let Some(attacker_index) = self.unit_index_by_id(attacker_id) else {
            return;
        };
        let Some(defender_index) = self.unit_index_by_id(defender_id) else {
            return;
        };
        let mut attacker = self.units[attacker_index].clone();
        if let Some(facing) = facing {
            attacker.facing = facing;
        }
        let defender = self.units[defender_index].clone();
        let (attack_firepower, defense_firepower) = self.combat_firepower(&attacker, &defender);

        let attack_power = Self::attack_power(&attacker);
        let defense_power = self.defense_power_against(&defender, nx, ny, attacker.kind);
        let mut att_hp = attacker.hp;
        let mut def_hp = defender.hp;
        if attack_power <= 0 {
            att_hp = 0;
        } else if defense_power <= 0 {
            def_hp = 0;
        }
        let mut visual_hp = self
            .visual_events
            .enabled
            .then(|| vec![(attacker.hp, defender.hp)]);
        let mut rounds = 0;
        while att_hp > 0 && def_hp > 0 {
            if self.rand_below(attack_power + defense_power) >= defense_power {
                def_hp -= attack_firepower;
            } else {
                att_hp -= defense_firepower;
            }
            rounds += 1;
            if let Some(hp) = &mut visual_hp {
                hp.push((att_hp.max(0), def_hp.max(0)));
            }
        }
        if let Some(mut hp) = visual_hp {
            hp.push((att_hp.max(0), def_hp.max(0)));
            self.publish_visual_event(VisualEvent::Combat(Box::new(
                super::super::presentation::CombatVisual {
                    attacker: attacker.clone(),
                    defender: defender.clone(),
                    hp,
                    city: self.city_at(nx, ny).cloned(),
                    bombard_effects: false,
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
        att_hp = att_hp.max(0);
        def_hp = def_hp.max(0);

        if let Some(a) = self.units.iter_mut().find(|u| u.id == attacker_id) {
            a.hp = att_hp;
            a.moves_left = if a.kind.bombard_rate() > 0 {
                0
            } else {
                (a.moves_left - MOVE_FRAGMENTS).max(0)
            };
            a.moved = true;
            if let Some(facing) = facing {
                a.facing = facing;
            }
        }
        if let Some(d) = self.units.iter_mut().find(|u| u.id == defender_id) {
            d.hp = def_hp;
        }

        if attack_power > 0 && defense_power > 0 {
            if def_hp == 0 {
                if attacker.kind != UnitKind::CruiseMissile {
                    self.promote_civ3_unit(attacker_id, defender.owner);
                }
            } else if att_hp == 0 {
                self.promote_civ3_unit(defender_id, attacker.owner);
            }
        }

        let attacker_name = attacker.kind.name();
        let defender_name = defender.kind.name();
        let attacker_owner = attacker.owner;
        let defender_owner = defender.owner;
        if def_hp <= 0 {
            self.remove_combat_losses(&defender, &attacker);
            self.apply_combat_population_loss(&attacker, nx, ny);
            if att_hp > 0 {
                self.try_enslave_defeated_unit(&attacker, &defender, nx, ny);
            }
            if attacker.kind == UnitKind::CruiseMissile {
                self.remove_units_with_cargo(vec![attacker_id]);
            }
            let occupied = self.units.iter().any(|unit| {
                (unit.x, unit.y) == (nx, ny) && !self.allied(unit.owner, attacker_owner)
            });
            if !occupied
                && !attacker.kind.is_aircraft()
                && !self.treaty_blocks_move(&attacker, nx, ny)
                && let Some(cost) = self.movement_cost_for(&attacker, nx, ny)
                && self.cities.iter().all(|city| {
                    (city.x, city.y) != (nx, ny)
                        || !self.at_war(city.owner, attacker_owner)
                        || self.class_can_occupy_city(attacker.kind)
                })
            {
                self.enter_tile(attacker_id, nx, ny, (cost - MOVE_FRAGMENTS).max(0), facing);
            }
            if attacker_owner == HUMAN || defender_owner == HUMAN {
                self.add_event(format!(
                    "{attacker_name} defeated {defender_name} after {rounds} combat rounds."
                ));
            }
        } else if att_hp <= 0 {
            self.remove_combat_losses(&attacker, &defender);
            if attacker_owner == HUMAN || defender_owner == HUMAN {
                self.add_event(format!(
                    "{defender_name} held the tile against {attacker_name}."
                ));
            }
        }
        if attacker_owner == HUMAN || defender_owner == HUMAN {
            self.refresh_visibility();
        }
        self.check_game_over();
    }
}
