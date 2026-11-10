//! Native vector overlays for cities and units on the world map.
use super::*;

pub(super) fn hp_fraction(hp: i32, maximum: i32) -> f32 {
    (hp.max(0) as f32 / maximum.max(1) as f32).clamp(0.0, 1.0)
}

impl Game {
    pub(super) fn draw_city_overlay(&self, _assets: &Assets, center: Vec2, zoom: f32, col: i32) {
        let color = if col == 2 {
            Color::from_rgba(230, 198, 95, 235)
        } else {
            Color::from_rgba(210, 76, 67, 235)
        };
        let radius = 8.0 * zoom;
        draw_circle(
            center.x + 27.0 * zoom,
            center.y - 18.0 * zoom,
            radius,
            color,
        );
        draw_circle_lines(
            center.x + 27.0 * zoom,
            center.y - 18.0 * zoom,
            radius,
            1.5 * zoom,
            BLACK,
        );
        draw_text(
            if col == 2 { "O" } else { "!" },
            center.x + 24.0 * zoom,
            center.y - 14.5 * zoom,
            13.0 * zoom,
            BLACK,
        );
    }

    pub(super) fn draw_unit_status(&self, _assets: &Assets, unit: &Unit, center: Vec2) {
        let zoom = self.zoom;
        let width = 38.0 * zoom;
        let height = (4.0 * zoom).max(2.0);
        let x = center.x - width * 0.5;
        let y = center.y - 29.0 * zoom;
        draw_rectangle(x, y, width, height, Color::from_rgba(20, 20, 20, 220));
        let fraction = hp_fraction(unit.hp, unit.max_hit_points());
        let color = if fraction > 0.6 {
            Color::from_rgba(84, 205, 102, 255)
        } else if fraction > 0.3 {
            Color::from_rgba(232, 190, 61, 255)
        } else {
            Color::from_rgba(218, 66, 58, 255)
        };
        draw_rectangle(x, y, width * fraction, height, color);
        if unit.veteran > 0 {
            draw_text(
                "★",
                center.x - 4.0 * zoom,
                center.y - 34.0 * zoom,
                10.0 * zoom,
                Color::from_rgba(255, 220, 112, 255),
            );
        }
        if unit.work_left > 0 || unit.automated || unit.state != UnitState::Idle {
            draw_circle(
                center.x + 21.0 * zoom,
                center.y - 24.0 * zoom,
                5.0 * zoom,
                Color::from_rgba(28, 39, 43, 240),
            );
            draw_circle_lines(
                center.x + 21.0 * zoom,
                center.y - 24.0 * zoom,
                5.0 * zoom,
                zoom,
                WHITE,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_bar_fraction_is_clamped_and_handles_invalid_maximum() {
        assert_eq!(hp_fraction(20, 20), 1.0);
        assert_eq!(hp_fraction(5, 10), 0.5);
        assert_eq!(hp_fraction(-1, 10), 0.0);
        assert_eq!(hp_fraction(5, 0), 1.0);
    }
}
