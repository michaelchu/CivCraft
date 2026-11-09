//! Reference six-direction river artwork adapted to the existing diamond view.
//! Hexemplio's hex-side is 16 in a 126x64 cell. Its cropped hex footprint is
//! narrowed per scanline into a 96x48 diamond, including the NW/SE tips.
use super::*;
const SOURCE_W: usize = 126;
const SOURCE_H: usize = 64;
const CIV3_RIVER_CELL_W: usize = 128;
const CIV3_RIVER_CELL_H: usize = 64;
const HEX_SIDE: f32 = 16.0;
const WIDTH: usize = TILE_SPRITE_W;
const HEIGHT: usize = TILE_SPRITE_H;
const SAMPLES: usize = 4;
const CELL_W: usize = WIDTH + 2;
const CELL_H: usize = HEIGHT + 2;
const MOUTH_COLUMNS: [usize; 6] = [0, 1, 5, 2, 3, 4];

/// OpenCiv3's junction sheet uses four surrounding tiles: north, east,
/// south, and west. Bit positions match Civ III's N, NE, E, SE, S, SW, W, NW
/// edge flags. The source reference has known generator approximations.
pub(super) fn civ3_junction_index(north: u8, east: u8, south: u8, west: u8) -> Option<usize> {
    let bit = |edges: u8, direction: usize| edges & (1 << direction) != 0;
    let mut index = 0;
    if bit(north, 5) || bit(west, 1) {
        index |= 1;
    }
    if bit(east, 7) || bit(north, 3) {
        index |= 2;
    }
    if bit(west, 3) || bit(south, 7) {
        index |= 4;
    }
    if bit(south, 1) || bit(east, 5) {
        index |= 8;
    }
    let point = bit(north, 4) || bit(east, 6) || bit(west, 2) || bit(south, 0);
    (index != 0 || point).then_some(index)
}

