//! Dedicated Civ III world painter and ordered map passes.

use super::*;
use crate::sim::{Base, RenderTileView, RenderedUnitView, UnitState, WorkKind};

fn unit_animation_action(unit: &RenderedUnitView) -> super::super::tileset::Civ3UnitAction {
    use super::super::tileset::Civ3UnitAction as Action;
    if let Some(work) = unit.work_kind {
        let pause = get_time().rem_euclid(3.0) >= 2.0;
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
        UnitState::Idle | UnitState::Sentry => Action::Fidget,
    }
}

fn unit_animation_progress(
    action: super::super::tileset::Civ3UnitAction,
    selected: bool,
    pacing: super::super::unit_animation_pacing::FidgetPacing,
    fortify_started_at: Option<f64>,
) -> (super::super::tileset::Civ3UnitAction, f32, Option<f32>) {
    use super::super::tileset::Civ3UnitAction as Action;

    if action == Action::Fortify {
        return fortify_started_at.map_or((Action::FortifyHold, 0.0, None), |started_at| {
            (
                Action::Fortify,
                (get_time() - started_at).max(0.0) as f32,
                None,
            )
        });
    }

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
    if matches!(action, Action::Default | Action::FortifyHold) {
        (action, 0.0, None)
    } else {
        const AMBIENT_CYCLE_SECONDS: f32 = 3.0;
        let phase = (get_time() as f32 / AMBIENT_CYCLE_SECONDS).fract();
        (action, phase, Some(AMBIENT_CYCLE_SECONDS))
    }
}

/// Projection and viewport for the shared world and city-map rendering pipeline.
#[derive(Clone, Copy)]
pub(in crate::client) struct WorldMapView {
    pub viewport: Rect,
    pub camera: Vec2,
    pub zoom: f32,
    pub interactive: bool,
}
impl WorldMapView {
    pub fn project(self, wrap: MapWrap, x: i32, y: i32) -> Vec2 {
        super::civ3_world_screen_position(wrap, self.viewport, self.camera, self.zoom, x, y)
    }
    pub fn tiles(self, wrap: MapWrap) -> Vec<(i32, i32, Vec2)> {
        visible_world_tiles(wrap, self.viewport, self.camera, self.zoom)
    }
}

/// Owns the Civ III world-pass orchestration for one frame.
///
/// The map-layer primitives still live on `Game`; keeping the frame inputs in
/// this renderer object gives the Civ III pass a concrete boundary while those
/// primitives are migrated out of the shared world module.
pub(in crate::client) struct Civ3WorldPainter<'a> {
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
            index | (usize::from(irrigated) * flag)
        })
}

fn route_sprite_index(neighbors: [bool; 8]) -> usize {
    // OpenCiv3 TileOverlayLayer::GetRoadFlag uses the 16x16 atlas's bit order.
    const FLAGS: [usize; 8] = [0x80, 0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40];
    neighbors
        .into_iter()
        .zip(FLAGS)
        .fold(0, |index, (connected, flag)| {
            index | (usize::from(connected) * flag)
        })
}

