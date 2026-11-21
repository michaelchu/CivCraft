//! Graphics-independent map and civilization RGB values shared by simulation and client.
use crate::sim::{MAX_CIVILIZATIONS, Terrain};

/// Normalized channels preserve the renderer's original float values, including
/// rebel colors that are not exact byte fractions. No graphics types cross here.
#[derive(Clone, Copy)]
pub(crate) struct Rgb([f32; 3]);

impl Rgb {
    const fn new(r: f32, g: f32, b: f32) -> Self {
        Self([r, g, b])
    }

    const fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    pub(crate) fn channels(self) -> [f32; 3] {
        self.0
    }

    pub(crate) fn to_rgb8(self) -> [u8; 3] {
        self.0.map(|channel| (channel * 255.0).round() as u8)
    }
}

const PLAYER_COLORS: [Rgb; MAX_CIVILIZATIONS + 1] = [
    Rgb::from_rgb8(236, 61, 58),
    Rgb::from_rgb8(245, 213, 55),
    Rgb::from_rgb8(56, 207, 226),
    Rgb::from_rgb8(151, 92, 212),
    Rgb::from_rgb8(241, 145, 42),
    Rgb::from_rgb8(225, 77, 171),
    Rgb::from_rgb8(76, 166, 94),
    Rgb::from_rgb8(85, 121, 217),
    Rgb::from_rgb8(169, 178, 56),
    Rgb::from_rgb8(72, 187, 154),
    Rgb::from_rgb8(201, 109, 178),
    Rgb::from_rgb8(209, 186, 82),
    Rgb::from_rgb8(78, 154, 190),
    Rgb::from_rgb8(175, 106, 79),
    Rgb::from_rgb8(122, 182, 77),
    Rgb::from_rgb8(147, 121, 198),
    Rgb::from_rgb8(179, 187, 191),
];

const SOURCE_COLORS: [Rgb; 32] = [
    Rgb::from_rgb8(230, 230, 230),
    Rgb::from_rgb8(227, 10, 10),
    Rgb::from_rgb8(255, 140, 0),
    Rgb::from_rgb8(230, 230, 0),
    Rgb::from_rgb8(7, 118, 7),
    Rgb::from_rgb8(29, 216, 218),
    Rgb::from_rgb8(52, 99, 255),
    Rgb::from_rgb8(255, 78, 237),
    Rgb::from_rgb8(121, 0, 156),
    Rgb::from_rgb8(144, 106, 6),
    Rgb::from_rgb8(120, 221, 0),
    Rgb::from_rgb8(148, 8, 8),
    Rgb::from_rgb8(147, 147, 147),
    Rgb::from_rgb8(0, 39, 146),
    Rgb::from_rgb8(14, 135, 151),
    Rgb::from_rgb8(243, 194, 252),
    Rgb::from_rgb8(60, 60, 60),
    Rgb::from_rgb8(192, 0, 255),
    Rgb::from_rgb8(205, 183, 0),
    Rgb::from_rgb8(126, 0, 255),
    Rgb::from_rgb8(172, 217, 234),
    Rgb::from_rgb8(114, 134, 55),
    Rgb::from_rgb8(255, 192, 0),
    Rgb::from_rgb8(255, 108, 0),
    Rgb::from_rgb8(196, 212, 149),
    Rgb::from_rgb8(117, 255, 173),
    Rgb::from_rgb8(189, 200, 68),
    Rgb::from_rgb8(175, 134, 82),
    Rgb::from_rgb8(111, 83, 10),
    Rgb::from_rgb8(72, 91, 44),
    Rgb::from_rgb8(251, 199, 255),
    Rgb::from_rgb8(198, 167, 237),
];

const SOURCE_BORDER_SECONDARY_COLORS: [Rgb; 32] = [
    Rgb::from_rgb8(184, 184, 184),
    Rgb::from_rgb8(176, 8, 8),
    Rgb::from_rgb8(179, 98, 0),
    Rgb::from_rgb8(191, 175, 0),
    Rgb::from_rgb8(5, 88, 5),
    Rgb::from_rgb8(23, 174, 176),
    Rgb::from_rgb8(42, 79, 204),
    Rgb::from_rgb8(200, 61, 186),
    Rgb::from_rgb8(107, 0, 138),
    Rgb::from_rgb8(120, 88, 5),
    Rgb::from_rgb8(100, 184, 0),
    Rgb::from_rgb8(131, 7, 7),
    Rgb::from_rgb8(126, 126, 126),
    Rgb::from_rgb8(0, 30, 114),
    Rgb::from_rgb8(12, 112, 126),
    Rgb::from_rgb8(185, 148, 192),
    Rgb::from_rgb8(32, 32, 32),
    Rgb::from_rgb8(131, 0, 175),
    Rgb::from_rgb8(143, 118, 0),
    Rgb::from_rgb8(48, 0, 177),
    Rgb::from_rgb8(104, 153, 172),
    Rgb::from_rgb8(88, 103, 42),
    Rgb::from_rgb8(194, 124, 0),
    Rgb::from_rgb8(223, 38, 0),
    Rgb::from_rgb8(118, 134, 71),
    Rgb::from_rgb8(41, 188, 101),
    Rgb::from_rgb8(101, 131, 52),
    Rgb::from_rgb8(127, 79, 18),
    Rgb::from_rgb8(68, 51, 6),
    Rgb::from_rgb8(47, 51, 13),
    Rgb::from_rgb8(173, 121, 177),
    Rgb::from_rgb8(136, 101, 180),
];

