//! Shared map geometry and coordinate transforms for generation, rules, and rendering.
//!
//! Map coordinates are adapted to Civ III's staggered square grid. Conversion,
//! neighbor offsets, distance, and [`MapWrap`] live here so callers use one
//! definition of adjacency and seam behavior. Campaign maps wrap east/west and
//! treat polar rows as bounded edges.
//!
//! Normalize coordinates through the selected topology before indexing map
//! arrays. The topology is part of the generated campaign and must be chosen
//! before generation rather than changed after tiles and features are built.

use super::{DEFAULT_CIVILIZATION_COUNT, PERCENT_SCALE, TILES_PER_PLAYER};
use std::collections::HashSet;
#[cfg(test)]
use std::collections::VecDeque;

pub(crate) const LAND_PERCENT: i32 = 30;
pub(crate) const MAP_TARGET_TILES: i32 =
    DEFAULT_CIVILIZATION_COUNT as i32 * TILES_PER_PLAYER * PERCENT_SCALE / LAND_PERCENT;
const MAP_X_RATIO: i32 = 3;
const MAP_Y_RATIO: i32 = 2;
const MAP_EVEN_FACTOR: i32 = 2;
const MAP_ISO_FACTOR: i32 = 2;

// Native _set_sizes at 0x100031f5c truncates
// sqrt(size / (xratio * yratio * iso * even^2)) + 0.49.  This integer
// comparison expresses the same 0.49 threshold at compile time, so the
// dimensions remain derived from the ruleset inputs instead of copied out.
const fn source_map_linear_scale(requested_tiles: i32, x_ratio: i32, y_ratio: i32) -> i32 {
    let denominator =
        (x_ratio * y_ratio * MAP_ISO_FACTOR * MAP_EVEN_FACTOR * MAP_EVEN_FACTOR) as i64;
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

pub(crate) const MAP_LINEAR_SCALE: i32 =
    source_map_linear_scale(MAP_TARGET_TILES, MAP_X_RATIO, MAP_Y_RATIO);
pub(crate) const MAP_W: i32 = MAP_X_RATIO * MAP_LINEAR_SCALE * MAP_EVEN_FACTOR;
pub(crate) const MAP_H: i32 = MAP_Y_RATIO * MAP_LINEAR_SCALE * MAP_EVEN_FACTOR * MAP_ISO_FACTOR;

// Freeciv's six-way map-space basis remains useful for its radius metric.
/// Freeciv's iso-hex neighbor offsets.
pub(crate) const ISOHEX_DIRS: [(i32, i32); 6] =
    [(0, -1), (1, 0), (1, 1), (0, 1), (-1, 0), (-1, -1)];

/// Eight Civ III directions in staggered-grid coordinates, clockwise from N.
///
/// Civ III's eight-direction neighbor offsets.
pub(crate) const EIGHT_WAY_DIRS: [(i32, i32); 8] = [
    (0, -2),
    (1, -1),
    (2, 0),
    (1, 1),
    (0, 2),
    (-1, 1),
    (-2, 0),
    (-1, -1),
];

pub(crate) fn isohex_vector_distance(dx: i32, dy: i32) -> i32 {
    let abs_dx = dx.abs();
    let abs_dy = dy.abs();
    if dx.signum() != dy.signum() && dx != 0 && dy != 0 {
        abs_dx + abs_dy
    } else {
        abs_dx.max(abs_dy)
    }
}

/// Civ III rank distance on staggered map coordinates. OpenCiv3's Tile_Math
/// uses this formula; rank two selects the manual's 21-square city radius.
///
/// Civ III rank distance; OpenCiv3's Tile_Math uses this formula.
pub(crate) fn rank_distance(dx: i32, dy: i32) -> i32 {
    let dx = dx.abs();
    let dy = dy.abs();
    (dx + dy) / 2 + (dx - dy).abs() / 4
}

pub(crate) fn city_radius_order_key(dx: i32, dy: i32) -> (i32, i32, i32) {
    (rank_distance(dx, dy), dx, dy)
}

// The fixed map-space origin cancels in coordinate differences and inverse
// transforms. It is independent of a campaign's width and height.
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
    (native_x.rem_euclid(MAP_W), native_y)
}

