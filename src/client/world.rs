use super::*;

impl Game {
    fn ocean_match_type(&self, x: i32, y: i32) -> u8 {
        match self.sim.render_tile(x, y).terrain {
            Terrain::DeepOcean => 0,
            Terrain::Ocean | Terrain::Lake | Terrain::Glacier => 1,
            _ => 2,
        }
    }

    fn ocean_neighbor_match(&self, x: i32, y: i32, dx: i32, dy: i32, current: u8) -> u8 {
        let (mx, my) = native_to_map(x, y);
        let (neighbor_x, neighbor_y) = map_to_native(mx + dx, my + dy);
        let Some((neighbor_x, neighbor_y)) = self.sim.map_wrap.normalize(neighbor_x, neighbor_y)
        else {
            return current;
        };
        if self.sim.render_tile(neighbor_x, neighbor_y).seen {
            self.ocean_match_type(neighbor_x, neighbor_y)
        } else {
            current
        }
    }

    fn draw_ocean_sprite(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
        tint: Color,
    ) {
        let current = self.ocean_match_type(x, y);
        let nearby = |dx, dy| self.ocean_neighbor_match(x, y, dx, dy, current);

        // Each visual corner belongs to four map tiles. Amplio2 stores all
        // land/shallow/deep combinations as a four-terrain cell group; the
        // generated corner atlas contributes only this tile's quarter.
        let groups = [
            [nearby(-1, -1), nearby(0, -1), current, nearby(-1, 0)],
            [nearby(0, -1), nearby(1, -1), nearby(1, 0), current],
            [current, nearby(1, 0), nearby(1, 1), nearby(0, 1)],
            [nearby(-1, 0), current, nearby(0, 1), nearby(-1, 1)],
        ];

        for (corner, matches) in groups.into_iter().enumerate() {
            let (source_row, source_col) = ocean_cell_source(matches);
            draw_texture_ex(
                &assets.ocean_corners,
                center.x - tile_w * 0.5,
                center.y - tile_h * 0.5,
                tint,
                DrawTextureParams {
                    dest_size: Some(vec2(tile_w, tile_h)),
                    source: Some(isolated_sprite_source(
                        (corner * 9 + source_row) as i32,
                        source_col as i32,
                        96.0,
                        48.0,
                    )),
                    ..Default::default()
                },
            );
        }
    }

    fn draw_terrain_sprite(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
    ) {
        let terrain = self.sim.render_tile(x, y).terrain;
        let tint = WHITE;

        if terrain == Terrain::Lake {
            self.draw_lake_sprite(assets, x, y, center, tile_w, tile_h);
            return;
        }
        if terrain.is_water() {
            self.draw_ocean_sprite(assets, x, y, center, tile_w, tile_h, tint);
            return;
        }

        let row = terrain.sprite_row().expect("land terrain sprite row");
        draw_texture_ex(
            &assets.terrain,
            center.x - tile_w * 0.5,
            center.y - tile_h * 0.5,
            tint,
            DrawTextureParams {
                dest_size: Some(vec2(tile_w, tile_h)),
                source: Some(isolated_sprite_source(row, 0, 96.0, 48.0)),
                ..Default::default()
            },
        );
    }

