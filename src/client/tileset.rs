//! Civ III sheet decoding. Both map views keep the same projection and painter passes.
use super::*;
use crate::sim::TerrainBase;
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

const WIDTH: usize = 128;
const HEIGHT: usize = 64;
const CELL_W: usize = WIDTH + 2;
const CELL_H: usize = HEIGHT + 2;
const PAGE: usize = 12;
const SIDES: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
// OpenCiv3's PCX loader uses the same default palette transparency indices.
const DEFAULT_PCX_TRANSPARENT_INDICES: [u8; 2] = [254, 255];
// 1.0 preserves the source alpha; higher values tighten the black fog fade.
const FOG_ALPHA_GAMMA: f32 = 1.8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Ground {
    Grass,
    Plains,
    Desert,
    Tundra,
    Coast,
    Sea,
    Ocean,
    Ice,
}
use Ground::*;

pub(crate) fn locate_source_root() -> Result<PathBuf, String> {
    let root = std::env::var_os("CIVCRAFT_SOURCE_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| {
            "CIVCRAFT_SOURCE_DIR is required. Set it to the Civ III install folder containing Conquests/Art/Terrain before starting CivCraft.".to_owned()
        })?;
    if root.join("Conquests/Art/Terrain").is_dir() {
        Ok(root)
    } else {
        Err(format!(
            "CIVCRAFT_SOURCE_DIR points to '{}', which does not look like a Civ III install folder (expected Conquests/Art/Terrain)",
            root.display()
        ))
    }
}

fn locate_default_art() -> Result<PathBuf, String> {
    locate_source_root().map(|source| source.join("Conquests/Art"))
}

pub(crate) fn load_source_fog_atlas() -> Result<Image, String> {
    let path = locate_source_root()?.join("Art/Terrain/FogOfWar.pcx");
    let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut image =
        decode_pcx(&bytes, &[254, 255]).map_err(|error| format!("{}: {error}", path.display()))?;
    if (image.width, image.height) != (1152, 576) {
        return Err(format!(
            "{}: expected a 1152x576 Civ III fog atlas, got {}x{}",
            path.display(),
            image.width,
            image.height
        ));
    }
    // FogOfWar.pcx stores opacity inversely in its grayscale palette; magenta
    // color-key pixels stay transparent even though they also have a palette value.
    for pixel in image.bytes.chunks_exact_mut(4) {
        let opacity = (255.0 - f32::from(pixel[0])) / 255.0;
        let curved_opacity = opacity.powf(FOG_ALPHA_GAMMA);
        pixel[3] = (f32::from(pixel[3]) * curved_opacity).round() as u8;
        pixel[0..3].fill(0);
    }
    Ok(image)
}

fn irrigation_source(index: usize) -> Rect {
    debug_assert!(index < 16, "irrigation source index must fit the 4x4 sheet");
    Rect::new(
        ((index % 4) * WIDTH) as f32,
        ((index / 4) * HEIGHT) as f32,
        WIDTH as f32,
        HEIGHT as f32,
    )
}

fn route_source(index: usize) -> Rect {
    debug_assert!(index < 256, "route source index must fit the 16x16 sheet");
    Rect::new(
        ((index % 16) * WIDTH) as f32,
        ((index / 16) * HEIGHT) as f32,
        WIDTH as f32,
        HEIGHT as f32,
    )
}

fn building_source(kind: TerrainBuilding) -> Rect {
    let (col, row) = match kind {
        TerrainBuilding::Fortress => (0, 0),
        TerrainBuilding::Mine => (2, 1),
        TerrainBuilding::BarbarianCamp => (2, 0),
    };
    Rect::new(
        (col * WIDTH) as f32,
        (row * HEIGHT) as f32,
        WIDTH as f32,
        HEIGHT as f32,
    )
}

const LAND_SHEETS: [(&str, &str, [Ground; 3]); 6] = [
    ("desert-grass-plains", "XDGP", [Desert, Grass, Plains]),
    ("desert-grass-coast", "XDGC", [Desert, Grass, Coast]),
    ("desert-plains-coast", "XDPC", [Desert, Plains, Coast]),
    ("plains-grass-coast", "XPGC", [Plains, Grass, Coast]),
    ("tundra-grass-coast", "XTGC", [Tundra, Grass, Coast]),
    ("grass-coast", "XGGC", [Grass, Grass, Coast]),
];
const OVERLAYS: [(&str, &str, u16, u16, usize); 6] = [
    ("vegetation", "grassland forests", 768, 616, 88),
    ("marsh", "marsh", 768, 264, 88),
    ("hills", "XHILLS", 128, 72, 72),
    ("mountains", "Mountains", 128, 88, 88),
    ("floodplains", "floodplains", 512, 256, 64),
    ("volcano", "Volcanos", 512, 352, 88),
];
const IRRIGATION: [(&str, &str); 4] = [
    ("irrigation", "irrigation"),
    ("irrigation-plains", "irrigation PLAINS"),
    ("irrigation-desert", "irrigation DESETT"),
    ("irrigation-tundra", "irrigation TUNDRA"),
];
const OPTIONAL_IMPROVEMENTS: [(&str, &str); 5] = [
    ("farmland", "farmland"),
    ("oil-well", "oil-well"),
    ("oil-platform", "oil-platform"),
    ("airstrip", "airstrip"),
    ("airbase", "airbase"),
];

#[derive(Clone, Copy)]
pub(super) enum TerrainBuilding {
    Fortress,
    Mine,
    BarbarianCamp,
}

pub(crate) enum Tileset {
    Civ3(PathBuf),
}

impl Tileset {
    pub(crate) fn select(name: &str) -> Result<Self, String> {
        match name {
            "civ3" | "conquests" => locate_default_art().map(Self::Civ3),
            "snoopy" | "snoopy-standard" | "snoopy-greener" | "snoopy-extended" => {
                let folder = match name {
                    "snoopy-standard" => "Sn00pys Terrain",
                    "snoopy-extended" => "Sn00pys TerrainX",
                    _ => "Sn00pys TerrainG",
                };
                locate_source_root().map(|source| {
                    Self::Civ3(source.join("civ3PTW/Scenarios").join(folder).join("Art"))
                })
            }
            _ => Err(format!(
                "Unknown tileset '{name}'. Available: civ3, conquests, snoopy-standard, snoopy-greener, snoopy-extended"
            )),
        }
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Civ3(_) => "original-civ3-art",
        }
    }
}

struct Theme {
    root: PathBuf,
    source_root: Option<PathBuf>,
    fallback_roots: Vec<PathBuf>,
    paths: HashMap<String, String>,
    transparent_indices: HashMap<String, Vec<u8>>,
    civilization_city_styles: HashMap<String, String>,
}

impl Theme {
    fn read(path: &Path) -> Result<Self, String> {
        let (root, manifest) = if path.is_dir() {
            (path, path.join("tileset.conf"))
        } else {
            (path.parent().unwrap_or(Path::new(".")), path.to_owned())
        };
        let mut paths = HashMap::new();
        let mut transparent_indices = HashMap::new();
        let mut civilization_city_styles = HashMap::new();
        if !path.is_dir() || manifest.exists() {
            let contents = fs::read_to_string(&manifest)
                .map_err(|error| format!("{}: {error}", manifest.display()))?;
            for (index, line) in contents.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let entry = line
                    .split_once('=')
                    .map(|(key, value)| (key.trim(), value.trim()));
                let valid = |key: &str| {
                    LAND_SHEETS.iter().any(|entry| entry.0 == key)
                        || OVERLAYS.iter().any(|entry| entry.0 == key)
                        || IRRIGATION.iter().any(|entry| entry.0 == key)
                        || key.strip_prefix("unit-").is_some_and(|name| {
                            !name.is_empty()
                                && name.chars().all(|character| {
                                    character.is_ascii_lowercase()
                                        || character.is_ascii_digit()
                                        || character == '-'
                                })
                        })
                        || matches!(
                            key,
                            "water"
                                | "sea"
                                | "ocean"
                                | "polar"
                                | "river-delta"
                                | "river-mountain"
                                | "roads"
                                | "railroads"
                                | "terrain-buildings"
                                | "pollution"
                                | "craters"
                                | "oil-well"
                                | "farmland"
                                | "oil-platform"
                                | "airstrip"
                                | "airbase"
                                | "resources"
                                | "resource-shadows"
                                | "city-asian"
                                | "city-american"
                                | "city-european"
                                | "city-middle_eastern"
                                | "city-roman"
                        )
                };
                let Some((key, value)) = entry else {
                    return Err(format!(
                        "{}:{}: expected a unique sheet role and path",
                        manifest.display(),
                        index + 1
                    ));
                };
                if let Some(role) = key.strip_prefix("transparent-") {
                    let parsed = value
                        .split(',')
                        .map(str::trim)
                        .map(str::parse::<u8>)
                        .collect::<Result<Vec<_>, _>>();
                    match parsed {
                        Ok(indices)
                            if valid(role)
                                && !indices.is_empty()
                                && !transparent_indices.contains_key(role) =>
                        {
                            transparent_indices.insert(role.to_owned(), indices);
                        }
                        _ => {
                            return Err(format!(
                                "{}:{}: expected unique transparent palette indices 0–255 for a sheet role",
                                manifest.display(),
                                index + 1
                            ));
                        }
                    }
                } else if let Some(civilization) = key.strip_prefix("civilization-style.") {
                    let valid_civilization = !civilization.is_empty()
                        && civilization.chars().all(|character| {
                            character.is_ascii_lowercase()
                                || character.is_ascii_digit()
                                || character == '-'
                        });
                    let valid_style = CITY_STYLES.iter().any(|(style, _)| *style == value);
                    if valid_civilization
                        && valid_style
                        && !civilization_city_styles.contains_key(civilization)
                    {
                        civilization_city_styles.insert(civilization.to_owned(), value.to_owned());
                    } else {
                        return Err(format!(
                            "{}:{}: expected a unique civilization-style.<slug> mapping to one of {}",
                            manifest.display(),
                            index + 1,
                            CITY_STYLES
                                .iter()
                                .map(|(style, _)| *style)
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                } else if valid(key) && !value.is_empty() && !paths.contains_key(key) {
                    paths.insert(key.to_owned(), value.to_owned());
                } else {
                    return Err(format!(
                        "{}:{}: expected a unique sheet role and path",
                        manifest.display(),
                        index + 1
                    ));
                }
            }
        }
        let source_root = locate_source_root().ok();
        let fallback_roots = if root.file_name().is_some_and(|name| name == "Art")
            && root
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == "Conquests")
            && root
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .is_some_and(|name| name == "source")
        {
            let source = root.parent().and_then(Path::parent).unwrap();
            vec![source.join("civ3PTW/Art"), source.join("Art")]
        } else {
            Vec::new()
        };
        if let Some(source) = &source_root {
            load_campaign_unit_ini_paths(source, &mut paths)?;
        }
        Ok(Self {
            root: root.to_owned(),
            source_root,
            fallback_roots,
            paths,
            transparent_indices,
            civilization_city_styles,
        })
    }

    fn image(
        &self,
        key: &str,
        default: &str,
        size: (u16, u16),
        required: bool,
    ) -> Result<Option<Image>, String> {
        let override_path = self.paths.get(key);
        let relative = override_path.map_or(default, String::as_str);
        let transparent_indices = self
            .transparent_indices
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or(&DEFAULT_PCX_TRANSPARENT_INDICES);
        let mut roots = vec![self.root.as_path()];
        if override_path.is_none() {
            roots.extend(self.fallback_roots.iter().map(PathBuf::as_path));
        }
        let mut last_missing = None;
        let (path, image) = 'roots: loop {
            for root in roots.iter().copied() {
                let path = root.join(relative);
                match read_theme_image(&path, transparent_indices) {
                    Ok(image) => break 'roots image,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        last_missing = Some(path)
                    }
                    Err(error) => return Err(format!("{}: {error}", path.display())),
                }
            }
            if !required && override_path.is_none() {
                return Ok(None);
            }
            let path = last_missing.unwrap_or_else(|| self.root.join(relative));
            let error = std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "file not found in configured source art folders",
            );
            return Err(format!("{}: {error}", path.display()));
        };
        if image.width < size.0 || image.height < size.1 {
            return Err(format!(
                "{}: expected at least {}×{} pixels",
                path.display(),
                size.0,
                size.1
            ));
        }
        Ok(Some(image))
    }
}

