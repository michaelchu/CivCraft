//! Dedicated Civ III world painter and ordered map passes.

use super::*;
use crate::sim::{Base, Unit, UnitState, WorkKind};

fn unit_animation_action(unit: &Unit) -> super::super::tileset::Civ3UnitAction {
    use super::super::tileset::Civ3UnitAction as Action;
    let pause = get_time().rem_euclid(3.0) >= 2.0;
    if let Some(work) = unit.work_kind {
        return match work {
            WorkKind::Road | WorkKind::Railroad => {
                if pause {
                    Action::PauseRoad
                } else {
                    Action::Road
                }
            }
            WorkKind::Mine | WorkKind::OilWell | WorkKind::OilPlatform => {
                if pause {
                    Action::PauseMine
                } else {
                    Action::Mine
                }
            }
            WorkKind::Irrigation | WorkKind::Farmland | WorkKind::Cultivate => {
                if pause {
                    Action::PauseIrrigate
                } else {
                    Action::Irrigate
                }
            }
            WorkKind::Plant => {
                if pause {
                    Action::PauseForest
                } else {
                    Action::Plant
                }
            }
            WorkKind::BuildBase(Base::Fortress) => Action::Fortress,
            WorkKind::BuildBase(_) | WorkKind::Clean(_) => Action::Build,
            // The source CAPTURE clips depict captured units (and are also
            // used for ICBM launch in the original INI). Pillaging has no
            // corresponding campaign INI state, so keep the unit on its
            // ordinary idle loop instead of reusing an unrelated action.
            WorkKind::Pillage(_) => Action::Fidget,
        };
    }
    match unit.state {
        UnitState::Fortifying => Action::Fortify,
        UnitState::Fortified => Action::FortifyHold,
        UnitState::Idle | UnitState::Sentry | UnitState::AirSuperiority => Action::Fidget,
    }
}

fn unit_animation_progress(
    action: super::super::tileset::Civ3UnitAction,
    selected: bool,
    pacing: super::super::unit_animation_pacing::FidgetPacing,
) -> (super::super::tileset::Civ3UnitAction, f32, Option<f32>) {
    use super::super::tileset::Civ3UnitAction as Action;

    if action == Action::Fidget {
        if !selected {
            return (Action::Default, 0.0, None);
        }
        // Play one slow gesture, then let the selected unit rest before the
        // next one begins.
        let period_seconds = pacing.play_seconds + pacing.rest_seconds;
        let phase = (get_time() as f32).rem_euclid(period_seconds);
        if phase >= pacing.play_seconds {
            return (Action::Default, 0.0, None);
        }
        return (
            Action::Fidget,
            phase / pacing.play_seconds,
            Some(pacing.play_seconds),
        );
    }
    if action == super::super::tileset::Civ3UnitAction::Default {
        (action, 0.0, None)
    } else {
        const AMBIENT_CYCLE_SECONDS: f32 = 3.0;
        let phase = (get_time() as f32 / AMBIENT_CYCLE_SECONDS).fract();
        (action, phase, Some(AMBIENT_CYCLE_SECONDS))
    }
}

/// Owns the Civ III world-pass orchestration for one frame.
///
/// The map-layer primitives still live on `Game`; keeping the frame inputs in
/// this renderer object gives the Civ III pass a concrete boundary while those
/// primitives are migrated out of the shared world module.
pub(super) struct Civ3WorldPainter<'a> {
    game: &'a Game,
    assets: &'a Assets,
}

/// OpenCiv3's irrigation layer selects one of 16 cells from the four diagonal
/// neighbor flags. OpenCiv3 marks this mapping as approximate, so keep it
/// isolated rather than treating it as confirmed original-engine behavior.
fn irrigation_sprite_index(diagonals: [bool; 4]) -> usize {
    diagonals
        .into_iter()
        .zip([0x1, 0x2, 0x4, 0x8])
        .fold(0, |index, (irrigated, flag)| {
            index | usize::from(irrigated) * flag
        })
}

fn route_sprite_index(neighbors: [bool; 8]) -> usize {
    // OpenCiv3 TileOverlayLayer::GetRoadFlag uses the 16x16 atlas's bit order.
    const FLAGS: [usize; 8] = [0x80, 0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40];
    neighbors
        .into_iter()
        .zip(FLAGS)
        .fold(0, |index, (connected, flag)| {
            index | usize::from(connected) * flag
        })
}

