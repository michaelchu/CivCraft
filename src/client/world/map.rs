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
        geometry: TileGeometry,
        tint: Color,
    ) {
        let TileGeometry {
            center,
            width: tile_w,
            height: tile_h,
        } = geometry;
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
            assets.draw_ocean_corner(
                center,
                tile_w,
                tile_h,
                isolated_sprite_source(
                    (corner * 9 + source_row) as i32,
                    source_col as i32,
                    96.0,
                    48.0,
                ),
                tint,
            );
        }
    }

    pub(super) fn draw_map_base(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        width: f32,
        height: f32,
    ) {
        if let Some(tileset) = &assets.civ3 {
            tileset.draw_base(self, x, y, center, vec2(width, height));
            return;
        }
        let terrain = self.sim.render_tile(x, y).terrain;
        if terrain.is_water() || terrain == Terrain::Glacier {
            assets.draw_masked_terrain_backing(terrain, center, width, height);
        } else {
            draw_diamond(center, width, height, terrain.minimap_color());
        }
        self.draw_terrain_sprite(assets, x, y, center, width, height);
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
            self.draw_ocean_sprite(
                assets,
                x,
                y,
                TileGeometry::new(center, tile_w, tile_h),
                tint,
            );
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

    fn draw_resource_icon(
        &self,
        texture: &Texture2D,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
        row: i32,
        col: i32,
    ) {
        let size = tile_w * 0.38;
        let y = center.y - tile_h * 0.50;
        draw_texture_ex(
            texture,
            center.x - size * 0.5,
            y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(size, size)),
                source: Some(Rect::new(
                    col as f32 * 237.0,
                    row as f32 * 237.0,
                    237.0,
                    237.0,
                )),
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
        geometry: TileGeometry,
        has_city: bool,
    ) {
        let TileGeometry {
            center,
            width: tile_w,
            height: tile_h,
        } = geometry;
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

    pub(super) fn edge_water_rows(&self) -> impl Iterator<Item = i32> + use<> {
        let height = self.sim.map_wrap.height();
        let count = if self.sim.map_wrap.wraps_y() {
            0
        } else {
            map_view::EDGE_WATER_ROWS
        };
        (-count..0).chain(height..height + count)
    }

    pub(super) fn world_fog_knowledge(&self, x: i32, y: i32) -> fog::Knowledge {
        // Decorative water inherits knowledge from the closest real polar row.
        // City work maps keep their ordinary off-map fog behavior.
        let y = if self.sim.map_wrap.wraps_y() {
            y
        } else {
            y.clamp(0, self.sim.map_wrap.height() - 1)
        };
        self.fog_knowledge(x, y)
    }

    fn draw_edge_water(&self, assets: &Assets, viewport: Rect, tile_w: f32, tile_h: f32) {
        for y in self.edge_water_rows() {
            for x in 0..self.sim.map_wrap.width() {
                let center = self.world_screen_position(x, y);
                if center.x < viewport.x - tile_w
                    || center.x > viewport.right() + tile_w
                    || center.y < viewport.y - tile_h
                    || center.y > viewport.bottom() + tile_h
                {
                    continue;
                }
                if !self
                    .sim
                    .render_tile(x, y.clamp(0, self.sim.map_wrap.height() - 1))
                    .seen
                {
                    draw_diamond(center, tile_w, tile_h, BLACK);
                    continue;
                }
                if let Some(tileset) = &assets.civ3 {
                    tileset.draw_base(self, x, y, center, vec2(tile_w, tile_h));
                    continue;
                }
                assets.draw_masked_terrain_backing(Terrain::DeepOcean, center, tile_w, tile_h);
                let (row, col) = ocean_cell_source([0; 4]);
                for corner in 0..4 {
                    assets.draw_ocean_corner(
                        center,
                        tile_w,
                        tile_h,
                        isolated_sprite_source(corner * 9 + row as i32, col as i32, 96.0, 48.0),
                        WHITE,
                    );
                }
            }
        }
    }

    pub(in crate::client) fn draw_world_amplio2(&self, assets: &Assets) {
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

    pub(super) fn draw_visible_base_foreground(
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
    pub(super) fn draw_map_layer(
        &self,
        assets: &Assets,
        layer: map_layers::MapLayer,
        x: i32,
        y: i32,
        geometry: TileGeometry,
        has_city: bool,
    ) {
        use map_layers::MapLayer;
        let TileGeometry {
            center,
            width: tile_w,
            height: tile_h,
        } = geometry;
        let tile = self.sim.render_tile(x, y);
        match layer {
            MapLayer::Blend => {
                if assets.civ3.is_none() {
                    self.draw_terrain_blending(assets, x, y, center, tile_w, tile_h);
                }
            }
            MapLayer::Terrain => {
                if !assets.civ3.as_ref().is_some_and(|tileset| {
                    tileset.draw_overlay(tile.terrain, (x, y), center, geometry.size())
                }) {
                    self.draw_matched_terrain_overlay(assets, x, y, center, tile_w, tile_h);
                }
            }
            MapLayer::Water => {
                // Native fill_irrigation_sprite_array suppresses fields under cities.
                if !has_city {
                    if tile.farmland {
                        self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 2, 6);
                    } else if tile.irrigation
                        && !assets.civ3.as_ref().is_some_and(|tileset| {
                            tileset.draw_irrigation(tile.terrain, center, geometry.size())
                        })
                    {
                        self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 1, 6);
                    }
                }
                self.draw_river(assets, x, y, center, tile_w, tile_h);
            }
            MapLayer::Roads => {
                if tile.road {
                    self.draw_road_sprites(assets, x, y, geometry, has_city);
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
                if let Some(resource) = tile
                    .active_resource()
                    .filter(|&resource| self.sim.resource_visible_to(HUMAN, resource))
                    && !assets.civ3.as_ref().is_some_and(|tileset| {
                        tileset.draw_resource(resource, center, geometry.size())
                    })
                {
                    let (row, col) = resource.icon_cell();
                    self.draw_resource_icon(
                        &assets.resource_icons,
                        center,
                        tile_w,
                        tile_h,
                        row,
                        col,
                    );
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
}
