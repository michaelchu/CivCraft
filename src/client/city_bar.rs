//! City-bar timing mirrors common/city.c's turns_to_build/turns_to_grow.
use super::*;

pub(super) fn label_top(center_y: f32, zoom: f32) -> f32 {
    // Keep a stable anchor with or without focus: the complete tile-sized
    // selection sprite extends farther down than either the unit or city.
    center_y + TILE_H * 0.5 * zoom + (4.0 * zoom).max(3.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn city_label_clears_the_selection_footprint_at_every_supported_zoom() {
        for zoom in [ZOOMED_OUT, START_ZOOM, ZOOMED_IN, MAX_ZOOM, 2.0] {
            assert!(label_top(100.0, zoom) >= 100.0 + TILE_H * zoom * 0.5 + 3.0);
        }
    }
}
