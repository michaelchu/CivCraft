//! Swappable map-rendering backends.
//!
//! The simulation and HUD stay shared. Each backend owns map projection and
//! picking; map passes may remain shared while the Civ III painter is migrated.
use super::civ3_geometry::Civ3Geometry;
use super::{Assets, Game};
use crate::sim::topology::{MAP_W, MapWrap, map_to_native};
use macroquad::prelude::*;

pub(crate) trait RenderBackend {
    fn name(&self) -> &'static str;

    fn draw_world(&self, game: &Game, assets: &Assets);

    fn screen_to_world(
        &self,
        point: Vec2,
        viewport: Rect,
        camera: Vec2,
        zoom: f32,
        wrap: MapWrap,
    ) -> Option<(i32, i32)>;

    fn world_screen_position(
        &self,
        wrap: MapWrap,
        viewport: Rect,
        camera: Vec2,
        zoom: f32,
        x: i32,
        y: i32,
    ) -> Vec2;
}

pub(crate) struct Amplio2Renderer;

impl RenderBackend for Amplio2Renderer {
    fn name(&self) -> &'static str {
        "amplio2"
    }

    fn draw_world(&self, game: &Game, assets: &Assets) {
        game.draw_world_map(assets);
    }

    fn screen_to_world(
        &self,
        point: Vec2,
        viewport: Rect,
        camera: Vec2,
        zoom: f32,
        wrap: MapWrap,
    ) -> Option<(i32, i32)> {
        let sx = point.x - (viewport.x + viewport.w * 0.5);
        let sy = point.y - (viewport.y + viewport.h * 0.5);
        let a = sx / (super::TILE_W * 0.5 * zoom);
        let b = sy / (super::TILE_H * 0.5 * zoom);
        let dx = (a + b) * 0.5;
        let dy = (b - a) * 0.5;
        let (x, y) = map_to_native(
            (camera.x + dx).round() as i32,
            (camera.y + dy).round() as i32,
        );
        wrap.normalize(x, y)
    }

    fn world_screen_position(
        &self,
        wrap: MapWrap,
        viewport: Rect,
        camera: Vec2,
        zoom: f32,
        x: i32,
        y: i32,
    ) -> Vec2 {
        super::world::amplio2_world_screen_position(wrap, viewport, camera, zoom, x, y)
    }
}

/// Civ III diamond-grid projection and picking. Terrain passes remain shared
/// until the dedicated painter is split from the legacy layer implementation.
pub(crate) struct Civ3Renderer;

impl RenderBackend for Civ3Renderer {
    fn name(&self) -> &'static str {
        "civ3"
    }

    fn draw_world(&self, game: &Game, assets: &Assets) {
        game.draw_world_map(assets);
    }

    fn screen_to_world(
        &self,
        point: Vec2,
        viewport: Rect,
        camera: Vec2,
        zoom: f32,
        wrap: MapWrap,
    ) -> Option<(i32, i32)> {
        let grid_camera = Vec2::new(
            (camera.x - camera.y + MAP_W as f32).rem_euclid(2.0 * wrap.width() as f32),
            camera.x + camera.y - MAP_W as f32,
        );
        let geometry =
            Civ3Geometry::new_wrapped(wrap.width() * 2, wrap.height(), true, wrap.wraps_y())
                .ok()?;
        let (x, y) = geometry.screen_to_storage(
            point.x - (viewport.x + viewport.w * 0.5),
            point.y - (viewport.y + viewport.h * 0.5),
            grid_camera.x,
            grid_camera.y,
            zoom,
            0.75,
        )?;
        wrap.normalize(x, y)
    }

    fn world_screen_position(
        &self,
        wrap: MapWrap,
        viewport: Rect,
        camera: Vec2,
        zoom: f32,
        x: i32,
        y: i32,
    ) -> Vec2 {
        super::world::civ3_world_screen_position(wrap, viewport, camera, zoom, x, y)
    }
}

pub(crate) fn create(name: &str) -> Result<std::rc::Rc<dyn RenderBackend>, String> {
    match name.to_ascii_lowercase().as_str() {
        "amplio2" => Ok(std::rc::Rc::new(Amplio2Renderer)),
        "civ3" => Ok(std::rc::Rc::new(Civ3Renderer)),
        _ => Err(format!(
            "Unknown renderer '{name}'. Available renderers: civ3, amplio2"
        )),
    }
}

pub(crate) fn default_renderer() -> std::rc::Rc<dyn RenderBackend> {
    std::rc::Rc::new(Civ3Renderer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::topology::MAP_W;

    #[test]
    fn startup_selection_registers_the_existing_renderer() {
        assert_eq!(create("amplio2").unwrap().name(), "amplio2");
        assert_eq!(create("AMPLIO2").unwrap().name(), "amplio2");
        assert_eq!(create("civ3").unwrap().name(), "civ3");
        assert_eq!(default_renderer().name(), "civ3");
    }

    #[test]
    fn amplio2_projection_round_trips_tile_centers() {
        let renderer = Amplio2Renderer;
        let viewport = Rect::new(0.0, 46.0, 900.0, 600.0);
        let camera = vec2(31.0, 24.0);
        let view_camera = vec2(
            (camera.x - camera.y + MAP_W as f32).rem_euclid(2.0 * MapWrap::X.width() as f32),
            camera.x + camera.y - MAP_W as f32,
        );
        for (x, y) in [
            (0, 0),
            (MapWrap::X.width() / 2, MapWrap::X.height() / 2),
            (MapWrap::X.width() - 1, MapWrap::X.height() - 1),
        ] {
            let point =
                renderer.world_screen_position(MapWrap::X, viewport, view_camera, 1.0, x, y);
            assert_eq!(
                renderer.screen_to_world(point, viewport, camera, 1.0, MapWrap::X),
                Some((x, y))
            );
        }
    }

    #[test]
    fn civ3_diamond_projection_and_picking_round_trip_tile_centers() {
        let renderer = Civ3Renderer;
        let viewport = Rect::new(0.0, 46.0, 900.0, 600.0);
        let camera = vec2(31.0, 24.0);
        let view_camera = vec2(
            (camera.x - camera.y + MAP_W as f32).rem_euclid(2.0 * MapWrap::X.width() as f32),
            camera.x + camera.y - MAP_W as f32,
        );
        for (x, y) in [
            (0, 0),
            (MapWrap::X.width() / 2, MapWrap::X.height() / 2),
            (MapWrap::X.width() - 1, MapWrap::X.height() - 1),
        ] {
            let point =
                renderer.world_screen_position(MapWrap::X, viewport, view_camera, 1.0, x, y);
            assert_eq!(
                renderer.screen_to_world(point, viewport, camera, 1.0, MapWrap::X),
                Some((x, y))
            );
        }
    }
}
