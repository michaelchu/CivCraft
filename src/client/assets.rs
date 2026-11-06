use super::*;

pub(super) const TILE_SPRITE_W: usize = 96;
pub(super) const TILE_SPRITE_H: usize = 48;
pub(super) const OCEAN_ATLAS_W: usize = TILE_SPRITE_W * 9;
pub(super) const OCEAN_ATLAS_H: usize = TILE_SPRITE_H * 9;
const OCEAN_CORNER_LAYERS: usize = 4;

pub(super) fn bordered_sprite_source(row: i32, col: i32, width: f32, height: f32) -> Rect {
    Rect::new(
        1.0 + col as f32 * (width + 1.0),
        1.0 + row as f32 * (height + 1.0),
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

fn build_ocean_corner_atlas(ocean: &Image, terrain: &Image) -> Image {
    assert_eq!(ocean.width(), OCEAN_ATLAS_W);
    assert_eq!(ocean.height(), OCEAN_ATLAS_H);

    let mut atlas = Image::gen_image_color(
        OCEAN_ATLAS_W as u16,
        (OCEAN_ATLAS_H * OCEAN_CORNER_LAYERS) as u16,
        BLANK,
    );
    let ocean_pixels = ocean.get_image_data();
    let terrain_pixels = terrain.get_image_data();
    let atlas_pixels = atlas.get_image_data_mut();

    // Amplio2's mask.tile sprite is at row 15, column 2. The one-pixel
    // gutters are part of the atlas layout, not part of any sprite.
    let mask_x = 1 + 2 * (TILE_SPRITE_W + 1);
    let mask_y = 1 + 15 * (TILE_SPRITE_H + 1);

    for corner in 0..OCEAN_CORNER_LAYERS {
        for cell_row in 0..9 {
            for cell_col in 0..9 {
                for y in 0..TILE_SPRITE_H {
                    for x in 0..TILE_SPRITE_W {
                        let source = match corner {
                            // Top and bottom use the opposite vertical half of
                            // the four-terrain cell group. Right and left do
                            // the same horizontally. This mirrors Freeciv's
                            // corner-cell assembly exactly.
                            0 if (24..72).contains(&x) && y < 24 => Some((x, y + 24)),
                            1 if x >= 48 && (12..36).contains(&y) => Some((x - 48, y)),
                            2 if (24..72).contains(&x) && y >= 24 => Some((x, y - 24)),
                            3 if x < 48 && (12..36).contains(&y) => Some((x + 48, y)),
                            _ => None,
                        };
                        let Some((source_x, source_y)) = source else {
                            continue;
                        };

                        let ocean_x = cell_col * TILE_SPRITE_W + source_x;
                        let ocean_y = cell_row * TILE_SPRITE_H + source_y;
                        let mut pixel = ocean_pixels[ocean_y * OCEAN_ATLAS_W + ocean_x];
                        let mask = terrain_pixels[(mask_y + y) * terrain.width() + mask_x + x][3];
                        pixel[3] = ((pixel[3] as u16 * mask as u16) / 255) as u8;

                        let atlas_y = corner * OCEAN_ATLAS_H + cell_row * TILE_SPRITE_H + y;
                        let atlas_x = cell_col * TILE_SPRITE_W + x;
                        atlas_pixels[atlas_y * OCEAN_ATLAS_W + atlas_x] = pixel;
                    }
                }
            }
        }
    }

    atlas
}

pub(crate) fn window_conf() -> Conf {
    let args: Vec<_> = std::env::args().collect();
    let size = args
        .windows(2)
        .find(|a| a[0] == "--window-size")
        .and_then(|a| a[1].split_once('x'))
        .and_then(|(w, h)| Some((w.parse::<i32>().ok()?, h.parse::<i32>().ok()?)))
        .filter(|&(w, h)| w >= 800 && h >= 600)
        .unwrap_or((1280, 720));
    Conf {
        window_title: "CivCraft".to_owned(),
        window_width: size.0,
        window_height: size.1,
        window_resizable: true,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

pub(crate) struct Assets {
    pub(super) terrain: Texture2D,
    pub(super) rivers: Texture2D,
    pub(super) terrain2: Texture2D,
    pub(super) hills: Texture2D,
    pub(super) mountains: Texture2D,
    pub(super) ocean_corners: Texture2D,
    pub(super) units: Texture2D,
    pub(super) cities: Texture2D,
    pub(super) bases: Texture2D,
}

impl Assets {
    pub(crate) fn new() -> Self {
        let terrain_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_terrain1.png"), None)
                .expect("valid Amplio2 terrain atlas");
        let terrain = Texture2D::from_image(&terrain_image);
        let river_image =
            Image::from_file_with_format(include_bytes!("../../assets/hexemplio_rivers.png"), None)
                .expect("valid Hexemplio river atlas");
        let rivers = Texture2D::from_image(&super::rivers::build_river_atlas(&river_image));
        rivers.set_filter(FilterMode::Nearest);
        let terrain2 = Texture2D::from_file_with_format(
            include_bytes!("../../assets/amplio2_terrain2.png"),
            None,
        );
        let hills = Texture2D::from_file_with_format(
            include_bytes!("../../assets/amplio2_hills.png"),
            None,
        );
        let mountains = Texture2D::from_file_with_format(
            include_bytes!("../../assets/amplio2_mountains.png"),
            None,
        );
        let ocean_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_ocean.png"), None)
                .expect("valid Amplio2 ocean atlas");
        let ocean_corners =
            Texture2D::from_image(&build_ocean_corner_atlas(&ocean_image, &terrain_image));
        let units = Texture2D::from_file_with_format(
            include_bytes!("../../assets/amplio2_units.png"),
            None,
        );
        let cities = Texture2D::from_file_with_format(
            include_bytes!("../../assets/amplio2_cities.png"),
            None,
        );
        let bases = Texture2D::from_file_with_format(
            include_bytes!("../../assets/amplio2_bases.png"),
            None,
        );
        bases.set_filter(FilterMode::Nearest);
        // These are tightly packed sprite atlases. Nearest filtering keeps a
        // tile from sampling the neighbouring cell at zoom levels below 1x.
        terrain.set_filter(FilterMode::Nearest);
        terrain2.set_filter(FilterMode::Nearest);
        hills.set_filter(FilterMode::Nearest);
        mountains.set_filter(FilterMode::Nearest);
        ocean_corners.set_filter(FilterMode::Nearest);
        units.set_filter(FilterMode::Nearest);
        cities.set_filter(FilterMode::Linear);
        Self {
            terrain,
            terrain2,
            rivers,
            hills,
            mountains,
            ocean_corners,
            units,
            cities,
            bases,
        }
    }
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

pub(crate) fn native_unit_rendering_playtest() -> (bool, bool, bool) {
    let atlas =
        Image::from_file_with_format(include_bytes!("../../assets/amplio2_units.png"), None)
            .expect("valid native unit atlas");
    let pixels = atlas.get_image_data();
    let visible = |kind| {
        let (row, col) = unit_sprite(kind);
        let source = bordered_sprite_source(row, col, 64.0, 48.0);
        let (x, y) = (source.x as usize, source.y as usize);
        x + 64 <= atlas.width()
            && y + 48 <= atlas.height()
            && (y..y + 48)
                .any(|row| (x..x + 64).any(|col| pixels[row * atlas.width() + col][3] != 0))
    };
    let added = [
        UnitKind::Pikemen,
        UnitKind::Riflemen,
        UnitKind::AlpineTroops,
        UnitKind::Fanatics,
        UnitKind::MechInf,
        UnitKind::Elephants,
        UnitKind::Crusaders,
        UnitKind::Dragoons,
        UnitKind::Cavalry,
        UnitKind::Armor,
        UnitKind::Artillery,
        UnitKind::Howitzer,
        UnitKind::Leader,
        UnitKind::Awacs,
        UnitKind::Storm,
    ];
    let complete =
        UnitKind::ALL_NATIVE.len() == 57 && UnitKind::ALL_NATIVE.into_iter().all(visible);
    let distinct = added
        .into_iter()
        .map(unit_sprite)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        == 15;
    let labels = added
        .into_iter()
        .all(|kind| !short_unit_name(kind).is_empty())
        && short_unit_name(UnitKind::Awacs) == "AWACS"
        && short_unit_name(UnitKind::MechInf) == "Mech. Inf.";
    (complete, distinct, labels)
}

#[cfg(test)]
mod native_unit_tests {
    use super::*;

    #[test]
    fn all_native_unit_sprites_have_pixels_and_new_types_have_distinct_atlas_cells() {
        assert_eq!(native_unit_rendering_playtest(), (true, true, true));
    }
}

impl Resource {
    pub(super) fn sprite(self, terrain: Terrain) -> (i32, i32) {
        match self {
            Self::Oil if terrain == Terrain::Glacier => (6, 4),
            Self::Gold => (4, 2),
            Self::Iron => (4, 4),
            Self::Game => (5, 2),
            Self::Furs => (5, 4),
            Self::Coal => (3, 2),
            Self::Fish => (9, 2),
            Self::Fruit => (8, 4),
            Self::Gems => (8, 2),
            Self::Buffalo => (1, 2),
            Self::Wheat => (1, 4),
            Self::Oasis => (0, 2),
            Self::Peat => (7, 2),
            Self::Pheasant => (2, 2),
            Self::Resources => (11, 4),
            Self::Ivory => (6, 2),
            Self::Silk => (2, 4),
            Self::Spice => (7, 4),
            Self::Whales => (9, 4),
            Self::Wine => (3, 4),
            Self::Oil => (0, 4),
        }
    }
}
