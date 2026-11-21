//! Minimap rendering and viewport navigation.

use super::*;

/// Terrain/fog changes with campaign commands; camera and playback markers do not.
#[derive(Default)]
pub(in crate::client) struct MinimapCache {
    revision: Option<u64>,
    texture: Option<Texture2D>,
    cities: Vec<crate::sim::MinimapCityView>,
}

fn minimap_image(sim: &ClientSimulation) -> Image {
    let wrap = sim.map_wrap();
    let width = 2 * wrap.width();
    let mut image = Image::gen_image_color(width as u16, wrap.height() as u16, BLACK);
    for y in 0..wrap.height() {
        for x in 0..wrap.width() {
            let tile = sim.minimap_tile_view(x, y);
            let color = if !tile.seen {
                Color::from_rgba(7, 13, 17, 255)
            } else if tile.visible {
                minimap_color(tile.terrain)
            } else {
                darken(minimap_color(tile.terrain), 0.53)
            };
            let pixel = 2 * x + y % 2;
            image.set_pixel(pixel as u32, y as u32, color);
            image.set_pixel(((pixel + 1) % width) as u32, y as u32, color);
        }
    }
    image
}

impl Game {
    pub(super) fn draw_minimap(&self) {
        let rect = self.minimap_rect();
        let scale = rect.h / self.sim.map_wrap().height() as f32;
        {
            let mut cache = self.minimap_cache.borrow_mut();
            if cache.revision != Some(self.sim.view_revision()) {
                let image = minimap_image(&self.sim);
                if let Some(texture) = &cache.texture {
                    texture.update(&image);
                } else {
                    let texture = Texture2D::from_image(&image);
                    texture.set_filter(FilterMode::Nearest);
                    cache.texture = Some(texture);
                }
                cache.cities = self.sim.minimap_city_views();
                cache.revision = Some(self.sim.view_revision());
            }
            draw_texture_ex(
                cache.texture.as_ref().unwrap(),
                rect.x,
                rect.y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(rect.w, rect.h)),
                    ..Default::default()
                },
            );
        }
        let city_overrides = self.playback.city_overrides();
        let override_views: Vec<_> = city_overrides
            .values()
            .map(|city| self.sim.minimap_city_override_view(city))
            .collect();
        let cache = self.minimap_cache.borrow();
        let cities = cache
            .cities
            .iter()
            .filter(|city| !city_overrides.contains_key(&city.id))
            .chain(override_views.iter());
        for city in cities {
            let origin = self.minimap_tile_origin(rect, city.x, city.y);
            let marker = Rect::new(origin.x, origin.y, 2.0 * scale, scale);
            let color = player_color(self.sim.player_team_palette_view(city.owner), city.owner);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimap_texture_preserves_staggered_tiles_and_horizontal_seam() {
        let mut sim =
            ClientSimulation::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
        sim.review_fixture().reveal_all_tiles();
        let image = minimap_image(&sim);
        let wrap = sim.map_wrap();
        assert_eq!(i32::from(image.width), 2 * wrap.width());
        assert_eq!(i32::from(image.height), wrap.height());
        for y in 0..wrap.height() {
            for x in 0..wrap.width() {
                let tile = sim.minimap_tile_view(x, y);
                let expected = if tile.visible {
                    minimap_color(tile.terrain)
                } else {
                    darken(minimap_color(tile.terrain), 0.53)
                };
                let pixel = (2 * x + y % 2) as u32;
                assert_eq!(
                    image.get_pixel(pixel, y as u32),
                    Color::from_rgba(
                        (expected.r * 255.0) as u8,
                        (expected.g * 255.0) as u8,
                        (expected.b * 255.0) as u8,
                        255
                    )
                );
                assert_eq!(
                    image.get_pixel(pixel, y as u32),
                    image.get_pixel((pixel + 1) % u32::from(image.width), y as u32)
                );
            }
        }
    }
}
