//! Dedicated Civ III world painter and ordered map passes.

use super::*;

impl Game {
    pub(in crate::client) fn draw_civ3_world_map(&self, assets: &Assets) {
        let viewport = Self::viewport_rect();
        draw_rectangle(
            viewport.x,
            viewport.y,
            viewport.w,
            viewport.h,
            map_view::MAP_UNKNOWN,
        );

        let tile_w = TILE_W * self.zoom;
        let tile_h = TILE_H * self.zoom;
        // Native rows are already in painter order. Cull before collecting and
        // project each visible tile once for both terrain passes.
        let order = visible_world_tiles(
            self.sim.map_wrap,
            viewport,
            self.camera_overview_position(),
            self.zoom,
            self.renderer.as_ref(),
        );
        let mut cities = std::collections::HashMap::new();
        let city_overrides = self.playback.city_overrides();
        let visible_tiles: std::collections::HashSet<_> =
            order.iter().map(|&(x, y, _)| (x, y)).collect();
        let observed_cities = self.sim.render_cities_where(|city| {
            visible_tiles.contains(&(city.x, city.y)) && !city_overrides.contains_key(&city.id)
        });
        let overridden_cities: Vec<_> = city_overrides
            .values()
            .filter(|city| visible_tiles.contains(&(city.x, city.y)))
            .map(|city| self.sim.observed_city(city))
            .collect();
        for observed in observed_cities
            .iter()
            .filter(|c| !city_overrides.contains_key(&c.city.id))
            .chain(overridden_cities.iter())
        {
            cities
                .entry((observed.city.x, observed.city.y))
                .or_insert(observed);
        }
        let mut stacks: std::collections::HashMap<(i32, i32), (&Unit, usize)> =
            std::collections::HashMap::new();
        let overrides = self.playback.overrides();
        for unit in self
            .sim
            .units()
            .iter()
            .filter(|u| !overrides.contains_key(&u.id))
            .chain(overrides.values())
            .filter(|u| {
                self.sim.unit_visible_to_human(u)
                    && (u.transport.is_none() || Some(u.id) == self.selected_unit)
            })
        {
            let entry = stacks.entry((unit.x, unit.y)).or_insert((unit, 0));
            entry.1 += 1;
            let priority = |u: &Unit| {
                (
                    u8::from(!self.playback.is_active(u.id)),
                    unit_draw_priority(u, self.selected_unit),
                )
            };
            if priority(unit) < priority(entry.0) {
                entry.0 = unit;
            }
        }
        self.draw_edge_water(assets, viewport, tile_w, tile_h);
        // Draw every tile base first. Tall terrain sprites are a separate
        // layer so later diamond bases cannot cut horizontal bands through
        // forests, hills, and mountains.
        for &(x, y, center) in &order {
            let tile = self.sim.render_tile(x, y);
            if !tile.seen {
                draw_diamond(center, tile_w, tile_h, BLACK);
                continue;
            }

            self.draw_map_base(assets, x, y, center, tile_w, tile_h);
        }

        for layer in map_layers::MAP_LAYERS {
            for &(x, y, center) in &order {
                if !self.sim.render_tile(x, y).seen {
                    continue;
                }
                let city = cities.get(&(x, y)).copied();
                if layer == map_layers::MapLayer::City {
                    if let Some(city) = city {
                        self.draw_observed_city(assets, city, center, self.zoom);
                    }
                } else {
                    self.draw_map_layer(
                        assets,
                        layer,
                        x,
                        y,
                        TileGeometry::new(center, tile_w, tile_h),
                        city.is_some(),
                    );
                }
            }
        }
        // Reference Fog layer: blend corners after terrain/cities and before
        // units, labels and focus sprites. Visual water inherits polar-row fog.
        for &(x, y, center) in &order {
            let (mx, my) = native_to_map(x, y);
            self.draw_corner_fog(assets, center, tile_w, tile_h, |dx, dy| {
                let (nx, ny) = map_to_native(mx + dx, my + dy);
                self.world_fog_knowledge(nx, ny)
            });
        }
        // Complete corner fog for the decorative rows and their southern edge.
        let bottom = self.sim.map_wrap.height() + map_view::EDGE_WATER_ROWS;
        let bottom_corner = (!self.sim.map_wrap.wraps_y()).then_some(bottom);
        for y in self.edge_water_rows().chain(bottom_corner) {
            for x in 0..self.sim.map_wrap.width() {
                let center = self.world_screen_position(x, y);
                if center.y > viewport.bottom() + tile_h || center.y < viewport.y - tile_h {
                    continue;
                }
                let (mx, my) = native_to_map(x, y);
                self.draw_corner_fog(assets, center, tile_w, tile_h, |dx, dy| {
                    let (nx, ny) = map_to_native(mx + dx, my + dy);
                    self.world_fog_knowledge(nx, ny)
                });
            }
        }
        for &(x, y, center) in &order {
            let tile = self.sim.render_tile(x, y);
            if !tile.visible {
                continue;
            }
            if let Some(&(unit, count)) = stacks.get(&(x, y)) {
                // Reference find_visible_unit hides idle city garrisons.
                // Focused units are drawn in their own final layer below.
                if !cities.contains_key(&(x, y))
                    && Some(unit.id) != self.selected_unit
                    && !self.playback.is_active(unit.id)
                {
                    self.draw_unit(assets, unit, center, count);
                }
            }
        }
        // Special3 is a complete foreground pass after ordinary units.
        for &(x, y, center) in &order {
            self.draw_visible_base_foreground(assets, x, y, center, tile_w, tile_h);
        }
        // Freeciv draws city descriptions in a separate CITYBAR layer.
        // Keep text above units and foreground terrain.
        for &(x, y, center) in &order {
            let tile = self.sim.render_tile(x, y);
            if tile.seen
                && let Some(city) = cities.get(&(x, y)).copied()
            {
                self.draw_city_label(&city.city, center);
            }
        }
        // Freeciv's FocusUnit layer follows city bars and other map objects.
        for &(x, y, center) in &order {
            if self.sim.render_tile(x, y).visible
                && let Some(&(unit, count)) = stacks.get(&(x, y))
                && Some(unit.id) == self.selected_unit
                && !self.playback.is_active(unit.id)
            {
                self.draw_unit(assets, unit, center, count);
            }
        }
        self.draw_playback(assets);
        self.draw_goto_preview(assets);
    }
}
