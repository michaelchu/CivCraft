//! Civ III FLC unit-animation decoding.

use macroquad::prelude::Image;

#[derive(Clone, Debug)]
pub(super) struct Flic {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) original_width: usize,
    pub(super) original_height: usize,
    pub(super) offset_left: usize,
    pub(super) offset_top: usize,
    /// Retained so frame playback can use the source animation cadence.
    #[allow(dead_code)]
    pub(super) animation_speed: u32,
    pub(super) palette: [[u8; 3]; 256],
    /// Palette state at each decoded frame, including palette animations.
    pub(super) frame_palettes: Vec<Vec<[[u8; 3]; 256]>>,
    /// Animation rows, each containing its frames as palette indices.
    pub(super) animations: Vec<Vec<Vec<u8>>>,
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, String> {
    let slice = bytes
        .get(at..at + 2)
        .ok_or_else(|| "truncated FLC header or chunk".to_owned())?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, String> {
    let slice = bytes
        .get(at..at + 4)
        .ok_or_else(|| "truncated FLC header or chunk".to_owned())?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn checked_end(start: usize, length: usize, limit: usize) -> Result<usize, String> {
    let end = start
        .checked_add(length)
        .ok_or_else(|| "FLC chunk size overflow".to_owned())?;
    if end > limit {
        return Err("FLC chunk exceeds file bounds".to_owned());
    }
    Ok(end)
}

fn decode_brun(data: &[u8], width: usize, height: usize) -> Result<(Vec<u8>, usize), String> {
    let mut pixels = vec![0; width * height];
    let mut at = 0;
    for y in 0..height {
        let _packet_count = *data
            .get(at)
            .ok_or_else(|| "truncated FLC byte-run row".to_owned())?;
        at += 1;
        let mut x = 0;
        while x < width {
            let count = *data
                .get(at)
                .ok_or_else(|| "truncated FLC byte-run packet".to_owned())?
                as i8;
            at += 1;
            if count == 0 {
                return Err("invalid zero-length FLC byte-run packet".to_owned());
            }
            let run = usize::from(count.unsigned_abs());
            if x + run > width {
                return Err("FLC byte-run packet exceeds scanline".to_owned());
            }
            if count > 0 {
                let value = *data
                    .get(at)
                    .ok_or_else(|| "truncated FLC byte-run repeat".to_owned())?;
                at += 1;
                pixels[y * width + x..y * width + x + run].fill(value);
            } else {
                let end = at + run;
                let literal = data
                    .get(at..end)
                    .ok_or_else(|| "truncated FLC byte-run literal".to_owned())?;
                pixels[y * width + x..y * width + x + run].copy_from_slice(literal);
                at = end;
            }
            x += run;
        }
    }
    Ok((pixels, at))
}

fn apply_delta(
    data: &[u8],
    pixels: &mut [u8],
    width: usize,
    height: usize,
) -> Result<usize, String> {
    let lines = usize::from(read_u16(data, 0)?);
    let mut at = 2;
    let mut y = 0usize;
    for _ in 0..lines {
        let mut packet_header = read_u16(data, at)?;
        at += 2;
        if packet_header & 0xc000 == 0xc000 {
            y = y
                .checked_add(usize::from((packet_header as i16).unsigned_abs()))
                .ok_or_else(|| "FLC delta scanline overflow".to_owned())?;
            packet_header = read_u16(data, at)?;
            at += 2;
        }
        if packet_header & 0x8000 != 0 {
            if y >= height || width == 0 {
                return Err("FLC delta odd pixel outside image".to_owned());
            }
            pixels[y * width + width - 1] = (packet_header & 0xff) as u8;
            packet_header = read_u16(data, at)?;
            at += 2;
        }
        if packet_header & 0xc000 != 0 {
            return Err("unsupported FLC delta packet header".to_owned());
        }
        if y >= height {
            return Err("FLC delta scanline outside image".to_owned());
        }
        let mut x = 0usize;
        for _ in 0..packet_header {
            x = x
                .checked_add(usize::from(
                    *data
                        .get(at)
                        .ok_or_else(|| "truncated FLC delta skip".to_owned())?,
                ))
                .ok_or_else(|| "FLC delta column overflow".to_owned())?;
            at += 1;
            let words = *data
                .get(at)
                .ok_or_else(|| "truncated FLC delta packet".to_owned())?
                as i8;
            at += 1;
            if words == 0 {
                return Err("invalid zero-length FLC delta packet".to_owned());
            }
            let count = usize::from(words.unsigned_abs()) * 2;
            if x + count > width {
                return Err("FLC delta packet exceeds scanline".to_owned());
            }
            let packet = if words > 0 {
                let end = at + count;
                let bytes = data
                    .get(at..end)
                    .ok_or_else(|| "truncated FLC delta literal".to_owned())?;
                at = end;
                bytes
            } else {
                data.get(at..at + 2)
                    .ok_or_else(|| "truncated FLC delta repeat".to_owned())?
            };
            let row = &mut pixels[y * width + x..y * width + x + count];
            if words > 0 {
                row.copy_from_slice(packet);
            } else {
                for pair in row.chunks_exact_mut(2) {
                    pair.copy_from_slice(packet);
                }
                at += 2;
            }
            x += count;
        }
        y += 1;
    }
    Ok(at)
}

pub(super) fn decode_flic(bytes: &[u8]) -> Result<Flic, String> {
    if bytes.len() < 128 {
        return Err("truncated FLC header".to_owned());
    }
    if read_u16(bytes, 4)? != 0xaf12 {
        return Err("unsupported FLC format: expected 0xAF12".to_owned());
    }
    let declared_size = read_u32(bytes, 0)? as usize;
    if declared_size < 128 || declared_size > bytes.len() {
        return Err("invalid FLC declared file size".to_owned());
    }
    let bytes = &bytes[..declared_size];
    let file_frames = usize::from(read_u16(bytes, 6)?);
    let width = usize::from(read_u16(bytes, 8)?);
    let height = usize::from(read_u16(bytes, 10)?);
    if width == 0 || height == 0 || width > 2048 || height > 2048 {
        return Err("invalid FLC frame dimensions".to_owned());
    }
    let animation_speed = read_u32(bytes, 16)?;
    let offset_left = usize::from(read_u16(bytes, 100)?);
    let offset_top = usize::from(read_u16(bytes, 102)?);
    let original_width = usize::from(read_u16(bytes, 104)?);
    let original_height = usize::from(read_u16(bytes, 106)?);
    let declared_animations = usize::from(read_u16(bytes, 0x60)?);
    let declared_per_animation = usize::from(read_u16(bytes, 0x62)?);
    let (animation_count, frames_per_animation) = if declared_animations == 0 {
        (1, file_frames)
    } else {
        (declared_animations, declared_per_animation)
    };
    if animation_count == 0
        || animation_count > 16
        || frames_per_animation == 0
        || frames_per_animation > 4096
        || animation_count * frames_per_animation > 16384
    {
        return Err("invalid Civ III FLC animation dimensions".to_owned());
    }
    let mut palette = [[0u8; 3]; 256];
    let mut animations = Vec::with_capacity(animation_count);
    let mut frame_palettes = Vec::with_capacity(animation_count);
    let mut offset = read_u32(bytes, 80)? as usize;
    if offset < 128 || offset >= bytes.len() {
        return Err("invalid FLC first-frame offset".to_owned());
    }
    for _ in 0..animation_count {
        let mut frames: Vec<Vec<u8>> = Vec::with_capacity(frames_per_animation);
        let mut palettes: Vec<[[u8; 3]; 256]> = Vec::with_capacity(frames_per_animation);
        for frame_index in 0..frames_per_animation {
            let frame_end = checked_end(offset, read_u32(bytes, offset)? as usize, bytes.len())?;
            if frame_end - offset < 16 {
                return Err("invalid FLC frame size".to_owned());
            }
            let frame_magic = read_u16(bytes, offset + 4)?;
            if frame_magic != 0xf1fa && frame_magic != 0xf100 {
                return Err("invalid FLC frame marker".to_owned());
            }
            let chunks = usize::from(read_u16(bytes, offset + 6)?);
            let mut pixels = if frame_index == 0 {
                vec![0; width * height]
            } else {
                frames[frame_index - 1].clone()
            };
            let mut chunk_at = offset + 16;
            for _ in 0..chunks {
                let chunk_size = read_u32(bytes, chunk_at)? as usize;
                let chunk_type = read_u16(bytes, chunk_at + 4)?;
                let chunk_data_at = chunk_at + 6;
                let declared_end = chunk_at.checked_add(chunk_size);
                let size_is_valid =
                    chunk_size >= 6 && declared_end.is_some_and(|end| end <= frame_end);
                // Some shipped Civ III palette chunks have an uninitialized
                // 0xCDCDCDCD size word. Decode their packet stream to determine
                // the actual end; all other chunks must carry a valid length.
                let chunk_end = if size_is_valid {
                    declared_end.unwrap()
                } else if chunk_type == 4 {
                    frame_end
                } else {
                    return Err("invalid FLC subchunk size".to_owned());
                };
                let data = &bytes[chunk_at + 6..chunk_end];
                let consumed = match chunk_type {
                    4 => {
                        let packets = usize::from(read_u16(data, 0)?);
                        let mut cursor = 2usize;
                        let mut color = 0usize;
                        for _ in 0..packets {
                            color = color
                                .checked_add(usize::from(
                                    *data
                                        .get(cursor)
                                        .ok_or_else(|| "truncated FLC palette skip".to_owned())?,
                                ))
                                .ok_or_else(|| "FLC palette index overflow".to_owned())?;
                            cursor += 1;
                            let count = match *data
                                .get(cursor)
                                .ok_or_else(|| "truncated FLC palette count".to_owned())?
                            {
                                0 => 256,
                                count => usize::from(count),
                            };
                            cursor += 1;
                            let palette_end = cursor + count * 3;
                            let colors = data
                                .get(cursor..palette_end)
                                .ok_or_else(|| "truncated FLC palette colors".to_owned())?;
                            if color + count > 256 {
                                return Err("FLC palette packet exceeds 256 colors".to_owned());
                            }
                            for (index, rgb) in colors.chunks_exact(3).enumerate() {
                                palette[color + index].copy_from_slice(rgb);
                            }
                            color += count;
                            cursor = palette_end;
                        }
                        cursor
                    }
                    7 => apply_delta(data, &mut pixels, width, height)?,
                    15 => {
                        let (decoded, used) = decode_brun(data, width, height)?;
                        pixels = decoded;
                        used
                    }
                    _ => return Err(format!("unsupported FLC subchunk type {chunk_type}")),
                };
                chunk_at = if size_is_valid {
                    chunk_end
                } else {
                    chunk_data_at + consumed
                };
            }
            if chunk_at != frame_end {
                return Err("FLC frame contains unaccounted bytes".to_owned());
            }
            frames.push(pixels);
            palettes.push(palette);
            offset = frame_end;
        }
        let ring_size = read_u32(bytes, offset)? as usize;
        if ring_size < 6 {
            return Err("invalid FLC ring-frame size".to_owned());
        }
        offset = checked_end(offset, ring_size, bytes.len())?;
        animations.push(frames);
        frame_palettes.push(palettes);
    }
    if declared_animations == 0 && file_frames == 0 {
        return Err("FLC contains no frames".to_owned());
    }
    Ok(Flic {
        width,
        height,
        original_width,
        original_height,
        offset_left,
        offset_top,
        animation_speed,
        palette,
        frame_palettes,
        animations,
    })
}

impl Flic {
    /// Create normal-color and team-tint layers from one indexed animation frame.
    fn image_layers_with_palette(
        &self,
        indices: &[u8],
        palette: &[[u8; 3]; 256],
    ) -> Result<(Image, Image), String> {
        if indices.len() != self.width * self.height {
            return Err("FLC frame pixel count does not match its dimensions".to_owned());
        }
        let mut base = Image::gen_image_color(
            self.width as u16,
            self.height as u16,
            macroquad::prelude::Color::from_rgba(0, 0, 0, 0),
        );
        let mut tint = Image::gen_image_color(
            self.width as u16,
            self.height as u16,
            macroquad::prelude::Color::from_rgba(0, 0, 0, 0),
        );
        for (index, &palette_index) in indices.iter().enumerate() {
            let color = palette[usize::from(palette_index)];
            if palette_index < 16 || (palette_index < 64 && palette_index % 2 == 0) {
                tint.get_image_data_mut()[index] = [255, 255, 255, 255];
            } else if (224..=239).contains(&palette_index) {
                let alpha = ((u16::from(palette_index - 224) * 255) / 15) as u8;
                base.get_image_data_mut()[index] = [255, 255, 255, alpha];
            } else if palette_index >= 240 {
                let alpha = (255 - palette_index) * 16;
                base.get_image_data_mut()[index] = [0, 0, 0, alpha];
            } else {
                base.get_image_data_mut()[index] = [color[0], color[1], color[2], 255];
            }
        }
        Ok((base, tint))
    }

    /// Pack direction rows and their frames into two GPU-friendly image atlases.
    pub(super) fn image_atlases(&self) -> Result<(Image, Image), String> {
        let frame_count = self.animations.first().map_or(0, Vec::len);
        if frame_count == 0 || self.animations.is_empty() {
            return Err("FLC contains no animation frames".to_owned());
        }
        if self
            .animations
            .iter()
            .any(|frames| frames.len() != frame_count)
        {
            return Err("FLC directions have inconsistent frame counts".to_owned());
        }
        // Keep each texture within a 4096px edge while retaining every frame.
        // Long Conquests animations (for example TankAttackA) exceed this if
        // frames are packed in a single horizontal strip.
        let frame_columns = frame_count.min(4096 / self.width);
        if frame_columns == 0 {
            return Err("FLC frame is wider than the supported texture size".to_owned());
        }
        let frame_rows = frame_count.div_ceil(frame_columns);
        let atlas_width = self
            .width
            .checked_mul(frame_columns)
            .ok_or_else(|| "FLC frame atlas width overflow".to_owned())?;
        let atlas_height = self
            .height
            .checked_mul(frame_rows)
            .and_then(|height| height.checked_mul(self.animations.len()))
            .ok_or_else(|| "FLC frame atlas height overflow".to_owned())?;
        if atlas_width > 4096 || atlas_height > 4096 || atlas_width * atlas_height > 16_777_216 {
            return Err("FLC frame atlas exceeds the supported texture size".to_owned());
        }
        let mut base = Image::gen_image_color(
            atlas_width as u16,
            atlas_height as u16,
            macroquad::prelude::Color::from_rgba(0, 0, 0, 0),
        );
        let mut tint = Image::gen_image_color(
            atlas_width as u16,
            atlas_height as u16,
            macroquad::prelude::Color::from_rgba(0, 0, 0, 0),
        );
        for (direction, frames) in self.animations.iter().enumerate() {
            for (frame_index, frame) in frames.iter().enumerate() {
                let palette = self
                    .frame_palettes
                    .get(direction)
                    .and_then(|palettes| palettes.get(frame_index))
                    .unwrap_or(&self.palette);
                let (frame_base, frame_tint) = self.image_layers_with_palette(frame, palette)?;
                for y in 0..self.height {
                    let src_start = y * self.width;
                    let src_end = src_start + self.width;
                    let frame_x = (frame_index % frame_columns) * self.width;
                    let frame_y =
                        (direction * frame_rows + frame_index / frame_columns) * self.height;
                    let dst_start = (frame_y + y) * atlas_width + frame_x;
                    let dst_end = dst_start + self.width;
                    base.get_image_data_mut()[dst_start..dst_end]
                        .copy_from_slice(&frame_base.get_image_data()[src_start..src_end]);
                    tint.get_image_data_mut()[dst_start..dst_end]
                        .copy_from_slice(&frame_tint.get_image_data()[src_start..src_end]);
                }
            }
        }
        Ok((base, tint))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(kind: u16, data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&((data.len() + 6) as u32).to_le_bytes());
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(data);
        bytes
    }

    fn frame(chunks: &[Vec<u8>]) -> Vec<u8> {
        let size = 16 + chunks.iter().map(Vec::len).sum::<usize>();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(size as u32).to_le_bytes());
        bytes.extend_from_slice(&0xf1fa_u16.to_le_bytes());
        bytes.extend_from_slice(&(chunks.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&[0; 8]);
        for chunk in chunks {
            bytes.extend_from_slice(chunk);
        }
        bytes
    }

    fn fixture() -> Vec<u8> {
        let palette = vec![1, 0, 1, 1, 10, 20, 30]; // one packet, skip one, set palette index 1
        let palette = chunk(4, &palette);
        let brun = chunk(15, &[1, 4, 1]); // one row: repeat palette index 1 four times
        let first = frame(&[palette, brun]);
        let mut delta = Vec::new();
        delta.extend_from_slice(&1_u16.to_le_bytes()); // one changed line
        delta.extend_from_slice(&1_i16.to_le_bytes()); // one word packet
        delta.extend_from_slice(&[1, 1, 3, 4]); // skip one, write two literal pixels
        let second = frame(&[chunk(7, &delta)]);
        let ring = [6_u32.to_le_bytes().as_slice(), &[0, 0]].concat();
        let mut bytes = vec![0; 128];
        bytes[4..6].copy_from_slice(&0xaf12_u16.to_le_bytes());
        bytes[6..8].copy_from_slice(&2_u16.to_le_bytes());
        bytes[8..10].copy_from_slice(&4_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
        bytes[12..14].copy_from_slice(&8_u16.to_le_bytes());
        bytes[16..20].copy_from_slice(&125_u32.to_le_bytes());
        bytes[100..102].copy_from_slice(&7_u16.to_le_bytes());
        bytes[102..104].copy_from_slice(&9_u16.to_le_bytes());
        bytes[104..106].copy_from_slice(&240_u16.to_le_bytes());
        bytes[106..108].copy_from_slice(&240_u16.to_le_bytes());
        bytes[80..84].copy_from_slice(&128_u32.to_le_bytes());
        bytes[0x60..0x62].copy_from_slice(&1_u16.to_le_bytes());
        bytes[0x62..0x64].copy_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&first);
        bytes.extend_from_slice(&second);
        bytes.extend_from_slice(&ring);
        let size = bytes.len() as u32;
        bytes[0..4].copy_from_slice(&size.to_le_bytes());
        bytes
    }

    #[test]
    fn decodes_civ3_animation_frames_palette_and_delta_pixels() {
        let flic = decode_flic(&fixture()).unwrap();
        assert_eq!((flic.width, flic.height), (4, 1));
        assert_eq!((flic.original_width, flic.original_height), (240, 240));
        assert_eq!((flic.offset_left, flic.offset_top), (7, 9));
        assert_eq!(flic.animation_speed, 125);
        assert_eq!(flic.palette[1], [10, 20, 30]);
        assert_eq!(flic.animations[0][0], [1, 1, 1, 1]);
        assert_eq!(flic.animations[0][1], [1, 3, 4, 1]);
    }

    #[test]
    fn rejects_truncated_and_non_civ3_flic_data() {
        assert!(decode_flic(&[0; 20]).unwrap_err().contains("truncated"));
        let mut bytes = fixture();
        bytes[4..6].copy_from_slice(&0xaf11_u16.to_le_bytes());
        assert!(decode_flic(&bytes).unwrap_err().contains("0xAF12"));
    }

    #[test]
    fn frame_layers_preserve_palette_team_tint_and_shadow_ranges() {
        let flic = decode_flic(&fixture()).unwrap();
        let mut palette = flic.clone();
        palette.palette[17] = [23, 45, 67];
        let frame = [0, 17, 224, 231];
        let (base, tint) = palette
            .image_layers_with_palette(&frame, &palette.palette)
            .unwrap();
        assert_eq!(base.get_image_data()[0], [0, 0, 0, 0]);
        assert_eq!(tint.get_image_data()[0], [255, 255, 255, 255]);
        assert_eq!(base.get_image_data()[1], [23, 45, 67, 255]);
        assert_eq!(tint.get_image_data()[2], [0, 0, 0, 0]);
        assert_eq!(base.get_image_data()[2], [255, 255, 255, 0]);
        assert_eq!(base.get_image_data()[3], [255, 255, 255, 119]);
        let (shadow, _) = palette
            .image_layers_with_palette(&[240, 255, 17, 17], &palette.palette)
            .unwrap();
        assert_eq!(shadow.get_image_data()[0], [0, 0, 0, 240]);
        assert_eq!(shadow.get_image_data()[1], [0, 0, 0, 0]);
    }

    #[test]
    fn image_atlases_pack_each_direction_and_frame_in_order() {
        let flic = decode_flic(&fixture()).unwrap();
        let (base, tint) = flic.image_atlases().unwrap();
        assert_eq!((base.width(), base.height()), (8, 1));
        assert_eq!(base.get_image_data()[0], [0, 0, 0, 0]);
        assert_eq!(base.get_image_data()[4], [0, 0, 0, 0]);
        assert_eq!(tint.get_image_data()[0], [255, 255, 255, 255]);
        assert_eq!(tint.get_image_data()[4], [255, 255, 255, 255]);
    }
}
