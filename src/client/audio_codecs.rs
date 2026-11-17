//! Pure audio format decoding; no playback, catalogs, or filesystem lookup.
use super::asset_error::{AssetError, AssetResult};
use std::{
    io::{Cursor, ErrorKind},
    path::Path,
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

pub(super) fn decode_to_wav(path: &Path, bytes: &[u8]) -> AssetResult<Vec<u8>> {
    decode_format(path, bytes).map_err(|e| AssetError::decode(path, e))
}
fn decode_format(path: &Path, bytes: &[u8]) -> Result<Vec<u8>, String> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp3" => decode_mp3_to_wav(bytes),
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

pub(super) fn concatenate_wav_clips(clips: &[Vec<u8>]) -> AssetResult<(Vec<u8>, f32)> {
    concatenate_wav_data(clips)
        .map_err(|e| AssetError::decode(Path::new("<Ancient intro clips>"), e))
}
fn concatenate_wav_data(clips: &[Vec<u8>]) -> Result<(Vec<u8>, f32), String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intro_clips_preserve_samples_duration_and_reject_incompatible_formats() {
        let clip = pcm_wav(8000, 1, &[0, 100, -100, 0]).unwrap();
        let (combined, duration) = concatenate_wav_clips(&[clip.clone(), clip.clone()]).unwrap();
        assert_eq!(&combined[44..], [&clip[44..], &clip[44..]].concat());
        assert!((duration - 8.0 / 8000.0).abs() < 1e-8);
        assert!(decode_to_wav(Path::new("intro.wav"), &combined).is_ok());
        let other = pcm_wav(16000, 1, &[0, 100, -100, 0]).unwrap();
        assert!(matches!(
            concatenate_wav_clips(&[clip, other]),
            Err(AssetError::Decode { .. })
        ));
    }
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
                matches!(decode_to_wav(Path::new(name),b"malformed"),Err(AssetError::Decode {path,..}) if path==Path::new(name)),
                "{name}"
            );
        }
    }
}