/// Resolve unique Civ III team palette indices (`1..32`, matching `ntp01`..`ntp31`)
/// for a match roster so two active civilizations do not share unit/border colors.
///
/// Civilizations whose `primary_color_index == secondary_color_index` (the canonical
/// base owners of palettes `1..=10`, such as Rome or America) reserve their primary
/// palette first; other civilizations claim `primary_color_index` if still free,
/// otherwise fall back to `secondary_color_index`, and finally to the lowest free
/// palette in `1..32` (palette `0` is reserved for barbarians).
pub(crate) fn resolve_team_palette_indices<'a>(
    civilization_ids: impl IntoIterator<Item = Option<&'a str>>,
) -> Vec<Option<u8>> {
    let entries: Vec<Option<&'static crate::rules::civilizations::Civilization>> = civilization_ids
        .into_iter()
        .map(|id| id.and_then(crate::rules::civilizations::by_id))
        .collect();
    let mut resolved = vec![None; entries.len()];
    let mut used: u32 = 1 << 0;

    for (slot, civilization) in entries.iter().enumerate() {
        if let Some(civ) = civilization
            && civ.primary_color_index == civ.secondary_color_index
            && (used & (1u32 << civ.primary_color_index)) == 0
        {
            used |= 1u32 << civ.primary_color_index;
            resolved[slot] = Some(civ.primary_color_index);
        }
    }

    for (slot, civilization) in entries.iter().enumerate() {
        if resolved[slot].is_some() {
            continue;
        }
        let Some(civ) = civilization else {
            continue;
        };
        let chosen = [civ.primary_color_index, civ.secondary_color_index]
            .into_iter()
            .find(|&idx| (idx as usize) < 32 && (used & (1u32 << idx)) == 0)
            .or_else(|| (1u8..32).find(|&idx| (used & (1u32 << idx)) == 0))
            .unwrap_or(civ.primary_color_index);
        used |= 1u32 << chosen;
        resolved[slot] = Some(chosen);
    }

    resolved
}

pub(crate) fn player_rgb_for_palette(palette_index: Option<u8>, owner: usize) -> Rgb {
    if let Some(&color) = palette_index.and_then(|index| SOURCE_COLORS.get(usize::from(index))) {
        return color;
    }
    const REBEL_COLORS: [Rgb; 5] = [
        Rgb::new(0.00, 0.89, 0.19),
        Rgb::new(0.00, 0.47, 0.95),
        Rgb::new(0.53, 0.24, 0.75),
        Rgb::new(1.00, 0.43, 0.76),
        Rgb::new(0.00, 0.62, 0.18),
    ];
    PLAYER_COLORS
        .get(owner)
        .copied()
        .unwrap_or_else(|| REBEL_COLORS[(owner - PLAYER_COLORS.len()) % REBEL_COLORS.len()])
}

pub(crate) fn border_colors_for_palette(
    palette_index: Option<u8>,
    owner: usize,
) -> ([u8; 3], [u8; 3]) {
    if let Some(index) = palette_index.map(usize::from)
        && let (Some(&main), Some(&secondary)) = (
            SOURCE_COLORS.get(index),
            SOURCE_BORDER_SECONDARY_COLORS.get(index),
        )
    {
        return (main.to_rgb8(), secondary.to_rgb8());
    }
    let main = player_rgb_for_palette(None, owner).to_rgb8();
    let secondary = main.map(|channel| (f32::from(channel) * 0.8).round() as u8);
    (main, secondary)
}

impl Terrain {
    pub(crate) fn minimap_rgb(self) -> Rgb {
        match self {
            Self::Coast => Rgb::from_rgb8(30, 140, 191),
            Self::Sea => Rgb::from_rgb8(18, 72, 132),
            Self::Ocean => Rgb::from_rgb8(12, 49, 102),
            Self::Lake => Rgb::from_rgb8(35, 99, 153),
            Self::Glacier => Rgb::from_rgb8(220, 232, 235),
            Self::Jungle => Rgb::from_rgb8(26, 107, 54),
            Self::Desert => Rgb::from_rgb8(211, 181, 103),
            Self::Forest => Rgb::from_rgb8(41, 91, 34),
            Self::Grassland => Rgb::from_rgb8(76, 146, 61),
            Self::Hills => Rgb::from_rgb8(99, 122, 63),
            Self::Mountains => Rgb::from_rgb8(126, 122, 112),
            Self::Plains => Rgb::from_rgb8(139, 165, 73),
            Self::Marsh => Rgb::from_rgb8(57, 91, 87),
            Self::Tundra => Rgb::from_rgb8(180, 184, 172),
            Self::FloodPlains => Rgb::from_rgb8(189, 159, 95),
            Self::Volcano => Rgb::from_rgb8(126, 85, 68),
            Self::GrasslandShield => Rgb::from_rgb8(76, 146, 61),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn roster_palette_resolution_uses_secondary_color_on_primary_collision() {
        let resolved = resolve_team_palette_indices([Some("rome"), Some("babylon"), None]);
        assert_eq!(resolved, vec![Some(1), Some(13), None]);

        // Even if a civilization with a dedicated secondary palette appears in
        // an earlier slot than the base owner of that primary palette, the base
        // owner retains its primary palette and the other takes its secondary.
        let reordered = resolve_team_palette_indices([Some("china"), Some("america")]);
        assert_eq!(reordered, vec![Some(15), Some(5)]);
    }

    #[test]
    fn every_seeded_match_roster_assigns_unique_team_palettes() {
        for seed in 0..32 {
            let roster = crate::rules::civilizations::select_roster(seed, MAX_CIVILIZATIONS);
            let palettes = resolve_team_palette_indices(
                roster
                    .iter()
                    .map(|civilization| Some(civilization.id.as_str())),
            );
            let unique: HashSet<u8> = palettes.into_iter().flatten().collect();
            assert_eq!(unique.len(), MAX_CIVILIZATIONS);
            assert!(!unique.contains(&0));
        }
    }
}
