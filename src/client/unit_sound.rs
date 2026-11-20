//! Unit sound lookup and playback, kept separate from simulation and drawing.
use super::asset_error::AssetResult;
mod worker;

use super::amb::read_amb_cues;
use super::audio_assets::AudioAssetLoader;
use super::media::Civ3MediaResolver;
use super::*;
use macroquad::audio::{PlaySoundParams, play_sound};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub(crate) struct UnitSoundRequest {
    #[cfg(test)]
    pub(crate) unit_id: u32,
    pub(crate) kind: UnitKind,
    pub(crate) era_index: usize,
    pub(crate) action: String,
    pub(crate) delay_seconds: f64,
}

// Cold assets and backgrounded windows must not produce a burst of old cues.
fn cue_is_fresh(at: f64, now: f64) -> bool {
    now - at <= 0.25
}

fn discard_registered_waveforms(
    prepared: &mut worker::Prepared,
    registered: impl Fn(&Path) -> bool,
) {
    let Ok(sound) = prepared else {
        return;
    };
    if sound
        .cues
        .iter()
        .any(|cue| !cue.wav.is_empty() && registered(&cue.path))
    {
        for cue in &mut std::sync::Arc::make_mut(sound).cues {
            if registered(&cue.path) {
                cue.wav = std::sync::Arc::new(Vec::new());
            }
        }
    }
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
    worker: Option<worker::PreparationWorker>,
    prepared: HashMap<PathBuf, worker::Prepared>,
    waiting: HashMap<PathBuf, Vec<f64>>,
    to_request: VecDeque<PathBuf>,
    in_flight: HashSet<PathBuf>,
    imports: VecDeque<(f64, worker::PreparedCue)>,
    release_pending: VecDeque<PathBuf>,
}

