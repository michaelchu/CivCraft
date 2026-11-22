//! Rendering of cities, units, and other world entities.

use super::*;

/// Civ III FLIC direction row 2 faces southeast, toward the lower right.
const FORTIFY_FACING: u8 = 2;

fn centered_sprite_position(center: Vec2, width: f32, height: f32) -> Vec2 {
    center - vec2(width, height) * 0.5
}

fn centered_text_baseline(row_top: f32, row_height: f32, metrics: TextDimensions) -> f32 {
    row_top + (row_height - metrics.height) * 0.5 + metrics.offset_y
}

fn city_size_text_color(background: Color) -> Color {
    let brightness = background.r * 0.299 + background.g * 0.587 + background.b * 0.114;
    if brightness >= 0.52 { BLACK } else { WHITE }
}

impl Game {
    pub(super) fn draw_observed_city(
        &self,
        assets: &Assets,
        city: &RenderedCityView,
        center: Vec2,
        zoom: f32,
    ) {
        self.draw_city_sprite(
            assets,
            city.size,
            city.art_style,
            city.art_era,
            city.civilization_id.as_deref(),
            city.occupied,
            city.disorder,
            center,
            zoom,
        );
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "City rendering combines city appearance with viewport geometry."
    )]
    fn draw_city_sprite(
        &self,
        assets: &Assets,
        size: i32,
        style: u8,
        era: u8,
        civilization_id: Option<&str>,
        occupied: bool,
        disorder: bool,
        center: Vec2,
        zoom: f32,
    ) {
        let zoom = zoom * WORLD_ART_SCALE;
        // Civ III sheets are four eras by three population bands.
        let col = match size {
            1..=6 => 0,
            7..=12 => 1,
            _ => 2,
        };
        let row = i32::from(era.min(3));
        let texture = assets.art_tileset.city_texture_at(style, civilization_id);
        let source = Rect::new(col as f32 * 167.0, row as f32 * 95.0, 167.0, 95.0);
        // Draw the source frame at native size, scaled only by world zoom.
        let (city_w, city_h) = (167.0 * zoom, 95.0 * zoom);
        let position = centered_sprite_position(center, city_w, city_h);
        draw_texture_ex(
            texture,
            position.x,
            position.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(city_w, city_h)),
                source: Some(source),
                ..Default::default()
            },
        );
        if occupied {
            self.draw_city_overlay(assets, center, zoom, 2);
        }
        if disorder {
            self.draw_city_overlay(assets, center, zoom, 5);
        }
    }

    pub(super) fn draw_city_label_at_zoom(&self, city: &RenderedCityView, center: Vec2, zoom: f32) {
        let top_text = city.growth_label.as_ref().map_or_else(
            || city.name.clone(),
            |growth| format!("{} : {growth}", city.name),
        );
        let center_font = (11.0 * zoom).max(9.0);
        let top_metrics = measure_cached(&top_text, center_font as u16);
        let bottom_metrics = city
            .production_label
            .as_ref()
            .map(|production| measure_cached(production, center_font as u16));
        // The reference selection ring is tile-sized; its bottom must clear
        // the label too, regardless of which unit is currently focused.
        let top = super::city_bar::label_top(center.y, zoom);
        let horizontal_padding = (6.0 * zoom).max(4.0);
        // Keep the two information rows close together, like the compact
        // Civ III banner, while leaving a small inset above and below them.
        let vertical_padding = (2.0 * zoom).max(1.5);
        let center_width = (top_metrics
            .width
            .max(bottom_metrics.as_ref().map_or(0.0, |metrics| metrics.width))
            + horizontal_padding * 2.0)
            .max((94.0 * zoom).max(82.0));
        let row_height = (center_font + vertical_padding * 0.5).max(12.0);
        let row_count = if city.production_label.is_some() {
            2.0
        } else {
            1.0
        };
        let height = row_height * row_count + vertical_padding;
        let size_text = city.size.to_string();
        let size_font = (19.0 * zoom).max(14.0);
        let size_metrics = measure_cached(&size_text, size_font as u16);
        let star_radius = (7.0 * zoom).max(5.5);
        let side_padding = (4.0 * zoom).max(2.5);
        let side_width = size_metrics.width.max(star_radius * 2.0) + side_padding * 2.0;
        let width = center_width + side_width * if city.capital { 2.0 } else { 1.0 };
        let left = center.x - width * 0.5;
        let palette_index = self.sim.player_team_palette_view(city.owner);
        let owner_color = player_color(palette_index, city.owner);

        // Civ III's city banner is a three-part bar: population, two centered
        // information rows, and a capital badge.
        draw_rectangle(
            left,
            top,
            side_width,
            height,
            Color {
                a: 205.0,
                ..owner_color
            },
        );
        draw_rectangle(
            left + side_width,
            top,
            center_width,
            height,
            Color::from_rgba(19, 20, 18, 185),
        );
        if city.capital {
            draw_rectangle(
                left + side_width + center_width,
                top,
                side_width,
                height,
                Color {
                    a: 205.0,
                    ..owner_color
                },
            );
        }
        draw_rectangle_lines(
            left,
            top,
            width,
            height,
            1.0,
            Color::from_rgba(15, 15, 13, 235),
        );
        draw_line(
            left + side_width,
            top,
            left + side_width,
            top + height,
            1.0,
            Color::from_rgba(15, 15, 13, 235),
        );
        if city.capital {
            draw_line(
                left + side_width + center_width,
                top,
                left + side_width + center_width,
                top + height,
                1.0,
                Color::from_rgba(15, 15, 13, 235),
            );
        }
        if city.production_label.is_some() {
            draw_line(
                left + side_width,
                top + row_height,
                left + side_width + center_width,
                top + row_height,
                1.0,
                Color::from_rgba(92, 92, 83, 210),
            );
        }

        let size_x = left + (side_width - size_metrics.width) * 0.5;
        let size_baseline = centered_text_baseline(top, height, size_metrics);
        draw_text(
            &size_text,
            size_x,
            size_baseline,
            size_font,
            city_size_text_color(owner_color),
        );

        let text_left = left + side_width;
        draw_text(
            &top_text,
            text_left + (center_width - top_metrics.width) * 0.5,
            centered_text_baseline(top, row_height, top_metrics),
            center_font,
            Color::from_rgba(244, 235, 201, 255),
        );
        if let (Some(production), Some(metrics)) =
            (city.production_label.as_ref(), bottom_metrics.as_ref())
        {
            draw_text(
                production,
                text_left + (center_width - metrics.width) * 0.5,
                centered_text_baseline(top + row_height, row_height, *metrics),
                center_font,
                Color::from_rgba(222, 220, 202, 255),
            );
        }

        if city.capital {
            let star_center = vec2(
                left + side_width + center_width + side_width * 0.5,
                top + height * 0.5,
            );
            let outer = star_radius;
            let inner = outer * 0.43;
            let mut points = [Vec2::ZERO; 10];
            for (index, point) in points.iter_mut().enumerate() {
                let angle =
                    -std::f32::consts::FRAC_PI_2 + index as f32 * std::f32::consts::PI / 5.0;
                let radius = if index % 2 == 0 { outer } else { inner };
                *point = star_center + vec2(angle.cos(), angle.sin()) * radius;
            }
            for index in 0..points.len() {
                draw_triangle(
                    star_center,
                    points[index],
                    points[(index + 1) % points.len()],
                    Color::from_rgba(255, 225, 67, 255),
                );
                draw_line(
                    points[index].x,
                    points[index].y,
                    points[(index + 1) % points.len()].x,
                    points[(index + 1) % points.len()].y,
                    1.0,
                    Color::from_rgba(42, 34, 12, 255),
                );
            }
        }
        if self.selected_city == Some(city.id) {
            draw_diamond_lines(
                center,
                TILE_W * zoom * 0.88,
                TILE_H * zoom * 0.82,
                3.0,
                Color::from_rgba(255, 234, 119, 255),
            );
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "A viewport supplies its own zoom for shared entity rendering."
    )]
    pub(in crate::client) fn draw_unit_at_zoom(
        &self,
        assets: &Assets,
        unit: &RenderedUnitView,
        center: Vec2,
        stack_count: usize,
        action: super::super::tileset::UnitArtAction,
        animation_progress: f32,
        playback_duration_seconds: Option<f32>,
        zoom: f32,
    ) {
        let unit_zoom = zoom * WORLD_ART_SCALE;
        let unit_w = 58.0 * unit_zoom;
        let unit_h = 44.0 * unit_zoom;
        let is_selected = self.selected_unit == Some(unit.id);
        let is_group_selected = self.selected_units.contains(&unit.id);
        if is_selected {
            assets.art_tileset.draw_unit_cursor(center, zoom);
        } else if is_group_selected {
            draw_ellipse_lines(
                center.x,
                center.y - 9.0 * zoom,
                unit_w * 0.47,
                unit_h * 0.57,
                0.0,
                (2.0 * zoom).max(1.5),
                Color::from_rgba(255, 220, 112, 230),
            );
        }
        let facing = if matches!(
            action,
            super::super::tileset::UnitArtAction::Fortify
                | super::super::tileset::UnitArtAction::FortifyHold
        ) {
            FORTIFY_FACING
        } else {
            unit.facing
        };
        let palette_index = self.sim.player_team_palette_view(unit.owner);
        let native_unit_drawn = assets.art_tileset.draw_unit(
            unit.kind,
            unit.owner,
            palette_index,
            facing,
            unit.era_index,
            action,
            animation_progress,
            playback_duration_seconds,
            center,
            unit_zoom,
            unit.moves_left > 0,
        );
        if !native_unit_drawn {
            let color = player_color(palette_index, unit.owner);
            draw_ellipse(
                center.x,
                center.y - 9.0 * zoom,
                unit_w * 0.35,
                unit_h * 0.42,
                0.0,
                color,
            );
            draw_ellipse_lines(
                center.x,
                center.y - 9.0 * zoom,
                unit_w * 0.35,
                unit_h * 0.42,
                0.0,
                (1.5 * zoom).max(1.0),
                BLACK,
            );
            draw_line(
                center.x,
                center.y - unit_h * 0.55,
                center.x,
                center.y + unit_h * 0.25,
                (2.0 * zoom).max(1.0),
                WHITE,
            );
        }
        self.draw_unit_health_at_zoom(assets, unit, center, zoom);
        if stack_count > 1 {
            draw_circle(
                center.x + 31.0 * zoom,
                center.y - 8.0 * zoom,
                (8.0 * zoom).max(6.0),
                Color::from_rgba(25, 24, 21, 235),
            );
            draw_text(
                stack_count.to_string(),
                center.x + 27.0 * zoom,
                center.y - 4.0 * zoom,
                (13.0 * zoom).max(10.0),
                WHITE,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{centered_sprite_position, centered_text_baseline, city_size_text_color};
    use macroquad::prelude::{BLACK, Color, TextDimensions, WHITE, vec2};

    #[test]
    fn city_size_text_uses_black_on_light_and_white_on_dark_backgrounds() {
        assert_eq!(
            city_size_text_color(Color::from_rgba(240, 220, 180, 255)),
            BLACK
        );
        assert_eq!(
            city_size_text_color(Color::from_rgba(40, 70, 150, 255)),
            WHITE
        );
    }

    #[test]
    fn city_sprite_bounds_are_centered_on_the_tile() {
        let tile_center = vec2(320.0, 240.0);
        for size in [vec2(90.0, 54.0), vec2(69.0, 52.0)] {
            let top_left = centered_sprite_position(tile_center, size.x, size.y);
            assert_eq!(top_left + size * 0.5, tile_center);
        }
    }

    #[test]
    fn city_label_text_is_vertically_centered_in_its_row() {
        let metrics = TextDimensions {
            width: 40.0,
            height: 8.0,
            offset_y: 7.0,
        };
        let baseline = centered_text_baseline(20.0, 14.0, metrics);
        let raster_top = baseline - metrics.offset_y;
        assert_eq!(raster_top - 20.0, 3.0);
        assert_eq!(20.0 + 14.0 - (raster_top + metrics.height), 3.0);
    }
}
