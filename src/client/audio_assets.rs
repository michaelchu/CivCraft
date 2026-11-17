//! Key-based audio lookup and shared decoding. Playback volume belongs to players.
use super::media::Civ3MediaResolver;
use macroquad::audio::{Sound, load_sound_from_bytes};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    io::Cursor,
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
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    cues: HashMap<String, AudioCue>,
}
#[derive(Clone)]
pub(super) struct AudioLibrary {
    pub media: Civ3MediaResolver,
    cues: HashMap<String, AudioCue>,
    defaults: HashMap<String, String>,
}
impl AudioLibrary {
    pub fn new(media: Civ3MediaResolver) -> Result<Self, String> {
        let defaults: Catalog = toml::from_str(include_str!("../../data/civ3_audio.toml"))
            .map_err(|e| e.to_string())?;
        let paths = defaults
            .cues
            .iter()
            .map(|(key, cue)| (cue.path.clone(), key.clone()))
            .collect();
        let mut cues = defaults.cues;
        // Lowest precedence first so the first configured root wins by key.
        let roots: Vec<_> = media.catalog_roots().collect();
        for root in roots.into_iter().rev() {
            if let Some(file) = Civ3MediaResolver::new(vec![root.to_owned()])
                .lookup("audio.toml")
                .map_err(|e| e.to_string())?
            {
                let catalog: Catalog = toml::from_str(
                    &fs::read_to_string(&file.path)
                        .map_err(|e| format!("{}: {e}", file.path.display()))?,
                )
                .map_err(|e| format!("{}: {e}", file.path.display()))?;
                cues.extend(catalog.cues);
            }
        }
        Ok(Self {
            media,
            cues,
            defaults: paths,
        })
    }
    pub fn configured() -> Result<Self, String> {
        Self::new(Civ3MediaResolver::configured()?)
    }
    pub fn cue(&self, key: &str) -> Result<&AudioCue, String> {
        self.cues
            .get(key)
            .ok_or_else(|| format!("Unknown audio key '{key}'"))
    }
    pub fn resolve_key(&self, key: &str) -> Result<Option<PathBuf>, String> {
        let cue = self.cue(key)?;
        let found = self
            .media
            .lookup(&cue.path)
            .map_err(|e| format!("Audio '{key}' ({}): {e}", cue.path))?;
        match found {
            Some(found) => Ok(Some(found.path)),
            None if cue.required => {
                Err(format!("Audio '{key}': {}", self.media.missing(&cue.path)))
            }
            None => Ok(None),
        }
    }
    pub fn resolve_default(&self, path: &str) -> Result<Option<PathBuf>, String> {
        if let Some(key) = self.defaults.get(path) {
            self.resolve_key(key)
        } else {
            Ok(self
                .media
                .lookup(path)
                .map_err(|e| format!("Audio '{path}': {e}"))?
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
    pub library: AudioLibrary,
    decoded: HashMap<PathBuf, Sound>,
}
impl AudioAssetLoader {
    pub fn new(library: AudioLibrary) -> Self {
        Self {
            library,
            decoded: HashMap::new(),
        }
    }
    /// Key metadata is returned so the caller selects its bus and loop policy.
    #[allow(dead_code)] // UI and ambience callers can use the same catalog without new player policy.
    pub async fn load(&mut self, key: &str) -> Result<Option<(Sound, AudioBus, bool)>, String> {
        let cue = self.library.cue(key)?.clone();
        let Some(path) = self.library.resolve_key(key)? else {
            return Ok(None);
        };
        let sound = self
            .load_path(&path)
            .await
            .map_err(|e| format!("Audio '{key}': {e}"))?;
        Ok(Some((sound, cue.bus, cue.looped)))
    }
    pub async fn load_path(&mut self, path: &Path) -> Result<Sound, String> {
        let path = self.library.media.existing(path)?;
        if let Some(sound) = self.decoded.get(&path) {
            return Ok(sound.clone());
        }
        let result = async {
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            let wav = decode_to_wav(&path, &bytes)?;
            load_sound_from_bytes(&wav).await.map_err(|e| e.to_string())
        }
        .await
        .map_err(|e: String| format!("{}: {e}", path.display()))?;
        self.decoded.insert(path, result.clone());
        Ok(result)
    }
}
pub(super) fn decode_to_wav(path: &Path, bytes: &[u8]) -> Result<Vec<u8>, String> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp3" => super::audio::decode_mp3_to_wav(bytes),
        "wav" => {
            validate_wav(bytes)?;
            Ok(bytes.to_vec())
        }
        "ogg" | "oga" => {
            let mut reader = lewton::inside_ogg::OggStreamReader::new(Cursor::new(bytes))
                .map_err(|e| e.to_string())?;
            let rate = reader.ident_hdr.audio_sample_rate;
            let channels = u16::from(reader.ident_hdr.audio_channels);
            let mut samples = Vec::new();
            while let Some(packet) = reader.read_dec_packet_itl().map_err(|e| e.to_string())? {
                samples.extend(packet);
            }
            pcm_wav(rate, channels, &samples)
        }
        other => Err(format!("Unsupported audio format '{other}'")),
    }
}
fn validate_wav(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("Invalid WAV header".into());
    }
    let end = (u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize)
        .checked_add(8)
        .filter(|&end| end <= bytes.len())
        .ok_or("Truncated WAV container")?;
    let mut position = 12usize;
    let mut format = false;
    let mut data = false;
    while position < end {
        if end - position < 8 {
            return Err("Truncated WAV chunk".into());
        }
        let size =
            u32::from_le_bytes(bytes[position + 4..position + 8].try_into().unwrap()) as usize;
        let next = position
            .checked_add(8)
            .and_then(|p| p.checked_add(size))
            .filter(|&p| p <= end)
            .ok_or("Truncated WAV chunk data")?;
        match &bytes[position..position + 4] {
            b"fmt " => {
                if size < 16 {
                    return Err("Invalid WAV format chunk".into());
                }
                format = true;
            }
            b"data" => {
                if size == 0 {
                    return Err("Empty WAV data".into());
                }
                data = true;
            }
            _ => {}
        }
        position = next
            .checked_add(size % 2)
            .ok_or("WAV chunk size overflow")?;
    }
    if !format || !data {
        return Err("WAV requires format and data chunks".into());
    }
    Ok(())
}
fn pcm_wav(rate: u32, channels: u16, samples: &[i16]) -> Result<Vec<u8>, String> {
    if samples.is_empty()
        || rate == 0
        || channels == 0
        || !samples.len().is_multiple_of(usize::from(channels))
    {
        return Err("Invalid PCM audio dimensions".into());
    }
    let size = u32::try_from(samples.len().checked_mul(2).ok_or("PCM data too large")?)
        .map_err(|_| "PCM data too large")?;
    let riff = size.checked_add(36).ok_or("PCM data too large")?;
    let align = channels.checked_mul(2).ok_or("Too many channels")?;
    let byte_rate = rate
        .checked_mul(u32::from(align))
        .ok_or("Invalid PCM byte rate")?;
    let mut wav = Vec::with_capacity(44 + size as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&align.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&size.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(wav)
}

#[cfg(test)]
mod tests {
    use super::super::media::AssetRoots;
    use super::*;
    #[test]
    fn decode_supported_formats_and_reject_malformed_audio() {
        let pcm = pcm_wav(8000, 1, &[0, 100, -100, 0]).unwrap();
        assert_eq!(decode_to_wav(Path::new("tone.WAV"), &pcm).unwrap(), pcm);
        for (name, bytes) in [
            (
                "tone.mp3",
                include_bytes!("../../tests/fixtures/audio/tone.mp3").as_slice(),
            ),
            (
                "tone.ogg",
                include_bytes!("../../tests/fixtures/audio/tone.ogg").as_slice(),
            ),
            (
                "tone.oga",
                include_bytes!("../../tests/fixtures/audio/tone.ogg").as_slice(),
            ),
        ] {
            let wav = decode_to_wav(Path::new(name), bytes).unwrap();
            assert_eq!(&wav[..4], b"RIFF");
            assert!(wav.len() > 44);
        }
        assert!(decode_to_wav(Path::new("truncated.wav"), &pcm[..pcm.len() - 1]).is_err());
        for name in ["bad.wav", "bad.mp3", "bad.ogg", "bad.oga", "bad.xyz"] {
            assert!(
                decode_to_wav(Path::new(name), b"malformed").is_err(),
                "{name}"
            );
        }
    }
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
        let library = AudioLibrary::new(Civ3MediaResolver::from_config(&AssetRoots {
            scenarios: vec![scenario.clone()],
            packaged: vec![package],
            ..Default::default()
        }))
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
            library
                .resolve_key("needed")
                .unwrap_err()
                .contains("needed")
        );
        fs::write(scenario.join("audio.toml"), "invalid TOML [").unwrap();
        assert!(AudioLibrary::new(library.media).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