    fn draw_terrain_extra(
        &self,
        texture: &Texture2D,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
        source_row: i32,
        source_col: i32,
    ) {
        draw_texture_ex(
            texture,
            center.x - tile_w * 0.5,
            center.y - tile_h * 0.5,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(tile_w, tile_h)),
                source: Some(isolated_sprite_source(source_row, source_col, 96.0, 48.0)),
                ..Default::default()
            },
        );
    }

    fn draw_base_sprite(&self, assets: &Assets, center: Vec2, w: f32, h: f32, row: i32, col: i32) {
        draw_texture_ex(
            &assets.bases,
            center.x - w * 0.5,
            center.y - h,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(w, h * 1.5)),
                source: Some(bordered_sprite_source(row, col, 96.0, 72.0)),
                ..Default::default()
            },
        );
    }

    fn draw_river(&self, assets: &Assets, x: i32, y: i32, center: Vec2, tile_w: f32, tile_h: f32) {
        let tile = self.sim.render_tile(x, y);
        let water = tile.terrain.is_water();
        if !water && !tile.river {
            return;
        }

        let neighbors = ISOHEX_DIRS.map(|(dx, dy)| {
            let Some((nx, ny)) = render_neighbor(self.sim.map_wrap, x, y, dx, dy) else {
                return super::rivers::RiverNeighbor {
                    known: false,
                    river: false,
                    water: false,
                };
            };
            let tile = self.sim.render_tile(nx, ny);
            super::rivers::RiverNeighbor {
                known: tile.seen,
                river: tile.river,
                water: tile.terrain.is_water(),
            }
        });
        let mask = super::rivers::connection_mask(water, neighbors);
        let draw_sprite = |source| {
            draw_texture_ex(
                &assets.rivers,
                center.x - tile_w * 0.5,
                center.y - tile_h * 0.5,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(tile_w, tile_h)),
                    source: Some(source),
                    ..Default::default()
                },
            )
        };
        if water {
            for direction in 0..6 {
                if mask & (1 << direction) != 0 {
                    draw_sprite(super::rivers::mouth_sprite(direction));
                }
            }
        } else {
            draw_sprite(super::rivers::river_sprite(mask));
        }
    }

    fn draw_road_sprites(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
        has_city: bool,
    ) {
        let neighbors = ISOHEX_DIRS.map(|(dx, dy)| {
            render_neighbor(self.sim.map_wrap, x, y, dx, dy).map_or((false, false), |(nx, ny)| {
                let tile = self.sim.render_tile(nx, ny);
                (tile.seen && tile.road, tile.seen && tile.railroad)
            })
        });
        for (row, col) in
            road_sprite_cells(self.sim.render_tile(x, y).railroad, has_city, neighbors)
        {
            self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, row, col);
        }
    }

    fn draw_territory_border(
        &self,
        x: i32,
        y: i32,
        owner: usize,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
    ) {
        let points = [
            vec2(center.x, center.y - tile_h * 0.5),
            vec2(center.x + tile_w * 0.5, center.y),
            vec2(center.x, center.y + tile_h * 0.5),
            vec2(center.x - tile_w * 0.5, center.y),
        ];
        let edge_neighbors = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let opacity = 0.95; // The shared fog layer handles remembered borders.
        let color = with_alpha(player_color(owner), opacity);
        let width = (3.5 * tile_w / TILE_W).clamp(2.75, 4.75);
        let shadow = Color::new(0.025, 0.035, 0.04, opacity * 0.9);
        let thread = Color::new(
            color.r * 0.45 + 0.55,
            color.g * 0.45 + 0.55,
            color.b * 0.45 + 0.55,
            opacity,
        );

        let mut geometry = self.border_geometry.borrow_mut();
        for (edge, (dx, dy)) in edge_neighbors.into_iter().enumerate() {
            if render_neighbor(self.sim.map_wrap, x, y, dx, dy)
                .is_some_and(|(nx, ny)| self.playback.tile_owner(&self.sim, nx, ny) == Some(owner))
            {
                continue;
            }
            geometry.draw(&points, edge, width, color, shadow, thread);
        }
    }

    pub(super) fn draw_world_amplio2(&self, assets: &Assets) {
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
        // Draw every tile base first. Tall terrain sprites are a separate
        // layer so later diamond bases cannot cut horizontal bands through
        // forests, hills, and mountains.
        for &(x, y, center) in &order {
            let tile = self.sim.render_tile(x, y);
            if !tile.seen {
                draw_diamond(center, tile_w, tile_h, BLACK);
                continue;
            }

            if tile.terrain.is_water() {
                assets.draw_water_backing(tile.terrain, center, tile_w, tile_h);
            } else {
                draw_diamond(center, tile_w, tile_h, tile.terrain.minimap_color());
            }
            self.draw_terrain_sprite(assets, x, y, center, tile_w, tile_h);
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
                        center,
                        tile_w,
                        tile_h,
                        city.is_some(),
                    );
                }
            }
        }
        // Reference Fog layer: blend corners after terrain/cities and before
        // units, labels and focus sprites. Out-of-map poles are remembered.
        for &(x, y, center) in &order {
            let (mx, my) = native_to_map(x, y);
            self.draw_corner_fog(assets, center, tile_w, tile_h, |dx, dy| {
                let (nx, ny) = map_to_native(mx + dx, my + dy);
                self.fog_knowledge(nx, ny)
            });
        }
        // Bottom polar corners have no south tile to drive the ordinary
        // loop, but still blend the last row with the off-map remembered edge.
        for y in MAP_H..if self.sim.map_wrap.wraps_y() {
            MAP_H
        } else {
            MAP_H + 2
        } {
            for x in 0..MAP_W {
                let center = world_screen_position(
                    self.sim.map_wrap,
                    viewport,
                    self.camera_overview_position(),
                    self.zoom,
                    x,
                    y,
                );
                if center.y > viewport.bottom() + tile_h || center.y < viewport.y - tile_h {
                    continue;
                }
                let (mx, my) = native_to_map(x, y);
                self.draw_corner_fog(assets, center, tile_w, tile_h, |dx, dy| {
                    let (nx, ny) = map_to_native(mx + dx, my + dy);
                    self.fog_knowledge(nx, ny)
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

    fn draw_visible_base_foreground(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        w: f32,
        h: f32,
    ) {
        let tile = self.sim.render_tile(x, y);
        if tile.visible
            && let Some(Base::Fort | Base::Fortress) = tile.base
        {
            let row = i32::from(tile.base == Some(Base::Fort));
            self.draw_base_sprite(assets, center, w, h, row, 3);
        }
    }

    /// The same ordered passes drive world rendering and the city work map.
    fn draw_map_layer(
        &self,
        assets: &Assets,
        layer: map_layers::MapLayer,
        x: i32,
        y: i32,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
        has_city: bool,
    ) {
        use map_layers::MapLayer;
        let tile = self.sim.render_tile(x, y);
        match layer {
            MapLayer::Blend => self.draw_terrain_blending(assets, x, y, center, tile_w, tile_h),
            MapLayer::Terrain => {
                self.draw_matched_terrain_overlay(assets, x, y, center, tile_w, tile_h)
            }
            MapLayer::Water => {
                // Native fill_irrigation_sprite_array suppresses fields under cities.
                if !has_city {
                    if tile.farmland {
                        self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 2, 6);
                    } else if tile.irrigation {
                        self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 1, 6);
                    }
                }
                self.draw_river(assets, x, y, center, tile_w, tile_h);
            }
            MapLayer::Roads => {
                if tile.road {
                    self.draw_road_sprites(assets, x, y, center, tile_w, tile_h, has_city);
                }
            }
            MapLayer::Special1 => {
                if let Some(Base::Fort | Base::Fortress) = tile.base {
                    let row = i32::from(tile.base == Some(Base::Fort));
                    self.draw_base_sprite(assets, center, tile_w, tile_h, row, 4);
                }
                if tile.oil_platform {
                    self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 7, 6);
                } else if tile.oil_well {
                    self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 0, 6);
                } else if tile.mine {
                    self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 3, 6);
                }
                if tile.hut {
                    self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 5, 6);
                }
                if let Some(resource) = tile.active_resource() {
                    let (row, col) = resource.sprite(tile.terrain);
                    self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, row, col);
                }
            }
            MapLayer::Grid => {
                if let Some(owner) = self.playback.tile_owner(&self.sim, x, y) {
                    self.draw_territory_border(x, y, owner, center, tile_w, tile_h);
                }
            }
            MapLayer::City => {}
            MapLayer::Special2 => {
                if let Some(Base::Airstrip | Base::Airbase) = tile.base {
                    let row = i32::from(tile.base == Some(Base::Airstrip));
                    self.draw_base_sprite(assets, center, tile_w, tile_h, row, 0);
                }
                if tile.pollution {
                    self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 4, 6);
                }
                if tile.fallout {
                    self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 6, 6);
                }
                if !tile.visible
                    && let Some(Base::Fort | Base::Fortress) = tile.base
                {
                    let row = i32::from(tile.base == Some(Base::Fort));
                    self.draw_base_sprite(assets, center, tile_w, tile_h, row, 3);
                }
            }
        }
    }

    pub(super) fn draw_city_map(
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
                let (x, y) = normalized.unwrap_or((x.rem_euclid(MAP_W), y));
                tiles.push((dx, dy, x, y, center, relevant));
            }
        }
        tiles.sort_by(|a, b| a.4.y.total_cmp(&b.4.y));
        for &(_, _, x, y, center, relevant) in &tiles {
            if !relevant || !self.sim.render_tile(x, y).seen {
                draw_diamond(center, layout.tile_w, layout.tile_h, BLACK);
            } else {
                let tile = self.sim.render_tile(x, y);
                if tile.terrain.is_water() {
                    assets.draw_water_backing(tile.terrain, center, layout.tile_w, layout.tile_h);
                } else {
                    draw_diamond(
                        center,
                        layout.tile_w,
                        layout.tile_h,
                        tile.terrain.minimap_color(),
                    );
                }
                self.draw_terrain_sprite(assets, x, y, center, layout.tile_w, layout.tile_h);
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
                    self.draw_map_layer(
                        assets,
                        layer,
                        x,
                        y,
                        center,
                        layout.tile_w,
                        layout.tile_h,
                        is_city,
                    );
                }
            }
        }
        for &(dx, dy, _, _, center, _) in &tiles {
            let (mx, my) = native_to_map(city.x, city.y);
            self.draw_corner_fog(assets, center, layout.tile_w, layout.tile_h, |ox, oy| {
                let (nx, ny) = map_to_native(mx + dx + ox, my + dy + oy);
                if !radius.contains(&(dx + ox, dy + oy)) {
                    super::fog::Knowledge::Unknown
                } else {
                    self.fog_knowledge(nx, ny)
                }
            });
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

    fn draw_observed_city(
        &self,
        assets: &Assets,
        observed: &ObservedCity,
        center: Vec2,
        zoom: f32,
    ) {
        let (row, col) = observed.art_cell;
        let city_w = 69.0 * zoom;
        let city_h = 52.0 * zoom;
        draw_texture_ex(
            &assets.cities,
            center.x - city_w * 0.5,
            center.y - city_h + 9.0 * zoom,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(city_w, city_h)),
                source: Some(isolated_sprite_source(row, col, 96.0, 72.0)),
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

    fn draw_city_label(&self, city: &City, center: Vec2) {
        let zoom = self.zoom;
        let label = format!("{}  {}", city.name, city.size);
        let font_size = (14.0 * zoom).max(10.0);
        let metrics = measure_text(&label, None, font_size as u16, 1.0);
        // The reference selection ring is tile-sized; its bottom must clear
        // the label too, regardless of which unit is currently focused.
        let top = super::city_bar::label_top(center.y, zoom);
        let padding = 3.0;
        let detail = self.city_bar_detail(city);
        let detail_font = (11.0 * zoom).max(9.0);
        let detail_width = detail.as_ref().map_or(0.0, |text| {
            measure_text(text, None, detail_font as u16, 1.0).width
        });
        let width = metrics.width.max(detail_width);
        let detail_height = if detail.is_some() {
            detail_font + padding
        } else {
            0.0
        };
        let height = metrics.height + padding * 2.0 + detail_height;
        let left = center.x - width * 0.5 - 6.0;
        draw_rectangle(
            left,
            top,
            width + 12.0,
            height,
            Color::from_rgba(19, 20, 18, 220),
        );
        draw_rectangle(left, top, 3.0, height, player_color(city.owner));
        draw_text(
            &label,
            center.x - metrics.width * 0.5,
            top + padding + metrics.offset_y,
            font_size,
            Color::from_rgba(244, 235, 201, 255),
        );
        if let Some(detail) = detail {
            text_in(
                &detail,
                center.x - detail_width * 0.5,
                top + metrics.height + padding * 2.0 + detail_font * 0.8,
                detail_font,
                width,
                Color::from_rgba(194, 205, 181, 255),
            );
        }
        // Banners use the owning civilization's gameplay color.
        let pole_x = left - 9.0;
        draw_line(pole_x, top + 1.0, pole_x, top + 16.0, 1.0, LIGHTGRAY);
        draw_triangle(
            vec2(pole_x, top + 1.0),
            vec2(pole_x + 8.0, top + 4.5),
            vec2(pole_x, top + 8.0),
            player_color(city.owner),
        );
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

    pub(super) fn draw_unit(&self, assets: &Assets, unit: &Unit, center: Vec2, stack_count: usize) {
        let (sprite_row, sprite_col) = unit_sprite(unit.kind);
        let unit_w = 58.0 * self.zoom;
        let unit_h = 44.0 * self.zoom;
        let is_selected = self.selected_unit == Some(unit.id);
        if is_selected {
            // Amplio2 unit.select:0..3, animated at Freeciv-web's 6 Hz.
            let frame = super::ui::selection_animation_frame();
            draw_texture_ex(
                &assets.selection,
                center.x - TILE_W * self.zoom * 0.5,
                center.y - TILE_H * self.zoom * 0.5,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(TILE_W * self.zoom, TILE_H * self.zoom)),
                    source: Some(Rect::new(frame as f32 * 96.0, 0.0, 96.0, 48.0)),
                    ..Default::default()
                },
            );
        }
        draw_texture_ex(
            &assets.units,
            center.x - unit_w * 0.5,
            center.y - unit_h + 9.0 * self.zoom,
            if unit.moves_left > 0 {
                WHITE
            } else {
                Color::new(0.62, 0.66, 0.68, 1.0)
            },
            DrawTextureParams {
                dest_size: Some(vec2(unit_w, unit_h)),
                source: Some(isolated_sprite_source(sprite_row, sprite_col, 64.0, 48.0)),
                ..Default::default()
            },
        );
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
                &stack_count.to_string(),
                center.x + 27.0 * self.zoom,
                center.y - 4.0 * self.zoom,
                (13.0 * self.zoom).max(10.0),
                WHITE,
            );
        }
    }
}