impl UnitSoundPlayer {
    pub(crate) fn new(context: &super::AssetContext) -> Self {
        let worker = context.audio.as_ref().and_then(|library| {
            worker::PreparationWorker::new(library.clone())
                .map_err(|error| eprintln!("Could not start unit audio worker: {error}"))
                .ok()
        });
        Self {
            enabled: worker.is_some(),
            loader: context.audio.clone().map(AudioAssetLoader::new),
            schedule: VecDeque::new(),
            worker,
            prepared: Default::default(),
            waiting: Default::default(),
            to_request: Default::default(),
            in_flight: Default::default(),
            imports: Default::default(),
            release_pending: Default::default(),
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
        for (path, mut prepared) in self.worker.as_ref().unwrap().ready() {
            discard_registered_waveforms(&mut prepared, |path| {
                self.loader.as_ref().unwrap().cached_sound(path).is_some()
            });
            self.in_flight.remove(&path);
            let waiting = self.waiting.remove(&path).unwrap_or_default();
            if let Ok(sound) = &prepared {
                if !muted {
                    for at in waiting {
                        self.queue_prepared(at, sound);
                    }
                }
            } else if let Err(error) = &prepared {
                eprintln!("Could not prepare unit sound {}: {error}", path.display());
            }
            self.prepared.insert(path, prepared);
        }
        while let Some(path) = self.release_pending.front() {
            if !self.worker.as_ref().unwrap().release(path.clone()) {
                break;
            }
            self.release_pending.pop_front();
        }
        if muted {
            self.schedule.clear();
            self.waiting.clear();
            self.to_request.clear();
            self.imports.clear();
            return;
        }
        let now = get_time();
        for (path, delay) in paths {
            let at = now + delay.max(0.0);
            if let Some(prepared) = self.prepared.get(&path).cloned() {
                if let Ok(sound) = prepared {
                    self.queue_prepared(at, &sound);
                }
            } else {
                if !self.waiting.contains_key(&path) {
                    if self.waiting.len() >= 128 {
                        continue;
                    }
                    self.waiting.insert(path.clone(), Vec::new());
                    if !self.in_flight.contains(&path) {
                        self.to_request.push_back(path.clone());
                    }
                }
                let waiting = self.waiting.get_mut(&path).unwrap();
                if waiting.len() < 256 {
                    waiting.push(at);
                }
            }
        }
        while let Some(path) = self.to_request.front() {
            match self.worker.as_ref().unwrap().request(path.clone()) {
                Ok(true) => {
                    self.in_flight.insert(self.to_request.pop_front().unwrap());
                }
                Ok(false) => break,
                Err(error) => {
                    let path = self.to_request.pop_front().unwrap();
                    self.waiting.remove(&path);
                    self.prepared.insert(path, Err(error.clone()));
                    eprintln!("Could not request unit sound: {error}");
                }
            }
        }
        let started = std::time::Instant::now();
        let mut registered = false;
        let mut added = false;
        for count in 0..64 {
            if count > 0 && started.elapsed() >= std::time::Duration::from_millis(2) {
                break;
            }
            let Some((_, cue)) = self.imports.front() else {
                break;
            };
            let needs_registration = self
                .loader
                .as_ref()
                .unwrap()
                .cached_sound(&cue.path)
                .is_none();
            if needs_registration && registered {
                break;
            }
            let (at, cue) = self.imports.pop_front().unwrap();
            if !cue_is_fresh(at, now) {
                continue;
            }
            if needs_registration {
                registered = true;
                if let Err(error) = self
                    .loader
                    .as_mut()
                    .unwrap()
                    .register_prepared(&cue.path, &cue.wav)
                    .await
                {
                    eprintln!("Could not register unit sound: {error}");
                    continue;
                }
            }
            if needs_registration {
                // Device sounds remain cached for this player's lifetime. Keep
                // cue metadata, but release duplicate CPU waveform buffers.
                for prepared in self.prepared.values_mut() {
                    discard_registered_waveforms(prepared, |path| path == cue.path);
                }
                self.release_pending.push_back(cue.path.clone());
            }
            self.schedule.push_back(ScheduledCue { at, path: cue.path });
            if self.schedule.len() > 512 {
                self.schedule.pop_front();
            }
            added = true;
        }
        if added {
            let mut cues: Vec<_> = self.schedule.drain(..).collect();
            cues.sort_by(|a, b| a.at.total_cmp(&b.at));
            self.schedule = cues.into();
        }
        while self.schedule.front().is_some_and(|cue| cue.at <= now) {
            let cue = self.schedule.pop_front().unwrap();
            if !cue_is_fresh(cue.at, now) {
                continue;
            }
            if let Some(sound) = self.loader.as_ref().unwrap().cached_sound(&cue.path) {
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

    fn queue_prepared(&mut self, at: f64, prepared: &worker::PreparedSound) {
        for cue in &prepared.cues {
            self.imports
                .push_back((at + cue.delay_seconds, cue.clone()));
            if self.imports.len() > 512 {
                self.imports.pop_front();
            }
        }
    }
}

fn resolve_amb_component(
    media: &Civ3MediaResolver,
    amb_path: &Path,
    reference: &str,
    index: &mut Option<HashMap<String, PathBuf>>,
) -> AssetResult<Option<PathBuf>> {
    if let Some(found) = super::unit_media::UnitMedia::new(media)
        .lookup(amb_path, reference)
        .map_err(|e| e.context(format!("AMB {}", amb_path.display())))?
    {
        return Ok(Some(found.path));
    }
    if index.is_none() {
        *index = Some(super::unit_media::build_unit_audio_index(media)?);
    }
    Ok(index
        .as_ref()
        .and_then(|index| index.get(&reference.to_ascii_lowercase()).cloned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn registered_waveforms_are_discarded_without_changing_shared_pending_results() {
        let sound = std::sync::Arc::new(worker::PreparedSound {
            cues: vec![
                worker::PreparedCue {
                    path: "one.wav".into(),
                    delay_seconds: 0.0,
                    wav: std::sync::Arc::new(vec![1]),
                },
                worker::PreparedCue {
                    path: "two.wav".into(),
                    delay_seconds: 1.0,
                    wav: std::sync::Arc::new(vec![2]),
                },
            ],
        });
        let mut prepared = Ok(sound.clone());
        discard_registered_waveforms(&mut prepared, |path| path == Path::new("one.wav"));
        let updated = prepared.unwrap();
        assert!(updated.cues[0].wav.is_empty());
        assert_eq!(&**updated.cues[1].wav, &[2]);
        assert_eq!(&**sound.cues[0].wav, &[1]);
        assert_eq!(updated.cues[1].delay_seconds, 1.0);
    }

    #[test]
    fn overdue_cues_expire_instead_of_catching_up_after_loading() {
        assert!(cue_is_fresh(10.0, 10.0));
        assert!(cue_is_fresh(11.0, 10.0));
        assert!(cue_is_fresh(10.0, 10.25));
        assert!(!cue_is_fresh(10.0, 10.251));
    }

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
