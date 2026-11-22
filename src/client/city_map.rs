//! City worker allocation uses the main map's terrain renderer and projection.
use super::*;

pub(super) struct CityMapLayout {
    pub bounds: Rect,
    pub tile_w: f32,
    pub tile_h: f32,
}
impl CityMapLayout {
    pub fn new(bounds: Rect) -> Self {
        // Outer centers are three half-tiles away, plus half a tile to the tip.
        // Leave extra headroom for mountain and city sprites.
        let zoom = ((bounds.w - 32.0) / (4.0 * TILE_W))
            .min((bounds.h - 32.0) / (4.0 * TILE_H + 24.0))
            .min(2.0);
        Self {
            bounds,
            tile_w: TILE_W * zoom,
            tile_h: TILE_H * zoom,
        }
    }
    pub fn center(&self, dx: i32, dy: i32) -> Vec2 {
        vec2(
            self.bounds.x + self.bounds.w * 0.5 + dx as f32 * self.tile_w * 0.5,
            self.bounds.y + self.bounds.h * 0.5 + 8.0 + dy as f32 * self.tile_h * 0.5,
        )
    }
    pub fn tile_rect(&self, dx: i32, dy: i32) -> Rect {
        let p = self.center(dx, dy);
        Rect::new(
            p.x - self.tile_w * 0.5,
            p.y - self.tile_h * 0.5,
            self.tile_w,
            self.tile_h,
        )
    }
}

