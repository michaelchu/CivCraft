//! Filesystem lookup, AMB expansion and decoding without audio-device access.
use super::super::audio_assets::AudioLibrary;
use super::*;
use std::{
    sync::{Arc, mpsc},
    thread,
};

#[derive(Clone)]
pub(super) struct PreparedCue {
    pub path: PathBuf,
    pub delay_seconds: f64,
    pub wav: Arc<Vec<u8>>,
}
#[derive(Clone)]
pub(super) struct PreparedSound {
    pub cues: Vec<PreparedCue>,
}
pub(super) type Prepared = Result<Arc<PreparedSound>, String>;
type Reply = (PathBuf, Prepared);

enum Request {
    Prepare(PathBuf),
    Release(PathBuf),
}

pub(super) struct PreparationWorker {
    requests: mpsc::SyncSender<Request>,
    completed: mpsc::Receiver<Reply>,
}
impl PreparationWorker {
    pub fn new(library: Arc<AudioLibrary>) -> Result<Self, String> {
        let (requests, incoming) = mpsc::sync_channel::<Request>(128);
        let (finished, completed) = mpsc::channel();
        thread::Builder::new()
            .name("civcraft-unit-audio".into())
            .spawn(move || {
                let mut cache = PreparationCache::default();
                while let Ok(request) = incoming.recv() {
                    let path = match request {
                        Request::Prepare(path) => path,
                        Request::Release(path) => {
                            cache.release(&path);
                            continue;
                        }
                    };
                    let prepared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        cache.prepare(&library.media, &path)
                    }))
                    .unwrap_or_else(|_| Err("unit audio preparation panicked".to_owned()));
                    if finished.send((path, prepared)).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            requests,
            completed,
        })
    }
    pub fn request(&self, path: PathBuf) -> Result<bool, String> {
        match self.requests.try_send(Request::Prepare(path)) {
            Ok(()) => Ok(true),
            Err(mpsc::TrySendError::Full(_)) => Ok(false),
            Err(mpsc::TrySendError::Disconnected(_)) => {
                Err("unit audio worker disconnected".into())
            }
        }
    }
    pub fn release(&self, path: PathBuf) -> bool {
        match self.requests.try_send(Request::Release(path)) {
            Ok(()) => true,
            Err(mpsc::TrySendError::Full(_)) => false,
            Err(mpsc::TrySendError::Disconnected(_)) => true,
        }
    }
    pub fn ready(&self) -> Vec<Reply> {
        self.completed.try_iter().collect()
    }
}

