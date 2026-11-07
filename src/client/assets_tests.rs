
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
fn water_backing_covers_mask_edges_without_changing_opaque_ocean_art() {
    let ocean =
        Image::from_file_with_format(include_bytes!("../../assets/amplio2_ocean.png"), None)
            .unwrap();
    let terrain =
        Image::from_file_with_format(include_bytes!("../../assets/amplio2_terrain1.png"), None)
            .unwrap();
    let water =
        Image::from_file_with_format(include_bytes!("../../assets/amplio2_water.png"), None)
            .unwrap();
    let corners = build_ocean_corner_atlas(&ocean, &terrain);
    let backing = masked_terrain_backing_atlas(&corners, &water, &terrain);
    assert!(backing.get_image_data().iter().all(|p| p[3] == 255));
    for kind in 0..2 {
        let (row, col) = ocean_cell_source([kind as u8; 4]);
        for y in 0..48 {
            for x in 0..96 {
                let original = (0..4)
                    .rev()
                    .map(|corner| {
                        corners.get_image_data()
                            [(corner * OCEAN_ATLAS_H + row * 48 + y) * OCEAN_ATLAS_W + col * 96 + x]
                    })
                    .find(|p| p[3] == 255);
                if let Some(pixel) = original {
                    assert_eq!(
                        backing.get_image_data()[(kind * 50 + 1 + y) * 98 + 1 + x],
                        pixel
                    );
                }
            }
        }
    }
    // Lake edges must use lake blue, independently of ocean colors.
    let lake_source = water.get_image_data()[(437 + 9 * 25 + 12) * water.width() + 1 + 24];
    assert_eq!(
        backing.get_image_data()[(2 * 50 + 1 + 12) * 98 + 1 + 48],
        lake_source
    );
}

#[test]
fn ocean_mask_overlaps_the_raster_edge_without_holes() {
    let ocean =
        Image::from_file_with_format(include_bytes!("../../assets/amplio2_ocean.png"), None)
            .unwrap();
    let terrain =
        Image::from_file_with_format(include_bytes!("../../assets/amplio2_terrain1.png"), None)
            .unwrap();
    let corners = build_ocean_corner_atlas(&ocean, &terrain);
    let (row, col) = ocean_cell_source([0; 4]);
    let mut extended = 0;
    for y in 0..48usize {
        for x in 0..96usize {
            let original =
                terrain.get_image_data()[(1 + 15 * 49 + y) * terrain.width() + 1 + 2 * 97 + x][3];
            let covered = (0..4).any(|corner| {
                corners.get_image_data()
                    [(corner * OCEAN_ATLAS_H + row * 48 + y) * OCEAN_ATLAS_W + col * 96 + x][3]
                    > 0
            });
            if original > 0 {
                assert!(covered, "hole at {x},{y}");
            }
            if original == 0 && covered {
                extended += 1;
            }
        }
    }
    assert!(
        extended > 0,
        "fractional zoom needs coverage beyond the native alpha edge"
    );
}

#[test]
fn ice_backing_fills_transparent_edges_with_ice_artwork() {
    let ocean =
        Image::from_file_with_format(include_bytes!("../../assets/amplio2_ocean.png"), None)
            .unwrap();
    let terrain =
        Image::from_file_with_format(include_bytes!("../../assets/amplio2_terrain1.png"), None)
            .unwrap();
    let water =
        Image::from_file_with_format(include_bytes!("../../assets/amplio2_water.png"), None)
            .unwrap();
    let corners = build_ocean_corner_atlas(&ocean, &terrain);
    let backing = masked_terrain_backing_atlas(&corners, &water, &terrain);
    for y in 0..48 {
        for x in 0..96 {
            let original = terrain.get_image_data()[(1 + 7 * 49 + y) * terrain.width() + 1 + x];
            let pixel = backing.get_image_data()[(3 * 50 + 1 + y) * 98 + 1 + x];
            assert_eq!(pixel[3], 255);
            if original[3] == 255 {
                assert_eq!(pixel, original);
            } else {
                assert!(pixel[0] > 200 && pixel[1] > 200, "edge must be ice-colored");
            }
        }
    }
}

#[test]
fn cliff_rim_extension_preserves_shadows_and_lake_cells() {
    let water =
        Image::from_file_with_format(include_bytes!("../../assets/amplio2_water.png"), None)
            .unwrap();
    let original = isolated_atlas_at(&water, 48, 24, 16, 12, (1, 437), 1);
    let fixed = build_water_corner_atlas(&water);
    let before = original.get_image_data();
    let after = fixed.get_image_data();
    let stride = fixed.width();
    let mut changed = 0;
    for (i, (&a, &b)) in before.iter().zip(after).enumerate() {
        if a != b {
            changed += 1;
            assert_eq!(a[3], 0, "opaque artwork/shadows must not move");
            assert!(i / stride < 3 * 26, "only ice-cliff rows may change");
            assert_eq!(
                b,
                before[i + stride],
                "new rim pixel must come from directly below"
            );
            assert!(b[0] >= 190 && b[1] >= 190 && b[2] >= 190 && b[3] == 255);
        }
    }
    assert!(changed > 0);
}

#[test]
fn ocean_diamond_geometry_cannot_cover_an_adjacent_ice_tile() {
    fn cross(a: Vec2, b: Vec2) -> f32 {
        a.x * b.y - a.y * b.x
    }
    fn in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
        let signs = [
            cross(b - a, p - a),
            cross(c - b, p - b),
            cross(a - c, p - c),
        ];
        signs.iter().all(|s| *s >= 0.0) || signs.iter().all(|s| *s <= 0.0)
    }
    for zoom in [0.625, 0.82, 1.0, 1.35] {
        let center = vec2(311.137, 127.213);
        let size = vec2(96.0, 48.0) * zoom;
        let mut vertices = vec![Vertex::new(0.0, 0.0, 0.0, 0.0, 0.0, WHITE); 4];
        set_diamond_vertices(
            &mut vertices,
            center,
            size,
            isolated_sprite_source(0, 0, 96.0, 48.0),
            vec2(882.0, 1800.0),
            WHITE,
        );
        let positions: Vec<_> = vertices
            .iter()
            .map(|v| vec2(v.position.x, v.position.y))
            .collect();
        let covered = |point| {
            in_triangle(point, positions[0], positions[1], positions[2])
                || in_triangle(point, positions[0], positions[2], positions[3])
        };
        assert!(covered(center + size * vec2(0.2, -0.2)));
        for delta in [
            vec2(0.3, -0.3),
            vec2(0.3, 0.3),
            vec2(-0.3, 0.3),
            vec2(-0.3, -0.3),
        ] {
            assert!(
                !covered(center + size * delta),
                "ocean must not cover neighboring ice at fractional zoom"
            );
        }
    }
}

#[test]
fn all_native_unit_sprites_have_pixels_and_new_types_have_distinct_atlas_cells() {
    assert_eq!(native_unit_rendering_playtest(), (true, true, true));
}
