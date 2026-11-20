//! Key-based audio lookup and shared decoding. Playback volume belongs to players.
use super::asset_error::{AssetError, AssetResult};
use super::media::Civ3MediaResolver;
use macroquad::audio::{Sound, load_sound_from_bytes};
use serde::Deserialize;
use std::sync::Arc;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum AudioBus {
    Music,
    Sfx,
    Ui,
    Ambience,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AudioCue {
    pub path: String,
    pub bus: AudioBus,
    pub looped: bool,
    #[serde(default)]
    pub required: bool,
}
// Built-in campaign mappings are code; only optional scenario/package overrides use TOML.
const CAMPAIGN_AUDIO: &[(&str, &str)] = &[
    ("campaign.ancient.NA", "Sounds/build/ancient/AncNAfull.mp3"),
    ("campaign.ancient.EC", "Sounds/build/ancient/AncECfull.mp3"),
    ("campaign.ancient.GR", "Sounds/build/ancient/AncGRfull.mp3"),
    ("campaign.ancient.ME", "Sounds/build/ancient/AncMEFull.mp3"),
    ("campaign.ancient.OR", "Sounds/build/ancient/AncORfull.mp3"),
    (
        "campaign.middle.NA",
        "Sounds/build/Middle Ages/MidNAFull.mp3",
    ),
    (
        "campaign.middle.EC",
        "Sounds/build/Middle Ages/MidECFull.mp3",
    ),
    (
        "campaign.middle.GR",
        "Sounds/build/Middle Ages/MidGRFull.mp3",
    ),
    (
        "campaign.middle.ME",
        "Sounds/build/Middle Ages/MidMEFull.mp3",
    ),
    (
        "campaign.middle.OR",
        "Sounds/build/Middle Ages/MidORFull.mp3",
    ),
    (
        "campaign.industrial.NA",
        "Sounds/build/IndModern/IndNAFull.mp3",
    ),
    (
        "campaign.industrial.EC",
        "Sounds/build/IndModern/IndECFull.mp3",
    ),
    (
        "campaign.industrial.GR",
        "Sounds/build/IndModern/IndGRFull.mp3",
    ),
    (
        "campaign.industrial.ME",
        "Sounds/build/IndModern/IndMEFull.mp3",
    ),
    (
        "campaign.industrial.OR",
        "Sounds/build/IndModern/IndORFull.mp3",
    ),
    ("campaign.modern.0", "Sounds/build/IndModern/SmashFull.mp3"),
    ("campaign.modern.1", "Sounds/build/IndModern/StarsFull.mp3"),
    (
        "campaign.modern.2",
        "Sounds/build/IndModern/Techno MixFull.mp3",
    ),
];

fn default_cues() -> HashMap<String, AudioCue> {
    CAMPAIGN_AUDIO
        .iter()
        .map(|&(key, path)| {
            (
                key.to_owned(),
                AudioCue {
                    path: path.to_owned(),
                    bus: AudioBus::Music,
                    looped: true,
                    required: false,
                },
            )
        })
        .collect()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    cues: HashMap<String, AudioCue>,
}
#[derive(Clone)]
pub(super) struct AudioLibrary {
    pub media: Arc<Civ3MediaResolver>,
    cues: HashMap<String, AudioCue>,
    defaults: HashMap<String, String>,
}
impl AudioLibrary {
    pub fn new(media: Arc<Civ3MediaResolver>) -> AssetResult<Self> {
        let mut cues = default_cues();
        let paths = cues
            .iter()
            .map(|(key, cue)| (cue.path.clone(), key.clone()))
            .collect();
        // Lowest precedence first so the first configured root wins by key.
        let roots: Vec<_> = media.catalog_roots().collect();
        for root in roots.into_iter().rev() {
            if let Some(file) =
                Civ3MediaResolver::new(vec![root.to_owned()]).lookup("audio.toml")?
            {
                let catalog: Catalog = toml::from_str(
                    &fs::read_to_string(&file.path)
                        .map_err(|e| AssetError::io("read audio catalog", &file.path, e))?,
                )
                .map_err(|source| AssetError::Configuration {
                    path: file.path.clone(),
                    source,
                })?;
                cues.extend(catalog.cues);
            }
        }
        Ok(Self {
            media,
            cues,
            defaults: paths,
        })
    }
    pub fn cue(&self, key: &str) -> AssetResult<&AudioCue> {
        self.cues.get(key).ok_or_else(|| AssetError::UnknownCue {
            key: key.to_owned(),
        })
    }
    pub fn resolve_key(&self, key: &str) -> AssetResult<Option<PathBuf>> {
        let cue = self.cue(key)?;
        let found = self
            .media
            .lookup(&cue.path)
            .map_err(|e| e.context(format!("Audio '{key}' ({})", cue.path)))?;
        match found {
            Some(found) => Ok(Some(found.path)),
            None if cue.required => Err(self
                .media
                .missing(&cue.path)
                .context(format!("Audio '{key}'"))),
            None => Ok(None),
        }
    }
    pub fn resolve_default(&self, path: &str) -> AssetResult<Option<PathBuf>> {
        if let Some(key) = self.defaults.get(path) {
            self.resolve_key(key)
        } else {
            Ok(self
                .media
                .lookup(path)
                .map_err(|e| e.context(format!("Audio '{path}'")))?
                .map(|p| p.path))
        }
    }
    pub fn default_looped(&self, path: &str) -> bool {
        self.defaults
            .get(path)
            .and_then(|key| self.cues.get(key))
            .is_some_and(|cue| cue.looped)
    }
}

pub(super) struct AudioAssetLoader {
    pub library: Arc<AudioLibrary>,
    decoded: HashMap<PathBuf, Sound>,
}
impl AudioAssetLoader {
    pub fn new(library: Arc<AudioLibrary>) -> Self {
        Self {
            library,
            decoded: HashMap::new(),
        }
    }
    /// Key metadata is returned so the caller selects its bus and loop policy.
    #[allow(dead_code)] // UI and ambience callers can use the same catalog without new player policy.
    pub async fn load(&mut self, key: &str) -> AssetResult<Option<(Sound, AudioBus, bool)>> {
        let cue = self.library.cue(key)?.clone();
        let Some(path) = self.library.resolve_key(key)? else {
            return Ok(None);
        };
        let sound = self
            .load_path(&path)
            .await
            .map_err(|e| e.context(format!("Audio '{key}'")))?;
        Ok(Some((sound, cue.bus, cue.looped)))
    }
    pub async fn load_path(&mut self, path: &Path) -> AssetResult<Sound> {
        // Resolved media paths have already passed boundary validation. Cached
        // sounds need no filesystem access on subsequent playback.
        if let Some(sound) = self.decoded.get(path) {
            return Ok(sound.clone());
        }
        let path = self.library.media.existing(path)?;
        if let Some(sound) = self.decoded.get(&path) {
            return Ok(sound.clone());
        }
        let bytes = fs::read(&path).map_err(|e| AssetError::io("read audio", &path, e))?;
        let wav = super::audio_codecs::decode_to_wav(&path, &bytes)?;
        let result = load_sound_from_bytes(&wav)
            .await
            .map_err(|e| AssetError::decode(&path, e))?;
        self.decoded.insert(path, result.clone());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::super::media::AssetRoots;
    use super::*;
    #[test]
    fn catalog_override_keys_metadata_and_missing_policy() {
        let root =
            std::env::temp_dir().join(format!("civcraft-audio-catalog-{}", std::process::id()));
        let package = root.join("package");
        let scenario = root.join("scenario");
        fs::create_dir_all(&package).unwrap();
        fs::create_dir_all(&scenario).unwrap();
        fs::write(package.join("audio.toml"),"[cues.\\\"campaign.ancient.GR\\\"]\npath=\\\"package.wav\\\"\nbus=\\\"music\\\"\nlooped=true\n".replace("\\\"","\"")).unwrap();
        fs::write(
            scenario.join("audio.toml"),
            r#"
[cues."campaign.ancient.GR"]
path="scenario.ogg"
bus="music"
looped=false
[cues.click]
path="absent.wav"
bus="ui"
looped=false
[cues.needed]
path="absent.wav"
bus="sfx"
looped=false
required=true
"#,
        )
        .unwrap();
        fs::write(scenario.join("scenario.ogg"), "fixture").unwrap();
        let library = AudioLibrary::new(Arc::new(Civ3MediaResolver::from_config(&AssetRoots {
            scenarios: vec![scenario.clone()],
            packaged: vec![package],
            ..Default::default()
        })))
        .unwrap();
        assert_eq!(
            library
                .resolve_default("Sounds/build/ancient/AncGRfull.mp3")
                .unwrap()
                .unwrap(),
            scenario.join("scenario.ogg").canonicalize().unwrap()
        );
        assert!(!library.default_looped("Sounds/build/ancient/AncGRfull.mp3"));
        assert_eq!(library.cue("click").unwrap().bus, AudioBus::Ui);
        assert!(library.resolve_key("click").unwrap().is_none());
        assert!(
            matches!(library.cue("unknown"),Err(AssetError::UnknownCue {key}) if key=="unknown")
        );
        assert!(
            matches!(library.resolve_key("needed"),Err(AssetError::Context {source,..}) if matches!(&*source,AssetError::Missing {..}))
        );

        assert!(
            library
                .resolve_key("needed")
                .unwrap_err()
                .to_string()
                .contains("needed")
        );
        fs::write(scenario.join("audio.toml"), "invalid TOML [").unwrap();
        assert!(matches!(
            AudioLibrary::new(library.media),
            Err(AssetError::Configuration { .. })
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