pub(crate) fn map_pos_to_native(map_x: i32, map_y: i32) -> (i32, i32) {
    let (native_x, native_y) = map_to_native(map_x, map_y);
    wrap_native(native_x, native_y)
}

pub(crate) fn offset_native_by_map(native_x: i32, native_y: i32, dx: i32, dy: i32) -> (i32, i32) {
    let (map_x, map_y) = native_to_map(native_x, native_y);
    map_pos_to_native(map_x + dx, map_y + dy)
}

#[cfg(test)]
fn wrapped_native_delta(delta: i32, size: i32) -> i32 {
    (delta + size / 2).rem_euclid(size) - size / 2
}

#[cfg(test)]
pub(crate) fn wrapped_map_vector(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> (i32, i32) {
    // Freeciv wraps in native coordinates first, then converts the resulting
    // virtual endpoint back to map coordinates before measuring distance.
    let native_dx = wrapped_native_delta(to_x - from_x, MAP_W);
    let native_dy = wrapped_native_delta(to_y - from_y, MAP_H);
    let (from_map_x, from_map_y) = native_to_map(from_x, from_y);
    let (to_map_x, to_map_y) = native_to_map(from_x + native_dx, from_y + native_dy);
    (to_map_x - from_map_x, to_map_y - from_map_y)
}

/// Per-campaign geometry. Never global: different saves can coexist in tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct MapWrap {
    width: i32,
    height: i32,
}
impl Default for MapWrap {
    fn default() -> Self {
        Self::X
    }
}
impl std::str::FromStr for MapWrap {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "x" | "wrapx" => Ok(Self::X),
            _ => Err("map topology is horizontal wrap only".into()),
        }
    }
}
impl std::fmt::Display for MapWrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("x")
    }
}
impl MapWrap {
    const MAX_DIMENSION: i32 = 512;
    pub(crate) const MAX_TILES: u64 = Self::MAX_DIMENSION as u64 * Self::MAX_DIMENSION as u64;

