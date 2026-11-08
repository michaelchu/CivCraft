//! Dedicated Civ III world painter and ordered map passes.

use super::*;

/// Owns the Civ III world-pass orchestration for one frame.
///
/// The map-layer primitives still live on `Game`; keeping the frame inputs in
/// this renderer object gives the Civ III pass a concrete boundary while those
/// primitives are migrated out of the shared world module.
pub(super) struct Civ3WorldPainter<'a> {
    game: &'a Game,
    assets: &'a Assets,
}

impl<'a> Civ3WorldPainter<'a> {
    pub(super) fn new(game: &'a Game, assets: &'a Assets) -> Self {
        Self { game, assets }
    }

    pub(super) fn draw(&self) {
        let assets = self.assets;
        self.draw_ordered_world_passes(assets);
    }

    pub(super) fn draw_map_base(&self, x: i32, y: i32, center: Vec2, width: f32, height: f32) {
        let game = self.game;
        let assets = self.assets;
        if let Some(tileset) = &assets.civ3 {
            tileset.draw_base(game, x, y, center, vec2(width, height));
        } else {
            game.draw_map_base_fallback(assets, x, y, center, width, height);
        }
    }

    pub(super) fn draw_map_layer(
        &self,
        layer: map_layers::MapLayer,
        x: i32,
        y: i32,
        geometry: TileGeometry,
        has_city: bool,
    ) {
        match layer {
            map_layers::MapLayer::Blend if self.assets.civ3.is_some() => {}
            map_layers::MapLayer::Terrain => {
                let tile = self.game.sim.render_tile(x, y);
                let drawn = self.assets.civ3.as_ref().is_some_and(|tileset| {
                    tileset.draw_overlay(tile.terrain, (x, y), geometry.center, geometry.size())
                });
                if !drawn {
                    self.game
                        .draw_map_layer_fallback(self.assets, layer, x, y, geometry, has_city);
                }
            }
            _ => self
                .game
                .draw_map_layer_fallback(self.assets, layer, x, y, geometry, has_city),
        }
    }

