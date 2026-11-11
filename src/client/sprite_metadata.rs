//! Civ III unit-art roles and player-color selection.

use super::*;

const PLAYER_COLORS: [Color; MAX_CIVILIZATIONS + 1] = [
    Color::from_rgba(236, 61, 58, 255),
    Color::from_rgba(245, 213, 55, 255),
    Color::from_rgba(56, 207, 226, 255),
    Color::from_rgba(151, 92, 212, 255),
    Color::from_rgba(241, 145, 42, 255),
    Color::from_rgba(225, 77, 171, 255),
    Color::from_rgba(76, 166, 94, 255),
    Color::from_rgba(85, 121, 217, 255),
    Color::from_rgba(169, 178, 56, 255),
    Color::from_rgba(72, 187, 154, 255),
    Color::from_rgba(201, 109, 178, 255),
    Color::from_rgba(209, 186, 82, 255),
    Color::from_rgba(78, 154, 190, 255),
    Color::from_rgba(175, 106, 79, 255),
    Color::from_rgba(122, 182, 77, 255),
    Color::from_rgba(147, 121, 198, 255),
    Color::from_rgba(179, 187, 191, 255),
];

const SOURCE_COLORS: [Color; 32] = [
    Color::from_rgba(251, 251, 251, 255),
    Color::from_rgba(255, 156, 138, 255),
    Color::from_rgba(255, 196, 113, 255),
    Color::from_rgba(253, 253, 167, 255),
    Color::from_rgba(53, 204, 52, 255),
    Color::from_rgba(199, 255, 255, 255),
    Color::from_rgba(153, 220, 246, 255),
    Color::from_rgba(255, 175, 248, 255),
    Color::from_rgba(191, 25, 234, 255),
    Color::from_rgba(204, 167, 71, 255),
    Color::from_rgba(205, 255, 123, 255),
    Color::from_rgba(218, 95, 82, 255),
    Color::from_rgba(199, 199, 199, 255),
    Color::from_rgba(99, 125, 214, 255),
    Color::from_rgba(90, 215, 232, 255),
    Color::from_rgba(254, 239, 255, 255),
    Color::from_rgba(136, 136, 136, 255),
    Color::from_rgba(219, 99, 255, 255),
    Color::from_rgba(246, 231, 102, 255),
    Color::from_rgba(191, 102, 255, 255),
    Color::from_rgba(223, 255, 255, 255),
    Color::from_rgba(184, 197, 141, 255),
    Color::from_rgba(255, 237, 102, 255),
    Color::from_rgba(255, 178, 102, 255),
    Color::from_rgba(240, 251, 207, 255),
    Color::from_rgba(185, 255, 224, 255),
    Color::from_rgba(254, 255, 168, 255),
    Color::from_rgba(232, 203, 166, 255),
    Color::from_rgba(183, 164, 112, 255),
    Color::from_rgba(145, 159, 126, 255),
    Color::from_rgba(255, 242, 255, 255),
    Color::from_rgba(241, 220, 255, 255),
];

pub(super) fn player_color(civilization_id: Option<&str>, owner: usize) -> Color {
    if let Some(civilization) = civilization_id.and_then(crate::rules::civilizations::by_id) {
        return SOURCE_COLORS[usize::from(civilization.primary_color_index)];
    }
    const REBEL_COLORS: [Color; 5] = [GREEN, BLUE, VIOLET, PINK, LIME];
    PLAYER_COLORS
        .get(owner)
        .copied()
        .unwrap_or_else(|| REBEL_COLORS[(owner - PLAYER_COLORS.len()) % REBEL_COLORS.len()])
}

pub(super) fn secondary_civilization_color(civilization_id: Option<&str>, owner: usize) -> Color {
    let Some(civilization) = civilization_id.and_then(crate::rules::civilizations::by_id) else {
        return player_color(None, owner);
    };
    SOURCE_COLORS[usize::from(civilization.secondary_color_index)]
}

impl Terrain {
    pub(super) fn minimap_color(self) -> Color {
        match self {
            Self::Coast => Color::from_rgba(30, 140, 191, 255),
            Self::Sea => Color::from_rgba(18, 72, 132, 255),
            Self::Ocean => Color::from_rgba(12, 49, 102, 255),
            Self::Lake => Color::from_rgba(35, 99, 153, 255),
            Self::Glacier => Color::from_rgba(220, 232, 235, 255),
            Self::Jungle => Color::from_rgba(26, 107, 54, 255),
            Self::Desert => Color::from_rgba(211, 181, 103, 255),
            Self::Forest => Color::from_rgba(41, 91, 34, 255),
            Self::Grassland => Color::from_rgba(76, 146, 61, 255),
            Self::Hills => Color::from_rgba(99, 122, 63, 255),
            Self::Mountains => Color::from_rgba(126, 122, 112, 255),
            Self::Plains => Color::from_rgba(139, 165, 73, 255),
            Self::Marsh => Color::from_rgba(57, 91, 87, 255),
            Self::Tundra => Color::from_rgba(180, 184, 172, 255),
            Self::FloodPlains => Color::from_rgba(189, 159, 95, 255),
            Self::Volcano => Color::from_rgba(126, 85, 68, 255),
            Self::GrasslandShield => Color::from_rgba(76, 146, 61, 255),
        }
    }
}

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
