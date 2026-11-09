//! Civ III river junctions on the staggered map grid.

use macroquad::prelude::Rect;

const CIV3_RIVER_CELL_W: usize = 128;
const CIV3_RIVER_CELL_H: usize = 64;

/// OpenCiv3's junction sheet uses north, east, south, and west tile states.
/// Edge bits follow Civ III's N, NE, E, SE, S, SW, W, NW order.
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
}