    fn draw_ordered_world_passes(&self, assets: &Assets) {
        let game = self.game;
        let viewport = Game::viewport_rect();
        draw_rectangle(
            viewport.x,
            viewport.y,
            viewport.w,
            viewport.h,
            map_view::MAP_UNKNOWN,
        );

        let tile_w = TILE_W * game.zoom;
        let tile_h = TILE_H * game.zoom;
        // Native rows are already in painter order. Cull before collecting and
        // project each visible tile once for both terrain passes.
        let order = visible_world_tiles(
            game.sim.map_wrap,
            viewport,
            game.camera_overview_position(),
            game.zoom,
            game.renderer.as_ref(),
        );
        let mut cities = std::collections::HashMap::new();
        let city_overrides = game.playback.city_overrides();
        let visible_tiles: std::collections::HashSet<_> =
            order.iter().map(|&(x, y, _)| (x, y)).collect();
        let observed_cities = game.sim.render_cities_where(|city| {
            visible_tiles.contains(&(city.x, city.y)) && !city_overrides.contains_key(&city.id)
        });
        let overridden_cities: Vec<_> = city_overrides
            .values()
            .filter(|city| visible_tiles.contains(&(city.x, city.y)))
            .map(|city| game.sim.observed_city(city))
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
        let overrides = game.playback.overrides();
        for unit in game
            .sim
            .units()
            .iter()
            .filter(|u| !overrides.contains_key(&u.id))
            .chain(overrides.values())
            .filter(|u| {
                game.sim.unit_visible_to_human(u)
                    && (u.transport.is_none() || Some(u.id) == game.selected_unit)
            })
        {
            let entry = stacks.entry((unit.x, unit.y)).or_insert((unit, 0));
            entry.1 += 1;
            let priority = |u: &Unit| {
                (
                    u8::from(!game.playback.is_active(u.id)),
                    unit_draw_priority(u, game.selected_unit),
                )
            };
            if priority(unit) < priority(entry.0) {
                entry.0 = unit;
            }
        }
        game.draw_edge_water(assets, viewport, tile_w, tile_h);
        // Draw every tile base first. Tall terrain sprites are a separate
        // layer so later diamond bases cannot cut horizontal bands through
        // forests, hills, and mountains.
        for &(x, y, center) in &order {
            let tile = game.sim.render_tile(x, y);
            if !tile.seen {
                draw_diamond(center, tile_w, tile_h, BLACK);
                continue;
            }

            self.draw_map_base(x, y, center, tile_w, tile_h);
        }

        for layer in map_layers::MAP_LAYERS {
            for &(x, y, center) in &order {
                if !game.sim.render_tile(x, y).seen {
                    continue;
                }
                let city = cities.get(&(x, y)).copied();
                if layer == map_layers::MapLayer::City {
                    if let Some(city) = city {
                        game.draw_observed_city(assets, city, center, game.zoom);
                    }
                } else {
                    self.draw_map_layer(
                        layer,
                        x,
                        y,
                        TileGeometry::new(center, tile_w, tile_h),
                        city.is_some(),
                    );
                }
            }
        }
        // Blend corners after terrain/cities and before units, labels and focus
        // sprites. Visual water inherits polar-row fog.
        for &(x, y, center) in &order {
            let (mx, my) = native_to_map(x, y);
            game.draw_corner_fog(assets, center, tile_w, tile_h, |dx, dy| {
                let (nx, ny) = map_to_native(mx + dx, my + dy);
                game.world_fog_knowledge(nx, ny)
            });
        }
        // Complete corner fog for the decorative rows and their southern edge.
        let bottom = game.sim.map_wrap.height() + map_view::EDGE_WATER_ROWS;
        let bottom_corner = (!game.sim.map_wrap.wraps_y()).then_some(bottom);
        for y in game.edge_water_rows().chain(bottom_corner) {
            for x in 0..game.sim.map_wrap.width() {
                let center = game.world_screen_position(x, y);
                if center.y > viewport.bottom() + tile_h || center.y < viewport.y - tile_h {
                    continue;
                }
                let (mx, my) = native_to_map(x, y);
                game.draw_corner_fog(assets, center, tile_w, tile_h, |dx, dy| {
                    let (nx, ny) = map_to_native(mx + dx, my + dy);
                    game.world_fog_knowledge(nx, ny)
                });
            }
        }
        for &(x, y, center) in &order {
            let tile = game.sim.render_tile(x, y);
            if !tile.visible {
                continue;
            }
            if let Some(&(unit, count)) = stacks.get(&(x, y)) {
                // Idle city garrisons are hidden; focused units have a later
                // foreground layer.
                if !cities.contains_key(&(x, y))
                    && Some(unit.id) != game.selected_unit
                    && !game.playback.is_active(unit.id)
                {
                    game.draw_unit(assets, unit, center, count);
                }
            }
        }
        for &(x, y, center) in &order {
            game.draw_visible_base_foreground(assets, x, y, center, tile_w, tile_h);
        }
        for &(x, y, center) in &order {
            let tile = game.sim.render_tile(x, y);
            if tile.seen
                && let Some(city) = cities.get(&(x, y)).copied()
            {
                game.draw_city_label(&city.city, center);
            }
        }
        for &(x, y, center) in &order {
            if game.sim.render_tile(x, y).visible
                && let Some(&(unit, count)) = stacks.get(&(x, y))
                && Some(unit.id) == game.selected_unit
                && !game.playback.is_active(unit.id)
            {
                game.draw_unit(assets, unit, center, count);
            }
        }
        game.draw_playback(assets);
        game.draw_goto_preview(assets);
    }
}

impl Game {
    pub(in crate::client) fn draw_civ3_world_map(&self, assets: &Assets) {
        Civ3WorldPainter::new(self, assets).draw();
    }
}
