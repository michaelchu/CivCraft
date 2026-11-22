//! Campaign music loaded through configured media roots.
use super::asset_error::{AssetError, AssetResult};
#[cfg(test)]
use std::path::Path;

use crate::rules::TechEra;
#[cfg(test)]
use crate::sim::HUMAN;
use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume, stop_sound,
};
use macroquad::prelude::get_frame_time;
use std::{
    fs,
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

const CULTURE_GROUP_CODES: [&str; 5] = ["NA", "EC", "GR", "ME", "OR"];
const GENERIC_MODERN_TRACKS: [&str; 3] = [
    "build/IndModern/SmashFull.mp3",
    "build/IndModern/StarsFull.mp3",
    "build/IndModern/Techno MixFull.mp3",
];
const MUSIC_FADE_SECONDS: f32 = 0.65;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FadeKind {
    None,
    In,
    Cross,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MusicCue {
    era: TechEra,
    culture_group: i32,
}

#[cfg(test)]
fn human_music_cue(
    era_for_player: impl Fn(usize) -> TechEra,
    culture_group_for_player: impl Fn(usize) -> i32,
) -> MusicCue {
    MusicCue {
        era: era_for_player(HUMAN),
        culture_group: culture_group_for_player(HUMAN),
    }
}

fn request_music_cue(requested: &mut Option<MusicCue>, cue: MusicCue) -> bool {
    if *requested == Some(cue) {
        false
    } else {
        *requested = Some(cue);
        true
    }
}

fn crossfade_weights(progress: f32) -> (f32, f32) {
    let progress = progress.clamp(0.0, 1.0);
    (1.0 - progress, progress)
}

/// Manages the current looping music track for an interactive campaign.
pub(crate) struct MusicPlayer {
    library: Option<std::sync::Arc<super::audio_assets::AudioLibrary>>,
    full_looped: bool,
    requested_cue: Option<MusicCue>,
    current_sound: Option<Sound>,
    incoming_sound: Option<Sound>,
    pending_load: Option<Receiver<(MusicCue, AssetResult<PreparedMusic>)>>,
    full_after_intro: Option<Sound>,
    intro_duration: Option<f32>,
    intro_elapsed: f32,
    playing: bool,
    incoming_playing: bool,
    fade_kind: FadeKind,
    fade_elapsed: f32,
    enabled: bool,
}

struct PreparedMusic {
    intro: Option<(Vec<u8>, f32)>,
    full_track: Vec<u8>,
    looped: bool,
}

impl MusicPlayer {
    pub(crate) fn new(context: &super::AssetContext) -> Self {
        let library = context.audio.clone();
        let enabled = library.is_some();
        Self {
            library,
            full_looped: true,
            requested_cue: None,
            current_sound: None,
            incoming_sound: None,
            pending_load: None,
            full_after_intro: None,
            intro_duration: None,
            intro_elapsed: 0.0,
            playing: false,
            incoming_playing: false,
            fade_kind: FadeKind::None,
            fade_elapsed: 0.0,
            enabled,
        }
    }

    /// Starts or changes music when the human player's era or culture changes.
    pub(crate) async fn update(
        &mut self,
        era: TechEra,
        culture_group: i32,
        muted: bool,
        volume: u8,
    ) {
        if !self.enabled {
            return;
        }
        let Some(library) = self.library.as_ref() else {
            return;
        };
        let cue = MusicCue { era, culture_group };

        if request_music_cue(&mut self.requested_cue, cue) {
            let paths = match resolve_library_music(library, cue) {
                Ok(paths) => paths,
                Err(error) => {
                    self.stop_current();
                    eprintln!("Could not resolve music: {error}");
                    return;
                }
            };
            let Some((paths, looped)) = paths else {
                self.stop_current();
                eprintln!(
                    "No Civ III music track found for {} culture group {}",
                    cue.era.name(),
                    cue.culture_group
                );
                return;
            };
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                let result = (|| {
                    let intro = if paths.clips.is_empty() {
                        None
                    } else {
                        let mut clip_bytes = Vec::with_capacity(paths.clips.len());
                        for path in paths.clips {
                            clip_bytes.push(fs::read(&path).map_err(|error| {
                                AssetError::io("read intro clip", &path, error)
                            })?);
                        }
                        let (wav, duration) =
                            super::audio_codecs::concatenate_wav_clips(&clip_bytes)?;
                        Some((wav, duration))
                    };
                    let bytes = fs::read(&paths.full_track)
                        .map_err(|error| AssetError::io("read music", &paths.full_track, error))?;
                    let full_track = super::audio_codecs::decode_to_wav(&paths.full_track, &bytes)?;
                    Ok(PreparedMusic {
                        intro,
                        full_track,
                        looped,
                    })
                })();
                let _ = sender.send((cue, result));
            });
            self.pending_load = Some(receiver);
        }

        if let Some(receiver) = self.pending_load.take() {
            match receiver.try_recv() {
                Ok((loaded_cue, result)) => {
                    if self.requested_cue == Some(loaded_cue) {
                        match result {
                            Ok(prepared) => {
                                let intro = match prepared.intro {
                                    Some((wav, duration)) => {
                                        match load_sound_from_bytes(&wav).await {
                                            Ok(sound) => Some((sound, duration)),
                                            Err(error) => {
                                                self.stop_current();
                                                eprintln!(
                                                    "Could not load Ancient intro clips: {error}"
                                                );
                                                None
                                            }
                                        }
                                    }
                                    None => None,
                                };
                                self.full_looped = prepared.looped;
                                match load_sound_from_bytes(&prepared.full_track).await {
                                    Ok(full_sound) => {
                                        if self.incoming_playing
                                            && let Some(previous) = &self.incoming_sound
                                        {
                                            stop_sound(previous);
                                        }
                                        let (sound, looped) =
                                            if let Some((intro_sound, duration)) = intro {
                                                self.full_after_intro = Some(full_sound);
                                                self.intro_duration = Some(duration);
                                                self.intro_elapsed = 0.0;
                                                (intro_sound, false)
                                            } else {
                                                self.full_after_intro = None;
                                                self.intro_duration = None;
                                                self.intro_elapsed = 0.0;
                                                (full_sound, prepared.looped)
                                            };
                                        if self.playing {
                                            self.incoming_sound = Some(sound);
                                            play_sound(
                                                self.incoming_sound.as_ref().unwrap(),
                                                PlaySoundParams {
                                                    looped,
                                                    volume: 0.0,
                                                },
                                            );
                                            self.incoming_playing = true;
                                            self.fade_kind = FadeKind::Cross;
                                        } else {
                                            self.current_sound = Some(sound);
                                            play_sound(
                                                self.current_sound.as_ref().unwrap(),
                                                PlaySoundParams {
                                                    looped,
                                                    volume: 0.0,
                                                },
                                            );
                                            self.playing = true;
                                            self.fade_kind = FadeKind::In;
                                        }
                                        self.fade_elapsed = 0.0;
                                    }
                                    Err(error) => {
                                        self.stop_current();
                                        eprintln!("Could not load music: {error}");
                                    }
                                }
                            }
                            Err(error) => {
                                self.stop_current();
                                eprintln!("Could not decode music: {error}");
                            }
                        }
                    }
                }
                Err(TryRecvError::Empty) => self.pending_load = Some(receiver),
                Err(TryRecvError::Disconnected) => {
                    self.stop_current();
                    eprintln!("Music loading thread ended before returning a track");
                }
            }
        }

        if let Some(duration) = self.intro_duration {
            self.intro_elapsed += get_frame_time().clamp(0.0, 0.1);
            if self.intro_elapsed >= duration
                && let Some(full) = self.full_after_intro.take()
            {
                if let Some(intro) = &self.current_sound {
                    stop_sound(intro);
                }
                self.current_sound = Some(full);
                play_sound(
                    self.current_sound.as_ref().unwrap(),
                    PlaySoundParams {
                        looped: self.full_looped,
                        volume: 0.0,
                    },
                );
                self.fade_kind = FadeKind::In;
                self.fade_elapsed = 0.0;
                self.intro_duration = None;
                self.intro_elapsed = 0.0;
            }
        }

        let volume = if muted {
            0.0
        } else {
            f32::from(volume.min(100)) / 100.0
        };
        if self.fade_kind != FadeKind::None {
            self.fade_elapsed += get_frame_time().clamp(0.0, 0.1);
        }
        let progress = (self.fade_elapsed / MUSIC_FADE_SECONDS).clamp(0.0, 1.0);
        match self.fade_kind {
            FadeKind::None => {
                if let Some(sound) = &self.current_sound {
                    set_sound_volume(sound, volume);
                }
            }
            FadeKind::In => {
                if let Some(sound) = &self.current_sound {
                    set_sound_volume(sound, volume * progress);
                }
                if progress >= 1.0 {
                    self.fade_kind = FadeKind::None;
                }
            }
            FadeKind::Cross => {
                let (outgoing, incoming) = crossfade_weights(progress);
                if let Some(sound) = &self.current_sound {
                    set_sound_volume(sound, volume * outgoing);
                }
                if let Some(sound) = &self.incoming_sound {
                    set_sound_volume(sound, volume * incoming);
                }
                if progress >= 1.0 {
                    if let Some(sound) = &self.current_sound {
                        stop_sound(sound);
                    }
                    self.current_sound = self.incoming_sound.take();
                    self.playing = self.current_sound.is_some();
                    self.incoming_playing = false;
                    self.fade_kind = FadeKind::None;
                    self.full_after_intro = None;
                    self.intro_duration = None;
                    self.intro_elapsed = 0.0;
                }
            }
        }
    }

    fn stop_current(&mut self) {
        if self.playing
            && let Some(sound) = &self.current_sound
        {
            stop_sound(sound);
        }
        if self.incoming_playing
            && let Some(sound) = &self.incoming_sound
        {
            stop_sound(sound);
        }
        self.current_sound = None;
        self.incoming_sound = None;
        self.playing = false;
        self.incoming_playing = false;
        self.fade_kind = FadeKind::None;
        self.fade_elapsed = 0.0;
        self.full_after_intro = None;
        self.intro_duration = None;
        self.intro_elapsed = 0.0;
    }
}

