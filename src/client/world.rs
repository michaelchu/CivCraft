//! World-map rendering and projection from simulation tiles to screen geometry.

use super::civ3_geometry::{CELL_H as CIV3_CELL_H, CELL_W as CIV3_CELL_W, Civ3Geometry};
use super::*;
mod entities;
mod map;

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
    fn decorative_edge_water_inherits_fog_without_extending_playable_geometry() {
        for knowledge in [
            fog::Knowledge::Unknown,
            fog::Knowledge::Remembered,
            fog::Knowledge::Visible,
        ] {
            let mut map = crate::mapgen::generate_with_wrap(42, MapWrap::X);
            if knowledge == fog::Knowledge::Remembered {
                for tile in &mut map.tiles {
                    tile.seen = true;
                }
            }
            let mut sim = Simulation::new(map);
            if knowledge == fog::Knowledge::Visible {
                sim.ui_review_reveal_poles();
            }
            let game = Game::from_simulation(sim);
            assert_eq!(game.edge_water_rows().collect::<Vec<_>>(), [-2, -1, 48, 49]);
            for padding in [-2, 49] {
                assert_eq!(game.world_fog_knowledge(2, padding), knowledge);
                assert!(game.sim.map_wrap.normalize(2, padding).is_none());
            }
        }
        let torus = Game::from_simulation(Simulation::new(crate::mapgen::generate_with_wrap(
            42,
            MapWrap::XY,
        )));
        assert_eq!(torus.edge_water_rows().count(), 0);
    }

    #[test]
    fn map_connections_stop_at_poles_and_still_wrap_horizontally() {
        assert_eq!(render_neighbor(MapWrap::X, 0, 0, 0, -1), None);
        assert_eq!(
            render_neighbor(MapWrap::X, 0, MapWrap::X.height() - 1, 1, 0),
            None
        );
        assert_eq!(
            render_neighbor(MapWrap::X, 0, 0, -1, 1),
            Some((MapWrap::X.width() - 1, 0))
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

/// Civ III staggered-diamond tile centers relative to the viewport center.
/// The existing rectangular tile storage is adapted to occupied staggered
/// coordinates here; simulation adjacency is migrated separately.
pub(super) fn civ3_world_screen_position(
    wrap: MapWrap,
    viewport: Rect,
    camera: Vec2,
    zoom: f32,
    x: i32,
    y: i32,
) -> Vec2 {
    let geometry = Civ3Geometry::new_wrapped(wrap.width() * 2, wrap.height(), true, wrap.wraps_y())
        .expect("valid Civ III world geometry");
    let (grid_x, grid_y) = geometry
        .storage_to_grid(x, y)
        .unwrap_or((2 * x + y.rem_euclid(2), y));
    let (tile_px, tile_py) = geometry.tile_center_virtual(grid_x, grid_y);
    let camera_px = (camera.x + 1.0) * CIV3_CELL_W;
    let camera_py = (camera.y + 1.0) * CIV3_CELL_H;
    let width_px = wrap.width() as f32 * 2.0 * CIV3_CELL_W;
    let dx = (tile_px - camera_px + width_px * 0.5).rem_euclid(width_px) - width_px * 0.5;
    let mut dy = tile_py - camera_py;
    if wrap.wraps_y() {
        let height_px = wrap.height() as f32 * CIV3_CELL_H;
        dy = (dy + height_px * 0.5).rem_euclid(height_px) - height_px * 0.5;
    }
    const ART_SCALE: f32 = 0.75;
    vec2(
        viewport.x + viewport.w * 0.5 + dx * ART_SCALE * zoom,
        viewport.y + viewport.h * 0.5 + dy * ART_SCALE * zoom,
    )
}
/// Legacy projection retained only while the old backend remains selectable.
pub(super) fn amplio2_world_screen_position(
    wrap: MapWrap,
    viewport: Rect,
    camera: Vec2,
    zoom: f32,
    x: i32,
    y: i32,
) -> Vec2 {
    let tile_x = (2 * x + y.rem_euclid(2)) as f32;
    let half_width = wrap.width() as f32;
    let dx = (tile_x - camera.x + half_width).rem_euclid(2.0 * half_width) - half_width;
    let dy = y as f32 - camera.y;
    let dy = if wrap.wraps_y() {
        (dy + wrap.height() as f32 * 0.5).rem_euclid(wrap.height() as f32)
            - wrap.height() as f32 * 0.5
    } else {
        dy
    };
    vec2(
        viewport.x + viewport.w * 0.5 + dx * TILE_W * 0.5 * zoom,
        viewport.y + viewport.h * 0.5 + dy * TILE_H * 0.5 * zoom,
    )
}

/// Kept for legacy projection checks until Amplio2 is retired.
#[cfg(test)]
fn world_screen_position(
    wrap: MapWrap,
    viewport: Rect,
    camera: Vec2,
    zoom: f32,
    x: i32,
    y: i32,
) -> Vec2 {
    amplio2_world_screen_position(wrap, viewport, camera, zoom, x, y)
}

fn visible_world_tiles(
    wrap: MapWrap,
    viewport: Rect,
    camera: Vec2,
    zoom: f32,
    renderer: &dyn RenderBackend,
) -> Vec<(i32, i32, Vec2)> {
    let origin = viewport.y + viewport.h * 0.5;
    let step = TILE_H * 0.5 * zoom;
    let first = (camera.y + (viewport.y - 80.0 - origin) / step)
        .floor()
        .max(0.0) as i32;
    let last = (camera.y + (viewport.bottom() + 80.0 - origin) / step)
        .ceil()
        .min((wrap.height() - 1) as f32) as i32;
    let (first, last) = if wrap.wraps_y() {
        (0, wrap.height() - 1)
    } else {
        (first, last)
    };
    let mut tiles = Vec::with_capacity(((last - first + 1).max(0) * wrap.width()) as usize);
    for y in first..=last {
        for x in 0..wrap.width() {
            let center = renderer.world_screen_position(wrap, viewport, camera, zoom, x, y);
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
                    let mut reference: Vec<_> = (0..MapWrap::X.height())
                        .flat_map(|y| (0..MapWrap::X.width()).map(move |x| (x, y)))
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
                        visible_world_tiles(
                            MapWrap::X,
                            viewport,
                            camera,
                            zoom,
                            &render_backend::Amplio2Renderer,
                        ),
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
        let torus = visible_world_tiles(
            MapWrap::XY,
            viewport,
            camera,
            1.0,
            &render_backend::Amplio2Renderer,
        );
        assert!(torus.iter().any(|t| t.1 == 0));
        assert!(torus.iter().any(|t| t.1 == MAP_H - 1));
        assert!(torus.windows(2).all(|p| p[0].2.y <= p[1].2.y));
        let cylinder = visible_world_tiles(
            MapWrap::X,
            viewport,
            camera,
            1.0,
            &render_backend::Amplio2Renderer,
        );
        assert!(cylinder.iter().all(|t| t.1 > MAP_H / 2));
        let a = world_screen_position(MapWrap::XY, viewport, camera, 1.0, 10, MAP_H - 1);
        let (x, y) = MapWrap::XY.offset(10, MAP_H - 1, 1, 0).unwrap();
        let b = world_screen_position(MapWrap::XY, viewport, camera, 1.0, x, y);
        assert!((a.y - b.y).abs() <= TILE_H * 0.5 + 0.01);
    }
}
