//! Loading and preparing textures, sprites, and other renderer assets.

use super::sprite_metadata::*;
use super::*;

pub(super) const TILE_SPRITE_W: usize = 96;
pub(super) const TILE_SPRITE_H: usize = 48;
pub(super) const OCEAN_ATLAS_W: usize = TILE_SPRITE_W * 9;
pub(super) const OCEAN_ATLAS_H: usize = TILE_SPRITE_H * 9;
const OCEAN_CORNER_LAYERS: usize = 4;

fn without_sprite_grid(mut image: Image, width: usize, height: usize) -> Image {
    let stride = image.width();
    for (index, pixel) in image.get_image_data_mut().iter_mut().enumerate() {
        if (index % stride).is_multiple_of(width + 1) || (index / stride).is_multiple_of(height + 1)
        {
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
    pub(super) civ3: Option<super::tileset::Civ3>,
    pub(super) terrain: Texture2D,
    pub(super) resource_icons: Texture2D,
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
    pub(super) leader_avatar_badges: Vec<Texture2D>,
    pub(super) leader_portraits: Vec<Vec<Texture2D>>,
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
    pub(super) fn leader_portrait(&self, owner: usize, attitude: &str) -> Option<&Texture2D> {
        let attitude_index = match attitude {
            "Furious" => 0,
            "Annoyed" => 1,
            "Cautious" => 2,
            "Polite" => 3,
            "Gracious" => 4,
            _ => 2,
        };
        self.leader_portraits.get(owner)?.get(attitude_index)
    }

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

    pub(crate) fn new(tileset: &super::Tileset) -> Result<Self, String> {
        let civ3 = match tileset {
            super::Tileset::Amplio2 => None,
            super::Tileset::Civ3(path) => Some(super::tileset::Civ3::load(path)?),
        };
        let terrain_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_terrain1.png"), None)
                .expect("valid Amplio2 terrain atlas");
        let fog_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_fog.png"), None)
                .expect("valid reference fog atlas");
        let fog = Texture2D::from_image(&super::fog::build_atlas(&fog_image));
        fog.set_filter(FilterMode::Nearest);
        let terrain = Texture2D::from_image(&isolated_atlas(&terrain_image, 96, 48, 9, 16, true));
        let resource_icon_image = Image::from_file_with_format(
            include_bytes!("../../assets/civ3_resources_generated.png"),
            None,
        )
        .expect("valid generated resource icon sheet");
        let resource_icons = Texture2D::from_image(&resource_icon_image);
        resource_icons.set_filter(FilterMode::Linear);
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
        let leader_portraits = [
            [
                include_bytes!("../../assets/leader_portraits/rome/furious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/rome/annoyed.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/rome/cautious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/rome/polite.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/rome/gracious.png").as_slice(),
            ],
            [
                include_bytes!("../../assets/leader_portraits/greece/furious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/greece/annoyed.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/greece/cautious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/greece/polite.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/greece/gracious.png").as_slice(),
            ],
            [
                include_bytes!("../../assets/leader_portraits/egypt/furious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/egypt/annoyed.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/egypt/cautious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/egypt/polite.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/egypt/gracious.png").as_slice(),
            ],
            [
                include_bytes!("../../assets/leader_portraits/babylon/furious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/babylon/annoyed.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/babylon/cautious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/babylon/polite.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/babylon/gracious.png").as_slice(),
            ],
            [
                include_bytes!("../../assets/leader_portraits/china/furious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/china/annoyed.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/china/cautious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/china/polite.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/china/gracious.png").as_slice(),
            ],
            [
                include_bytes!("../../assets/leader_portraits/japan/furious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/japan/annoyed.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/japan/cautious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/japan/polite.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/japan/gracious.png").as_slice(),
            ],
        ]
        .into_iter()
        .map(|portraits| {
            portraits
                .into_iter()
                .map(|bytes| Texture2D::from_file_with_format(bytes, None))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
        let leader_avatar_badges = [
            include_bytes!("../../assets/leader_avatars/rome.png").as_slice(),
            include_bytes!("../../assets/leader_avatars/greece.png").as_slice(),
            include_bytes!("../../assets/leader_avatars/egypt.png").as_slice(),
            include_bytes!("../../assets/leader_avatars/babylon.png").as_slice(),
            include_bytes!("../../assets/leader_avatars/china.png").as_slice(),
            include_bytes!("../../assets/leader_avatars/japan.png").as_slice(),
        ]
        .into_iter()
        .map(|bytes| Texture2D::from_file_with_format(bytes, None))
        .collect::<Vec<_>>();
        for badge in &leader_avatar_badges {
            badge.set_filter(FilterMode::Linear);
        }
        for portraits in &leader_portraits {
            for portrait in portraits {
                portrait.set_filter(FilterMode::Linear);
            }
        }
        Ok(Self {
            civ3,
            goto,
            terrain,
            resource_icons,
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
            leader_avatar_badges,
            leader_portraits,
        })
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
#[path = "assets_tests.rs"]
mod native_unit_tests;
