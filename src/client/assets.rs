//! Loading and preparing textures, sprites, and other renderer assets.

use super::sprite_metadata::*;
use super::*;

// Give each sprite its own duplicated edge pixels, as if loaded separately.
// Source sheets can contain opaque grid lines immediately outside the cells.
fn isolated_atlas(
    source: &Image,
    width: usize,
    height: usize,
    cols: usize,
    rows: usize,
    bordered: bool,
) -> Image {
    let gap = usize::from(bordered);
    isolated_atlas_at(source, width, height, cols, rows, (gap, gap), gap)
}

fn isolated_atlas_at(
    source: &Image,
    width: usize,
    height: usize,
    cols: usize,
    rows: usize,
    origin: (usize, usize),
    gap: usize,
) -> Image {
    let mut atlas = Image::gen_image_color(
        (cols * (width + 2)) as u16,
        (rows * (height + 2)) as u16,
        BLANK,
    );
    let stride = atlas.width();
    let src = source.get_image_data();
    let dst = atlas.get_image_data_mut();
    for row in 0..rows {
        for col in 0..cols {
            for y in 0..height + 2 {
                for x in 0..width + 2 {
                    let sx = origin.0 + col * (width + gap) + x.saturating_sub(1).min(width - 1);
                    let sy = origin.1 + row * (height + gap) + y.saturating_sub(1).min(height - 1);
                    dst[(row * (height + 2) + y) * stride + col * (width + 2) + x] =
                        src[sy * source.width() + sx];
                }
            }
        }
    }
    atlas
}

