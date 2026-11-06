use super::*;

impl Game {
    fn ocean_match_type(&self, x: i32, y: i32) -> u8 {
        match self.sim.tile(x, y).terrain {
            Terrain::DeepOcean => 0,
            Terrain::Ocean | Terrain::Lake => 1,
            _ => 2,
        }
    }

    fn ocean_neighbor_match(&self, x: i32, y: i32, dx: i32, dy: i32, current: u8) -> u8 {
        let (neighbor_x, neighbor_y) = offset_native_by_map(x, y, dx, dy);
        if self.sim.tile(neighbor_x, neighbor_y).seen {
            self.ocean_match_type(neighbor_x, neighbor_y)
        } else {
            // Do not reveal a coastline through unexplored terrain.
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
                    source: Some(Rect::new(
                        (source_col * TILE_SPRITE_W) as f32,
                        (corner * OCEAN_ATLAS_H + source_row * TILE_SPRITE_H) as f32,
                        TILE_SPRITE_W as f32,
                        TILE_SPRITE_H as f32,
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
        visible: bool,
    ) {
        let terrain = self.sim.tile(x, y).terrain;
        let tint = if visible {
            WHITE
        } else {
            Color::new(0.54, 0.59, 0.62, 1.0)
        };

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
                source: Some(bordered_sprite_source(row, 0, 96.0, 48.0)),
                ..Default::default()
            },
        );
    }

    fn draw_terrain_overlay(
        &self,
        assets: &Assets,
        terrain: Terrain,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
        visible: bool,
    ) {
        let tint = if visible {
            WHITE
        } else {
            Color::new(0.54, 0.59, 0.62, 1.0)
        };
        match terrain {
            Terrain::Forest | Terrain::Jungle => {
                draw_texture_ex(
                    &assets.terrain2,
                    center.x - tile_w * 0.5,
                    center.y - tile_h * 0.5,
                    tint,
                    DrawTextureParams {
                        dest_size: Some(vec2(tile_w, tile_h)),
                        source: Some(bordered_sprite_source(
                            if terrain == Terrain::Jungle { 2 } else { 4 },
                            0,
                            96.0,
                            48.0,
                        )),
                        ..Default::default()
                    },
                );
            }
            Terrain::Hills => {
                draw_texture_ex(
                    &assets.hills,
                    center.x - tile_w * 0.5,
                    center.y - tile_h * 0.5,
                    tint,
                    DrawTextureParams {
                        dest_size: Some(vec2(tile_w, tile_h)),
                        source: Some(bordered_sprite_source(0, 0, 96.0, 48.0)),
                        ..Default::default()
                    },
                );
            }
            Terrain::Mountains => {
                let zoom = tile_w / TILE_W;
                let mountain_h = 66.0 * zoom;
                draw_texture_ex(
                    &assets.mountains,
                    center.x - tile_w * 0.5,
                    center.y + tile_h * 0.5 - mountain_h + 6.0 * zoom,
                    tint,
                    DrawTextureParams {
                        dest_size: Some(vec2(tile_w, mountain_h)),
                        source: Some(bordered_sprite_source(0, 0, 96.0, 66.0)),
                        ..Default::default()
                    },
                );
            }
            _ => {}
        }
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
                source: Some(bordered_sprite_source(source_row, source_col, 96.0, 48.0)),
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
        let tile = self.sim.tile(x, y);
        let water = tile.terrain.is_water();
        if !water && !tile.river {
            return;
        }

        let mut mask = 0;
        for (direction, (dx, dy)) in [(0, -1), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
            let (nx, ny) = offset_native_by_map(x, y, dx, dy);
            let neighbor = self.sim.tile(nx, ny);
            if neighbor.seen && (neighbor.river || (!water && neighbor.terrain.is_water())) {
                if water {
                    self.draw_terrain_extra(
                        &assets.water,
                        center,
                        tile_w,
                        tile_h,
                        4,
                        direction as i32,
                    );
                } else {
                    mask |= 8 >> direction;
                }
            }
        }
        if !water {
            self.draw_terrain_extra(
                &assets.water,
                center,
                tile_w,
                tile_h,
                2 + mask / 8,
                mask % 8,
            );
        }

        // Amplio2 has four river exits per diamond. The two additional
        // isometric-hex neighbors meet this projection at its vertical tips.
        for step in [-1, 1] {
            let (nx, ny) = offset_native_by_map(x, y, step, step);
            let neighbor = self.sim.tile(nx, ny);
            if neighbor.seen && (neighbor.river || (!water && neighbor.terrain.is_water())) {
                let tip = center.y + step as f32 * tile_h * 0.5;
                let end = if water {
                    center.y + step as f32 * tile_h * 0.23
                } else {
                    center.y
                };
                draw_line(
                    center.x,
                    tip,
                    center.x,
                    end,
                    tile_w * 0.048,
                    Color::from_rgba(23, 70, 136, 255),
                );
                draw_line(
                    center.x,
                    tip,
                    center.x,
                    end,
                    tile_w * 0.024,
                    Color::from_rgba(56, 126, 190, 255),
                );
            }
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
    ) {
        let mut connected = false;
        let railroad = self.sim.tile(x, y).railroad;
        for ((dx, dy), source_col) in [
            ((0, -1), 1),
            ((1, 0), 3),
            ((1, 1), 4),
            ((0, 1), 5),
            ((-1, 0), 7),
            ((-1, -1), 8),
        ] {
            let (neighbor_x, neighbor_y) = offset_native_by_map(x, y, dx, dy);
            let neighbor = self.sim.tile(neighbor_x, neighbor_y);
            if neighbor.seen && neighbor.road {
                let row = if railroad && neighbor.railroad {
                    13
                } else {
                    12
                };
                self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, row, source_col);
                connected = true;
            }
        }
        if !connected {
            let row = if railroad { 13 } else { 12 };
            self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, row, 0);
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
        visible: bool,
    ) {
        let points = [
            vec2(center.x, center.y - tile_h * 0.5),
            vec2(center.x + tile_w * 0.5, center.y),
            vec2(center.x, center.y + tile_h * 0.5),
            vec2(center.x - tile_w * 0.5, center.y),
        ];
        let edge_neighbors = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let opacity = if visible { 0.95 } else { 0.42 };
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
            let (neighbor_x, neighbor_y) = offset_native_by_map(x, y, dx, dy);
            if self.sim.displayed_tile_owner(neighbor_x, neighbor_y) == Some(owner) {
                continue;
            }
            geometry.draw(&points, edge, width, color, shadow, thread);
        }
    }

