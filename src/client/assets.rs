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
    Conf {
        window_title: "CivCraft".to_owned(),
        window_width: 1280,
        window_height: 720,
        window_resizable: true,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

pub(crate) struct Assets {
    pub(super) terrain: Texture2D,
    pub(super) terrain2: Texture2D,
    pub(super) hills: Texture2D,
    pub(super) mountains: Texture2D,
    pub(super) ocean_corners: Texture2D,
    pub(super) units: Texture2D,
    pub(super) cities: Texture2D,
}

impl Assets {
    pub(crate) fn new() -> Self {
        let terrain_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_terrain1.png"), None)
                .expect("valid Amplio2 terrain atlas");
        let terrain = Texture2D::from_image(&terrain_image);
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
            hills,
            mountains,
            ocean_corners,
            units,
            cities,
        }
    }
}

pub(super) const PLAYER_COLORS: [Color; PLAYER_COUNT] = [
    Color::from_rgba(236, 61, 58, 255),
    Color::from_rgba(245, 213, 55, 255),
    Color::from_rgba(56, 207, 226, 255),
    Color::from_rgba(151, 92, 212, 255),
    Color::from_rgba(241, 145, 42, 255),
    Color::from_rgba(225, 77, 171, 255),
];

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
            Self::Ocean => None,
        }
    }

    pub(super) fn minimap_color(self) -> Color {
        match self {
            Self::Ocean => Color::from_rgba(18, 72, 132, 255),
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
        UnitKind::Worker => (2, 14),
        UnitKind::Diplomat => (0, 11),
        UnitKind::Explorer => (1, 13),
        UnitKind::Warrior => (0, 18),
        UnitKind::Phalanx => (1, 1),
        UnitKind::Archer => (1, 8),
        UnitKind::Legion => (0, 16),
        UnitKind::Chariot => (0, 9),
        UnitKind::Catapult => (0, 7),
        UnitKind::Horseman => (0, 8),
    }
}
