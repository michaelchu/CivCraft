//! HUD icons: Lucide glyphs rasterized to white PNGs and tinted when drawn.

use super::*;
use std::{cell::RefCell, collections::HashMap};

#[derive(Clone, Copy)]
pub(in crate::client) enum HudIcon {
    Music,
    MusicMuted,
    Government,
    Religion,
    Diplomacy,
    Research,
    Science,
    Tax,
    Luxury,
    Spaceship,
    City,
    Road,
    Water,
    Pickaxe,
    Wheat,
    Tree,
    Cycle,
    Broom,
    Shield,
    Burst,
    Home,
    Gear,
    Pause,
    Upgrade,
    Target,
    Parachute,
    Plane,
    Boat,
    Wonder,
    Cross,
    More,
}

macro_rules! lucide {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../data/presentation/icons/lucide/",
            $name,
            ".png"
        ))
        .as_slice()
    };
}

/// The embedded PNG for `icon`. Lucide has no parachute, so paradrop uses an
/// arrow dropping onto a point.
fn icon_png(icon: HudIcon) -> &'static [u8] {
    match icon {
        HudIcon::Music => lucide!("volume-2"),
        HudIcon::MusicMuted => lucide!("volume-x"),
        HudIcon::Government => lucide!("landmark"),
        HudIcon::Religion => lucide!("sparkles"),
        HudIcon::Diplomacy => lucide!("messages-square"),
        HudIcon::Research => lucide!("book-open"),
        HudIcon::Science => lucide!("flask-conical"),
        HudIcon::Tax => lucide!("coins"),
        HudIcon::Luxury => lucide!("gem"),
        HudIcon::Spaceship => lucide!("rocket"),
        HudIcon::City => lucide!("building"),
        HudIcon::Road => lucide!("route"),
        HudIcon::Water => lucide!("droplet"),
        HudIcon::Pickaxe => lucide!("pickaxe"),
        HudIcon::Wheat => lucide!("wheat"),
        HudIcon::Tree => lucide!("tree-pine"),
        HudIcon::Cycle => lucide!("refresh-cw"),
        HudIcon::Broom => lucide!("brush-cleaning"),
        HudIcon::Shield => lucide!("shield"),
        HudIcon::Burst => lucide!("bomb"),
        HudIcon::Home => lucide!("house"),
        HudIcon::Gear => lucide!("settings"),
        HudIcon::Pause => lucide!("pause"),
        HudIcon::Upgrade => lucide!("arrow-big-up"),
        HudIcon::Target => lucide!("crosshair"),
        HudIcon::Parachute => lucide!("arrow-down-to-dot"),
        HudIcon::Plane => lucide!("plane"),
        HudIcon::Boat => lucide!("ship"),
        HudIcon::Wonder => lucide!("sparkles"),
        HudIcon::Cross => lucide!("x"),
        HudIcon::More => lucide!("ellipsis"),
    }
}

thread_local! {
    static TEXTURES: RefCell<HashMap<usize, Texture2D>> = RefCell::new(HashMap::new());
}

fn icon_texture(icon: HudIcon) -> Texture2D {
    TEXTURES.with(|cache| {
        cache
            .borrow_mut()
            .entry(icon as usize)
            .or_insert_with(|| {
                let image = Image::from_file_with_format(icon_png(icon), Some(ImageFormat::Png))
                    .expect("embedded icon PNGs are valid");
                let texture = Texture2D::from_image(&image);
                texture.set_filter(FilterMode::Linear);
                texture
            })
            .clone()
    })
}

/// Draws `icon` centered on `center`, `size` pixels square, tinted `color`.
/// Lucide leaves a 2px margin on its 24px grid; the draw box is enlarged to
/// make the glyph itself about `size` across.
pub(in crate::client) fn draw_hud_icon(icon: HudIcon, center: Vec2, size: f32, color: Color) {
    let side = size * 24.0 / 20.0;
    draw_texture_ex(
        &icon_texture(icon),
        center.x - side * 0.5,
        center.y - side * 0.5,
        color,
        DrawTextureParams {
            dest_size: Some(vec2(side, side)),
            ..Default::default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant; the exhaustive match in `icon_png` keeps this honest.
    const ALL: [HudIcon; 30] = [
        HudIcon::Music,
        HudIcon::MusicMuted,
        HudIcon::Government,
        HudIcon::Diplomacy,
        HudIcon::Research,
        HudIcon::Science,
        HudIcon::Tax,
        HudIcon::Luxury,
        HudIcon::Spaceship,
        HudIcon::City,
        HudIcon::Road,
        HudIcon::Water,
        HudIcon::Pickaxe,
        HudIcon::Wheat,
        HudIcon::Tree,
        HudIcon::Cycle,
        HudIcon::Broom,
        HudIcon::Shield,
        HudIcon::Burst,
        HudIcon::Home,
        HudIcon::Gear,
        HudIcon::Pause,
        HudIcon::Upgrade,
        HudIcon::Target,
        HudIcon::Parachute,
        HudIcon::Plane,
        HudIcon::Boat,
        HudIcon::Wonder,
        HudIcon::Cross,
        HudIcon::More,
    ];

    #[test]
    fn every_icon_is_a_white_stroke_on_a_transparent_96px_square() {
        for icon in ALL {
            let image =
                image::load_from_memory_with_format(icon_png(icon), image::ImageFormat::Png)
                    .unwrap_or_else(|error| panic!("{}: {error}", icon as usize))
                    .to_rgba8();
            assert_eq!(image.dimensions(), (96, 96), "icon {}", icon as usize);
            assert_eq!(image.get_pixel(0, 0).0[3], 0, "icon {}", icon as usize);
            let solid: Vec<_> = image.pixels().filter(|p| p.0[3] > 200).collect();
            assert!(!solid.is_empty(), "icon {} has no stroke", icon as usize);
            assert!(
                solid.iter().all(|p| p.0[..3].iter().all(|&c| c >= 240)),
                "icon {} is not white",
                icon as usize
            );
        }
    }
}
