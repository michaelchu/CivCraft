//! Tile visibility and map-layer drawing for the rendered world.

use super::*;

fn should_draw_territory_edge(owner: usize, neighbor: Option<Option<usize>>) -> bool {
    neighbor != Some(Some(owner))
}

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
            let neighbor_owner =
                neighbor.map(|(nx, ny)| self.sim.territory_geometry_owner_view(nx, ny));
            // Continue real contours beneath the fog fringe; the fog atlas
            // clips them at the same boundary as terrain.
            if !should_draw_territory_edge(owner, neighbor_owner) {
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

    pub(super) fn draw_edge_water(&self, assets: &Assets, view: super::civ3::WorldMapView) {
        let viewport = view.viewport;
        let tile_w = TILE_W * view.zoom;
        let tile_h = TILE_H * view.zoom;
        let width = self.sim.map_wrap().width();
        for y in self.edge_water_rows() {
            for x in 0..width {
                let center = view.project(self.sim.map_wrap(), x, y);
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

    pub(super) fn draw_grid_layer(&self, assets: &Assets, x: i32, y: i32, geometry: TileGeometry) {
        let owner = if self.sim.render_tile_view(x, y).seen {
            self.sim.territory_border_owner_view(x, y)
        } else {
            self.sim.territory_geometry_owner_view(x, y)
        };
        if let Some(owner) = owner {
            self.draw_territory_border(
                assets,
                x,
                y,
                owner,
                geometry.center,
                geometry.width / TILE_W,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::should_draw_territory_edge;

    #[test]
    fn territory_edges_follow_ownership_beneath_the_fog_mask() {
        assert!(should_draw_territory_edge(1, Some(Some(2))));
        assert!(!should_draw_territory_edge(1, Some(Some(1))));
        assert!(should_draw_territory_edge(1, Some(None)));
        assert!(should_draw_territory_edge(1, None));
    }
}
