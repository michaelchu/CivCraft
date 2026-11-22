//! Combat outcome calculation and the state changes caused by attacks.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhysicalAttackResult {
    Allowed,
    NonNativeTarget,
    Unreachable,
}

/// Individual member identities choose exchange boundaries. Visual proxies keep
/// the Army's map id so normal playback hides the carrier and restores it later.
struct ArmyVisualExchange {
    member_ids: (u32, u32),
    combat: super::super::presentation::CombatVisual,
}

impl Simulation {
    fn combat_visual_actor(&self, root: &Unit, attacking: bool, hp: i32) -> (u32, Unit) {
        if let Some(mut member) = self.army_combat_member(root, attacking) {
            let member_id = member.id;
            member.id = root.id;
            member.facing = root.facing;
            (member_id, member)
        } else {
            let mut unit = root.clone();
            unit.hp = hp;
            (root.id, unit)
        }
    }

    pub(crate) fn attack_power(unit: &Unit) -> i32 {
        Self::attack_power_with_base(unit, unit.kind.attack_strength())
    }

    fn attack_power_with_base(unit: &Unit, attack: i32) -> i32 {
        let mut power = percent_of(attack * POWER_FACTOR, unit.veteran_power());
        if super::super::combat_rules::selected().tired_attack
            && unit.moves_left < campaign_move_fragments()
        {
            power = power * unit.moves_left.max(0) / campaign_move_fragments();
        }
        power
    }

    pub(in crate::sim) fn attack_power_against(&self, attacker: &Unit, x: i32, y: i32) -> i32 {
        if let Some(member) = self.army_combat_member(attacker, true) {
            return self.attack_power_against(&member, x, y);
        }
        self.attack_power_against_with_base_for_action(
            attacker,
            x,
            y,
            attacker.kind.attack_strength(),
            self.combat_action_for_unit(attacker),
        )
    }

    /// Apply Freeciv's shared attack modifiers to an explicitly mapped
    /// content strength. Civ III air units retain their source `bombard_strength`
    /// for the C2C3 Bombard action even when their ordinary attack is zero.
    #[cfg(test)]
    pub(in crate::sim) fn attack_power_against_with_base(
        &self,
        attacker: &Unit,
        x: i32,
        y: i32,
        attack: i32,
    ) -> i32 {
        self.attack_power_against_with_base_for_action(attacker, x, y, attack, "Attack")
    }

    pub(in crate::sim) fn attack_power_against_with_base_for_action(
        &self,
        attacker: &Unit,
        x: i32,
        y: i32,
        attack: i32,
        action: &str,
    ) -> i32 {
        let base_power = Self::attack_power_with_base(attacker, attack);
        let context = crate::sim::action_rules::SimulationRequirementContext::for_unit_at(
            self,
            attacker,
            (x, y),
        )
        .with_action(action);
        let bonus = context.effect(EffectKind::AttackBonus);
        let bonus = i32::try_from(bonus).expect("campaign C2C3 attack bonus fits in i32");
        apply_percent_bonus(base_power, bonus)
    }

    #[cfg(test)]
    pub(crate) fn defense_power(&self, unit: &Unit, x: i32, y: i32) -> i32 {
        self.defense_power_against(unit, x, y, unit.kind)
    }

    pub(crate) fn defense_power_against(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
        attacker: UnitKind,
    ) -> i32 {
        self.defense_power_with_attacker(unit, x, y, Some(attacker), None)
    }

    /// Score an unfortified virtual defender without an attacking unit type.
    /// Freeciv's `get_virtual_defense_power(..., nullptr, ...)` deliberately
    /// skips attacker-dependent combat bonuses while retaining terrain,
    /// nativity, extra, and fortification effects.
    pub(in crate::sim) fn virtual_defense_power(
        &self,
        owner: usize,
        kind: UnitKind,
        x: i32,
        y: i32,
    ) -> i32 {
        let unit = self.make_unit(0, owner, kind, x, y);
        self.defense_power_with_attacker(&unit, x, y, None, None)
    }

    pub(in crate::sim) fn defense_power_with_building(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
        attacker: UnitKind,
        building: Option<Building>,
    ) -> i32 {
        self.defense_power_with_attacker(unit, x, y, Some(attacker), building)
    }