/// Rendering has horizontal seams and bounded polar rows.
fn render_neighbor(wrap: MapWrap, x: i32, y: i32, dx: i32, dy: i32) -> Option<(i32, i32)> {
    wrap.offset(x, y, dx, dy)
}

fn road_sprite_cells(rail: bool, city: bool, neighbors: [(bool, bool); 6]) -> Vec<(i32, i32)> {
    let cols = [1, 3, 4, 5, 7, 8];
    let mut cells = Vec::with_capacity(7);
    // Native roads are complete before rails; raw road must not cover rail centers.
    for (index, &(road, other_rail)) in neighbors.iter().enumerate() {
        if road && !(rail && other_rail) {
            cells.push((12, cols[index]));
        }
    }
    if rail {
        for (index, &(_, other_rail)) in neighbors.iter().enumerate() {
            if other_rail {
                cells.push((13, cols[index]));
            }
        }
        if !city && !neighbors.iter().any(|n| n.1) {
            cells.push((13, 0));
        }
    } else if !city && !neighbors.iter().any(|n| n.0) {
        cells.push((12, 0));
    }
    cells
}

#[cfg(test)]
mod infrastructure_tests {
    use super::*;
    #[test]
    fn map_connections_stop_at_poles_and_still_wrap_horizontally() {
        assert_eq!(render_neighbor(MapWrap::X, 0, 0, 0, -1), None);
        assert_eq!(render_neighbor(MapWrap::X, 0, MAP_H - 1, 1, 0), None);
        assert_eq!(
            render_neighbor(MapWrap::X, 0, 0, -1, 1),
            Some((MAP_W - 1, 0))
        );
    }
    #[test]
    fn mixed_rail_junction_finishes_roads_before_rails_and_keeps_isolated_rail() {
        let mut near = [(false, false); 6];
        near[0] = (true, true);
        near[1] = (true, false);
        assert_eq!(road_sprite_cells(true, false, near), [(12, 3), (13, 1)]);
        near[0] = (false, false);
        assert_eq!(road_sprite_cells(true, false, near), [(12, 3), (13, 0)]);
        assert!(road_sprite_cells(false, true, [(false, false); 6]).is_empty());
    }
}

