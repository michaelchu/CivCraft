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
                let mountain_h = 66.0 * self.zoom;
                draw_texture_ex(
                    &assets.mountains,
                    center.x - tile_w * 0.5,
                    center.y + tile_h * 0.5 - mountain_h + 6.0 * self.zoom,
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
                self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 12, source_col);
                connected = true;
            }
        }
        if !connected {
            self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 12, 0);
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
        let color = with_alpha(PLAYER_COLORS[owner], if visible { 0.82 } else { 0.42 });

        for (edge, (dx, dy)) in edge_neighbors.into_iter().enumerate() {
            let (neighbor_x, neighbor_y) = offset_native_by_map(x, y, dx, dy);
            if self.sim.tile(neighbor_x, neighbor_y).owner == Some(owner) {
                continue;
            }
            let start = points[edge];
            let end = points[(edge + 1) % points.len()];
            draw_line(start.x, start.y, end.x, end.y, 2.0, color);
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

        let mut order: Vec<(i32, i32, f32)> = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .map(|(x, y)| {
                let p = self.world_to_screen(x, y);
                (x, y, p.y)
            })
            .collect();
        order.sort_by(|a, b| a.2.total_cmp(&b.2));

        let tile_w = TILE_W * self.zoom;
        let tile_h = TILE_H * self.zoom;
        // Draw every tile base first. Tall terrain sprites are a separate
        // layer so later diamond bases cannot cut horizontal bands through
        // forests, hills, and mountains.
        for &(x, y, _) in &order {
            let center = self.world_to_screen(x, y);
            if center.x < viewport.x - tile_w
                || center.x > viewport.x + viewport.w + tile_w
                || center.y < viewport.y - 80.0
                || center.y > viewport.y + viewport.h + 80.0
            {
                continue;
            }
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

        for (x, y, _) in order {
            let center = self.world_to_screen(x, y);
            if center.x < viewport.x - tile_w
                || center.x > viewport.x + viewport.w + tile_w
                || center.y < viewport.y - 80.0
                || center.y > viewport.y + viewport.h + 80.0
            {
                continue;
            }
            let tile = self.sim.tile(x, y);
            if !tile.seen {
                continue;
            }

            self.draw_terrain_overlay(assets, tile.terrain, center, tile_w, tile_h, tile.visible);

            if let Some(owner) = tile.owner {
                self.draw_territory_border(x, y, owner, center, tile_w, tile_h, tile.visible);
            }
            if tile.road {
                self.draw_road_sprites(assets, x, y, center, tile_w, tile_h);
            }
            if tile.irrigation {
                self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 1, 6);
            }
            if tile.mine {
                self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 3, 6);
            }
            self.draw_river(assets, x, y, center, tile_w, tile_h);
            if tile.hut {
                self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, 5, 6);
            }
            if let Some(resource) = tile.resource {
                let (row, col) = resource.sprite(tile.terrain);
                self.draw_terrain_extra(&assets.terrain, center, tile_w, tile_h, row, col);
            }
            if !tile.visible {
                draw_diamond(center, tile_w, tile_h, Color::from_rgba(18, 29, 39, 112));
                continue;
            }

            if let Some(city) = self.sim.cities().iter().find(|c| c.x == x && c.y == y) {
                self.draw_city(assets, city, center);
            }
            let mut stack: Vec<&Unit> = self
                .sim
                .units()
                .iter()
                .filter(|u| u.x == x && u.y == y)
                .collect();
            stack.sort_by_key(|u| (u.transport.is_some(), Some(u.id) != self.selected_unit));
            if let Some(unit) = stack.first() {
                self.draw_unit(assets, unit, center, stack.len());
            }
        }
    }

    fn draw_city(&self, assets: &Assets, city: &City, center: Vec2) {
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
        let city_w = 69.0 * self.zoom;
        let city_h = 52.0 * self.zoom;
        draw_texture_ex(
            &assets.cities,
            center.x - city_w * 0.5,
            center.y - city_h + 9.0 * self.zoom,
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
        let label_y = center.y + 10.0 * self.zoom;
        let label = format!("{}  {}", city.name, city.size);
        let width = measure_text(&label, None, (14.0 * self.zoom).max(10.0) as u16, 1.0).width;
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
            PLAYER_COLORS[city.owner],
        );
        draw_text(
            &label,
            center.x - width * 0.5,
            label_y,
            (14.0 * self.zoom).max(10.0),
            Color::from_rgba(244, 235, 201, 255),
        );
        if self.selected_city == Some(city.id) {
            draw_diamond_lines(
                center,
                TILE_W * self.zoom * 0.88,
                TILE_H * self.zoom * 0.82,
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
            PLAYER_COLORS[unit.owner],
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
