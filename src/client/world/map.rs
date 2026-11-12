//! Tile visibility and map-layer drawing for the rendered world.

use super::*;

impl Game {
    fn draw_territory_border(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        owner: usize,
        center: Vec2,
        zoom: f32,
    ) {
        let hill_terrain = |terrain| matches!(terrain, Terrain::Hills | Terrain::Mountains);
        let current_hilly = hill_terrain(self.sim.render_tile_view(x, y).terrain);
        let civilization_id = self.sim.player_civilization_id_view(owner);
        let color = player_color(civilization_id.as_deref(), owner);
        // Source rows are NW, NE, SW, SE; tile edges are traversed clockwise
        // from the top-right edge, matching Civ III's staggered neighbors.
        for (dx, dy, row) in [(0, -1, 1), (1, 0, 3), (0, 1, 2), (-1, 0, 0)] {
            let neighbor = render_neighbor(self.sim.map_wrap(), x, y, dx, dy);
            if let Some((nx, ny)) = neighbor
                && self.playback.tile_owner(&self.sim, nx, ny) == Some(owner)
            {
                continue;
            }
            let hilly = current_hilly
                && neighbor.is_some_and(|(nx, ny)| {
                    hill_terrain(self.sim.render_tile_view(nx, ny).terrain)
                });
            assets
                .civ3
                .draw_territory_border(center, row, hilly, color, zoom);
        }
    }

    pub(super) fn edge_water_rows(&self) -> impl Iterator<Item = i32> + use<> {
        let height = self.sim.map_wrap().height();
        let count = map_view::EDGE_WATER_ROWS;
        (-count..0).chain(height..height + count)
    }

    pub(super) fn world_fog_knowledge(&self, x: i32, y: i32) -> fog::Knowledge {
        // Decorative water inherits knowledge from the closest real polar row.
        // City work maps keep their ordinary off-map fog behavior.
        let y = y.clamp(0, self.sim.map_wrap().height() - 1);
        self.fog_knowledge(x, y)
    }

    pub(super) fn draw_edge_water(
        &self,
        assets: &Assets,
        viewport: Rect,
        tile_w: f32,
        tile_h: f32,
    ) {
        let width = self.sim.map_wrap().width();
        for y in self.edge_water_rows() {
            for x in 0..width {
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
                    .render_tile_view(x, y.clamp(0, self.sim.map_wrap().height() - 1))
                    .seen
                {
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
        let tile = self.sim.render_tile_view(x, y);
        if tile.visible && matches!(tile.base, Some(Base::Fort | Base::Fortress)) {
            assets.civ3.draw_terrain_building(
                super::super::tileset::TerrainBuilding::Fortress,
                center,
                vec2(w, h),
            );
        }
    }

    pub(super) fn draw_grid_layer(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        geometry: TileGeometry,
        zoom: f32,
    ) {
        if let Some(owner) = self.playback.tile_owner(&self.sim, x, y) {
            self.draw_territory_border(assets, x, y, owner, geometry.center, zoom);
        }
    }
}