/// Isometric normal-tile centers relative to the exact viewport center,
/// matching mapview_common.c::center_tile_mapcanvas and tile_to_canvas_pos.
pub(super) fn world_screen_position(
    wrap: MapWrap,
    viewport: Rect,
    camera: Vec2,
    zoom: f32,
    x: i32,
    y: i32,
) -> Vec2 {
    let tile_x = (2 * x + y.rem_euclid(2)) as f32;
    let half_width = MAP_W as f32;
    let dx = (tile_x - camera.x + half_width).rem_euclid(2.0 * half_width) - half_width;
    let dy = y as f32 - camera.y;
    let dy = if wrap.wraps_y() {
        (dy + MAP_H as f32 * 0.5).rem_euclid(MAP_H as f32) - MAP_H as f32 * 0.5
    } else {
        dy
    };
    vec2(
        viewport.x + viewport.w * 0.5 + dx * TILE_W * 0.5 * zoom,
        viewport.y + viewport.h * 0.5 + dy * TILE_H * 0.5 * zoom,
    )
}
fn visible_world_tiles(
    wrap: MapWrap,
    viewport: Rect,
    camera: Vec2,
    zoom: f32,
) -> Vec<(i32, i32, Vec2)> {
    let origin = viewport.y + viewport.h * 0.5;
    let step = TILE_H * 0.5 * zoom;
    let first = (camera.y + (viewport.y - 80.0 - origin) / step)
        .floor()
        .max(0.0) as i32;
    let last = (camera.y + (viewport.bottom() + 80.0 - origin) / step)
        .ceil()
        .min((MAP_H - 1) as f32) as i32;
    let (first, last) = if wrap.wraps_y() {
        (0, MAP_H - 1)
    } else {
        (first, last)
    };
    let mut tiles = Vec::with_capacity(((last - first + 1).max(0) * MAP_W) as usize);
    for y in first..=last {
        for x in 0..MAP_W {
            let center = world_screen_position(wrap, viewport, camera, zoom, x, y);
            if center.x < viewport.x - TILE_W * zoom
                || center.x > viewport.right() + TILE_W * zoom
                || center.y < viewport.y - 80.0
                || center.y > viewport.bottom() + 80.0
            {
                continue;
            }
            tiles.push((x, y, center));
        }
    }
    if wrap.wraps_y() {
        tiles.sort_by(|a, b| a.2.y.total_cmp(&b.2.y));
    }
    tiles
}
#[cfg(test)]
mod visibility_tests {
    use super::*;
    #[test]
    fn row_culling_preserves_original_tiles_and_painter_order_at_wrap_and_poles() {
        for viewport in [
            Rect::new(0.0, 46.0, 484.0, 426.0),
            Rect::new(0.0, 46.0, 1284.0, 726.0),
            Rect::new(0.0, 46.0, 2800.0, 1700.0),
        ] {
            for camera in [
                vec2(0.0, 0.0),
                vec2(2.0 * MAP_W as f32 - 0.2, (MAP_H - 1) as f32),
                vec2(23.4, 18.8),
            ] {
                for zoom in [MIN_ZOOM, START_ZOOM, MAX_ZOOM] {
                    let mut reference: Vec<_> = (0..MAP_H)
                        .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
                        .map(|(x, y)| {
                            (
                                x,
                                y,
                                world_screen_position(MapWrap::X, viewport, camera, zoom, x, y),
                            )
                        })
                        .collect();
                    reference.sort_by(|a, b| a.2.y.total_cmp(&b.2.y));
                    reference.retain(|(_, _, p)| {
                        !(p.x < viewport.x - TILE_W * zoom
                            || p.x > viewport.right() + TILE_W * zoom
                            || p.y < viewport.y - 80.0
                            || p.y > viewport.bottom() + 80.0)
                    });
                    assert_eq!(
                        visible_world_tiles(MapWrap::X, viewport, camera, zoom),
                        reference
                    );
                }
            }
        }
    }
}