fn load_campaign_unit_ini_paths(
    source: &Path,
    paths: &mut HashMap<String, String>,
) -> Result<(), String> {
    let roots = [
        ("base", source.join("Art/Units")),
        ("ptw", source.join("civ3PTW/Art/Units")),
        ("conquests", source.join("Conquests/Art/Units")),
    ]
    .into_iter()
    .collect::<HashMap<_, _>>();
    let data = include_str!("../../data/rulesets/civ3_conquests/units.tsv");
    let mut identities = 0;
    for line in data.lines().skip(1) {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() < 33 {
            return Err("Civ III unit catalog has a malformed row while loading art".into());
        }
        identities += 1;
        let role = format!("unit-{}", fields[0].replace('_', "-"));
        let folders = fields[31].split('|').collect::<Vec<_>>();
        let labels = fields[32].split('|').collect::<Vec<_>>();
        if folders.len() != labels.len() {
            return Err(format!(
                "{} has unmatched art folder/root variants",
                fields[1]
            ));
        }
        for (variant, (folder, label)) in folders.into_iter().zip(labels).enumerate() {
            let Some(root) = roots.get(label) else {
                return Err(format!("{} has unknown art root '{label}'", fields[1]));
            };
            let directory = root.join(folder);
            let ini = fs::read_dir(&directory)
                .map_err(|error| format!("{}: {error}", directory.display()))?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|path| {
                    path.extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("ini"))
                })
                .ok_or_else(|| format!("{}: no unit INI found", directory.display()))?;
            let key = if variant == 0 {
                role.clone()
            } else {
                let era = [
                    "ancient-times",
                    "middle-ages",
                    "industrial-ages",
                    "modern-times",
                ]
                .get(variant)
                .ok_or_else(|| format!("{} has too many era art variants", fields[1]))?;
                format!("{role}-era-{era}")
            };
            paths
                .entry(key)
                .or_insert_with(|| ini.to_string_lossy().into_owned());
        }
    }
    if identities != 92 {
        return Err(format!(
            "expected 92 campaign units when loading art, got {identities}"
        ));
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct UnitAnimationSource {
    role: String,
    path: PathBuf,
    timing_seconds: Option<f32>,
    normal_speed: Option<u32>,
    fast_speed: Option<u32>,
}

impl Theme {
    fn resolve_theme_file(&self, reference: &str) -> Result<PathBuf, String> {
        if Path::new(reference).is_absolute() && Path::new(reference).is_file() {
            return Ok(Path::new(reference).to_owned());
        }
        let mut bases = vec![self.root.clone()];
        bases.extend(self.fallback_roots.iter().cloned());
        if let Some(source) = &self.source_root {
            bases.push(source.clone());
        }
        for base in bases {
            if let Some(path) = resolve_case_insensitive_path(&base, reference) {
                return Ok(path);
            }
        }
        Err(format!(
            "{}: file '{reference}' was not found in the configured art roots",
            self.root.display()
        ))
    }

    fn resolve_unit_file(&self, ini_path: &Path, reference: &str) -> Result<PathBuf, String> {
        let mut bases = Vec::with_capacity(self.fallback_roots.len() + 3);
        if let Some(parent) = ini_path.parent() {
            bases.push(parent.to_owned());
            // Expansion manifests often name an animation shared from the
            // same unit's base-game folder using only its filename. Search
            // that corresponding unit directory before the broader art roots.
            if let Some(unit_folder) = parent.file_name() {
                bases.extend(
                    self.fallback_roots
                        .iter()
                        .map(|root| root.join("Units").join(unit_folder)),
                );
                if let Some(source) = &self.source_root {
                    bases.extend(
                        [
                            source.join("Conquests/Art"),
                            source.join("civ3PTW/Art"),
                            source.join("Art"),
                        ]
                        .into_iter()
                        .map(|root| root.join("Units").join(unit_folder)),
                    );
                }
            }
        }
        bases.push(self.root.clone());
        bases.extend(self.fallback_roots.iter().cloned());
        if let Some(source) = &self.source_root {
            bases.push(source.clone());
        }
        bases.dedup();
        for base in bases {
            if let Some(path) = resolve_case_insensitive_path(&base, reference) {
                return Ok(path);
            }
        }
        Err(format!(
            "{}: animation file '{reference}' was not found relative to the INI or in the configured Civ III art roots",
            ini_path.display()
        ))
    }
}

fn resolve_case_insensitive_path(base: &Path, reference: &str) -> Option<PathBuf> {
    let normalized = reference.trim().trim_matches('"').replace('\\', "/");
    let mut current = if normalized.starts_with('/') {
        PathBuf::from("/")
    } else {
        base.to_owned()
    };
    for component in normalized.split('/') {
        if component.is_empty() || component == "." || component.ends_with(':') {
            continue;
        }
        if component == ".." {
            current.pop();
            continue;
        }
        let exact = current.join(component);
        if exact.exists() {
            current = exact;
            continue;
        }
        let matched = fs::read_dir(&current)
            .ok()?
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(component)
            })?;
        current = matched.path();
    }
    current.is_file().then_some(current)
}

fn expand_unit_animation_ini(
    theme: &Theme,
    role: &str,
    relative_path: &str,
) -> Result<Vec<UnitAnimationSource>, String> {
    let ini_path = theme.resolve_theme_file(relative_path)?;
    let contents = fs::read_to_string(&ini_path)
        .map_err(|error| format!("{}: {error}", ini_path.display()))?;
    let mut section = String::new();
    let mut animations_raw = Vec::<(String, String)>::new();
    let mut timings = HashMap::<String, f32>::new();
    let mut normal_speed = None;
    let mut fast_speed = None;
    let mut animations = Vec::new();
    for line in contents.lines() {
        let line = line.split(';').next().unwrap_or_default().trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_ascii_uppercase();
            continue;
        }
        let Some((action, filename)) = line.split_once('=') else {
            continue;
        };
        let action = action.trim().to_ascii_uppercase();
        let value = filename.trim().trim_matches('"');
        match section.as_str() {
            "ANIMATIONS" if !value.is_empty() && value.to_ascii_lowercase().ends_with(".flc") => {
                animations_raw.push((action, value.to_owned()));
            }
            "TIMING" if !value.is_empty() => {
                let duration = value.parse::<f32>().map_err(|error| {
                    format!(
                        "{}: invalid [Timing] value for {action}: {error}",
                        ini_path.display()
                    )
                })?;
                if !duration.is_finite() || duration < 0.0 {
                    return Err(format!(
                        "{}: invalid negative/nonfinite timing for {action}",
                        ini_path.display()
                    ));
                }
                timings.insert(action, duration);
            }
            "SPEED" if !value.is_empty() => {
                let speed = value.parse::<u32>().map_err(|error| {
                    format!(
                        "{}: invalid [Speed] value for {action}: {error}",
                        ini_path.display()
                    )
                })?;
                if speed == 0 {
                    return Err(format!(
                        "{}: [Speed] value for {action} must be positive",
                        ini_path.display()
                    ));
                }
                let slot = match action.as_str() {
                    "NORMAL SPEED" => &mut normal_speed,
                    "FAST SPEED" => &mut fast_speed,
                    _ => continue,
                };
                if slot.is_some_and(|existing| existing != speed) {
                    return Err(format!(
                        "{}: conflicting [Speed] values for {action}",
                        ini_path.display()
                    ));
                }
                *slot = Some(speed);
            }
            _ => {}
        }
    }
    for (action, filename) in animations_raw {
        let suffix = unit_animation_suffix(&action);
        let path = theme.resolve_unit_file(&ini_path, &filename)?;
        animations.push(UnitAnimationSource {
            role: format!("{role}{suffix}"),
            path,
            timing_seconds: timings.get(&action).copied(),
            normal_speed,
            fast_speed,
        });
    }
    if !animations.iter().any(|animation| animation.role == role) {
        return Err(format!(
            "{}: [Animations] has no DEFAULT FLC",
            ini_path.display()
        ));
    }
    Ok(animations)
}

fn unit_animation_suffix(action: &str) -> String {
    let suffix = match action {
        "DEFAULT" => return String::new(),
        "BLANK" => "blank",
        "WALK" => "walk",
        "RUN" => "run",
        "ATTACK1" => "attack",
        "ATTACK2" => "attack-2",
        "ATTACK3" => "attack-3",
        "DEFEND" => "defend",
        "DEATH" => "death",
        "DEAD" => "dead",
        "FORTIFY" => "fortify",
        "FORTIFYHOLD" => "fortify-hold",
        "FIDGET" => "fidget",
        "VICTORY" => "victory",
        "TURNLEFT" => "turn-left",
        "TURNRIGHT" => "turn-right",
        "BUILD" => "build",
        "ROAD" => "road",
        "STOP_AT_LAST_FRAME" => "stop-at-last-frame",
        "PAUSEROAD" => "pause-road",
        "PAUSEMINE" => "pause-mine",
        "PAUSEIRRIGATE" => "pause-irrigate",
        "MINE" => "mine",
        "IRRIGATE" => "irrigate",
        "FORTRESS" => "fortress",
        "CAPTURE" => "capture",
        "FOREST" => "forest",
        "PAUSEFOREST" => "pause-forest",
        "JUNGLE" => "jungle",
        _ => return format!("-{}", action.to_ascii_lowercase().replace('_', "-")),
    };
    format!("-{suffix}")
}

fn read_theme_image(
    path: &Path,
    transparent_indices: &[u8],
) -> Result<(PathBuf, Image), std::io::Error> {
    let mut candidates = vec![path.to_owned()];
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
    {
        candidates.push(path.with_extension("pcx"));
        candidates.push(path.with_extension("PCX"));
    }
    // Original Civ III installations mix both filename and extension casing.
    // Resolve the basename case-insensitively and accept either supported image
    // format so source trees load consistently on case-sensitive filesystems.
    if let Some(parent) = path.parent() {
        if let (Some(wanted), Ok(entries)) = (path.file_stem(), fs::read_dir(parent)) {
            let wanted = wanted.to_string_lossy();
            for entry in entries.flatten() {
                let candidate = entry.path();
                let is_supported = candidate.extension().is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("pcx") || ext.eq_ignore_ascii_case("png")
                });
                if is_supported
                    && candidate
                        .file_stem()
                        .is_some_and(|stem| stem.to_string_lossy().eq_ignore_ascii_case(&wanted))
                {
                    candidates.push(candidate);
                }
            }
        }
    }
    let mut last_error = None;
    for candidate in candidates {
        match fs::read(&candidate) {
            Ok(bytes) => {
                let image = if candidate
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("pcx"))
                {
                    decode_pcx(&bytes, transparent_indices).map_err(|message| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, message)
                    })?
                } else {
                    Image::from_file_with_format(&bytes, Some(ImageFormat::Png)).map_err(
                        |error| {
                            std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
                        },
                    )?
                };
                return Ok((candidate, image));
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error
        .unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "file not found")))
}

