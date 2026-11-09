//! The supported Civ III map projection, picking, and painter backend.
//!
//! Simulation and HUD state stay shared; this backend routes map rendering and
//! input through Civ III geometry.
use super::civ3_geometry::Civ3Geometry;
use super::{Assets, Game};
use crate::sim::topology::{MAP_W, MapWrap};
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

/// Civ III diamond-grid projection, picking, and ordered map painter.
pub(crate) struct Civ3Renderer;

impl RenderBackend for Civ3Renderer {
    fn name(&self) -> &'static str {
        "civ3"
    }

    fn draw_world(&self, game: &Game, assets: &Assets) {
        game.draw_civ3_world_map(assets);
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
        let geometry = Civ3Geometry::new(wrap.width() * 2, wrap.height(), true).ok()?;
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
        "civ3" => Ok(std::rc::Rc::new(Civ3Renderer)),
        _ => Err(format!(
            "Unknown renderer '{name}'. Available renderer: civ3"
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
        assert_eq!(create("civ3").unwrap().name(), "civ3");
        assert!(create("amplio2").is_err());
        assert_eq!(default_renderer().name(), "civ3");
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
