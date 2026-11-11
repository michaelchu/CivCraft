//! Civ III goto preview drawn with native map geometry.
use super::*;

impl Game {
    pub(super) fn goto_preview(&self) -> Option<GotoPreview> {
        if !self.goto_mode || self.playback.pending() || self.selected_city.is_some() {
            return None;
        }
        let pointer = ui_pointer();
        if !Self::viewport_rect().contains(pointer) {
            return None;
        }
        let (x, y) = self.screen_to_world(pointer)?;
        self.sim.goto_preview_view(self.selected_unit?, x, y)
    }

    pub(super) fn draw_goto_preview(&self, _assets: &Assets) {
        let Some(preview) = self.goto_preview() else {
            return;
        };
        let viewport = Self::viewport_rect();
        let dpi = screen_dpi_scale();
        unsafe {
            get_internal_gl().quad_gl.scissor(Some((
                (viewport.x * dpi) as i32,
                (viewport.y * dpi) as i32,
                (viewport.w * dpi) as i32,
                (viewport.h * dpi) as i32,
            )));
        }
        let project = |point: &GotoPoint| self.world_screen_position(point.x, point.y);
        let period_x = self.sim.map_wrap().width() as f32 * TILE_W * self.zoom;
        for pair in preview.points.windows(2) {
            let from = project(&pair[0]);
            let raw_to = project(&pair[1]);
            let to = from
                + vec2(
                    map_view::shortest_wrapped_delta(raw_to.x - from.x, period_x),
                    raw_to.y - from.y,
                );
            for shift_x in [-period_x, 0.0, period_x] {
                let (a, b) = (from + vec2(shift_x, 0.0), to + vec2(shift_x, 0.0));
                if a.x.max(b.x) < viewport.x
                    || a.x.min(b.x) > viewport.right()
                    || a.y.max(b.y) < viewport.y
                    || a.y.min(b.y) > viewport.bottom()
                {
                    continue;
                }
                draw_line(
                    a.x,
                    a.y,
                    b.x,
                    b.y,
                    (4.0 * self.zoom).max(2.5),
                    Color::from_rgba(0, 36, 36, 230),
                );
                draw_line(
                    a.x,
                    a.y,
                    b.x,
                    b.y,
                    (2.0 * self.zoom).max(1.5),
                    Color::from_rgba(0, 255, 200, 255),
                );
            }
        }
        for (index, point) in preview.points.iter().enumerate().skip(1) {
            let destination = index + 1 == preview.points.len();
            if !destination && point.moves_left > 0 {
                continue;
            }
            let center = project(point);
            let size = self.zoom;
            let color = if destination {
                Color::from_rgba(255, 211, 74, 255)
            } else {
                Color::from_rgba(93, 239, 197, 255)
            };
            draw_diamond(
                center,
                TILE_W * size * 0.38,
                TILE_H * size * 0.38,
                Color::new(color.r, color.g, color.b, 0.32),
            );
            draw_diamond_lines(
                center,
                TILE_W * size * 0.38,
                TILE_H * size * 0.38,
                (2.0 * size).max(1.0),
                color,
            );
            let label = point.turn.to_string();
            draw_text(
                &label,
                center.x - 4.0 * size,
                center.y + 4.0 * size,
                (13.0 * size).max(8.0),
                WHITE,
            );
        }
        unsafe {
            get_internal_gl().quad_gl.scissor(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seam_segments_take_the_short_adjacent_route() {
        assert_eq!(map_view::shortest_wrapped_delta(990.0, 1000.0), -10.0);
        assert_eq!(map_view::shortest_wrapped_delta(-990.0, 1000.0), 10.0);
        assert_eq!(map_view::shortest_wrapped_delta(24.0, 1000.0), 24.0);
    }
}