pub(super) fn tile_hit(rect: Rect, point: Vec2) -> bool {
    let center = vec2(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
    (point.x - center.x).abs() / (rect.w * 0.5) + (point.y - center.y).abs() / (rect.h * 0.5) <= 1.0
}

// Integer half-tile vertices give adjacent diamonds identical edge keys, even
// when the view is scaled. Hover wins if both tiles contribute the same edge.
type TileEdge = ((i32, i32), (i32, i32));
fn city_tile_edges(
    tiles: impl IntoIterator<Item = (i32, i32, bool)>,
) -> std::collections::BTreeMap<TileEdge, bool> {
    let mut edges = std::collections::BTreeMap::new();
    for (dx, dy, hovered) in tiles {
        let vertices = [(dx, dy - 1), (dx + 1, dy), (dx, dy + 1), (dx - 1, dy)];
        for index in 0..4 {
            let (a, b) = (vertices[index], vertices[(index + 1) % 4]);
            let key = if a < b { (a, b) } else { (b, a) };
            edges
                .entry(key)
                .and_modify(|highlight| *highlight |= hovered)
                .or_insert(hovered);
        }
    }
    edges
}

impl Game {
    pub(super) fn draw_city_tiles_map(
        &self,
        assets: &Assets,
        city: &CityManagerView,
        panel: Rect,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        let bounds = panel;
        let layout = CityMapLayout::new(bounds);
        let scale = screen_dpi_scale();
        unsafe {
            get_internal_gl().quad_gl.scissor(Some((
                (bounds.x * scale) as i32,
                (bounds.y * scale) as i32,
                (bounds.w * scale) as i32,
                (bounds.h * scale) as i32,
            )));
        }
        let view = world::WorldMapView {
            viewport: Rect::new(bounds.x, bounds.y + 8.0, bounds.w, bounds.h),
            camera: vec2((2 * city.x + city.y.rem_euclid(2)) as f32, city.y as f32),
            zoom: layout.tile_w / TILE_W,
            interactive: false,
        };
        let tiles = view.tiles(self.sim.map_wrap());
        world::Civ3WorldPainter::new(self, assets).draw_view(
            assets,
            view,
            || {
                // Cover the entire terrain underlay, including the fog fringe.
                // Fog masks this optional city-ownership emphasis afterward.
                for &(x, y, center) in &tiles {
                    if self.sim.territory_geometry_owner_view(x, y) != Some(city.owner) {
                        draw_diamond(
                            center,
                            layout.tile_w,
                            layout.tile_h,
                            Color::from_rgba(5, 15, 24, 75),
                        );
                    }
                }
            },
            || self.draw_city_work_overlays(city, &layout, actions),
        );
        unsafe {
            get_internal_gl().quad_gl.scissor(None);
        }
        draw_rectangle_lines(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            1.0,
            Color::from_rgba(105, 102, 86, 255),
        );
    }

    fn draw_city_work_overlays(
        &self,
        city: &CityManagerView,
        layout: &CityMapLayout,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        let mouse = ui_pointer();
        let mut outlined_tiles = Vec::new();
        for tile in &city.map_tiles {
            if !tile.seen {
                continue;
            }
            let is_city = tile.is_city;
            let worked = tile.worked;
            let rect = layout.tile_rect(tile.dx, tile.dy);
            let center = layout.center(tile.dx, tile.dy);
            let hovered = tile.can_assign && tile_hit(rect, mouse);
            if worked || is_city || hovered {
                outlined_tiles.push((tile.dx, tile.dy, hovered));
            }
            if is_city {
                continue;
            }
            let (food, shields, trade) = tile.yields;
            let label = format!("{food}/{shields}/{trade}");
            let size = if layout.tile_w < 95.0 { 12.0 } else { 14.0 };
            let measured = measure_cached(&label, size as u16);
            let label_y = center.y + layout.tile_h * 0.22;
            let pill = Rect::new(
                center.x - measured.width * 0.5 - 5.0,
                label_y - measured.height * 0.5 - 3.0,
                measured.width + 10.0,
                measured.height + 6.0,
            );
            draw_rectangle(
                pill.x,
                pill.y,
                pill.w,
                pill.h,
                Color::from_rgba(9, 17, 20, 220),
            );
            draw_text(
                &label,
                pill.x + 5.0,
                pill.y + (pill.h - measured.height) * 0.5 + measured.offset_y,
                size,
                if worked || is_city {
                    Color::from_rgba(247, 220, 137, 255)
                } else {
                    LIGHTGRAY
                },
            );
            if tile.can_assign {
                actions.push((rect, UiAction::WorkCityTile(city.id, tile.x, tile.y)));
            }
        }
        // Share worked-tile edges so the yellow outline has one consistent width.
        for (((ax, ay), (bx, by)), hovered) in city_tile_edges(outlined_tiles) {
            let (a, b) = (layout.center(ax, ay), layout.center(bx, by));
            draw_line(a.x, a.y, b.x, b.y, 2.0, if hovered { WHITE } else { GOLD });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adjacent_worked_tiles_share_one_edge() {
        let edges = city_tile_edges([(0, 0, false), (1, 1, false)]);
        assert_eq!(edges.len(), 7);
        assert_eq!(edges.get(&((0, 1), (1, 0))), Some(&false));
        // Tiles in the same staggered row meet at a tip.
        assert_eq!(city_tile_edges([(0, 0, false), (2, 0, false)]).len(), 8);
    }
    #[test]
    fn hover_highlight_owns_shared_edge_regardless_of_tile_order() {
        let edges = city_tile_edges([(0, 0, false), (1, 1, true)]);
        assert_eq!(edges.get(&((0, 1), (1, 0))), Some(&true));
        assert_eq!(edges, city_tile_edges([(1, 1, true), (0, 0, false)]));
    }
    #[test]
    fn worked_tiles_match_world_positions_and_clicks_across_seams_and_poles() {
        let wrap = MapWrap::with_dimensions(80, 80).unwrap();
        let bounds = Rect::new(22.0, 201.0, 708.0, 275.0);
        let layout = CityMapLayout::new(bounds);
        let zoom = layout.tile_w / TILE_W;
        for (cx, cy) in [(0, 0), (0, 21), (79, 20), (79, 79)] {
            let camera = vec2((2 * cx + cy % 2) as f32, cy as f32);
            let view = world::WorldMapView {
                viewport: Rect::new(bounds.x, bounds.y + 8.0, bounds.w, bounds.h),
                camera,
                zoom,
                interactive: false,
            };
            let tiles: Vec<_> = wrap.city_radius(cx, cy).collect();
            let actions: Vec<_> = tiles
                .iter()
                .map(|&(dx, dy, x, y)| (layout.tile_rect(dx, dy), UiAction::WorkCityTile(1, x, y)))
                .collect();
            for (dx, dy, x, y) in tiles {
                let expected = view.project(wrap, x, y);
                assert!(layout.center(dx, dy).distance(expected) < 0.001);
                assert!(matches!(Game::ui_action_at(expected, &actions),
                    Some(UiAction::WorkCityTile(1, tx, ty)) if (tx, ty) == (x, y)));
            }
        }
    }
    #[test]
    fn city_radius_fits_compact_and_large_panels() {
        for (w, h) in [(752.0, 520.0), (920.0, 640.0)] {
            let bounds = Rect::new(22.0, 201.0, w - 44.0, h - 245.0);
            let layout = CityMapLayout::new(bounds);
            for dy in -3_i32..=3 {
                for dx in -3_i32..=3 {
                    if (dx - dy).rem_euclid(2) != 0 || rank_distance(dx, dy) > 2 {
                        continue;
                    }
                    let r = layout.tile_rect(dx, dy);
                    assert!(r.x >= bounds.x && r.right() <= bounds.right());
                    assert!(r.y >= bounds.y && r.bottom() <= bounds.bottom());
                }
            }
        }
    }
    #[test]
    fn diamond_hit_testing_rejects_bounding_box_corners() {
        let rect = Rect::new(10.0, 20.0, 96.0, 48.0);
        assert!(tile_hit(rect, vec2(58.0, 44.0)));
        assert!(!tile_hit(rect, vec2(11.0, 21.0)));
        assert!(!tile_hit(rect, vec2(105.0, 67.0)));
    }
}
