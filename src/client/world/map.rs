//! Tile visibility and map-layer drawing for the rendered world.

use super::*;

impl Game {
    pub(super) fn draw_resource_icon(
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
        let count = map_view::EDGE_WATER_ROWS;
        (-count..0).chain(height..height + count)
    }

    pub(super) fn world_fog_knowledge(&self, x: i32, y: i32) -> fog::Knowledge {
        // Decorative water inherits knowledge from the closest real polar row.
        // City work maps keep their ordinary off-map fog behavior.
        let y = y.clamp(0, self.sim.map_wrap.height() - 1);
        self.fog_knowledge(x, y)
    }

    pub(super) fn draw_edge_water(
        &self,
        assets: &Assets,
        viewport: Rect,
        tile_w: f32,
        tile_h: f32,
    ) {
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
                assets
                    .civ3
                    .draw_base(self, x, y, center, vec2(tile_w, tile_h));
            }
        }
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
        if tile.visible && matches!(tile.base, Some(Base::Fort | Base::Fortress)) {
            assets.civ3.draw_terrain_building(
                super::super::tileset::TerrainBuilding::Fortress,
                center,
                vec2(w, h),
            );
        }
    }

    pub(super) fn draw_grid_layer(&self, x: i32, y: i32, geometry: TileGeometry) {
        if let Some(owner) = self.playback.tile_owner(&self.sim, x, y) {
            self.draw_territory_border(
                x,
                y,
                owner,
                geometry.center,
                geometry.width,
                geometry.height,
            );
        }
    }
}