    fn defense_power_with_attacker(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
        attacker: Option<UnitKind>,
        building: Option<Building>,
    ) -> i32 {
        if let Some(member) = self.army_combat_member(unit, false) {
            return self.defense_power_with_attacker(&member, x, y, attacker, building);
        }
        // Freeciv applies these as separate integer-multiplication stages:
        // terrain and nativity in get_defense_power, unit-type multipliers,
        // Defend_Bonus, unit-type dividers, extras, then fortification.
        let mut power = percent_of(
            unit.kind.stats().defense * POWER_FACTOR,
            unit.veteran_power(),
        );
        let movement_rules = MovementRules::campaign();
        let defender_class = movement_rules
            .unit_class_for_kind(unit.kind)
            .expect("campaign C2C3 defender unit class is explicitly mapped");
        let class_rules = movement_rules.unit_class_movement_data(defender_class);
        let tile = self.tile(x, y);
        if class_rules.terrain_defense {
            let terrain_bonus = movement_rules
                .terrain_defense_bonus(tile.terrain)
                .expect("campaign C2C3 terrain defense is explicitly mapped");
            power = apply_percent_bonus(power, terrain_bonus);
        }
        let native = movement_rules
            .movement_tile_native_to_class(
                tile.terrain,
                tile.road,
                tile.railroad,
                tile.river,
                tile.base,
                defender_class,
            )
            .expect("campaign C2C3 defender tile nativity is supported");
        if !native {
            power = power * class_rules.non_native_def_pct / 100;
        }

        if let Some(attacker) = attacker {
            let defense_multiplier =
                100 + self.unit_combat_bonus_total(
                    unit.kind,
                    attacker,
                    crate::rules::engine_rules::CombatBonusKind::DefenseMultiplierPct,
                ) + 100
                    * self.unit_combat_bonus_total(
                        unit.kind,
                        attacker,
                        crate::rules::engine_rules::CombatBonusKind::DefenseMultiplier,
                    );
            power = (power * defense_multiplier / 100).max(0);
            let defend_bonus = self.defend_bonus_against(unit.owner, attacker, x, y, building);
            power = (apply_percent_bonus(power, defend_bonus)).max(0);
            let defense_divider =
                100 + self.unit_combat_bonus_total(
                    attacker,
                    unit.kind,
                    crate::rules::engine_rules::CombatBonusKind::DefenseDividerPct,
                ) + 100
                    * self.unit_combat_bonus_total(
                        attacker,
                        unit.kind,
                        crate::rules::engine_rules::CombatBonusKind::DefenseDivider,
                    );
            assert!(
                defense_divider > 0,
                "campaign C2C3 combat bonuses produced a nonpositive defense divider"
            );
            power = power * 100 / defense_divider;
        }
        // combat.c:defense_multiplication applies the extra attribute after
        // defense effects and before fortification. River is native only to
        // its selected C2C3 classes and is separate from terrain defense.
        let river_bonus = if tile.river {
            movement_rules
                .extra_class_defense_bonus("River", defender_class)
                .expect("campaign C2C3 river defense data is valid")
        } else {
            0
        };
        power += power * river_bonus / 100;

        let context =
            crate::sim::action_rules::SimulationRequirementContext::for_unit_at(self, unit, (x, y));
        let fortify_bonus = context.effect(EffectKind::FortifyDefenseBonus);
        let fortify_bonus =
            i32::try_from(fortify_bonus).expect("campaign C2C3 fortification bonus fits in i32");
        apply_percent_bonus(power, fortify_bonus)
    }

    fn defend_bonus_against(
        &self,
        defender_owner: usize,
        attacker: UnitKind,
        x: i32,
        y: i32,
        building: Option<Building>,
    ) -> i32 {
        let attacker_type = Unit::new(0, defender_owner, attacker, x, y);
        let context = crate::sim::action_rules::SimulationRequirementContext::for_unit_at(
            self,
            &attacker_type,
            (x, y),
        )
        .with_building_override(building);
        let defend_bonus = context.effect(EffectKind::DefendBonus);
        i32::try_from(defend_bonus).expect("campaign C2C3 defender bonus fits in i32")
    }

    fn unit_combat_bonus_total(
        &self,
        unit: UnitKind,
        target: UnitKind,
        kind: crate::rules::engine_rules::CombatBonusKind,
    ) -> i32 {
        let Some(bonuses) = crate::sim::action_rules::unit_combat_bonuses(unit) else {
            return 0;
        };
        bonuses
            .iter()
            .filter(|bonus| bonus.kind == kind)
            .filter(|bonus| {
                crate::sim::action_rules::unit_type_flag(target, &bonus.flag)
                    .expect("campaign C2C3 target flag is explicitly mapped")
            })
            .try_fold(0_i64, |total, bonus| {
                total.checked_add(i64::from(bonus.value))
            })
            .and_then(|total| i32::try_from(total).ok())
            .expect("campaign C2C3 combat-bonus total fits in i32")
    }

