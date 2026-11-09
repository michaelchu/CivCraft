//! Rendering of cities, units, and other world entities.

use super::*;

fn centered_sprite_position(center: Vec2, width: f32, height: f32) -> Vec2 {
    center - vec2(width, height) * 0.5
}

fn centered_text_baseline(row_top: f32, row_height: f32, metrics: TextDimensions) -> f32 {
    row_top + (row_height - metrics.height) * 0.5 + metrics.offset_y
}

impl Game {
    pub(in crate::client) fn draw_city_map(
        &self,
        assets: &Assets,
        city: &City,
        layout: &super::city_map::CityMapLayout,
    ) {
        let bounds = layout.bounds;
        let scale = screen_dpi_scale();
        // QuadGl records clipping per draw call. Coordinates are physical pixels
        // from the top-left, so clip both terrain overhangs and surrounding fog.
        unsafe {
            get_internal_gl().quad_gl.scissor(Some((
                (bounds.x * scale) as i32,
                (bounds.y * scale) as i32,
                (bounds.w * scale) as i32,
                (bounds.h * scale) as i32,
            )));
        }
        draw_rectangle(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            map_view::MAP_UNKNOWN,
        );
        let radius: std::collections::HashSet<_> = self
            .sim
            .map_city_radius(city)
            .map(|(dx, dy, _, _)| (dx, dy))
            .collect();
        let span =
            ((bounds.w / layout.tile_w + bounds.h / layout.tile_h).ceil() as i32 + 3).min(24);
        let mut tiles = Vec::new();
        for dy in -span..=span {
            for dx in -span..=span {
                let center = layout.center(dx, dy);
                if center.x + layout.tile_w < bounds.x
                    || center.x - layout.tile_w > bounds.right()
                    || center.y + layout.tile_h < bounds.y
                    || center.y - layout.tile_h * 2.0 > bounds.bottom()
                {
                    continue;
                }
                let (mx, my) = native_to_map(city.x, city.y);
                let (x, y) = map_to_native(mx + dx, my + dy);
                let normalized = self.sim.map_wrap.normalize(x, y);
                let relevant = normalized.is_some() && radius.contains(&(dx, dy));
                let (x, y) = normalized.unwrap_or((x.rem_euclid(self.sim.map_wrap.width()), y));
                tiles.push((dx, dy, x, y, center, relevant));
            }
        }
        tiles.sort_by(|a, b| a.4.y.total_cmp(&b.4.y));
        let painter = super::civ3::Civ3WorldPainter::new(self, assets);
        for &(_, _, x, y, center, relevant) in &tiles {
            if !relevant || !self.sim.render_tile(x, y).seen {
                draw_diamond(center, layout.tile_w, layout.tile_h, BLACK);
            } else {
                painter.draw_map_base(x, y, center, layout.tile_w, layout.tile_h);
            }
        }
        for layer in map_layers::MAP_LAYERS {
            for &(_, _, x, y, center, relevant) in &tiles {
                if !relevant || !self.sim.render_tile(x, y).seen {
                    continue;
                }
                let is_city = (x, y) == (city.x, city.y);
                if layer == map_layers::MapLayer::City {
                    if is_city {
                        self.draw_city_at_scale(assets, city, center, layout.tile_w / TILE_W);
                    }
                } else {
                    painter.draw_map_layer(
                        layer,
                        x,
                        y,
                        TileGeometry::new(center, layout.tile_w, layout.tile_h),
                        is_city,
                    );
                }
            }
        }
        for &(_, _, x, y, center, _) in &tiles {
            let city_center = (city.x, city.y);
            self.draw_fog_intersection(
                assets,
                center,
                layout.tile_w,
                layout.tile_h,
                (x, y),
                None,
                |nx, ny| {
                    if self
                        .sim
                        .map_wrap
                        .civ3_distance(city_center.0, city_center.1, nx, ny)
                        > 2
                    {
                        super::fog::Knowledge::Unknown
                    } else {
                        self.fog_knowledge(nx, ny)
                    }
                },
            );
        }
        for &(_, _, x, y, center, relevant) in &tiles {
            if relevant {
                self.draw_visible_base_foreground(
                    assets,
                    x,
                    y,
                    center,
                    layout.tile_w,
                    layout.tile_h,
                );
            }
        }
        unsafe {
            get_internal_gl().quad_gl.scissor(None);
        }
    }

