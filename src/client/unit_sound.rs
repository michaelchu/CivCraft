//! Unit sound lookup and playback, kept separate from simulation and drawing.

use super::amb::read_amb_cues;
use super::*;
use macroquad::audio::{PlaySoundParams, Sound, load_sound_from_bytes, play_sound};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub(crate) struct UnitSoundRequest {
    pub(crate) unit: Unit,
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
    cache: HashMap<PathBuf, Sound>,
    schedule: VecDeque<ScheduledCue>,
    component_index: Option<HashMap<String, PathBuf>>,
}

impl UnitSoundPlayer {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            cache: HashMap::new(),
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
        if !self.enabled {
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
                            match resolve_amb_component(&path, &filename, &mut self.component_index)
                            {
                                Some(wav) => self.schedule.push_back(ScheduledCue {
                                    at: now + delay_seconds.max(0.0) + delay,
                                    path: wav,
                                }),
                                None => eprintln!(
                                    "Unit sound {} references missing WAV '{filename}'",
                                    path.display()
                                ),
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
            if !self.cache.contains_key(&cue.path) {
                match fs::read(&cue.path) {
                    Ok(bytes) => match load_sound_from_bytes(&bytes).await {
                        Ok(sound) => {
                            self.cache.insert(cue.path.clone(), sound);
                        }
                        Err(error) => {
                            eprintln!("Could not load unit sound {}: {error}", cue.path.display());
                            continue;
                        }
                    },
                    Err(error) => {
                        eprintln!("Could not read unit sound {}: {error}", cue.path.display());
                        continue;
                    }
                }
            }
            if let Some(sound) = self.cache.get(&cue.path) {
                play_sound(
                    sound,
                    PlaySoundParams {
                        looped: false,
                        volume: f32::from(volume.min(100)) / 100.0,
                    },
                );
            }
        }
    }
}

fn resolve_case_insensitive_path(base: &Path, reference: &str) -> Option<PathBuf> {
    let normalized = reference.trim().trim_matches('"').replace('\\', "/");
    let mut path = base.to_owned();
    for component in normalized.split('/') {
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." {
            path.pop();
            continue;
        }
        let exact = path.join(component);
        if exact.exists() {
            path = exact;
            continue;
        }
        path = fs::read_dir(&path)
            .ok()?
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(component)
            })?
            .path();
    }
    path.is_file().then_some(path)
}

fn resolve_amb_component(
    amb_path: &Path,
    reference: &str,
    index: &mut Option<HashMap<String, PathBuf>>,
) -> Option<PathBuf> {
    let parent = amb_path.parent().unwrap_or(Path::new("."));
    if let Some(found) = resolve_case_insensitive_path(parent, reference) {
        return Some(found);
    }
    let folder = parent.file_name()?;
    let source = parent
        .ancestors()
        .find(|path| {
            ["Art/Units", "civ3PTW/Art/Units", "Conquests/Art/Units"]
                .iter()
                .any(|relative| path.join(relative).is_dir())
        })?;
    for root in [
        source.join("Conquests/Art"),
        source.join("civ3PTW/Art"),
        source.join("Art"),
    ] {
        if let Some(found) =
            resolve_case_insensitive_path(&root.join("Units").join(folder), reference)
        {
            return Some(found);
        }
    }
    if index.is_none() {
        *index = Some(build_unit_audio_index(source));
    }
    index
        .as_ref()?
        .get(&reference.to_ascii_lowercase())
        .cloned()
}

fn build_unit_audio_index(source: &Path) -> HashMap<String, PathBuf> {
    let roots = [
        source.join("Conquests/Art/Units"),
        source.join("civ3PTW/Art/Units"),
        source.join("Art/Units"),
    ];
    let mut files = HashMap::new();
    let mut pending = roots.into_iter().collect::<Vec<_>>();
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
                && let Some(name) = path.file_name()
            {
                files
                    .entry(name.to_string_lossy().to_ascii_lowercase())
                    .or_insert(path);
            }
        }
    }
    files
}