    pub(in crate::sim) fn combat_firepower(&self, attacker: &Unit, defender: &Unit) -> (i32, i32) {
        let army_attacker = self.army_combat_member(attacker, true);
        let army_defender = self.army_combat_member(defender, false);
        let attacker = army_attacker.as_ref().unwrap_or(attacker);
        let defender = army_defender.as_ref().unwrap_or(defender);
        let mut attack = attacker.kind.stats().firepower;
        let mut defense = defender.kind.stats().firepower;
        let in_city = self
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (defender.x, defender.y));
        let attacker_city_buster =
            crate::sim::action_rules::unit_type_flag(attacker.kind, "CityBuster")
                .expect("campaign C2C3 CityBuster flags are explicitly mapped");
        if in_city && attacker_city_buster {
            attack *= 2;
        }
        let attacker_bad_wall =
            crate::sim::action_rules::unit_type_flag(attacker.kind, "BadWallAttacker")
                .expect("campaign C2C3 BadWallAttacker flags are explicitly mapped");
        if attacker_bad_wall
            && self.defend_bonus_against(
                defender.owner,
                attacker.kind,
                defender.x,
                defender.y,
                None,
            ) > 0
        {
            let low_firepower =
                super::super::combat_rules::selected().low_firepower_badwallattacker;
            attack = attack.min(low_firepower);
        }
        let defender_bad_city =
            crate::sim::action_rules::unit_type_flag(defender.kind, "BadCityDefender")
                .expect("campaign C2C3 BadCityDefender flags are explicitly mapped");
        if in_city && defender_bad_city {
            attack *= 2;
            defense =
                defense.min(super::super::combat_rules::selected().low_firepower_pearl_harbor);
        }
        if self.unit_combat_bonus_total(
            attacker.kind,
            defender.kind,
            crate::rules::engine_rules::CombatBonusKind::LowFirepower,
        ) != 0
        {
            defense =
                defense.min(super::super::combat_rules::selected().low_firepower_combat_bonus);
        }
        self.cap_non_native_bombard_firepower(attacker, defender, attack, defense)
    }

    fn cap_non_native_bombard_firepower(
        &self,
        attacker: &Unit,
        defender: &Unit,
        mut attack: i32,
        mut defense: i32,
    ) -> (i32, i32) {
        let non_native_bombard_target =
            super::super::action_rules::unit_class_flag(defender.kind, "NonNatBombardTgt")
                .expect("campaign C2C3 defender unit class is explicitly mapped");
        if non_native_bombard_target
            && !self.combat_unit_is_native_at(attacker.kind, defender.x, defender.y)
            && (!self.combat_unit_can_exist_at_tile(defender, attacker.x, attacker.y)
                || !self.combat_unit_is_native_at(attacker.kind, attacker.x, attacker.y))
        {
            let low_firepower = super::super::combat_rules::selected().low_firepower_nonnat_bombard;
            attack = attack.min(low_firepower);
            defense = defense.min(low_firepower);
        }
        (attack, defense)
    }

    fn combat_unit_is_native_at(&self, kind: UnitKind, x: i32, y: i32) -> bool {
        let movement = MovementRules::campaign();
        let class = movement
            .unit_class_for_kind(kind)
            .expect("campaign C2C3 unit class is explicitly mapped");
        let tile = self.tile(x, y);
        movement
            .movement_tile_native_to_class(
                tile.terrain,
                tile.road,
                tile.railroad,
                tile.river,
                tile.base,
                class,
            )
            .expect("campaign C2C3 unit nativity is supported")
    }

    pub(in crate::sim) fn defender_rank(
        &self,
        attacker: &Unit,
        defender: &Unit,
        x: i32,
        y: i32,
    ) -> (i32, i32, i32) {
        let attack_power = self.attack_power_against(attacker, x, y);
        let attack_kind = self
            .army_combat_member(attacker, true)
            .map_or(attacker.kind, |u| u.kind);
        let defense_power = self.defense_power_against(defender, x, y, attack_kind);
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
        let mut defender_value = (100_000.0 * (1.0 - attacker_win)) as i32;
        let game_loss = crate::sim::action_rules::unit_type_flag(defender.kind, "GameLoss")
            .expect("campaign C2C3 GameLoss flag is explicitly mapped");
        if game_loss && !self.stack_vulnerable(x, y) {
            // common/combat.c::get_defender keeps a GameLoss unit as the last
            // defender while the target stack is protected.
            defender_value = -1;
        }
        let rounds_survived = (defender.hp + attack_firepower - 1) / attack_firepower;
        let defense_rating = defense_power * rounds_survived * defense_firepower;
        (defender_value, -defender_stats.cost, defense_rating)
    }

    pub(in crate::sim) fn combat_target_reachable(
        &self,
        attacker: UnitKind,
        defender: UnitKind,
        x: i32,
        y: i32,
    ) -> bool {
        if self.city_at(x, y).is_some() {
            return true;
        }
        let can_target = crate::sim::action_rules::unit_can_target_class(attacker, defender)
            .expect("campaign C2C3 combat target classes are explicitly mapped");
        if can_target {
            return true;
        }

        let movement_rules = MovementRules::campaign();
        let defender_class = movement_rules
            .unit_class_for_kind(defender)
            .expect("campaign C2C3 defender unit class is explicitly mapped");
        self.tile(x, y).base.is_some_and(|base| {
            movement_rules
                .extra_is_native_to_class(base.extra_rule_name(), defender_class)
                .expect("campaign C2C3 combat base nativity is supported")
        })
    }

    fn physical_attack_result(
        &self,
        attacker: &Unit,
        defender: &Unit,
        x: i32,
        y: i32,
    ) -> PhysicalAttackResult {
        let destination_native = self.combat_unit_is_native_at(attacker.kind, x, y);
        let attack_non_native =
            crate::sim::action_rules::unit_class_flag(attacker.kind, "AttackNonNative")
                .expect("campaign C2C3 attacker AttackNonNative class flag is explicitly mapped");
        let only_native_attack =
            crate::sim::action_rules::unit_type_flag(attacker.kind, "Only_Native_Attack")
                .expect("campaign C2C3 attacker Only_Native_Attack flag is explicitly mapped");
        if !destination_native && (!attack_non_native || only_native_attack) {
            return PhysicalAttackResult::NonNativeTarget;
        }

        if !self.combat_target_reachable(attacker.kind, defender.kind, x, y) {
            return PhysicalAttackResult::Unreachable;
        }
        PhysicalAttackResult::Allowed
    }

    fn physical_attack_stack_allowed_with_policy(
        &self,
        attacker: &Unit,
        x: i32,
        y: i32,
        unreachable_protects: bool,
    ) -> bool {
        let mut any_untransported_unit = false;
        let mut any_reachable_unit = false;
        let mut any_neverprotect_unit = false;

        for defender in self
            .units
            .iter()
            .filter(|unit| (unit.x, unit.y) == (x, y) && unit.transport.is_none())
        {
            any_untransported_unit = true;
            let result = self.physical_attack_result(attacker, defender, x, y);
            if unreachable_protects {
                if result == PhysicalAttackResult::Unreachable
                    && crate::sim::action_rules::unit_type_flag(defender.kind, "NeverProtects")
                        .expect("campaign C2C3 NeverProtects flag is explicitly mapped")
                {
                    any_neverprotect_unit = true;
                    continue;
                }
                if result != PhysicalAttackResult::Allowed {
                    return false;
                }
                any_reachable_unit = true;
            } else if result == PhysicalAttackResult::Allowed {
                return true;
            }
        }

        if unreachable_protects {
            // An empty tile is physically valid. A stack made only of
            // unreachable NeverProtects units is not a valid attack target.
            any_reachable_unit || !any_neverprotect_unit
        } else {
            !any_untransported_unit
        }
    }

    /// Mirror `unit_attack_units_at_tile_result` for the selected ruleset's
    /// whole-stack policy. Actor action enablers are checked by the action
    /// caller; this handles source combat reachability, native destination
    /// restrictions, transported-unit exclusion, and NeverProtects.
    pub(in crate::sim) fn physical_attack_stack_allowed(
        &self,
        attacker: &Unit,
        x: i32,
        y: i32,
    ) -> bool {
        self.physical_attack_stack_allowed_with_policy(
            attacker,
            x,
            y,
            super::super::combat_rules::selected().unreachable_protects,
        )
    }

    pub(in crate::sim) fn combat_action_for_unit(&self, attacker: &Unit) -> &'static str {
        if crate::sim::action_rules::unit_class_flag(attacker.kind, "Missile")
            .expect("campaign C2C3 attacker Missile class flag is explicitly mapped")
        {
            "Suicide Attack"
        } else {
            "Attack"
        }
    }

    /// Mirror `common/movement.c::can_exist_at_tile` for a campaign unit.
    /// C2C3 `citymindist` is currently 3, so its one-tile city-channel exception cannot
    /// apply; cities still admit BuildAnywhere classes and classes native at
    /// the city center or an adjacent tile.
    fn combat_unit_can_exist_at_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        let movement = MovementRules::campaign();
        let Some(class) = movement.unit_class_for_kind(unit.kind).ok() else {
            return false;
        };
        let class_rules = movement.unit_class_movement_data(class);
        let native_at = |tx, ty| {
            let tile = self.tile(tx, ty);
            movement
                .movement_tile_native_to_class(
                    tile.terrain,
                    tile.road,
                    tile.railroad,
                    tile.river,
                    tile.base,
                    class,
                )
                .expect("campaign C2C3 unit nativity is supported")
        };
        let tile = self.tile(x, y);
        if self.city_at(x, y).is_some() {
            return class_rules.build_anywhere
                || native_at(x, y)
                || self
                    .map_wrap
                    .eight_way_neighbors(x, y)
                    .any(|(tx, ty)| native_at(tx, ty));
        }

        let coast_strict = crate::sim::action_rules::unit_type_flag(unit.kind, "CoastStrict")
            .expect("campaign C2C3 CoastStrict flags are explicitly mapped");
        if coast_strict && tile.terrain.is_water() && !self.safe_ocean_tile(x, y) {
            return false;
        }
        native_at(x, y)
    }

    /// Mirror `common/movement.c::can_unit_survive_at_tile` for the selected
    /// C2C3 class and retained Civ III unit. A survivable location must allow
    /// physical existence and must not require an unavailable refuel point or
    /// lose hit points each turn.
    pub(in crate::sim) fn unit_can_survive_at_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.combat_unit_can_exist_at_tile(unit, x, y) {
            return false;
        }
        if self.city_at(x, y).is_some() {
            return true;
        }

        let base_refuels = self.tile(x, y).base.is_some_and(|base| {
            let extra_name = base.extra_rule_name();
            crate::rules::engine_rules::extra_by_name(extra_name).is_some_and(|record| {
                record.has_flag("Refuel")
                    && super::super::action_rules::unit_class_rule_name(unit.kind).is_some_and(
                        |class| {
                            record
                                .native_to
                                .unwrap_or_default()
                                .iter()
                                .any(|name| name.eq_ignore_ascii_case(&class))
                        },
                    )
            })
        });
        if base_refuels {
            return true;
        }

        if crate::sim::action_rules::unit_type_flag(unit.kind, "Coast")
            .expect("campaign C2C3 Coast flag is explicitly mapped")
            && self.safe_ocean_tile(x, y)
        {
            return true;
        }
        if unit.kind.fuel_capacity() > 0 {
            return false;
        }

        true
    }

    fn safe_ocean_tile(&self, x: i32, y: i32) -> bool {
        let movement = MovementRules::campaign();
        self.map_wrap.eight_way_neighbors(x, y).any(|(tx, ty)| {
            let terrain = self.tile(tx, ty).terrain;
            matches!(
                movement.terrain_has_flag(
                    terrain,
                    super::super::movement_rules::TerrainFlag::UnsafeCoast
                ),
                Ok(false)
            )
        })
    }

    /// `common/movement.c::unit_can_defend_here`: carried units only defend
    /// when their selected `tp_defense` policy permits it. `Alight` requires
    /// both a legal on-tile existence and a ruleset-enabled deboard/unload.
    pub(in crate::sim) fn combat_unit_can_defend_here(&self, unit: &Unit) -> bool {
        if self.is_army_member(unit) {
            return false;
        }
        let Some(carrier_id) = unit.transport else {
            return self.combat_unit_can_exist_at_tile(unit, unit.x, unit.y);
        };
        let Some(carrier) = self.unit_by_id(carrier_id) else {
            return false;
        };
        let transport_defense = match crate::sim::action_rules::unit_transport_defense(unit.kind) {
            Some(policy) => policy,
            None if crate::sim::action_rules::unit_class_rule_name(unit.kind).is_some() => {
                // Unmapped Civ III identities use Freeciv's selected engine
                // default from ruleload.c: TDT_ALIGHT.
                crate::rules::engine_rules::TransportDefense::Alight
            }
            None => return false,
        };
        match transport_defense {
            crate::rules::engine_rules::TransportDefense::Blocked => false,
            crate::rules::engine_rules::TransportDefense::Always => true,
            crate::rules::engine_rules::TransportDefense::Alight => {
                self.combat_unit_can_exist_at_tile(unit, unit.x, unit.y)
                    && (self.action_available_for_unit_fail_closed(
                        unit,
                        "Transport Deboard",
                        (carrier.x, carrier.y),
                    ) || self.action_available_for_unit_fail_closed(
                        carrier,
                        "Transport Unload",
                        (unit.x, unit.y),
                    ))
            }
        }
    }

    pub(in crate::sim) fn best_defender_id(&self, attacker: &Unit, x: i32, y: i32) -> Option<u32> {
        let mut best = None;
        let mut best_rank = None;
        for defender in self.tile_units_ordered(x, y).filter(|unit| {
            self.at_war(unit.owner, attacker.owner)
                && (!unit.kind.is_fixed_wing_aircraft() || attacker.kind.is_fixed_wing_aircraft())
                && self.combat_unit_can_defend_here(unit)
                && self.physical_attack_result(attacker, unit, x, y)
                    == PhysicalAttackResult::Allowed
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
        // common/combat.c:is_stack_vulnerable. The selected ruleset contains
        // no NoStackDeath extras; city tiles are the only protected locations.
        super::super::combat_rules::selected().killstack
            && !self.cities.iter().any(|city| (city.x, city.y) == (x, y))
    }

    pub(in crate::sim) fn combat_stack_losses<'a>(
        &'a self,
        loser: &'a Unit,
        winner: &'a Unit,
    ) -> impl Iterator<Item = &'a Unit> {
        let (x, y) = (loser.x, loser.y);
        let stack_vulnerable = self.stack_vulnerable(x, y);
        let winner_kind = winner.kind;
        let winner_owner = winner.owner;
        self.units.iter().filter(move |unit| {
            unit.id == loser.id
                || (stack_vulnerable
                    && (unit.x, unit.y) == (x, y)
                    && self.at_war(unit.owner, winner_owner)
                    && self.combat_target_reachable(winner_kind, unit.kind, x, y))
        })
    }

    pub(in crate::sim) fn resolve_zoc_attacks(&mut self, mover_id: u32, _x: i32, _y: i32) -> bool {
        // Freeciv Zone of Control is enforced as a movement legality rule in
        // `zone_blocks_step`; it does not trigger a combat round or damage.
        self.unit_by_id(mover_id).is_some()
    }

    fn remove_combat_losses(&mut self, loser: &Unit, winner: &Unit) {
        let losses = self
            .combat_stack_losses(loser, winner)
            .map(|unit| unit.id)
            .collect();
        let before = self.units.len();
        self.remove_units_with_cargo_because(losses, lifecycle::UnitRemoval::Combat);
        self.nation_scores[winner.owner].units_killed += (before - self.units.len()) as i32;
    }

    /// Conquests units with Bombard strength can fire one defensive shot
    /// when their tile is attacked or when a nearby Fortress unit fires at
    /// a passing enemy. The reaction is nonlethal and is spent whether it hits
    /// or misses.
    pub(in crate::sim) fn try_defensive_bombard(
        &mut self,
        attacker_id: u32,
        x: i32,
        y: i32,
    ) -> bool {
        let Some(attacker) = self.unit_by_id(attacker_id).cloned() else {
            return false;
        };
        let mut shooter = None;
        for (sx, sy, _) in self.map_wrap.tiles_within_steps(x, y, 1) {
            if (sx, sy) != (x, y) && self.tile(sx, sy).base != Some(Base::Fortress) {
                continue;
            }
            shooter = self
                .tile_units_ordered(sx, sy)
                .find(|unit| {
                    self.at_war(unit.owner, attacker.owner)
                        && unit.transport.is_none()
                        && unit.kind.bombard_strength() > 0
                        && unit.kind.bombard_rate() > 0
                        && !unit.defensive_bombarded
                })
                .cloned();
            if shooter.is_some() {
                break;
            }
        }
        if shooter.is_none() && attacker.kind.is_naval() {
            let coastal_fortress = self.cities.iter().find(|city| {
                self.at_war(city.owner, attacker.owner)
                    && city.has_building(crate::rules::Building::COASTAL_DEFENSE)
                    && self.map_wrap.step_distance(city.x, city.y, x, y) == 1
            });
            if coastal_fortress.is_some() {
                // Civ III's Coastal Fortress has naval bombardment strength 8
                // (the manual documents the reaction shot, the value is from
                // the Civ III unit/building reference).
                let attack = 8;
                let defense = self.defense_power_against(
                    &attacker,
                    attacker.x,
                    attacker.y,
                    UnitKind::Battleship,
                );
                if attack + defense > 0
                    && self.rand_below(attack + defense) < attack
                    && let Some(unit) = self.units.iter_mut().find(|unit| unit.id == attacker_id)
                {
                    unit.hp = (unit.hp - 1).max(1);
                }
                self.sync_army_damage(attacker_id);
                return true;
            }
        }
        let Some(shooter) = shooter else {
            return false;
        };

        if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == shooter.id) {
            unit.defensive_bombarded = true;
        }
        let attack = self.attack_power_against_with_base_for_action(
            &shooter,
            attacker.x,
            attacker.y,
            shooter.kind.bombard_strength(),
            "Bombard",
        );
        let defense = self.defense_power_against(&attacker, attacker.x, attacker.y, shooter.kind);
        if attack + defense > 0
            && self.rand_below(attack + defense) < attack
            && let Some(unit) = self.units.iter_mut().find(|unit| unit.id == attacker_id)
        {
            unit.hp = (unit.hp - 1).max(1);
        }
        self.sync_army_damage(attacker_id);
        true
    }

    pub(in crate::sim) fn resolve_combat(
        &mut self,
        attacker_id: u32,
        defender_id: u32,
        nx: i32,
        ny: i32,
        facing: Option<u8>,
    ) {
        self.resolve_combat_inner(attacker_id, defender_id, nx, ny, facing, true);
    }

    pub(in crate::sim) fn resolve_combat_inner(
        &mut self,
        attacker_id: u32,
        defender_id: u32,
        nx: i32,
        ny: i32,
        facing: Option<u8>,
        check_air_superiority: bool,
    ) {
        if check_air_superiority
            && self
                .unit_by_id(attacker_id)
                .is_some_and(|attacker| attacker.kind.is_aircraft())
            && !self.try_air_superiority_interceptions(attacker_id, (nx, ny))
        {
            return;
        }
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
        let want_combat_trace = self.trace.wants::<military_observation::CombatEvent>();
        let city_before = want_combat_trace
            .then(|| self.city_at(nx, ny).map(|city| (city.id, city.owner)))
            .flatten();
        let attacker_elite = want_combat_trace && self.unit_is_elite(&attacker);
        let defender_elite = want_combat_trace && self.unit_is_elite(&defender);
        let attacker_army_members = if want_combat_trace && attacker.kind == UnitKind::Army {
            self.cargo_count(attacker.id)
        } else {
            0
        };
        let defender_army_members = if want_combat_trace && defender.kind == UnitKind::Army {
            self.cargo_count(defender.id)
        } else {
            0
        };
        let defender_was_damaged = defender.hp < defender.max_hit_points();
        let attacker_moves_used = (self.unit_move_rate(&attacker) - attacker.moves_left).max(0);
        let defender_moves_used = (self.unit_move_rate(&defender) - defender.moves_left).max(0);
        let (attack_firepower, defense_firepower) = self.combat_firepower(&attacker, &defender);

        let air_to_air = attacker.kind.is_aircraft() && defender.kind.is_aircraft();
        let attack_power = if air_to_air {
            attacker.kind.stats().attack
        } else {
            self.attack_power_against(&attacker, nx, ny)
        };
        let defense_power = if air_to_air {
            defender.kind.stats().defense
        } else {
            self.defense_power_against(
                &defender,
                nx,
                ny,
                self.army_combat_member(&attacker, true)
                    .map_or(attacker.kind, |u| u.kind),
            )
        };
        let attacker_strength =
            i64::from(attack_power) * i64::from(attacker.hp) * i64::from(attack_firepower);
        let defender_strength =
            i64::from(defense_power) * i64::from(defender.hp) * i64::from(defense_firepower);
        let (attacker_veteran_chance, defender_veteran_chance) =
            super::super::veterancy::combat_veterancy_chances(
                attacker_strength,
                defender_strength,
                super::super::combat_rules::selected().combat_odds_scaled_veterancy,
            );
        let mut att_hp = attacker.hp;
        let mut def_hp = defender.hp;
        if attack_power <= 0 {
            att_hp = 0;
        } else if defense_power <= 0 {
            def_hp = 0;
        }
        let army_battle = attacker.kind == UnitKind::Army || defender.kind == UnitKind::Army;
        let mut army_exchanges: Option<Vec<ArmyVisualExchange>> =
            (self.visual_events.enabled && army_battle).then(Vec::new);
        let mut visual_hp =
            (self.visual_events.enabled && !army_battle).then(|| vec![(attacker.hp, defender.hp)]);
        let mut rounds = 0;
        while att_hp > 0 && def_hp > 0 {
            let exchange = army_exchanges.as_ref().map(|_| {
                let (attacker_member, a) = self.combat_visual_actor(&attacker, true, att_hp);
                let (defender_member, d) = self.combat_visual_actor(&defender, false, def_hp);
                ArmyVisualExchange {
                    member_ids: (attacker_member, defender_member),
                    combat: super::super::presentation::CombatVisual {
                        hp: vec![(a.hp, d.hp)],
                        attacker: a,
                        defender: d,
                        city: self.city_at(nx, ny).cloned(),
                        bombard_effects: false,
                        battle_ended: false,
                    },
                }
            });
            // Ordinary combat keeps its original powers and RNG sequence.
            // Armies switch to another member as the active member is exhausted.
            let (round_attack, round_defense, attack_fp, defense_fp) =
                if attacker.kind == UnitKind::Army || defender.kind == UnitKind::Army {
                    let (a, d) = self.combat_firepower(&attacker, &defender);
                    (
                        self.attack_power_against(&attacker, nx, ny),
                        self.defense_power_against(
                            &defender,
                            nx,
                            ny,
                            self.army_combat_member(&attacker, true)
                                .map_or(attacker.kind, |u| u.kind),
                        ),
                        a,
                        d,
                    )
                } else {
                    (
                        attack_power,
                        defense_power,
                        attack_firepower,
                        defense_firepower,
                    )
                };
            if self.rand_below(round_attack + round_defense) >= round_defense {
                def_hp = if defender.kind == UnitKind::Army {
                    self.damage_army_member(&defender, attack_fp, false)
                } else {
                    def_hp - attack_fp
                };
            } else {
                att_hp = if attacker.kind == UnitKind::Army {
                    self.damage_army_member(&attacker, defense_fp, true)
                } else {
                    att_hp - defense_fp
                };
            }
            if let (Some(exchanges), Some(mut exchange)) = (&mut army_exchanges, exchange) {
                let member_hp = |root: &Unit, member_id, total_hp: i32| {
                    if root.kind == UnitKind::Army {
                        self.unit_by_id(member_id).map_or(0, |unit| unit.hp.max(0))
                    } else {
                        total_hp.max(0)
                    }
                };
                let hp = (
                    member_hp(&attacker, exchange.member_ids.0, att_hp),
                    member_hp(&defender, exchange.member_ids.1, def_hp),
                );
                let battle_ended = att_hp <= 0 || def_hp <= 0;
                if let Some(previous) = exchanges
                    .last_mut()
                    .filter(|previous| previous.member_ids == exchange.member_ids)
                {
                    previous.combat.hp.push(hp);
                    previous.combat.battle_ended = battle_ended;
                } else {
                    exchange.combat.hp.push(hp);
                    exchange.combat.battle_ended = battle_ended;
                    exchanges.push(exchange);
                }
            }
            rounds += 1;
            if let Some(hp) = &mut visual_hp {
                hp.push((att_hp.max(0), def_hp.max(0)));
            }
        }
        if let Some(exchanges) = army_exchanges {
            if exchanges.is_empty() {
                // An empty Army or a powerless opponent has no selected member exchange.
                visual_hp = Some(vec![(attacker.hp, defender.hp)]);
            } else {
                for exchange in exchanges {
                    self.publish_visual_event(VisualEvent::Combat(Box::new(exchange.combat)));
                }
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
                    battle_ended: true,
                },
            )));
        }
        att_hp = att_hp.max(0);
        def_hp = def_hp.max(0);

        let mut damaged_attacker = attacker.clone();
        damaged_attacker.hp = att_hp;
        let attacker_moves_left = (self.unit_move_rate(&damaged_attacker)
            - attacker_moves_used
            - campaign_move_fragments())
        .max(0);
        let mut damaged_defender = defender.clone();
        damaged_defender.hp = def_hp;
        let defender_moves_left =
            (self.unit_move_rate(&damaged_defender) - defender_moves_used).max(0);

        if let Some(a) = self.units.iter_mut().find(|u| u.id == attacker_id) {
            a.hp = att_hp;
            // unithand.c::do_attack recomputes the allowance from post-combat
            // HP, preserves prior movement used, then action pricing charges
            // the attack's single-move cost.
            a.moves_left = attacker_moves_left;
            a.moved = true;
            if let Some(facing) = facing {
                a.facing = facing;
            }
        }
        if let Some(d) = self.units.iter_mut().find(|u| u.id == defender_id) {
            d.hp = def_hp;
            d.moves_left = defender_moves_left;
        }

        // Judged on the units as they entered the fight, so a promotion earned
        // in this fight does not make the winner eligible.
        let attacker_ineligible = self.great_leader_ineligibility_reason(&attacker, &defender);
        let defender_ineligible = self.great_leader_ineligibility_reason(&defender, &attacker);
        let attacker_can_lead = attacker_ineligible.is_none();
        let defender_can_lead = defender_ineligible.is_none();
        let experienced_attacker = self
            .army_combat_member(&attacker, true)
            .map_or(attacker_id, |u| u.id);
        let experienced_defender = self
            .army_combat_member(&defender, false)
            .map_or(defender_id, |u| u.id);
        self.promote_combat_veterans(
            experienced_attacker,
            experienced_defender,
            attack_power <= 0 || defense_power <= 0,
            attacker_veteran_chance,
            defender_veteran_chance,
        );
        // Only a real kill can create a Great Leader, never a powerless attack.
        let powerless_combat = attack_power <= 0 || defense_power <= 0;
        let mut leader_roll_denominator = None;
        let mut leader_created_id = None;
        if !powerless_combat {
            if def_hp <= 0 && attacker_can_lead {
                leader_roll_denominator = Some(self.great_leader_denominator(attacker.owner, true));
                leader_created_id = self.draw_great_leader(attacker_id, true);
            } else if att_hp <= 0 && defender_can_lead {
                leader_roll_denominator =
                    Some(self.great_leader_denominator(defender.owner, false));
                leader_created_id = self.draw_great_leader(defender_id, false);
            }
        }

        let attacker_name = attacker.kind.name();
        let defender_name = defender.kind.name();
        let attacker_owner = attacker.owner;
        let defender_owner = defender.owner;
        let mut attacker_occupied_tile = false;
        let mut missile_expended = false;
        if def_hp <= 0 {
            self.remove_combat_losses(&defender, &attacker);
            self.apply_combat_population_loss(&attacker, nx, ny);
            if attacker.kind == UnitKind::CruiseMissile {
                missile_expended = true;
                self.remove_units_with_cargo_because(
                    vec![attacker_id],
                    lifecycle::UnitRemoval::Expended,
                );
            }
            // server/unithand.c consumes one `fc_rand(100)` roll for every
            // surviving attacker win, even when the selected chance is 0 or
            // 100. Stack and movement blockers are checked after this roll.
            let occupies = self.unit_by_id(attacker_id).is_some()
                && self.rand_below(100) < super::super::combat_rules::selected().occupychance;
            let occupied = self.units.iter().any(|unit| {
                (unit.x, unit.y) == (nx, ny) && !self.military_access(unit.owner, attacker_owner)
            });
            if occupies
                && !occupied
                && !attacker.kind.is_aircraft()
                && !self.treaty_blocks_move(&attacker, nx, ny)
                && let Some(cost) = self.movement_cost_for(&attacker, nx, ny)
                && self.cities.iter().all(|city| {
                    (city.x, city.y) != (nx, ny)
                        || !self.at_war(city.owner, attacker_owner)
                        || self.class_can_occupy_city(attacker.kind)
                })
            {
                // Freeciv's occupy_move temporarily restores the unit's full
                // movement allowance so post-attack forced actions (notably
                // the selected Conquer City action) can pass MinMoveFrags.
                // It then charges only the occupation move beyond the attack's
                // single-move cost against the post-combat allowance.
                let old_moves = self
                    .unit_by_id(attacker_id)
                    .map_or(0, |unit| unit.moves_left);
                let full_moves = self.unit_move_rate(&attacker);
                if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == attacker_id) {
                    unit.moves_left = full_moves;
                }
                let entered = self.enter_tile(attacker_id, nx, ny, cost, facing);
                attacker_occupied_tile = entered;
                if entered
                    && let Some(unit) = self.units.iter_mut().find(|unit| unit.id == attacker_id)
                {
                    let occupation_cost =
                        (full_moves - unit.moves_left - campaign_move_fragments()).max(0);
                    unit.moves_left = (old_moves - occupation_cost).max(0);
                }
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
        if want_combat_trace {
            let winner = if def_hp <= 0 {
                "attacker"
            } else if att_hp <= 0 {
                "defender"
            } else {
                "none"
            };
            let (leader_eligible, leader_ineligible_reason) = if powerless_combat {
                (false, Some("powerless_combat"))
            } else if def_hp <= 0 {
                (attacker_can_lead, attacker_ineligible)
            } else if att_hp <= 0 {
                (defender_can_lead, defender_ineligible)
            } else {
                (false, Some("no_kill"))
            };
            let city_captured = city_before.is_some_and(|(city_id, prev_owner)| {
                prev_owner != attacker_owner
                    && self
                        .city_by_id(city_id)
                        .is_none_or(|city| city.owner == attacker_owner)
            });
            self.trace_combat(military_observation::CombatEvent {
                attacker_id,
                attacker_owner,
                attacker_kind: attacker.kind.name(),
                attacker_role: military_observation::military_unit_role(attacker.kind),
                attacker_elite,
                attacker_is_army: attacker.kind == UnitKind::Army,
                attacker_army_members,
                attacker_hp_before: attacker.hp,
                attacker_hp_after: att_hp,
                attacker_cost: attacker.kind.stats().cost,
                defender_id,
                defender_owner,
                defender_kind: defender.kind.name(),
                defender_role: military_observation::military_unit_role(defender.kind),
                defender_elite,
                defender_is_army: defender.kind == UnitKind::Army,
                defender_army_members,
                defender_hp_before: defender.hp,
                defender_hp_after: def_hp,
                defender_cost: defender.kind.stats().cost,
                tile: (nx, ny),
                city_id: city_before.map(|(id, _)| id),
                city_owner: city_before.map(|(_, owner)| owner),
                attack_power,
                defense_power,
                rounds,
                winner,
                attacker_occupied_tile,
                city_captured,
                missile_expended,
                powerless_combat,
                defender_was_damaged,
                leader_eligible,
                leader_ineligible_reason,
                leader_roll_denominator,
                leader_created_id,
            });
        }
        if attacker_owner == HUMAN || defender_owner == HUMAN {
            self.refresh_visibility();
        }
        self.check_game_over();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_combat_game() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
        }
        game
    }

    #[test]
    fn physical_attack_checks_the_complete_stack_with_selected_protection_policy() {
        let mut game = flat_combat_game();
        let attacker_id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        let target = (11, 20);
        game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
        game.spawn_unit(1, UnitKind::Fighter, target.0, target.1);
        let attacker = game.unit_by_id(attacker_id).unwrap().clone();

        assert!(
            !game.physical_attack_stack_allowed_with_policy(&attacker, target.0, target.1, true)
        );
        assert!(
            game.physical_attack_stack_allowed_with_policy(&attacker, target.0, target.1, false)
        );

        let transported_fighter = game
            .units
            .iter_mut()
            .find(|unit| unit.kind == UnitKind::Fighter)
            .unwrap();
        transported_fighter.transport = Some(999);
        assert!(
            game.physical_attack_stack_allowed_with_policy(&attacker, target.0, target.1, true)
        );
    }

    #[test]
    fn physical_attack_accepts_empty_stack_and_respects_non_native_targets() {
        let mut game = flat_combat_game();
        let attacker_id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        let attacker = game.unit_by_id(attacker_id).unwrap().clone();
        assert!(game.physical_attack_stack_allowed(&attacker, 11, 20));

        let sea_tile = (12, 20);
        game.tile_mut(sea_tile.0, sea_tile.1).terrain = Terrain::Ocean;
        let land_target = game.spawn_unit(1, UnitKind::Warrior, sea_tile.0, sea_tile.1);
        assert!(!game.physical_attack_stack_allowed(&attacker, sea_tile.0, sea_tile.1));

        game.remove_units_with_cargo(vec![land_target]);
        let sea_attacker_id = game.spawn_unit(HUMAN, UnitKind::Battleship, 10, 21);
        let sea_attacker = game.unit_by_id(sea_attacker_id).unwrap().clone();
        let land_tile = (11, 21);
        game.spawn_unit(1, UnitKind::Warrior, land_tile.0, land_tile.1);
        assert!(game.physical_attack_stack_allowed(&sea_attacker, land_tile.0, land_tile.1));
    }

    #[test]
    fn defender_selection_is_per_unit_while_stack_legality_checks_all_units() {
        let mut game = flat_combat_game();
        game.sign_pact(HUMAN, 1, Pact::War);
        let attacker_id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        let target = (11, 20);
        let defender_id = game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
        game.spawn_unit(1, UnitKind::Fighter, target.0, target.1);
        let attacker = game.unit_by_id(attacker_id).unwrap().clone();

        assert_eq!(
            game.best_defender_id(&attacker, target.0, target.1),
            Some(defender_id)
        );
        assert!(!game.physical_attack_stack_allowed(&attacker, target.0, target.1));

        let fighter = game
            .units
            .iter()
            .find(|unit| unit.kind == UnitKind::Fighter)
            .unwrap()
            .id;
        game.remove_units_with_cargo(vec![fighter]);
        assert_eq!(
            game.best_defender_id(&attacker, target.0, target.1),
            Some(defender_id)
        );
        assert!(game.physical_attack_stack_allowed(&attacker, target.0, target.1));
    }

    #[test]
    fn selected_c2c3_roster_has_no_never_protects_exception_units() {
        assert!(UnitKind::all().all(|kind| {
            crate::sim::action_rules::unit_type_flag(kind, "NeverProtects") == Some(false)
        }));
    }

    #[test]
    fn non_native_bombard_caps_both_firepowers_only_for_selected_target_class() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.tile_mut(10, 24).terrain = Terrain::Ocean;
        game.tile_mut(11, 24).terrain = Terrain::Grassland;
        let attacker = game.make_unit(1, HUMAN, UnitKind::Battleship, 10, 24);
        let defender = game.make_unit(2, 1, UnitKind::Warrior, 11, 24);
        let cap = super::super::combat_rules::selected().low_firepower_nonnat_bombard;

        assert_eq!(
            game.cap_non_native_bombard_firepower(&attacker, &defender, 4, 3),
            (cap, cap),
            "a Sea-class attack on a non-native Land-class target caps both firepowers"
        );

        game.create_city(HUMAN, 10, 24);
        assert_eq!(
            game.cap_non_native_bombard_firepower(&attacker, &defender, 4, 3),
            (4, 3),
            "the cap does not apply when the defender could exist at the attacker's source tile"
        );

        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        let native_attacker = game.make_unit(3, HUMAN, UnitKind::Artillery, 10, 24);
        assert_eq!(
            game.cap_non_native_bombard_firepower(&native_attacker, &defender, 4, 3),
            (4, 3),
            "the cap does not apply when the attacker is native at the defender tile"
        );
    }

    #[test]
    fn zone_of_control_does_not_roll_combat_or_damage_a_mover() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
            tile.base = None;
        }
        let mover_id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        game.spawn_unit(1, UnitKind::Cavalry, 11, 20);
        let hp_before = game.unit_by_id(mover_id).unwrap().hp;
        let random_before = game.random_snapshot();

        assert!(game.resolve_zoc_attacks(mover_id, 10, 20));

        assert_eq!(game.unit_by_id(mover_id).unwrap().hp, hp_before);
        assert_eq!(game.random_snapshot(), random_before);
    }
}
