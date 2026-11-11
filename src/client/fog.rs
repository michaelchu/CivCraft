//! Civ III map visibility shading, shared by world and city tile views.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Knowledge {
    Unknown,
    Remembered,
    Visible,
}

impl Game {
    pub(super) fn fog_knowledge(&self, x: i32, y: i32) -> Knowledge {
        let Some((x, y)) = self.sim.map_wrap.normalize(x, y) else {
            return Knowledge::Remembered;
        };
        let tile = self.sim.render_tile_view(x, y);
        if !tile.seen {
            Knowledge::Unknown
        } else if tile.visible {
            Knowledge::Visible
        } else {
            Knowledge::Remembered
        }
    }

    pub(super) fn draw_fog_intersection(
        &self,
        assets: &Assets,
        center: Vec2,
        w: f32,
        h: f32,
        tile: (i32, i32),
        target_tile: Option<(i32, i32)>,
        at: impl Fn(i32, i32) -> Knowledge,
    ) {
        // OpenCiv3 renders the north intersection of each tile: its north
        // neighbor, the tile itself as south, and its northeast/northwest
        // neighbors as east/west. Resolve them through the same staggered
        // Civ III topology used by movement and visibility.
        let neighbor = |direction| {
            self.sim
                .map_wrap
                .civ3_neighbor(tile.0, tile.1, direction)
                .map_or(Knowledge::Unknown, |coord| {
                    fog_status(coord, target_tile, &at)
                })
        };
        let north = neighbor(0); // NORTH
        let south = fog_status(tile, target_tile, &at);
        let east = neighbor(1); // NORTHEAST
        let west = neighbor(7); // NORTHWEST
        let (row, col) = source_fog_cell(north, south, east, west);
        // OpenCiv3 skips atlas cell (8, 8), where all four surrounding tiles
        // are currently visible.
        if (row, col) == (8, 8) {
            return;
        }
        draw_texture_ex(
            &assets.source_fog,
            center.x - w * 0.5,
            center.y - h,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(w, h)),
                source: Some(Rect::new(
                    col as f32 * 128.0,
                    row as f32 * 64.0,
                    128.0 * 0.999,
                    64.0 * 0.999,
                )),
                ..Default::default()
            },
        );
    }
}

fn fog_status(
    coord: (i32, i32),
    target_tile: Option<(i32, i32)>,
    at: &impl Fn(i32, i32) -> Knowledge,
) -> Knowledge {
    if coord.0 < 0 || coord.1 < 0 {
        return Knowledge::Unknown;
    }
    let knowledge = at(coord.0, coord.1);
    if knowledge != Knowledge::Unknown && Some(coord) == target_tile {
        Knowledge::Visible
    } else {
        knowledge
    }
}

/// Match OpenCiv3's 9x9 FogOfWar atlas indexing. `north` and `west` select
/// the column; `east` and `south` select the row. Each state contributes 0
/// when unknown, 1 when remembered, and 2 when currently visible, with the
/// west/south state in the high trit.
pub(super) fn source_fog_cell(
    north: Knowledge,
    south: Knowledge,
    east: Knowledge,
    west: Knowledge,
) -> (usize, usize) {
    let active = |state| match state {
        Knowledge::Unknown => 0,
        Knowledge::Remembered => 1,
        Knowledge::Visible => 2,
    };
    let column = active(north) + active(west) * 3;
    let row = active(east) + active(south) * 3;
    (row, column)
}