struct MusicPaths {
    clips: Vec<PathBuf>,
    full_track: PathBuf,
}

#[cfg(test)]
fn resolve_music_file(root: &Path, relative: &Path) -> Option<PathBuf> {
    let roots = if root.file_name().is_some_and(|name| name == "Sounds") {
        let install = root.parent()?;
        vec![
            install.join("Conquests/Sounds"),
            install.join("civ3PTW/Sounds"),
            root.to_owned(),
        ]
    } else {
        vec![root.to_owned()]
    };
    super::media::MediaResolver::new(roots)
        .resolve(relative.to_str()?)
        .ok()
        .filter(|p| p.is_file())
}

#[cfg(test)]
fn resolve_music_paths(root: &Path, cue: MusicCue) -> Option<MusicPaths> {
    let full_track = resolve_track(root, cue.era, cue.culture_group)?;
    let clips = if cue.era == TechEra::Ancient {
        if let Some(code) = group_code(cue.culture_group) {
            (b'a'..=b'i')
                .filter_map(|letter| {
                    resolve_music_file(
                        root,
                        Path::new(&format!("build/ancient/Anc{code}{}.wav", letter as char)),
                    )
                })
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    Some(MusicPaths { clips, full_track })
}

fn group_code(group: i32) -> Option<&'static str> {
    CULTURE_GROUP_CODES
        .get(usize::try_from(group).ok()?)
        .copied()
}

fn exact_track(era: TechEra, group: i32) -> Option<PathBuf> {
    let code = group_code(group)?;
    let relative = match era {
        TechEra::Ancient => {
            let full = if code == "ME" { "Full" } else { "full" };
            format!("build/ancient/Anc{code}{full}.mp3")
        }
        TechEra::MiddleAges => {
            format!("build/Middle Ages/Mid{code}Full.mp3")
        }
        TechEra::Industrial => {
            format!("build/IndModern/Ind{code}Full.mp3")
        }
        TechEra::Modern => return None,
    };
    Some(PathBuf::from(relative))
}

fn ordered_group_codes(group: i32) -> impl Iterator<Item = &'static str> {
    let offset = usize::try_from(group).unwrap_or_default() % CULTURE_GROUP_CODES.len();
    (0..CULTURE_GROUP_CODES.len())
        .map(move |index| CULTURE_GROUP_CODES[(offset + index) % CULTURE_GROUP_CODES.len()])
}

