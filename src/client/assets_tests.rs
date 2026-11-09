//! Regression tests for client behavior, including edge cases and interactions with the simulation.

use super::*;

#[test]
fn isolated_cells_exclude_grid_lines_and_duplicate_only_their_own_edges() {
    for bordered in [false, true] {
        let gap = usize::from(bordered);
        let size = 2 * (2 + gap) + gap;
        let mut source = Image::gen_image_color(size as u16, size as u16, GREEN);
        let colors = [
            Color::from_rgba(255, 0, 0, 255),
            Color::from_rgba(0, 0, 255, 255),
            Color::from_rgba(255, 255, 0, 255),
            WHITE,
        ];
        for row in 0..2 {
            for col in 0..2 {
                for y in 0..2 {
                    for x in 0..2 {
                        source.set_pixel(
                            (gap + col * (2 + gap) + x) as u32,
                            (gap + row * (2 + gap) + y) as u32,
                            colors[row * 2 + col],
                        );
                    }
                }
            }
        }
        let atlas = isolated_atlas(&source, 2, 2, 2, 2, bordered);
        for row in 0..2 {
            for col in 0..2 {
                for y in 0..4 {
                    for x in 0..4 {
                        assert_eq!(
                            atlas.get_pixel((col * 4 + x) as u32, (row * 4 + y) as u32),
                            colors[row * 2 + col]
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn all_native_unit_sprites_have_pixels_and_new_types_have_distinct_atlas_cells() {
    assert_eq!(native_unit_rendering_playtest(), (true, true, true));
}
