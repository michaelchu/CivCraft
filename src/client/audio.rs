//! Campaign music loaded from the user's Civilization III installation.

use crate::rules::TechEra;
#[cfg(test)]
use crate::sim::HUMAN;
use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume, stop_sound,
};
use macroquad::prelude::get_frame_time;
use std::{
    fs,
    io::{Cursor, ErrorKind},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    errors::Error as DecodeError,
    formats::FormatOptions,
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
    probe::Hint,
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
    root: Option<PathBuf>,
    requested_cue: Option<MusicCue>,
    current_sound: Option<Sound>,
    incoming_sound: Option<Sound>,
    pending_load: Option<Receiver<(MusicCue, Result<PreparedMusic, String>)>>,
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
}

impl MusicPlayer {
    pub(crate) fn new(enabled: bool) -> Self {
        let root = enabled
            .then(|| super::tileset::locate_source_root().ok())
            .flatten()
            .map(|path| path.join("Sounds"));
        Self {
            root,
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
        let Some(root) = self.root.as_deref() else {
            return;
        };
        let cue = MusicCue { era, culture_group };

        if request_music_cue(&mut self.requested_cue, cue) {
            let Some(paths) = resolve_music_paths(root, cue) else {
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
                            clip_bytes.push(
                                fs::read(&path)
                                    .map_err(|error| format!("{}: {error}", path.display()))?,
                            );
                        }
                        let (wav, duration) = concatenate_wav_clips(&clip_bytes)?;
                        Some((wav, duration))
                    };
                    let bytes = fs::read(&paths.full_track)
                        .map_err(|error| format!("{}: {error}", paths.full_track.display()))?;
                    let full_track = decode_mp3_to_wav(&bytes)
                        .map_err(|error| format!("{}: {error}", paths.full_track.display()))?;
                    Ok(PreparedMusic { intro, full_track })
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
                                                (full_sound, true)
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
                        looped: true,
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

fn resolve_music_paths(root: &Path, cue: MusicCue) -> Option<MusicPaths> {
    let full_track = resolve_track(root, cue.era, cue.culture_group)?;
    let clips = if cue.era == TechEra::Ancient {
        if let Some(code) = group_code(cue.culture_group) {
            (b'a'..=b'i')
                .map(|letter| root.join(format!("build/ancient/Anc{code}{}.wav", letter as char)))
                .filter(|path| path.is_file())
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    Some(MusicPaths { clips, full_track })
}

fn concatenate_wav_clips(clips: &[Vec<u8>]) -> Result<(Vec<u8>, f32), String> {
    let first = clips
        .first()
        .ok_or_else(|| "no Ancient WAV clips found".to_owned())?;
    if first.len() < 44 || &first[0..4] != b"RIFF" || &first[8..12] != b"WAVE" {
        return Err("Ancient WAV clip has an unsupported header".to_owned());
    }
    let format = &first[12..40];
    let byte_rate = u32::from_le_bytes(first[28..32].try_into().unwrap());
    if byte_rate == 0 {
        return Err("Ancient WAV clip has an invalid byte rate".to_owned());
    }
    let mut audio = Vec::new();
    for clip in clips {
        if clip.len() < 44
            || &clip[0..4] != b"RIFF"
            || &clip[8..12] != b"WAVE"
            || &clip[12..40] != format
            || &clip[36..40] != b"data"
        {
            return Err("Ancient WAV clips do not share a supported PCM format".to_owned());
        }
        let data_size = u32::from_le_bytes(clip[40..44].try_into().unwrap()) as usize;
        let data_end = 44_usize
            .checked_add(data_size)
            .filter(|&end| end <= clip.len())
            .ok_or_else(|| "Ancient WAV clip is truncated".to_owned())?;
        audio.extend_from_slice(&clip[44..data_end]);
    }
    let data_size =
        u32::try_from(audio.len()).map_err(|_| "Ancient intro is too large".to_owned())?;
    let riff_size = data_size
        .checked_add(36)
        .ok_or_else(|| "Ancient intro is too large".to_owned())?;
    let mut wav = Vec::with_capacity(44 + audio.len());
    wav.extend_from_slice(&first[..44]);
    wav[4..8].copy_from_slice(&riff_size.to_le_bytes());
    wav[40..44].copy_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(&audio);
    let duration = data_size as f32 / byte_rate as f32;
    Ok((wav, duration))
}

fn decode_mp3_to_wav(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let stream = MediaSourceStream::new(
        Box::new(Cursor::new(bytes.to_vec())),
        MediaSourceStreamOptions::default(),
    );
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| error.to_string())?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| "MP3 has no audio track".to_owned())?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|error| error.to_string())?;
    let mut sample_rate = None;
    let mut channel_count = None;
    let mut samples = Vec::<i16>::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(DecodeError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => break,
            Err(DecodeError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(error) => return Err(error.to_string()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(DecodeError::DecodeError(_)) => continue,
            Err(DecodeError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(error) => return Err(error.to_string()),
        };
        let spec = *decoded.spec();
        let rate = spec.rate;
        let channels = spec.channels.count();
        if sample_rate.is_some_and(|current| current != rate)
            || channel_count.is_some_and(|current| current != channels)
        {
            return Err("MP3 changes sample rate or channel count mid-track".to_owned());
        }
        sample_rate = Some(rate);
        channel_count = Some(channels);
        let mut buffer = SampleBuffer::<i16>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        samples.extend_from_slice(buffer.samples());
    }

    let sample_rate = sample_rate.ok_or_else(|| "MP3 contained no decoded samples".to_owned())?;
    let channels = channel_count.ok_or_else(|| "MP3 contained no audio channels".to_owned())?;
    let data_bytes = u32::try_from(samples.len() * 2)
        .map_err(|_| "decoded MP3 is too large for a WAV buffer".to_owned())?;
    let channels = u16::try_from(channels).map_err(|_| "MP3 has too many channels".to_owned())?;
    let block_align = channels * 2;
    let byte_rate = sample_rate * u32::from(block_align);

    let mut wav = Vec::with_capacity(44 + data_bytes as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(wav)
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

/// Resolves an exact culture/era track first, then a deterministic era fallback.
fn resolve_track(root: &Path, era: TechEra, group: i32) -> Option<PathBuf> {
    let exact = exact_track(era, group).map(|path| root.join(path));
    if let Some(path) = exact.filter(|path| path.is_file()) {
        return Some(path);
    }

    let fallbacks: Vec<PathBuf> = match era {
        TechEra::Ancient => ordered_group_codes(group)
            .map(|code| {
                let full = if code == "ME" { "Full" } else { "full" };
                root.join(format!("build/ancient/Anc{code}{full}.mp3"))
            })
            .collect(),
        TechEra::MiddleAges => ordered_group_codes(group)
            .map(|code| root.join(format!("build/Middle Ages/Mid{code}Full.mp3")))
            .collect(),
        TechEra::Industrial => ordered_group_codes(group)
            .map(|code| root.join(format!("build/IndModern/Ind{code}Full.mp3")))
            .collect(),
        TechEra::Modern => {
            let offset = usize::try_from(group).unwrap_or_default() % GENERIC_MODERN_TRACKS.len();
            (0..GENERIC_MODERN_TRACKS.len())
                .map(|index| {
                    root.join(GENERIC_MODERN_TRACKS[(offset + index) % GENERIC_MODERN_TRACKS.len()])
                })
                .collect()
        }
    };

    fallbacks.into_iter().find(|path| path.is_file())
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
        root
    }

    #[test]
    fn culture_groups_map_to_the_civ3_music_codes() {
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
        let wav = decode_mp3_to_wav(&fs::read(path).unwrap()).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert!(wav.len() > 44);
    }
}
