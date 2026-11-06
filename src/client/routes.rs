//! Native Freeciv goto presentation: cyan segments, turn boundaries and tip.
//! Artwork: Amplio2 terrain1.spec path.{step,normal,exhausted_mp}; turn
//! numbers use tiles.spec city.size_* fallback, as client/tilespec.c does.
use super::*;

// Isolate sprites from the source sheets' colored grid lines at fractional zoom.
fn goto_source(index: usize) -> Rect {
    Rect::new(
        (index % 10 * 98 + 1) as f32,
        (index / 10 * 50 + 1) as f32,
        96.0,
        48.0,
    )
}
fn goto_marker_source(cell: (i32, i32)) -> Rect {
    goto_source(match cell {
        (14, 7) => 0,
        (14, 8) => 1,
        (15, 7) => 2,
        _ => unreachable!(),
    })
}
fn goto_digit_source(row: i32, digit: i32) -> Rect {
    goto_source((3 + (3 - row) * 10 + digit) as usize)
}

pub(super) fn build_goto_atlas(terrain: &Image, status: &Image) -> Image {
    let mut atlas = Image::gen_image_color(980, 200, BLANK);
    let width = atlas.width();
    let dst = atlas.get_image_data_mut();
    for index in 0..33usize {
        let (src, row, col) = if index < 3 {
            let (r, c) = [(14, 7), (14, 8), (15, 7)][index];
            (terrain, r, c)
        } else {
            (status, 3 - (index - 3) / 10, (index - 3) % 10)
        };
        let source = src.get_image_data();
        for y in 0..48usize {
            for x in 0..96usize {
                dst[(index / 10 * 50 + 1 + y) * width + index % 10 * 98 + 1 + x] =
                    source[(1 + row * 49 + y) * src.width() + 1 + col * 97 + x];
            }
        }
    }
    atlas
}

fn shortest_delta(delta: f32, period: f32) -> f32 {
    (delta + period * 0.5).rem_euclid(period) - period * 0.5
}

pub(super) fn marker_cell(destination: bool, moves_left: i32) -> Option<(i32, i32)> {
    if destination {
        Some(if moves_left > 0 { (15, 7) } else { (14, 8) })
    } else {
        (moves_left == 0).then_some((14, 7))
    }
}

impl Game {
    pub(super) fn goto_preview(&self) -> Option<GotoPreview> {
        if !self.goto_mode || self.playback.pending() || self.selected_city.is_some() {
            return None;
        }
        let pointer = ui_pointer();
        if !Self::viewport_rect().contains(pointer) {
            return None;
        }
        let (x, y) = self.screen_to_world(pointer)?;
        self.sim.preview_goto(self.selected_unit?, x, y)
    }

