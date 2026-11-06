//! Native mapview_common.c::get_mapview_scroll_window / calc_mapview_origin.
//! Preserve absolute GUI arithmetic, including native integer rounding when
//! expanding the scroll window for viewports larger than the map.
use super::*;

// data/misc/colors.tilespec: mapview_unknown = (0,0,0).
pub(super) const MAP_UNKNOWN: Color = BLACK;

pub(super) fn vertical_scroll_bounds(view_height: f32, zoom: f32) -> (f32, f32) {
    let (mx, my) = native_to_map(0, 0);
    let mut min = (mx + my) as f32 * TILE_H * 0.5 * zoom;
    let (mx, my) = native_to_map(MAP_W - 1, MAP_H - 1);
    let mut max = (mx + my) as f32 * TILE_H * 0.5 * zoom + TILE_H * zoom;
    // Native uses an integer diff and splits the expansion with integer division.
    let diff = (view_height - (max - min)) as i32;
    if diff > 0 {
        min -= (diff / 2) as f32;
        max += ((diff + 1) / 2) as f32;
    }
    (min, max - view_height)
}

pub(super) fn normalize_camera_y(wrap: MapWrap, center_y: f32, view_height: f32, zoom: f32) -> f32 {
    let step = TILE_H * 0.5 * zoom;
    // Convert our center-in-native-row coordinates to the reference GUI origin.
    let first = MAP_W as f32 * TILE_H * 0.5 * zoom;
    let origin = first + center_y * TILE_H * 0.5 * zoom + TILE_H * zoom * 0.5 - view_height * 0.5;
    let origin = if wrap.wraps_y() {
        first + (origin - first).rem_euclid(MAP_H as f32 * step)
    } else {
        let (min, max) = vertical_scroll_bounds(view_height, zoom);
        // CLIP is MAX(min, MIN(value,max)), including subpixel oversized maps.
        origin.min(max).max(min)
    };
    (origin - first + view_height * 0.5 - TILE_H * zoom * 0.5) / step
}

#[cfg(test)]
mod tests {
    use super::*;

    // Literal translation of native get_mapview_scroll_window, using absolute
    // map_to_gui_pos(NATIVE_TO_MAP_POS(...)) rather than the production helper.
    fn reference_scroll_origin(y: f32, view: f32, zoom: f32, wrap: MapWrap) -> f32 {
        let (mx, my) = native_to_map(0, 0);
        let first = (mx + my) as f32 * 24.0 * zoom;
        let (mx, my) = native_to_map(MAP_W - 1, MAP_H - 1);
        let mut last = (mx + my) as f32 * 24.0 * zoom + 48.0 * zoom;
        let mut min = first;
        let origin = first + y * 24.0 * zoom + 24.0 * zoom - view / 2.0;
        if wrap.wraps_y() {
            return first + (origin - first).rem_euclid(MAP_H as f32 * 24.0 * zoom);
        }
        let diff = (view - (last - min)) as i32;
        if diff > 0 {
            min -= (diff / 2) as f32;
            last += ((diff + 1) / 2) as f32;
        }
        origin.min(last - view).max(min)
    }

    #[test]
    fn polar_scroll_matches_native_absolute_gui_bounds_at_all_sizes_and_zooms() {
        for view in [426.0, 550.0, 726.0, 1560.0, 1700.0, 2800.0] {
            for zoom in [MIN_ZOOM, START_ZOOM, 1.0, MAX_ZOOM, 1.5] {
                for y in [-1000.25, -0.2, 0.0, 23.4, 63.0, 64.3, 1000.25] {
                    for wrap in [MapWrap::X, MapWrap::XY] {
                        let result = normalize_camera_y(wrap, y, view, zoom);
                        let actual =
                            MAP_W as f32 * 24.0 * zoom + result * 24.0 * zoom + 24.0 * zoom
                                - view / 2.0;
                        let expected = reference_scroll_origin(y, view, zoom, wrap);
                        // Large virtual origins differ slightly from absolute native
                        // f32 math through cancellation; require < 1/50 pixel.
                        assert!(
                            (actual - expected).abs() < 0.02,
                            "{wrap} view={view} zoom={zoom} y={y}: {actual} vs {expected}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn poles_use_half_normal_tile_clearance_and_no_custom_sprite_margin() {
        let view = 550.0;
        let north = normalize_camera_y(MapWrap::X, -1000.0, view, 1.0);
        let south = normalize_camera_y(MapWrap::X, 1000.0, view, 1.0);
        assert!((view / 2.0 - north * 24.0 - 24.0).abs() < 0.001);
        assert!(
            (view / 2.0 + (MAP_H - 1) as f32 * 24.0 - south * 24.0 - (view - 24.0)).abs() < 0.001
        );
        assert_eq!(
            (MAP_UNKNOWN.r, MAP_UNKNOWN.g, MAP_UNKNOWN.b, MAP_UNKNOWN.a),
            (0.0, 0.0, 0.0, 1.0)
        );
    }

    #[test]
    fn torus_origin_preserves_fractional_pixels_and_never_clamps_at_a_pole() {
        for y in [-0.25, 0.25, MAP_H as f32 - 0.25] {
            let a = normalize_camera_y(MapWrap::XY, y, 550.0, 0.82);
            let b = normalize_camera_y(MapWrap::XY, y + MAP_H as f32, 550.0, 0.82);
            assert!((a - b).abs() < 0.0001);
            assert!((normalize_camera_y(MapWrap::XY, a, 550.0, 0.82) - a).abs() < 0.0001);
        }
    }
}
