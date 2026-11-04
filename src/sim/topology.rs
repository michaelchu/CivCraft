use super::{PERCENT_SCALE, PLAYER_COUNT};

const TILES_PER_PLAYER: i32 = 100;
pub(crate) const LAND_PERCENT: i32 = 30;
pub(crate) const MAP_TARGET_TILES: i32 =
    PLAYER_COUNT as i32 * TILES_PER_PLAYER * PERCENT_SCALE / LAND_PERCENT;
const MAP_X_RATIO: i32 = 1;
const MAP_Y_RATIO: i32 = 1;
const MAP_EVEN_FACTOR: i32 = 2;
const MAP_ISO_FACTOR: i32 = 2;

// Native _set_sizes at 0x100031f5c truncates
// sqrt(size / (xratio * yratio * iso * even^2)) + 0.49.  This integer
// comparison expresses the same 0.49 threshold at compile time, so the
// dimensions remain derived from the ruleset inputs instead of copied out.
const fn source_map_linear_scale(requested_tiles: i32) -> i32 {
    let denominator =
        (MAP_X_RATIO * MAP_Y_RATIO * MAP_ISO_FACTOR * MAP_EVEN_FACTOR * MAP_EVEN_FACTOR) as i64;
    let requested_scaled = requested_tiles as i64 * 10_000;
    let mut scale = 1_i64;
    loop {
        let next = scale + 1;
        let threshold = next * 100 - 49;
        if denominator * threshold * threshold > requested_scaled {
            return scale as i32;
        }
        scale = next;
    }
}

pub(super) const MAP_LINEAR_SCALE: i32 = source_map_linear_scale(MAP_TARGET_TILES);
pub(crate) const MAP_W: i32 = MAP_X_RATIO * MAP_LINEAR_SCALE * MAP_EVEN_FACTOR;
pub(crate) const MAP_H: i32 = MAP_Y_RATIO * MAP_LINEAR_SCALE * MAP_EVEN_FACTOR * MAP_ISO_FACTOR;
pub(crate) const LAND_TILE_TARGET: usize = (MAP_W * MAP_H * LAND_PERCENT / PERCENT_SCALE) as usize;

// Civ2Civ3 uses isometric hexes: northeast and southwest are the two
// unavailable directions in Freeciv's eight-direction coordinate system.
pub(crate) const ISOHEX_DIRS: [(i32, i32); 6] =
    [(0, -1), (1, 0), (1, 1), (0, 1), (-1, 0), (-1, -1)];

pub(crate) fn isohex_vector_distance(dx: i32, dy: i32) -> i32 {
    let abs_dx = dx.abs();
    let abs_dy = dy.abs();
    if dx.signum() != dy.signum() && dx != 0 && dy != 0 {
        abs_dx + abs_dy
    } else {
        abs_dx.max(abs_dy)
    }
}

pub(crate) fn is_isohex_step(dx: i32, dy: i32) -> bool {
    ISOHEX_DIRS.contains(&(dx, dy))
}

pub(crate) fn native_to_map(native_x: i32, native_y: i32) -> (i32, i32) {
    let map_x = (native_y + native_y.rem_euclid(2)) / 2 + native_x;
    let map_y = native_y - map_x + MAP_W;
    (map_x, map_y)
}

pub(crate) fn map_to_native(map_x: i32, map_y: i32) -> (i32, i32) {
    let native_y = map_x + map_y - MAP_W;
    let native_x = (2 * map_x - native_y - native_y.rem_euclid(2)) / 2;
    (native_x, native_y)
}

pub(crate) fn wrap_native(native_x: i32, native_y: i32) -> (i32, i32) {
    (native_x.rem_euclid(MAP_W), native_y.rem_euclid(MAP_H))
}

pub(crate) fn map_pos_to_native(map_x: i32, map_y: i32) -> (i32, i32) {
    let (native_x, native_y) = map_to_native(map_x, map_y);
    wrap_native(native_x, native_y)
}

pub(crate) fn offset_native_by_map(native_x: i32, native_y: i32, dx: i32, dy: i32) -> (i32, i32) {
    let (map_x, map_y) = native_to_map(native_x, native_y);
    map_pos_to_native(map_x + dx, map_y + dy)
}

fn wrapped_native_delta(delta: i32, size: i32) -> i32 {
    (delta + size / 2).rem_euclid(size) - size / 2
}

pub(crate) fn wrapped_map_vector(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> (i32, i32) {
    // Freeciv wraps in native coordinates first, then converts the resulting
    // virtual endpoint back to map coordinates before measuring distance.
    let native_dx = wrapped_native_delta(to_x - from_x, MAP_W);
    let native_dy = wrapped_native_delta(to_y - from_y, MAP_H);
    let (from_map_x, from_map_y) = native_to_map(from_x, from_y);
    let (to_map_x, to_map_y) = native_to_map(from_x + native_dx, from_y + native_dy);
    (to_map_x - from_map_x, to_map_y - from_map_y)
}
