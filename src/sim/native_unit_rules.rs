//! Derived capabilities of native Conquests units. No foreign unit identities
//! or parallel stat catalog participate in this view. String flags are only
//! the vocabulary of the retained generic requirement evaluator.
use super::UnitKind;
use super::unit_catalog::{UnitAiStrategy, UnitClass, UnitFlag};
use std::sync::OnceLock;

pub(super) struct UnitCapabilities {
    pub(super) id: &'static str,
    pub(super) unit_class: String,
    pub(super) flags: Vec<String>,
    pub(super) class_flags: Vec<String>,
}

pub(super) fn get(kind: UnitKind) -> &'static UnitCapabilities {
    static RULES: OnceLock<Vec<(UnitKind, UnitCapabilities)>> = OnceLock::new();
    &RULES
        .get_or_init(|| UnitKind::all().map(|kind| (kind, derive(kind))).collect())
        .iter()
        .find(|(candidate, _)| *candidate == kind)
        .expect("native unit capabilities")
        .1
}

fn derive(kind: UnitKind) -> UnitCapabilities {
    let rule = kind.conquests_rule();
    let metadata = kind.metadata();
    let class = metadata.class;
    let mut flags = Vec::new();
    for flag in [
        UnitFlag::Workers,
        UnitFlag::NoVeteran,
        UnitFlag::NoBuild,
        UnitFlag::Cities,
        UnitFlag::Marines,
        UnitFlag::Submarine,
        UnitFlag::Nuclear,
        UnitFlag::OneAttack,
        UnitFlag::Bombarder,
        UnitFlag::Fortress,
        UnitFlag::Capturer,
        UnitFlag::Capturable,
        UnitFlag::GameLoss,
        UnitFlag::AirAttacker,
    ] {
        if kind.has_flag(flag) {
            flags.push(format!("{flag:?}"));
        }
    }
    // Unique-unit civilization availability does not mean “one instance only”.
    for (name, enabled) in [
        (
            "NonMil",
            rule.attack == 0
                && rule.defense == 0
                && rule.bombard_strength == 0
                && class != UnitClass::Missile
                && kind != UnitKind::Army,
        ),
        ("IgTer", rule.has_capability("IgnoreTerrain")),
        ("HasNoZOC", rule.zone_of_control == 0),
        ("Flagless", rule.has_capability("HiddenNationality")),
        (
            "Coast",
            rule.has_capability("SinksInSea") || rule.has_capability("SinksInOcean"),
        ),
        ("Paratroopers", rule.has_capability("Paradrop")),
        ("AddToCity", rule.has_capability("JoinCity")),
        ("Cant_Fortify", !rule.has_capability("Fortify")),
    ] {
        if enabled {
            flags.push(name.to_owned());
        }
    }
    let ground = class.is_ground();
    let aerial = rule.unit_type == 2 || class == UnitClass::Missile;
    let mut class_flags = Vec::new();
    for (name, enabled) in [
        ("Ground", ground),
        ("TerrainSpeed", ground),
        ("TerrainDefense", ground),
        ("ZOC", ground),
        ("CanFortify", rule.has_capability("Fortify")),
        ("CanPillage", ground && rule.attack > 0),
        ("CanOccupyCity", rule.can_occupy_city()),
        ("BuildAnywhere", ground),
        ("Barracks", class.has_barracks()),
        ("Airliftable", rule.has_capability("Airlift")),
        ("Aerial", aerial),
        ("Missile", class == UnitClass::Missile),
        ("Unreachable", class.is_unreachable()),
        ("NonNatBombardTgt", ground),
        ("AttackNonNative", rule.unit_type == 1),
        ("AttFromNonNative", rule.unit_type == 1),
        (
            "DoesntOccupyTile",
            aerial || (rule.attack == 0 && kind != UnitKind::Army),
        ),
    ] {
        if enabled {
            class_flags.push(name.to_owned());
        }
    }
    UnitCapabilities {
        id: rule.id,
        unit_class: class.rule_name().to_owned(),
        flags,
        class_flags,
    }
}

