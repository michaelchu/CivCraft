use std::{fs, path::Path};

/// Reads the timed WAV cue names from Civ III's PRGM/KMAP/MIDI AMB container.
pub(super) fn read_amb_cues(path: &Path) -> Result<Vec<(f64, String)>, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let mut offset = 0;
    let mut wav_names = Vec::new();
    let mut tracks = Vec::new();
    let mut division = 0u16;
    while offset + 8 <= bytes.len() {
        let tag = &bytes[offset..offset + 4];
        if tag == b"prgm" || tag == b"glbl" {
            let size = read_u32_le(&bytes, offset + 4)? as usize;
            offset = offset
                .checked_add(8 + size)
                .filter(|end| *end <= bytes.len())
                .ok_or("invalid AMB chunk size")?;
        } else if tag == b"kmap" {
            let (name, end_name) = read_cstr(&bytes, offset + 20)?;
            let count = read_u32_le(&bytes, end_name + 1)? as usize;
            if count > 256 {
                return Err("unreasonable AMB KMAP item count".into());
            }
            let mut item = end_name + 1 + 8;
            let mut first = String::new();
            for index in 0..count {
                let (wav, end) = read_cstr(&bytes, item + 12)?;
                if index == 0 {
                    first = wav;
                }
                item = end + 5;
            }
            if !name.is_empty() {
                wav_names.push(first);
            }
            // A few shipped KMAP chunks (notably GalleyAttack) have an
            // inaccurate declared size. Their item terminator still leads
            // directly to the next known chunk, with up to three pad bytes.
            let next_chunk = (item..=item.saturating_add(4).min(bytes.len().saturating_sub(4)))
                .find(|at| {
                    matches!(
                        &bytes[*at..*at + 4],
                        b"prgm" | b"kmap" | b"glbl" | b"MThd" | b"MTrk"
                    )
                });
            if let Some(next) = next_chunk {
                offset = next;
            } else {
                let size = read_u32_le(&bytes, offset + 4)? as usize;
                offset = offset
                    .checked_add(8 + size)
                    .filter(|end| *end <= bytes.len())
                    .ok_or("invalid AMB KMAP chunk size")?;
            }
        } else if tag == b"MThd" {
            let header_size = read_u32_be(&bytes, offset + 4)? as usize;
            if header_size < 6 || offset + 8 + header_size > bytes.len() {
                return Err("invalid AMB MIDI header".into());
            }
            division = u16::from_be_bytes([bytes[offset + 12], bytes[offset + 13]]);
            offset += 8 + header_size;
        } else if tag == b"MTrk" {
            let size = read_u32_be(&bytes, offset + 4)? as usize;
            let end = offset
                .checked_add(8 + size)
                .filter(|end| *end <= bytes.len())
                .ok_or("invalid AMB MIDI track size")?;
            tracks.push(bytes[offset + 8..end].to_vec());
            offset = end;
        } else {
            return Err(format!("unknown AMB chunk at byte {offset}"));
        }
    }
    if division == 0 || tracks.is_empty() {
        return Err("AMB has no usable MIDI timeline".into());
    }
    let tempo = midi_tempo(&tracks[0]).unwrap_or(500_000);
    let seconds_per_tick = f64::from(tempo) / 1_000_000.0 / f64::from(division);
    let mut cues = Vec::new();
    for (index, track) in tracks.iter().skip(1).enumerate() {
        let Some(wav) = wav_names.get(index).filter(|name| !name.is_empty()) else {
            continue;
        };
        if let Some(tick) = first_note_on_tick(track)? {
            cues.push((f64::from(tick) * seconds_per_tick, wav.clone()));
        }
    }
    cues.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(cues)
}

fn midi_tempo(track: &[u8]) -> Option<u32> {
    let mut i = 0;
    let mut running = 0u8;
    while i < track.len() {
        let (_, n) = read_varlen(track, i).ok()?;
        i = n;
        let mut status = *track.get(i)?;
        if status < 0x80 {
            status = running;
        } else {
            i += 1;
        }
        if status == 0xff {
            let kind = *track.get(i)?;
            i += 1;
            let (length, next) = read_varlen(track, i).ok()?;
            i = next;
            if kind == 0x51 && length == 3 {
                return Some(
                    (u32::from(*track.get(i)?) << 16)
                        | (u32::from(*track.get(i + 1)?) << 8)
                        | u32::from(*track.get(i + 2)?),
                );
            }
            i = i.checked_add(length as usize)?;
        } else if (0x80..0xf0).contains(&status) {
            running = status;
            let n = if status & 0xe0 == 0xc0 { 1 } else { 2 };
            i = i.checked_add(n)?;
        } else {
            return None;
        }
    }
    None
}

