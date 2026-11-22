//! Civ III river junctions on the staggered map grid.

use macroquad::prelude::Rect;

/// Civ III river sprite sheet cell size.
const RIVER_CELL_W: usize = 128;
/// Civ III river sprite sheet cell size.
const RIVER_CELL_H: usize = 64;

/// OpenCiv3's junction sheet uses north, east, south, and west tile states.
/// Edge bits follow Civ III's N, NE, E, SE, S, SW, W, NW order.
///
/// Uses the Civ III river sprite sheet layout.
pub(super) fn river_junction_index(north: u8, east: u8, south: u8, west: u8) -> Option<usize> {
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

/// Uses the Civ III river sprite sheet layout.
pub(super) fn river_sheet_cell(index: usize, is_mouth: bool) -> RiverSheet {
    let diagonal_only = matches!(index, 1 | 2 | 4 | 8);
    if diagonal_only && is_mouth {
        RiverSheet::Delta
    } else {
        RiverSheet::Mountain
    }
}

/// Civ III river sprite sheets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RiverSheet {
    Delta,
    Mountain,
}

/// Uses the Civ III river sprite sheet layout.
pub(super) fn river_sprite(index: usize) -> Rect {
    Rect::new(
        (index % 4 * RIVER_CELL_W) as f32,
        (index / 4 * RIVER_CELL_H) as f32,
        RIVER_CELL_W as f32,
        RIVER_CELL_H as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors Civ III behavior.
    #[test]
    fn native_junction_uses_reciprocal_edge_flags() {
        assert_eq!(river_junction_index(1 << 5, 0, 0, 0), Some(1));
        assert_eq!(river_junction_index(0, 1 << 7, 0, 0), Some(2));
        assert_eq!(river_junction_index(0, 0, 0, 1 << 3), Some(4));
        assert_eq!(river_junction_index(0, 0, 1 << 1, 0), Some(8));
        assert_eq!(river_junction_index(0, 0, 0, 0), None);
        assert_eq!(river_junction_index(1 << 4, 0, 0, 0), Some(0));
    }

    /// Mirrors Civ III behavior.
    #[test]
    fn native_river_masks_select_full_128_by_64_atlas_cells() {
        assert_eq!(river_sprite(0), Rect::new(0.0, 0.0, 128.0, 64.0));
        assert_eq!(river_sprite(3), Rect::new(384.0, 0.0, 128.0, 64.0));
        assert_eq!(river_sprite(12), Rect::new(0.0, 192.0, 128.0, 64.0));
        assert_eq!(river_sprite(15), Rect::new(384.0, 192.0, 128.0, 64.0));
    }

    /// Mirrors Civ III behavior.
    #[test]
    fn native_selects_delta_art_only_for_actual_mouth_junctions() {
        assert_eq!(river_sheet_cell(1, true), RiverSheet::Delta);
        assert_eq!(river_sheet_cell(1, false), RiverSheet::Mountain);
        assert_eq!(river_sheet_cell(3, true), RiverSheet::Mountain);
    }
}