/// AI classifications are CivCraft policy over native strategy bits, not
/// substitute units or a second source of gameplay statistics.
pub(super) fn has_role(kind: UnitKind, role: &str) -> bool {
    let rule = kind.conquests_rule();
    match role.to_ascii_lowercase().as_str() {
        "defendgood" | "defendgoodstartunit" => {
            kind.has_ai_strategy(UnitAiStrategy::Defense)
                || (rule.unit_type == 0 && rule.defense > 1 && !rule.has_capability("Army"))
        }
        "defendok" | "defendokstartunit" => {
            rule.unit_type == 0 && rule.defense > 0 && !rule.has_capability("Army")
        }
        "hunter" => {
            (kind.has_ai_strategy(UnitAiStrategy::Offense) && rule.moves > 1)
                || kind.has_ai_strategy(UnitAiStrategy::NavalPower)
                || kind.has_ai_strategy(UnitAiStrategy::CruiseMissile)
                || kind.has_ai_strategy(UnitAiStrategy::Icbm)
        }
        "ferryboat" | "ferrystartunit" => {
            kind.has_ai_strategy(UnitAiStrategy::NavalTransport) && rule.capacity > 0
        }
        "explorer" | "explorerstartunit" => kind.has_ai_strategy(UnitAiStrategy::Explore),
        "citiesstartunit" => kind.has_ai_strategy(UnitAiStrategy::Settle),
        "settlers" | "workerstartunit" => kind.has_ai_strategy(UnitAiStrategy::Terraform),
        "attackfaststartunit" | "attackstrongstartunit" => {
            kind.has_ai_strategy(UnitAiStrategy::Offense)
        }
        // These choices belong to the native settlement AI configuration.
        "barbarian" | "barbarianbuild" | "barbariansea" | "hut" => {
            kind.conquests_id() == Some(barbarian_units().basic_unit.as_str())
        }
        "barbariantech" | "barbarianbuildtech" | "barbarianseatech" | "huttech" => {
            kind.conquests_id() == Some(barbarian_units().advanced_unit.as_str())
        }
        "barbarianboat" | "barbarianboattech" => {
            kind.conquests_id() == Some(barbarian_units().sea_unit.as_str())
        }
        "partisan" => {
            kind.conquests_id()
                == Some(
                    crate::rules::native::settlements()
                        .citizens
                        .partisan_unit
                        .as_str(),
                )
        }
        _ => false,
    }
}
fn barbarian_units() -> &'static crate::rules::native::NativeBarbarianUnits {
    &crate::rules::native::settlements().barbarians
}

/// Cargo restrictions and capacity are PRTO data, independent of engine classes.
pub(super) fn can_carry(carrier: UnitKind, passenger: UnitKind) -> bool {
    let source = carrier.conquests_rule();
    let cargo = passenger.conquests_rule();
    if source.capacity == 0 || carrier == passenger {
        return false;
    }
    if carrier == UnitKind::Army {
        return cargo.unit_type == 0
            && cargo.attack > 0
            && cargo.defense > 0
            && cargo.capacity == 0
            && !passenger.is_nuclear_weapon();
    }
    if source.has_capability("CarryIcbm") {
        return cargo.has_capability("Icbm");
    }
    if source.has_capability("CarryAir") {
        return cargo.unit_type == 2;
    }
    if source.has_capability("CarryFootOnly") && !cargo.has_capability("Foot") {
        return false;
    }
    cargo.unit_type == 0
}

#[cfg(test)]
mod single_definition_tests {
    use super::*;
    use crate::sim::movement_rules::MovementRules;

    /// City capture, fortifying and class flags each have one definition,
    /// so the movement table, action-enabler flags and unit helpers agree.
    #[test]
    fn unit_properties_agree_across_every_consumer() {
        for kind in UnitKind::all() {
            let flag = |name| crate::sim::action_rules::unit_class_flag(kind, name).unwrap();
            let movement = &MovementRules::campaign_unit_movement_data(kind).1;
            assert_eq!(kind.can_occupy_city(), flag("CanOccupyCity"), "{kind:?}");
            assert_eq!(kind.can_occupy_city(), movement.can_occupy_city, "{kind:?}");
            assert_eq!(
                kind.unit_class().has_barracks(),
                flag("Barracks"),
                "{kind:?}"
            );
        }
        for (kind, expected) in [
            (UnitKind::Catapult, false),
            (UnitKind::Artillery, false),
            (UnitKind::Helicopter, false),
            (UnitKind::Warrior, true),
            (UnitKind::Marine, true),
            (UnitKind::Army, true),
        ] {
            assert_eq!(kind.can_occupy_city(), expected, "{kind:?}");
        }
        assert!(!UnitKind::Destroyer.can_fortify());
        assert!(!UnitKind::Fighter.can_fortify());
        assert!(UnitKind::Spearman.can_fortify());
    }
}
