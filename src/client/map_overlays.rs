//! Native vector overlays for cities on the world map.
use super::*;

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

    pub(super) fn draw_unit_health(&self, assets: &Assets, unit: &RenderedUnitView, center: Vec2) {
        let camera_zoom = self.zoom.clamp(0.5, 1.0);
        let max_hp = unit.max_hp.max(1);
        let current_hp = unit.hp.clamp(0, max_hp);
        let has_health_bar = unit.attack > 0 || unit.defense > 0;
        let segment_height = (if max_hp <= 6 {
            4.0
        } else if max_hp <= 12 {
            2.0
        } else {
            1.0
        }) / camera_zoom;
        let width = 2.0 / camera_zoom;
        let gap = 1.0 / camera_zoom;
        let total_height = segment_height * max_hp as f32 + gap * (max_hp - 1) as f32;
        let x = center.x - 26.0;
        let y = if has_health_bar {
            center.y - 8.0 - total_height
        } else {
            center.y - 26.0
        };

        if has_health_bar {
            draw_rectangle(x, y, width, total_height, BLACK);

            let fraction = current_hp as f32 / max_hp as f32;
            let color = if fraction >= 0.67 {
                Color::from_rgba(0, 255, 0, 255)
            } else if fraction >= 0.34 && max_hp > 2 {
                Color::from_rgba(255, 255, 0, 255)
            } else {
                Color::from_rgba(255, 0, 0, 255)
            };

            for segment in 0..current_hp {
                let segment_y =
                    y + total_height - segment_height - segment as f32 * (segment_height + gap);
                draw_rectangle(x, segment_y, width, segment_height, color);
            }

            if unit.state == UnitState::Fortified {
                draw_rectangle_lines(
                    x - 0.5,
                    y - 0.5,
                    width + 1.0,
                    total_height + 1.0,
                    1.0,
                    WHITE,
                );
            }
        }

        if unit.owner == HUMAN {
            assets.civ3.draw_unit_movement_indicator(
                vec2(x, y),
                unit.moves_left,
                unit.movement_rate,
                camera_zoom,
            );
        }
    }
}