    pub(super) fn draw_world(&self, assets: &Assets) {
        let viewport = Self::viewport_rect();
        draw_rectangle(
            viewport.x,
            viewport.y,
            viewport.w,
            viewport.h,
            Color::from_rgba(8, 28, 48, 255),
        );

        let tile_w = TILE_W * self.zoom;
        let tile_h = TILE_H * self.zoom;
        // Native rows are already in painter order. Cull before collecting and
        // project each visible tile once for both terrain passes.
        let order = visible_world_tiles(viewport, self.camera_overview_position(), self.zoom);
        let mut cities = std::collections::HashMap::new();
        for city in self.sim.cities() {
            cities.entry((city.x, city.y)).or_insert(city);
        }
        let mut stacks: std::collections::HashMap<(i32, i32), (&Unit, usize)> =
            std::collections::HashMap::new();
        for unit in self
            .sim
            .units()
            .iter()
            .filter(|u| self.sim.unit_visible_to_human(u))
        {
            let entry = stacks.entry((unit.x, unit.y)).or_insert((unit, 0));
            entry.1 += 1;
            let priority = |u: &Unit| (u.transport.is_some(), Some(u.id) != self.selected_unit);
            if priority(unit) < priority(entry.0) {
                entry.0 = unit;
            }
        }
        // Draw every tile base first. Tall terrain sprites are a separate
        // layer so later diamond bases cannot cut horizontal bands through
        // forests, hills, and mountains.
        for &(x, y, center) in &order {
            let tile = self.sim.tile(x, y);
            if !tile.seen {
                draw_diamond(
                    center,
                    tile_w - 1.0,
                    tile_h - 1.0,
                    Color::from_rgba(5, 15, 24, 255),
                );
                continue;
            }

            draw_diamond(center, tile_w, tile_h, tile.terrain.minimap_color());
            self.draw_terrain_sprite(assets, x, y, center, tile_w, tile_h, tile.visible);
        }

        let reveal_cities = self.sim.reveals_cities(HUMAN);
        for (x, y, center) in order {
            let tile = self.sim.tile(x, y);
            if !tile.seen {
                continue;
            }

            self.draw_tile_details(assets, x, y, center, tile_w, tile_h);
            if (tile.visible || reveal_cities)
                && let Some(city) = cities.get(&(x, y)).copied()
            {
                self.draw_city(assets, city, center);
            }
            if !tile.visible {
                continue;
            }
            if let Some(&(unit, count)) = stacks.get(&(x, y)) {
                self.draw_unit(assets, unit, center, count);
            }
            if let Some(Base::Fort | Base::Fortress) = tile.base {
                let row = i32::from(tile.base == Some(Base::Fort));
                self.draw_base_sprite(assets, center, tile_w, tile_h, row, 3);
            }
        }
    }

