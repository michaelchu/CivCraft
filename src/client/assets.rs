//! Loading and preparing textures, sprites, and other renderer assets.

use super::*;

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
    pub(super) source_fog: Texture2D,
}

impl Assets {
    pub(crate) fn unit_sound_path(
        &self,
        kind: crate::sim::UnitKind,
        era: usize,
        action: &str,
    ) -> Option<std::path::PathBuf> {
        self.civ3
            .unit_sound_source(kind, era, action)
            .map(|source| source.path.clone())
    }

    pub(crate) fn new(tileset: &super::Tileset) -> Result<Self, String> {
        let civ3 = match tileset {
            super::Tileset::Civ3(path) => super::tileset::Civ3::load(path)?,
        };
        let source_fog_image = super::tileset::load_source_fog_atlas()?;
        let source_fog = Texture2D::from_image(&source_fog_image);
        // Match OpenCiv3's nearest-filter sampling of the source fog atlas.
        source_fog.set_filter(FilterMode::Nearest);
        Ok(Self {
            civ3,
            source_fog,
        })
    }
}
