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

fn without_sprite_grid(mut image: Image, width: usize, height: usize) -> Image {
    let stride = image.width();
    for (index, pixel) in image.get_image_data_mut().iter_mut().enumerate() {
        if (index % stride) % (width + 1) == 0 || (index / stride) % (height + 1) == 0 {
            *pixel = [0; 4];
        }
    }
    image
}

// Give each sprite its own duplicated edge pixels, as if loaded separately.
// Source sheets can contain opaque grid lines immediately outside the cells.
fn isolated_atlas(
    source: &Image,
    width: usize,
    height: usize,
    cols: usize,
    rows: usize,
    bordered: bool,
) -> Image {
    let gap = usize::from(bordered);
    isolated_atlas_at(source, width, height, cols, rows, (gap, gap), gap)
}

fn isolated_atlas_at(
    source: &Image,
    width: usize,
    height: usize,
    cols: usize,
    rows: usize,
    origin: (usize, usize),
    gap: usize,
) -> Image {
    let mut atlas = Image::gen_image_color(
        (cols * (width + 2)) as u16,
        (rows * (height + 2)) as u16,
        BLANK,
    );
    let stride = atlas.width();
    let src = source.get_image_data();
    let dst = atlas.get_image_data_mut();
    for row in 0..rows {
        for col in 0..cols {
            for y in 0..height + 2 {
                for x in 0..width + 2 {
                    let sx = origin.0 + col * (width + gap) + x.saturating_sub(1).min(width - 1);
                    let sy = origin.1 + row * (height + gap) + y.saturating_sub(1).min(height - 1);
                    dst[(row * (height + 2) + y) * stride + col * (width + 2) + x] =
                        src[sy * source.width() + sx];
                }
            }
        }
    }
    atlas
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
                        // Cover the rasterized diamond edge at fractional zoom. Adjacent
                        // tiles share this artwork; a one-texel overlap prevents
                        // transparent mask pixels from exposing the backing.
                        let mut mask = 0;
                        for ny in y.saturating_sub(1)..=(y + 1).min(47) {
                            for nx in x.saturating_sub(1)..=(x + 1).min(95) {
                                mask = mask.max(
                                    terrain_pixels[(mask_y + ny) * terrain.width() + mask_x + nx]
                                        [3],
                                );
                            }
                        }
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

fn build_water_corner_atlas(water: &Image) -> Image {
    let mut atlas = isolated_atlas_at(water, 48, 24, 16, 12, (1, 437), 1);
    // Extend only the bright ice lip upward into transparent pixels. Moving
    // the entire cliff would move its shadows onto the back of the ice tile.
    let original = atlas.get_image_data().to_vec();
    let stride = atlas.width();
    let pixels = atlas.get_image_data_mut();
    for row in 0..3 {
        for col in 0..16 {
            for y in 0..25 {
                for x in 0..50 {
                    let at = (row * 26 + y) * stride + col * 50 + x;
                    let below = original[at + stride];
                    if original[at][3] == 0
                        && below[3] == 255
                        && below[0] >= 190
                        && below[1] >= 190
                        && below[2] >= 190
                    {
                        pixels[at] = below;
                    }
                }
            }
        }
    }
    atlas
}

// Back transparent mask edges with the matching artwork, clipped by shared
// diamond geometry. A flat minimap/average color produces visible diagonal seams.
fn masked_terrain_backing_atlas(corners: &Image, water: &Image, terrain: &Image) -> Image {
    let mut image = Image::gen_image_color(96, 192, BLANK);
    for kind in 0..4 {
        for corner in 0..4 {
            for y in 0..48usize {
                for x in 0..96usize {
                    let pixel = if kind < 2 {
                        let (row, col) = ocean_cell_source([kind as u8; 4]);
                        corners.get_image_data()
                            [(corner * OCEAN_ATLAS_H + row * 48 + y) * OCEAN_ATLAS_W + col * 96 + x]
                    } else if kind == 3 {
                        terrain.get_image_data()[(1 + 7 * 49 + y) * terrain.width() + 1 + x]
                    } else {
                        let (ox, oy, row, col) = [
                            (24, 0, 9, 0),
                            (24, 24, 10, 0),
                            (48, 12, 11, 1),
                            (0, 12, 11, 0),
                        ][corner];
                        if x < ox || x >= ox + 48 || y < oy || y >= oy + 24 {
                            continue;
                        }
                        water.get_image_data()
                            [(437 + row * 25 + y - oy) * water.width() + 1 + col * 49 + x - ox]
                    };
                    if pixel[3] > 0 {
                        image.get_image_data_mut()[(kind * 48 + y) * 96 + x] = pixel;
                    }
                }
            }
        }
        // Extend the nearest artwork across the transparent raster edge. Only
        // the geometric diamond is drawn, so this cannot cover adjacent tiles.
        for y in 0..48usize {
            let start = (kind * 48 + y) * 96;
            let row = &mut image.get_image_data_mut()[start..start + 96];
            let visible: Vec<_> = row
                .iter()
                .enumerate()
                .filter(|(_, p)| p[3] > 0)
                .map(|(x, p)| (x, *p))
                .collect();
            assert!(
                !visible.is_empty(),
                "masked terrain artwork must cover every tile row"
            );
            for (x, pixel) in row.iter_mut().enumerate() {
                if pixel[3] == 0 {
                    *pixel = visible
                        .iter()
                        .min_by_key(|(sx, _)| x.abs_diff(*sx))
                        .unwrap()
                        .1;
                }
                pixel[3] = 255;
            }
        }
    }
    isolated_atlas(&image, 96, 48, 1, 4, false)
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
    pub(super) terrain_blends: Texture2D,
    pub(super) terrain_ice_blends: Texture2D,
    pub(super) water: Texture2D,
    pub(super) fog: Texture2D,
    pub(super) rivers: Texture2D,
    pub(super) terrain2: Texture2D,
    pub(super) hills: Texture2D,
    pub(super) mountains: Texture2D,
    ocean_corners: std::cell::RefCell<Mesh>,
    masked_terrain_backing: std::cell::RefCell<Mesh>,
    pub(super) units: Texture2D,
    pub(super) selection: Texture2D,
    pub(super) cities: Texture2D,
    pub(super) bases: Texture2D,
    pub(super) status: Texture2D,
    pub(super) goto: Texture2D,
    pub(super) explosions: Texture2D,
    pub(super) veterancy: Texture2D,
    pub(super) activities: Texture2D,
}

fn textured_diamond(
    mesh: &std::cell::RefCell<Mesh>,
    center: Vec2,
    width: f32,
    height: f32,
    source: Rect,
    tint: Color,
) {
    let mut mesh = mesh.borrow_mut();
    let texture = mesh.texture.as_ref().unwrap();
    let (tw, th) = (texture.width(), texture.height());
    set_diamond_vertices(
        &mut mesh.vertices,
        center,
        vec2(width, height),
        source,
        vec2(tw, th),
        tint,
    );
    draw_mesh(&mesh);
}

fn set_diamond_vertices(
    vertices: &mut [Vertex],
    center: Vec2,
    size: Vec2,
    source: Rect,
    texture_size: Vec2,
    tint: Color,
) {
    for (vertex, (x, y)) in
        vertices
            .iter_mut()
            .zip([(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)])
    {
        *vertex = Vertex::new(
            center.x + (x - 0.5) * size.x,
            center.y + (y - 0.5) * size.y,
            0.0,
            (source.x + x * source.w) / texture_size.x,
            (source.y + y * source.h) / texture_size.y,
            tint,
        );
    }
}

impl Assets {
    pub(super) fn draw_masked_terrain_backing(
        &self,
        terrain: Terrain,
        center: Vec2,
        width: f32,
        height: f32,
    ) {
        let row = match terrain {
            Terrain::DeepOcean => 0,
            Terrain::Lake => 2,
            Terrain::Glacier => 3,
            _ => 1,
        };
        let source = isolated_sprite_source(row, 0, 96.0, 48.0);
        textured_diamond(
            &self.masked_terrain_backing,
            center,
            width,
            height,
            source,
            WHITE,
        );
    }

    pub(super) fn draw_ocean_corner(
        &self,
        center: Vec2,
        width: f32,
        height: f32,
        source: Rect,
        tint: Color,
    ) {
        // The raster mask overlaps by one texel to cover sampling gaps. Clip
        // that overlap to the geometric tile so it cannot paint over ice.
        textured_diamond(&self.ocean_corners, center, width, height, source, tint);
    }

    pub(crate) fn new() -> Self {
        let terrain_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_terrain1.png"), None)
                .expect("valid Amplio2 terrain atlas");
        let fog_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_fog.png"), None)
                .expect("valid reference fog atlas");
        let fog = Texture2D::from_image(&super::fog::build_atlas(&fog_image));
        fog.set_filter(FilterMode::Nearest);
        let terrain = Texture2D::from_image(&isolated_atlas(&terrain_image, 96, 48, 9, 16, true));
        let terrain_blends =
            Texture2D::from_image(&super::terrain::build_blend_atlas(&terrain_image));
        terrain_blends.set_filter(FilterMode::Nearest);
        let terrain_ice_blends =
            Texture2D::from_image(&super::terrain::build_ice_blend_atlas(&terrain_image));
        terrain_ice_blends.set_filter(FilterMode::Nearest);
        let water_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_water.png"), None)
                .expect("valid water atlas");
        // Coast/lake cells use a separate 48x24 grid starting lower on the sheet.
        let water = Texture2D::from_image(&build_water_corner_atlas(&water_image));
        water.set_filter(FilterMode::Nearest);
        let river_image =
            Image::from_file_with_format(include_bytes!("../../assets/hexemplio_rivers.png"), None)
                .expect("valid Hexemplio river atlas");
        let rivers = Texture2D::from_image(&super::rivers::build_river_atlas(&river_image));
        rivers.set_filter(FilterMode::Nearest);
        let load_terrain = |bytes: &[u8], height, cols, rows| {
            let image = Image::from_file_with_format(bytes, None).expect("valid terrain atlas");
            Texture2D::from_image(&isolated_atlas(&image, 96, height, cols, rows, true))
        };
        let terrain2 = load_terrain(
            include_bytes!("../../assets/amplio2_terrain2.png"),
            48,
            9,
            6,
        );
        let hills = load_terrain(include_bytes!("../../assets/amplio2_hills.png"), 48, 4, 4);
        let mountains = load_terrain(
            include_bytes!("../../assets/amplio2_mountains.png"),
            66,
            4,
            4,
        );
        let ocean_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_ocean.png"), None)
                .expect("valid Amplio2 ocean atlas");
        let raw_ocean_corners = build_ocean_corner_atlas(&ocean_image, &terrain_image);
        let backing_texture = Texture2D::from_image(&masked_terrain_backing_atlas(
            &raw_ocean_corners,
            &water_image,
            &terrain_image,
        ));
        backing_texture.set_filter(FilterMode::Nearest);
        let masked_terrain_backing = std::cell::RefCell::new(Mesh {
            vertices: vec![Vertex::new(0.0, 0.0, 0.0, 0.0, 0.0, WHITE); 4],
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(backing_texture),
        });
        let ocean_corners =
            Texture2D::from_image(&isolated_atlas(&raw_ocean_corners, 96, 48, 9, 36, false));
        ocean_corners.set_filter(FilterMode::Nearest);
        let ocean_corners = std::cell::RefCell::new(Mesh {
            vertices: vec![Vertex::new(0.0, 0.0, 0.0, 0.0, 0.0, WHITE); 4],
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(ocean_corners),
        });
        let units_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_units.png"), None)
                .expect("valid unit atlas");
        let units = Texture2D::from_image(&isolated_atlas(&units_image, 64, 48, 20, 3, true));
        let selection = Texture2D::from_file_with_format(
            include_bytes!("../../assets/amplio2_select.png"),
            None,
        );
        selection.set_filter(FilterMode::Nearest);
        let cities_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_cities.png"), None)
                .expect("valid city atlas");
        let cities = Texture2D::from_image(&isolated_atlas(&cities_image, 96, 72, 10, 10, true));
        let explosions = Texture2D::from_file_with_format(
            include_bytes!("../../assets/amplio2_explosions.png"),
            None,
        );
        explosions.set_filter(FilterMode::Nearest);
        let status_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_tiles.png"), None)
                .expect("valid unit status atlas");
        let status = Texture2D::from_image(&without_sprite_grid(status_image.clone(), 96, 48));
        let goto = Texture2D::from_image(&super::routes::build_goto_atlas(
            &terrain_image,
            &status_image,
        ));
        goto.set_filter(FilterMode::Nearest);
        let veterancy = Texture2D::from_file_with_format(
            include_bytes!("../../assets/amplio2_veterancy.png"),
            None,
        );
        let activities = Texture2D::from_file_with_format(
            include_bytes!("../../assets/amplio2_activities.png"),
            None,
        );
        for texture in [&status, &veterancy, &activities] {
            texture.set_filter(FilterMode::Nearest);
        }
        let bases_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_bases.png"), None)
                .expect("valid bases atlas");
        let bases = Texture2D::from_image(&without_sprite_grid(bases_image, 96, 72));
        bases.set_filter(FilterMode::Nearest);
        // Padding isolates cells; nearest filtering retains the original pixel
        // artwork at fractional zoom without blending neighboring texels.
        terrain.set_filter(FilterMode::Nearest);
        terrain2.set_filter(FilterMode::Nearest);
        hills.set_filter(FilterMode::Nearest);
        mountains.set_filter(FilterMode::Nearest);
        units.set_filter(FilterMode::Nearest);
        cities.set_filter(FilterMode::Linear);
        Self {
            goto,
            terrain,
            terrain_blends,
            terrain_ice_blends,
            water,
            fog,
            terrain2,
            rivers,
            hills,
            mountains,
            ocean_corners,
            masked_terrain_backing,
            units,
            selection,
            cities,
            bases,
            status,
            explosions,
            veterancy,
            activities,
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
    fn isolated_cells_exclude_grid_lines_and_duplicate_only_their_own_edges() {
        for bordered in [false, true] {
            let gap = usize::from(bordered);
            let size = 2 * (2 + gap) + gap;
            let mut source = Image::gen_image_color(size as u16, size as u16, GREEN);
            let colors = [
                Color::from_rgba(255, 0, 0, 255),
                Color::from_rgba(0, 0, 255, 255),
                Color::from_rgba(255, 255, 0, 255),
                WHITE,
            ];
            for row in 0..2 {
                for col in 0..2 {
                    for y in 0..2 {
                        for x in 0..2 {
                            source.set_pixel(
                                (gap + col * (2 + gap) + x) as u32,
                                (gap + row * (2 + gap) + y) as u32,
                                colors[row * 2 + col],
                            );
                        }
                    }
                }
            }
            let atlas = isolated_atlas(&source, 2, 2, 2, 2, bordered);
            for row in 0..2 {
                for col in 0..2 {
                    for y in 0..4 {
                        for x in 0..4 {
                            assert_eq!(
                                atlas.get_pixel((col * 4 + x) as u32, (row * 4 + y) as u32),
                                colors[row * 2 + col]
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn water_backing_covers_mask_edges_without_changing_opaque_ocean_art() {
        let ocean =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_ocean.png"), None)
                .unwrap();
        let terrain =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_terrain1.png"), None)
                .unwrap();
        let water =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_water.png"), None)
                .unwrap();
        let corners = build_ocean_corner_atlas(&ocean, &terrain);
        let backing = masked_terrain_backing_atlas(&corners, &water, &terrain);
        assert!(backing.get_image_data().iter().all(|p| p[3] == 255));
        for kind in 0..2 {
            let (row, col) = ocean_cell_source([kind as u8; 4]);
            for y in 0..48 {
                for x in 0..96 {
                    let original = (0..4)
                        .rev()
                        .map(|corner| {
                            corners.get_image_data()[(corner * OCEAN_ATLAS_H + row * 48 + y)
                                * OCEAN_ATLAS_W
                                + col * 96
                                + x]
                        })
                        .find(|p| p[3] == 255);
                    if let Some(pixel) = original {
                        assert_eq!(
                            backing.get_image_data()[(kind * 50 + 1 + y) * 98 + 1 + x],
                            pixel
                        );
                    }
                }
            }
        }
        // Lake edges must use lake blue, independently of ocean colors.
        let lake_source = water.get_image_data()[(437 + 9 * 25 + 12) * water.width() + 1 + 24];
        assert_eq!(
            backing.get_image_data()[(2 * 50 + 1 + 12) * 98 + 1 + 48],
            lake_source
        );
    }

    #[test]
    fn ocean_mask_overlaps_the_raster_edge_without_holes() {
        let ocean =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_ocean.png"), None)
                .unwrap();
        let terrain =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_terrain1.png"), None)
                .unwrap();
        let corners = build_ocean_corner_atlas(&ocean, &terrain);
        let (row, col) = ocean_cell_source([0; 4]);
        let mut extended = 0;
        for y in 0..48usize {
            for x in 0..96usize {
                let original = terrain.get_image_data()
                    [(1 + 15 * 49 + y) * terrain.width() + 1 + 2 * 97 + x][3];
                let covered = (0..4).any(|corner| {
                    corners.get_image_data()
                        [(corner * OCEAN_ATLAS_H + row * 48 + y) * OCEAN_ATLAS_W + col * 96 + x][3]
                        > 0
                });
                if original > 0 {
                    assert!(covered, "hole at {x},{y}");
                }
                if original == 0 && covered {
                    extended += 1;
                }
            }
        }
        assert!(
            extended > 0,
            "fractional zoom needs coverage beyond the native alpha edge"
        );
    }

    #[test]
    fn ice_backing_fills_transparent_edges_with_ice_artwork() {
        let ocean =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_ocean.png"), None)
                .unwrap();
        let terrain =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_terrain1.png"), None)
                .unwrap();
        let water =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_water.png"), None)
                .unwrap();
        let corners = build_ocean_corner_atlas(&ocean, &terrain);
        let backing = masked_terrain_backing_atlas(&corners, &water, &terrain);
        for y in 0..48 {
            for x in 0..96 {
                let original = terrain.get_image_data()[(1 + 7 * 49 + y) * terrain.width() + 1 + x];
                let pixel = backing.get_image_data()[(3 * 50 + 1 + y) * 98 + 1 + x];
                assert_eq!(pixel[3], 255);
                if original[3] == 255 {
                    assert_eq!(pixel, original);
                } else {
                    assert!(pixel[0] > 200 && pixel[1] > 200, "edge must be ice-colored");
                }
            }
        }
    }

    #[test]
    fn cliff_rim_extension_preserves_shadows_and_lake_cells() {
        let water =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_water.png"), None)
                .unwrap();
        let original = isolated_atlas_at(&water, 48, 24, 16, 12, (1, 437), 1);
        let fixed = build_water_corner_atlas(&water);
        let before = original.get_image_data();
        let after = fixed.get_image_data();
        let stride = fixed.width();
        let mut changed = 0;
        for (i, (&a, &b)) in before.iter().zip(after).enumerate() {
            if a != b {
                changed += 1;
                assert_eq!(a[3], 0, "opaque artwork/shadows must not move");
                assert!(i / stride < 3 * 26, "only ice-cliff rows may change");
                assert_eq!(
                    b,
                    before[i + stride],
                    "new rim pixel must come from directly below"
                );
                assert!(b[0] >= 190 && b[1] >= 190 && b[2] >= 190 && b[3] == 255);
            }
        }
        assert!(changed > 0);
    }

    #[test]
    fn ocean_diamond_geometry_cannot_cover_an_adjacent_ice_tile() {
        fn cross(a: Vec2, b: Vec2) -> f32 {
            a.x * b.y - a.y * b.x
        }
        fn in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
            let signs = [
                cross(b - a, p - a),
                cross(c - b, p - b),
                cross(a - c, p - c),
            ];
            signs.iter().all(|s| *s >= 0.0) || signs.iter().all(|s| *s <= 0.0)
        }
        for zoom in [0.625, 0.82, 1.0, 1.35] {
            let center = vec2(311.137, 127.213);
            let size = vec2(96.0, 48.0) * zoom;
            let mut vertices = vec![Vertex::new(0.0, 0.0, 0.0, 0.0, 0.0, WHITE); 4];
            set_diamond_vertices(
                &mut vertices,
                center,
                size,
                isolated_sprite_source(0, 0, 96.0, 48.0),
                vec2(882.0, 1800.0),
                WHITE,
            );
            let positions: Vec<_> = vertices
                .iter()
                .map(|v| vec2(v.position.x, v.position.y))
                .collect();
            let covered = |point| {
                in_triangle(point, positions[0], positions[1], positions[2])
                    || in_triangle(point, positions[0], positions[2], positions[3])
            };
            assert!(covered(center + size * vec2(0.2, -0.2)));
            for delta in [
                vec2(0.3, -0.3),
                vec2(0.3, 0.3),
                vec2(-0.3, 0.3),
                vec2(-0.3, -0.3),
            ] {
                assert!(
                    !covered(center + size * delta),
                    "ocean must not cover neighboring ice at fractional zoom"
                );
            }
        }
    }

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