/// Decode the 8-bit, one-plane, indexed RLE PCX form used by Civ III terrain mods.
fn decode_pcx(bytes: &[u8], transparent_indices: &[u8]) -> Result<Image, String> {
    if bytes.len() < 897 || bytes[0] != 0x0a || bytes[2] != 1 || bytes[3] != 8 || bytes[65] != 1 {
        return Err("unsupported PCX: expected 8-bit, single-plane RLE image".into());
    }
    let le = |offset: usize| u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
    let xmin = le(4) as usize;
    let ymin = le(6) as usize;
    let xmax = le(8) as usize;
    let ymax = le(10) as usize;
    let width = xmax
        .checked_sub(xmin)
        .and_then(|v| v.checked_add(1))
        .ok_or("invalid PCX bounds")?;
    let height = ymax
        .checked_sub(ymin)
        .and_then(|v| v.checked_add(1))
        .ok_or("invalid PCX bounds")?;
    let bytes_per_line = le(66) as usize;
    if width == 0
        || height == 0
        || width > u16::MAX as usize
        || height > u16::MAX as usize
        || bytes_per_line < width
    {
        return Err("invalid PCX dimensions".into());
    }
    let palette_at = bytes.len() - 769;
    if bytes[palette_at] != 0x0c {
        return Err("PCX is missing its 256-color palette".into());
    }
    let pixel_count = width.checked_mul(height).ok_or("PCX dimensions overflow")?;
    let needed = bytes_per_line
        .checked_mul(height)
        .ok_or("PCX dimensions overflow")?;
    let mut indices = Vec::with_capacity(needed);
    let mut cursor = 128;
    while indices.len() < needed {
        if cursor >= palette_at {
            return Err("truncated PCX RLE data".into());
        }
        let value = bytes[cursor];
        cursor += 1;
        let (count, color) = if value & 0xc0 == 0xc0 {
            if cursor >= palette_at {
                return Err("truncated PCX run".into());
            }
            let count = (value & 0x3f) as usize;
            let color = bytes[cursor];
            cursor += 1;
            (count, color)
        } else {
            (1, value)
        };
        if count == 0 || indices.len() + count > needed {
            return Err("invalid PCX RLE run length".into());
        }
        indices.resize(indices.len() + count, color);
    }
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    for y in 0..height {
        for &index in &indices[y * bytes_per_line..y * bytes_per_line + width] {
            let p = palette_at + 1 + usize::from(index) * 3;
            let (red, green, blue) = (bytes[p], bytes[p + 1], bytes[p + 2]);
            let alpha = if transparent_indices.contains(&index)
                || (red > 200 && green < 40 && blue > 200)
            {
                0 // Civ III PCX magenta color key.
            } else {
                255
            };
            rgba.extend_from_slice(&[red, green, blue, alpha]);
        }
    }
    Ok(Image {
        width: width as u16,
        height: height as u16,
        bytes: rgba,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Junction {
    ground: [Ground; 4],
    variant: u8,
    straight_ice: bool,
}

struct SheetCell<'a> {
    image: &'a Image,
    offset: usize,
}

impl<'a> SheetCell<'a> {
    fn new(image: &'a Image, row: usize, col: usize) -> Self {
        Self {
            image,
            offset: row * HEIGHT * image.width() + col * WIDTH,
        }
    }

    fn lookup(image: &'a Image, [n, e, s, w]: [usize; 4]) -> Self {
        Self::new(image, e + 3 * s, n + 3 * w)
    }

    fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        self.image.get_image_data()[self.offset + y * self.image.width() + x]
    }
}

struct GroundArt {
    land: Vec<Image>,
    water: Image,
    sea: Image,
    ocean: Image,
    polar: Image,
}

fn render_ground(tile: &Tile) -> Ground {
    use Terrain::*;
    if TerrainBase::is_feature_terrain(tile.terrain) {
        return match tile.base_terrain {
            TerrainBase::Grassland => Ground::Grass,
            TerrainBase::Plains => Ground::Plains,
            TerrainBase::Desert => Ground::Desert,
            TerrainBase::Tundra => Ground::Tundra,
            TerrainBase::Coast => Ground::Coast,
            TerrainBase::Sea => Ground::Sea,
            TerrainBase::Ocean => Ground::Ocean,
            TerrainBase::Ice => Ground::Ice,
        };
    }
    match tile.terrain {
        Glacier => Ground::Ice,
        Tundra => Ground::Tundra,
        Coast | Lake => Ground::Coast,
        Sea => Ground::Sea,
        Ocean => Ground::Ocean,
        FloodPlains | Desert => Ground::Desert,
        Plains => Ground::Plains,
        Grassland | GrasslandShield => Ground::Grass,
        Forest | Jungle | Hills | Mountains | Marsh | Volcano => unreachable!(),
    }
}

fn smooth(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn mix(a: [u8; 4], b: [u8; 4], weight: f32) -> [u8; 4] {
    std::array::from_fn(|i| {
        (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * weight).round() as u8
    })
}

fn water_depth(ground: Ground) -> usize {
    match ground {
        Coast => 0,
        Sea => 1,
        _ => 2,
    }
}

fn is_water(ground: Ground) -> bool {
    matches!(ground, Coast | Sea | Ocean)
}

fn ice_row(ground: [Ground; 4]) -> Option<usize> {
    match ground.map(|tile| tile == Ice) {
        [true, true, false, true] => Some(0),
        [true, false, false, false] => Some(1),
        [false, true, true, true] => Some(2),
        [false, false, true, false] => Some(3),
        _ => None,
    }
}

impl GroundArt {
    fn land_sheet(&self, ground: [Ground; 4], tip: Option<Ground>) -> usize {
        if tip.is_none()
            && ground.contains(&Coast)
            && ground.contains(&Grass)
            && ground.iter().all(|&tile| tile == Grass || tile == Coast)
        {
            return 5;
        }
        let mut best = (0, 0);
        for (index, (_, _, codes)) in LAND_SHEETS.iter().enumerate() {
            let score = ground.iter().filter(|tile| codes.contains(tile)).count()
                + 5 * usize::from(tip.is_some_and(|tile| codes.contains(&tile)));
            if score > best.1 {
                best = (index, score);
            }
        }
        best.0
    }

    fn rasterize(&self, key: Junction) -> Image {
        let ground = key.ground;
        let variant = usize::from(key.variant);
        let pure_water = ground.iter().all(|&tile| is_water(tile));
        let pure_ice = ground == [Ice; 4];
        let ice_over_water = ground.iter().all(|&tile| is_water(tile) || tile == Ice);
        let depths = ground.map(water_depth);
        let ice = SheetCell::new(&self.polar, 0, variant);
        let ocean = SheetCell::lookup(&self.water, depths);
        let mut patch = [None, None, None, None];
        let mut tundra_blend = false;
        let main = if pure_ice {
            SheetCell::new(&self.polar, 0, variant)
        } else if key.straight_ice {
            let row = ice_row(ground).expect("straight polar front has a sheet row");
            SheetCell::new(
                &self.polar,
                row,
                if row == 2 { 1 + variant % 7 } else { variant },
            )
        } else if pure_water && depths == [1; 4] {
            SheetCell::new(&self.sea, variant, (variant * 5) % 9)
        } else if pure_water && depths == [2; 4] {
            SheetCell::new(&self.ocean, variant, (variant * 5) % 9)
        } else if ice_over_water {
            SheetCell::lookup(&self.water, depths)
        } else {
            let land = ground.map(|tile| match tile {
                Ice => Tundra,
                Sea | Ocean => Coast,
                tile => tile,
            });
            tundra_blend = land.contains(&Tundra)
                && land
                    .iter()
                    .any(|tile| ![Tundra, Grass, Coast].contains(tile));
            let index = if tundra_blend {
                4
            } else {
                self.land_sheet(land, None)
            };
            let codes = LAND_SHEETS[index].2;
            for (i, tile) in land.into_iter().enumerate() {
                if !codes.contains(&tile) {
                    let other = self.land_sheet(land, Some(tile));
                    let other_codes = LAND_SHEETS[other].2;
                    let fallback = other_codes
                        .iter()
                        .position(|&code| code == tile)
                        .unwrap_or(0);
                    patch[i] = Some(SheetCell::lookup(
                        &self.land[other],
                        land.map(|t| {
                            other_codes
                                .iter()
                                .position(|&code| code == t)
                                .unwrap_or(fallback)
                        }),
                    ));
                }
            }
            SheetCell::lookup(
                &self.land[index],
                land.map(|tile| {
                    codes
                        .iter()
                        .position(|&code| code == tile)
                        .unwrap_or(usize::from(tundra_blend))
                }),
            )
        };
        let blend_ice = ground.contains(&Ice) && !pure_ice && !key.straight_ice;
        let mut image = Image::gen_image_color(CELL_W as u16, CELL_H as u16, BLANK);
        for (index, pixel) in image.get_image_data_mut().iter_mut().enumerate() {
            let y = (index / CELL_W).saturating_sub(1).clamp(1, HEIGHT - 2);
            // Extend the diamond colors into its transparent perimeter. The
            // mesh clips to the diamond, so fractional zoom samples real art.
            let half = (2 * y.min(HEIGHT - 1 - y)).saturating_sub(1).max(1);
            let x = (index % CELL_W)
                .saturating_sub(1)
                .clamp(64 - half, 63 + half);
            let dx = (x as f32 + 0.5) / 64.0 - 1.0;
            let dy = (y as f32 + 0.5) / 32.0 - 1.0;
            let polar_y = y.min(HEIGHT - 1 - y);
            let mut color = main.pixel(x, if pure_ice { polar_y } else { y });
            let signed_tip = [-dy - dx.abs(), dx - dy.abs(), dy - dx.abs(), -dx - dy.abs()];
            for (i, distance) in signed_tip.into_iter().enumerate() {
                if let Some(other) = &patch[i] {
                    let width = if tundra_blend { 0.28 } else { 0.08 };
                    color = mix(color, other.pixel(x, y), smooth(0.5 + distance / width));
                } else if blend_ice && !ice_over_water && is_water(ground[i]) && depths[i] != 0 {
                    color = mix(color, ocean.pixel(x, y), smooth(0.5 + distance / 0.28));
                }
            }
            if blend_ice {
                let u = ((dx + dy + 1.0) * 0.5).clamp(0.0, 1.0);
                let v = ((dy - dx + 1.0) * 0.5).clamp(0.0, 1.0);
                let weights = [(1.0 - u) * (1.0 - v), u * (1.0 - v), u * v, (1.0 - u) * v];
                let coverage: f32 = weights
                    .into_iter()
                    .zip(ground)
                    .filter(|(_, tile)| *tile == Ice)
                    .map(|(weight, _)| weight)
                    .sum();
                // The noise repeats on the junction lattice, including at
                // both wrap seams. Its vertical phase alternates by row.
                let xx = x as f32 + 0.5;
                let yy = y as f32 + 0.5 + f32::from(key.variant & 1) * 32.0;
                let noise = 0.026
                    * (xx * std::f32::consts::TAU / 32.0 + yy * std::f32::consts::TAU / 64.0).sin()
                    + 0.019
                        * (xx * std::f32::consts::TAU / 64.0 - yy * std::f32::consts::TAU / 32.0)
                            .sin();
                color = mix(
                    color,
                    ice.pixel(x, polar_y),
                    smooth((coverage + noise - 0.44) / 0.12),
                );
            }
            *pixel = [color[0], color[1], color[2], 255];
        }
        image
    }
}

#[derive(Default)]
struct Atlas {
    cells: HashMap<Junction, usize>,
    pages: Vec<Mesh>,
}

pub(super) struct Civ3 {
    art: GroundArt,
    atlas: RefCell<Atlas>,
    overlays: Vec<Option<Texture2D>>,
    irrigation: Vec<Option<Texture2D>>,
    roads: Option<Texture2D>,
    railroads: Option<Texture2D>,
    terrain_buildings: Option<Texture2D>,
    pollution: Option<Texture2D>,
    craters: Option<Texture2D>,
    optional_improvements: HashMap<&'static str, Texture2D>,
    unit_art: RefCell<HashMap<String, Civ3UnitArt>>,
    unit_cursor: Civ3UnitCursor,
    unit_animation_sources: HashMap<String, UnitAnimationSource>,
    animation_pacing: super::unit_animation_pacing::UnitAnimationPacing,
    river_delta: Texture2D,
    river_mountain: Texture2D,
    resources: Option<ResourceArt>,
    cities: HashMap<&'static str, Texture2D>,
    civilization_city_styles: HashMap<String, String>,
}

struct Civ3UnitCursor {
    texture: Texture2D,
    width: f32,
    height: f32,
    original_width: f32,
    original_height: f32,
    offset_left: f32,
    offset_top: f32,
    frame_count: usize,
    frame_columns: usize,
    frame_duration_seconds: f32,
}

struct Civ3UnitArt {
    base: Texture2D,
    tint: Texture2D,
    width: f32,
    height: f32,
    frame_count: usize,
    frame_columns: usize,
    direction_count: usize,
    original_width: f32,
    original_height: f32,
    offset_left: f32,
    offset_top: f32,
    timing_seconds: Option<f32>,
    #[allow(dead_code)]
    normal_speed: Option<u32>,
    #[allow(dead_code)]
    fast_speed: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)] // Retain the full Civ III action vocabulary for INI animation mappings.
pub(super) enum Civ3UnitAction {
    #[default]
    Default,
    Blank,
    Walk,
    Run,
    Attack,
    Attack2,
    Attack3,
    Defend,
    Death,
    Dead,
    Victory,
    Fidget,
    Fortify,
    FortifyHold,
    TurnLeft,
    TurnRight,
    StopAtLastFrame,
    Build,
    Road,
    PauseRoad,
    Mine,
    PauseMine,
    Irrigate,
    PauseIrrigate,
    Forest,
    PauseForest,
    Jungle,
    Plant,
    Fortress,
    Capture,
}

impl Civ3UnitAction {
    fn pacing_key(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Blank => "blank",
            Self::Walk => "walk",
            Self::Run => "run",
            Self::Attack => "attack",
            Self::Attack2 => "attack2",
            Self::Attack3 => "attack3",
            Self::Defend => "defend",
            Self::Death => "death",
            Self::Dead => "dead",
            Self::Victory => "victory",
            Self::Fidget => "fidget",
            Self::Fortify => "fortify",
            Self::FortifyHold => "fortifyhold",
            Self::TurnLeft => "turnleft",
            Self::TurnRight => "turnright",
            Self::StopAtLastFrame => "stop_at_last_frame",
            Self::Build => "build",
            Self::Road => "road",
            Self::PauseRoad => "pauseroad",
            Self::Mine => "mine",
            Self::PauseMine => "pausemine",
            Self::Irrigate => "irrigate",
            Self::PauseIrrigate => "pauseirrigate",
            Self::Forest => "forest",
            Self::PauseForest => "pauseforest",
            Self::Jungle => "jungle",
            Self::Plant => "plant",
            Self::Fortress => "fortress",
            Self::Capture => "capture",
        }
    }

    fn is_work_action(self) -> bool {
        matches!(
            self,
            Self::Build
                | Self::Road
                | Self::PauseRoad
                | Self::Mine
                | Self::PauseMine
                | Self::Irrigate
                | Self::PauseIrrigate
                | Self::Forest
                | Self::PauseForest
                | Self::Jungle
                | Self::Plant
                | Self::Fortress
        )
    }

    fn role_suffix(self) -> Option<&'static str> {
        match self {
            Self::Default => None,
            Self::Blank => Some("-blank"),
            Self::Walk => Some("-walk"),
            Self::Run => Some("-run"),
            Self::Attack => Some("-attack"),
            Self::Attack2 => Some("-attack-2"),
            Self::Attack3 => Some("-attack-3"),
            Self::Defend => Some("-defend"),
            Self::Death => Some("-death"),
            Self::Dead => Some("-dead"),
            Self::Victory => Some("-victory"),
            Self::Fidget => Some("-fidget"),
            Self::Fortify => Some("-fortify"),
            Self::FortifyHold => Some("-fortify-hold"),
            Self::TurnLeft => Some("-turn-left"),
            Self::TurnRight => Some("-turn-right"),
            Self::StopAtLastFrame => Some("-stop-at-last-frame"),
            Self::Build => Some("-build"),
            Self::Road => Some("-road"),
            Self::PauseRoad => Some("-pause-road"),
            Self::Mine => Some("-mine"),
            Self::PauseMine => Some("-pause-mine"),
            Self::Irrigate => Some("-irrigate"),
            Self::PauseIrrigate => Some("-pause-irrigate"),
            Self::Forest => Some("-forest"),
            Self::PauseForest => Some("-pause-forest"),
            Self::Jungle => Some("-jungle"),
            Self::Plant => Some("-forest"),
            Self::Fortress => Some("-fortress"),
            Self::Capture => Some("-capture"),
        }
    }
}