    pub(crate) const X: Self = Self {
        width: 3 * source_map_linear_scale(MAP_TARGET_TILES, 3, 2) * MAP_EVEN_FACTOR,
        height: 2
            * source_map_linear_scale(MAP_TARGET_TILES, 3, 2)
            * MAP_EVEN_FACTOR
            * MAP_ISO_FACTOR,
    };
    pub(crate) fn with_dimensions(width: i32, height: i32) -> Result<Self, String> {
        if !(16..=Self::MAX_DIMENSION).contains(&width)
            || !(16..=Self::MAX_DIMENSION).contains(&height)
            || width % 2 != 0
            || height % 2 != 0
        {
            return Err(format!(
                "map dimensions must be even and between 16 and {} tiles per side",
                Self::MAX_DIMENSION
            ));
        }
        Ok(Self { width, height })
    }
    pub(crate) const fn width(self) -> i32 {
        self.width
    }
    pub(crate) const fn height(self) -> i32 {
        self.height
    }
    pub(crate) const fn tile_count(self) -> usize {
        self.width() as usize * self.height() as usize
    }
    pub(crate) fn index(self, x: i32, y: i32) -> usize {
        assert!(
            (0..self.height()).contains(&y),
            "off-map polar tile ({x},{y})"
        );
        (y * self.width() + x.rem_euclid(self.width())) as usize
    }
    pub(crate) fn position(self, index: usize) -> (i32, i32) {
        (index as i32 % self.width(), index as i32 / self.width())
    }
    pub(crate) fn city_radius(self, x: i32, y: i32) -> impl Iterator<Item = (i32, i32, i32, i32)> {
        let grid_x = 2 * x + y.rem_euclid(2);
        (-3_i32..=3).flat_map(move |dy| {
            (-3_i32..=3).filter_map(move |dx| {
                if (dx - dy).rem_euclid(2) != 0 || rank_distance(dx, dy) > 2 {
                    return None;
                }
                let ny = y + dy;
                let nx = (grid_x + dx - ny.rem_euclid(2)) / 2;
                self.normalize(nx, ny).map(|(nx, ny)| (dx, dy, nx, ny))
            })
        })
    }
    pub(crate) fn normalize(self, x: i32, y: i32) -> Option<(i32, i32)> {
        (0..self.height())
            .contains(&y)
            .then_some((x.rem_euclid(self.width()), y))
    }
    pub(crate) fn offset(self, x: i32, y: i32, dx: i32, dy: i32) -> Option<(i32, i32)> {
        let (mx, my) = native_to_map(x, y);
        let (nx, ny) = map_to_native(mx + dx, my + dy);
        self.normalize(nx, ny)
    }
    /// Civ III FLIC animation row order.
    pub(crate) fn flic_direction_index(self, direction: usize) -> Option<u8> {
        match direction {
            0 => Some(5), // N
            1 => Some(4), // NE
            2 => Some(3), // E
            3 => Some(2), // SE
            4 => Some(1), // S
            5 => Some(0), // SW
            6 => Some(7), // W
            7 => Some(6), // NW
            _ => None,
        }
    }
    /// Neighbors in Civ III direction order, across the staggered-grid adapter.
    ///
    /// Uses Civ III's eight-direction neighbor model.
    pub(crate) fn eight_way_neighbors(
        self,
        x: i32,
        y: i32,
    ) -> impl Iterator<Item = (i32, i32)> + use<> {
        (0..EIGHT_WAY_DIRS.len())
            .filter_map(move |direction| self.eight_way_neighbor(x, y, direction))
    }
    /// Shortest step count on the horizontally wrapped Civ III neighbor graph.
    ///
    /// Step count on Civ III's eight-direction neighbor graph.
    pub(crate) fn step_distance(self, ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
        let start_x = 2 * ax + ay.rem_euclid(2);
        let target_x = 2 * bx + by.rem_euclid(2);
        let horizontal_period = 2 * self.width();
        let mut distance = i32::MAX;
        for horizontal_wrap in -1..=1 {
            let dx = target_x + horizontal_wrap * horizontal_period - start_x;
            let dy = by - ay;
            // The eight legal generators change the doubled-grid
            // Manhattan norm by two, so this is their exact step count.
            distance = distance.min((dx.abs() + dy.abs()) / 2);
        }
        distance
    }

    /// Whether `target` lies in the Civ III culture border shape of `radius`
    /// around `source`. Radius 1 is the 3×3 square (the city and its eight
    /// neighbors). Each larger radius extends every side outward by one tile
    /// and trims the corner tiles, giving the 21-tile "fat X" at radius 2, 45
    /// tiles at 3, and so on. The test is made in the doubled grid used by
    /// `step_distance`: step distance at most `radius` and axis span at most
    /// `2 * radius - 1` (never less than the square's span of 2).
    ///
    /// Civ III culture border shape (the 'fat X').
    pub(crate) fn border_shape_contains(
        self,
        source: (i32, i32),
        target: (i32, i32),
        radius: i32,
    ) -> bool {
        let source_x = 2 * source.0 + source.1.rem_euclid(2);
        let target_x = 2 * target.0 + target.1.rem_euclid(2);
        let horizontal_period = 2 * self.width();
        let dy = (target.1 - source.1).abs();
        let max_span = (2 * radius - 1).max(2);
        (-1..=1).any(|horizontal_wrap| {
            let dx = (target_x + horizontal_wrap * horizontal_period - source_x).abs();
            dx + dy <= 2 * radius && dx.max(dy) <= max_span
        })
    }

