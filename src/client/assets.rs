//! Loading and preparing textures, sprites, and other renderer assets.

use super::*;
use super::{
    asset_error::AssetResult,
    media::{AssetRoots, Civ3MediaResolver},
};
use std::sync::Arc;

/// One startup configuration snapshot shared by every asset consumer.
pub(crate) struct AssetContext {
    pub(super) media: Arc<Civ3MediaResolver>,
    #[cfg(feature = "desktop-audio")]
    pub(super) audio: Option<Arc<super::audio_assets::AudioLibrary>>,
}
impl AssetContext {
    pub(crate) fn from_env(audio_enabled: bool) -> AssetResult<Self> {
        Self::new(
            AssetRoots::from_env()?,
            audio_enabled,
            std::env::var_os("CIVCRAFT_MEDIA_TRACE").is_some(),
        )
    }
    pub(super) fn new(roots: AssetRoots, audio_enabled: bool, trace: bool) -> AssetResult<Self> {
        let media = Arc::new(Civ3MediaResolver::from_config(&roots).tracing(trace));
        #[cfg(feature = "desktop-audio")]
        let audio = if audio_enabled {
            match super::audio_assets::AudioLibrary::new(media.clone()) {
                Ok(library) => Some(Arc::new(library)),
                Err(error) => {
                    eprintln!("Could not configure audio: {error}");
                    None
                }
            }
        } else {
            None
        };
        #[cfg(not(feature = "desktop-audio"))]
        let _ = audio_enabled;
        Ok(Self {
            media,
            #[cfg(feature = "desktop-audio")]
            audio,
        })
    }
    pub(crate) fn select_tileset(&self, name: &str) -> AssetResult<Tileset> {
        Tileset::select(name, &self.media)
    }
}

/// Smallest window size, and the width-to-height ratio that every window size
/// keeps, in both the launch size and the user's resizes.
const WINDOW_MIN_SIZE: (i32, i32) = (1280, 720);
const WINDOW_ASPECT_RATIO: (i32, i32) = (16, 9);

pub(crate) fn window_conf() -> Conf {
    let args: Vec<_> = std::env::args().collect();
    let size = args
        .windows(2)
        .find(|a| a[0] == "--window-size")
        .and_then(|a| a[1].split_once('x'))
        .and_then(|(w, h)| Some((w.parse::<i32>().ok()?, h.parse::<i32>().ok()?)))
        .filter(|&(w, h)| {
            w >= WINDOW_MIN_SIZE.0
                && h >= WINDOW_MIN_SIZE.1
                && w * WINDOW_ASPECT_RATIO.1 == h * WINDOW_ASPECT_RATIO.0
        })
        .unwrap_or(WINDOW_MIN_SIZE);
    let conf = Conf {
        window_title: "CivCraft".to_owned(),
        window_width: size.0,
        window_height: size.1,
        window_resizable: true,
        window_min_size: Some(WINDOW_MIN_SIZE),
        window_aspect_ratio: Some(WINDOW_ASPECT_RATIO),
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    };
    // Miniquad defaults to the deprecated OpenGL backend on macOS. Its
    // NSOpenGLPixelFormat request can fail before the window is created on
    // current macOS display setups, while the supported Metal backend avoids
    // that context-creation path.
    #[cfg(target_os = "macos")]
    {
        let mut conf = conf;
        conf.platform.apple_gfx_api = macroquad::miniquad::conf::AppleGfxApi::Metal;
        // Miniquad's Metal render pipelines are single-sampled. The window
        // framebuffer has to match the pipeline's sample count.
        conf.sample_count = 1;
        conf
    }
    #[cfg(not(target_os = "macos"))]
    {
        conf
    }
}

pub(crate) struct Assets {
    pub(super) civ3: super::tileset::Civ3,
    pub(super) source_fog: Texture2D,
    pub(super) hut_sound: Option<std::path::PathBuf>,
    pub(super) bomb_sound: Option<std::path::PathBuf>,
    pub(super) sam_sound: Option<std::path::PathBuf>,
}

