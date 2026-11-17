//! Unit sound lookup and playback, kept separate from simulation and drawing.

use super::amb::read_amb_cues;
use super::audio_assets::{AudioAssetLoader, AudioLibrary};
use super::media::Civ3MediaResolver;
use super::*;
use macroquad::audio::{PlaySoundParams, play_sound};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub(crate) struct UnitSoundRequest {
    pub(crate) unit_id: u32,
    pub(crate) kind: UnitKind,
    pub(crate) era_index: usize,
    pub(crate) action: String,
    pub(crate) delay_seconds: f64,
}

struct ScheduledCue {
    at: f64,
    path: PathBuf,
}

/// Owns decoded one-shot sounds and scheduled cues for unit actions.
pub(crate) struct UnitSoundPlayer {
    enabled: bool,
    loader: Option<AudioAssetLoader>,
    schedule: VecDeque<ScheduledCue>,
    component_index: Option<HashMap<String, PathBuf>>,
}

impl UnitSoundPlayer {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            loader: if enabled {
                match AudioLibrary::configured() {
                    Ok(library) => Some(AudioAssetLoader::new(library)),
                    Err(error) => {
                        eprintln!("Could not configure unit audio: {error}");
                        None
                    }
                }
            } else {
                None
            },
            schedule: VecDeque::new(),
            component_index: None,
        }
    }

    pub(crate) async fn update(
        &mut self,
        paths: impl IntoIterator<Item = (PathBuf, f64)>,
        muted: bool,
        volume: u8,
    ) {
        if !self.enabled || self.loader.is_none() {
            return;
        }
        if muted {
            self.schedule.clear();
            return;
        }
        let now = get_time();
        for (path, delay_seconds) in paths {
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("amb"))
            {
                match read_amb_cues(&path) {
                    Ok(cues) => {
                        for (delay, filename) in cues {
                            match resolve_amb_component(
                                &self.loader.as_ref().unwrap().library.media,
                                &path,
                                &filename,
                                &mut self.component_index,
                            ) {
                                Ok(Some(wav)) => self.schedule.push_back(ScheduledCue {
                                    at: now + delay_seconds.max(0.0) + delay,
                                    path: wav,
                                }),
                                Ok(None) => eprintln!(
                                    "Unit sound {} references missing WAV '{filename}'",
                                    path.display()
                                ),
                                Err(error) => eprintln!("Could not resolve unit cue: {error}"),
                            }
                        }
                    }
                    Err(error) => {
                        eprintln!("Could not parse unit sound {}: {error}", path.display())
                    }
                }
            } else {
                self.schedule.push_back(ScheduledCue {
                    at: now + delay_seconds.max(0.0),
                    path,
                });
            }
        }
        let mut cues = self.schedule.drain(..).collect::<Vec<_>>();
        cues.sort_by(|a, b| a.at.total_cmp(&b.at));
        self.schedule = cues.into();
        while self.schedule.front().is_some_and(|cue| cue.at <= now) {
            let cue = self.schedule.pop_front().unwrap();
            match self.loader.as_mut().unwrap().load_path(&cue.path).await {
                Ok(sound) => play_sound(
                    &sound,
                    PlaySoundParams {
                        looped: false,
                        volume: f32::from(volume.min(100)) / 100.0,
                    },
                ),
                Err(error) => eprintln!("Could not load unit sound: {error}"),
            }
        }
    }
}

fn resolve_amb_component(
    media: &Civ3MediaResolver,
    amb_path: &Path,
    reference: &str,
    index: &mut Option<HashMap<String, PathBuf>>,
) -> Result<Option<PathBuf>, String> {
    if let Some(found) = media
        .unit_lookup(amb_path, reference)
        .map_err(|e| format!("{}: {e}", amb_path.display()))?
    {
        return Ok(Some(found.path));
    }
    if index.is_none() {
        *index = Some(build_unit_audio_index(media)?);
    }
    Ok(index
        .as_ref()
        .and_then(|index| index.get(&reference.to_ascii_lowercase()).cloned()))
}
fn build_unit_audio_index(media: &Civ3MediaResolver) -> Result<HashMap<String, PathBuf>, String> {
    fn visit(
        directory: &Path,
        files: &mut HashMap<String, PathBuf>,
        visited: &mut std::collections::HashSet<PathBuf>,
        media: &Civ3MediaResolver,
    ) -> Result<(), String> {
        let directory = match Civ3MediaResolver::new(vec![directory.to_owned()])
            .lookup(".")
            .map_err(|e| e.to_string())?
        {
            Some(found) => found.path,
            None => return Ok(()),
        };
        if !visited.insert(directory.clone()) {
            return Ok(());
        }
        media.existing(&directory)?;
        let mut entries = fs::read_dir(&directory)
            .map_err(|e| format!("{}: {e}", directory.display()))?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        entries.sort();
        for path in entries {
            if path.is_dir() {
                visit(&path, files, visited, media)?
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
            {
                let path = media.existing(&path)?;
                files
                    .entry(
                        path.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .to_ascii_lowercase(),
                    )
                    .or_insert(path);
            }
        }
        Ok(())
    }
    let mut files = HashMap::new();
    let mut visited = std::collections::HashSet::new();
    for root in media.prefixed("Art/Units").roots() {
        visit(root, &mut files, &mut visited, media)?
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn amb_global_fallback_is_deterministic_and_obeys_edition_precedence() {
        let root = std::env::temp_dir().join(format!("civcraft-amb-media-{}", std::process::id()));
        let owner = root.join("Conquests/Art/Units/Archer/run.amb");
        fs::create_dir_all(owner.parent().unwrap()).unwrap();
        fs::write(&owner, "cue").unwrap();
        let expansion = root.join("Conquests/Art/Units/Other/Shared.WAV");
        let base = root.join("Art/Units/Other/shared.wav");
        for path in [&expansion, &base] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "wav").unwrap();
        }
        let media = Civ3MediaResolver::installation(&root);
        let mut index = None;
        let found = resolve_amb_component(&media, &owner, "shared.wav", &mut index)
            .unwrap()
            .unwrap();
        assert_eq!(found, expansion.canonicalize().unwrap());
        assert!(index.is_some());
        assert!(
            resolve_amb_component(&media, &owner, "missing.wav", &mut index)
                .unwrap()
                .is_none()
        );
        assert!(resolve_amb_component(&media, &owner, "/absolute", &mut index).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
