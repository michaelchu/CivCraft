//! Minimap rendering and viewport navigation.

use super::*;

impl Game {
    pub(super) fn draw_minimap(&self) {
        let rect = self.minimap_rect();
        let scale = rect.h / self.sim.map_wrap.height() as f32;
        for y in 0..self.sim.map_wrap.height() {
            for x in 0..self.sim.map_wrap.width() {
                let tile = self.sim.minimap_tile_view(x, y);
                let color = if !tile.seen {
                    Color::from_rgba(7, 13, 17, 255)
                } else if tile.visible {
                    tile.terrain.minimap_color()
                } else {
                    darken(tile.terrain.minimap_color(), 0.53)
                };
                let origin = self.minimap_tile_origin(rect, x, y);
                let width = (rect.right() - origin.x).min(2.0 * scale);
                draw_rectangle(origin.x, origin.y, width, scale, color);
                if y % 2 == 1 && x == self.sim.map_wrap.width() - 1 {
                    draw_rectangle(rect.x, origin.y, scale, scale, color);
                }
            }
        }
        let city_overrides = self.playback.city_overrides();
        let mut cities = self.sim.minimap_city_views();
        cities.retain(|city| !city_overrides.contains_key(&city.id));
        cities.extend(
            city_overrides
                .values()
                .map(|city| self.sim.minimap_city_override_view(city)),
        );
        for city in cities {
            let origin = self.minimap_tile_origin(rect, city.x, city.y);
            let marker = Rect::new(origin.x, origin.y, 2.0 * scale, scale);
            let color = player_color(city.civilization_id.as_deref(), city.owner);
            if let Some(part) = marker.intersect(rect) {
                draw_rectangle(part.x, part.y, part.w, part.h, color);
            }
            if marker.right() > rect.right() {
                draw_rectangle(
                    rect.x,
                    marker.y,
                    marker.right() - rect.right(),
                    marker.h,
                    color,
                );
            }
        }

        let viewport = Self::viewport_rect();
        let camera =
            vec2(rect.x, rect.y) + (self.camera_overview_position() + vec2(1.0, 0.5)) * scale;
        let width = viewport.w * scale / (TILE_W * 0.5 * self.zoom);
        let height = viewport.h * scale / (TILE_H * 0.5 * self.zoom);
        let shifts_y = [0.0];
        for &wrap_y in &shifts_y {
            for wrap_x in [-rect.w, 0.0, rect.w] {
                let left = camera.x + wrap_x - width * 0.5;
                let top = camera.y + wrap_y - height * 0.5;
                let right = left + width;
                let bottom = top + height;
                for y in [top, bottom] {
                    if (rect.y..=rect.bottom()).contains(&y)
                        && left < rect.right()
                        && right > rect.x
                    {
                        draw_line(left.max(rect.x), y, right.min(rect.right()), y, 1.0, WHITE);
                    }
                }
                for x in [left, right] {
                    if (rect.x..=rect.right()).contains(&x)
                        && top < rect.bottom()
                        && bottom > rect.y
                    {
                        draw_line(x, top.max(rect.y), x, bottom.min(rect.bottom()), 1.0, WHITE);
                    }
                }
            }
        }
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            2.0,
            Color::from_rgba(160, 126, 61, 255),
        );
    }
}
