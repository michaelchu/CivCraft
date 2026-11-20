//! Asset-free final minimap, using the desktop's staggered, horizontally wrapped projection.
use crate::sim::{observation::MinimapSnapshot, trace::Phase};
use image::{Rgb, RgbImage};
use std::{fs, path::Path};

pub(super) fn write_final(
    report: &Path,
    turn: i32,
    phase: Phase,
    map: &MinimapSnapshot,
) -> Result<serde_json::Value, String> {
    let filename = report.file_name().unwrap().to_string_lossy();
    let path = report.with_file_name(format!("{filename}.minimap.png"));
    let temporary = path.with_extension("png.tmp");
    render(map)?
        .save_with_format(&temporary, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    fs::rename(temporary, &path).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "format":"png", "path":path.file_name().unwrap().to_string_lossy(),
        "turn":turn, "phase":phase, "width":map.width, "height":map.height,
        "civilizations":map.civilizations,
        "lookup":"Final state only. Full map without fog of war; terrain tinted by current owner and outlined city markers. Colors are listed in civilizations. No per-turn minimap history is recorded."
    }))
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
    use crate::sim::observation::MinimapCity;
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

    #[test]
    fn renders_claimed_tiles_without_frontier_guides() {
        let width = 8;
        let height = 8;
        let owners = (0..height)
            .flat_map(|_| (0..width).map(|x| Some(if x < width / 2 { 0 } else { 1 })))
            .collect();
        let map = MinimapSnapshot {
            width: width as i32,
            height: height as i32,
            terrain: vec![[100, 100, 100]; width * height],
            owners,
            cities: vec![],
            civilizations: vec![
                crate::sim::observation::MinimapCivilization {
                    id: 0,
                    name: "A".into(),
                    color: [200, 0, 0],
                    alive: true,
                },
                crate::sim::observation::MinimapCivilization {
                    id: 1,
                    name: "B".into(),
                    color: [0, 0, 200],
                    alive: true,
                },
            ],
        };
        let image = render(&map).unwrap();
        let expected_a = [162, 37, 37];
        let expected_b = [37, 37, 162];
        assert_eq!(image.get_pixel(42, 3).0, expected_a);
        assert_eq!(image.get_pixel(54, 3).0, expected_b);
    }
}