pub(super) fn civ3_river_sheet(index: usize, has_water: bool) -> Civ3RiverSheet {
    let diagonal_only = matches!(index, 1 | 2 | 4 | 8);
    if diagonal_only && !has_water {
        Civ3RiverSheet::Mountain
    } else {
        Civ3RiverSheet::Delta
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Civ3RiverSheet {
    Delta,
    Mountain,
}

pub(super) fn civ3_river_sprite(index: usize) -> Rect {
    Rect::new(
        (index % 4 * CIV3_RIVER_CELL_W) as f32,
        (index / 4 * CIV3_RIVER_CELL_H) as f32,
        CIV3_RIVER_CELL_W as f32,
        CIV3_RIVER_CELL_H as f32,
    )
}

#[derive(Clone, Copy, Default)]
pub(super) struct RiverNeighbor {
    pub known: bool,
    pub river: bool,
    pub water: bool,
}
/// Clockwise N,E,SE,S,W,NW, matching Freeciv's isohex cardinal directions.
pub(super) fn connection_mask(water: bool, neighbors: [RiverNeighbor; 6]) -> u8 {
    neighbors.into_iter().enumerate().fold(0, |mask, (dir, n)| {
        if n.known && (n.river || (!water && n.water)) {
            mask | (1 << dir)
        } else {
            mask
        }
    })
}
/// The spec's cells are ordered N,E,S,W,SE,NW, not clockwise mask order.
fn source_index(mask: u8) -> usize {
    let mut index = 0;
    for (direction, bit) in [0, 1, 4, 2, 3, 5].into_iter().enumerate() {
        if mask & (1 << direction) != 0 {
            index |= 1 << bit;
        }
    }
    index
}
pub(super) fn river_sprite(mask: u8) -> Rect {
    Rect::new(
        (1 + mask as usize % 8 * CELL_W) as f32,
        (1 + mask as usize / 8 * CELL_H) as f32,
        WIDTH as f32,
        HEIGHT as f32,
    )
}
pub(super) fn mouth_sprite(direction: usize) -> Rect {
    Rect::new(
        (1 + direction * CELL_W) as f32,
        (1 + 8 * CELL_H) as f32,
        WIDTH as f32,
        HEIGHT as f32,
    )
}

pub(super) fn build_river_atlas(rivers: &Image) -> Image {
    let mut atlas = Image::gen_image_color((8 * CELL_W) as u16, (9 * CELL_H) as u16, BLANK);
    for mask in 0..64 {
        let index = source_index(mask);
        adapt_cell(
            &mut atlas,
            mask as usize % 8,
            mask as usize / 8,
            rivers,
            index % 8,
            index / 8,
        );
    }
    for (direction, column) in MOUTH_COLUMNS.into_iter().enumerate() {
        adapt_cell(&mut atlas, direction, 8, rivers, column, 8);
    }
    atlas
}
fn adapt_cell(
    atlas: &mut Image,
    target_x: usize,
    target_y: usize,
    source: &Image,
    col: usize,
    row: usize,
) {
    let source_pixels = source.get_image_data();
    let source_stride = source.width();
    let atlas_stride = atlas.width();
    let output = atlas.get_image_data_mut();
    let origin = (1 + col * (SOURCE_W + 1), 1 + row * (SOURCE_H + 1));
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let mut alpha = 0_u32;
            let mut channels = [0_u32; 3];
            // Area samples preserve narrow river channels when the hex's flat tip
            // collapses to a diamond point. Accumulate premultiplied colors so the
            // transparent atlas background cannot leave black fringes.
            for sample_y in 0..SAMPLES {
                for sample_x in 0..SAMPLES {
                    let px = x as f32 + (sample_x as f32 + 0.5) / SAMPLES as f32;
                    let py = y as f32 + (sample_y as f32 + 0.5) / SAMPLES as f32;
                    let height_fraction =
                        1.0 - ((py - HEIGHT as f32 * 0.5) / (HEIGHT as f32 * 0.5)).abs();
                    let target_half = WIDTH as f32 * 0.5 * height_fraction;
                    let offset = px - WIDTH as f32 * 0.5;
                    if offset.abs() > target_half {
                        continue;
                    }
                    let source_half =
                        HEX_SIDE * 0.5 + (SOURCE_W as f32 * 0.5 - HEX_SIDE) * height_fraction;
                    let sx = (SOURCE_W as f32 * 0.5 + offset / target_half * source_half)
                        .floor()
                        .clamp(0.0, (SOURCE_W - 1) as f32) as usize;
                    let sy = (py * SOURCE_H as f32 / HEIGHT as f32)
                        .floor()
                        .min((SOURCE_H - 1) as f32) as usize;
                    let pixel = source_pixels[(origin.1 + sy) * source_stride + origin.0 + sx];
                    alpha += pixel[3] as u32;
                    for channel in 0..3 {
                        channels[channel] += pixel[channel] as u32 * pixel[3] as u32;
                    }
                }
            }
            if alpha > 0 {
                let mut pixel = [0_u8; 4];
                for channel in 0..3 {
                    pixel[channel] = (channels[channel] / alpha) as u8;
                }
                pixel[3] = (alpha / (SAMPLES * SAMPLES) as u32) as u8;
                output[(1 + target_y * CELL_H + y) * atlas_stride + 1 + target_x * CELL_W + x] =
                    pixel;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civ3_junction_uses_reciprocal_edge_flags() {
        assert_eq!(civ3_junction_index(1 << 5, 0, 0, 0), Some(1));
        assert_eq!(civ3_junction_index(0, 1 << 7, 0, 0), Some(2));
        assert_eq!(civ3_junction_index(0, 0, 0, 1 << 3), Some(4));
        assert_eq!(civ3_junction_index(0, 0, 1 << 1, 0), Some(8));
        assert_eq!(civ3_junction_index(0, 0, 0, 0), None);
        assert_eq!(civ3_junction_index(1 << 4, 0, 0, 0), Some(0));
    }

    #[test]
    fn civ3_river_masks_select_full_128_by_64_atlas_cells() {
        assert_eq!(civ3_river_sprite(0), Rect::new(0.0, 0.0, 128.0, 64.0));
        assert_eq!(civ3_river_sprite(3), Rect::new(384.0, 0.0, 128.0, 64.0));
        assert_eq!(civ3_river_sprite(12), Rect::new(0.0, 192.0, 128.0, 64.0));
        assert_eq!(civ3_river_sprite(15), Rect::new(384.0, 192.0, 128.0, 64.0));
    }

    #[test]
    fn civ3_selects_delta_art_at_water_mouths_and_mountain_art_inland() {
        assert_eq!(civ3_river_sheet(1, true), Civ3RiverSheet::Delta);
        assert_eq!(civ3_river_sheet(1, false), Civ3RiverSheet::Mountain);
        assert_eq!(civ3_river_sheet(3, false), Civ3RiverSheet::Delta);
    }

    fn atlas() -> &'static Image {
        static ATLAS: std::sync::OnceLock<Image> = std::sync::OnceLock::new();
        ATLAS.get_or_init(|| {
            build_river_atlas(
                &Image::from_file_with_format(
                    include_bytes!("../../assets/hexemplio_rivers.png"),
                    None,
                )
                .unwrap(),
            )
        })
    }
    #[test]
    fn all_64_masks_follow_the_reference_sprite_tags() {
        let spec = include_str!("../../assets/hexemplio_rivers.spec");
        for mask in 0..64_u8 {
            let tag = format!(
                "road.river_s_n{}e{}se{}s{}w{}nw{}:0",
                mask & 1,
                (mask >> 1) & 1,
                (mask >> 2) & 1,
                (mask >> 3) & 1,
                (mask >> 4) & 1,
                (mask >> 5) & 1
            );
            let line = spec.lines().find(|line| line.contains(&tag)).unwrap();
            let coordinates: Vec<_> = line
                .split(',')
                .take(2)
                .map(|v| v.trim().parse::<usize>().unwrap())
                .collect();
            assert_eq!(
                (source_index(mask) / 8, source_index(mask) % 8),
                (coordinates[0], coordinates[1])
            );
        }
    }
    #[test]
    fn all_six_outlets_follow_the_reference_sprite_tags() {
        let spec = include_str!("../../assets/hexemplio_rivers.spec");
        for (direction, name) in ["n", "e", "se", "s", "w", "nw"].into_iter().enumerate() {
            let line = spec
                .lines()
                .find(|line| line.contains(&format!("\"road.river_outlet_{name}:0\"")))
                .unwrap();
            let coordinates: Vec<_> = line
                .split(',')
                .take(2)
                .map(|v| v.trim().parse::<usize>().unwrap())
                .collect();
            assert_eq!(
                (8, MOUTH_COLUMNS[direction]),
                (coordinates[0], coordinates[1])
            );
        }
    }
    #[test]
    fn fog_and_water_do_not_invent_river_connections() {
        let river = RiverNeighbor {
            known: true,
            river: true,
            water: false,
        };
        let lake = RiverNeighbor {
            known: true,
            river: false,
            water: true,
        };
        let hidden = RiverNeighbor {
            known: false,
            river: true,
            water: true,
        };
        let neighbors = [river, lake, hidden, RiverNeighbor::default(), river, lake];
        assert_eq!(connection_mask(false, neighbors), 0b110011);
        assert_eq!(connection_mask(true, neighbors), 0b010001);
    }
    fn edge_ink(sprite: Rect, direction: usize) -> usize {
        let (x, y) = [(72, 12), (72, 36), (48, 47), (24, 36), (24, 12), (48, 0)][direction];
        let atlas = atlas();
        let pixels = atlas.get_image_data();
        let mut count = 0;
        for dy in -4..=4 {
            for dx in -4..=4 {
                let (x, y) = (x + dx, y + dy);
                if x >= 0
                    && y >= 0
                    && x < WIDTH as i32
                    && y < HEIGHT as i32
                    && pixels[(sprite.y as usize + y as usize) * atlas.width()
                        + sprite.x as usize
                        + x as usize][3]
                        > 0
                {
                    count += 1;
                }
            }
        }
        count
    }
    #[test]
    fn adapted_channels_and_mouths_reach_all_six_connections() {
        for direction in 0..6 {
            assert!(
                edge_ink(river_sprite(1 << direction), direction) > 0,
                "river missing direction {direction}"
            );
            assert!(
                edge_ink(mouth_sprite(direction), direction) > 0,
                "outlet missing direction {direction}"
            );
        }
    }
    #[test]
    fn diagonal_rivers_and_reciprocal_outlets_reach_the_shared_tip() {
        let image = atlas();
        let pixels = image.get_image_data();
        for (direction, row, opposite_row) in [(5, 0, HEIGHT - 1), (2, HEIGHT - 1, 0)] {
            let river = river_sprite(1 << direction);
            let mouth = mouth_sprite((direction + 3) % 6);
            let alpha_at = |sprite: Rect, y: usize| {
                (WIDTH / 2 - 1..=WIDTH / 2)
                    .map(|x| {
                        pixels[(sprite.y as usize + y) * image.width() + sprite.x as usize + x][3]
                            as u32
                    })
                    .sum::<u32>()
            };
            assert!(
                alpha_at(river, row) > 0,
                "river misses shared tip for direction {direction}"
            );
            assert!(
                alpha_at(mouth, opposite_row) > 0,
                "mouth misses shared tip for direction {direction}"
            );
        }
    }
    #[test]
    fn adaptation_keeps_art_inside_the_diamond_without_atlas_grid_lines() {
        let a = atlas();
        let pixels = a.get_image_data();
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                if ((x as f32 + 0.5 - 48.0) / 48.0).abs() + ((y as f32 + 0.5 - 24.0) / 24.0).abs()
                    > 1.08
                {
                    for mask in 0..64 {
                        assert_eq!(
                            pixels[(1 + mask / 8 * CELL_H + y) * a.width()
                                + 1
                                + mask % 8 * CELL_W
                                + x][3],
                            0
                        );
                    }
                }
            }
        }
        assert!(
            !pixels
                .iter()
                .any(|p| p[0] == 0 && p[1] == 255 && p[2] == 0 && p[3] > 0)
        );
    }
}