fn contamination_sprite_index(diagonal_neighbors: [bool; 4], variation: usize) -> usize {
    let mask = diagonal_neighbors
        .into_iter()
        .zip([0x1, 0x2, 0x4, 0x8])
        .fold(0usize, |index, (connected, flag)| {
            index | usize::from(connected) * flag
        });
    if mask == 0 {
        variation % 10
    } else {
        // OpenCiv3 reserves the first ten cells for isolated variants, then
        // places the 16 diagonal-connectivity masks in atlas cells 10..=24.
        10 + mask - 1
    }
}

impl<'a> Civ3WorldPainter<'a> {
    pub(super) fn new(game: &'a Game, assets: &'a Assets) -> Self {
        Self { game, assets }
    }

    pub(super) fn draw(&self) {
        let assets = self.assets;
        self.draw_ordered_world_passes(assets);
    }

    pub(super) fn draw_map_base(&self, x: i32, y: i32, center: Vec2, width: f32, height: f32) {
        let game = self.game;
        self.assets
            .civ3
            .draw_base(game, x, y, center, vec2(width, height));
    }

    pub(super) fn draw_map_layer(
        &self,
        layer: map_layers::MapLayer,
        x: i32,
        y: i32,
        geometry: TileGeometry,
        has_city: bool,
    ) {
        match layer {
            map_layers::MapLayer::Blend => {}
            map_layers::MapLayer::Terrain => {
                let tile = self.game.sim.render_tile(x, y);
                self.assets.civ3.draw_overlay(
                    tile.terrain,
                    (x, y),
                    geometry.center,
                    geometry.size(),
                );
            }
            map_layers::MapLayer::Water => self.draw_water_layer(x, y, geometry, has_city),
            map_layers::MapLayer::Roads => {
                self.draw_road_layer(x, y, geometry);
            }
            map_layers::MapLayer::Special1 => self.draw_special1(x, y, geometry),
            map_layers::MapLayer::Special2 => self.draw_special2(x, y, geometry),
            map_layers::MapLayer::Grid => self.game.draw_grid_layer(x, y, geometry),
            map_layers::MapLayer::City => {}
        }
    }

    fn draw_special1(&self, x: i32, y: i32, geometry: TileGeometry) {
        use super::super::tileset::TerrainBuilding;
        let game = self.game;
        let assets = self.assets;
        let tile = game.sim.render_tile(x, y);
        if matches!(tile.base, Some(Base::Fort | Base::Fortress)) {
            assets.civ3.draw_terrain_building(
                TerrainBuilding::Fortress,
                geometry.center,
                geometry.size(),
            );
        }
        if tile.oil_platform {
            assets
                .civ3
                .draw_optional_improvement("oil-platform", geometry.center, geometry.size());
        } else if tile.oil_well {
            assets
                .civ3
                .draw_optional_improvement("oil-well", geometry.center, geometry.size());
        } else if tile.mine {
            assets.civ3.draw_terrain_building(
                TerrainBuilding::Mine,
                geometry.center,
                geometry.size(),
            );
        }
        if tile.hut {
            assets.civ3.draw_terrain_building(
                TerrainBuilding::BarbarianCamp,
                geometry.center,
                geometry.size(),
            );
        }
        if let Some(resource) = tile
            .active_resource()
            .filter(|&resource| game.sim.resource_visible_to(HUMAN, resource))
            && !assets
                .civ3
                .draw_resource(resource, geometry.center, geometry.size())
        {
            let (row, col) = resource.icon_cell();
            game.draw_resource_icon(
                &assets.resource_icons,
                geometry.center,
                geometry.width,
                geometry.height,
                row,
                col,
            );
        }
    }

