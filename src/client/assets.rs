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
    pub(super) resource_icons: Texture2D,
    pub(super) leader_portraits: Vec<Vec<Texture2D>>,
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

    fn legacy_portrait_slot(civilization_id: &str) -> Option<usize> {
        match civilization_id {
            "rome" => Some(0),
            "greece" => Some(1),
            "egypt" => Some(2),
            "babylon" => Some(3),
            "china" => Some(4),
            "japan" => Some(5),
            _ => None,
        }
    }

    pub(super) fn leader_portrait(
        &self,
        civilization_id: Option<&str>,
        attitude: &str,
    ) -> Option<&Texture2D> {
        let slot = Self::legacy_portrait_slot(civilization_id?)?;
        let attitude_index = match attitude {
            "Furious" => 0,
            "Annoyed" => 1,
            "Cautious" => 2,
            "Polite" => 3,
            "Gracious" => 4,
            _ => 2,
        };
        self.leader_portraits.get(slot)?.get(attitude_index)
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
        let source_fog_image = super::tileset::load_source_fog_atlas()?;
        let source_fog = Texture2D::from_image(&source_fog_image);
        // Match OpenCiv3's nearest-filter sampling of the source fog atlas.
        source_fog.set_filter(FilterMode::Nearest);
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
        for portraits in &leader_portraits {
            for portrait in portraits {
                portrait.set_filter(FilterMode::Linear);
            }
        }
        Ok(Self {
            civ3,
            source_fog,
            resource_icons,
            leader_portraits,
        })
    }
}