    /// CivWiki's reverse-engineered culture-flip capital distance: max of the
    /// two diagonal-axis spans plus half the shorter span. Horizontal map
    /// wrapping shifts both axes, so compare the target's adjacent wrap images.
    ///
    /// Civ III culture-flip capital distance.
    #[cfg(test)]
    pub(crate) fn culture_flip_distance(self, ax: i32, ay: i32, bx: i32, by: i32) -> f64 {
        let (ax, ay) = native_to_map(ax, ay);
        let (bx, by) = native_to_map(bx, by);
        (-1..=1)
            .map(|wrap| {
                let dx = (bx + wrap * self.width() - ax).abs();
                let dy = (by - wrap * self.width() - ay).abs();
                f64::from(dx.max(dy)) + f64::from(dx.min(dy)) * 0.5
            })
            .fold(f64::INFINITY, f64::min)
    }

    /// Twice the culture distance between two tiles, as an integer: twice the
    /// longer diagonal-axis span plus the shorter span. Border contests rank
    /// claims by this value, which, unlike the step count, does not give a
    /// diamond of ties between two cities. Horizontal wrapping is handled as in
    /// `culture_flip_distance`.
    ///
    /// Civ III culture distance used for border contests.
    pub(crate) fn culture_rank_distance(self, ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
        let (ax, ay) = native_to_map(ax, ay);
        let (bx, by) = native_to_map(bx, by);
        (-1..=1)
            .map(|wrap| {
                let dx = (bx + wrap * self.width() - ax).abs();
                let dy = (by - wrap * self.width() - ay).abs();
                2 * dx.max(dy) + dx.min(dy)
            })
            .min()
            .unwrap_or(i32::MAX)
    }

    /// Step distances on Civ III's eight-direction neighbor graph.
    #[cfg(test)]
    pub(crate) fn step_distances_from(self, x: i32, y: i32) -> Vec<i32> {
        let mut distances = vec![i32::MAX; (self.width() * self.height()) as usize];
        let origin = self.index(x, y);
        distances[origin] = 0;
        let mut pending = VecDeque::from([(x, y)]);
        while let Some((cx, cy)) = pending.pop_front() {
            let next_distance = distances[self.index(cx, cy)] + 1;
            for (nx, ny) in self.eight_way_neighbors(cx, cy) {
                let index = self.index(nx, ny);
                if distances[index] == i32::MAX {
                    distances[index] = next_distance;
                    pending.push_back((nx, ny));
                }
            }
        }
        distances
    }
    /// Radius over Civ III's eight-direction neighbor graph.
    pub(crate) fn tiles_within_steps(
        self,
        x: i32,
        y: i32,
        max_distance: i32,
    ) -> Vec<(i32, i32, i32)> {
        // Most calls cover unit or city sight. In three steps a tile moves at
        // most three columns and six rows, so one word can track visits.
        const SMALL_RADIUS: i32 = 3;
        if max_distance <= SMALL_RADIUS && self.width() > 2 * SMALL_RADIUS {
            let center_x = x.rem_euclid(self.width());
            let mut visited = 0_u128;
            let mut mark = |nx: i32, ny: i32| {
                let mut dx = nx - center_x;
                if dx < -SMALL_RADIUS {
                    dx += self.width();
                } else if dx > SMALL_RADIUS {
                    dx -= self.width();
                }
                let dy = ny - y;
                let bit = (dy + 2 * SMALL_RADIUS) * (2 * SMALL_RADIUS + 1) + dx + SMALL_RADIUS;
                let mask = 1_u128 << bit;
                let first = visited & mask == 0;
                visited |= mask;
                first
            };
            self.index(x, y);
            mark(center_x, y);
            return self.tiles_within_steps_with_visited(x, y, max_distance, mark);
        }

        let mut visited = HashSet::new();
        visited.insert(self.index(x, y));
        self.tiles_within_steps_with_visited(x, y, max_distance, |nx, ny| {
            visited.insert(self.index(nx, ny))
        })
    }