    pub(super) fn draw_goto_preview(&self, assets: &Assets) {
        let Some(preview) = self.goto_preview() else {
            return;
        };
        let viewport = Self::viewport_rect();
        let dpi = screen_dpi_scale();
        unsafe {
            get_internal_gl().quad_gl.scissor(Some((
                (viewport.x * dpi) as i32,
                (viewport.y * dpi) as i32,
                (viewport.w * dpi) as i32,
                (viewport.h * dpi) as i32,
            )));
        }
        let project = |point: &GotoPoint| self.world_screen_position(point.x, point.y);
        let period_x = self.sim.map_wrap.width() as f32 * TILE_W * self.zoom;
        let period_y = self.sim.map_wrap.height() as f32 * TILE_H * 0.5 * self.zoom;
        let shifts_y = if self.sim.map_wrap.wraps_y() {
            vec![-period_y, 0.0, period_y]
        } else {
            vec![0.0]
        };
        for pair in preview.points.windows(2) {
            let from = project(&pair[0]);
            let raw_to = project(&pair[1]);
            // A wrapped edge is two clipped short segments, never a line
            // crossing the complete screen between opposite map edges.
            let to = from
                + vec2(
                    shortest_delta(raw_to.x - from.x, period_x),
                    if self.sim.map_wrap.wraps_y() {
                        shortest_delta(raw_to.y - from.y, period_y)
                    } else {
                        raw_to.y - from.y
                    },
                );
            for shift_x in [-period_x, 0.0, period_x] {
                for &shift_y in &shifts_y {
                    let shift = vec2(shift_x, shift_y);
                    let (a, b) = (from + shift, to + shift);
                    if a.x.max(b.x) < viewport.x
                        || a.x.min(b.x) > viewport.right()
                        || a.y.max(b.y) < viewport.y
                        || a.y.min(b.y) > viewport.bottom()
                    {
                        continue;
                    }
                    draw_line(
                        a.x,
                        a.y,
                        b.x,
                        b.y,
                        (4.0 * self.zoom).max(2.5),
                        Color::from_rgba(0, 36, 36, 230),
                    );
                    draw_line(
                        a.x,
                        a.y,
                        b.x,
                        b.y,
                        (2.0 * self.zoom).max(1.5),
                        Color::from_rgba(0, 255, 200, 255),
                    );
                }
            }
        }
        for (index, point) in preview.points.iter().enumerate().skip(1) {
            let destination = index + 1 == preview.points.len();
            let Some((row, col)) = marker_cell(destination, point.moves_left) else {
                continue;
            };
            let center = project(point);
            let origin = center - vec2(TILE_W, TILE_H) * self.zoom * 0.5;
            let params = |source| DrawTextureParams {
                dest_size: Some(vec2(TILE_W, TILE_H) * self.zoom),
                source: Some(source),
                ..Default::default()
            };
            draw_texture_ex(
                &assets.goto,
                origin.x,
                origin.y,
                WHITE,
                params(goto_marker_source((row, col))),
            );
            // Digits are aligned and composited inside full-tile atlas cells.
            for (row, divisor) in [(3, 1), (2, 10), (1, 100)] {
                if divisor > 1 && point.turn < divisor {
                    continue;
                }
                draw_texture_ex(
                    &assets.goto,
                    origin.x,
                    origin.y,
                    WHITE,
                    params(goto_digit_source(row, (point.turn / divisor) % 10)),
                );
            }
        }
        unsafe {
            get_internal_gl().quad_gl.scissor(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn goto_atlas_preserves_original_sprites_and_discards_grid_lines() {
        let terrain =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_terrain1.png"), None)
                .unwrap();
        let status =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_tiles.png"), None)
                .unwrap();
        let atlas = build_goto_atlas(&terrain, &status);
        let dst = atlas.get_image_data();
        for index in 0..40usize {
            let (source, row, col) = if index < 3 {
                let (r, c) = [(14, 7), (14, 8), (15, 7)][index];
                (&terrain, r, c)
            } else {
                (&status, 3 - (index.min(32) - 3) / 10, (index - 3) % 10)
            };
            for y in 0..50usize {
                for x in 0..98usize {
                    let actual = dst[(index / 10 * 50 + y) * 980 + index % 10 * 98 + x];
                    if index >= 33 || x == 0 || x == 97 || y == 0 || y == 49 {
                        assert_eq!(actual[3], 0);
                    } else {
                        assert_eq!(
                            actual,
                            source.get_image_data()[(row * 49 + y) * source.width() + col * 97 + x]
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn reference_path_markers_distinguish_boundaries_and_destination_movement() {
        assert_eq!(marker_cell(false, 1), None);
        assert_eq!(marker_cell(false, 0), Some((14, 7)));
        assert_eq!(marker_cell(true, 1), Some((15, 7)));
        assert_eq!(marker_cell(true, 0), Some((14, 8)));
    }
    #[test]
    fn seam_segments_take_the_short_adjacent_route() {
        assert_eq!(shortest_delta(990.0, 1000.0), -10.0);
        assert_eq!(shortest_delta(-990.0, 1000.0), 10.0);
        assert_eq!(shortest_delta(24.0, 1000.0), 24.0);
    }
}