fn contamination_sprite_index(diagonal_neighbors: [bool; 4], variation: usize) -> usize {
    let mask = diagonal_neighbors
        .into_iter()
        .zip([0x1, 0x2, 0x4, 0x8])
        .fold(0usize, |index, (connected, flag)| {
            index | (usize::from(connected) * flag)
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
    pub(in crate::client) fn new(game: &'a Game, assets: &'a Assets) -> Self {
        Self { game, assets }
    }

    pub(super) fn draw(&self) {
        let assets = self.assets;
        let view = WorldMapView {
            viewport: Game::viewport_rect(),
            camera: self.game.camera_overview_position(),
            zoom: self.game.zoom,
            interactive: true,
        };
        self.draw_view(assets, view, || {}, || {});
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
                let tile = self.game.sim.render_tile_view(x, y);
                let mut hilly_neighbors = 0;
                let mut forests = 0;
                let mut jungles = 0;
                // The mountain/hill atlases store NW, NE, SW, SE adjacency
                // masks. Use Civ III's staggered-grid directions here: the
                // legacy render_neighbor offsets resolve to different tiles.
                for (bit, direction) in [7, 1, 5, 3].into_iter().enumerate() {
                    let Some((nx, ny)) = self.game.sim.map_wrap().civ3_neighbor(x, y, direction)
                    else {
                        continue;
                    };
                    let neighbor = self.game.sim.render_tile_view(nx, ny);
                    if !neighbor.seen {
                        continue;
                    }
                    if matches!(
                        neighbor.terrain,
                        Terrain::Hills | Terrain::Mountains | Terrain::Volcano
                    ) {
                        hilly_neighbors |= 1 << bit;
                    } else if neighbor.terrain == Terrain::Forest {
                        forests += 1;
                    } else if neighbor.terrain == Terrain::Jungle {
                        jungles += 1;
                    }
                }
                let dominant_vegetation = if forests > jungles {
                    Some(Terrain::Forest)
                } else if jungles > forests {
                    Some(Terrain::Jungle)
                } else if forests > 0 {
                    Some(if x.rem_euclid(2) == 0 {
                        Terrain::Forest
                    } else {
                        Terrain::Jungle
                    })
                } else {
                    None
                };
                self.assets.civ3.draw_overlay(
                    tile.terrain,
                    super::super::tileset::TerrainOverlayContext {
                        hilly_neighbors,
                        dominant_vegetation,
                    },
                    (x, y),
                    geometry.center,
                    geometry.size(),
                );
            }
            map_layers::MapLayer::Water => self.draw_water_layer(x, y, geometry, has_city),
            map_layers::MapLayer::Roads => {
                self.draw_road_layer(x, y, geometry);
            }
            map_layers::MapLayer::Special1 => self.draw_special1(x, y, geometry, has_city),
            map_layers::MapLayer::Special2 => self.draw_special2(x, y, geometry, has_city),
            map_layers::MapLayer::Grid => self.game.draw_grid_layer(self.assets, x, y, geometry),
            map_layers::MapLayer::City => {}
        }
    }

    fn draw_special1(&self, x: i32, y: i32, geometry: TileGeometry, has_city: bool) {
        use super::super::tileset::TerrainBuilding;
        let game = self.game;
        let assets = self.assets;
        let tile = game.sim.render_tile_view(x, y);
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
        if let Some(resource) = tile.resource {
            assets
                .civ3
                .draw_resource(resource, geometry.center, geometry.size());
        }
        // Keep city buildings visible above fallout/pollution on a city center tile.
        if has_city {
            self.draw_tile_contamination(x, y, geometry);
        }
    }

    fn draw_tile_contamination(&self, x: i32, y: i32, geometry: TileGeometry) {
        let game = self.game;
        let assets = self.assets;
        let tile = game.sim.render_tile_view(x, y);
        let contamination_index = |pollution: bool| {
            let diagonal_directions = [7, 1, 3, 5]; // NW, NE, SE, SW
            let diagonals = diagonal_directions.map(|direction| {
                game.sim
                    .map_wrap()
                    .civ3_neighbor(x, y, direction)
                    .is_some_and(|(nx, ny)| {
                        let neighbor = game.sim.render_tile_view(nx, ny);
                        neighbor.seen
                            && if pollution {
                                neighbor.pollution || neighbor.fallout
                            } else {
                                neighbor.craters
                            }
                    })
            });
            let variation = x
                .wrapping_mul(37)
                .wrapping_add(y.wrapping_mul(13))
                .unsigned_abs() as usize;
            contamination_sprite_index(diagonals, variation)
        };
        if tile.pollution || tile.fallout {
            assets.civ3.draw_contamination(
                false,
                contamination_index(true),
                geometry.center,
                geometry.size(),
            );
        }
        if tile.craters {
            assets.civ3.draw_contamination(
                true,
                contamination_index(false),
                geometry.center,
                geometry.size(),
            );
        }
    }

    fn draw_special2(&self, x: i32, y: i32, geometry: TileGeometry, has_city: bool) {
        use super::super::tileset::TerrainBuilding;
        let game = self.game;
        let assets = self.assets;
        let tile = game.sim.render_tile_view(x, y);
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
        if !has_city {
            self.draw_tile_contamination(x, y, geometry);
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
        let tile = game.sim.render_tile_view(x, y);
        if !has_city {
            if tile.farmland {
                self.assets
                    .civ3
                    .draw_optional_improvement("farmland", center, geometry.size());
            } else if tile.irrigation {
                let wrap = game.sim.map_wrap();
                let diagonal_directions = [7, 1, 5, 3]; // NW, NE, SW, SE
                let diagonals = diagonal_directions.map(|direction| {
                    wrap.civ3_neighbor(x, y, direction).is_some_and(|(nx, ny)| {
                        let neighbor = game.sim.render_tile_view(nx, ny);
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
        let tile = game.sim.render_tile_view(x, y);
        if !tile.road && !tile.railroad {
            return true;
        }
        let neighbors: [Option<RenderTileView>; 8] = std::array::from_fn(|direction| {
            game.sim
                .map_wrap()
                .civ3_neighbor(x, y, direction)
                .map(|(nx, ny)| game.sim.render_tile_view(nx, ny))
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
        if tile.railroad
            && road_index != 0
            && !tileset.draw_road(road_index, geometry.center, geometry.size())
        {
            return false;
        }
        if tile.railroad {
            tileset.draw_railroad(railroad_index, geometry.center, geometry.size())
        } else {
            tileset.draw_road(road_index, geometry.center, geometry.size())
        }
    }

    pub(in crate::client) fn draw_view(
        &self,
        assets: &Assets,
        view: WorldMapView,
        mut before_borders: impl FnMut(),
        mut before_cities: impl FnMut(),
    ) {
        let game = self.game;
        let viewport = view.viewport;
        draw_rectangle(
            viewport.x,
            viewport.y,
            viewport.w,
            viewport.h,
            map_view::MAP_UNKNOWN,
        );

        let tile_w = TILE_W * view.zoom;
        let tile_h = TILE_H * view.zoom;
        // Native rows are already in painter order. Cull before collecting and
        // project each visible tile once for both terrain passes.
        let order = visible_world_tiles(game.sim.map_wrap(), viewport, view.camera, view.zoom);
        let mut cities = std::collections::HashMap::new();
        let city_overrides = game.playback.city_overrides();
        let visible_tiles: std::collections::HashSet<_> =
            order.iter().map(|&(x, y, _)| (x, y)).collect();
        let observed_cities: Vec<_> = game
            .sim
            .rendered_city_views_in(&visible_tiles)
            .into_iter()
            .filter(|city| !city_overrides.contains_key(&city.id))
            .collect();
        let overridden_cities: Vec<_> = city_overrides
            .values()
            .filter(|city| visible_tiles.contains(&(city.x, city.y)))
            .cloned()
            .collect();
        for city in observed_cities.iter().chain(overridden_cities.iter()) {
            cities.entry((city.x, city.y)).or_insert(city);
        }
        let overrides = game.playback.overrides();
        let overridden_units: Vec<_> = overrides.values().cloned().collect();
        let units = game
            .sim
            .rendered_unit_views_in(&visible_tiles)
            .into_iter()
            .filter(|unit| !overrides.contains_key(&unit.id))
            .chain(overridden_units)
            .filter(|unit| {
                unit.visible && (unit.transport.is_none() || Some(unit.id) == game.selected_unit)
            });
        let mut stacks: std::collections::HashMap<(i32, i32), (RenderedUnitView, usize)> =
            std::collections::HashMap::new();
        for unit in units {
            let priority = |u: &RenderedUnitView| {
                let city_tile = cities.contains_key(&(u.x, u.y));
                // A city's selected/playback unit wins; otherwise prefer its
                // fortified garrison over ordinary inactive units.
                (
                    u8::from(city_tile && Some(u.id) != game.selected_unit),
                    u8::from(!game.playback.is_active(u.id)),
                    u8::from(!game.selected_units.contains(&u.id)),
                    u8::from(
                        city_tile
                            && !matches!(u.state, UnitState::Fortifying | UnitState::Fortified),
                    ),
                    unit_draw_priority(u, game.selected_unit),
                )
            };
            if let Some(entry) = stacks.get_mut(&(unit.x, unit.y)) {
                entry.1 += 1;
                if priority(&unit) < priority(&entry.0) {
                    entry.0 = unit;
                }
            } else {
                stacks.insert((unit.x, unit.y), (unit, 1));
            }
        }
        game.draw_edge_water(assets, view);
        // Draw every tile base first. Tall terrain sprites are a separate
        // layer so later diamond bases cannot cut horizontal bands through
        // forests, hills, and mountains.
        for &(x, y, center) in &order {
            let tile = game.sim.render_tile_view(x, y);
            // OpenCiv3's terrain pass contributes neighboring base sprites
            // before fog. CivCraft's base sprites don't have that same overlap,
            // so provide a one-tile underlay around the entire known boundary.
            // The fog atlas masks this fringe into black; deeper unknown
            // terrain remains hidden.
            let fog_fringe = game
                .sim
                .map_wrap()
                .civ3_neighbors(x, y)
                .any(|(nx, ny)| game.sim.render_tile_view(nx, ny).seen);
            if !tile.seen && !fog_fringe {
                continue;
            }
            self.draw_map_base(x, y, center, tile_w, tile_h);
        }
        if view.interactive
            && let Some(unit_id) = game.selected_unit
            && let Some((targets, color)) = match game.targeting {
                TargetingMode::Bombard { precision, .. } => Some((
                    if precision {
                        game.sim.precision_bombardable_targets_in_range(unit_id)
                    } else {
                        game.sim.bombardable_targets_in_range(unit_id)
                    },
                    Color::from_rgba(236, 68, 54, 205),
                )),
                TargetingMode::Rebase => Some((
                    game.sim.rebaseable_bases_in_range(unit_id),
                    Color::from_rgba(72, 184, 245, 220),
                )),
                TargetingMode::Recon => Some((
                    game.sim.recon_targets_in_range(unit_id),
                    Color::from_rgba(238, 206, 92, 140),
                )),
                _ => None,
            }
        {
            for (x, y) in targets {
                let center = view.project(game.sim.map_wrap(), x, y);
                if center.x < viewport.x - tile_w
                    || center.x > viewport.right() + tile_w
                    || center.y < viewport.y - tile_h
                    || center.y > viewport.bottom() + tile_h
                {
                    continue;
                }
                let half_w = tile_w * 0.5;
                let half_h = tile_h * 0.5;
                let corners = [
                    Vec2::new(center.x, center.y - half_h),
                    Vec2::new(center.x + half_w, center.y),
                    Vec2::new(center.x, center.y + half_h),
                    Vec2::new(center.x - half_w, center.y),
                ];
                for index in 0..corners.len() {
                    let from = corners[index];
                    let to = corners[(index + 1) % corners.len()];
                    draw_line(from.x, from.y, to.x, to.y, 2.0, color);
                }
            }
        }
        for layer in map_layers::MAP_LAYERS {
            if layer == map_layers::MapLayer::Grid {
                before_borders();
            }
            if layer == map_layers::MapLayer::City {
                before_cities();
            }
            for &(x, y, center) in &order {
                if !game.sim.render_tile_view(x, y).seen {
                    if layer == map_layers::MapLayer::Grid
                        && game
                            .sim
                            .map_wrap()
                            .civ3_neighbors(x, y)
                            .any(|(nx, ny)| game.sim.render_tile_view(nx, ny).seen)
                    {
                        game.draw_grid_layer(
                            assets,
                            x,
                            y,
                            TileGeometry::new(center, tile_w, tile_h),
                        );
                    }
                    continue;
                }
                let city = cities.get(&(x, y)).copied();
                if layer == map_layers::MapLayer::City {
                    if let Some(city) = city {
                        game.draw_observed_city(assets, city, center, view.zoom);
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
        let target_tile = (view.interactive && viewport.contains(pointer))
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
        let bottom = game.sim.map_wrap().height() + map_view::EDGE_WATER_ROWS;
        let bottom_corner = Some(bottom);
        for y in game.edge_water_rows().chain(bottom_corner) {
            for x in 0..game.sim.map_wrap().width() {
                let center = view.project(game.sim.map_wrap(), x, y);
                if center.y > viewport.bottom() + tile_h || center.y < viewport.y - tile_h {
                    continue;
                }
                let clamped_y = y.clamp(0, game.sim.map_wrap().height() - 1);
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
            let tile = game.sim.render_tile_view(x, y);
            if tile.seen
                && let Some(city) = cities.get(&(x, y)).copied()
            {
                game.draw_city_label_at_zoom(city, center, view.zoom);
            }
        }
        for &(x, y, center) in &order {
            if let Some((unit, count)) = stacks.get(&(x, y)) {
                // All visible city stacks use their representative unit, including
                // idle defenders. Focused units have a later foreground layer.
                if Some(unit.id) != game.selected_unit && !game.playback.is_active(unit.id) {
                    let action = unit_animation_action(unit);
                    let (action, progress, duration) = unit_animation_progress(
                        action,
                        false,
                        self.assets.civ3.fidget_pacing(unit.kind),
                        self.game
                            .fortify_animation_started_at
                            .get(&unit.id)
                            .copied(),
                    );
                    game.draw_unit_at_zoom(
                        assets, unit, center, *count, action, progress, duration, view.zoom,
                    );
                }
            }
        }
        for &(x, y, center) in &order {
            game.draw_visible_base_foreground(assets, x, y, center, tile_w, tile_h);
        }
        for &(x, y, center) in &order {
            if let Some((unit, count)) = stacks.get(&(x, y))
                && Some(unit.id) == game.selected_unit
                && !game.playback.is_active(unit.id)
            {
                let action = unit_animation_action(unit);
                let (action, progress, duration) = unit_animation_progress(
                    action,
                    true,
                    self.assets.civ3.fidget_pacing(unit.kind),
                    self.game
                        .fortify_animation_started_at
                        .get(&unit.id)
                        .copied(),
                );
                game.draw_unit_at_zoom(
                    assets, unit, center, *count, action, progress, duration, view.zoom,
                );
            }
        }
        game.draw_playback(assets, view);
        if view.interactive {
            game.draw_goto_preview(assets);
        }
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
    fn fortify_order_plays_once_then_holds_across_turns() {
        use super::{unit_animation_action, unit_animation_progress};
        use crate::client::tileset::Civ3UnitAction::{Fortify, FortifyHold};
        use crate::sim::{ClientSimulation, Command, HUMAN, PresentationEventView, Simulation};

        let mut sim = ClientSimulation::new(Simulation::new(crate::test_support::seeded_map()));
        let id = sim
            .rendered_unit_views()
            .into_iter()
            .find(|unit| unit.owner == HUMAN && sim.unit_action_view(unit.id).unwrap().fortify)
            .unwrap()
            .id;
        let action = |sim: &ClientSimulation| {
            let unit = sim
                .rendered_unit_views()
                .into_iter()
                .find(|unit| unit.id == id)
                .unwrap();
            unit_animation_action(&unit)
        };
        let fortify_events = |sim: &mut ClientSimulation| {
            sim.take_presentation_event_views().into_iter().filter(|event| {
                matches!(event, PresentationEventView::Fortify { unit, .. } if unit.id == id)
            }).count()
        };

        assert!(sim.apply(Command::Fortify(id)));
        assert_eq!(fortify_events(&mut sim), 1);
        assert_eq!(action(&sim), Fortify);
        for _ in 0..2 {
            assert!(sim.apply(Command::EndTurn));
            assert_eq!(fortify_events(&mut sim), 0);
            assert_eq!(action(&sim), FortifyHold);
            for selected in [false, true] {
                assert_eq!(
                    unit_animation_progress(action(&sim), selected, Default::default(), Some(0.0)),
                    (FortifyHold, 0.0, None)
                );
            }
        }
    }

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