    /// Radius over Civ III's eight-direction neighbor graph.
    fn tiles_within_steps_with_visited(
        self,
        x: i32,
        y: i32,
        max_distance: i32,
        mut mark: impl FnMut(i32, i32) -> bool,
    ) -> Vec<(i32, i32, i32)> {
        let span = 2 * max_distance.clamp(0, 3) + 1;
        let mut tiles = Vec::with_capacity((span * span) as usize);
        tiles.push((x, y, 0));
        let mut next = 0;
        while let Some(&(cx, cy, distance)) = tiles.get(next) {
            next += 1;
            if distance >= max_distance {
                continue;
            }
            for (nx, ny) in self.eight_way_neighbors(cx, cy) {
                if mark(nx, ny) {
                    tiles.push((nx, ny, distance + 1));
                }
            }
        }
        tiles
    }

    /// Return the selected Freeciv map's `square_iterate` neighborhood in its
    /// deterministic order, adapted to CivCraft's X-wrapped, bounded-Y map.
    /// Freeciv sorts the map-space offsets by real topology distance, then
    /// `dx`, then `dy`; preserving that order keeps bounce RNG outcomes stable.
    ///
    /// Follows Freeciv's square_iterate ordering.
    pub(crate) fn square_radius_in_source_order(
        self,
        x: i32,
        y: i32,
        radius: i32,
    ) -> Vec<(i32, i32)> {
        let (center_map_x, center_map_y) = native_to_map(x, y);
        let mut offsets = Vec::new();
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                let distance = isohex_vector_distance(dx, dy);
                if distance <= radius {
                    offsets.push((distance, dx, dy));
                }
            }
        }
        offsets.sort_unstable();

        let mut seen = HashSet::new();
        offsets
            .into_iter()
            .filter_map(|(_, dx, dy)| {
                let (native_x, native_y) = map_to_native(center_map_x + dx, center_map_y + dy);
                if !(0..self.height()).contains(&native_y) {
                    return None;
                }
                let native_x = native_x.rem_euclid(self.width());
                let index = self.index(native_x, native_y);
                seen.insert(index).then_some((native_x, native_y))
            })
            .collect()
    }

    /// Neighbor in Civ III's eight-direction grid, adapting the current
    /// rectangular tile storage to occupied staggered coordinates.
    ///
    /// Civ III's eight-direction grid.
    pub(crate) fn eight_way_neighbor(self, x: i32, y: i32, direction: usize) -> Option<(i32, i32)> {
        let (dx, dy) = *EIGHT_WAY_DIRS.get(direction)?;
        let grid_x = 2 * x + y.rem_euclid(2) + dx;
        let neighbor_y = y + dy;
        let neighbor_y = if (0..self.height()).contains(&neighbor_y) {
            neighbor_y
        } else {
            return None;
        };
        let grid_x = grid_x.rem_euclid(2 * self.width());
        if grid_x.rem_euclid(2) != neighbor_y.rem_euclid(2) {
            return None;
        }
        Some(((grid_x - neighbor_y.rem_euclid(2)) / 2, neighbor_y))
    }
    /// Civ III's eight-direction grid.
    pub(crate) fn eight_way_offset(self, x: i32, y: i32, dx: i32, dy: i32) -> Option<(i32, i32)> {
        let direction = EIGHT_WAY_DIRS.iter().position(|&step| step == (dx, dy))?;
        self.eight_way_neighbor(x, y, direction)
    }
}

#[cfg(test)]
mod flic_direction_tests {
    use super::*;

    /// Mirrors Civ III behavior.
    #[test]
    fn native_direction_indices_follow_flic_row_order() {
        let wrap = MapWrap::X;
        let expected_rows = [5, 4, 3, 2, 1, 0, 7, 6];
        for (direction, expected) in expected_rows.into_iter().enumerate() {
            assert_eq!(wrap.flic_direction_index(direction), Some(expected));
        }
        assert_eq!(wrap.flic_direction_index(EIGHT_WAY_DIRS.len()), None);
    }
}