fn music_candidates(era: TechEra, group: i32) -> Vec<PathBuf> {
    let mut candidates: Vec<_> = exact_track(era, group).into_iter().collect();
    let fallbacks: Vec<_> = match era {
        TechEra::Ancient => ordered_group_codes(group)
            .map(|code| {
                let full = if code == "ME" { "Full" } else { "full" };
                PathBuf::from(format!("build/ancient/Anc{code}{full}.mp3"))
            })
            .collect(),
        TechEra::MiddleAges => ordered_group_codes(group)
            .map(|code| PathBuf::from(format!("build/Middle Ages/Mid{code}Full.mp3")))
            .collect(),
        TechEra::Industrial => ordered_group_codes(group)
            .map(|code| PathBuf::from(format!("build/IndModern/Ind{code}Full.mp3")))
            .collect(),
        TechEra::Modern => {
            let offset = usize::try_from(group).unwrap_or_default() % GENERIC_MODERN_TRACKS.len();
            (0..GENERIC_MODERN_TRACKS.len())
                .map(|i| {
                    PathBuf::from(GENERIC_MODERN_TRACKS[(offset + i) % GENERIC_MODERN_TRACKS.len()])
                })
                .collect()
        }
    };
    candidates.extend(fallbacks);
    candidates
}
#[cfg(test)]
fn resolve_track(root: &Path, era: TechEra, group: i32) -> Option<PathBuf> {
    music_candidates(era, group)
        .into_iter()
        .find_map(|p| resolve_music_file(root, &p))
}
fn resolve_library_music(
    library: &super::audio_assets::AudioLibrary,
    cue: MusicCue,
) -> AssetResult<Option<(MusicPaths, bool)>> {
    for candidate in music_candidates(cue.era, cue.culture_group) {
        let logical = format!("Sounds/{}", candidate.to_string_lossy());
        if let Some(full_track) = library.resolve_default(&logical)? {
            let mut clips = Vec::new();
            if cue.era == TechEra::Ancient
                && let Some(code) = group_code(cue.culture_group)
            {
                for letter in b'a'..=b'i' {
                    if let Some(path) = library.resolve_default(&format!(
                        "Sounds/build/ancient/Anc{code}{}.wav",
                        letter as char
                    ))? {
                        clips.push(path);
                    }
                }
            }
            return Ok(Some((
                MusicPaths { clips, full_track },
                library.default_looped(&logical),
            )));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_sounds() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "civcraft-audio-{}-{}",
            std::process::id(),
            TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        root.canonicalize().unwrap()
    }

    #[test]
    fn music_uses_expansion_precedence_and_case_insensitive_clip_names() {
        let install = temp_sounds();
        let root = install.join("Sounds");
        let base = root.join("build/ancient");
        let expansion = install.join("conquests/sounds/BUILD/ANCIENT");
        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&expansion).unwrap();
        fs::write(base.join("AncGRfull.mp3"), "base").unwrap();
        let track = expansion.join("ancgrFULL.MP3");
        let clip = expansion.join("ancgrA.WAV");
        fs::write(&track, "expansion").unwrap();
        fs::write(&clip, "intro").unwrap();
        let paths = resolve_music_paths(
            &root,
            MusicCue {
                era: TechEra::Ancient,
                culture_group: 2,
            },
        )
        .unwrap();
        assert_eq!(paths.full_track, track);
        assert_eq!(paths.clips, vec![clip]);
        fs::remove_dir_all(install).unwrap();
    }

    /// Mirrors Civ III behavior.
    #[test]
    fn culture_groups_map_to_the_native_music_codes() {
        assert_eq!(group_code(0), Some("NA"));
        assert_eq!(group_code(1), Some("EC"));
        assert_eq!(group_code(2), Some("GR"));
        assert_eq!(group_code(3), Some("ME"));
        assert_eq!(group_code(4), Some("OR"));
        assert_eq!(group_code(5), None);
    }

    #[test]
    fn human_era_change_requests_a_track_once() {
        let mut requested = None;
        let ancient = MusicCue {
            era: TechEra::Ancient,
            culture_group: 2,
        };
        let middle_ages = MusicCue {
            era: TechEra::MiddleAges,
            culture_group: 2,
        };

        assert!(request_music_cue(&mut requested, ancient));
        assert!(!request_music_cue(&mut requested, ancient));
        assert!(request_music_cue(&mut requested, middle_ages));
        assert!(!request_music_cue(&mut requested, middle_ages));
    }

    #[test]
    fn ai_era_changes_do_not_change_the_human_music_cue() {
        let mut eras = [TechEra::Ancient, TechEra::Ancient];
        let groups = [2, 4];
        let initial = human_music_cue(|owner| eras[owner], |owner| groups[owner]);

        eras[1] = TechEra::Modern;
        assert_eq!(
            human_music_cue(|owner| eras[owner], |owner| groups[owner]),
            initial
        );

        eras[HUMAN] = TechEra::MiddleAges;
        let after_human_discovery = human_music_cue(|owner| eras[owner], |owner| groups[owner]);
        assert_eq!(after_human_discovery.era, TechEra::MiddleAges);
        assert_ne!(after_human_discovery, initial);
    }

    #[test]
    fn changing_culture_group_also_requests_a_new_track_once() {
        let mut requested = None;
        let ancient_greek = MusicCue {
            era: TechEra::Ancient,
            culture_group: 2,
        };
        let ancient_oriental = MusicCue {
            era: TechEra::Ancient,
            culture_group: 4,
        };

        assert!(request_music_cue(&mut requested, ancient_greek));
        assert!(request_music_cue(&mut requested, ancient_oriental));
        assert!(!request_music_cue(&mut requested, ancient_oriental));
    }

    #[test]
    fn crossfade_moves_smoothly_from_the_outgoing_to_incoming_track() {
        assert_eq!(crossfade_weights(0.0), (1.0, 0.0));
        assert_eq!(crossfade_weights(0.5), (0.5, 0.5));
        assert_eq!(crossfade_weights(1.0), (0.0, 1.0));
        assert_eq!(crossfade_weights(2.0), (0.0, 1.0));
    }

    #[test]
    fn exact_tracks_resolve_for_all_ancient_and_medieval_groups() {
        let root = temp_sounds();
        for era in [TechEra::Ancient, TechEra::MiddleAges] {
            for group in 0..5 {
                let relative = exact_track(era, group).unwrap();
                let full_path = root.join(relative);
                fs::create_dir_all(full_path.parent().unwrap()).unwrap();
                fs::write(&full_path, b"track").unwrap();
                assert_eq!(resolve_track(&root, era, group), Some(full_path));
            }
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn industrial_missing_group_uses_an_industrial_fallback() {
        let root = temp_sounds();
        let fallback = root.join("build/IndModern/IndECFull.mp3");
        fs::create_dir_all(fallback.parent().unwrap()).unwrap();
        fs::write(&fallback, b"track").unwrap();
        assert_eq!(resolve_track(&root, TechEra::Industrial, 0), Some(fallback));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn modern_tracks_resolve_from_the_generic_modern_pool() {
        let root = temp_sounds();
        let fallback = root.join("build/IndModern/StarsFull.mp3");
        fs::create_dir_all(fallback.parent().unwrap()).unwrap();
        fs::write(&fallback, b"track").unwrap();
        assert_eq!(resolve_track(&root, TechEra::Modern, 1), Some(fallback));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn every_era_and_culture_group_has_a_resolvable_track_or_fallback() {
        let root = temp_sounds();
        let groups = [0, 1, 2, 3, 4];
        for group in groups {
            for era in [TechEra::Ancient, TechEra::MiddleAges, TechEra::Industrial] {
                if let Some(relative) = exact_track(era, group)
                    && (era != TechEra::Industrial || group != 0)
                {
                    let path = root.join(relative);
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(path, b"track").unwrap();
                }
            }
        }
        for relative in GENERIC_MODERN_TRACKS {
            let path = root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"track").unwrap();
        }

        for era in TechEra::ALL {
            for group in groups {
                assert!(
                    resolve_track(&root, era, group).is_some(),
                    "{era:?}, group {group}"
                );
            }
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installed_mp3_decodes_to_macroquad_compatible_wav_when_source_is_configured() {
        let Ok(root) = super::super::tileset::locate_source_root() else {
            return;
        };
        let path = root.join("Sounds/build/ancient/AncGRfull.mp3");
        if !path.is_file() {
            return;
        }
        let wav =
            super::super::audio_codecs::decode_to_wav(&path, &fs::read(&path).unwrap()).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert!(wav.len() > 44);
    }
}
