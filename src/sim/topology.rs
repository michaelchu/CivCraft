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

use super::{PERCENT_SCALE, PLAYER_COUNT, Tile};
use std::collections::{HashSet, VecDeque};

const TILES_PER_PLAYER: i32 = 100;
pub(crate) const LAND_PERCENT: i32 = 30;
pub(crate) const MAP_TARGET_TILES: i32 =
    PLAYER_COUNT as i32 * TILES_PER_PLAYER * PERCENT_SCALE / LAND_PERCENT;
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

pub(super) const MAP_LINEAR_SCALE: i32 =
    source_map_linear_scale(MAP_TARGET_TILES, MAP_X_RATIO, MAP_Y_RATIO);
pub(crate) const MAP_W: i32 = MAP_X_RATIO * MAP_LINEAR_SCALE * MAP_EVEN_FACTOR;
pub(crate) const MAP_H: i32 = MAP_Y_RATIO * MAP_LINEAR_SCALE * MAP_EVEN_FACTOR * MAP_ISO_FACTOR;

// Legacy six-way steps retained only for systems not yet migrated to the
// Civ III staggered-grid topology.
pub(crate) const ISOHEX_DIRS: [(i32, i32); 6] =
    [(0, -1), (1, 0), (1, 1), (0, 1), (-1, 0), (-1, -1)];