#[derive(Default)]
struct PreparationCache {
    sounds: HashMap<PathBuf, Prepared>,
    waveforms: HashMap<PathBuf, Arc<Vec<u8>>>,
    component_index: Option<HashMap<String, PathBuf>>,
    registered: HashSet<PathBuf>,
}
impl PreparationCache {
    fn release(&mut self, path: &Path) {
        self.waveforms.remove(path);
        self.registered.insert(path.to_owned());
        for prepared in self
            .sounds
            .values_mut()
            .filter_map(|result| result.as_mut().ok())
        {
            if prepared
                .cues
                .iter()
                .any(|cue| cue.path == path && !cue.wav.is_empty())
            {
                for cue in &mut Arc::make_mut(prepared).cues {
                    if cue.path == path {
                        cue.wav = Arc::new(Vec::new());
                    }
                }
            }
        }
    }
    fn prepare(&mut self, media: &MediaResolver, requested: &Path) -> Prepared {
        if let Some(prepared) = self.sounds.get(requested) {
            return prepared.clone();
        }
        let prepared = self.prepare_uncached(media, requested).map(Arc::new);
        self.sounds.insert(requested.to_owned(), prepared.clone());
        prepared
    }
    fn prepare_uncached(
        &mut self,
        media: &MediaResolver,
        requested: &Path,
    ) -> Result<PreparedSound, String> {
        let path = media
            .existing(requested)
            .map_err(|error| error.to_string())?;
        let is_amb = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("amb"));
        let sources = if is_amb {
            let mut sources = Vec::new();
            for (delay, reference) in read_amb_cues(&path)? {
                match resolve_amb_component(media, &path, &reference, &mut self.component_index) {
                    Ok(Some(path)) => sources.push((delay, path)),
                    Ok(None) => eprintln!(
                        "Unit sound {} references missing WAV '{reference}'",
                        path.display()
                    ),
                    Err(error) => eprintln!("Could not resolve unit cue: {error}"),
                }
            }
            sources
        } else {
            vec![(0.0, path)]
        };
        let mut cues = Vec::new();
        for (delay_seconds, source) in sources {
            let prepared = (|| {
                let path = media.existing(&source).map_err(|error| error.to_string())?;
                let wav = if self.registered.contains(&path) {
                    Arc::new(Vec::new())
                } else if let Some(wav) = self.waveforms.get(&path) {
                    wav.clone()
                } else {
                    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
                    let wav = Arc::new(
                        super::super::audio_codecs::decode_to_wav(&path, &bytes)
                            .map_err(|error| error.to_string())?,
                    );
                    self.waveforms.insert(path.clone(), wav.clone());
                    wav
                };
                Ok::<_, String>(PreparedCue {
                    path,
                    delay_seconds,
                    wav,
                })
            })();
            match prepared {
                Ok(cue) => cues.push(cue),
                Err(error) if is_amb => eprintln!("Could not prepare AMB component: {error}"),
                Err(error) => return Err(error),
            }
        }
        Ok(PreparedSound { cues })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preparation_reuses_decoded_waveforms_after_source_files_are_removed() {
        let root = std::env::temp_dir().join(format!("civcraft-audio-wave-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("tone.mp3");
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/audio/tone.mp3"),
        )
        .unwrap();
        let media = MediaResolver::new(vec![root.clone()]);
        let mut cache = PreparationCache::default();
        let first = cache.prepare(&media, &path).unwrap();
        assert_eq!(first.cues.len(), 1);
        assert!(first.cues[0].wav.starts_with(b"RIFF"));
        std::fs::remove_file(&path).unwrap();
        let second = cache.prepare(&media, &path).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        cache.release(&first.cues[0].path);
        assert!(cache.waveforms.is_empty());
        let registered = cache.prepare(&media, &path).unwrap();
        assert!(registered.cues[0].wav.is_empty());
        assert_eq!(registered.cues[0].path, first.cues[0].path);
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn shipped_worker_amb_is_cached_with_timing_and_shared_component_data() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("source");
        let path = root.join("Art/Units/Worker/WorkerRun.amb");
        if !path.is_file() {
            return;
        }
        let media = MediaResolver::installation(&root);
        let mut cache = PreparationCache::default();
        let sound = cache.prepare(&media, &path).unwrap();
        let source = read_amb_cues(&path).unwrap();
        assert_eq!(sound.cues.len(), source.len());
        for (cue, (delay, _)) in sound.cues.iter().zip(source.iter()) {
            assert_eq!(cue.delay_seconds, *delay);
            assert!(cue.wav.starts_with(b"RIFF"));
        }
        assert!(Arc::ptr_eq(&sound, &cache.prepare(&media, &path).unwrap()));
    }

    #[test]
    fn preparation_caches_failures_and_never_reads_outside_media_roots() {
        let root = std::env::temp_dir().join(format!("civcraft-audio-prep-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let media = MediaResolver::new(vec![root.clone()]);
        let mut cache = PreparationCache::default();
        let missing = root.join("missing.wav");
        let first = cache.prepare(&media, &missing).err().unwrap();
        std::fs::write(&missing, b"malformed").unwrap();
        assert_eq!(cache.prepare(&media, &missing).err().unwrap(), first);
        assert!(cache.prepare(&media, Path::new("/etc/hosts")).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
