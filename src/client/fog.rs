//! Amplio2's three-state corner fog, shared by world and city tile views.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Knowledge {
    Unknown,
    Remembered,
    Visible,
}

pub(super) fn source_cell(states: [Knowledge; 4]) -> (usize, usize) {
    // fog.spec is ordered f,k,u, unlike native ternary IDs u,f,k.
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
    // Clockwise N/E/S/W tiles around this tile's top vertex.
    [at(-1, -1), at(0, -1), at(0, 0), at(-1, 0)]
}

// Native Freeciv loads each sprite separately. Our GPU atlas needs gutters
// to preserve that isolation at fractional zoom, especially near the unused
// all-visible cell containing credits rather than a drawable fog sprite.
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

impl Game {
    pub(super) fn fog_knowledge(&self, x: i32, y: i32) -> Knowledge {
        if !(0..MAP_H).contains(&y) {
            return Knowledge::Remembered;
        }
        let tile = self.sim.tile(x.rem_euclid(MAP_W), y);
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
        // Each lattice corner is drawn once, exactly as LAYER_FOG in the
        // reference. The sprite is centered on this tile's top vertex.
        let group = corner_group(at);
        if group == [Knowledge::Visible; 4] {
            return;
        }
        let (row, col) = source_cell(group);
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_corner_state_matches_the_reference_sprite_tag() {
        let states = [
            Knowledge::Remembered,
            Knowledge::Visible,
            Knowledge::Unknown,
        ];
        let spec = include_str!("../../assets/amplio2_fog.spec");
        for a in 0..3 {
            for b in 0..3 {
                for c in 0..3 {
                    for d in 0..3 {
                        let group = [states[a], states[b], states[c], states[d]];
                        let ids = ['f', 'k', 'u'];
                        let tag = format!("t.fog_{}_{}_{}_{}", ids[a], ids[b], ids[c], ids[d]);
                        let line = spec
                            .lines()
                            .find(|line| line.contains(&format!("\"{tag}\"")))
                            .unwrap();
                        // All-visible is commented out in the source: skip drawing it.
                        let fields: Vec<_> = line
                            .trim()
                            .trim_start_matches(';')
                            .split(',')
                            .map(str::trim)
                            .collect();
                        assert_eq!(
                            source_cell(group),
                            (fields[0].parse().unwrap(), fields[1].parse().unwrap())
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn atlas_has_the_reference_full_fog_and_soft_transition_alpha() {
        let image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_fog.png"), None)
                .unwrap();
        assert_eq!((image.width(), image.height()), (864, 432));
        let pixels = image.get_image_data();
        assert_eq!(pixels[24 * 864 + 48][3], 133); // f_f_f_f
        assert_eq!(pixels[(8 * 48 + 24) * 864 + 8 * 96 + 48][3], 255); // u_u_u_u
        let (row, col) = source_cell([
            Knowledge::Visible,
            Knowledge::Visible,
            Knowledge::Unknown,
            Knowledge::Unknown,
        ]);
        assert!((0..48).any(|y| (0..96).any(|x| {
            let alpha = pixels[(row * 48 + y) * 864 + col * 96 + x][3];
            alpha > 0 && alpha < 255
        })));
    }
    #[test]
    fn knowledge_wraps_horizontally_but_does_not_sample_across_poles() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.ui_review_fog();
        let game = Game::from_simulation(sim);
        assert_eq!(game.fog_knowledge(20, 24), Knowledge::Visible);
        assert_eq!(game.fog_knowledge(20 + MAP_W, 24), Knowledge::Visible);
        assert_eq!(game.fog_knowledge(20, -1), Knowledge::Remembered);
        assert_eq!(game.fog_knowledge(20, MAP_H), Knowledge::Remembered);
    }
    #[test]
    fn atlas_isolates_sprites_and_never_samples_the_unused_credits_cell() {
        let source =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_fog.png"), None)
                .unwrap();
        let atlas = build_atlas(&source);
        assert_eq!((atlas.width(), atlas.height()), (882, 450));
        let src = source.get_image_data();
        let dst = atlas.get_image_data();
        for row in 0..9 {
            for col in 0..9 {
                for y in 0..50usize {
                    for x in 0..98usize {
                        let actual = dst[(row * 50 + y) * 882 + col * 98 + x];
                        if (row, col) == (4, 4) {
                            assert_eq!(actual[3], 0);
                        } else {
                            let sx = x.saturating_sub(1).min(95);
                            let sy = y.saturating_sub(1).min(47);
                            assert_eq!(actual, src[(row * 48 + sy) * 864 + col * 96 + sx]);
                            if actual[3] > 0 {
                                assert_eq!(&actual[..3], &[0, 0, 0]);
                            }
                        }
                    }
                }
            }
        }
    }
}