pub(crate) fn window_conf() -> Conf {
    let args: Vec<_> = std::env::args().collect();
    let size = args
        .windows(2)
        .find(|a| a[0] == "--window-size")
        .and_then(|a| a[1].split_once('x'))
        .and_then(|(w, h)| Some((w.parse::<i32>().ok()?, h.parse::<i32>().ok()?)))
        .filter(|&(w, h)| w >= 800 && h >= 600)
        .unwrap_or((1280, 720));
    Conf {
        window_title: "CivCraft".to_owned(),
        window_width: size.0,
        window_height: size.1,
        window_resizable: true,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

pub(crate) struct Assets {
    pub(super) civ3: super::tileset::Civ3,
    pub(super) fog: Texture2D,
    pub(super) resource_icons: Texture2D,
    // The legacy unit atlas remains for compact non-map UI icons only.
    pub(super) units: Texture2D,
    pub(super) leader_avatar_badges: Vec<Texture2D>,
    pub(super) leader_portraits: Vec<Vec<Texture2D>>,
}

impl Assets {
    pub(super) fn leader_portrait(&self, owner: usize, attitude: &str) -> Option<&Texture2D> {
        let attitude_index = match attitude {
            "Furious" => 0,
            "Annoyed" => 1,
            "Cautious" => 2,
            "Polite" => 3,
            "Gracious" => 4,
            _ => 2,
        };
        self.leader_portraits.get(owner)?.get(attitude_index)
    }

    pub(crate) fn new(tileset: &super::Tileset) -> Result<Self, String> {
        let civ3 = match tileset {
            super::Tileset::Civ3(path) => super::tileset::Civ3::load(path)?,
        };
        let resource_icon_image = Image::from_file_with_format(
            include_bytes!("../../assets/civ3_resources_generated.png"),
            None,
        )
        .expect("valid generated resource icon sheet");
        let resource_icons = Texture2D::from_image(&resource_icon_image);
        resource_icons.set_filter(FilterMode::Linear);
        let fog_image = Image::from_file_with_format(
            include_bytes!("../../assets/amplio2_fog.png"),
            None,
        )
        .expect("valid Amplio2 fog atlas");
        let fog = Texture2D::from_image(&crate::client::fog::build_atlas(&fog_image));
        // Fog cells have isolated gutters, so linear sampling can soften the
        // transition without bleeding a neighboring atlas state.
        fog.set_filter(FilterMode::Linear);
        let units_image =
            Image::from_file_with_format(include_bytes!("../../assets/amplio2_units.png"), None)
                .expect("valid unit UI atlas");
        let units = Texture2D::from_image(&isolated_atlas(&units_image, 64, 48, 20, 3, true));
        units.set_filter(FilterMode::Nearest);
        let leader_portraits = [
            [
                include_bytes!("../../assets/leader_portraits/rome/furious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/rome/annoyed.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/rome/cautious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/rome/polite.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/rome/gracious.png").as_slice(),
            ],
            [
                include_bytes!("../../assets/leader_portraits/greece/furious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/greece/annoyed.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/greece/cautious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/greece/polite.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/greece/gracious.png").as_slice(),
            ],
            [
                include_bytes!("../../assets/leader_portraits/egypt/furious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/egypt/annoyed.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/egypt/cautious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/egypt/polite.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/egypt/gracious.png").as_slice(),
            ],
            [
                include_bytes!("../../assets/leader_portraits/babylon/furious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/babylon/annoyed.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/babylon/cautious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/babylon/polite.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/babylon/gracious.png").as_slice(),
            ],
            [
                include_bytes!("../../assets/leader_portraits/china/furious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/china/annoyed.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/china/cautious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/china/polite.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/china/gracious.png").as_slice(),
            ],
            [
                include_bytes!("../../assets/leader_portraits/japan/furious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/japan/annoyed.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/japan/cautious.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/japan/polite.png").as_slice(),
                include_bytes!("../../assets/leader_portraits/japan/gracious.png").as_slice(),
            ],
        ]
        .into_iter()
        .map(|portraits| {
            portraits
                .into_iter()
                .map(|bytes| Texture2D::from_file_with_format(bytes, None))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
        let leader_avatar_badges = [
            include_bytes!("../../assets/leader_avatars/rome.png").as_slice(),
            include_bytes!("../../assets/leader_avatars/greece.png").as_slice(),
            include_bytes!("../../assets/leader_avatars/egypt.png").as_slice(),
            include_bytes!("../../assets/leader_avatars/babylon.png").as_slice(),
            include_bytes!("../../assets/leader_avatars/china.png").as_slice(),
            include_bytes!("../../assets/leader_avatars/japan.png").as_slice(),
        ]
        .into_iter()
        .map(|bytes| Texture2D::from_file_with_format(bytes, None))
        .collect::<Vec<_>>();
        for badge in &leader_avatar_badges {
            badge.set_filter(FilterMode::Linear);
        }
        for portraits in &leader_portraits {
            for portrait in portraits {
                portrait.set_filter(FilterMode::Linear);
            }
        }
        Ok(Self {
            civ3,
            fog,
            resource_icons,
            units,
            leader_avatar_badges,
            leader_portraits,
        })
    }
}

pub(crate) fn native_unit_rendering_playtest() -> (bool, bool, bool) {
    let atlas =
        Image::from_file_with_format(include_bytes!("../../assets/amplio2_units.png"), None)
            .expect("valid native unit atlas");
    let pixels = atlas.get_image_data();
    let visible = |kind| {
        let (row, col) = unit_sprite(kind);
        let source = bordered_sprite_source(row, col, 64.0, 48.0);
        let (x, y) = (source.x as usize, source.y as usize);
        x + 64 <= atlas.width()
            && y + 48 <= atlas.height()
            && (y..y + 48)
                .any(|row| (x..x + 64).any(|col| pixels[row * atlas.width() + col][3] != 0))
    };
    let added = [
        UnitKind::Pikemen,
        UnitKind::Riflemen,
        UnitKind::AlpineTroops,
        UnitKind::Fanatics,
        UnitKind::MechInf,
        UnitKind::Elephants,
        UnitKind::Crusaders,
        UnitKind::Dragoons,
        UnitKind::Cavalry,
        UnitKind::Armor,
        UnitKind::Artillery,
        UnitKind::Howitzer,
        UnitKind::Leader,
        UnitKind::Awacs,
        UnitKind::Storm,
    ];
    let complete =
        UnitKind::ALL_NATIVE.len() == 66 && UnitKind::ALL_NATIVE.into_iter().all(visible);
    let distinct = added
        .into_iter()
        .map(unit_sprite)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        == 15;
    let labels = added
        .into_iter()
        .all(|kind| !short_unit_name(kind).is_empty())
        && short_unit_name(UnitKind::Awacs) == "AWACS"
        && short_unit_name(UnitKind::MechInf) == "Mech. Inf.";
    (complete, distinct, labels)
}

#[cfg(test)]
#[path = "assets_tests.rs"]
mod native_unit_tests;
