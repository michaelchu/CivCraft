//! Ruleset-driven hitpoint recovery and attrition.

use super::*;
use crate::rules::freeciv::{RulesetData, effect_value};
use crate::sim::movement_rules::MovementRules;

impl Simulation {
    /// Calculate Freeciv's `unit_gain_hitpoints()` delta at a coordinate.
    /// Civ III hitpoint capacity remains the mapped unit-content value; the
    /// selected C2C3 ruleset supplies regeneration, minimum HP, recovery, and
    /// non-native attrition.
    pub(super) fn unit_hitpoint_gain_at(&self, unit: &Unit, x: i32, y: i32) -> i32 {
        let base = unit.max_hit_points();
        if base <= 0 {
            return 0;
        }
        (unit.hp + self.unit_hitpoint_delta_per_turn(unit, x, y)).clamp(0, base) - unit.hp
    }

    fn unit_hitpoint_delta_per_turn(&self, unit: &Unit, x: i32, y: i32) -> i32 {
        let rules = RulesetData::campaign_default();
        let context =
            super::action_rules::SimulationRequirementContext::for_unit_at(self, unit, (x, y));
        let base = unit.max_hit_points();
        if base <= 0 {
            return 0;
        }

        let mut gain = 0;
        if !unit.moved {
            let hp_regen = effect_value(&rules.effects, "HP_Regen", &context)
                .expect("campaign C2C3 HP_Regen requirements are supported");
            let min_hp_pct = effect_value(&rules.effects, "Min_HP_Pct", &context)
                .expect("campaign C2C3 Min_HP_Pct requirements are supported");
            let regen_2 = effect_value(&rules.effects, "HP_Regen_2", &context)
                .expect("campaign C2C3 HP_Regen_2 requirements are supported");
            let regen = percent_of(
                base,
                i32::try_from(hp_regen).expect("campaign HP_Regen fits in i32"),
            );
            let minimum = percent_of(
                base,
                i32::try_from(min_hp_pct).expect("campaign Min_HP_Pct fits in i32"),
            );
            // common/unit.c::hp_gain_coord uses ceil(base / 10), then
            // truncates after applying HP_Regen_2.
            let regen_2_base = (base + 9) / 10;
            let regen_2 = regen_2_base
                * i32::try_from(regen_2).expect("campaign HP_Regen_2 fits in i32")
                / 10;
            gain += regen.max(minimum) + regen_2;
        }

        gain += i32::try_from(
            effect_value(&rules.effects, "Unit_Recover", &context)
                .expect("campaign C2C3 Unit_Recover requirements are supported"),
        )
        .expect("campaign Unit_Recover fits in i32");

        // The selected server default is killunhomed=0, so no additional
        // unhomed attrition applies. Non-native class attrition remains active
        // unless a city, native base, or transporter protects the unit.
        if self.city_at(x, y).is_none()
            && !self.unit_is_on_native_base(unit.kind, x, y)
            && unit.transport.is_none()
        {
            gain -= base * self.unit_class_hp_loss_pct(unit.kind) / 100;
        }

        gain
    }

    /// Net recovery per stationary turn for planning an aircraft's repair
    /// destination. This intentionally ignores current damage so the planner
    /// can estimate the number of turns needed to reach full HP.
    pub(super) fn stationary_unit_hp_recovery_at(&self, unit: &Unit, x: i32, y: i32) -> i32 {
        let mut stationary = unit.clone();
        stationary.moved = false;
        self.unit_hitpoint_delta_per_turn(&stationary, x, y).max(0)
    }

    fn unit_class_hp_loss_pct(&self, kind: UnitKind) -> i32 {
        let class = MovementRules::campaign()
            .unit_class_for_kind(kind)
            .expect("campaign C2C3 unit class is explicitly mapped");
        let class_name = MovementRules::campaign()
            .unit_class_movement_data(class)
            .expect("campaign C2C3 unit class data is valid")
            .rule_name;
        let record = RulesetData::campaign_default()
            .rule_records
            .iter()
            .find(|record| {
                record.namespace.eq_ignore_ascii_case("unit_class")
                    && record.rule_name.eq_ignore_ascii_case(&class_name)
            })
            .expect("selected C2C3 unit class record exists");
        i32::try_from(
            record
                .integer_field("hp_loss_pct")
                .expect("selected C2C3 class hp_loss_pct is valid")
                .unwrap_or(0),
        )
        .expect("selected C2C3 class hp_loss_pct fits in i32")
    }