/// Eight Civ III directions in staggered-grid coordinates, clockwise from N.
pub(crate) const CIV3_DIRS: [(i32, i32); 8] = [
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
pub(crate) fn civ3_rank_distance(dx: i32, dy: i32) -> i32 {
    let dx = dx.abs();
    let dy = dy.abs();
    (dx + dy) / 2 + (dx - dy).abs() / 4
}

pub(crate) fn city_radius_order_key(dx: i32, dy: i32) -> (i32, i32, i32) {
    (civ3_rank_distance(dx, dy), dx, dy)
}

pub(crate) fn is_isohex_step(dx: i32, dy: i32) -> bool {
    ISOHEX_DIRS.contains(&(dx, dy))
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
    pub(crate) const X: Self = Self {
        width: 3 * source_map_linear_scale(MAP_TARGET_TILES, 3, 2) * MAP_EVEN_FACTOR,
        height: 2
            * source_map_linear_scale(MAP_TARGET_TILES, 3, 2)
            * MAP_EVEN_FACTOR
            * MAP_ISO_FACTOR,
    };
    pub(crate) fn with_dimensions(width: i32, height: i32) -> Result<Self, String> {
        if !(16..=512).contains(&width)
            || !(16..=512).contains(&height)
            || width % 2 != 0
            || height % 2 != 0
        {
            return Err("map dimensions must be even and between 16 and 512 tiles per side".into());
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
                if (dx - dy).rem_euclid(2) != 0 || civ3_rank_distance(dx, dy) > 2 {
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
    /// Map a legacy simulation step to OpenCiv3's FLC direction row order
    /// (SW, S, SE, E, NE, N, NW, W). Pass the step before wrapping so a seam
    /// crossing retains its intended direction.
    pub(crate) fn civ3_flic_direction(self, dx: i32, dy: i32) -> Option<u8> {
        match (dx, dy) {
            (0, -1) => Some(4),
            (1, 0) => Some(2),
            (1, 1) => Some(1),
            (0, 1) => Some(0),
            (-1, 0) => Some(6),
            (-1, -1) => Some(5),
            _ => None,
        }
    }
    pub(crate) fn civ3_flic_direction_index(self, direction: usize) -> Option<u8> {
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
    pub(crate) fn civ3_neighbors(self, x: i32, y: i32) -> impl Iterator<Item = (i32, i32)> + use<> {
        (0..CIV3_DIRS.len()).filter_map(move |direction| self.civ3_neighbor(x, y, direction))
    }
    /// Shortest step count on the horizontally wrapped Civ III neighbor graph.
    pub(crate) fn civ3_distance(self, ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
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

    /// CivWiki's reverse-engineered culture-flip capital distance: max of the
    /// two diagonal-axis spans plus half the shorter span. Horizontal map
    /// wrapping shifts both axes, so compare the target's adjacent wrap images.
    pub(crate) fn civ3_culture_distance(self, ax: i32, ay: i32, bx: i32, by: i32) -> f64 {
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

    pub(crate) fn civ3_distances_from(self, x: i32, y: i32) -> Vec<i32> {
        let mut distances = vec![i32::MAX; (self.width() * self.height()) as usize];
        let origin = self.index(x, y);
        distances[origin] = 0;
        let mut pending = VecDeque::from([(x, y)]);
        while let Some((cx, cy)) = pending.pop_front() {
            let next_distance = distances[self.index(cx, cy)] + 1;
            for (nx, ny) in self.civ3_neighbors(cx, cy) {
                let index = self.index(nx, ny);
                if distances[index] == i32::MAX {
                    distances[index] = next_distance;
                    pending.push_back((nx, ny));
                }
            }
        }
        distances
    }
    pub(crate) fn civ3_radius(self, x: i32, y: i32, max_distance: i32) -> Vec<(i32, i32, i32)> {
        let mut visited = HashSet::new();
        let mut pending = VecDeque::from([(x, y, 0)]);
        visited.insert(self.index(x, y));
        let mut tiles = Vec::new();
        while let Some((cx, cy, distance)) = pending.pop_front() {
            tiles.push((cx, cy, distance));
            if distance >= max_distance {
                continue;
            }
            for (nx, ny) in self.civ3_neighbors(cx, cy) {
                if visited.insert(self.index(nx, ny)) {
                    pending.push_back((nx, ny, distance + 1));
                }
            }
        }
        tiles
    }
    /// Neighbor in Civ III's eight-direction grid, adapting the current
    /// rectangular tile storage to occupied staggered coordinates.
    pub(crate) fn civ3_neighbor(self, x: i32, y: i32, direction: usize) -> Option<(i32, i32)> {
        let (dx, dy) = *CIV3_DIRS.get(direction)?;
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
    pub(crate) fn civ3_offset(self, x: i32, y: i32, dx: i32, dy: i32) -> Option<(i32, i32)> {
        let direction = CIV3_DIRS.iter().position(|&step| step == (dx, dy))?;
        self.civ3_neighbor(x, y, direction)
    }
}

#[cfg(test)]
mod civ3_flic_direction_tests {
    use super::*;

    #[test]
    fn legacy_neighbor_steps_select_their_civ3_flic_direction_rows() {
        let wrap = MapWrap::X;
        let expected = [4, 2, 1, 0, 6, 5];
        for ((dx, dy), row) in ISOHEX_DIRS.into_iter().zip(expected) {
            assert_eq!(wrap.civ3_flic_direction(dx, dy), Some(row));
        }
    }

    #[test]
    fn flic_direction_selection_handles_horizontal_seam_neighbors() {
        let wrap = MapWrap::X;
        let from = (wrap.width() - 1, 1);
        let step = (0, -1);
        let to = wrap.offset(from.0, from.1, step.0, step.1).unwrap();
        assert_eq!(to.0, 0);
        assert_eq!(wrap.civ3_flic_direction(step.0, step.1), Some(4));
    }

    #[test]
    fn civ3_direction_indices_follow_flic_row_order() {
        let wrap = MapWrap::X;
        let expected_rows = [5, 4, 3, 2, 1, 0, 7, 6];
        for (direction, expected) in expected_rows.into_iter().enumerate() {
            assert_eq!(wrap.civ3_flic_direction_index(direction), Some(expected));
        }
        assert_eq!(wrap.civ3_flic_direction_index(CIV3_DIRS.len()), None);
    }
}

/// Rebuild reciprocal Civ III river-edge bits from the transitional tile
/// river markers. This keeps river artwork connected on the eight-direction
/// map while the generator is migrated from its six-direction path model.
pub(crate) fn rebuild_civ3_river_edges(wrap: MapWrap, tiles: &mut [Tile]) {
    for tile in tiles.iter_mut() {
        tile.river_edges = 0;
    }
    for index in 0..wrap.tile_count() {
        if !tiles[index].river {
            continue;
        }
        let (x, y) = wrap.position(index);
        for direction in 0..CIV3_DIRS.len() {
            let Some((nx, ny)) = wrap.civ3_neighbor(x, y, direction) else {
                continue;
            };
            let neighbor = wrap.index(nx, ny);
            if tiles[neighbor].river || tiles[neighbor].terrain.is_water() {
                tiles[index].river_edges |= 1 << direction;
                tiles[neighbor].river_edges |= 1 << ((direction + 4) % 8);
            }
        }
    }
}

/// Remove nonreciprocal Civ III river bits after tiles are converted or
/// removed at a map edge.
pub(crate) fn prune_civ3_river_edges(wrap: MapWrap, tiles: &mut [Tile]) {
    let original: Vec<_> = tiles.iter().map(|tile| tile.river_edges).collect();
    for index in 0..wrap.tile_count() {
        let (x, y) = wrap.position(index);
        for direction in 0..CIV3_DIRS.len() {
            if original[index] & (1 << direction) == 0 {
                continue;
            }
            let reciprocal = wrap.civ3_neighbor(x, y, direction).is_some_and(|(nx, ny)| {
                let neighbor = wrap.index(nx, ny);
                original[neighbor] & (1 << ((direction + 4) % 8)) != 0
            });
            if !reciprocal {
                tiles[index].river_edges &= !(1 << direction);
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
    use crate::sim::{Command, HUMAN, Simulation, Terrain, UnitKind};
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

    #[test]
    fn civ3_neighbors_are_reciprocal_across_rows_seams_and_bounded_poles() {
        for wrap in [MapWrap::X] {
            for y in 0..wrap.height() {
                for x in 0..wrap.width() {
                    for direction in 0..CIV3_DIRS.len() {
                        let Some((nx, ny)) = wrap.civ3_neighbor(x, y, direction) else {
                            assert!(!(0..wrap.height()).contains(&(y + CIV3_DIRS[direction].1)));
                            continue;
                        };
                        assert_eq!(
                            wrap.civ3_neighbor(nx, ny, (direction + 4) % 8),
                            Some((x, y))
                        );
                    }
                }
            }
            for y in 0..wrap.height() {
                assert_eq!(wrap.civ3_neighbor(wrap.width() - 1, y, 2), Some((0, y)));
            }
            for y in 0..wrap.height() {
                for x in 0..wrap.width() {
                    for (dx, dy) in ISOHEX_DIRS {
                        if let Some(neighbor) = wrap.offset(x, y, dx, dy) {
                            assert!((0..CIV3_DIRS.len()).any(|direction| {
                                wrap.civ3_neighbor(x, y, direction) == Some(neighbor)
                            }));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn civ3_neighbor_iterator_exposes_all_eight_directions() {
        for wrap in [MapWrap::X] {
            let (x, y) = (wrap.width() / 2, wrap.height() / 2);
            let neighbors: Vec<_> = wrap.civ3_neighbors(x, y).collect();
            let expected: Vec<_> = (0..CIV3_DIRS.len())
                .filter_map(|direction| wrap.civ3_neighbor(x, y, direction))
                .collect();
            assert_eq!(neighbors, expected);
            assert_eq!(neighbors.len(), 8);
        }
    }

    #[test]
    fn civ3_distance_matches_neighbor_graph_with_horizontal_wrap() {
        for wrap in [MapWrap::X] {
            let origins = [
                (0, 0),
                (wrap.width() - 1, 1),
                (wrap.width() / 2, wrap.height() / 2),
                (wrap.width() / 3, wrap.height() - 1),
            ];
            for (x, y) in origins {
                let graph = wrap.civ3_distances_from(x, y);
                for index in 0..wrap.tile_count() {
                    let (tx, ty) = wrap.position(index);
                    assert_eq!(
                        wrap.civ3_distance(x, y, tx, ty),
                        graph[index],
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
            wrap.civ3_culture_distance(origin.0, origin.1, diagonal.0, diagonal.1),
            5.0
        );

        let seam_a = (0, wrap.height() / 2);
        let seam_b = (wrap.width() - 1, wrap.height() / 2);
        assert_eq!(
            wrap.civ3_culture_distance(seam_a.0, seam_a.1, seam_b.0, seam_b.1),
            1.5
        );
        assert_eq!(
            wrap.civ3_culture_distance(seam_a.0, seam_a.1, seam_b.0, seam_b.1),
            wrap.civ3_culture_distance(seam_b.0, seam_b.1, seam_a.0, seam_a.1)
        );
    }

    #[test]
    fn civ3_radius_matches_full_distance_field_at_edges_and_wraps() {
        for wrap in [MapWrap::X] {
            for origin in [(0, 0), (wrap.width() - 1, wrap.height() - 1)] {
                let distances = wrap.civ3_distances_from(origin.0, origin.1);
                let radius: HashSet<_> = wrap
                    .civ3_radius(origin.0, origin.1, 4)
                    .into_iter()
                    .map(|(x, y, distance)| {
                        assert_eq!(distance, distances[wrap.index(x, y)]);
                        (x, y)
                    })
                    .collect();
                for (index, &distance) in distances.iter().enumerate() {
                    let (x, y) = wrap.position(index);
                    assert_eq!(radius.contains(&(x, y)), distance <= 4);
                }
            }
        }
    }

    #[test]
    fn civ3_city_radius_has_twenty_one_distinct_tiles_and_wraps_at_the_seam() {
        for wrap in [MapWrap::X] {
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

    #[test]
    fn generated_river_markers_become_reciprocal_civ3_edges() {
        let wrap = MapWrap::X;
        let mut map = crate::mapgen::generate_with_wrap(42, wrap);
        let (x, y) = (10, 20);
        let (nx, ny) = wrap.civ3_neighbor(x, y, 1).unwrap();
        map.tiles[wrap.index(x, y)].river = true;
        map.tiles[wrap.index(nx, ny)].river = true;
        rebuild_civ3_river_edges(wrap, &mut map.tiles);
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
            for direction in 0..CIV3_DIRS.len() {
                if tile.river_edges & (1 << direction) == 0 {
                    continue;
                }
                edge_count += 1;
                let (nx, ny) = wrap.civ3_neighbor(x, y, direction).unwrap();
                assert_ne!(
                    map.tiles[wrap.index(nx, ny)].river_edges & (1 << ((direction + 4) % 8)),
                    0
                );
            }
        }
        assert!(edge_count > 0);
    }

    #[test]
    fn each_campaign_obeys_its_seams_for_movement_sight_routes_and_city_radius() {
        for wrap in [MapWrap::X] {
            let mut sim = Simulation::new(crate::mapgen::generate_with_wrap(42, wrap));
            sim.units.clear();
            sim.cities.clear();
            for tile in &mut sim.tiles {
                tile.terrain = Terrain::Grassland;
                tile.road = false;
                tile.railroad = false;
                tile.river = false;
                tile.base = None;
                tile.hut = false;
                tile.owner = None;
                tile.seen = false;
                tile.visible = false;
                tile.known_by.fill(false);
            }
            let id = sim.spawn_unit(HUMAN, UnitKind::Warrior, 10, 0);
            sim.refresh_visibility();
            let other = (10, wrap.height() - 1);
            assert!(!sim.tile(other.0, other.1).visible);
            assert_eq!(
                sim.map_wrap.civ3_neighbors(10, 0).any(|p| p == other),
                false
            );
            let actor = sim.units[0].clone();
            let path = sim.plan_route(&actor, other.0, other.1, true);
            assert!(path.len() > 20);
            assert!(!sim.apply(Command::Move(id, 0, -1)));
            let city = sim.create_city(HUMAN, 10, 0);
            let _ = city;
            let radius: Vec<_> = sim.map_city_radius(sim.cities.last().unwrap()).collect();
            assert_eq!(radius.iter().any(|p| p.3 >= wrap.height() - 2), false);
            assert!(radius.iter().all(|p| (0..wrap.height()).contains(&p.3)));
            // Population loss must remove fields across this mode's actual seam.
            let mut edge_city = sim.cities.last().unwrap().clone();
            edge_city.x = 0;
            edge_city.y = 20;
            edge_city.specialists = [0; 3];
            edge_city.worked_tiles = vec![(wrap.width() - 1, 20)];
            edge_city.remove_citizen_assignments(1, wrap);
            assert!(edge_city.worked_tiles.is_empty());
            // Horizontal wrapping is enabled in both modes.
            assert_eq!(wrap.offset(0, 20, -1, 1), Some((wrap.width() - 1, 20)));
        }
    }
}

#[cfg(test)]
mod campaign_geometry_tests {
    use super::*;
    use crate::sim::{Command, HUMAN, Simulation, UnitKind};
    #[test]
    fn both_geometries_coexist_while_campaigns_found_and_advance() {
        for seed in [42, 1337] {
            let mut games = [MapWrap::X]
                .map(|wrap| Simulation::new(crate::mapgen::generate_with_wrap(seed, wrap)));
            for sim in &mut games {
                assert_eq!(sim.tiles.len(), sim.map_wrap.tile_count());
                assert_eq!(sim.route_memory.len(), sim.tiles.len());
                let founder = sim
                    .units
                    .iter()
                    .find(|u| u.owner == HUMAN && u.kind == UnitKind::Settler)
                    .unwrap()
                    .id;
                assert!(sim.apply(Command::FoundCity(founder)));
            }
            for _ in 0..12 {
                for sim in &mut games {
                    assert!(sim.apply(Command::EndTurn));
                }
            }
            for sim in &games {
                assert!(
                    sim.units
                        .iter()
                        .all(|u| (0..sim.map_wrap.width()).contains(&u.x)
                            && (0..sim.map_wrap.height()).contains(&u.y))
                );
                for city in &sim.cities {
                    let radius: Vec<_> = sim.map_city_radius(city).collect();
                    assert!(
                        radius
                            .iter()
                            .all(|&(dx, dy, _, _)| civ3_rank_distance(dx, dy) <= 2)
                    );
                    assert_eq!(radius.len(), 21);
                }
            }
        }
    }
}