// control.c::find_visible_unit: focus first, then owned transporter, owned
// unit, other transporter, other unit. Carried units are never representatives.
fn unit_draw_priority(unit: &Unit, selected: Option<u32>) -> u8 {
    if Some(unit.id) == selected {
        0
    } else if unit.transport.is_some() {
        5
    } else if unit.owner == HUMAN && unit.kind.transport_capacity() > 0 {
        1
    } else if unit.owner == HUMAN {
        2
    } else if unit.kind.transport_capacity() > 0 {
        3
    } else {
        4
    }
}

#[cfg(test)]
mod representative_tests {
    use super::*;
    #[test]
    fn reference_stack_order_prefers_focus_then_owned_transports_then_owned_units() {
        let sim = Simulation::new(crate::mapgen::generate(42));
        let mut own_unit = sim.units()[0].clone();
        own_unit.owner = HUMAN;
        own_unit.kind = UnitKind::Warrior;
        let mut own_ship = own_unit.clone();
        own_ship.id += 1;
        own_ship.kind = UnitKind::Transport;
        let mut other_unit = own_unit.clone();
        other_unit.owner = 1;
        other_unit.id += 2;
        let mut other_ship = other_unit.clone();
        other_ship.id += 1;
        other_ship.kind = UnitKind::Transport;
        let mut cargo = own_unit.clone();
        cargo.id += 4;
        cargo.transport = Some(own_ship.id);
        let units = [&cargo, &other_unit, &other_ship, &own_unit, &own_ship];
        let mut sorted = units.to_vec();
        sorted.sort_by_key(|u| unit_draw_priority(u, None));
        assert_eq!(
            sorted.iter().map(|u| u.id).collect::<Vec<_>>(),
            vec![
                own_ship.id,
                own_unit.id,
                other_ship.id,
                other_unit.id,
                cargo.id
            ]
        );
        assert_eq!(
            units
                .iter()
                .min_by_key(|u| unit_draw_priority(u, Some(cargo.id)))
                .unwrap()
                .id,
            cargo.id
        );
    }
}

#[cfg(test)]
mod wrap_projection_tests {
    use super::*;
    #[test]
    fn torus_rows_cross_the_vertical_seam_in_painter_order() {
        let viewport = Rect::new(0.0, 46.0, 964.0, 550.0);
        let camera = vec2(20.0, MAP_H as f32 - 0.2);
        let torus = visible_world_tiles(MapWrap::XY, viewport, camera, 1.0);
        assert!(torus.iter().any(|t| t.1 == 0));
        assert!(torus.iter().any(|t| t.1 == MAP_H - 1));
        assert!(torus.windows(2).all(|p| p[0].2.y <= p[1].2.y));
        let cylinder = visible_world_tiles(MapWrap::X, viewport, camera, 1.0);
        assert!(cylinder.iter().all(|t| t.1 > MAP_H / 2));
        let a = world_screen_position(MapWrap::XY, viewport, camera, 1.0, 10, MAP_H - 1);
        let (x, y) = MapWrap::XY.offset(10, MAP_H - 1, 1, 0).unwrap();
        let b = world_screen_position(MapWrap::XY, viewport, camera, 1.0, x, y);
        assert!((a.y - b.y).abs() <= TILE_H * 0.5 + 0.01);
    }
}