fn first_note_on_tick(track: &[u8]) -> Result<Option<u32>, String> {
    let mut i = 0;
    let mut tick = 0u32;
    let mut running = 0u8;
    while i < track.len() {
        let (delta, next) = read_varlen(track, i)?;
        i = next;
        tick = tick.saturating_add(delta);
        let mut status = *track.get(i).ok_or("truncated AMB MIDI event")?;
        if status < 0x80 {
            status = running;
        } else {
            i += 1;
        }
        if status == 0xff {
            i += 1;
            let (length, next) = read_varlen(track, i)?;
            i = next
                .checked_add(length as usize)
                .ok_or("invalid MIDI meta event")?;
        } else if (0x80..0xf0).contains(&status) {
            running = status;
            let n = if status & 0xe0 == 0xc0 { 1 } else { 2 };
            let data = track
                .get(i..i + n)
                .ok_or("truncated AMB MIDI channel event")?;
            if status & 0xf0 == 0x90 && data.len() == 2 && data[1] > 0 {
                return Ok(Some(tick));
            }
            i += n;
        } else {
            return Err("unsupported AMB MIDI event".into());
        }
    }
    Ok(None)
}

fn read_varlen(bytes: &[u8], mut i: usize) -> Result<(u32, usize), String> {
    let mut value = 0u32;
    for _ in 0..4 {
        let byte = *bytes.get(i).ok_or("truncated MIDI variable length")?;
        i += 1;
        value = (value << 7) | u32::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Ok((value, i));
        }
    }
    Err("oversized MIDI variable length".into())
}
fn read_cstr(bytes: &[u8], start: usize) -> Result<(String, usize), String> {
    let tail = bytes.get(start..).ok_or("truncated AMB string")?;
    let len = tail
        .iter()
        .position(|b| *b == 0)
        .ok_or("unterminated AMB string")?;
    Ok((
        String::from_utf8_lossy(&tail[..len]).into_owned(),
        start + len,
    ))
}
fn read_u32_le(bytes: &[u8], i: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        bytes
            .get(i..i + 4)
            .ok_or("truncated AMB integer")?
            .try_into()
            .unwrap(),
    ))
}
fn read_u32_be(bytes: &[u8], i: usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(
        bytes
            .get(i..i + 4)
            .ok_or("truncated AMB integer")?
            .try_into()
            .unwrap(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};

    #[test]
    fn worker_run_amb_yields_timed_cues_resolving_to_local_wavs() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("source/Art/Units/Worker/WorkerRun.amb");
        if !source.is_file() {
            return;
        }
        let cues = read_amb_cues(&source).unwrap();
        assert!(!cues.is_empty());
        assert!(cues.windows(2).all(|pair| pair[0].0 <= pair[1].0));
        for (_, name) in cues {
            assert!(
                source.parent().unwrap().join(&name).is_file(),
                "missing AMB component {name}"
            );
        }
    }

    #[test]
    fn invalid_amb_data_is_rejected_without_panicking() {
        let path =
            std::env::temp_dir().join(format!("invalid-unit-amb-{}.amb", std::process::id()));
        fs::write(&path, b"not an AMB").unwrap();
        assert!(read_amb_cues(&path).is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn campaign_amb_files_parse_across_all_art_roots_when_available() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("source");
        if !source.is_dir() {
            return;
        }
        let mut pending = vec![
            source.join("Art/Units"),
            source.join("civ3PTW/Art/Units"),
            source.join("Conquests/Art/Units"),
        ];
        let mut failures = Vec::new();
        while let Some(dir) = pending.pop() {
            let Ok(entries) = fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("amb"))
                    && let Err(error) = read_amb_cues(&path)
                {
                    failures.push(format!("{}: {error}", path.display()));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "AMB parser failures: {}",
            failures.join("\n")
        );
    }
}
