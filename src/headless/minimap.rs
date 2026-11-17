//! Asset-free minimap evidence, using the desktop's staggered, horizontally wrapped projection.
use crate::sim::{
    observation::{MinimapCity, MinimapCivilization, MinimapSnapshot},
    trace::Phase,
};
use image::{Rgb, RgbImage};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
struct Frame {
    turn: i32,
    phase: Phase,
    path: String,
    cities: Vec<MinimapCity>,
    civilizations: Vec<MinimapCivilization>,
    owned_tiles: std::collections::BTreeMap<usize, usize>,
}

pub(super) struct MinimapReport {
    directory: PathBuf,
    final_path: PathBuf,
    viewer_path: PathBuf,
    frames: Vec<Frame>,
}
impl MinimapReport {
    pub fn new(report: &Path) -> Self {
        let filename = report.file_name().unwrap().to_string_lossy();
        Self {
            directory: report.with_file_name(format!("{filename}.minimaps")),
            final_path: report.with_file_name(format!("{filename}.minimap.png")),
            viewer_path: report.with_file_name(format!("{filename}.minimap.html")),
            frames: Vec::new(),
        }
    }
    pub fn capture(
        &mut self,
        turn: i32,
        phase: Phase,
        map: &MinimapSnapshot,
    ) -> Result<(), String> {
        fs::create_dir_all(&self.directory).map_err(|e| e.to_string())?;
        let filename = format!("{:06}.png", self.frames.len());
        render(map)?
            .save(self.directory.join(&filename))
            .map_err(|e| e.to_string())?;
        let mut owned_tiles = std::collections::BTreeMap::new();
        for owner in map.owners.iter().flatten() {
            *owned_tiles.entry(*owner).or_default() += 1;
        }
        self.frames.push(Frame {
            turn,
            phase,
            path: url_path(&format!(
                "{}/{}",
                self.directory.file_name().unwrap().to_string_lossy(),
                filename
            )),
            cities: map.cities.clone(),
            civilizations: map.civilizations.clone(),
            owned_tiles,
        });
        Ok(())
    }
    pub fn publish(&self) -> Result<serde_json::Value, String> {
        if self.frames.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        let last = self
            .directory
            .join(format!("{:06}.png", self.frames.len() - 1));
        let temporary = self.final_path.with_extension("png.tmp");
        fs::copy(last, &temporary).map_err(|e| e.to_string())?;
        fs::rename(temporary, &self.final_path).map_err(|e| e.to_string())?;
        let data = serde_json::to_string(&self.frames)
            .map_err(|e| e.to_string())?
            .replace('<', "\\u003c");
        let html = include_str!("minimap.html").replace("__FRAMES__", &data);
        let temporary = self.viewer_path.with_extension("html.tmp");
        fs::write(&temporary, html).map_err(|e| e.to_string())?;
        fs::rename(temporary, &self.viewer_path).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({
            "format":"png", "path":self.final_path.file_name().unwrap().to_string_lossy(),
            "viewer":self.viewer_path.file_name().unwrap().to_string_lossy(),
            "frames":self.frames.len(), "turn":self.frames.last().unwrap().turn,
            "lookup":"Open viewer locally to scrub every snapshot. Full map without fog of war; terrain tinted by current owner, outlined city markers. PNG is the latest captured state; a running checkpoint is not the final turn."
        }))
    }
}

fn url_path(path: &str) -> String {
    path.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._/".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn render(map: &MinimapSnapshot) -> Result<RgbImage, String> {
    let width = u32::try_from(map.width)
        .ok()
        .filter(|w| *w > 0)
        .ok_or("invalid minimap width")?;
    let height = u32::try_from(map.height)
        .ok()
        .filter(|h| *h > 0)
        .ok_or("invalid minimap height")?;
    let count = (width as usize)
        .checked_mul(height as usize)
        .ok_or("minimap too large")?;
    if map.terrain.len() != count || map.owners.len() != count {
        return Err("minimap tile arrays do not match dimensions".into());
    }
    let scale = (960 / (width * 2)).clamp(1, 6);
    let mut image = RgbImage::new(width * 2 * scale, height * scale);
    let color = |owner| {
        map.civilizations
            .iter()
            .find(|p| p.id == owner)
            .map(|p| p.color)
    };
    // Tile span is two projection units wide; odd rows start one unit to the right.
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) as usize;
            let terrain = map.terrain[i];
            let rgb = map.owners[i]
                .and_then(color)
                .map(|c| {
                    std::array::from_fn(|k| {
                        ((u16::from(terrain[k]) * 3 + u16::from(c[k]) * 5) / 8) as u8
                    })
                })
                .unwrap_or(terrain);
            for dy in 0..scale {
                for dx in 0..2 * scale {
                    let px = ((2 * x + y % 2) * scale + dx) % image.width();
                    image.put_pixel(px, y * scale + dy, Rgb(rgb));
                }
            }
        }
    }
    for city in &map.cities {
        if city.x < 0 || city.x >= map.width || city.y < 0 || city.y >= map.height {
            return Err("minimap city outside map".into());
        }
        let cx = (2 * city.x as u32 + city.y as u32 % 2 + 1) * scale;
        let cy = city.y as u32 * scale + scale / 2;
        let radius = (scale / 2).max(1) as i32;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let y = cy as i32 + dy;
                if !(0..image.height() as i32).contains(&y) {
                    continue;
                }
                let x = (cx as i32 + dx).rem_euclid(image.width() as i32) as u32;
                let rgb = if dx.abs() == radius || dy.abs() == radius {
                    [255, 255, 255]
                } else {
                    color(city.owner).unwrap_or([0, 0, 0])
                };
                image.put_pixel(x, y as u32, Rgb(rgb));
            }
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staggered_rows_wrap_and_city_markers_wrap_without_changing_snapshot() {
        let mut map = MinimapSnapshot {
            width: 2,
            height: 2,
            terrain: vec![[10, 20, 30], [40, 50, 60], [70, 80, 90], [100, 110, 120]],
            owners: vec![None; 4],
            cities: vec![],
            civilizations: vec![],
        };
        let image = render(&map).unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [10, 20, 30]);
        assert_eq!(image.get_pixel(0, 6).0, [100, 110, 120]);
        assert_eq!(image.get_pixel(6, 6).0, [70, 80, 90]);
        map.cities.push(MinimapCity {
            id: 1,
            owner: 0,
            name: "Edge".into(),
            x: 1,
            y: 1,
            population: 1,
        });
        let before = serde_json::to_value(&map).unwrap();
        let image = render(&map).unwrap();
        assert_eq!(image.get_pixel(0, 9).0, [0, 0, 0]);
        assert_eq!(image.get_pixel(3, 9).0, [255, 255, 255]);
        assert_eq!(serde_json::to_value(&map).unwrap(), before);
        map.owners.pop();
        assert!(render(&map).is_err());
    }
}