    fn draw_special2(&self, x: i32, y: i32, geometry: TileGeometry) {
        use super::super::tileset::TerrainBuilding;
        let game = self.game;
        let assets = self.assets;
        let tile = game.sim.render_tile(x, y);
        if let Some(Base::Airstrip | Base::Airbase) = tile.base {
            let role = if tile.base == Some(Base::Airstrip) {
                "airstrip"
            } else {
                "airbase"
            };
            assets
                .civ3
                .draw_optional_improvement(role, geometry.center, geometry.size());
        }
        let contamination_index = |pollution: bool| {
            let diagonal_directions = [7, 1, 3, 5]; // NW, NE, SE, SW
            let diagonals = diagonal_directions.map(|direction| {
                game.sim
                    .map_wrap
                    .civ3_neighbor(x, y, direction)
                    .is_some_and(|(nx, ny)| {
                        let neighbor = game.sim.render_tile(nx, ny);
                        neighbor.seen
                            && if pollution {
                                neighbor.pollution
                            } else {
                                neighbor.fallout || neighbor.craters
                            }
                    })
            });
            let variation = x
                .wrapping_mul(37)
                .wrapping_add(y.wrapping_mul(13))
                .unsigned_abs() as usize;
            contamination_sprite_index(diagonals, variation)
        };
        if tile.pollution {
            assets.civ3.draw_contamination(
                false,
                contamination_index(true),
                geometry.center,
                geometry.size(),
            );
        }
        if tile.fallout || tile.craters {
            assets.civ3.draw_contamination(
                true,
                contamination_index(false),
                geometry.center,
                geometry.size(),
            );
        }
        if !tile.visible && matches!(tile.base, Some(Base::Fort | Base::Fortress)) {
            assets.civ3.draw_terrain_building(
                TerrainBuilding::Fortress,
                geometry.center,
                geometry.size(),
            );
        }
    }

    fn draw_water_layer(&self, x: i32, y: i32, geometry: TileGeometry, has_city: bool) {
        let game = self.game;
        let TileGeometry { center, .. } = geometry;
        let tile = game.sim.render_tile(x, y);
        if !has_city {
            if tile.farmland {
                self.assets
                    .civ3
                    .draw_optional_improvement("farmland", center, geometry.size());
            } else if tile.irrigation {
                let wrap = game.sim.map_wrap;
                let diagonal_directions = [7, 1, 5, 3]; // NW, NE, SW, SE
                let diagonals = diagonal_directions.map(|direction| {
                    wrap.civ3_neighbor(x, y, direction).is_some_and(|(nx, ny)| {
                        let neighbor = game.sim.render_tile(nx, ny);
                        neighbor.seen && neighbor.irrigation
                    })
                });
                let index = irrigation_sprite_index(diagonals);
                self.assets
                    .civ3
                    .draw_irrigation(tile.base_terrain, center, geometry.size(), index);
            }
        }
        self.assets
            .civ3
            .draw_river(game, x, y, center, geometry.size());
    }

    fn draw_road_layer(&self, x: i32, y: i32, geometry: TileGeometry) -> bool {
        let game = self.game;
        let tileset = &self.assets.civ3;
        let tile = game.sim.render_tile(x, y);
        if !tile.road && !tile.railroad {
            return true;
        }
        let neighbors: [Option<&crate::sim::Tile>; 8] = std::array::from_fn(|direction| {
            game.sim
                .map_wrap
                .civ3_neighbor(x, y, direction)
                .map(|(nx, ny)| game.sim.render_tile(nx, ny))
        });
        let road_connections = std::array::from_fn(|direction| {
            neighbors[direction]
                .is_some_and(|neighbor| neighbor.seen && (neighbor.road || neighbor.railroad))
        });
        let railroad_connections = std::array::from_fn(|direction| {
            neighbors[direction].is_some_and(|neighbor| neighbor.seen && neighbor.railroad)
        });
        let (road_index, railroad_index) = if tile.railroad {
            let road_only_connections = std::array::from_fn(|direction| {
                neighbors[direction]
                    .is_some_and(|neighbor| neighbor.seen && neighbor.road && !neighbor.railroad)
            });
            (
                route_sprite_index(road_only_connections),
                route_sprite_index(railroad_connections),
            )
        } else {
            (route_sprite_index(road_connections), 0)
        };
        if tile.railroad && road_index != 0 {
            if !tileset.draw_road(road_index, geometry.center, geometry.size()) {
                return false;
            }
        }
        if tile.railroad {
            tileset.draw_railroad(railroad_index, geometry.center, geometry.size())
        } else {
            tileset.draw_road(road_index, geometry.center, geometry.size())
        }
    }