/// Rebuild reciprocal Civ III river-edge bits from the transitional tile
/// river markers. This keeps river artwork connected on the eight-direction
/// map while the generator is migrated from its six-direction path model.
pub(crate) trait RiverEdgeTile {
    fn has_river(&self) -> bool;
    fn is_water(&self) -> bool;
    fn river_edges(&self) -> u8;
    fn set_river_edges(&mut self, edges: u8);
}

/// Civ III reciprocal river edge model.
pub(crate) fn rebuild_river_edges<T: RiverEdgeTile>(wrap: MapWrap, tiles: &mut [T]) {
    for tile in tiles.iter_mut() {
        tile.set_river_edges(0);
    }
    for index in 0..wrap.tile_count() {
        if !tiles[index].has_river() {
            continue;
        }
        let (x, y) = wrap.position(index);
        for direction in 0..EIGHT_WAY_DIRS.len() {
            let Some((nx, ny)) = wrap.eight_way_neighbor(x, y, direction) else {
                continue;
            };
            let neighbor = wrap.index(nx, ny);
            if tiles[neighbor].has_river() || tiles[neighbor].is_water() {
                tiles[index].set_river_edges(tiles[index].river_edges() | (1 << direction));
                tiles[neighbor]
                    .set_river_edges(tiles[neighbor].river_edges() | (1 << ((direction + 4) % 8)));
            }
        }
    }
}

/// Remove nonreciprocal Civ III river bits after tiles are converted or
/// removed at a map edge.
///
/// Civ III reciprocal river edge model.
pub(crate) fn prune_river_edges<T: RiverEdgeTile>(wrap: MapWrap, tiles: &mut [T]) {
    let original: Vec<_> = tiles.iter().map(RiverEdgeTile::river_edges).collect();
    for index in 0..wrap.tile_count() {
        let (x, y) = wrap.position(index);
        for direction in 0..EIGHT_WAY_DIRS.len() {
            if original[index] & (1 << direction) == 0 {
                continue;
            }
            let reciprocal = wrap
                .eight_way_neighbor(x, y, direction)
                .is_some_and(|(nx, ny)| {
                    let neighbor = wrap.index(nx, ny);
                    original[neighbor] & (1 << ((direction + 4) % 8)) != 0
                });
            if !reciprocal {
                tiles[index].set_river_edges(tiles[index].river_edges() & !(1 << direction));
            }
        }
    }
}
#[cfg(test)]
mod wrap_tests {
    use super::*;

    #[test]
    fn custom_horizontal_map_size_controls_normalization_and_storage() {
        let map = MapWrap::with_dimensions(80, 60).unwrap();
        assert_eq!(
            (map.width(), map.height(), map.tile_count()),
            (80, 60, 4800)
        );
        assert_eq!(map.normalize(81, 12), Some((1, 12)));
        assert_eq!(map.normalize(2, -1), None);
        assert!(MapWrap::with_dimensions(81, 60).is_err());
    }
    #[test]
    fn horizontal_wrap_is_the_only_supported_topology() {
        assert_eq!(MapWrap::default(), MapWrap::X);
        assert_eq!("x".parse::<MapWrap>().unwrap(), MapWrap::X);
        assert!("xy".parse::<MapWrap>().is_err());
        assert!("y".parse::<MapWrap>().is_err());
        assert!("none".parse::<MapWrap>().is_err());
        assert_eq!(
            MapWrap::X.normalize(-1, 0),
            Some((MapWrap::X.width() - 1, 0))
        );
        assert_eq!(MapWrap::X.normalize(0, -1), None);
    }