    /// Shared terrain/improvement pass for both the world and city map views.
    fn draw_tile_details(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
    ) {
        let tile = self.sim.tile(x, y);
        self.draw_terrain_overlay(assets, tile.terrain, center, tile_w, tile_h, tile.visible);

        if let Some(owner) = self.sim.displayed_tile_owner(x, y) {
            self.draw_territory_border(x, y, owner, center, tile_w, tile_h, tile.visible);
        }
        if tile.road {
            self.draw_road_sprites(assets, x, y, center, tile_w, tile_h);
        }
        if tile.farmland {
            self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 2, 6);
        } else if tile.irrigation {
            self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 1, 6);
        }
        if tile.oil_platform {
            self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 7, 6);
        } else if tile.oil_well {
            self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 0, 6);
        } else if tile.mine {
            self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 3, 6);
        }
        self.draw_river(assets, x, y, center, tile_w, tile_h);
        if tile.hut {
            self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 5, 6);
        }
        if let Some(resource) = tile.active_resource() {
            let (row, col) = resource.sprite(tile.terrain);
            self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, row, col);
        }
        if tile.pollution {
            self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 4, 6);
        }
        if tile.fallout {
            self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 6, 6);
        }
        if let Some(base) = tile.base {
            let (row, col) = match base {
                Base::Fort => (1, 4),
                Base::Fortress => (0, 4),
                Base::Airstrip => (1, 0),
                Base::Airbase => (0, 0),
            };
            self.draw_base_sprite(assets, center, tile_w, tile_h, row, col);
            if !tile.visible && col == 4 {
                self.draw_base_sprite(assets, center, tile_w, tile_h, row, 3);
            }
        }
        if !tile.visible {
            draw_diamond(center, tile_w, tile_h, Color::from_rgba(18, 29, 39, 112));
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
            Color::from_rgba(8, 28, 48, 255),
        );
        let radius: std::collections::HashSet<_> = Simulation::city_radius(city)
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
                let x = x.rem_euclid(MAP_W);
                let relevant = (0..MAP_H).contains(&y) && radius.contains(&(dx, dy));
                tiles.push((dx, dy, x, y, center, relevant));
            }
        }
        tiles.sort_by(|a, b| a.4.y.total_cmp(&b.4.y));
        for &(_, _, x, y, center, relevant) in &tiles {
            if !relevant || !self.sim.tile(x, y).seen {
                draw_diamond(
                    center,
                    layout.tile_w - 1.0,
                    layout.tile_h - 1.0,
                    Color::from_rgba(5, 15, 24, 255),
                );
            } else {
                let tile = self.sim.tile(x, y);
                draw_diamond(
                    center,
                    layout.tile_w,
                    layout.tile_h,
                    tile.terrain.minimap_color(),
                );
                self.draw_terrain_sprite(
                    assets,
                    x,
                    y,
                    center,
                    layout.tile_w,
                    layout.tile_h,
                    tile.visible,
                );
            }
        }
        for &(_, _, x, y, center, relevant) in &tiles {
            if relevant && self.sim.tile(x, y).seen {
                self.draw_tile_details(assets, x, y, center, layout.tile_w, layout.tile_h);
                if (x, y) == (city.x, city.y) {
                    self.draw_city_at_scale(assets, city, center, layout.tile_w / TILE_W, false);
                }
            }
        }
        unsafe {
            get_internal_gl().quad_gl.scissor(None);
        }
    }

    fn draw_city(&self, assets: &Assets, city: &City, center: Vec2) {
        self.draw_city_at_scale(assets, city, center, self.zoom, true);
    }

    fn draw_city_at_scale(
        &self,
        assets: &Assets,
        city: &City,
        center: Vec2,
        zoom: f32,
        label: bool,
    ) {
        let style = [
            (Tech::SUPERCONDUCTORS, 9),
            (Tech::ROCKETRY, 8),
            (Tech::AUTOMOBILE, 7),
            (Tech::RAILROAD, 6),
        ]
        .into_iter()
        .find(|&(tech, _)| self.sim.has_tech(city.owner, tech));
        let (row, col) = if let Some((_, row)) = style {
            let size_band = [5, 9, 13, 17]
                .into_iter()
                .filter(|&size| city.size >= size)
                .count();
            (row, size_band)
        } else {
            (2, (city.id as usize + city.owner * 2) % 11)
        };
        let city_w = 69.0 * zoom;
        let city_h = 52.0 * zoom;
        draw_texture_ex(
            &assets.cities,
            center.x - city_w * 0.5,
            center.y - city_h + 9.0 * zoom,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(city_w, city_h)),
                source: Some(Rect::new(
                    1.0 + col as f32 * 97.0,
                    1.0 + row as f32 * 73.0,
                    96.0,
                    72.0,
                )),
                ..Default::default()
            },
        );
        if !label {
            return;
        }
        let label_y = center.y + 10.0 * zoom;
        let label = format!("{}  {}", city.name, city.size);
        let width = measure_text(&label, None, (14.0 * zoom).max(10.0) as u16, 1.0).width;
        draw_rectangle(
            center.x - width * 0.5 - 6.0,
            label_y - 13.0,
            width + 12.0,
            17.0,
            Color::from_rgba(19, 20, 18, 220),
        );
        draw_rectangle(
            center.x - width * 0.5 - 6.0,
            label_y - 13.0,
            3.0,
            17.0,
            player_color(city.owner),
        );
        draw_text(
            &label,
            center.x - width * 0.5,
            label_y,
            (14.0 * zoom).max(10.0),
            Color::from_rgba(244, 235, 201, 255),
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

    fn draw_unit(&self, assets: &Assets, unit: &Unit, center: Vec2, stack_count: usize) {
        let (sprite_row, sprite_col) = unit_sprite(unit.kind);
        let unit_w = 58.0 * self.zoom;
        let unit_h = 44.0 * self.zoom;
        let is_selected = self.selected_unit == Some(unit.id);
        if is_selected {
            draw_diamond(
                center + vec2(0.0, 4.0 * self.zoom),
                55.0 * self.zoom,
                23.0 * self.zoom,
                Color::from_rgba(255, 235, 112, 185),
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
                source: Some(Rect::new(
                    1.0 + sprite_col as f32 * 65.0,
                    1.0 + sprite_row as f32 * 49.0,
                    64.0,
                    48.0,
                )),
                ..Default::default()
            },
        );
        draw_circle(
            center.x - 21.0 * self.zoom,
            center.y - 25.0 * self.zoom,
            (5.0 * self.zoom).max(3.2),
            player_color(unit.owner),
        );
        if stack_count > 1 {
            draw_circle(
                center.x + 19.0 * self.zoom,
                center.y - 21.0 * self.zoom,
                (8.0 * self.zoom).max(6.0),
                Color::from_rgba(25, 24, 21, 235),
            );
            draw_text(
                &stack_count.to_string(),
                center.x + 15.0 * self.zoom,
                center.y - 17.0 * self.zoom,
                (13.0 * self.zoom).max(10.0),
                WHITE,
            );
        }
    }
}