    fn draw_ordered_world_passes(&self, assets: &Assets) {
        let game = self.game;
        let viewport = Game::viewport_rect();
        draw_rectangle(
            viewport.x,
            viewport.y,
            viewport.w,
            viewport.h,
            map_view::MAP_UNKNOWN,
        );

        let tile_w = TILE_W * game.zoom;
        let tile_h = TILE_H * game.zoom;
        // Native rows are already in painter order. Cull before collecting and
        // project each visible tile once for both terrain passes.
        let order = visible_world_tiles(
            game.sim.map_wrap,
            viewport,
            game.camera_overview_position(),
            game.zoom,
            game.renderer.as_ref(),
        );
        let mut cities = std::collections::HashMap::new();
        let city_overrides = game.playback.city_overrides();
        let visible_tiles: std::collections::HashSet<_> =
            order.iter().map(|&(x, y, _)| (x, y)).collect();
        let observed_cities = game.sim.render_cities_where(|city| {
            visible_tiles.contains(&(city.x, city.y)) && !city_overrides.contains_key(&city.id)
        });
        let overridden_cities: Vec<_> = city_overrides
            .values()
            .filter(|city| visible_tiles.contains(&(city.x, city.y)))
            .map(|city| game.sim.observed_city(city))
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
        let overrides = game.playback.overrides();
        for unit in game
            .sim
            .units()
            .iter()
            .filter(|u| !overrides.contains_key(&u.id))
            .chain(overrides.values())
            .filter(|u| {
                game.sim.unit_visible_to_human(u)
                    && (u.transport.is_none() || Some(u.id) == game.selected_unit)
            })
        {
            let entry = stacks.entry((unit.x, unit.y)).or_insert((unit, 0));
            entry.1 += 1;
            let priority = |u: &Unit| {
                (
                    u8::from(!game.playback.is_active(u.id)),
                    unit_draw_priority(u, game.selected_unit),
                )
            };
            if priority(unit) < priority(entry.0) {
                entry.0 = unit;
            }
        }
        game.draw_edge_water(assets, viewport, tile_w, tile_h);
        // Draw every tile base first. Tall terrain sprites are a separate
        // layer so later diamond bases cannot cut horizontal bands through
        // forests, hills, and mountains.
        for &(x, y, center) in &order {
            let tile = game.sim.render_tile(x, y);
            // OpenCiv3's terrain pass contributes neighboring base sprites
            // before fog. CivCraft's base sprites don't have that same overlap,
            // so provide a one-tile underlay around the entire known boundary.
            // The fog atlas masks this fringe into black; deeper unknown
            // terrain remains hidden.
            let fog_fringe = game
                .sim
                .map_wrap
                .civ3_neighbors(x, y)
                .any(|(nx, ny)| game.sim.render_tile(nx, ny).seen);
            if !tile.seen && !fog_fringe {
                continue;
            }
            self.draw_map_base(x, y, center, tile_w, tile_h);
        }

        for layer in map_layers::MAP_LAYERS {
            for &(x, y, center) in &order {
                if !game.sim.render_tile(x, y).seen {
                    continue;
                }
                let city = cities.get(&(x, y)).copied();
                if layer == map_layers::MapLayer::City {
                    if let Some(city) = city {
                        game.draw_observed_city(assets, city, center, game.zoom);
                    }
                } else {
                    self.draw_map_layer(
                        layer,
                        x,
                        y,
                        TileGeometry::new(center, tile_w, tile_h),
                        city.is_some(),
                    );
                }
            }
        }
        // Blend corners after terrain/cities and before units, labels and focus
        // sprites. Visual water inherits polar-row fog.
        let pointer = ui_pointer();
        let target_tile = viewport
            .contains(pointer)
            .then(|| game.screen_to_world(pointer))
            .flatten();
        for &(x, y, center) in &order {
            game.draw_fog_intersection(
                assets,
                center,
                tile_w,
                tile_h,
                (x, y),
                target_tile,
                |nx, ny| game.world_fog_knowledge(nx, ny),
            );
        }
        // Complete corner fog for the decorative rows and their southern edge.
        let bottom = game.sim.map_wrap.height() + map_view::EDGE_WATER_ROWS;
        let bottom_corner = Some(bottom);
        for y in game.edge_water_rows().chain(bottom_corner) {
            for x in 0..game.sim.map_wrap.width() {
                let center = game.world_screen_position(x, y);
                if center.y > viewport.bottom() + tile_h || center.y < viewport.y - tile_h {
                    continue;
                }
                let clamped_y = y.clamp(0, game.sim.map_wrap.height() - 1);
                game.draw_fog_intersection(
                    assets,
                    center,
                    tile_w,
                    tile_h,
                    (x, clamped_y),
                    target_tile,
                    |nx, ny| game.world_fog_knowledge(nx, ny),
                );
            }
        }
        for &(x, y, center) in &order {
            let tile = game.sim.render_tile(x, y);
            if !tile.visible {
                continue;
            }
            if let Some(&(unit, count)) = stacks.get(&(x, y)) {
                // Idle city garrisons are hidden; focused units have a later
                // foreground layer.
                if !cities.contains_key(&(x, y))
                    && Some(unit.id) != game.selected_unit
                    && !game.playback.is_active(unit.id)
                {
                    let action = unit_animation_action(unit);
                    let (action, progress, duration) = unit_animation_progress(
                        action,
                        false,
                        self.assets.civ3.fidget_pacing(unit.kind),
                    );
                    game.draw_unit(assets, unit, center, count, action, progress, duration);
                }
            }
        }
        for &(x, y, center) in &order {
            game.draw_visible_base_foreground(assets, x, y, center, tile_w, tile_h);
        }
        for &(x, y, center) in &order {
            let tile = game.sim.render_tile(x, y);
            if tile.seen
                && let Some(city) = cities.get(&(x, y)).copied()
            {
                game.draw_city_label(&city.city, center);
            }
        }
        for &(x, y, center) in &order {
            if game.sim.render_tile(x, y).visible
                && let Some(&(unit, count)) = stacks.get(&(x, y))
                && Some(unit.id) == game.selected_unit
                && !game.playback.is_active(unit.id)
            {
                let action = unit_animation_action(unit);
                let (action, progress, duration) = unit_animation_progress(
                    action,
                    true,
                    self.assets.civ3.fidget_pacing(unit.kind),
                );
                game.draw_unit(assets, unit, center, count, action, progress, duration);
            }
        }
        game.draw_playback(assets);
        game.draw_goto_preview(assets);
    }
}

