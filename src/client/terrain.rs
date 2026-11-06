//! Amplio2 terrain matching and masked blending.
//!
//! Native reference: client/tilespec.c::build_tile_data,
//! fill_terrain_sprite_array, fill_terrain_sprite_blending, and
//! tileset_setup_tile_type; data/amplio2.tilespec and its terrain specs.
use super::*;

// Visual cardinal directions belong to the tileset, independently of the
// six movement directions of the isohex game map.
const CARDINAL: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
// Native direction4 order is North, South, East, West.
const BLEND_DIRS: [(i32, i32); 4] = [(0, -1), (0, 1), (1, 0), (-1, 0)];
const BLEND_OFFSETS: [(usize, usize); 4] = [(48, 0), (0, 24), (48, 24), (0, 0)];
const BLEND_CELL_W: usize = 98;
const BLEND_CELL_H: usize = 50;
const TERRAIN_ROWS: usize = 13;
const CORNERS: [([(i32, i32); 3], (f32, f32)); 4] = [
    ([(-1, 0), (-1, -1), (0, -1)], (24.0, 0.0)),
    ([(1, 0), (1, 1), (0, 1)], (24.0, 24.0)),
    ([(0, -1), (1, -1), (1, 0)], (48.0, 12.0)),
    ([(0, 1), (-1, 1), (-1, 0)], (0.0, 12.0)),
];

fn same_overlay(a: Terrain, b: Terrain) -> bool {
    match a {
        Terrain::Forest | Terrain::Jungle => matches!(b, Terrain::Forest | Terrain::Jungle),
        Terrain::Hills | Terrain::Mountains => matches!(b, Terrain::Hills | Terrain::Mountains),
        _ => false,
    }
}

fn overlay_cell(terrain: Terrain, neighbors: [Terrain; 4]) -> Option<(i32, i32)> {
    let mask = neighbors
        .into_iter()
        .enumerate()
        .fold(0, |mask, (bit, neighbor)| {
            mask | (i32::from(same_overlay(terrain, neighbor)) << bit)
        });
    match terrain {
        Terrain::Forest | Terrain::Jungle => Some((
            if terrain == Terrain::Forest { 4 } else { 2 } + mask / 8,
            mask % 8,
        )),
        Terrain::Hills | Terrain::Mountains => {
            // The hills/mountains specs arrange N/W across columns, E/S rows.
            Some((
                ((mask >> 1) & 1) * 2 + ((mask >> 2) & 1),
                (mask & 1) * 2 + ((mask >> 3) & 1),
            ))
        }
        _ => None,
    }
}

fn overlay_height(terrain: Terrain) -> f32 {
    if terrain == Terrain::Mountains {
        66.0
    } else {
        48.0
    }
}

fn overlay_top(center_y: f32, terrain: Terrain, zoom: f32) -> f32 {
    // Native tall mountain origin: tile_top + (48 - 72) + 6.
    // The 6px layer offset already compensates for its 66px image occupying
    // the bottom of a 72px full tile; bottom anchoring must not add it again.
    center_y + TILE_H * 0.5 * zoom - overlay_height(terrain) * zoom
}

fn blend_source(terrain: Terrain) -> Option<(usize, usize)> {
    match terrain {
        Terrain::DeepOcean => None,
        Terrain::Ocean | Terrain::Lake => Some((15, 3)),
        // terrain1.spec: t.blend.arctic is an explicit blender even though
        // tile_arctic has blend_layer=0. It paints ice onto neighbors only.
        Terrain::Glacier => Some((7, 0)),
        _ => terrain.sprite_row().map(|row| (row as usize, 0)),
    }
}

fn receives_blending(terrain: Terrain) -> bool {
    // These have blend_layer=0. Arctic nevertheless has an explicit blender,
    // so its ice can extend onto a neighboring land tile.
    !matches!(terrain, Terrain::DeepOcean | Terrain::Glacier)
}

// Native fill_terrain_sprite_layer + fill_terrain_sprite_blending: the
// receiving tile must enable blending; the neighbor may supply an explicit
// blender (arctic does) even when it does not itself receive blends.
fn blends_neighbor(current: Terrain, other: Terrain) -> bool {
    receives_blending(current) && current != other && blend_source(other).is_some()
}

fn custom_ice_pair(current: Terrain, other: Terrain) -> bool {
    current != other
        && !current.is_water()
        && !other.is_water()
        && (current == Terrain::Glacier || other == Terrain::Glacier)
}

