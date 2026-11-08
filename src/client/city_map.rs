//! City worker allocation uses the main map's terrain renderer and projection.
use super::*;

pub(super) struct CityMapLayout {
    pub bounds: Rect,
    pub tile_w: f32,
    pub tile_h: f32,
}
impl CityMapLayout {
    pub fn new(bounds: Rect) -> Self {
        // The Civ III 21-square radius spans five tile widths and heights.
        // Leave extra headroom for mountain and city sprites.
        let zoom = ((bounds.w - 32.0) / (5.0 * TILE_W))
            .min((bounds.h - 32.0) / (5.0 * TILE_H + 24.0))
            .min(2.0);
        Self {
            bounds,
            tile_w: TILE_W * zoom,
            tile_h: TILE_H * zoom,
        }
    }
    pub fn center(&self, dx: i32, dy: i32) -> Vec2 {
        vec2(
            self.bounds.x + self.bounds.w * 0.5 + (dx - dy) as f32 * self.tile_w * 0.5,
            self.bounds.y + self.bounds.h * 0.5 + 8.0 + (dx + dy) as f32 * self.tile_h * 0.5,
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
        let (u, v) = (dx - dy, dx + dy);
        let vertices = [(u, v - 1), (u + 1, v), (u, v + 1), (u - 1, v)];
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
        city: &City,
        panel: Rect,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        draw_text("WORKED TILES", panel.x + 22.0, panel.y + 187.0, 18.0, WHITE);
        let bounds = Rect::new(
            panel.x + 22.0,
            panel.y + 201.0,
            panel.w - 44.0,
            panel.h - 245.0,
        );
        let layout = CityMapLayout::new(bounds);
        self.draw_city_map(assets, city, &layout);
        let mouse = ui_pointer();
        let mut outlined_tiles = Vec::new();
        for (dx, dy, x, y) in self.sim.map_city_radius(city) {
            let tile = self.sim.tile(x, y);
            if !tile.seen {
                continue;
            }
            let is_city = (dx, dy) == (0, 0);
            let worked = city.worked_tiles.contains(&(x, y));
            let enabled = !is_city
                && (worked
                    || (city.specialists.iter().any(|&n| n > 0)
                        && self.sim.can_assign_city_tile(city, x, y)));
            let rect = layout.tile_rect(dx, dy);
            let center = layout.center(dx, dy);
            let hovered = enabled && tile_hit(rect, mouse);
            if !worked && !is_city {
                draw_diamond(
                    center,
                    layout.tile_w - 2.0,
                    layout.tile_h - 2.0,
                    Color::from_rgba(5, 15, 24, 100),
                );
            }
            if worked || is_city || hovered {
                outlined_tiles.push((dx, dy, hovered));
            }
            let label = if is_city {
                "City".to_owned()
            } else {
                let (food, shields, trade) = tile.yields();
                format!("{food}/{shields}/{trade}")
            };
            let size = if layout.tile_w < 95.0 { 12.0 } else { 14.0 };
            let measured = measure_text(&label, None, size as u16, 1.0);
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
            if enabled {
                actions.push((rect, UiAction::WorkCityTile(city.id, x, y)));
            }
        }
        // Draw after dimming all unworked tiles so shared edges keep one color
        // and thickness rather than accumulating two inset diamond outlines.
        let origin = layout.center(0, 0);
        let vertex = |(u, v): (i32, i32)| {
            origin
                + vec2(
                    u as f32 * layout.tile_w * 0.5,
                    v as f32 * layout.tile_h * 0.5,
                )
        };
        for ((a, b), hovered) in city_tile_edges(outlined_tiles) {
            let (a, b) = (vertex(a), vertex(b));
            draw_line(a.x, a.y, b.x, b.y, 2.0, if hovered { WHITE } else { GOLD });
        }
        draw_rectangle_lines(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            1.0,
            Color::from_rgba(105, 102, 86, 255),
        );
        text_in(
            "Food / shields / trade · Gold: worked · Click a tile to assign a citizen",
            panel.x + 22.0,
            panel.bottom() - 17.0,
            14.0,
            panel.w - 44.0,
            LIGHTGRAY,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adjacent_worked_tiles_share_one_edge() {
        let edges = city_tile_edges([(0, 0, false), (1, 0, false)]);
        assert_eq!(edges.len(), 7);
        assert_eq!(edges.get(&((0, 1), (1, 0))), Some(&false));
        // Diagonal diamonds meet at a vertex, not along an edge.
        assert_eq!(city_tile_edges([(0, 0, false), (1, 1, false)]).len(), 8);
    }
    #[test]
    fn hover_highlight_owns_shared_edge_regardless_of_tile_order() {
        let edges = city_tile_edges([(0, 0, false), (1, 0, true)]);
        assert_eq!(edges.get(&((0, 1), (1, 0))), Some(&true));
        assert_eq!(edges, city_tile_edges([(1, 0, true), (0, 0, false)]));
    }
    #[test]
    fn city_radius_fits_compact_and_large_panels() {
        for (w, h) in [(752.0, 520.0), (920.0, 640.0)] {
            let bounds = Rect::new(22.0, 201.0, w - 44.0, h - 245.0);
            let layout = CityMapLayout::new(bounds);
            for dy in -3_i32..=3 {
                for dx in -3_i32..=3 {
                    if (dx - dy).rem_euclid(2) != 0 || civ3_rank_distance(dx, dy) > 2 {
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