const CITY_STYLES: [(&str, &str); 5] = [
    ("asian", "rASIAN"),
    ("american", "rAMER"),
    ("european", "rEURO"),
    ("middle_eastern", "rMIDEAST"),
    ("roman", "rROMAN"),
];

fn civilization_city_style<'a>(
    civilization_name: &str,
    mappings: &'a HashMap<String, String>,
) -> Option<&'a str> {
    let slug = civilization_name.to_ascii_lowercase().replace(' ', "-");
    mappings.get(&slug).map(String::as_str)
}

impl Civ3 {
    pub(super) fn load(path: &Path) -> Result<Self, String> {
        let animation_pacing = super::unit_animation_pacing::UnitAnimationPacing::load()?;
        let theme = Theme::read(path)?;
        let required = |key: &str, file: &str, size| {
            theme
                .image(key, &format!("Terrain/{file}.png"), size, true)
                .map(Option::unwrap)
        };
        let land = LAND_SHEETS
            .iter()
            .map(|(key, file, _)| required(key, file, (1152, 576)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut polar = required("polar", "polarICEcaps-final", (1024, 256))?;
        prepare_keyed_overlay(&mut polar);
        let art = GroundArt {
            land,
            water: required("water", "wCSO", (1152, 576))?,
            sea: required("sea", "wSSS", (1152, 576))?,
            ocean: required("ocean", "wOOO", (1152, 576))?,
            polar,
        };
        let optional = |key, file, size| -> Result<_, String> {
            Ok(theme
                .image(key, &format!("Terrain/{file}.png"), size, false)?
                .map(|mut image| {
                    if matches!(key, "floodplains" | "volcano") {
                        prepare_keyed_overlay(&mut image);
                    }
                    let texture = Texture2D::from_image(&image);
                    texture.set_filter(FilterMode::Nearest);
                    texture
                }))
        };
        let overlays = OVERLAYS
            .iter()
            .map(|(key, file, w, h, _)| optional(key, file, (*w, *h)))
            .collect::<Result<_, _>>()?;
        let irrigation = IRRIGATION
            .iter()
            .map(|(key, file)| optional(key, file, (512, 256)))
            .collect::<Result<_, _>>()?;
        let mut fallback_theme = None;
        let route_texture = |key: &str,
                             file: &str,
                             fallback_theme: &mut Option<Theme>|
         -> Result<Option<Texture2D>, String> {
            let default = format!("Terrain/{file}.png");
            let image = theme.image(key, &default, (2048, 1024), false)?;
            let image = if image.is_some() {
                image
            } else {
                if fallback_theme.is_none() {
                    *fallback_theme = Some(Theme::read(&locate_default_art()?)?);
                }
                fallback_theme
                    .as_ref()
                    .unwrap()
                    .image(key, &default, (2048, 1024), false)?
            };
            Ok(image.map(|image| {
                let texture = Texture2D::from_image(&image);
                texture.set_filter(FilterMode::Nearest);
                texture
            }))
        };
        let roads = route_texture("roads", "roads", &mut fallback_theme)?;
        let railroads = route_texture("railroads", "railroads", &mut fallback_theme)?;
        let improvement_texture = |key: &str,
                                   file: &str,
                                   size: (u16, u16),
                                   fallback_theme: &mut Option<Theme>|
         -> Result<Option<Texture2D>, String> {
            let default = format!("Terrain/{file}.png");
            let image = theme.image(key, &default, size, false)?;
            let image = if image.is_some() {
                image
            } else {
                if fallback_theme.is_none() {
                    *fallback_theme = Some(Theme::read(&locate_default_art()?)?);
                }
                fallback_theme
                    .as_ref()
                    .unwrap()
                    .image(key, &default, size, false)?
            };
            Ok(image.map(|mut image| {
                prepare_keyed_overlay(&mut image);
                let texture = Texture2D::from_image(&image);
                texture.set_filter(FilterMode::Nearest);
                texture
            }))
        };
        let terrain_buildings = improvement_texture(
            "terrain-buildings",
            "TerrainBuildings",
            (384, 256),
            &mut fallback_theme,
        )?;
        let pollution =
            improvement_texture("pollution", "pollution", (640, 320), &mut fallback_theme)?;
        let craters = improvement_texture("craters", "craters", (640, 320), &mut fallback_theme)?;
        let mut optional_improvements = HashMap::new();
        for (role, file) in OPTIONAL_IMPROVEMENTS {
            if let Some(mut image) = theme.image(
                role,
                &format!("Terrain/{file}.png"),
                (WIDTH as u16, HEIGHT as u16),
                false,
            )? {
                prepare_keyed_overlay(&mut image);
                let texture = Texture2D::from_image(&image);
                texture.set_filter(FilterMode::Nearest);
                optional_improvements.insert(role, texture);
            }
        }
        let river_texture = |key, file| -> Result<Texture2D, String> {
            let image = required(key, file, (512, 256))?;
            let texture = Texture2D::from_image(&image);
            texture.set_filter(FilterMode::Nearest);
            Ok(texture)
        };
        let river_delta = river_texture("river-delta", "deltaRivers")?;
        let river_mountain = river_texture("river-mountain", "mtnRivers")?;
        let resources = theme.image("resources", "resources.png", (300, 300), false)?;
        let shadows = theme.image(
            "resource-shadows",
            "resources_shadows.png",
            (300, 300),
            false,
        )?;
        let resources = resources.map(|mut image| {
            let present = prepare_resource_sheet(&mut image, shadows.as_ref());
            let texture = Texture2D::from_image(&image);
            texture.set_filter(FilterMode::Nearest);
            ResourceArt { texture, present }
        });
        let mut cities = HashMap::new();
        for (style, file) in CITY_STYLES {
            let key = format!("city-{style}");
            let default = format!("Cities/{file}.png");
            let mut image = theme
                .image(&key, &default, (501, 380), true)?
                .expect("required city image");
            if image.width() < 501 || image.height() < 380 {
                return Err(format!(
                    "{}: expected at least 501×380 pixels",
                    path.display()
                ));
            }
            // Civ III city sheets use the bright magenta key color for transparency.
            for pixel in image.get_image_data_mut() {
                if pixel[0] > 200 && pixel[1] < 40 && pixel[2] > 200 {
                    pixel[3] = 0;
                }
            }
            let texture = Texture2D::from_image(&image);
            texture.set_filter(FilterMode::Nearest);
            cities.insert(style, texture);
        }
        let unit_paths = theme
            .paths
            .iter()
            .filter(|(role, _)| role.starts_with("unit-"))
            .collect::<Vec<_>>();
        let mut resolved_unit_paths = HashMap::<String, UnitAnimationSource>::new();
        for (role, relative_path) in unit_paths.iter().copied().filter(|(_, path)| {
            !Path::new(path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ini"))
        }) {
            resolved_unit_paths.insert(
                role.clone(),
                UnitAnimationSource {
                    role: role.clone(),
                    path: theme.resolve_theme_file(relative_path)?,
                    timing_seconds: None,
                    normal_speed: None,
                    fast_speed: None,
                },
            );
        }
        for (role, relative_path) in unit_paths.iter().copied().filter(|(_, path)| {
            Path::new(path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ini"))
        }) {
            for animation in expand_unit_animation_ini(&theme, role, relative_path)? {
                resolved_unit_paths
                    .entry(animation.role.clone())
                    .or_insert(animation);
            }
        }
        let cursor_path = locate_source_root()?.join("Art/Animations/Cursor/Cursor.flc");
        let cursor_bytes = fs::read(&cursor_path)
            .map_err(|error| format!("{}: {error}", cursor_path.display()))?;
        let cursor_flic = super::flc::decode_flic(&cursor_bytes)
            .map_err(|error| format!("{}: {error}", cursor_path.display()))?;
        let (mut cursor_image, cursor_tint) = cursor_flic
            .image_atlases()
            .map_err(|error| format!("{}: {error}", cursor_path.display()))?;
        // The cursor artwork is monochrome white. Civ III uses the unit shadow
        // palette range (240..=255) for some arrow pixels, but those indices
        // should fade the white cursor rather than draw black unit shadows.
        for (base, tint) in cursor_image
            .get_image_data_mut()
            .iter_mut()
            .zip(cursor_tint.get_image_data().iter())
        {
            let alpha = base[3].max(tint[3]);
            base[0] = 255;
            base[1] = 255;
            base[2] = 255;
            base[3] = alpha;
        }
        let cursor_texture = Texture2D::from_image(&cursor_image);
        cursor_texture.set_filter(FilterMode::Nearest);
        let cursor_frame_count = cursor_flic.animations[0].len();
        let unit_cursor = Civ3UnitCursor {
            texture: cursor_texture,
            width: cursor_flic.width as f32,
            height: cursor_flic.height as f32,
            original_width: cursor_flic.original_width.max(cursor_flic.width) as f32,
            original_height: cursor_flic.original_height.max(cursor_flic.height) as f32,
            offset_left: cursor_flic.offset_left as f32,
            offset_top: cursor_flic.offset_top as f32,
            frame_count: cursor_frame_count,
            frame_columns: cursor_frame_count.min(4096 / cursor_flic.width),
            frame_duration_seconds: (cursor_flic.animation_speed as f32 / 1000.0).max(0.001),
        };
        Ok(Self {
            art,
            atlas: RefCell::default(),
            overlays,
            irrigation,
            roads,
            railroads,
            terrain_buildings,
            pollution,
            craters,
            optional_improvements,
            unit_art: RefCell::default(),
            unit_cursor,
            unit_animation_sources: resolved_unit_paths,
            animation_pacing,
            river_delta,
            river_mountain,
            resources,
            cities,
            civilization_city_styles: theme.civilization_city_styles.clone(),
        })
    }

    pub(super) fn draw_unit_cursor(&self, center: Vec2, zoom: f32) {
        let cursor = &self.unit_cursor;
        let elapsed = get_time() as f32;
        let frame =
            ((elapsed / cursor.frame_duration_seconds).floor() as usize) % cursor.frame_count;
        let source = Rect::new(
            (frame % cursor.frame_columns) as f32 * cursor.width,
            (frame / cursor.frame_columns) as f32 * cursor.height,
            cursor.width,
            cursor.height,
        );
        // Fit the visible FLC frame inside one tile. The FLC's 240px canvas
        // includes margins, so use the frame size for scale and the recorded
        // offsets only when positioning it on that canvas.
        let scale = zoom * (TILE_W / cursor.width).min(TILE_H / cursor.height);
        let size = vec2(cursor.width * scale, cursor.height * scale);
        let position = center
            - vec2(
                cursor.original_width * scale,
                cursor.original_height * scale,
            ) * 0.5
            + vec2(cursor.offset_left * scale, cursor.offset_top * scale);
        draw_texture_ex(
            &cursor.texture,
            position.x,
            position.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(size),
                source: Some(source),
                ..Default::default()
            },
        );
    }

    pub(super) fn city_texture(&self, style: &str) -> &Texture2D {
        self.cities.get(style).unwrap_or(&self.cities["european"])
    }

    pub(super) fn fidget_pacing(
        &self,
        kind: UnitKind,
    ) -> super::unit_animation_pacing::FidgetPacing {
        self.animation_pacing.fidget(kind)
    }

    pub(super) fn city_texture_at(&self, style: u8, civilization_id: Option<&str>) -> &Texture2D {
        if let Some(civilization) = civilization_id.and_then(crate::rules::civilizations::by_id) {
            if let Some(style) =
                civilization_city_style(&civilization.name, &self.civilization_city_styles)
            {
                return self.city_texture(style);
            }
        }
        let style = ["asian", "american", "european", "middle_eastern", "roman"]
            .get(style as usize)
            .copied()
            .unwrap_or("european");
        self.city_texture(style)
    }

    fn unit_animation_role(&self, kind: UnitKind, era: usize) -> String {
        let role = super::sprite_metadata::unit_art_role(kind);
        let era_suffix = [
            "ancient-times",
            "middle-ages",
            "industrial-ages",
            "modern-times",
        ]
        .get(era)
        .copied()
        .unwrap_or("modern-times");
        let era_role = format!("{role}-era-{era_suffix}");
        if self.unit_animation_sources.contains_key(&era_role) {
            era_role
        } else {
            role
        }
    }

    fn load_unit_animation(&self, role: &str) -> Result<(), String> {
        if self.unit_art.borrow().contains_key(role) {
            return Ok(());
        }
        let source = self
            .unit_animation_sources
            .get(role)
            .ok_or_else(|| format!("no Civ III animation is mapped for {role}"))?;
        let bytes = fs::read(&source.path)
            .map_err(|error| format!("{}: {error}", source.path.display()))?;
        let flic = super::flc::decode_flic(&bytes)
            .map_err(|error| format!("{}: {error}", source.path.display()))?;
        let (base_image, tint_image) = flic
            .image_atlases()
            .map_err(|error| format!("{}: {error}", source.path.display()))?;
        let base = Texture2D::from_image(&base_image);
        let tint = Texture2D::from_image(&tint_image);
        base.set_filter(FilterMode::Nearest);
        tint.set_filter(FilterMode::Nearest);
        self.unit_art.borrow_mut().insert(
            role.to_owned(),
            Civ3UnitArt {
                base,
                tint,
                width: flic.width as f32,
                height: flic.height as f32,
                frame_count: flic.animations[0].len(),
                frame_columns: flic.animations[0].len().min(4096 / flic.width),
                direction_count: flic.animations.len(),
                original_width: flic.original_width.max(flic.width) as f32,
                original_height: flic.original_height.max(flic.height) as f32,
                offset_left: flic.offset_left as f32,
                offset_top: flic.offset_top as f32,
                timing_seconds: source.timing_seconds,
                normal_speed: source.normal_speed,
                fast_speed: source.fast_speed,
            },
        );
        Ok(())
    }

    /// Draw a compact unit portrait using the same source FLC art as the map.
    pub(super) fn draw_unit_icon(
        &self,
        kind: UnitKind,
        owner: usize,
        civilization_id: Option<&str>,
        era: usize,
        center: Vec2,
        bounds: Vec2,
        has_moves: bool,
    ) -> bool {
        let role = self.unit_animation_role(kind, era);
        if self.load_unit_animation(&role).is_err() {
            return false;
        }
        let (width, height) = {
            let loaded = self.unit_art.borrow();
            let Some(art) = loaded.get(&role) else {
                return false;
            };
            (art.original_width.max(1.0), art.original_height.max(1.0))
        };
        let scale = (bounds.x / width).min(bounds.y / height);
        let zoom = scale * (64.0 / TILE_W);
        self.draw_unit(
            kind,
            owner,
            civilization_id,
            2,
            era,
            Civ3UnitAction::Default,
            0.0,
            None,
            center,
            zoom,
            has_moves,
        )
    }

    /// Draw the first frame for a unit's current direction from a mod FLC.
    pub(super) fn draw_unit(
        &self,
        kind: UnitKind,
        owner: usize,
        civilization_id: Option<&str>,
        facing: u8,
        era: usize,
        action: Civ3UnitAction,
        progress: f32,
        playback_duration_seconds: Option<f32>,
        center: Vec2,
        zoom: f32,
        has_moves: bool,
    ) -> bool {
        let role = self.unit_animation_role(kind, era);
        let action_role = action
            .role_suffix()
            .map_or_else(|| role.clone(), |suffix| format!("{role}{suffix}"));
        let alternate_role = match action {
            Civ3UnitAction::Attack2 | Civ3UnitAction::Attack3 => Some(format!("{role}-attack")),
            Civ3UnitAction::FortifyHold => Some(format!("{role}-fortify")),
            Civ3UnitAction::PauseRoad => Some(format!("{role}-road")),
            Civ3UnitAction::PauseMine => Some(format!("{role}-mine")),
            Civ3UnitAction::PauseIrrigate => Some(format!("{role}-irrigate")),
            Civ3UnitAction::PauseForest => Some(format!("{role}-forest")),
            Civ3UnitAction::Plant => Some(format!("{role}-jungle")),
            _ => None,
        };
        let selected_role = [
            Some(action_role.as_str()),
            alternate_role.as_deref(),
            Some(role.as_str()),
        ]
        .into_iter()
        .flatten()
        .find(|candidate| self.unit_animation_sources.contains_key(*candidate));
        let Some(selected_role) = selected_role else {
            return false;
        };
        if self.load_unit_animation(selected_role).is_err() {
            return false;
        }
        let loaded_unit_art = self.unit_art.borrow();
        let art = loaded_unit_art.get(selected_role).unwrap();
        let frame = if action == Civ3UnitAction::Default {
            0
        } else {
            let event_duration = playback_duration_seconds.unwrap_or(1.0).max(0.001);
            // Ambient fidgets are scheduled as one finite gesture by the world
            // renderer; don't replay the clip several times during that window.
            let source_cycle_duration = art
                .timing_seconds
                .filter(|seconds| *seconds > 0.0)
                .unwrap_or(event_duration);
            let cycle_duration = if action == Civ3UnitAction::Fidget {
                event_duration
            } else {
                source_cycle_duration
                    * self.animation_pacing.cycle_duration_multiplier(
                        kind,
                        action.pacing_key(),
                        action.is_work_action(),
                    )
            };
            let elapsed_cycles = progress.clamp(0.0, 1.0) * event_duration / cycle_duration;
            let cycle_progress = if action == Civ3UnitAction::StopAtLastFrame {
                elapsed_cycles.min(1.0)
            } else {
                elapsed_cycles.fract()
            };
            ((cycle_progress * art.frame_count as f32).floor() as usize)
                .min(art.frame_count.saturating_sub(1))
        };
        let direction = if usize::from(facing) < art.direction_count {
            usize::from(facing)
        } else if art.direction_count > 2 {
            2
        } else {
            0
        };
        let source = Rect::new(
            (frame % art.frame_columns) as f32 * art.width,
            (direction * art.frame_count.div_ceil(art.frame_columns) + frame / art.frame_columns)
                as f32
                * art.height,
            art.width,
            art.height,
        );
        let scale = zoom * (TILE_W / 64.0);
        let size = vec2(art.width * scale, art.height * scale);
        let position = center - vec2(art.original_width * scale, art.original_height * scale) * 0.5
            + vec2(art.offset_left * scale, art.offset_top * scale);
        let base_tint = if has_moves {
            WHITE
        } else {
            Color::new(0.62, 0.66, 0.68, 1.0)
        };
        let team = super::player_color(civilization_id, owner);
        let team_tint = if has_moves {
            team
        } else {
            Color::new(team.r * 0.62, team.g * 0.66, team.b * 0.68, team.a)
        };
        for (texture, tint) in [(&art.base, base_tint), (&art.tint, team_tint)] {
            draw_texture_ex(
                texture,
                position.x,
                position.y,
                tint,
                DrawTextureParams {
                    dest_size: Some(size),
                    source: Some(source),
                    ..Default::default()
                },
            );
        }
        true
    }

    pub(super) fn draw_river(&self, game: &Game, x: i32, y: i32, center: Vec2, size: Vec2) {
        use super::rivers::Civ3RiverSheet;

        let wrap = game.sim.map_wrap;
        let at = |direction| {
            wrap.civ3_neighbor(x, y, direction)
                .map(|(nx, ny)| game.sim.render_tile(nx, ny))
        };
        let Some(north) = at(1) else { return };
        let Some(east) = at(2) else { return };
        let Some(south) = at(3) else { return };
        let west = game.sim.render_tile(x, y);
        if !west.seen {
            return;
        }
        let edges = |tile: &Tile| if tile.seen { tile.river_edges } else { 0 };
        let Some(index) = super::rivers::civ3_junction_index(
            edges(north),
            edges(east),
            edges(south),
            edges(west),
        ) else {
            return;
        };
        let has_water = [north, east, south, west]
            .into_iter()
            .any(|tile| tile.seen && tile.terrain.is_water());
        let texture = match super::rivers::civ3_river_sheet(index, has_water) {
            Civ3RiverSheet::Delta => &self.river_delta,
            Civ3RiverSheet::Mountain => &self.river_mountain,
        };
        draw_texture_ex(
            texture,
            center.x,
            center.y - size.y * 0.5,
            WHITE,
            DrawTextureParams {
                dest_size: Some(size),
                source: Some(super::rivers::civ3_river_sprite(index)),
                ..Default::default()
            },
        );
    }

    pub(super) fn draw_road(&self, index: usize, center: Vec2, size: Vec2) -> bool {
        let Some(texture) = &self.roads else {
            return false;
        };
        draw_sprite(texture, center, size, route_source(index));
        true
    }

    pub(super) fn draw_railroad(&self, index: usize, center: Vec2, size: Vec2) -> bool {
        let Some(texture) = &self.railroads else {
            return false;
        };
        draw_sprite(texture, center, size, route_source(index));
        true
    }

    /// Terrain improvement art uses the original TerrainBuildings sheet.
    pub(super) fn draw_terrain_building(
        &self,
        kind: TerrainBuilding,
        center: Vec2,
        size: Vec2,
    ) -> bool {
        let Some(texture) = &self.terrain_buildings else {
            return false;
        };
        draw_sprite(texture, center, size, building_source(kind));
        true
    }

    /// Optional one-cell Civ III mod art for improvements not present in the
    /// original game or selected theme terrain sheet.
    pub(super) fn draw_optional_improvement(
        &self,
        role: &'static str,
        center: Vec2,
        size: Vec2,
    ) -> bool {
        let Some(texture) = self.optional_improvements.get(role) else {
            return false;
        };
        draw_sprite(
            texture,
            center,
            size,
            Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        );
        true
    }

    /// The referenced pollution/crater sheets use 5x5 128x64 cells. The index
    /// comes from the neighbor mask and stable single-tile variant selection.
    pub(super) fn draw_contamination(
        &self,
        craters: bool,
        index: usize,
        center: Vec2,
        size: Vec2,
    ) -> bool {
        let texture = if craters {
            &self.craters
        } else {
            &self.pollution
        };
        let Some(texture) = texture else { return false };
        draw_sprite(
            texture,
            center,
            size,
            Rect::new(
                ((index % 5) * WIDTH) as f32,
                ((index / 5) * HEIGHT) as f32,
                WIDTH as f32,
                HEIGHT as f32,
            ),
        );
        true
    }

    fn draw_corner(&self, key: Junction, side: usize, center: Vec2, size: Vec2) {
        let mut atlas = self.atlas.borrow_mut();
        let next = atlas.cells.len();
        let index = *atlas.cells.entry(key).or_insert(next);
        let page = index / (PAGE * PAGE);
        let cell = index % (PAGE * PAGE);
        let (left, top) = (cell % PAGE * CELL_W, cell / PAGE * CELL_H);
        if page == atlas.pages.len() {
            let image =
                Image::gen_image_color((PAGE * CELL_W) as u16, (PAGE * CELL_H) as u16, BLANK);
            let texture = Texture2D::from_image(&image);
            texture.set_filter(FilterMode::Linear);
            atlas.pages.push(Mesh {
                vertices: vec![Vertex::new(0., 0., 0., 0., 0., WHITE); 4],
                indices: vec![0, 1, 2, 0, 2, 3],
                texture: Some(texture),
            });
        }
        let mesh = &mut atlas.pages[page];
        if index == next {
            mesh.texture.as_ref().unwrap().update_part(
                &self.art.rasterize(key),
                left as i32,
                top as i32,
                CELL_W as i32,
                CELL_H as i32,
            );
        }
        let (sx, sy) = SIDES[side];
        for (vertex, (mut x, mut y)) in
            mesh.vertices
                .iter_mut()
                .zip([(0.0, 0.0), (0.25, -0.25), (0.0, -0.5), (-0.25, -0.25)])
        {
            for _ in 0..side {
                (x, y) = (-y, x);
            }
            let u = (left as f32 + 1.0 + (0.5 + x - sx as f32 * 0.5) * WIDTH as f32)
                / (PAGE * CELL_W) as f32;
            let v = (top as f32 + 1.0 + (0.5 + y - sy as f32 * 0.5) * HEIGHT as f32)
                / (PAGE * CELL_H) as f32;
            *vertex = Vertex::new(
                center.x + x * size.x,
                center.y + y * size.y,
                0.0,
                u,
                v,
                WHITE,
            );
        }
        draw_mesh(mesh);
    }

    pub(super) fn draw_base(&self, game: &Game, x: i32, y: i32, center: Vec2, size: Vec2) {
        let wrap = game.sim.map_wrap;
        let px = 2 * x + y.rem_euclid(2);
        let fallback = if (0..wrap.height()).contains(&y) {
            render_ground(game.sim.render_tile(x, y))
        } else {
            Ice
        };
        let ground = |x: i32, y: i32| {
            wrap.normalize(x.div_euclid(2), y).map_or(Ice, |(nx, ny)| {
                let tile = game.sim.render_tile(nx, ny);
                if tile.seen {
                    render_ground(tile)
                } else {
                    fallback
                }
            })
        };
        for (side, (dx, dy)) in SIDES.into_iter().enumerate() {
            let (jx, jy) = (px + dx, y + dy);
            let tiles = SIDES.map(|(sx, sy)| ground(jx + sx, jy + sy));
            let straight_ice = ice_row(tiles).is_some()
                && tiles.iter().all(|&t| t == Ice || t == Ocean)
                && [-2, 2].into_iter().all(|offset| {
                    SIDES
                        .into_iter()
                        .zip(tiles)
                        .all(|((sx, sy), tile)| ground(jx + sx + offset, jy + sy) == tile)
                });
            let variant = if tiles.contains(&Ice) || tiles == [Ocean; 4] {
                let x = jx.rem_euclid(2 * wrap.width());
                let y = jy;
                // Even/odd junction rows select the matching texture noise phase.
                (2 * ((x.div_euclid(2) * 37 + y * 13).rem_euclid(4)) + y.rem_euclid(2)) as u8
            } else {
                0
            };
            self.draw_corner(
                Junction {
                    ground: tiles,
                    variant,
                    straight_ice,
                },
                side,
                center,
                size,
            );
        }
    }

    pub(super) fn draw_overlay(
        &self,
        terrain: Terrain,
        (x, y): (i32, i32),
        center: Vec2,
        size: Vec2,
    ) -> bool {
        if terrain == Terrain::GrasslandShield {
            // The supplied terrain sheets have no separate Grassland shield
            // sheet; Civ III depicts this yield variant with a central rocky
            // tuft. Recreate that small marker over the ordinary Grassland.
            let base_y = center.y + size.y * 0.05;
            let rock = Color::from_rgba(91, 83, 61, 255);
            let light = Color::from_rgba(171, 155, 108, 255);
            draw_ellipse(
                center.x - size.x * 0.04,
                base_y,
                size.x * 0.055,
                size.y * 0.035,
                0.0,
                rock,
            );
            draw_ellipse(
                center.x + size.x * 0.035,
                base_y - size.y * 0.01,
                size.x * 0.045,
                size.y * 0.03,
                0.0,
                light,
            );
            draw_ellipse(
                center.x + size.x * 0.08,
                base_y + size.y * 0.015,
                size.x * 0.04,
                size.y * 0.025,
                0.0,
                rock,
            );
            return true;
        }
        let (index, row, count) = match terrain {
            Terrain::Forest => (0, 6, 5),
            Terrain::Jungle => (0, 2, 6),
            Terrain::Marsh => (1, 2, 6),
            Terrain::Hills => (2, 0, 1),
            Terrain::Mountains => (3, 0, 1),
            Terrain::FloodPlains => (4, 0, 4),
            Terrain::Volcano => (5, 0, 4),
            _ => return true,
        };
        let Some(texture) = &self.overlays[index] else {
            return false;
        };
        let height = OVERLAYS[index].4 as f32;
        let col = (x * 37 + y * 13).rem_euclid(count) as f32;
        let dest = vec2(size.x, size.y * height / HEIGHT as f32);
        draw_sprite(
            texture,
            center + vec2(0.0, (size.y - dest.y) * 0.5),
            dest,
            Rect::new(
                col * WIDTH as f32,
                row as f32 * height,
                WIDTH as f32,
                height,
            ),
        );
        true
    }

    pub(super) fn draw_irrigation(
        &self,
        terrain: TerrainBase,
        center: Vec2,
        size: Vec2,
        index: usize,
    ) -> bool {
        let terrain_index = match terrain {
            TerrainBase::Plains => 1,
            TerrainBase::Desert => 2,
            TerrainBase::Tundra => 3,
            _ => 0,
        };
        let Some(texture) = &self.irrigation[terrain_index] else {
            return false;
        };
        draw_sprite(texture, center, size, irrigation_source(index));
        true
    }

    pub(super) fn draw_resource(&self, resource: Resource, center: Vec2, size: Vec2) -> bool {
        let Some(art) = &self.resources else {
            return false;
        };
        let (row, col) = resource_cell(resource);
        if !art.present[row][col] {
            return false;
        }
        let width = size.x * 0.44;
        draw_sprite(
            &art.texture,
            center - vec2(0.0, size.y * 0.05),
            vec2(width, width),
            Rect::new((col * 50 + 1) as f32, (row * 50 + 1) as f32, 49.0, 49.0),
        );
        true
    }
}

fn prepare_keyed_overlay(image: &mut Image) {
    for pixel in image.get_image_data_mut() {
        let magenta = pixel[0] > 200 && pixel[1] < 40 && pixel[2] > 200;
        let green = pixel[0] < 40 && pixel[1] > 200 && pixel[2] < 40;
        if magenta || green {
            pixel[3] = 0;
        }
    }
}

struct ResourceArt {
    texture: Texture2D,
    present: [[bool; 6]; 6],
}

fn resource_cell(resource: Resource) -> (usize, usize) {
    use Resource::*;
    match resource {
        Horses => (0, 0),
        Iron => (0, 1),
        Saltpeter => (0, 2),
        Coal => (0, 3),
        Oil => (0, 4),
        Rubber => (0, 5),
        Aluminum => (1, 0),
        Uranium => (1, 1),
        Wine => (1, 2),
        Furs => (1, 3),
        Dyes => (1, 4),
        Incense => (1, 5),
        Spice => (2, 0),
        Ivory => (2, 1),
        Silk => (2, 2),
        Gems => (2, 3),
        Whales => (2, 4),
        Game => (2, 5),
        Fish => (3, 0),
        Cattle => (3, 1),
        Wheat => (3, 2),
        Gold => (3, 3),
        TropicalFruit => (3, 4),
        Oasis => (3, 5),
        Sugar => (4, 0),
        Tobacco => (4, 1),
    }
}

fn prepare_resource_sheet(image: &mut Image, shadows: Option<&Image>) -> [[bool; 6]; 6] {
    let mut present = [[false; 6]; 6];
    let width = image.width();
    let pixels = image.get_image_data_mut();
    for y in 0..300 {
        for x in 0..300 {
            let pixel = &mut pixels[y * width + x];
            if x % 50 == 0 || y % 50 == 0 {
                // The original atlas grid is opaque. Clear its pixels before
                // upload so exact UV endpoints cannot sample a magenta line.
                *pixel = [0; 4];
            } else if pixel[3] != 0 {
                present[y / 50][x / 50] = true;
            } else if let Some(shadows) = shadows {
                let shadow = shadows.get_image_data()[y * shadows.width() + x];
                // Civ III stores the shadow mask in green. The red channel
                // carries inverse opacity, so draw it as a neutral black
                // shadow instead of tinting the resource sprite green.
                let opacity = 255u8.saturating_sub(shadow[0]);
                pixel.copy_from_slice(&[
                    0,
                    0,
                    0,
                    ((u16::from(opacity) * u16::from(shadow[3])) / 255) as u8,
                ]);
            }
        }
    }
    present
}

fn draw_sprite(texture: &Texture2D, center: Vec2, size: Vec2, source: Rect) {
    draw_texture_ex(
        texture,
        center.x - size.x * 0.5,
        center.y - size.y * 0.5,
        WHITE,
        DrawTextureParams {
            dest_size: Some(size),
            source: Some(source),
            ..Default::default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn irrigation_mask_selects_the_expected_cell_in_the_four_by_four_sheet() {
        assert_eq!(irrigation_source(0), Rect::new(0.0, 0.0, 128.0, 64.0));
        assert_eq!(irrigation_source(3), Rect::new(384.0, 0.0, 128.0, 64.0));
        assert_eq!(irrigation_source(4), Rect::new(0.0, 64.0, 128.0, 64.0));
        assert_eq!(irrigation_source(15), Rect::new(384.0, 192.0, 128.0, 64.0));
    }

    #[test]
    fn decodes_indexed_pcx_palette_and_scanline_padding() {
        let mut bytes = vec![0u8; 128];
        bytes[0] = 0x0a;
        bytes[2] = 1;
        bytes[3] = 8;
        bytes[8..10].copy_from_slice(&2u16.to_le_bytes()); // xmax: three pixels
        bytes[10..12].copy_from_slice(&0u16.to_le_bytes());
        bytes[65] = 1;
        bytes[66..68].copy_from_slice(&4u16.to_le_bytes()); // one padded scanline
        bytes.extend_from_slice(&[1, 2, 0xc1, 3, 4]);
        bytes.push(0x0c);
        let mut palette = vec![0u8; 768];
        palette[3..6].copy_from_slice(&[10, 20, 30]);
        palette[6..9].copy_from_slice(&[40, 50, 60]);
        palette[9..12].copy_from_slice(&[255, 0, 255]);
        bytes.extend_from_slice(&palette);

        let image = decode_pcx(&bytes, &DEFAULT_PCX_TRANSPARENT_INDICES).unwrap();
        assert_eq!((image.width, image.height), (3, 1));
        assert_eq!(
            image.get_image_data(),
            &[[10, 20, 30, 255], [40, 50, 60, 255], [255, 0, 255, 0]]
        );
    }

    #[test]
    fn pcx_transparency_can_use_palette_indices_with_nonmagenta_colors() {
        let mut bytes = vec![0u8; 128];
        bytes[0] = 0x0a;
        bytes[2] = 1;
        bytes[3] = 8;
        bytes[8..10].copy_from_slice(&0u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&0u16.to_le_bytes());
        bytes[65] = 1;
        bytes[66..68].copy_from_slice(&1u16.to_le_bytes());
        bytes.push(1);
        bytes.push(0x0c);
        let mut palette = vec![0u8; 768];
        palette[3..6].copy_from_slice(&[0, 255, 0]);
        bytes.extend_from_slice(&palette);

        let image = decode_pcx(&bytes, &[1]).unwrap();
        assert_eq!(image.get_image_data(), &[[0, 255, 0, 0]]);
    }

    #[test]
    fn rejects_non_indexed_pcx() {
        let mut bytes = vec![0u8; 897];
        bytes[0] = 0x0a;
        assert!(
            decode_pcx(&bytes, &DEFAULT_PCX_TRANSPARENT_INDICES)
                .unwrap_err()
                .contains("8-bit, single-plane")
        );
    }

    #[test]
    fn decodes_the_original_sn00py_civ3_theme_assets() {
        let art = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../source/civ3PTW/Scenarios/Sn00pys TerrainG/Art");

        let files = [
            "Terrain/XDGP.pcx",
            "Terrain/XDGC.pcx",
            "Terrain/XDPC.pcx",
            "Terrain/XPGC.pcx",
            "Terrain/XTGC.pcx",
            "Terrain/XGGC.pcx",
            "Terrain/wCSO.pcx",
            "Terrain/wSSS.pcx",
            "Terrain/wOOO.pcx",
            "Terrain/polarICEcaps-final.pcx",
            "Terrain/deltaRivers.pcx",
            "Terrain/mtnRivers.pcx",
            "Terrain/grassland forests.pcx",
            "Terrain/XHILLS.PCX",
            "Terrain/Mountains.pcx",
            "Terrain/floodplains.pcx",
            "Terrain/irrigation.pcx",
            "Terrain/irrigation PLAINS.pcx",
            "Terrain/irrigation DESETT.pcx",
            "Terrain/irrigation TUNDRA.pcx",
            "Cities/rASIAN.PCX",
            "Cities/rAMER.PCX",
            "Cities/rEURO.PCX",
            "Cities/rMIDEAST.PCX",
            "Cities/rROMAN.PCX",
            "resources.pcx",
            "resources_shadows.pcx",
        ];

        for relative_path in files {
            let path = art.join(relative_path);
            let bytes = fs::read(&path).unwrap_or_else(|error| {
                panic!(
                    "could not read reference Civ III asset {}: {error}",
                    path.display()
                )
            });
            let image = decode_pcx(&bytes, &DEFAULT_PCX_TRANSPARENT_INDICES)
                .unwrap_or_else(|error| panic!("could not decode {}: {error}", path.display()));
            assert!(image.width > 0 && image.height > 0, "{}", path.display());
        }
    }

    #[test]
    fn selects_original_default_and_all_bundled_snoopy_terrain_paths() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../source");
        let cases = [
            ("civ3", root.join("Conquests/Art")),
            (
                "snoopy-standard",
                root.join("civ3PTW/Scenarios/Sn00pys Terrain/Art"),
            ),
            (
                "snoopy-greener",
                root.join("civ3PTW/Scenarios/Sn00pys TerrainG/Art"),
            ),
            (
                "snoopy-extended",
                root.join("civ3PTW/Scenarios/Sn00pys TerrainX/Art"),
            ),
        ];
        for (name, expected) in cases {
            let selected = Tileset::select(name).unwrap();
            let Tileset::Civ3(path) = selected;
            assert_eq!(
                path.canonicalize().unwrap(),
                expected.canonicalize().unwrap()
            );
        }
    }

    #[test]
    fn every_selectable_tileset_resolves_required_terrain_sheets() {
        let cases = [
            "civ3",
            "snoopy-standard",
            "snoopy-greener",
            "snoopy-extended",
        ];
        let required = [
            ("desert-grass-plains", "XDGP", (1152, 576)),
            ("desert-grass-coast", "XDGC", (1152, 576)),
            ("desert-plains-coast", "XDPC", (1152, 576)),
            ("plains-grass-coast", "XPGC", (1152, 576)),
            ("tundra-grass-coast", "XTGC", (1152, 576)),
            ("grass-coast", "XGGC", (1152, 576)),
            ("water", "wCSO", (1152, 576)),
            ("sea", "wSSS", (1152, 576)),
            ("ocean", "wOOO", (1152, 576)),
            ("polar", "polarICEcaps-final", (1024, 256)),
            ("river-delta", "deltaRivers", (512, 256)),
            ("river-mountain", "mtnRivers", (512, 256)),
        ];
        for name in cases {
            let Tileset::Civ3(root) = Tileset::select(name).unwrap();
            let theme = Theme::read(&root).unwrap();
            for (role, file, size) in required {
                let image = theme
                    .image(role, &format!("Terrain/{file}.png"), size, true)
                    .unwrap_or_else(|error| panic!("{name} {role}: {error}"))
                    .unwrap();
                assert!(
                    image.width >= size.0 && image.height >= size.1,
                    "{name} {role}"
                );
            }
        }
    }

    #[test]
    fn theme_manifest_accepts_original_civ3_river_sheet_roles() {
        let directory = std::env::temp_dir().join(format!(
            "civ3-river-theme-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&directory).unwrap();
        let manifest = directory.join("tileset.conf");
        fs::write(
            &manifest,
            "river-delta = Terrain/deltaRivers.pcx\nriver-mountain = Terrain/mtnRivers.pcx\ntransparent-river-delta = 254,255\ntransparent-river-mountain = 254,255\n",
        )
        .unwrap();
        let theme = Theme::read(&directory).unwrap();
        assert_eq!(
            theme.paths.get("river-delta").map(String::as_str),
            Some("Terrain/deltaRivers.pcx")
        );
        assert_eq!(
            theme.paths.get("river-mountain").map(String::as_str),
            Some("Terrain/mtnRivers.pcx")
        );
        assert_eq!(theme.transparent_indices["river-delta"], [254, 255]);
        assert_eq!(theme.transparent_indices["river-mountain"], [254, 255]);
        fs::remove_file(manifest).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn theme_manifest_maps_civilizations_to_civ3_city_styles() {
        let directory = std::env::temp_dir().join(format!(
            "civ3-city-style-theme-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&directory).unwrap();
        let manifest = directory.join("tileset.conf");
        fs::write(
            &manifest,
            "civilization-style.romans = roman\ncivilization-style.greeks = european\ncivilization-style.egyptians = middle_eastern\n",
        )
        .unwrap();
        let theme = Theme::read(&directory).unwrap();
        assert_eq!(theme.civilization_city_styles["romans"], "roman");
        assert_eq!(theme.civilization_city_styles["greeks"], "european");
        assert_eq!(
            theme.civilization_city_styles["egyptians"],
            "middle_eastern"
        );
        assert_eq!(
            civilization_city_style("Romans", &theme.civilization_city_styles),
            Some("roman")
        );
        assert_eq!(
            civilization_city_style("Greeks", &theme.civilization_city_styles),
            Some("european")
        );

        fs::write(&manifest, "civilization-style.romans = nonexistent\n").unwrap();
        assert!(
            Theme::read(&directory)
                .err()
                .unwrap()
                .contains("civilization-style")
        );
        fs::remove_file(manifest).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn theme_manifest_accepts_pc_or_png_road_sheet_roles() {
        let directory = std::env::temp_dir().join(format!(
            "civ3-road-theme-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&directory).unwrap();
        let manifest = directory.join("tileset.conf");
        fs::write(
            &manifest,
            "roads = Terrain/roads.pcx\nrailroads = Terrain/railroads.png\n",
        )
        .unwrap();
        let theme = Theme::read(&directory).unwrap();
        assert_eq!(theme.paths["roads"], "Terrain/roads.pcx");
        assert_eq!(theme.paths["railroads"], "Terrain/railroads.png");
        fs::remove_file(manifest).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn theme_manifest_accepts_civ3_improvement_sheet_roles() {
        let directory = std::env::temp_dir().join(format!(
            "civ3-improvement-theme-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&directory).unwrap();
        let manifest = directory.join("tileset.conf");
        fs::write(
            &manifest,
            "terrain-buildings = Terrain/TerrainBuildings.pcx\npollution = Terrain/pollution.pcx\ncraters = Terrain/craters.png\nfarmland = Terrain/farmland.pcx\noil-well = Terrain/oil-well.pcx\noil-platform = Terrain/oil-platform.png\nairstrip = Terrain/airstrip.pcx\nairbase = Terrain/airbase.png\nunit-warrior = Units/Warrior/Default.flc\nunit-warrior-run = Units/Warrior/Run.flc\nunit-warrior-attack = Units/Warrior/Attack.flc\nunit-stealth-fighter = Units/Stealth Fighter/Default.flc\n",
        )
        .unwrap();
        let theme = Theme::read(&directory).unwrap();
        assert_eq!(
            theme.paths["terrain-buildings"],
            "Terrain/TerrainBuildings.pcx"
        );
        assert_eq!(theme.paths["pollution"], "Terrain/pollution.pcx");
        assert_eq!(theme.paths["craters"], "Terrain/craters.png");
        for (role, _) in OPTIONAL_IMPROVEMENTS {
            assert!(theme.paths.contains_key(role));
        }
        assert_eq!(theme.paths["unit-warrior"], "Units/Warrior/Default.flc");
        assert_eq!(theme.paths["unit-warrior-run"], "Units/Warrior/Run.flc");
        assert_eq!(
            theme.paths["unit-warrior-attack"],
            "Units/Warrior/Attack.flc"
        );
        assert_eq!(
            theme.paths["unit-stealth-fighter"],
            "Units/Stealth Fighter/Default.flc"
        );
        fs::remove_file(manifest).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn civ3_unit_ini_expands_all_states_timings_and_shared_windows_paths() {
        let directory = std::env::temp_dir().join(format!(
            "civ3-unit-ini-theme-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let unit_directory = directory.join("civ3PTW/Art/Units/Berserk");
        let shared_directory = directory.join("Art/Units/Longbowman");
        fs::create_dir_all(&unit_directory).unwrap();
        fs::create_dir_all(&shared_directory).unwrap();
        let ini = unit_directory.join("Berserk.INI");
        fs::write(
            &ini,
            "[Speed]\nFast Speed=225\n[Animations]\nDEFAULT=Default.flc\nRUN=Run.flc\nATTACK1=AttackA.flc\nATTACK2=..\\..\\..\\..\\Art\\Units\\Longbowman\\AttackB.FLC\nDEATH=Death.flc\nFIDGET=Fidget.flc\nCAPTURE=Capture.flc\n[Timing]\nDEFAULT=0.5\nRUN=0.25\nATTACK1=0.4\nATTACK2=0.75\nDEATH=0.8\nFIDGET=1.25\nCAPTURE=0.5\n",
        )
        .unwrap();
        for filename in [
            "Default.flc",
            "Run.flc",
            "AttackA.flc",
            "Death.flc",
            "Fidget.flc",
            "Capture.flc",
        ] {
            fs::write(unit_directory.join(filename), b"stub").unwrap();
        }
        let shared_attack = shared_directory.join("AttackB.flc");
        fs::write(&shared_attack, b"stub").unwrap();
        let theme = Theme::read(&directory.join("civ3PTW/Art")).unwrap();
        let animations =
            expand_unit_animation_ini(&theme, "unit-berserk", "Units/Berserk/Berserk.INI").unwrap();
        let by_role = animations
            .iter()
            .map(|animation| (animation.role.as_str(), animation))
            .collect::<HashMap<_, _>>();
        assert_eq!(by_role["unit-berserk"].timing_seconds, Some(0.5));
        assert_eq!(by_role["unit-berserk-run"].timing_seconds, Some(0.25));
        assert_eq!(by_role["unit-berserk-attack"].timing_seconds, Some(0.4));
        assert_eq!(by_role["unit-berserk-attack-2"].timing_seconds, Some(0.75));
        assert_eq!(by_role["unit-berserk-death"].timing_seconds, Some(0.8));
        assert_eq!(by_role["unit-berserk-fidget"].timing_seconds, Some(1.25));
        assert_eq!(by_role["unit-berserk-capture"].timing_seconds, Some(0.5));
        assert!(
            by_role["unit-berserk-attack-2"]
                .path
                .starts_with(&shared_directory)
        );
        assert!(by_role["unit-berserk-attack-2"].path.is_file());
        fs::remove_file(ini).unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn every_campaign_unit_ini_animation_resolves_in_the_installed_art_roots() {
        let theme = Theme::read(&locate_default_art().unwrap()).unwrap();
        assert_eq!(
            theme
                .paths
                .keys()
                .filter(|role| role.starts_with("unit-"))
                .count(),
            98,
            "all unit identities and era-specific Army/Leader INIs are mapped"
        );
        let source = theme.source_root.as_ref().unwrap();
        let unit_roots = [
            ("base", source.join("Art/Units")),
            ("ptw", source.join("civ3PTW/Art/Units")),
            ("conquests", source.join("Conquests/Art/Units")),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();
        let mut unit_count = 0;
        let mut ini_count = 0;
        let mut animation_count = 0;
        let mut decoded_files = std::collections::HashSet::new();
        for line in include_str!("../../data/rulesets/civ3_conquests/units.tsv")
            .lines()
            .skip(1)
        {
            let fields = line.split('\t').collect::<Vec<_>>();
            let role = format!("unit-{}", fields[0].replace('_', "-"));
            let folders = fields[31].split('|').collect::<Vec<_>>();
            let roots = fields[32].split('|').collect::<Vec<_>>();
            assert_eq!(folders.len(), roots.len(), "{}", fields[1]);
            unit_count += 1;
            for (folder, root) in folders.into_iter().zip(roots) {
                let directory = unit_roots[root].join(folder);
                let mut ini_files = fs::read_dir(&directory)
                    .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("ini"))
                    });
                let ini = ini_files.next().unwrap();
                assert!(ini_files.next().is_none(), "{}", directory.display());
                let animations = expand_unit_animation_ini(&theme, &role, ini.to_str().unwrap())
                    .unwrap_or_else(|error| panic!("{}: {error}", fields[1]));
                assert!(
                    animations.iter().any(|animation| animation.role == role),
                    "{}",
                    fields[1]
                );
                for animation in &animations {
                    if decoded_files.insert(animation.path.clone()) {
                        let bytes = fs::read(&animation.path).unwrap_or_else(|error| {
                            panic!("{}: {error}", animation.path.display())
                        });
                        let flic = super::flc::decode_flic(&bytes).unwrap_or_else(|error| {
                            panic!("{}: {error}", animation.path.display())
                        });
                        flic.image_atlases().unwrap_or_else(|error| {
                            panic!("{}: {error}", animation.path.display())
                        });
                    }
                }
                animation_count += animations.len();
                ini_count += 1;
            }
        }
        assert_eq!(unit_count, 92);
        assert_eq!(
            ini_count, 98,
            "Army and Leader each have four era-specific INIs"
        );
        assert!(
            animation_count > ini_count,
            "all nonempty declared actions should load"
        );
        assert!(decoded_files.len() > 300, "all campaign FLCs should decode");
    }

    #[test]
    fn original_improvement_sheets_match_reference_atlas_dimensions() {
        let theme = Theme::read(&locate_default_art().unwrap()).unwrap();
        for (name, file, size) in [
            ("terrain-buildings", "TerrainBuildings", (512, 256)),
            ("pollution", "pollution", (640, 320)),
            ("craters", "craters", (640, 320)),
        ] {
            let image = theme
                .image(name, &format!("Terrain/{file}.png"), size, true)
                .unwrap()
                .unwrap();
            assert_eq!((image.width, image.height), size, "{name}");
        }
        assert_eq!(
            building_source(TerrainBuilding::Fortress),
            Rect::new(0.0, 0.0, 128.0, 64.0)
        );
        assert_eq!(
            building_source(TerrainBuilding::Mine),
            Rect::new(256.0, 64.0, 128.0, 64.0)
        );
        assert_eq!(
            building_source(TerrainBuilding::BarbarianCamp),
            Rect::new(256.0, 0.0, 128.0, 64.0)
        );
    }

    #[test]
    fn original_route_sheets_preserve_the_sixteen_by_sixteen_cell_grid() {
        let theme = Theme::read(&locate_default_art().unwrap()).unwrap();
        for (name, dimensions) in [("roads.pcx", (2048, 1024)), ("railroads.pcx", (2048, 1088))] {
            let role = name.trim_end_matches(".pcx");
            let image = theme
                .image(role, &format!("Terrain/{role}.png"), (2048, 1024), true)
                .unwrap()
                .unwrap();
            assert_eq!((image.width, image.height), dimensions, "{role}");
        }
        assert_eq!(route_source(0), Rect::new(0.0, 0.0, 128.0, 64.0));
        assert_eq!(route_source(15), Rect::new(1920.0, 0.0, 128.0, 64.0));
        assert_eq!(route_source(16), Rect::new(0.0, 64.0, 128.0, 64.0));
        assert_eq!(route_source(255), Rect::new(1920.0, 960.0, 128.0, 64.0));
    }

    #[test]
    fn resources_keep_art_and_shadows_without_sampling_the_grid() {
        let mut image = Image::gen_image_color(300, 300, BLANK);
        let shadows = Image::gen_image_color(300, 300, BLACK);
        image.get_image_data_mut()[50] = [200, 0, 200, 255];
        image.get_image_data_mut()[301] = [23, 44, 55, 255];
        let present = prepare_resource_sheet(&mut image, Some(&shadows));
        assert!(present[0][0]);
        assert!(!present[0][1], "a shadow alone cannot supply an icon");
        let pixels = image.get_image_data();
        assert_eq!(pixels[50], [0; 4]);
        assert_eq!(pixels[350], [0; 4]);
        assert_eq!(pixels[301], [23, 44, 55, 255]);
        assert_eq!(pixels[302], [0, 0, 0, 255]);
    }

    #[test]
    fn civ3_optional_overlay_key_colors_are_transparent() {
        let mut image = Image::gen_image_color(2, 1, BLANK);
        image.get_image_data_mut()[0] = [255, 0, 255, 255];
        image.get_image_data_mut()[1] = [0, 255, 0, 255];
        prepare_keyed_overlay(&mut image);
        assert_eq!(image.get_image_data(), &[[255, 0, 255, 0], [0, 255, 0, 0]]);
    }

    #[test]
    fn original_civ3_sheet_supplies_each_resource_in_its_own_cell() {
        use Resource::*;
        let resources = [
            Horses,
            Iron,
            Saltpeter,
            Coal,
            Oil,
            Rubber,
            Aluminum,
            Uranium,
            Wine,
            Furs,
            Dyes,
            Incense,
            Spice,
            Ivory,
            Silk,
            Gems,
            Whales,
            Game,
            Fish,
            Cattle,
            Wheat,
            Gold,
            TropicalFruit,
            Oasis,
            Sugar,
            Tobacco,
        ];
        let theme = Theme::read(&locate_default_art().unwrap()).unwrap();
        let mut image = theme
            .image("resources", "resources.png", (300, 300), true)
            .unwrap()
            .unwrap();
        let present = prepare_resource_sheet(&mut image, None);
        let mut seen = std::collections::HashSet::new();
        for resource in resources {
            let (row, col) = resource_cell(resource);
            assert!(present[row][col], "{resource:?}");
            assert!(seen.insert((row, col)), "{resource:?}");
        }
    }
}
