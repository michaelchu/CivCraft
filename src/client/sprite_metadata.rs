use super::*;

pub(super) fn bordered_sprite_source(row: i32, col: i32, width: f32, height: f32) -> Rect {
    Rect::new(
        1.0 + col as f32 * (width + 1.0),
        1.0 + row as f32 * (height + 1.0),
        width,
        height,
    )
}

pub(super) fn isolated_sprite_source(row: i32, col: i32, width: f32, height: f32) -> Rect {
    Rect::new(
        1.0 + col as f32 * (width + 2.0),
        1.0 + row as f32 * (height + 2.0),
        width,
        height,
    )
}

pub(super) fn ocean_cell_source(matches: [u8; 4]) -> (usize, usize) {
    let [north, east, south, west] = matches;
    (
        8 - (east as usize + 3 * south as usize),
        8 - (north as usize + 3 * west as usize),
    )
}

const PLAYER_COLORS: [Color; PLAYER_COUNT + 1] = [
    Color::from_rgba(236, 61, 58, 255),
    Color::from_rgba(245, 213, 55, 255),
    Color::from_rgba(56, 207, 226, 255),
    Color::from_rgba(151, 92, 212, 255),
    Color::from_rgba(241, 145, 42, 255),
    Color::from_rgba(225, 77, 171, 255),
    Color::from_rgba(179, 187, 191, 255),
];

pub(super) fn player_color(owner: usize) -> Color {
    const REBEL_COLORS: [Color; 5] = [GREEN, BLUE, VIOLET, PINK, LIME];
    PLAYER_COLORS
        .get(owner)
        .copied()
        .unwrap_or_else(|| REBEL_COLORS[(owner - PLAYER_COLORS.len()) % REBEL_COLORS.len()])
}

impl Terrain {
    pub(super) fn sprite_row(self) -> Option<i32> {
        match self {
            Self::Desert => Some(0),
            Self::Plains => Some(1),
            Self::Grassland => Some(2),
            Self::Forest => Some(3),
            Self::Hills => Some(4),
            Self::Mountains => Some(5),
            Self::Tundra => Some(6),
            Self::Swamp => Some(8),
            Self::Glacier => Some(7),
            Self::Jungle => Some(9),
            Self::Ocean | Self::DeepOcean | Self::Lake => None,
        }
    }

    pub(super) fn minimap_color(self) -> Color {
        match self {
            Self::Ocean => Color::from_rgba(18, 72, 132, 255),
            Self::DeepOcean => Color::from_rgba(12, 49, 102, 255),
            Self::Lake => Color::from_rgba(35, 99, 153, 255),
            Self::Glacier => Color::from_rgba(220, 232, 235, 255),
            Self::Jungle => Color::from_rgba(26, 107, 54, 255),
            Self::Desert => Color::from_rgba(211, 181, 103, 255),
            Self::Forest => Color::from_rgba(41, 91, 34, 255),
            Self::Grassland => Color::from_rgba(76, 146, 61, 255),
            Self::Hills => Color::from_rgba(99, 122, 63, 255),
            Self::Mountains => Color::from_rgba(126, 122, 112, 255),
            Self::Plains => Color::from_rgba(139, 165, 73, 255),
            Self::Swamp => Color::from_rgba(57, 91, 87, 255),
            Self::Tundra => Color::from_rgba(180, 184, 172, 255),
        }
    }
}

pub(super) fn unit_sprite(kind: UnitKind) -> (i32, i32) {
    match kind {
        UnitKind::Settlers => (1, 4),
        UnitKind::Migrants => (2, 17),
        UnitKind::Worker => (2, 14),
        UnitKind::Engineer => (2, 2),
        UnitKind::Diplomat | UnitKind::BarbarianLeader => (0, 11),
        UnitKind::Spy => (2, 1),
        UnitKind::Explorer => (1, 13),
        UnitKind::Warrior => (0, 18),
        UnitKind::Phalanx => (1, 1),
        UnitKind::Archer => (1, 8),
        UnitKind::Legion => (0, 16),
        UnitKind::Chariot => (0, 9),
        UnitKind::Catapult => (0, 7),
        UnitKind::Musketeers => (0, 19),
        UnitKind::Marines => (2, 0),
        UnitKind::Paratroopers => (2, 9),
        UnitKind::Knights => (0, 15),
        UnitKind::Partisan => (1, 16),
        UnitKind::Cannon => (0, 4),
        UnitKind::Bomber => (0, 3),
        UnitKind::Fighter => (0, 12),
        UnitKind::StealthFighter => (2, 7),
        UnitKind::StealthBomber => (2, 6),
        UnitKind::Helicopter => (2, 4),
        UnitKind::Trireme => (1, 7),
        UnitKind::Caravel => (1, 3),
        UnitKind::Galleon => (1, 15),
        UnitKind::Transport => (1, 6),
        UnitKind::Carrier => (0, 6),
        UnitKind::Frigate => (0, 13),
        UnitKind::Ironclad => (0, 14),
        UnitKind::Destroyer => (1, 11),
        UnitKind::Cruiser => (0, 10),
        UnitKind::Battleship => (0, 2),
        UnitKind::Submarine => (1, 5),
        UnitKind::AegisCruiser => (2, 8),
        UnitKind::CruiseMissile => (1, 10),
        UnitKind::Nuclear => (1, 0),
        UnitKind::Caravan => (0, 5),
        UnitKind::Freight => (1, 14),
        UnitKind::Horseman => (0, 8),
        UnitKind::Pikemen => (1, 17),
        UnitKind::Riflemen => (1, 2),
        UnitKind::AlpineTroops => (2, 5),
        UnitKind::Fanatics => (2, 12),
        UnitKind::MechInf => (0, 17),
        UnitKind::Elephants => (2, 10),
        UnitKind::Crusaders => (2, 11),
        UnitKind::Dragoons => (1, 12),
        UnitKind::Cavalry => (1, 9),
        UnitKind::Armor => (0, 0),
        UnitKind::Artillery => (2, 3),
        UnitKind::Howitzer => (0, 1),
        UnitKind::Leader => (2, 15),
        UnitKind::Awacs => (2, 13),
        UnitKind::Storm => (2, 18),
    }
}

impl Resource {
    pub(super) fn icon_cell(self) -> (i32, i32) {
        match self {
            Self::Aluminum => (0, 0),
            Self::Coal => (0, 1),
            Self::Horses => (0, 2),
            Self::Iron => (0, 3),
            Self::Oil => (1, 0),
            Self::Rubber => (1, 1),
            Self::Saltpeter => (1, 2),
            Self::Uranium => (1, 3),
            Self::Dyes => (2, 0),
            Self::Furs => (2, 1),
            Self::Gems => (2, 2),
            Self::Incense => (2, 3),
            Self::Ivory => (3, 0),
            Self::Silk => (3, 1),
            Self::Spice => (3, 2),
            Self::Wine => (3, 3),
            Self::Cattle => (4, 0),
            Self::Fish => (4, 1),
            Self::Gold => (4, 2),
            Self::Game => (4, 3),
            Self::Oasis => (5, 0),
            Self::Sugar => (5, 1),
            Self::Tobacco => (5, 2),
            Self::TropicalFruit => (5, 3),
            Self::Whales => (6, 1),
            Self::Wheat => (6, 2),
        }
    }
}
