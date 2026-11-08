//! Civ III staggered-grid coordinates, independent of the current campaign grid.
//!
//! The coordinate convention follows the visible grid documented in the Civ III
//! manual and the reference implementation in OpenCiv3. A tile sprite is 128×64
//! pixels; cells are 64×32 pixels, and occupied coordinates satisfy
//! `x % 2 == y % 2`.

pub(crate) const CELL_W: f32 = 64.0;
pub(crate) const CELL_H: f32 = 32.0;

pub(crate) const DIRECTIONS: [(i32, i32); 8] = [
    (0, -2),  // north
    (1, -1),  // northeast
    (2, 0),   // east
    (1, 1),   // southeast
    (0, 2),   // south
    (-1, 1),  // southwest
    (-2, 0),  // west
    (-1, -1), // northwest
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Civ3Geometry {
    width: i32,
    height: i32,
    wrap_x: bool,
    wrap_y: bool,
}

impl Civ3Geometry {
    pub(crate) fn new(width: i32, height: i32, wrap_x: bool) -> Result<Self, &'static str> {
        Self::new_wrapped(width, height, wrap_x, false)
    }

    pub(crate) fn new_wrapped(
        width: i32,
        height: i32,
        wrap_x: bool,
        wrap_y: bool,
    ) -> Result<Self, &'static str> {
        if width <= 0 || width % 2 != 0 || height <= 0 || (wrap_y && height % 2 != 0) {
            return Err("Civ III grid requires a positive even width and positive height");
        }
        Ok(Self {
            width,
            height,
            wrap_x,
            wrap_y,
        })
    }

    pub(crate) fn tile_count(self) -> usize {
        (self.width / 2 * self.height) as usize
    }

    /// Return a row-major tile-array index for an occupied staggered coordinate.
    pub(crate) fn index(self, x: i32, y: i32) -> Option<usize> {
        let y = if self.wrap_y {
            y.rem_euclid(self.height)
        } else if (0..self.height).contains(&y) {
            y
        } else {
            return None;
        };
        let x = if self.wrap_x {
            x.rem_euclid(self.width)
        } else if (0..self.width).contains(&x) {
            x
        } else {
            return None;
        };
        if x.rem_euclid(2) != y.rem_euclid(2) {
            return None;
        }
        Some((y * (self.width / 2) + x / 2) as usize)
    }

    /// Convert an array index into its occupied staggered coordinate.
    pub(crate) fn coordinates(self, index: usize) -> Option<(i32, i32)> {
        if index >= self.tile_count() {
            return None;
        }
        let row_width = (self.width / 2) as usize;
        let y = (index / row_width) as i32;
        let x = (index % row_width) as i32 * 2 + y.rem_euclid(2);
        Some((x, y))
    }

    /// Adapt the current rectangular row/column storage coordinate to the
    /// staggered coordinate occupied by the same tile.
    pub(crate) fn storage_to_grid(self, x: i32, y: i32) -> Option<(i32, i32)> {
        self.index(2 * x + y.rem_euclid(2), y)?;
        Some((2 * x + y.rem_euclid(2), y))
    }

    /// Convert and normalize an occupied staggered coordinate into the
    /// current row/column storage layout.
    pub(crate) fn grid_to_storage(self, x: i32, y: i32) -> Option<(i32, i32)> {
        let index = self.index(x, y)?;
        let (grid_x, row) = self.coordinates(index)?;
        Some(((grid_x - row.rem_euclid(2)) / 2, row))
    }

    pub(crate) fn neighbor(self, x: i32, y: i32, direction: usize) -> Option<(i32, i32)> {
        let &(dx, dy) = DIRECTIONS.get(direction)?;
        let x = x + dx;
        let y = y + dy;
        let index = self.index(x, y)?;
        self.coordinates(index)
    }

    /// Center of a tile in map pixels, before camera and zoom transforms.
    pub(crate) fn tile_center(self, x: i32, y: i32) -> Option<(f32, f32)> {
        let index = self.index(x, y)?;
        let (x, y) = self.coordinates(index)?;
        Some(self.tile_center_virtual(x, y))
    }

    /// Center for unbounded virtual coordinates used while panning and drawing
    /// decorative water beyond a bounded map edge.
    pub(crate) fn tile_center_virtual(self, x: i32, y: i32) -> (f32, f32) {
        ((x + 1) as f32 * CELL_W, (y + 1) as f32 * CELL_H)
    }

    /// Find the virtual tile coordinate under a point expressed in cell units.
    /// Coordinates may be outside the map, allowing the camera to wrap first.
    pub(crate) fn tile_at_cell_point(px: f32, py: f32) -> (i32, i32) {
        let x = px.floor() as i32;
        let y = py.floor() as i32;
        let fx = px - x as f32;
        let fy = py - y as f32;
        let mut tile_x = x;
        let mut tile_y = y;
        let odd_x = x.rem_euclid(2) != 0;
        let odd_y = y.rem_euclid(2) != 0;
        if odd_x != odd_y {
            if fy > fx {
                tile_x -= 1;
            } else {
                tile_y -= 1;
            }
        } else if fy < 1.0 - fx {
            tile_x -= 1;
            tile_y -= 1;
        }
        (tile_x, tile_y)
    }

    /// Pick a tile from screen-space displacement relative to the camera's
    /// tile center. `art_scale` maps native PCX pixels to screen pixels.
    pub(crate) fn screen_to_storage(
        self,
        dx_pixels: f32,
        dy_pixels: f32,
        camera_grid_x: f32,
        camera_grid_y: f32,
        zoom: f32,
        art_scale: f32,
    ) -> Option<(i32, i32)> {
        let cell_x = camera_grid_x + 1.0 + dx_pixels / (CELL_W * art_scale * zoom);
        let cell_y = camera_grid_y + 1.0 + dy_pixels / (CELL_H * art_scale * zoom);
        let (grid_x, y) = Self::tile_at_cell_point(cell_x, cell_y);
        self.grid_to_storage(grid_x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staggered_coordinates_round_trip_through_flat_storage() {
        let map = Civ3Geometry::new(8, 6, false).unwrap();
        assert_eq!(map.tile_count(), 24);
        for index in 0..map.tile_count() {
            let (x, y) = map.coordinates(index).unwrap();
            assert_eq!(map.index(x, y), Some(index));
            assert_eq!(x.rem_euclid(2), y.rem_euclid(2));
        }
        assert_eq!(map.coordinates(map.tile_count()), None);
        assert_eq!(map.index(0, 1), None);
    }

    #[test]
    fn eight_neighbors_are_reciprocal_and_preserve_stagger() {
        let map = Civ3Geometry::new(10, 8, false).unwrap();
        let (x, y) = (4, 4);
        for direction in 0..DIRECTIONS.len() {
            let (nx, ny) = map.neighbor(x, y, direction).unwrap();
            assert_eq!(nx.rem_euclid(2), ny.rem_euclid(2));
            let reverse = (direction + 4) % 8;
            assert_eq!(map.neighbor(nx, ny, reverse), Some((x, y)));
        }
        assert_eq!(map.neighbor(4, 0, 0), None);
        assert_eq!(map.neighbor(0, 4, 6), None);
    }

    #[test]
    fn horizontal_wrap_preserves_parity_and_reciprocity() {
        let map = Civ3Geometry::new(8, 6, true).unwrap();
        assert_eq!(map.neighbor(6, 2, 2), Some((0, 2)));
        assert_eq!(map.neighbor(0, 2, 6), Some((6, 2)));
        assert_eq!(map.tile_center(-2, 0), map.tile_center(6, 0));
    }

    #[test]
    fn projected_tile_centers_pick_the_same_coordinate() {
        let map = Civ3Geometry::new(12, 8, true).unwrap();
        for index in 0..map.tile_count() {
            let (x, y) = map.coordinates(index).unwrap();
            let (px, py) = map.tile_center(x, y).unwrap();
            let picked = Civ3Geometry::tile_at_cell_point(px / CELL_W, py / CELL_H);
            assert_eq!(picked, (x, y));
        }
    }

    #[test]
    fn storage_adapter_matches_staggered_array_indices() {
        let map = Civ3Geometry::new(16, 8, true).unwrap();
        for y in 0..map.height {
            for x in 0..map.width / 2 {
                let (grid_x, grid_y) = map.storage_to_grid(x, y).unwrap();
                assert_eq!(map.grid_to_storage(grid_x, grid_y), Some((x, y)));
                assert_eq!(
                    map.index(grid_x, grid_y),
                    Some((y * (map.width / 2) + x) as usize)
                );
            }
        }
    }

    #[test]
    fn screen_picking_round_trips_tile_centers_across_rows_and_seam() {
        let map = Civ3Geometry::new(16, 8, true).unwrap();
        let zoom = 1.17;
        let art_scale = 0.75;
        let camera = map.storage_to_grid(7, 3).unwrap();
        for y in 0..map.height {
            for x in 0..map.width / 2 {
                let (grid_x, grid_y) = map.storage_to_grid(x, y).unwrap();
                let dx = (grid_x - camera.0) as f32 * CELL_W * art_scale * zoom;
                let dy = (grid_y - camera.1) as f32 * CELL_H * art_scale * zoom;
                assert_eq!(
                    map.screen_to_storage(
                        dx,
                        dy,
                        camera.0 as f32,
                        camera.1 as f32,
                        zoom,
                        art_scale
                    ),
                    Some((x, y))
                );
            }
        }
    }

    #[test]
    fn rejects_invalid_dimensions() {
        assert!(Civ3Geometry::new(0, 4, false).is_err());
        assert!(Civ3Geometry::new(7, 4, false).is_err());
        assert!(Civ3Geometry::new(8, 0, false).is_err());
    }
}