impl Game {
    pub(in crate::client) fn draw_civ3_world_map(&self, assets: &Assets) {
        Civ3WorldPainter::new(self, assets).draw();
    }
}

#[cfg(test)]
mod tests {
    use super::{contamination_sprite_index, irrigation_sprite_index, route_sprite_index};

    #[test]
    fn irrigation_indices_follow_the_four_diagonal_source_flags() {
        assert_eq!(irrigation_sprite_index([false; 4]), 0);
        assert_eq!(irrigation_sprite_index([true, false, false, false]), 1);
        assert_eq!(irrigation_sprite_index([false, true, false, false]), 2);
        assert_eq!(irrigation_sprite_index([false, false, true, false]), 4);
        assert_eq!(irrigation_sprite_index([false, false, false, true]), 8);
        assert_eq!(irrigation_sprite_index([true; 4]), 15);
    }

    #[test]
    fn road_indices_follow_all_eight_civ3_directions() {
        let mut neighbors = [false; 8];
        for (direction, expected) in [0x80, 0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40]
            .into_iter()
            .enumerate()
        {
            neighbors[direction] = true;
            assert_eq!(route_sprite_index(neighbors), expected);
            neighbors[direction] = false;
        }
        assert_eq!(route_sprite_index([true; 8]), 0xff);
    }

    #[test]
    fn contamination_atlas_uses_diagonal_connectivity_after_isolated_variants() {
        assert_eq!(contamination_sprite_index([false; 4], 0), 0);
        assert_eq!(contamination_sprite_index([false; 4], 9), 9);
        assert_eq!(contamination_sprite_index([false; 4], 10), 0);
        assert_eq!(
            contamination_sprite_index([true, false, false, false], 0),
            10
        );
        assert_eq!(
            contamination_sprite_index([false, true, false, false], 0),
            11
        );
        assert_eq!(
            contamination_sprite_index([false, false, true, false], 0),
            13
        );
        assert_eq!(
            contamination_sprite_index([false, false, false, true], 0),
            17
        );
        assert_eq!(contamination_sprite_index([true; 4], 0), 24);
    }
}
