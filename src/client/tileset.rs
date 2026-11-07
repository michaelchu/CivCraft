//! Civ III sheet decoding. Both map views keep the same projection and painter passes.
use super::*;
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
const CORNER_NEIGHBORS: [(i32, i32); 8] = [
    (1, -1),
    (1, 1),
    (-1, 1),
    (-1, -1),
    (2, 0),
    (-2, 0),
    (0, 2),
    (0, -2),
];

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

const LAND_SHEETS: [(&str, &str, [Ground; 3]); 6] = [
    ("desert-grass-plains", "XDGP", [Desert, Grass, Plains]),
    ("desert-grass-coast", "XDGC", [Desert, Grass, Coast]),
    ("desert-plains-coast", "XDPC", [Desert, Plains, Coast]),
    ("plains-grass-coast", "XPGC", [Plains, Grass, Coast]),
    ("tundra-grass-coast", "XTGC", [Tundra, Grass, Coast]),
    ("grass-coast", "XGGC", [Grass, Grass, Coast]),
];
const OVERLAYS: [(&str, &str, u16, u16, usize); 4] = [
    ("vegetation", "grassland forests (normal)", 768, 616, 88),
    ("marsh", "marsh", 768, 264, 88),
    ("hills", "XHILLS", 128, 72, 72),
    ("mountains", "Mountains", 128, 88, 88),
];
const IRRIGATION: [(&str, &str); 4] = [
    ("irrigation", "irrigation"),
    ("irrigation-plains", "irrigation PLAINS"),
    ("irrigation-desert", "irrigation DESETT"),
    ("irrigation-tundra", "irrigation TUNDRA"),
];

pub(crate) enum Tileset {
    Amplio2,
    Civ3(PathBuf),
}

impl Tileset {
    pub(crate) fn select(name: &str, theme: Option<&str>) -> Result<Self, String> {
        match (name, theme) {
            ("amplio2", None) => Ok(Self::Amplio2),
            ("civ3", Some(path)) => Ok(Self::Civ3(path.into())),
            ("civ3", None) => {
                let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
                let exe = std::env::current_exe().map_err(|error| error.to_string())?;
                cwd.ancestors()
                    .chain(exe.ancestors().skip(1))
                    .map(|parent| parent.join("tilesets/civ3"))
                    .find(|path| path.join("Terrain").is_dir())
                    .map(Self::Civ3)
                    .ok_or_else(|| {
                        "Civ III art not found. Install it under tilesets/civ3 or use --tileset-theme PATH.".into()
                    })
            }
            ("amplio2", Some(_)) => Err("--tileset-theme requires --tileset civ3".into()),
            _ => Err(format!(
                "Unknown tileset '{name}'. Available tilesets: amplio2, civ3"
            )),
        }
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Amplio2 => "amplio2",
            Self::Civ3(_) => "civ3",
        }
    }
}

struct Theme {
    root: PathBuf,
    paths: HashMap<String, String>,
}

impl Theme {
    fn read(path: &Path) -> Result<Self, String> {
        let (root, manifest) = if path.is_dir() {
            (path, path.join("tileset.conf"))
        } else {
            (path.parent().unwrap_or(Path::new(".")), path.to_owned())
        };
        let mut paths = HashMap::new();
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
                        || matches!(
                            key,
                            "water" | "ocean" | "polar" | "resources" | "resource-shadows"
                        )
                };
                if let Some((key, value)) = entry
                    && valid(key)
                    && !value.is_empty()
                    && !paths.contains_key(key)
                {
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
        Ok(Self {
            root: root.to_owned(),
            paths,
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
        let path = self
            .root
            .join(override_path.map_or(default, String::as_str));
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error)
                if !required
                    && override_path.is_none()
                    && error.kind() == std::io::ErrorKind::NotFound =>
            {
                return Ok(None);
            }
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        let image = Image::from_file_with_format(&bytes, Some(ImageFormat::Png))
            .map_err(|error| format!("{}: {error}", path.display()))?;
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
    ocean: Image,
    polar: Image,
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
    resources: Option<ResourceArt>,
}

impl Civ3 {
    pub(super) fn load(path: &Path) -> Result<Self, String> {
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
        let art = GroundArt {
            land,
            water: required("water", "wCSO", (1152, 576))?,
            ocean: required("ocean", "wOOO", (1152, 576))?,
            polar: required("polar", "polarICEcaps-final", (1024, 256))?,
        };
        let optional = |key, file, size| -> Result<_, String> {
            Ok(theme
                .image(key, &format!("Terrain/{file}.png"), size, false)?
                .map(|image| {
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
        Ok(Self {
            art,
            atlas: RefCell::default(),
            overlays,
            irrigation,
            resources,
        })
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
        let fallback = if (0..wrap.height()).contains(&y) || wrap.wraps_y() {
            game.sim.render_tile(x, y).terrain
        } else {
            Terrain::DeepOcean
        };
        let terrain = |x: i32, y: i32| {
            wrap.normalize(x.div_euclid(2), y)
                .map_or(Terrain::DeepOcean, |(nx, ny)| {
                    let tile = game.sim.render_tile(nx, ny);
                    if tile.seen { tile.terrain } else { fallback }
                })
        };
        let ground = |x: i32, y: i32| match terrain(x, y) {
            Terrain::Glacier => Ice,
            Terrain::Tundra => Tundra,
            Terrain::Plains => Plains,
            Terrain::Desert => Desert,
            t if t.is_water() => {
                if CORNER_NEIGHBORS.iter().any(|&(dx, dy)| {
                    let next = terrain(x + dx, y + dy);
                    !next.is_water() && next != Terrain::Glacier
                }) {
                    Coast
                } else if t == Terrain::DeepOcean {
                    Ocean
                } else {
                    Sea
                }
            }
            _ => Grass,
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
                let y = if wrap.wraps_y() {
                    jy.rem_euclid(wrap.height())
                } else {
                    jy
                };
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
        let (index, row, count) = match terrain {
            Terrain::Forest => (0, 6, 5),
            Terrain::Jungle => (0, 2, 6),
            Terrain::Swamp => (1, 2, 6),
            Terrain::Hills => (2, 0, 1),
            Terrain::Mountains => (3, 0, 1),
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

    pub(super) fn draw_irrigation(&self, terrain: Terrain, center: Vec2, size: Vec2) -> bool {
        let index = match terrain {
            Terrain::Plains => 1,
            Terrain::Desert => 2,
            Terrain::Tundra => 3,
            _ => 0,
        };
        let Some(texture) = &self.irrigation[index] else {
            return false;
        };
        draw_sprite(texture, center, size, Rect::new(384.0, 192.0, 128.0, 64.0));
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
                *pixel = shadows.get_image_data()[y * shadows.width() + x];
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
    fn bundled_civ3_sheet_supplies_each_resource_in_its_own_cell() {
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
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tilesets/civ3/resources.png");
        let mut image =
            Image::from_file_with_format(&fs::read(path).unwrap(), Some(ImageFormat::Png)).unwrap();
        let present = prepare_resource_sheet(&mut image, None);
        let mut seen = std::collections::HashSet::new();
        for resource in resources {
            let (row, col) = resource_cell(resource);
            assert!(present[row][col], "{resource:?}");
            assert!(seen.insert((row, col)), "{resource:?}");
        }
    }
}
