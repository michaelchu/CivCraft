//! Civ III screen/world projection and tile picking.

use super::civ3_geometry::Civ3Geometry;
use crate::world::topology::{MAP_W, MapWrap};
use macroquad::prelude::*;

pub(super) fn screen_to_world(
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

pub(super) fn world_screen_position(
    wrap: MapWrap,
    viewport: Rect,
    camera: Vec2,
    zoom: f32,
    x: i32,
    y: i32,
) -> Vec2 {
    super::world::civ3_world_screen_position(wrap, viewport, camera, zoom, x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civ3_diamond_projection_and_picking_round_trip_tile_centers() {
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
            let point = world_screen_position(MapWrap::X, viewport, view_camera, 1.0, x, y);
            assert_eq!(
                screen_to_world(point, viewport, camera, 1.0, MapWrap::X),
                Some((x, y))
            );
        }
    }
}
