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
    Rgb::from_rgb8(251, 251, 251),
    Rgb::from_rgb8(255, 156, 138),
    Rgb::from_rgb8(255, 196, 113),
    Rgb::from_rgb8(253, 253, 167),
    Rgb::from_rgb8(53, 204, 52),
    Rgb::from_rgb8(199, 255, 255),
    Rgb::from_rgb8(153, 220, 246),
    Rgb::from_rgb8(255, 175, 248),
    Rgb::from_rgb8(191, 25, 234),
    Rgb::from_rgb8(204, 167, 71),
    Rgb::from_rgb8(205, 255, 123),
    Rgb::from_rgb8(218, 95, 82),
    Rgb::from_rgb8(199, 199, 199),
    Rgb::from_rgb8(99, 125, 214),
    Rgb::from_rgb8(90, 215, 232),
    Rgb::from_rgb8(254, 239, 255),
    Rgb::from_rgb8(136, 136, 136),
    Rgb::from_rgb8(219, 99, 255),
    Rgb::from_rgb8(246, 231, 102),
    Rgb::from_rgb8(191, 102, 255),
    Rgb::from_rgb8(223, 255, 255),
    Rgb::from_rgb8(184, 197, 141),
    Rgb::from_rgb8(255, 237, 102),
    Rgb::from_rgb8(255, 178, 102),
    Rgb::from_rgb8(240, 251, 207),
    Rgb::from_rgb8(185, 255, 224),
    Rgb::from_rgb8(254, 255, 168),
    Rgb::from_rgb8(232, 203, 166),
    Rgb::from_rgb8(183, 164, 112),
    Rgb::from_rgb8(145, 159, 126),
    Rgb::from_rgb8(255, 242, 255),
    Rgb::from_rgb8(241, 220, 255),
];

pub(crate) fn player_rgb(civilization_id: Option<&str>, owner: usize) -> Rgb {
    if let Some(civilization) = civilization_id.and_then(crate::rules::civilizations::by_id) {
        return SOURCE_COLORS[usize::from(civilization.primary_color_index)];
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

pub(crate) fn secondary_civilization_rgb(civilization_id: Option<&str>, owner: usize) -> Rgb {
    let Some(civilization) = civilization_id.and_then(crate::rules::civilizations::by_id) else {
        return player_rgb(None, owner);
    };
    SOURCE_COLORS[usize::from(civilization.secondary_color_index)]
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