    fn draw_city_at_scale(&self, assets: &Assets, city: &City, center: Vec2, zoom: f32) {
        let observed = self.sim.observed_city(city);
        self.draw_observed_city(assets, &observed, center, zoom);
    }

    pub(super) fn draw_observed_city(
        &self,
        assets: &Assets,
        observed: &ObservedCity,
        center: Vec2,
        zoom: f32,
    ) {
        // Civ III sheets are four eras by three population bands.
        let col = match observed.city.size {
            1..=6 => 0,
            7..=12 => 1,
            _ => 2,
        };
        let row = i32::from(observed.civ3_era.min(3));
        let texture = assets
            .civ3
            .city_texture_at(observed.civ3_style, observed.city.owner);
        let source = Rect::new(col as f32 * 167.0, row as f32 * 95.0, 167.0, 95.0);
        let (city_w, city_h) = (90.0 * zoom, 54.0 * zoom);
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
        if observed.occupied {
            self.draw_city_overlay(assets, center, zoom, 2);
        }
        if observed.disorder {
            self.draw_city_overlay(assets, center, zoom, 5);
        }
    }

    pub(super) fn draw_city_label(&self, city: &City, center: Vec2) {
        let zoom = self.zoom;
        let detail = self.city_bar_detail(city);
        let top_text = detail.as_ref().map_or_else(
            || city.name.clone(),
            |detail| format!("{} : {}", city.name, detail.growth),
        );
        let center_font = (11.0 * zoom).max(9.0);
        let top_metrics = measure_text(&top_text, None, center_font as u16, 1.0);
        let bottom_metrics = detail
            .as_ref()
            .map(|detail| measure_text(&detail.production, None, center_font as u16, 1.0));
        // The reference selection ring is tile-sized; its bottom must clear
        // the label too, regardless of which unit is currently focused.
        let top = super::city_bar::label_top(center.y, zoom);
        let horizontal_padding = (6.0 * zoom).max(4.0);
        let vertical_padding = (3.0 * zoom).max(2.0);
        let center_width = (top_metrics
            .width
            .max(bottom_metrics.as_ref().map_or(0.0, |metrics| metrics.width))
            + horizontal_padding * 2.0)
            .max((94.0 * zoom).max(82.0));
        let row_height = (center_font + vertical_padding).max(12.0);
        let row_count = if detail.is_some() { 2.0 } else { 1.0 };
        let height = row_height * row_count + vertical_padding;
        let size_text = city.size.to_string();
        let size_font = (19.0 * zoom).max(14.0);
        let size_metrics = measure_text(&size_text, None, size_font as u16, 1.0);
        let star_radius = (7.0 * zoom).max(5.5);
        let side_padding = (4.0 * zoom).max(2.5);
        let side_width = size_metrics.width.max(star_radius * 2.0) + side_padding * 2.0;
        let width = center_width + side_width * 2.0;
        let left = center.x - width * 0.5;
        let owner_color = player_color(city.owner);

        // Civ III's city banner is a three-part bar: population, two centered
        // information rows, and a capital badge.
        draw_rectangle(left, top, side_width, height, owner_color);
        draw_rectangle(
            left + side_width,
            top,
            center_width,
            height,
            Color::from_rgba(19, 20, 18, 220),
        );
        draw_rectangle(
            left + side_width + center_width,
            top,
            side_width,
            height,
            owner_color,
        );
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
        draw_line(
            left + side_width + center_width,
            top,
            left + side_width + center_width,
            top + height,
            1.0,
            Color::from_rgba(15, 15, 13, 235),
        );
        if detail.is_some() {
            draw_line(
                left + side_width,
                top + row_height,
                left + side_width + center_width,
                top + row_height,
                1.0,
                Color::from_rgba(92, 92, 83, 210),
            );
        }

        draw_text(
            &size_text,
            left + (side_width - size_metrics.width) * 0.5,
            centered_text_baseline(top, height, size_metrics),
            size_font,
            WHITE,
        );

        let text_left = left + side_width;
        draw_text(
            &top_text,
            text_left + (center_width - top_metrics.width) * 0.5,
            centered_text_baseline(top, row_height, top_metrics),
            center_font,
            Color::from_rgba(244, 235, 201, 255),
        );
        if let (Some(detail), Some(metrics)) = (detail.as_ref(), bottom_metrics.as_ref()) {
            draw_text(
                &detail.production,
                text_left + (center_width - metrics.width) * 0.5,
                centered_text_baseline(top + row_height, row_height, *metrics),
                center_font,
                Color::from_rgba(222, 220, 202, 255),
            );
        }

        if city.buildings.contains(&Building::PALACE)
            || city.buildings.contains(&Building::ECCLESIASTICAL_PALACE)
        {
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

    pub(in crate::client) fn draw_unit(
        &self,
        assets: &Assets,
        unit: &Unit,
        center: Vec2,
        stack_count: usize,
        action: super::super::tileset::Civ3UnitAction,
        animation_progress: f32,
    ) {
        let unit_w = 58.0 * self.zoom;
        let unit_h = 44.0 * self.zoom;
        let is_selected = self.selected_unit == Some(unit.id);
        if is_selected {
            draw_diamond_lines(
                center,
                TILE_W * self.zoom * 0.94,
                TILE_H * self.zoom * 0.9,
                (2.5 * self.zoom).max(1.5),
                Color::from_rgba(255, 226, 88, 230),
            );
        }
        let native_unit_drawn = assets.civ3.draw_unit(
            unit.kind,
            unit.owner,
            unit.facing,
            action,
            animation_progress,
            center,
            self.zoom,
            unit.moves_left > 0,
        );
        if !native_unit_drawn {
            let color = if unit.moves_left > 0 {
                player_color(unit.owner)
            } else {
                Color::from_rgba(115, 120, 125, 255)
            };
            draw_ellipse(
                center.x,
                center.y - 9.0 * self.zoom,
                unit_w * 0.35,
                unit_h * 0.42,
                0.0,
                color,
            );
            draw_ellipse_lines(
                center.x,
                center.y - 9.0 * self.zoom,
                unit_w * 0.35,
                unit_h * 0.42,
                0.0,
                (1.5 * self.zoom).max(1.0),
                BLACK,
            );
            draw_line(
                center.x,
                center.y - unit_h * 0.55,
                center.x,
                center.y + unit_h * 0.25,
                (2.0 * self.zoom).max(1.0),
                WHITE,
            );
        }
        self.draw_unit_status(assets, unit, center);
        draw_circle(
            center.x - 21.0 * self.zoom,
            center.y - 25.0 * self.zoom,
            (5.0 * self.zoom).max(3.2),
            player_color(unit.owner),
        );
        if stack_count > 1 {
            draw_circle(
                center.x + 31.0 * self.zoom,
                center.y - 8.0 * self.zoom,
                (8.0 * self.zoom).max(6.0),
                Color::from_rgba(25, 24, 21, 235),
            );
            draw_text(
                stack_count.to_string(),
                center.x + 27.0 * self.zoom,
                center.y - 4.0 * self.zoom,
                (13.0 * self.zoom).max(10.0),
                WHITE,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{centered_sprite_position, centered_text_baseline};
    use macroquad::prelude::{TextDimensions, vec2};

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