    /// Mirrors Civ III behavior.
    #[test]
    fn native_neighbors_are_reciprocal_across_rows_seams_and_bounded_poles() {
        for wrap in [MapWrap::X] {
            for y in 0..wrap.height() {
                for x in 0..wrap.width() {
                    for (direction, &(_, dy)) in EIGHT_WAY_DIRS.iter().enumerate() {
                        let Some((nx, ny)) = wrap.eight_way_neighbor(x, y, direction) else {
                            assert!(!(0..wrap.height()).contains(&(y + dy)));
                            continue;
                        };
                        assert_eq!(
                            wrap.eight_way_neighbor(nx, ny, (direction + 4) % 8),
                            Some((x, y))
                        );
                    }
                }
            }
            for y in 0..wrap.height() {
                assert_eq!(
                    wrap.eight_way_neighbor(wrap.width() - 1, y, 2),
                    Some((0, y))
                );
            }
            for y in 0..wrap.height() {
                for x in 0..wrap.width() {
                    for (dx, dy) in ISOHEX_DIRS {
                        if let Some(neighbor) = wrap.offset(x, y, dx, dy) {
                            assert!((0..EIGHT_WAY_DIRS.len()).any(|direction| {
                                wrap.eight_way_neighbor(x, y, direction) == Some(neighbor)
                            }));
                        }
                    }
                }
            }
        }
    }

    /// Mirrors Civ III behavior.
    #[test]
    fn native_neighbor_iterator_exposes_all_eight_directions() {
        {
            let wrap = MapWrap::X;
            let (x, y) = (wrap.width() / 2, wrap.height() / 2);
            let neighbors: Vec<_> = wrap.eight_way_neighbors(x, y).collect();
            let expected: Vec<_> = (0..EIGHT_WAY_DIRS.len())
                .filter_map(|direction| wrap.eight_way_neighbor(x, y, direction))
                .collect();
            assert_eq!(neighbors, expected);
            assert_eq!(neighbors.len(), 8);
        }
    }

    /// Mirrors Freeciv behavior.
    #[test]
    fn reference_square_radius_keeps_source_order_and_adapts_horizontal_seams() {
        let wrap = MapWrap::X;
        let center = (wrap.width() / 2, wrap.height() / 2);
        let tiles = wrap.square_radius_in_source_order(center.0, center.1, 2);
        assert_eq!(tiles.len(), 19, "ISO|HEX radius two contains 19 tiles");
        let (map_x, map_y) = native_to_map(center.0, center.1);
        let expected_prefix =
            [(0, 0), (-1, -1), (-1, 0), (0, -1), (0, 1), (1, 0), (1, 1)].map(|(dx, dy)| {
                let (x, y) = map_to_native(map_x + dx, map_y + dy);
                (x.rem_euclid(wrap.width()), y)
            });
        assert_eq!(&tiles[..expected_prefix.len()], expected_prefix);

        let seam = wrap.square_radius_in_source_order(0, center.1, 2);
        assert!(seam.contains(&(wrap.width() - 1, center.1)));
        assert_eq!(seam, wrap.square_radius_in_source_order(0, center.1, 2));
    }

    /// Mirrors Civ III behavior.
    #[test]
    fn native_distance_matches_neighbor_graph_with_horizontal_wrap() {
        for wrap in [MapWrap::X] {
            let origins = [
                (0, 0),
                (wrap.width() - 1, 1),
                (wrap.width() / 2, wrap.height() / 2),
                (wrap.width() / 3, wrap.height() - 1),
            ];
            for (x, y) in origins {
                let graph = wrap.step_distances_from(x, y);
                for (index, &distance) in graph.iter().enumerate() {
                    let (tx, ty) = wrap.position(index);
                    assert_eq!(
                        wrap.step_distance(x, y, tx, ty),
                        distance,
                        "wrap={wrap:?}, origin=({x},{y}), target=({tx},{ty})"
                    );
                }
            }
        }
    }

    #[test]
    fn culture_flip_distance_uses_diagonal_axes_and_horizontal_wrap() {
        let wrap = MapWrap::X;
        let origin = (wrap.width() / 2, wrap.height() / 2);
        let diagonal = offset_native_by_map(origin.0, origin.1, 4, 2);
        assert_eq!(
            wrap.culture_flip_distance(origin.0, origin.1, diagonal.0, diagonal.1),
            5.0
        );

        let seam_a = (0, wrap.height() / 2);
        let seam_b = (wrap.width() - 1, wrap.height() / 2);
        assert_eq!(
            wrap.culture_flip_distance(seam_a.0, seam_a.1, seam_b.0, seam_b.1),
            1.5
        );
        assert_eq!(
            wrap.culture_flip_distance(seam_a.0, seam_a.1, seam_b.0, seam_b.1),
            wrap.culture_flip_distance(seam_b.0, seam_b.1, seam_a.0, seam_a.1)
        );
    }