    fn unit_is_on_native_base(&self, kind: UnitKind, x: i32, y: i32) -> bool {
        let Some(base) = self.tile(x, y).base else {
            return false;
        };
        let movement = MovementRules::campaign();
        let class = movement
            .unit_class_for_kind(kind)
            .expect("campaign C2C3 unit class is explicitly mapped");
        movement
            .extra_is_native_to_class(
                crate::sim::movement_rules::c2c3_base_extra_rule_name(base),
                class,
            )
            .expect("campaign C2C3 base nativity is supported")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site_game() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game
    }

    #[test]
    fn c2c3_air_hp_recovery_uses_airstrip_airbase_airport_and_city_effects() {
        let mut game = site_game();
        let airstrip = (10, 20);
        game.tile_mut(airstrip.0, airstrip.1).base = Some(Base::Airstrip);
        let airbase = (12, 20);
        game.tile_mut(airbase.0, airbase.1).base = Some(Base::Airbase);
        let airport = (14, 20);
        game.create_city(HUMAN, airport.0, airport.1);
        game.cities
            .last_mut()
            .unwrap()
            .buildings
            .push(Building::AIRPORT);
        let city = (16, 20);
        game.create_city(HUMAN, city.0, city.1);

        let id = game.spawn_unit(HUMAN, UnitKind::Fighter, airstrip.0, airstrip.1);
        let fighter = game.unit_by_id(id).unwrap().clone();
        let hp = fighter.max_hit_points();
        let expected_airstrip = hp * 10 / 100;
        let expected_airbase = hp * 34 / 100;
        let expected_city = hp * 33 / 100;

        assert_eq!(
            game.stationary_unit_hp_recovery_at(&fighter, airstrip.0, airstrip.1),
            expected_airstrip
        );
        assert_eq!(
            game.stationary_unit_hp_recovery_at(&fighter, airbase.0, airbase.1),
            expected_airbase
        );
        assert_eq!(
            game.stationary_unit_hp_recovery_at(&fighter, airport.0, airport.1),
            hp
        );
        assert_eq!(
            game.stationary_unit_hp_recovery_at(&fighter, city.0, city.1),
            expected_city
        );

        let carrier = game.spawn_unit(HUMAN, UnitKind::Carrier, 18, 20);
        let carrier_id = game.unit_by_id(carrier).unwrap().id;
        let mut aboard = fighter;
        aboard.transport = Some(carrier_id);
        assert_eq!(game.stationary_unit_hp_recovery_at(&aboard, 18, 20), 0);
    }

    #[test]
    fn unit_hitpoint_gain_applies_moved_gate_fortification_and_non_native_attrition() {
        let mut game = site_game();
        let (x, y) = (10, 20);
        let mut warrior = Unit::new(999, HUMAN, UnitKind::Warrior, x, y);
        warrior.veteran = 3;
        warrior.hp = 1;
        assert_eq!(game.unit_hitpoint_gain_at(&warrior, x, y), 1);

        warrior.state = UnitState::Fortified;
        assert_eq!(game.unit_hitpoint_gain_at(&warrior, x, y), 2);

        warrior.moved = true;
        assert_eq!(game.unit_hitpoint_gain_at(&warrior, x, y), 0);

        let mut helicopter = Unit::new(1000, HUMAN, UnitKind::Helicopter, x, y);
        helicopter.hp = 1;
        let expected_attrition = helicopter.max_hit_points() * 10 / 100;
        assert_eq!(
            game.unit_hitpoint_gain_at(&helicopter, x, y),
            -expected_attrition
        );
        game.create_city(HUMAN, x, y);
        assert_eq!(
            game.unit_hitpoint_gain_at(&helicopter, x, y),
            helicopter.max_hit_points() * 33 / 100
        );
    }
}
