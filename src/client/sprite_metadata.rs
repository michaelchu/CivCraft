//! Civ III unit-art roles and player-color selection.

use super::*;

pub(super) use crate::map_palette::{player_color, secondary_civilization_color};

pub(super) fn unit_art_role(kind: UnitKind) -> String {
    kind.conquests_id()
        .map(|id| format!("unit-{}", id.replace('_', "-")))
        .unwrap_or_default()
}

#[cfg(test)]
mod unit_art_role_tests {
    use super::unit_art_role;
    use crate::sim::UnitKind;

    #[test]
    fn unit_art_roles_are_stable_kebab_case_manifest_keys() {
        assert_eq!(unit_art_role(UnitKind::Settler), "unit-settler");
        assert_eq!(unit_art_role(UnitKind::Warrior), "unit-warrior");
        assert_eq!(
            unit_art_role(UnitKind::StealthFighter),
            "unit-stealth-fighter"
        );
        assert_eq!(unit_art_role(UnitKind::MechInfantry), "unit-mech-infantry");
        assert_eq!(unit_art_role(UnitKind::ICBM), "unit-icbm");
    }

    #[test]
    fn all_active_conquests_identities_have_their_source_art_roles() {
        assert_eq!(UnitKind::ALL_NATIVE.len(), 92);
        for kind in UnitKind::ALL_NATIVE {
            let id = kind.conquests_id().expect("active unit needs a source id");
            assert_eq!(
                unit_art_role(kind),
                format!("unit-{}", id.replace('_', "-"))
            );
        }
    }
}