/// Preserve the original projection and float operation
/// order so rendering optimizations do not shift existing artwork by a pixel.
pub(super) fn world_screen_position(
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
    vec2(
        viewport.x + viewport.w * 0.5 + dx * TILE_W * 0.5 * zoom,
        viewport.y + viewport.h * 0.47 + dy * TILE_H * 0.5 * zoom,
    )
}
fn visible_world_tiles(viewport: Rect, camera: Vec2, zoom: f32) -> Vec<(i32, i32, Vec2)> {
    let origin = viewport.y + viewport.h * 0.47;
    let step = TILE_H * 0.5 * zoom;
    let first = (camera.y + (viewport.y - 80.0 - origin) / step)
        .floor()
        .max(0.0) as i32;
    let last = (camera.y + (viewport.bottom() + 80.0 - origin) / step)
        .ceil()
        .min((MAP_H - 1) as f32) as i32;
    let mut tiles = Vec::with_capacity(((last - first + 1).max(0) * MAP_W) as usize);
    for y in first..=last {
        for x in 0..MAP_W {
            let center = world_screen_position(viewport, camera, zoom, x, y);
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
                        .map(|(x, y)| (x, y, world_screen_position(viewport, camera, zoom, x, y)))
                        .collect();
                    reference.sort_by(|a, b| a.2.y.total_cmp(&b.2.y));
                    reference.retain(|(_, _, p)| {
                        !(p.x < viewport.x - TILE_W * zoom
                            || p.x > viewport.right() + TILE_W * zoom
                            || p.y < viewport.y - 80.0
                            || p.y > viewport.bottom() + 80.0)
                    });
                    assert_eq!(visible_world_tiles(viewport, camera, zoom), reference);
                }
            }
        }
    }
}
