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
        let tile = self.sim.tile(x, y);
        if !tile.seen {
            Knowledge::Unknown
        } else if tile.visible {
            Knowledge::Visible
        } else {
            Knowledge::Remembered
        }
    }

    pub(super) fn draw_corner_fog(
        &self,
        assets: &Assets,
        center: Vec2,
        w: f32,
        h: f32,
        at: impl Fn(i32, i32) -> Knowledge,
    ) {
        let states = corner_group(at);
        if states == [Knowledge::Visible; 4] {
            return;
        }
        let (row, col) = source_cell(states);
        draw_texture_ex(
            &assets.fog,
            center.x - w * 0.5,
            center.y - h,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(w, h)),
                source: Some(Rect::new(
                    1.0 + col as f32 * 98.0,
                    1.0 + row as f32 * 50.0,
                    96.0,
                    48.0,
                )),
                ..Default::default()
            },
        );
    }
}

pub(super) fn source_cell(states: [Knowledge; 4]) -> (usize, usize) {
    // Amplio2's fog.spec is ordered remembered (f), visible (k), unknown (u).
    let index = |state| match state {
        Knowledge::Remembered => 0,
        Knowledge::Visible => 1,
        Knowledge::Unknown => 2,
    };
    (
        index(states[0]) * 3 + index(states[1]),
        index(states[2]) * 3 + index(states[3]),
    )
}

pub(super) fn corner_group(at: impl Fn(i32, i32) -> Knowledge) -> [Knowledge; 4] {
    // Clockwise tiles around this lattice corner, matching fog.spec tags.
    [at(-1, -1), at(0, -1), at(0, 0), at(-1, 0)]
}

pub(super) fn build_atlas(source: &Image) -> Image {
    assert_eq!((source.width(), source.height()), (864, 432));
    let mut atlas = Image::gen_image_color(9 * 98, 9 * 50, BLANK);
    let src = source.get_image_data();
    let dst = atlas.get_image_data_mut();
    for row in 0..9 {
        for col in 0..9 {
            if (row, col) == (4, 4) {
                continue;
            }
            for y in 0..50usize {
                for x in 0..98usize {
                    let sx = x.saturating_sub(1).min(95);
                    let sy = y.saturating_sub(1).min(47);
                    dst[(row * 50 + y) * 882 + col * 98 + x] =
                        src[(row * 48 + sy) * 864 + col * 96 + sx];
                }
            }
        }
    }
    atlas
}