// Deliberate visual styling: a six-pixel spatial stipple, independent of
// gameplay RNG. Clip to the diamond to avoid double-blended tile edges.
fn ice_dither_alpha(direction: usize, x: usize, y: usize) -> u8 {
    let nx = (x as f32 + 0.5 - 48.0) / 48.0;
    let ny = (y as f32 + 0.5 - 24.0) / 24.0;
    if nx.abs() + ny.abs() > 1.0 {
        return 0;
    }
    let toward_neighbor = [nx - ny, ny - nx, nx + ny, -nx - ny][direction];
    let t = (1.0 - (1.0 - toward_neighbor) * 24.0 / 6.0).clamp(0.0, 1.0);
    let coverage = (127.5 * t * t * (3.0 - 2.0 * t)).round() as u8;
    let mut hash = (x as u32).wrapping_mul(0x9e3779b9)
        ^ (y as u32).wrapping_mul(0x85ebca6b)
        ^ (direction as u32).wrapping_mul(0xc2b2ae35);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7feb352d);
    hash ^= hash >> 15;
    if (hash & 255) < u32::from(coverage) {
        255
    } else {
        0
    }
}

pub(super) fn build_ice_blend_atlas(terrain: &Image) -> Image {
    build_blend_atlas_impl(terrain, true)
}

fn blend_rect(terrain: Terrain, direction: usize) -> Rect {
    Rect::new(
        (direction * BLEND_CELL_W + 1) as f32,
        (terrain as usize * BLEND_CELL_H + 1) as f32,
        TILE_W,
        TILE_H,
    )
}

fn water_corner_cell(lake: bool, corner: usize, mask: i32) -> Rect {
    let (row, parity) = [(0, 0), (1, 0), (2, 1), (2, 0)][corner];
    Rect::new(
        (1 + (mask * 2 + parity) * 49) as f32,
        (437 + (row + if lake { 9 } else { 0 }) * 25) as f32,
        48.0,
        24.0,
    )
}

/// Build the exact native dither-masked quadrants once at asset load. Each
/// generated cell preserves its full tile origin and has a transparent gutter.
pub(super) fn build_blend_atlas(terrain: &Image) -> Image {
    build_blend_atlas_impl(terrain, false)
}

fn build_blend_atlas_impl(terrain: &Image, custom: bool) -> Image {
    let mut atlas = Image::gen_image_color(
        (4 * BLEND_CELL_W) as u16,
        (TERRAIN_ROWS * BLEND_CELL_H) as u16,
        BLANK,
    );
    let terrains = [
        Terrain::Ocean,
        Terrain::DeepOcean,
        Terrain::Lake,
        Terrain::Glacier,
        Terrain::Jungle,
        Terrain::Desert,
        Terrain::Forest,
        Terrain::Grassland,
        Terrain::Hills,
        Terrain::Mountains,
        Terrain::Plains,
        Terrain::Swamp,
        Terrain::Tundra,
    ];
    let source_pixels = terrain.get_image_data();
    let pixels = atlas.get_image_data_mut();
    for kind in terrains {
        let Some((row, col)) = blend_source(kind) else {
            continue;
        };
        let sx = 1 + col * 97;
        let sy = 1 + row * 49;
        for (direction, (ox, oy)) in BLEND_OFFSETS.into_iter().enumerate() {
            for y in 0..48 {
                for x in 0..96 {
                    if !custom && (!(oy..oy + 24).contains(&y) || !(ox..ox + 48).contains(&x)) {
                        continue;
                    }
                    let mut pixel = source_pixels[(sy + y) * terrain.width() + sx + x];
                    if custom && pixel[3] == 0 {
                        if let Some(nearest) = (0..96usize)
                            .filter(|&px| {
                                source_pixels[(sy + y) * terrain.width() + sx + px][3] > 0
                            })
                            .min_by_key(|&px| x.abs_diff(px))
                        {
                            pixel = source_pixels[(sy + y) * terrain.width() + sx + nearest];
                        }
                    }
                    // t.dither_tile is row 15, col 0. Native crop_sprite applies
                    // its alpha at the source-relative (x,y), not quadrant-local.
                    let mask = if custom {
                        ice_dither_alpha(direction, x, y)
                    } else {
                        source_pixels[(1 + 15 * 49 + y) * terrain.width() + 1 + x][3]
                    };
                    pixel[3] = ((pixel[3] as u16 * mask as u16) / 255) as u8;
                    let ax = direction * BLEND_CELL_W + 1 + x;
                    let ay = kind as usize * BLEND_CELL_H + 1 + y;
                    pixels[ay * 4 * BLEND_CELL_W + ax] = pixel;
                }
            }
        }
    }
    atlas
}