impl Assets {
    pub(crate) fn unit_sound_path(
        &self,
        kind: crate::sim::UnitKind,
        era: usize,
        action: &str,
    ) -> Option<std::path::PathBuf> {
        if action.eq_ignore_ascii_case("HUT") {
            return self.hut_sound.clone();
        }
        if action.eq_ignore_ascii_case("BOMB_EFFECT") {
            return self.bomb_sound.clone();
        }
        if action.eq_ignore_ascii_case("SAM_EFFECT") {
            return self.sam_sound.clone();
        }
        self.civ3
            .unit_sound_source(kind, era, action)
            .map(|source| source.path.clone())
    }

    pub(crate) fn new(tileset: &super::Tileset, context: &AssetContext) -> AssetResult<Self> {
        let civ3 = match tileset {
            super::Tileset::Civ3(path) => super::tileset::Civ3::load(path, &context.media)?,
        };
        let source_fog_image = super::tileset::load_source_fog_atlas(&context.media)?;
        let source_fog = Texture2D::from_image(&source_fog_image);
        // Match OpenCiv3's nearest-filter sampling of the source fog atlas.
        source_fog.set_filter(FilterMode::Nearest);
        #[cfg(feature = "desktop-audio")]
        let hut_sound = match context.audio.as_ref() {
            Some(library) => library.resolve_key("hut")?,
            None => context
                .media
                .lookup("Sounds/Hut.wav")?
                .map(|found| found.path),
        };
        #[cfg(not(feature = "desktop-audio"))]
        let hut_sound = context
            .media
            .lookup("Sounds/Hut.wav")?
            .map(|found| found.path);
        let bomb_sound = context
            .media
            .base_policy()
            .lookup("Art/Animations/Bombs/Bomb.wav")?
            .map(|found| found.path);
        let sam_sound = context
            .media
            .base_policy()
            .lookup("Art/Animations/SAM/SAMAttack.wav")?
            .map(|found| found.path);
        Ok(Self {
            civ3,
            source_fog,
            hut_sound,
            bomb_sound,
            sam_sound,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn injected_context_keeps_one_configuration_snapshot() {
        let root =
            std::env::temp_dir().join(format!("civcraft-asset-context-{}", std::process::id()));
        fs::create_dir_all(root.join("Conquests/Art")).unwrap();
        let roots = AssetRoots {
            install: Some(root.clone()),
            ..Default::default()
        };
        let context = AssetContext::new(roots, false, false).unwrap();
        let Tileset::Civ3(selected) = context.select_tileset("civ3").unwrap();
        assert_eq!(selected, root.join("Conquests/Art").canonicalize().unwrap());
        assert_eq!(context.media.resolve("Art").unwrap(), selected);
        #[cfg(feature = "desktop-audio")]
        {
            let package = root.join("package");
            fs::create_dir_all(&package).unwrap();
            let catalog = package.join("audio.toml");
            fs::write(
                &catalog,
                "[cues.click]\npath='click.wav'\nbus='ui'\nlooped=false\n",
            )
            .unwrap();
            let context = AssetContext::new(
                AssetRoots {
                    install: Some(root.clone()),
                    packaged: vec![package],
                    ..Default::default()
                },
                true,
                false,
            )
            .unwrap();
            let library = context.audio.as_ref().unwrap();
            assert!(Arc::ptr_eq(&context.media, &library.media));
            let loader = super::super::audio_assets::AudioAssetLoader::new(library.clone());
            assert!(Arc::ptr_eq(library, &loader.library));
            fs::write(&catalog, "broken TOML [").unwrap();
            // Consumers keep the parsed catalog; later disk edits cannot change it.
            assert_eq!(loader.library.cue("click").unwrap().path, "click.wav");
        }
        fs::remove_dir_all(root).unwrap();
    }
}