    /// Mirrors Civ III behavior.
    #[test]
    fn native_radius_matches_full_distance_field_at_edges_and_wraps() {
        let wrap = MapWrap::with_dimensions(16, 16).unwrap();
        for origin in [(0, 0), (15, 1), (0, 8), (15, 15)] {
            let distances = wrap.step_distances_from(origin.0, origin.1);
            let wide = wrap.tiles_within_steps(origin.0, origin.1, 4);
            let radius: HashSet<_> = wide
                .iter()
                .map(|&(x, y, distance)| {
                    assert_eq!(distance, distances[wrap.index(x, y)]);
                    (x, y)
                })
                .collect();
            for (index, &distance) in distances.iter().enumerate() {
                let (x, y) = wrap.position(index);
                assert_eq!(radius.contains(&(x, y)), distance <= 4);
            }
            for small in 0..=3 {
                let count = wide.partition_point(|&(_, _, distance)| distance <= small);
                assert_eq!(
                    wrap.tiles_within_steps(origin.0, origin.1, small),
                    wide[..count]
                );
            }
        }
    }

    /// Mirrors Civ III behavior.
    #[test]
    fn native_city_radius_has_twenty_one_distinct_tiles_and_wraps_at_the_seam() {
        {
            let wrap = MapWrap::X;
            let center = (wrap.width() - 1, wrap.height() / 2);
            let radius: Vec<_> = wrap.city_radius(center.0, center.1).collect();
            let unique: std::collections::HashSet<_> =
                radius.iter().map(|&(_, _, x, y)| (x, y)).collect();
            assert_eq!(radius.len(), 21);
            assert_eq!(unique.len(), 21);
            assert!(
                radius
                    .iter()
                    .any(|&(dx, dy, x, y)| { (dx, dy) == (0, 0) && (x, y) == center })
            );
            assert!(radius.iter().any(|&(_, _, x, _)| x == 0));
        }
    }

    /// Mirrors Civ III behavior.
    #[test]
    fn generated_river_markers_become_reciprocal_native_edges() {
        let wrap = MapWrap::X;
        let mut map = crate::mapgen::generate_with_wrap(42, wrap);
        let (x, y) = (10, 20);
        let (nx, ny) = wrap.eight_way_neighbor(x, y, 1).unwrap();
        map.tiles[wrap.index(x, y)].river = true;
        map.tiles[wrap.index(nx, ny)].river = true;
        rebuild_river_edges(wrap, &mut map.tiles);
        assert_ne!(map.tiles[wrap.index(x, y)].river_edges & (1 << 1), 0);
        assert_ne!(map.tiles[wrap.index(nx, ny)].river_edges & (1 << 5), 0);
    }

    #[test]
    fn generated_river_courses_have_only_reciprocal_edges() {
        let wrap = MapWrap::X;
        let map = crate::mapgen::generate_with_wrap(42, wrap);
        let mut edge_count = 0;
        for (index, tile) in map.tiles.iter().enumerate() {
            let (x, y) = wrap.position(index);
            for direction in 0..EIGHT_WAY_DIRS.len() {
                if tile.river_edges & (1 << direction) == 0 {
                    continue;
                }
                edge_count += 1;
                let (nx, ny) = wrap.eight_way_neighbor(x, y, direction).unwrap();
                assert_ne!(
                    map.tiles[wrap.index(nx, ny)].river_edges & (1 << ((direction + 4) % 8)),
                    0
                );
            }
        }
        assert!(edge_count > 0);
    }
}