impl Game {
    fn matched_neighbor(&self, x: i32, y: i32, dx: i32, dy: i32, current: Terrain) -> Terrain {
        let (mx, my) = native_to_map(x, y);
        let (nx, ny) = map_to_native(mx + dx, my + dy);
        // build_tile_data repeats current terrain at unknown and unreal edges.
        let Some((nx, ny)) = self.sim.map_wrap.normalize(nx, ny) else {
            return current;
        };
        if !self.sim.tile(nx, ny).seen {
            current
        } else {
            self.sim.render_tile(nx, ny).terrain
        }
    }

    pub(super) fn draw_terrain_blending(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
    ) {
        let current = self.sim.render_tile(x, y).terrain;
        for (direction, (dx, dy)) in BLEND_DIRS.into_iter().enumerate() {
            let other = self.matched_neighbor(x, y, dx, dy, current);
            let custom = custom_ice_pair(current, other);
            if !custom && !blends_neighbor(current, other) {
                continue;
            }
            draw_texture_ex(
                if custom {
                    &assets.terrain_ice_blends
                } else {
                    &assets.terrain_blends
                },
                center.x - tile_w * 0.5,
                center.y - tile_h * 0.5,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(tile_w, tile_h)),
                    source: Some(blend_rect(other, direction)),
                    ..Default::default()
                },
            );
        }
    }

    pub(super) fn draw_matched_terrain_overlay(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
    ) {
        let terrain = self.sim.render_tile(x, y).terrain;
        let neighbors = CARDINAL.map(|(dx, dy)| self.matched_neighbor(x, y, dx, dy, terrain));
        if let Some((row, col)) = overlay_cell(terrain, neighbors) {
            let zoom = tile_w / TILE_W;
            let texture = match terrain {
                Terrain::Forest | Terrain::Jungle => &assets.terrain2,
                Terrain::Hills => &assets.hills,
                Terrain::Mountains => &assets.mountains,
                _ => unreachable!(),
            };
            let source_h = overlay_height(terrain);
            draw_texture_ex(
                texture,
                center.x - tile_w * 0.5,
                overlay_top(center.y, terrain, zoom),
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(tile_w, source_h * zoom)),
                    source: Some(isolated_sprite_source(row, col, 96.0, source_h)),
                    ..Default::default()
                },
            );
        }
        if matches!(terrain, Terrain::Ocean | Terrain::DeepOcean) {
            self.draw_ice_cliffs(assets, x, y, center, tile_w, tile_h, terrain);
        }
    }

    pub(super) fn draw_lake_sprite(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        w: f32,
        h: f32,
    ) {
        self.draw_water_corners(assets, x, y, center, w, h, Terrain::Lake, true);
    }

    fn draw_ice_cliffs(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        w: f32,
        h: f32,
        current: Terrain,
    ) {
        self.draw_water_corners(assets, x, y, center, w, h, current, false);
    }

    fn draw_water_corners(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        w: f32,
        h: f32,
        current: Terrain,
        lake: bool,
    ) {
        // Native CELL_CORNER / MATCH_PAIR: m0, m1, m2 follow the three
        // neighbors clockwise around each of u,d,r,l corners. Missing/unknown
        // neighbors repeat the water tile and therefore never reveal ice.
        for (corner, (neighbors, (ox, oy))) in CORNERS.into_iter().enumerate() {
            let mask = neighbors
                .into_iter()
                .enumerate()
                .fold(0, |mask, (bit, (dx, dy))| {
                    let neighbor = self.matched_neighbor(x, y, dx, dy, current);
                    // Lake layer0 matches land, while arctic layer0 deliberately
                    // has shallow match type. Ice cliffs match layer1 ice.
                    let matches = if lake {
                        !neighbor.is_water() && neighbor != Terrain::Glacier
                    } else {
                        neighbor == Terrain::Glacier
                    };
                    mask | (i32::from(matches) << bit)
                });
            if mask == 0 && !lake {
                continue;
            }
            draw_texture_ex(
                &assets.water,
                center.x - w * 0.5 + ox * w / TILE_W,
                center.y - h * 0.5 + oy * h / TILE_H,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(w * 0.5, h * 0.5)),
                    source: Some({
                        let original = water_corner_cell(lake, corner, mask);
                        isolated_sprite_source(
                            ((original.y - 437.0) / 25.0) as i32,
                            ((original.x - 1.0) / 49.0) as i32,
                            48.0,
                            24.0,
                        )
                    }),
                    ..Default::default()
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arctic_uses_the_reference_explicit_blender_and_one_way_blending() {
        // amplio2/terrain1.spec: row 7, col 0 = t.blend.arctic.
        // amplio2.tilespec: tile_arctic blend_layer=0; grassland blend_layer=1.
        assert_eq!(blend_source(Terrain::Glacier), Some((7, 0)));
        for land in [
            Terrain::Grassland,
            Terrain::Tundra,
            Terrain::Desert,
            Terrain::Swamp,
            Terrain::Plains,
        ] {
            assert!(blends_neighbor(land, Terrain::Glacier));
            assert!(!blends_neighbor(Terrain::Glacier, land));
        }
        assert!(!blends_neighbor(Terrain::Glacier, Terrain::Glacier));
        assert!(!blends_neighbor(Terrain::DeepOcean, Terrain::Glacier));
        assert!(blends_neighbor(Terrain::Ocean, Terrain::Glacier));
    }

    #[test]
    fn custom_ice_stipple_stays_inside_the_diamond_and_excludes_water() {
        assert!(custom_ice_pair(Terrain::Glacier, Terrain::Grassland));
        assert!(custom_ice_pair(Terrain::Tundra, Terrain::Glacier));
        for water in [Terrain::Ocean, Terrain::DeepOcean, Terrain::Lake] {
            assert!(!custom_ice_pair(Terrain::Glacier, water));
            assert!(!custom_ice_pair(water, Terrain::Glacier));
        }
        assert!(!custom_ice_pair(Terrain::Glacier, Terrain::Glacier));
        assert!(!custom_ice_pair(Terrain::Tundra, Terrain::Grassland));
        let mut opaque = 0;
        for direction in 0..4 {
            for y in 0..48 {
                for x in 0..96 {
                    let alpha = ice_dither_alpha(direction, x, y);
                    assert!(alpha == 0 || alpha == 255);
                    let nx = (x as f32 + 0.5 - 48.0) / 48.0;
                    let ny = (y as f32 + 0.5 - 24.0) / 24.0;
                    if nx.abs() + ny.abs() > 1.0 {
                        assert_eq!(alpha, 0);
                    }
                    opaque += usize::from(alpha == 255);
                }
            }
            assert_eq!(ice_dither_alpha(direction, 48, 24), 0);
        }
        assert!(opaque > 50 && opaque < 1000, "keep the stipple narrow");
    }

    #[test]
    fn every_connected_sprite_matches_the_original_spec_tag() {
        for terrain in [
            Terrain::Forest,
            Terrain::Jungle,
            Terrain::Hills,
            Terrain::Mountains,
        ] {
            let spec = match terrain {
                Terrain::Forest | Terrain::Jungle => {
                    include_str!("../../assets/amplio2_terrain2.spec")
                }
                Terrain::Hills => include_str!("../../assets/amplio2_hills.spec"),
                _ => include_str!("../../assets/amplio2_mountains.spec"),
            };
            for mask in 0..16 {
                let neighbors = std::array::from_fn(|bit| {
                    if mask & (1 << bit) != 0 {
                        terrain
                    } else {
                        Terrain::Desert
                    }
                });
                let (row, col) = overlay_cell(terrain, neighbors).unwrap();
                let name = match terrain {
                    Terrain::Forest => "forest",
                    Terrain::Jungle => "jungle",
                    Terrain::Hills => "hills",
                    _ => "mountains",
                };
                let tag = format!(
                    "t.l1.{name}_n{}e{}s{}w{}",
                    mask & 1,
                    (mask >> 1) & 1,
                    (mask >> 2) & 1,
                    (mask >> 3) & 1
                );
                let line = spec
                    .lines()
                    .find(|line| line.contains(&format!("\"{tag}\"")))
                    .unwrap();
                let parts: Vec<_> = line.split(',').collect();
                assert_eq!(
                    (row, col),
                    (
                        parts[0].trim().parse::<i32>().unwrap(),
                        parts[1].trim().parse::<i32>().unwrap()
                    ),
                    "{tag}"
                );
            }
        }
    }
    #[test]
    fn mixed_forests_and_hill_ranges_share_reference_match_types() {
        assert_eq!(
            overlay_cell(Terrain::Forest, [Terrain::Jungle; 4]),
            Some((5, 7))
        );
        assert_eq!(
            overlay_cell(Terrain::Mountains, [Terrain::Hills; 4]),
            Some((3, 3))
        );
        assert!(!same_overlay(Terrain::Forest, Terrain::Hills));
    }

    #[test]
    fn mountain_anchor_matches_native_full_tile_offset_and_layer_offset() {
        // amplio2.tilespec: normal 96x48; native full tile height 3*48/2;
        // tile_mountains.layer1_offset_y=6; mountains.spec: height=66.
        for zoom in [MIN_ZOOM, START_ZOOM, 1.0, MAX_ZOOM] {
            let center_y = 100.0;
            let tile_top = center_y - 24.0 * zoom;
            let native_top = tile_top + (48.0 - 72.0 + 6.0) * zoom;
            assert!((overlay_top(center_y, Terrain::Mountains, zoom) - native_top).abs() < 0.0001);
            assert!(
                (native_top + overlay_height(Terrain::Mountains) * zoom - (center_y + 24.0 * zoom))
                    .abs()
                    < 0.0001
            );
        }
    }
    #[test]
    fn ice_and_lake_corner_cells_match_original_water_spec() {
        let spec = include_str!("../../assets/amplio2_water.spec");
        for lake in [false, true] {
            for (corner, letter) in ['u', 'd', 'r', 'l'].into_iter().enumerate() {
                for mask in 0..8 {
                    let state = |bit| {
                        if mask & (1 << bit) != 0 {
                            if lake { 'l' } else { 'i' }
                        } else {
                            if lake { 's' } else { 'w' }
                        }
                    };
                    let tag = format!(
                        "t.l{}.{}_cell_{letter}_{}_{}_{}",
                        if lake { 0 } else { 1 },
                        if lake { "lake" } else { "coast" },
                        state(0),
                        state(1),
                        state(2)
                    );
                    let line = spec
                        .lines()
                        .find(|line| line.contains(&format!("\"{tag}\"")))
                        .unwrap();
                    let parts: Vec<_> = line.split(',').collect();
                    let row = parts[0].trim().parse::<i32>().unwrap();
                    let col = parts[1].trim().parse::<i32>().unwrap();
                    let actual = water_corner_cell(lake, corner, mask);
                    assert_eq!(
                        (actual.x, actual.y),
                        ((1 + col * 49) as f32, (437 + row * 25) as f32),
                        "{tag}"
                    );
                }
            }
        }
    }
    #[test]
    fn blend_atlas_matches_reference_crop_and_dither_pixel_for_pixel() {
        let image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_terrain1.png"), None)
                .unwrap();
        let atlas = build_blend_atlas(&image);
        for kind in [
            Terrain::Ocean,
            Terrain::Lake,
            Terrain::Glacier,
            Terrain::Desert,
            Terrain::Grassland,
            Terrain::Forest,
            Terrain::Jungle,
            Terrain::Hills,
            Terrain::Mountains,
            Terrain::Plains,
            Terrain::Swamp,
            Terrain::Tundra,
        ] {
            let (row, col) = blend_source(kind).unwrap();
            for (direction, (ox, oy)) in BLEND_OFFSETS.into_iter().enumerate() {
                for y in 0..48 {
                    for x in 0..96 {
                        let pixel = atlas.get_image_data()[(kind as usize * BLEND_CELL_H + 1 + y)
                            * atlas.width()
                            + direction * BLEND_CELL_W
                            + 1
                            + x];
                        if (ox..ox + 48).contains(&x) && (oy..oy + 24).contains(&y) {
                            let mut expected = image.get_image_data()
                                [(1 + row * 49 + y) * image.width() + 1 + col * 97 + x];
                            let mask = image.get_image_data()
                                [(1 + 15 * 49 + y) * image.width() + 1 + x][3];
                            expected[3] = ((expected[3] as u16 * mask as u16) / 255) as u8;
                            assert_eq!(pixel, expected);
                        } else {
                            assert_eq!(pixel, [0, 0, 0, 0]);
                        }
                    }
                }
            }
        }
        assert!(!receives_blending(Terrain::Glacier));
        assert!(!receives_blending(Terrain::DeepOcean));
        assert!(blend_source(Terrain::Glacier).is_some());
        assert!(blend_source(Terrain::DeepOcean).is_none());
    }
}
