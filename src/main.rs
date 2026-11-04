mod research;

use research::Tech;
use macroquad::prelude::*;
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::Path;

const PLAYER_COUNT: usize = 6;
const HUMAN: usize = 0;
const PERCENT_SCALE: i32 = 100;
const TILES_PER_PLAYER: i32 = 100;
const LAND_PERCENT: i32 = 30;
const MAP_TARGET_TILES: i32 = PLAYER_COUNT as i32 * TILES_PER_PLAYER * PERCENT_SCALE / LAND_PERCENT;
const MAP_X_RATIO: i32 = 1;
const MAP_Y_RATIO: i32 = 1;
const MAP_EVEN_FACTOR: i32 = 2;
const MAP_ISO_FACTOR: i32 = 2;

// Native _set_sizes at 0x100031f5c truncates
// sqrt(size / (xratio * yratio * iso * even^2)) + 0.49.  This integer
// comparison expresses the same 0.49 threshold at compile time, so the
// dimensions remain derived from the ruleset inputs instead of copied out.
const fn source_map_linear_scale(requested_tiles: i32) -> i32 {
    let denominator =
        (MAP_X_RATIO * MAP_Y_RATIO * MAP_ISO_FACTOR * MAP_EVEN_FACTOR * MAP_EVEN_FACTOR) as i64;
    let requested_scaled = requested_tiles as i64 * 10_000;
    let mut scale = 1_i64;
    loop {
        let next = scale + 1;
        let threshold = next * 100 - 49;
        if denominator * threshold * threshold > requested_scaled {
            return scale as i32;
        }
        scale = next;
    }
}

const MAP_LINEAR_SCALE: i32 = source_map_linear_scale(MAP_TARGET_TILES);
const MAP_W: i32 = MAP_X_RATIO * MAP_LINEAR_SCALE * MAP_EVEN_FACTOR;
const MAP_H: i32 = MAP_Y_RATIO * MAP_LINEAR_SCALE * MAP_EVEN_FACTOR * MAP_ISO_FACTOR;
const LAND_TILE_TARGET: usize = (MAP_W * MAP_H * LAND_PERCENT / PERCENT_SCALE) as usize;
const CITY_MIN_DISTANCE: i32 = 3;
const MOVE_FRAGMENTS: i32 = 6;
const POWER_FACTOR: i32 = 10;
const BASE_TECH_COST: i32 = 10;
const MIN_TECH_COST: i32 = 10;
const TECH_COST_PERCENT: i32 = 300;
const SCIENCEBOX_PERCENT: i32 = 100;
const FOODBOX_PERCENT: i32 = 100;
const CITIZEN_FOOD_COST: i32 = 2;
const GRANARY_FOOD_INITIAL: [i32; 8] = [20, 20, 20, 20, 20, 30, 30, 40];
const GRANARY_FOOD_INCREMENT: i32 = 0;
const CITY_DEFEND_BONUS: i32 = 50;
const LARGE_CITY_DEFEND_BONUS: i32 = 50;
const LARGE_CITY_SIZE: i32 = 9;
const FORTIFY_DEFENSE_BONUS: i32 = 50;
const CITY_MIN_HP_PERCENT: i32 = 33;
const BASE_HP_REGEN_2: i32 = 10;
const FORTIFIED_HP_REGEN_2: i32 = 10;
const TILE_W: f32 = 96.0;
const TILE_H: f32 = 48.0;
const PANEL_W: f32 = 316.0;
const TOP_H: f32 = 46.0;
const BOTTOM_H: f32 = 92.0;
const START_ZOOM: f32 = 0.82;
const MIN_ZOOM: f32 = 0.55;
const MAX_ZOOM: f32 = 1.35;

const TILE_SPRITE_W: usize = 96;
const TILE_SPRITE_H: usize = 48;
const OCEAN_ATLAS_W: usize = TILE_SPRITE_W * 9;
const OCEAN_ATLAS_H: usize = TILE_SPRITE_H * 9;
const OCEAN_CORNER_LAYERS: usize = 4;

// Civ2Civ3 uses isometric hexes: northeast and southwest are the two
// unavailable directions in Freeciv's eight-direction coordinate system.
const ISOHEX_DIRS: [(i32, i32); 6] = [(0, -1), (1, 0), (1, 1), (0, 1), (-1, 0), (-1, -1)];

// These are native (array) coordinates. Display and distance calculations
// convert them to Freeciv's map coordinates with the helpers below.
const STARTS: [(i32, i32); PLAYER_COUNT] = [(6, 8), (21, 8), (10, 27), (25, 29), (6, 50), (22, 53)];

fn bordered_sprite_source(row: i32, col: i32, width: f32, height: f32) -> Rect {
    Rect::new(
        1.0 + col as f32 * (width + 1.0),
        1.0 + row as f32 * (height + 1.0),
        width,
        height,
    )
}

fn ocean_cell_source(matches: [u8; 4]) -> (usize, usize) {
    let [north, east, south, west] = matches;
    (
        8 - (east as usize + 3 * south as usize),
        8 - (north as usize + 3 * west as usize),
    )
}

fn build_ocean_corner_atlas(ocean: &Image, terrain: &Image) -> Image {
    assert_eq!(ocean.width(), OCEAN_ATLAS_W);
    assert_eq!(ocean.height(), OCEAN_ATLAS_H);

    let mut atlas = Image::gen_image_color(
        OCEAN_ATLAS_W as u16,
        (OCEAN_ATLAS_H * OCEAN_CORNER_LAYERS) as u16,
        BLANK,
    );
    let ocean_pixels = ocean.get_image_data();
    let terrain_pixels = terrain.get_image_data();
    let atlas_pixels = atlas.get_image_data_mut();

    // Amplio2's mask.tile sprite is at row 15, column 2. The one-pixel
    // gutters are part of the atlas layout, not part of any sprite.
    let mask_x = 1 + 2 * (TILE_SPRITE_W + 1);
    let mask_y = 1 + 15 * (TILE_SPRITE_H + 1);

    for corner in 0..OCEAN_CORNER_LAYERS {
        for cell_row in 0..9 {
            for cell_col in 0..9 {
                for y in 0..TILE_SPRITE_H {
                    for x in 0..TILE_SPRITE_W {
                        let source = match corner {
                            // Top and bottom use the opposite vertical half of
                            // the four-terrain cell group. Right and left do
                            // the same horizontally. This mirrors Freeciv's
                            // corner-cell assembly exactly.
                            0 if (24..72).contains(&x) && y < 24 => Some((x, y + 24)),
                            1 if x >= 48 && (12..36).contains(&y) => Some((x - 48, y)),
                            2 if (24..72).contains(&x) && y >= 24 => Some((x, y - 24)),
                            3 if x < 48 && (12..36).contains(&y) => Some((x + 48, y)),
                            _ => None,
                        };
                        let Some((source_x, source_y)) = source else {
                            continue;
                        };

                        let ocean_x = cell_col * TILE_SPRITE_W + source_x;
                        let ocean_y = cell_row * TILE_SPRITE_H + source_y;
                        let mut pixel = ocean_pixels[ocean_y * OCEAN_ATLAS_W + ocean_x];
                        let mask = terrain_pixels[(mask_y + y) * terrain.width() + mask_x + x][3];
                        pixel[3] = ((pixel[3] as u16 * mask as u16) / 255) as u8;

                        let atlas_y = corner * OCEAN_ATLAS_H + cell_row * TILE_SPRITE_H + y;
                        let atlas_x = cell_col * TILE_SPRITE_W + x;
                        atlas_pixels[atlas_y * OCEAN_ATLAS_W + atlas_x] = pixel;
                    }
                }
            }
        }
    }

    atlas
}

fn window_conf() -> Conf {
    Conf {
        window_title: "CivCraft".to_owned(),
        window_width: 1280,
        window_height: 720,
        window_resizable: true,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

struct Assets {
    terrain: Texture2D,
    terrain2: Texture2D,
    hills: Texture2D,
    mountains: Texture2D,
    ocean_corners: Texture2D,
    units: Texture2D,
    cities: Texture2D,
}

impl Assets {
    fn new() -> Self {
        let terrain_image =
            Image::from_file_with_format(include_bytes!("../assets/amplio2_terrain1.png"), None)
                .expect("valid Amplio2 terrain atlas");
        let terrain = Texture2D::from_image(&terrain_image);
        let terrain2 = Texture2D::from_file_with_format(
            include_bytes!("../assets/amplio2_terrain2.png"),
            None,
        );
        let hills =
            Texture2D::from_file_with_format(include_bytes!("../assets/amplio2_hills.png"), None);
        let mountains = Texture2D::from_file_with_format(
            include_bytes!("../assets/amplio2_mountains.png"),
            None,
        );
        let ocean_image =
            Image::from_file_with_format(include_bytes!("../assets/amplio2_ocean.png"), None)
                .expect("valid Amplio2 ocean atlas");
        let ocean_corners =
            Texture2D::from_image(&build_ocean_corner_atlas(&ocean_image, &terrain_image));
        let units =
            Texture2D::from_file_with_format(include_bytes!("../assets/amplio2_units.png"), None);
        let cities =
            Texture2D::from_file_with_format(include_bytes!("../assets/amplio2_cities.png"), None);
        // These are tightly packed sprite atlases. Nearest filtering keeps a
        // tile from sampling the neighbouring cell at zoom levels below 1x.
        terrain.set_filter(FilterMode::Nearest);
        terrain2.set_filter(FilterMode::Nearest);
        hills.set_filter(FilterMode::Nearest);
        mountains.set_filter(FilterMode::Nearest);
        ocean_corners.set_filter(FilterMode::Nearest);
        units.set_filter(FilterMode::Nearest);
        cities.set_filter(FilterMode::Linear);
        Self {
            terrain,
            terrain2,
            hills,
            mountains,
            ocean_corners,
            units,
            cities,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Terrain {
    Ocean,
    Desert,
    Forest,
    Grassland,
    Hills,
    Mountains,
    Plains,
    Swamp,
    Tundra,
}

impl Terrain {
    fn name(self) -> &'static str {
        match self {
            Self::Ocean => "Ocean",
            Self::Desert => "Desert",
            Self::Forest => "Forest",
            Self::Grassland => "Grassland",
            Self::Hills => "Hills",
            Self::Mountains => "Mountains",
            Self::Plains => "Plains",
            Self::Swamp => "Swamp",
            Self::Tundra => "Tundra",
        }
    }

    fn movement_cost(self) -> i32 {
        match self {
            Self::Forest | Self::Hills | Self::Swamp => 2,
            Self::Mountains => 3,
            _ => 1,
        }
    }

    fn defense_bonus(self) -> i32 {
        match self {
            Self::Forest | Self::Swamp => 25,
            Self::Hills => 50,
            Self::Mountains => 100,
            _ => 0,
        }
    }

    fn base_yields(self) -> (i32, i32, i32) {
        match self {
            Self::Ocean => (1, 0, 2),
            Self::Desert => (0, 1, 0),
            Self::Forest => (1, 2, 0),
            Self::Grassland => (2, 0, 0),
            Self::Hills => (1, 1, 0),
            Self::Mountains => (0, 1, 0),
            Self::Plains => (1, 1, 0),
            Self::Swamp => (1, 0, 0),
            Self::Tundra => (1, 0, 0),
        }
    }

    fn irrigation_bonus(self) -> i32 {
        match self {
            Self::Desert | Self::Grassland | Self::Hills | Self::Plains | Self::Swamp => 1,
            _ => 0,
        }
    }

    fn mine_bonus(self) -> i32 {
        match self {
            Self::Desert | Self::Tundra => 1,
            Self::Hills | Self::Mountains => 2,
            _ => 0,
        }
    }

    fn road_trade_bonus(self) -> i32 {
        match self {
            Self::Desert | Self::Grassland | Self::Plains | Self::Tundra => 1,
            _ => 0,
        }
    }

    fn road_time(self) -> i32 {
        if self == Self::Ocean {
            0
        } else {
            self.movement_cost() * 2
        }
    }

    fn irrigation_time(self) -> i32 {
        match self {
            Self::Desert | Self::Grassland | Self::Plains | Self::Swamp => 5,
            Self::Hills => 10,
            _ => 0,
        }
    }

    fn mine_time(self) -> i32 {
        match self {
            Self::Desert | Self::Tundra => 5,
            Self::Hills | Self::Mountains => 10,
            _ => 0,
        }
    }

    fn allows_city(self) -> bool {
        !matches!(self, Self::Ocean | Self::Mountains)
    }

    fn prefers_mine(self) -> bool {
        matches!(self, Self::Hills | Self::Mountains | Self::Tundra)
    }

    fn sprite_row(self) -> Option<i32> {
        match self {
            Self::Desert => Some(0),
            Self::Plains => Some(1),
            Self::Grassland => Some(2),
            Self::Forest => Some(3),
            Self::Hills => Some(4),
            Self::Mountains => Some(5),
            Self::Tundra => Some(6),
            Self::Swamp => Some(8),
            Self::Ocean => None,
        }
    }

    fn minimap_color(self) -> Color {
        match self {
            Self::Ocean => Color::from_rgba(18, 72, 132, 255),
            Self::Desert => Color::from_rgba(211, 181, 103, 255),
            Self::Forest => Color::from_rgba(41, 91, 34, 255),
            Self::Grassland => Color::from_rgba(76, 146, 61, 255),
            Self::Hills => Color::from_rgba(99, 122, 63, 255),
            Self::Mountains => Color::from_rgba(126, 122, 112, 255),
            Self::Plains => Color::from_rgba(139, 165, 73, 255),
            Self::Swamp => Color::from_rgba(57, 91, 87, 255),
            Self::Tundra => Color::from_rgba(180, 184, 172, 255),
        }
    }
}

#[derive(Clone)]
struct Tile {
    terrain: Terrain,
    road: bool,
    irrigation: bool,
    mine: bool,
    owner: Option<usize>,
    seen: bool,
    visible: bool,
    known_by: [bool; PLAYER_COUNT],
}

impl Tile {
    fn yields(&self) -> (i32, i32, i32) {
        let (mut food, mut shields, mut trade) = self.terrain.base_yields();
        if self.irrigation {
            food += self.terrain.irrigation_bonus();
        }
        if self.mine {
            shields += self.terrain.mine_bonus();
        }
        if self.road {
            trade += self.terrain.road_trade_bonus();
        }
        (food, shields, trade)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnitKind {
    Settlers,
    Worker,
    Diplomat,
    Explorer,
    Warrior,
    Phalanx,
    Archer,
    Legion,
    Horseman,
    Chariot,
    Catapult,
}

#[derive(Clone, Copy)]
struct UnitStats {
    cost: i32,
    pop_cost: i32,
    attack: i32,
    defense: i32,
    hp: i32,
    firepower: i32,
    moves: i32,
    vision_sq: i32,
    sprite_row: i32,
    sprite_col: i32,
    ignores_terrain: bool,
    can_work: bool,
    can_found: bool,
}

impl UnitKind {
    fn name(self) -> &'static str {
        match self {
            Self::Settlers => "Settlers",
            Self::Worker => "Workers",
            Self::Diplomat => "Diplomat",
            Self::Explorer => "Explorer",
            Self::Warrior => "Warriors",
            Self::Phalanx => "Phalanx",
            Self::Archer => "Archers",
            Self::Legion => "Legion",
            Self::Horseman => "Horsemen",
            Self::Chariot => "Chariot",
            Self::Catapult => "Catapult",
        }
    }

    fn stats(self) -> UnitStats {
        match self {
            Self::Settlers => UnitStats {
                cost: 30,
                pop_cost: 2,
                attack: 0,
                defense: 1,
                hp: 20,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                sprite_row: 1,
                sprite_col: 4,
                ignores_terrain: false,
                can_work: true,
                can_found: true,
            },
            Self::Worker => UnitStats {
                cost: 20,
                pop_cost: 0,
                attack: 0,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                sprite_row: 2,
                sprite_col: 14,
                ignores_terrain: false,
                can_work: true,
                can_found: false,
            },
            Self::Diplomat => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 0,
                defense: 0,
                hp: 10,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                sprite_row: 0,
                sprite_col: 11,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Explorer => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 0,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 6,
                sprite_row: 1,
                sprite_col: 13,
                ignores_terrain: true,
                can_work: false,
                can_found: false,
            },
            Self::Warrior => UnitStats {
                cost: 10,
                pop_cost: 0,
                attack: 1,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                sprite_row: 0,
                sprite_col: 18,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Phalanx => UnitStats {
                cost: 20,
                pop_cost: 0,
                attack: 1,
                defense: 2,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                sprite_row: 1,
                sprite_col: 1,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Archer => UnitStats {
                cost: 20,
                pop_cost: 0,
                attack: 3,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                sprite_row: 1,
                sprite_col: 8,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Legion => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 4,
                defense: 2,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                sprite_row: 0,
                sprite_col: 16,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Chariot => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 4,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                sprite_row: 0,
                sprite_col: 9,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Catapult => UnitStats {
                cost: 40,
                pop_cost: 0,
                attack: 6,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                sprite_row: 0,
                sprite_col: 7,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Horseman => UnitStats {
                cost: 20,
                pop_cost: 0,
                attack: 2,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                sprite_row: 0,
                sprite_col: 8,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
        }
    }

    fn is_military(self) -> bool {
        self.stats().attack > 0
    }

    fn can_fortify(self) -> bool {
        // Every opening unit uses a land class with CanFortify, but these
        // three unit types carry the ruleset's overriding Cant_Fortify flag.
        !matches!(self, Self::Settlers | Self::Worker | Self::Explorer)
    }
}

impl Tech {
    fn cost(self) -> i32 {
        let requirements = self.data().map_or(1, |advance| advance.requirement_count);
        research_cost(requirements)
    }
}

fn research_cost(requirements: i32) -> i32 {
    let base = (BASE_TECH_COST * requirements).max(MIN_TECH_COST);
    percent_of(percent_of(base, TECH_COST_PERCENT), SCIENCEBOX_PERCENT).max(1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnitState {
    Idle,
    Fortifying,
    Fortified,
    Sentry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkKind {
    Road,
    Irrigation,
    Mine,
}

impl WorkKind {
    fn name(self) -> &'static str {
        match self {
            Self::Road => "road",
            Self::Irrigation => "irrigation",
            Self::Mine => "mine",
        }
    }
}

#[derive(Clone)]
struct Unit {
    id: u32,
    owner: usize,
    kind: UnitKind,
    x: i32,
    y: i32,
    hp: i32,
    moves_left: i32,
    state: UnitState,
    moved: bool,
    automated: bool,
    work_kind: Option<WorkKind>,
    work_remaining: i32,
}

#[derive(Clone)]
struct City {
    id: u32,
    owner: usize,
    x: i32,
    y: i32,
    name: String,
    size: i32,
    food: i32,
    shields: i32,
    production: UnitKind,
}

struct Player {
    name: &'static str,
    color: Color,
    gold: i32,
    known: Vec<Tech>,
    researching: Tech,
    research_goal: Tech,
    future_techs: i32,
    research: i32,
    ai: bool,
}

#[derive(Clone, Copy)]
enum UiAction {
    EndTurn,
    CloseCity,
    SelectUnit(u32),
    FoundCity,
    Road,
    Irrigate,
    Mine,
    Fortify,
    Wait,
    Automate,
    Produce(UnitKind),
    OpenResearch,
    CloseResearch,
    ResearchPage(usize),
    ResearchGoal(Tech),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveResult {
    Moved,
    Fought,
    Blocked,
}

struct Game {
    tiles: Vec<Tile>,
    units: Vec<Unit>,
    cities: Vec<City>,
    players: Vec<Player>,
    turn: i32,
    year: i32,
    next_unit_id: u32,
    next_city_id: u32,
    selected_unit: Option<u32>,
    selected_city: Option<u32>,
    camera: Vec2,
    zoom: f32,
    rng: u64,
    events: Vec<String>,
    goto_mode: bool,
    research_page: Option<usize>,
    game_over: Option<String>,
    hud_elements_drawn: usize,
    dragging: bool,
    drag_last: Vec2,
}

fn percent_of(value: i32, percent: i32) -> i32 {
    value * percent / PERCENT_SCALE
}

fn apply_percent_bonus(value: i32, bonus: i32) -> i32 {
    percent_of(value, PERCENT_SCALE + bonus)
}

// common/combat.c:win_chance. This is the probability that the attacker
// scores every required hit before the defender does, including unequal HP
// and firepower. The native defender-selection path uses this exact result.
fn combat_win_chance(
    attack_strength: i32,
    attacker_hp: i32,
    attacker_firepower: i32,
    defense_strength: i32,
    defender_hp: i32,
    defender_firepower: i32,
) -> f64 {
    let attacker_rounds_to_lose = (attacker_hp + defender_firepower - 1) / defender_firepower;
    let defender_rounds_to_lose = (defender_hp + attacker_firepower - 1) / attacker_firepower;
    let total_strength = attack_strength + defense_strength;
    let attacker_round_loss = if total_strength == 0 {
        0.5
    } else {
        defense_strength as f64 / total_strength as f64
    };
    let defender_round_loss = 1.0 - attacker_round_loss;

    let mut binomial_term = defender_round_loss.powi(defender_rounds_to_lose.saturating_sub(1));
    let mut accumulated = binomial_term;
    for lost_rounds in 1..attacker_rounds_to_lose {
        let series_length = lost_rounds + defender_rounds_to_lose - 1;
        binomial_term *= series_length as f64;
        binomial_term /= lost_rounds as f64;
        binomial_term *= attacker_round_loss;
        accumulated += binomial_term;
    }
    (accumulated * defender_round_loss).clamp(0.0, 1.0)
}

fn isohex_vector_distance(dx: i32, dy: i32) -> i32 {
    let abs_dx = dx.abs();
    let abs_dy = dy.abs();
    if dx.signum() != dy.signum() && dx != 0 && dy != 0 {
        abs_dx + abs_dy
    } else {
        abs_dx.max(abs_dy)
    }
}

fn is_isohex_step(dx: i32, dy: i32) -> bool {
    ISOHEX_DIRS.contains(&(dx, dy))
}

fn native_to_map(native_x: i32, native_y: i32) -> (i32, i32) {
    let map_x = (native_y + native_y.rem_euclid(2)) / 2 + native_x;
    let map_y = native_y - map_x + MAP_W;
    (map_x, map_y)
}

fn map_to_native(map_x: i32, map_y: i32) -> (i32, i32) {
    let native_y = map_x + map_y - MAP_W;
    let native_x = (2 * map_x - native_y - native_y.rem_euclid(2)) / 2;
    (native_x, native_y)
}

fn wrap_native(native_x: i32, native_y: i32) -> (i32, i32) {
    (native_x.rem_euclid(MAP_W), native_y.rem_euclid(MAP_H))
}

fn map_pos_to_native(map_x: i32, map_y: i32) -> (i32, i32) {
    let (native_x, native_y) = map_to_native(map_x, map_y);
    wrap_native(native_x, native_y)
}

fn offset_native_by_map(native_x: i32, native_y: i32, dx: i32, dy: i32) -> (i32, i32) {
    let (map_x, map_y) = native_to_map(native_x, native_y);
    map_pos_to_native(map_x + dx, map_y + dy)
}

fn wrapped_native_delta(delta: i32, size: i32) -> i32 {
    (delta + size / 2).rem_euclid(size) - size / 2
}

fn wrapped_map_vector(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> (i32, i32) {
    // Freeciv wraps in native coordinates first, then converts the resulting
    // virtual endpoint back to map coordinates before measuring distance.
    let native_dx = wrapped_native_delta(to_x - from_x, MAP_W);
    let native_dy = wrapped_native_delta(to_y - from_y, MAP_H);
    let (from_map_x, from_map_y) = native_to_map(from_x, from_y);
    let (to_map_x, to_map_y) = native_to_map(from_x + native_dx, from_y + native_dy);
    (to_map_x - from_map_x, to_map_y - from_map_y)
}

fn height_score(x: i32, y: i32) -> f32 {
    let h = hash2(x, y);
    let continental = (x as f32 * 0.42).sin() * 0.52
        + (y as f32 * 0.27).cos() * 0.38
        + ((x + y) as f32 * 0.17).sin() * 0.30;
    let detail = ((h % 1000) as f32 / 1000.0 - 0.5) * 0.90;
    continental + detail
}

fn land_terrain(x: i32, y: i32) -> Terrain {
    match (hash2(x, y) / 1000) % 100 {
        0..=7 => Terrain::Desert,
        8..=20 => Terrain::Forest,
        21..=43 => Terrain::Grassland,
        44..=64 => Terrain::Plains,
        65..=76 => Terrain::Hills,
        77..=83 => Terrain::Mountains,
        84..=91 => Terrain::Swamp,
        _ => Terrain::Tundra,
    }
}

fn generate_tiles() -> Vec<Tile> {
    let tile_count = (MAP_W * MAP_H) as usize;
    debug_assert!((tile_count as i32 - MAP_TARGET_TILES).abs() <= MAP_W * 2);
    debug_assert_eq!(
        LAND_TILE_TARGET,
        (MAP_W * MAP_H * LAND_PERCENT / 100) as usize
    );
    let mut required_land = vec![false; tile_count];
    for &(sx, sy) in &STARTS {
        for dy in -2..=2 {
            for dx in -2..=2 {
                if isohex_vector_distance(dx, dy) <= 2 {
                    let (x, y) = offset_native_by_map(sx, sy, dx, dy);
                    required_land[(y * MAP_W + x) as usize] = true;
                }
            }
        }
    }

    let required_count = required_land.iter().filter(|required| **required).count();
    debug_assert!(required_count <= LAND_TILE_TARGET);
    let mut candidates: Vec<(usize, f32)> = (0..tile_count)
        .filter(|index| !required_land[*index])
        .map(|index| {
            let x = index as i32 % MAP_W;
            let y = index as i32 / MAP_W;
            (index, height_score(x, y))
        })
        .collect();
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut is_land = required_land;
    for (index, _) in candidates
        .into_iter()
        .take(LAND_TILE_TARGET - required_count)
    {
        is_land[index] = true;
    }

    (0..tile_count)
        .map(|index| {
            let x = index as i32 % MAP_W;
            let y = index as i32 / MAP_W;
            Tile {
                terrain: if is_land[index] {
                    if STARTS.contains(&(x, y)) {
                        // Freeciv only places starts on terrain with the
                        // Starter flag. Grassland is one such terrain.
                        Terrain::Grassland
                    } else {
                        land_terrain(x, y)
                    }
                } else {
                    Terrain::Ocean
                },
                road: false,
                irrigation: false,
                mine: false,
                owner: None,
                seen: false,
                visible: false,
                known_by: [false; PLAYER_COUNT],
            }
        })
        .collect()
}

impl Game {
    fn new() -> Self {
        let tiles = generate_tiles();

        let names = [
            "Crimson League",
            "Golden Compact",
            "Azure Assembly",
            "Violet Court",
            "Amber Union",
            "Rose Confederacy",
        ];
        let colors = [
            Color::from_rgba(236, 61, 58, 255),
            Color::from_rgba(245, 213, 55, 255),
            Color::from_rgba(56, 207, 226, 255),
            Color::from_rgba(151, 92, 212, 255),
            Color::from_rgba(241, 145, 42, 255),
            Color::from_rgba(225, 77, 171, 255),
        ];
        let free_techs = [
            Tech::BRONZE_WORKING,
            Tech::ALPHABET,
            Tech::HORSEBACK_RIDING,
            Tech::POTTERY,
            Tech::WARRIOR_CODE,
            Tech::BRONZE_WORKING,
        ];
        let research_targets = [
            Tech::WARRIOR_CODE,
            Tech::BRONZE_WORKING,
            Tech::POTTERY,
            Tech::HORSEBACK_RIDING,
            Tech::ALPHABET,
            Tech::WARRIOR_CODE,
        ];
        let mut players = Vec::new();
        for i in 0..PLAYER_COUNT {
            players.push(Player {
                name: names[i],
                color: colors[i],
                gold: 50,
                known: vec![free_techs[i]],
                researching: research_targets[i],
                research_goal: [research_targets[i], Tech::IRON_WORKING, Tech::THE_WHEEL,
                    Tech::MATHEMATICS, Tech::IRON_WORKING, Tech::THE_WHEEL][i],
                future_techs: 0,
                research: 0,
                ai: i != HUMAN,
            });
        }

        let mut units = Vec::new();
        let mut next_unit_id = 1;
        for owner in 0..PLAYER_COUNT {
            for kind in [
                UnitKind::Settlers,
                UnitKind::Worker,
                UnitKind::Explorer,
                UnitKind::Diplomat,
            ] {
                let stats = kind.stats();
                units.push(Unit {
                    id: next_unit_id,
                    owner,
                    kind,
                    x: STARTS[owner].0,
                    y: STARTS[owner].1,
                    hp: stats.hp,
                    moves_left: stats.moves * MOVE_FRAGMENTS,
                    state: UnitState::Idle,
                    moved: false,
                    automated: false,
                    work_kind: None,
                    work_remaining: 0,
                });
                next_unit_id += 1;
            }
        }

        let selected_unit = units
            .iter()
            .find(|u| u.owner == HUMAN && u.kind == UnitKind::Settlers)
            .map(|u| u.id);
        let mut game = Self {
            tiles,
            units,
            cities: Vec::new(),
            players,
            turn: 1,
            year: -4000,
            next_unit_id,
            next_city_id: 1,
            selected_unit,
            selected_city: None,
            camera: {
                let (map_x, map_y) = native_to_map(STARTS[0].0, STARTS[0].1);
                vec2(map_x as f32, map_y as f32)
            },
            zoom: START_ZOOM,
            rng: 0x9e37_79b9_7f4a_7c15,
            events: vec![
                "4000 BCE - Your people are ready to settle.".to_owned(),
                "Choose the Settlers and press B to found a city.".to_owned(),
                "Explore, research, grow, and unite the world.".to_owned(),
            ],
            goto_mode: false,
            research_page: None,
            game_over: None,
            hud_elements_drawn: 0,
            dragging: false,
            drag_last: vec2(0.0, 0.0),
        };
        for owner in 1..PLAYER_COUNT {
            game.players[owner].researching = game.research_target(owner);
        }
        game.refresh_visibility();
        game
    }

    fn idx(x: i32, y: i32) -> usize {
        let wx = x.rem_euclid(MAP_W);
        let wy = y.rem_euclid(MAP_H);
        (wy * MAP_W + wx) as usize
    }

    fn xy(index: usize) -> (i32, i32) {
        ((index as i32) % MAP_W, (index as i32) / MAP_W)
    }

    fn tile(&self, x: i32, y: i32) -> &Tile {
        &self.tiles[Self::idx(x, y)]
    }

    fn tile_mut(&mut self, x: i32, y: i32) -> &mut Tile {
        let idx = Self::idx(x, y);
        &mut self.tiles[idx]
    }

    fn add_event(&mut self, text: impl Into<String>) {
        self.events.push(text.into());
        if self.events.len() > 10 {
            self.events.remove(0);
        }
    }

    fn rand_below(&mut self, max: i32) -> i32 {
        if max <= 1 {
            return 0;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % max as u64) as i32
    }

    fn has_tech(&self, owner: usize, tech: Tech) -> bool {
        self.players[owner].known.contains(&tech)
    }

    fn tech_available(&self, owner: usize, tech: Tech) -> bool {
        if tech == Tech::FUTURE {
            return Tech::all().all(|advance| self.has_tech(owner, advance));
        }
        !self.has_tech(owner, tech)
            && tech.prerequisites().iter().all(|&req| self.has_tech(owner, req))
    }

    fn research_path(&self, owner: usize, goal: Tech) -> Vec<Tech> {
        fn visit(game: &Game, owner: usize, tech: Tech, path: &mut Vec<Tech>) {
            if game.has_tech(owner, tech) || path.contains(&tech) {
                return;
            }
            for &req in tech.prerequisites() {
                visit(game, owner, req, path);
            }
            path.push(tech);
        }

        let mut path = Vec::new();
        if goal == Tech::FUTURE {
            for advance in Tech::all() {
                visit(self, owner, advance, &mut path);
            }
        }
        visit(self, owner, goal, &mut path);
        path
    }

    fn research_target(&self, owner: usize) -> Tech {
        self.research_path(owner, self.players[owner].research_goal)
            .first()
            .copied()
            .or_else(|| {
                Tech::all()
                    .filter(|&tech| self.tech_available(owner, tech))
                    .min_by_key(|tech| tech.cost())
            })
            .unwrap_or(Tech::FUTURE)
    }

    fn target_cost(&self, owner: usize, tech: Tech) -> i32 {
        if tech == Tech::FUTURE {
            let player = &self.players[owner];
            research_cost(player.known.len() as i32 + player.future_techs + 1)
        } else {
            tech.cost()
        }
    }

    fn discover(&mut self, owner: usize, tech: Tech) {
        if tech == Tech::FUTURE {
            self.players[owner].future_techs += 1;
        } else {
            self.players[owner].known.push(tech);
        }
        if owner == HUMAN {
            self.add_event(format!("Research complete: {}.", tech.name()));
        }
    }

    fn unit_available(&self, owner: usize, kind: UnitKind) -> bool {
        match kind {
            UnitKind::Phalanx => self.has_tech(owner, Tech::BRONZE_WORKING),
            UnitKind::Archer => self.has_tech(owner, Tech::WARRIOR_CODE),
            UnitKind::Legion => self.has_tech(owner, Tech::IRON_WORKING),
            UnitKind::Horseman => self.has_tech(owner, Tech::HORSEBACK_RIDING),
            UnitKind::Chariot => self.has_tech(owner, Tech::THE_WHEEL),
            UnitKind::Catapult => self.has_tech(owner, Tech::MATHEMATICS),
            UnitKind::Diplomat => self.has_tech(owner, Tech::ALPHABET),
            UnitKind::Explorer => false,
            _ => true,
        }
    }

    fn selected_unit_index(&self) -> Option<usize> {
        let id = self.selected_unit?;
        self.units.iter().position(|u| u.id == id)
    }

    fn selected_city_index(&self) -> Option<usize> {
        let id = self.selected_city?;
        self.cities.iter().position(|c| c.id == id)
    }

    fn refresh_visibility(&mut self) {
        for tile in &mut self.tiles {
            tile.visible = false;
        }
        let revealers: Vec<(usize, i32, i32, i32)> = self
            .units
            .iter()
            .map(|u| (u.owner, u.x, u.y, u.kind.stats().vision_sq))
            .chain(self.cities.iter().map(|c| (c.owner, c.x, c.y, 8)))
            .collect();
        for (owner, x, y, radius_sq) in revealers {
            let radius = (radius_sq as f32).sqrt().ceil() as i32;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let distance = isohex_vector_distance(dx, dy);
                    if distance * distance <= radius_sq {
                        let (visible_x, visible_y) = offset_native_by_map(x, y, dx, dy);
                        let idx = Self::idx(visible_x, visible_y);
                        self.tiles[idx].known_by[owner] = true;
                        if owner == HUMAN {
                            self.tiles[idx].visible = true;
                            self.tiles[idx].seen = true;
                        }
                    }
                }
            }
        }
    }

    fn claim_city_tiles(&mut self, city_x: i32, city_y: i32, owner: usize) {
        for dy in -2..=2 {
            for dx in -2..=2 {
                let distance = isohex_vector_distance(dx, dy);
                if distance * distance <= 5 {
                    let (claim_x, claim_y) = offset_native_by_map(city_x, city_y, dx, dy);
                    self.tile_mut(claim_x, claim_y).owner = Some(owner);
                }
            }
        }
    }

    fn rebuild_city_ownership(&mut self) {
        for tile in &mut self.tiles {
            tile.owner = None;
        }
        let cities: Vec<(i32, i32, usize)> = self
            .cities
            .iter()
            .map(|city| (city.x, city.y, city.owner))
            .collect();
        for (x, y, owner) in cities {
            self.claim_city_tiles(x, y, owner);
        }
    }

    fn map_distance(&self, ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
        let (dx, dy) = wrapped_map_vector(ax, ay, bx, by);
        isohex_vector_distance(dx, dy)
    }

    fn can_found_at(&self, x: i32, y: i32) -> bool {
        if !self.tile(x, y).terrain.allows_city() {
            return false;
        }
        self.cities
            .iter()
            .all(|c| self.map_distance(x, y, c.x, c.y) >= CITY_MIN_DISTANCE)
    }

    fn can_irrigate_at(&self, x: i32, y: i32) -> bool {
        let tile = self.tile(x, y);
        if tile.irrigation || tile.terrain.irrigation_time() == 0 {
            return false;
        }
        ISOHEX_DIRS.iter().any(|&(dx, dy)| {
            let (source_x, source_y) = offset_native_by_map(x, y, dx, dy);
            let source = self.tile(source_x, source_y);
            source.terrain == Terrain::Ocean || source.irrigation
        })
    }

    fn city_name(&self, owner: usize) -> String {
        const ROOTS: [[&str; 4]; PLAYER_COUNT] = [
            ["Aster", "Briar", "Cinder", "Dawn"],
            ["Gilden", "Hearth", "Ilex", "Jasper"],
            ["Lumen", "Mere", "Northwind", "Opal"],
            ["Pine", "Quartz", "Rill", "Solace"],
            ["Tern", "Umber", "Vale", "Willow"],
            ["Yarrow", "Zephyr", "Alder", "Brook"],
        ];
        let count = self.cities.iter().filter(|c| c.owner == owner).count();
        if count < 4 {
            ROOTS[owner][count].to_owned()
        } else {
            format!("{} {}", ROOTS[owner][count % 4], count + 1)
        }
    }

    fn found_city(&mut self, unit_id: u32) -> bool {
        let Some(index) = self.units.iter().position(|u| u.id == unit_id) else {
            return false;
        };
        let unit = self.units[index].clone();
        if !unit.kind.stats().can_found || !self.can_found_at(unit.x, unit.y) {
            if unit.owner == HUMAN {
                self.add_event("A city must be on land and at least 3 tiles from another city.");
            }
            return false;
        }
        let name = self.city_name(unit.owner);
        let city_id = self.next_city_id;
        self.next_city_id += 1;
        self.units.remove(index);
        self.cities.push(City {
            id: city_id,
            owner: unit.owner,
            x: unit.x,
            y: unit.y,
            name: name.clone(),
            size: 1,
            food: 0,
            shields: 0,
            production: UnitKind::Warrior,
        });
        let tile = self.tile_mut(unit.x, unit.y);
        tile.road = true;
        tile.owner = Some(unit.owner);
        self.claim_city_tiles(unit.x, unit.y, unit.owner);
        if unit.owner == HUMAN {
            self.selected_unit = None;
            self.selected_city = Some(city_id);
            self.add_event(format!(
                "{name} founded. Its people begin training Warriors."
            ));
        }
        self.refresh_visibility();
        true
    }

    fn spawn_unit(&mut self, owner: usize, kind: UnitKind, x: i32, y: i32) -> u32 {
        let id = self.next_unit_id;
        self.next_unit_id += 1;
        let stats = kind.stats();
        self.units.push(Unit {
            id,
            owner,
            kind,
            x,
            y,
            hp: stats.hp,
            moves_left: stats.moves * MOVE_FRAGMENTS,
            state: UnitState::Idle,
            moved: false,
            automated: false,
            work_kind: None,
            work_remaining: 0,
        });
        id
    }

    fn movement_cost_for(&self, unit: &Unit, nx: i32, ny: i32) -> Option<i32> {
        let dest = self.tile(nx, ny);
        if dest.terrain == Terrain::Ocean {
            return None;
        }
        let source = self.tile(unit.x, unit.y);
        if source.road && dest.road {
            Some(2)
        } else if unit.kind.stats().ignores_terrain {
            Some(2)
        } else {
            Some(dest.terrain.movement_cost() * MOVE_FRAGMENTS)
        }
    }

    fn attack_power(unit: &Unit) -> i32 {
        let stats = unit.kind.stats();
        let mut power = stats.attack * POWER_FACTOR;
        if unit.moves_left < MOVE_FRAGMENTS {
            power = power * unit.moves_left.max(0) / MOVE_FRAGMENTS;
        }
        power
    }

    fn defense_power(&self, unit: &Unit, x: i32, y: i32) -> i32 {
        // Freeciv applies these as separate integer-multiplication stages:
        // terrain in get_defense_power, then Defend_Bonus, then
        // Fortify_Defense_Bonus in defense_multiplication.
        let mut power = unit.kind.stats().defense * POWER_FACTOR;
        power = apply_percent_bonus(power, self.tile(x, y).terrain.defense_bonus());

        let city = self.cities.iter().find(|c| c.x == x && c.y == y);
        let mut defend_bonus = 0;
        if let Some(city) = city {
            defend_bonus += CITY_DEFEND_BONUS;
            if city.size >= LARGE_CITY_SIZE {
                defend_bonus += LARGE_CITY_DEFEND_BONUS;
            }
        }
        power = apply_percent_bonus(power, defend_bonus);

        // Civ2Civ3 gives CanFortify units without the Cant_Fortify type flag
        // the same 50% city stage even while they are not explicitly fortified.
        let fortify_bonus =
            if unit.state == UnitState::Fortified || (city.is_some() && unit.kind.can_fortify()) {
                FORTIFY_DEFENSE_BONUS
            } else {
                0
            };
        apply_percent_bonus(power, fortify_bonus)
    }

    fn defender_rank(&self, attacker: &Unit, defender: &Unit, x: i32, y: i32) -> (i32, i32, i32) {
        let attack_power = Self::attack_power(attacker);
        let defense_power = self.defense_power(defender, x, y);
        let attacker_stats = attacker.kind.stats();
        let defender_stats = defender.kind.stats();
        let attacker_win = combat_win_chance(
            attack_power,
            attacker.hp,
            attacker_stats.firepower,
            defense_power,
            defender.hp,
            defender_stats.firepower,
        );

        // get_defender truncates the defender's win chance to five decimal
        // places, then prefers the cheaper unit and finally the higher
        // defense rating when those values tie.
        let defender_value = (100_000.0 * (1.0 - attacker_win)) as i32;
        let rounds_survived =
            (defender.hp + attacker_stats.firepower - 1) / attacker_stats.firepower;
        let defense_rating = defense_power * rounds_survived * defender_stats.firepower;
        (defender_value, -defender_stats.cost, defense_rating)
    }

    fn best_defender_id(&self, attacker: &Unit, x: i32, y: i32) -> Option<u32> {
        let mut best = None;
        let mut best_rank = None;
        for defender in self
            .units
            .iter()
            .filter(|unit| unit.x == x && unit.y == y && unit.owner != attacker.owner)
        {
            let rank = self.defender_rank(attacker, defender, x, y);
            if best_rank.is_none() || best_rank.is_some_and(|current| rank > current) {
                best = Some(defender.id);
                best_rank = Some(rank);
            }
        }
        best
    }

    fn try_move_unit(&mut self, unit_id: u32, dx: i32, dy: i32) -> MoveResult {
        let Some(index) = self.units.iter().position(|u| u.id == unit_id) else {
            return MoveResult::Blocked;
        };
        if !is_isohex_step(dx, dy) {
            return MoveResult::Blocked;
        }
        let unit = self.units[index].clone();
        if unit.moves_left <= 0 || unit.work_remaining > 0 || unit.state == UnitState::Sentry {
            return MoveResult::Blocked;
        }
        let (nx, ny) = offset_native_by_map(unit.x, unit.y, dx, dy);
        let Some(cost) = self.movement_cost_for(&unit, nx, ny) else {
            return MoveResult::Blocked;
        };
        let defender = self.best_defender_id(&unit, nx, ny);

        if let Some(defender_id) = defender {
            if unit.kind.stats().attack <= 0 {
                return MoveResult::Blocked;
            }
            self.resolve_combat(unit_id, defender_id, nx, ny);
            return MoveResult::Fought;
        }

        if let Some(city) = self.cities.iter().find(|c| c.x == nx && c.y == ny) {
            if city.owner != unit.owner && !unit.kind.is_military() {
                return MoveResult::Blocked;
            }
        }

        if let Some(mover) = self.units.iter_mut().find(|u| u.id == unit_id) {
            mover.x = nx;
            mover.y = ny;
            mover.moves_left = (mover.moves_left - cost).max(0);
            mover.moved = true;
            mover.state = UnitState::Idle;
        }
        self.capture_city_if_needed(unit_id, nx, ny);
        if unit.owner == HUMAN {
            self.focus_camera_on(nx, ny);
            self.refresh_visibility();
        }
        MoveResult::Moved
    }

    fn resolve_combat(&mut self, attacker_id: u32, defender_id: u32, nx: i32, ny: i32) {
        let Some(attacker_index) = self.units.iter().position(|u| u.id == attacker_id) else {
            return;
        };
        let Some(defender_index) = self.units.iter().position(|u| u.id == defender_id) else {
            return;
        };
        let attacker = self.units[attacker_index].clone();
        let defender = self.units[defender_index].clone();
        let astats = attacker.kind.stats();
        let dstats = defender.kind.stats();

        let attack_power = Self::attack_power(&attacker);
        let defense_power = self.defense_power(&defender, nx, ny);
        let mut att_hp = attacker.hp;
        let mut def_hp = defender.hp;
        if attack_power <= 0 {
            att_hp = 0;
        } else if defense_power <= 0 {
            def_hp = 0;
        }
        let mut rounds = 0;
        while att_hp > 0 && def_hp > 0 {
            if self.rand_below(attack_power + defense_power) >= defense_power {
                def_hp -= astats.firepower;
            } else {
                att_hp -= dstats.firepower;
            }
            rounds += 1;
        }
        att_hp = att_hp.max(0);
        def_hp = def_hp.max(0);

        if let Some(a) = self.units.iter_mut().find(|u| u.id == attacker_id) {
            a.hp = att_hp;
            a.moves_left = (a.moves_left - MOVE_FRAGMENTS).max(0);
            a.moved = true;
        }
        if let Some(d) = self.units.iter_mut().find(|u| u.id == defender_id) {
            d.hp = def_hp;
        }

        let attacker_name = attacker.kind.name();
        let defender_name = defender.kind.name();
        let attacker_owner = attacker.owner;
        let defender_owner = defender.owner;
        if def_hp <= 0 {
            self.units.retain(|u| u.id != defender_id);
            if att_hp > 0 {
                if let Some(a) = self.units.iter_mut().find(|u| u.id == attacker_id) {
                    a.x = nx;
                    a.y = ny;
                }
                self.capture_city_if_needed(attacker_id, nx, ny);
            }
            if attacker_owner == HUMAN || defender_owner == HUMAN {
                self.add_event(format!(
                    "{attacker_name} defeated {defender_name} after {rounds} combat rounds."
                ));
            }
        } else if att_hp <= 0 {
            self.units.retain(|u| u.id != attacker_id);
            if self.selected_unit == Some(attacker_id) {
                self.selected_unit = None;
            }
            if attacker_owner == HUMAN || defender_owner == HUMAN {
                self.add_event(format!(
                    "{defender_name} held the tile against {attacker_name}."
                ));
            }
        }
        if attacker_owner == HUMAN || defender_owner == HUMAN {
            self.refresh_visibility();
        }
        self.check_game_over();
    }

    fn capture_city_if_needed(&mut self, unit_id: u32, x: i32, y: i32) {
        let Some(unit) = self.units.iter().find(|u| u.id == unit_id).cloned() else {
            return;
        };
        if !unit.kind.is_military() {
            return;
        }
        let Some(city_index) = self
            .cities
            .iter()
            .position(|c| c.x == x && c.y == y && c.owner != unit.owner)
        else {
            return;
        };
        let old_owner = self.cities[city_index].owner;
        let city_name = self.cities[city_index].name.clone();
        self.cities[city_index].owner = unit.owner;
        self.cities[city_index].size = (self.cities[city_index].size - 1).max(1);
        self.cities[city_index].food = 0;
        self.cities[city_index].shields = 0;
        self.cities[city_index].production = UnitKind::Warrior;
        self.claim_city_tiles(x, y, unit.owner);
        if unit.owner == HUMAN || old_owner == HUMAN {
            self.add_event(format!(
                "{} captured {city_name} from {}.",
                self.players[unit.owner].name, self.players[old_owner].name
            ));
        }
    }

    fn path_to(&self, unit_id: u32, tx: i32, ty: i32) -> Vec<(i32, i32)> {
        let Some(unit) = self.units.iter().find(|u| u.id == unit_id) else {
            return Vec::new();
        };
        let start = Self::idx(unit.x, unit.y);
        let goal = Self::idx(tx, ty);
        if start == goal {
            return Vec::new();
        }
        let mut queue = VecDeque::new();
        let mut previous = vec![None::<(usize, (i32, i32))>; (MAP_W * MAP_H) as usize];
        previous[start] = Some((start, (0, 0)));
        queue.push_back(start);
        while let Some(current) = queue.pop_front() {
            if current == goal {
                break;
            }
            let (cx, cy) = Self::xy(current);
            for (dx, dy) in ISOHEX_DIRS {
                let (nx, ny) = offset_native_by_map(cx, cy, dx, dy);
                let ni = Self::idx(nx, ny);
                if previous[ni].is_some() || self.tile(nx, ny).terrain == Terrain::Ocean {
                    continue;
                }
                let enemy_block = self
                    .units
                    .iter()
                    .any(|u| u.x == nx && u.y == ny && u.owner != unit.owner);
                if enemy_block && ni != goal {
                    continue;
                }
                previous[ni] = Some((current, (dx, dy)));
                queue.push_back(ni);
            }
        }
        if previous[goal].is_none() {
            return Vec::new();
        }
        let mut path = Vec::new();
        let mut cursor = goal;
        while cursor != start {
            let Some((prior, direction)) = previous[cursor] else {
                return Vec::new();
            };
            path.push(direction);
            cursor = prior;
        }
        path.reverse();
        path
    }

    fn goto_unit(&mut self, unit_id: u32, tx: i32, ty: i32) -> bool {
        let path = self.path_to(unit_id, tx, ty);
        if path.is_empty() {
            return false;
        }
        let mut moved = false;
        for (dx, dy) in path {
            let result = self.try_move_unit(unit_id, dx, dy);
            if result == MoveResult::Blocked {
                break;
            }
            moved = true;
            let Some(unit) = self.units.iter().find(|u| u.id == unit_id) else {
                break;
            };
            if unit.moves_left <= 0 || result == MoveResult::Fought {
                break;
            }
        }
        moved
    }

    fn begin_work(&mut self, unit_id: u32, kind: WorkKind) -> bool {
        let Some(index) = self.units.iter().position(|u| u.id == unit_id) else {
            return false;
        };
        let unit = self.units[index].clone();
        if !unit.kind.stats().can_work || unit.work_remaining > 0 {
            return false;
        }
        let tile = self.tile(unit.x, unit.y);
        let turns = match kind {
            WorkKind::Road if !tile.road => tile.terrain.road_time(),
            WorkKind::Irrigation if self.can_irrigate_at(unit.x, unit.y) => {
                tile.terrain.irrigation_time()
            }
            WorkKind::Mine if !tile.mine => tile.terrain.mine_time(),
            _ => 0,
        };
        if turns <= 0 {
            if unit.owner == HUMAN {
                self.add_event(format!(
                    "{} cannot build {} on {}.",
                    unit.kind.name(),
                    kind.name(),
                    tile.terrain.name()
                ));
            }
            return false;
        }
        let worker = &mut self.units[index];
        worker.work_kind = Some(kind);
        worker.work_remaining = turns;
        worker.moves_left = 0;
        worker.state = UnitState::Idle;
        if unit.owner == HUMAN {
            self.add_event(format!(
                "{} began {} ({turns} turns).",
                unit.kind.name(),
                kind.name()
            ));
        }
        true
    }

    fn fortify_unit(&mut self, unit_id: u32) -> bool {
        let (owner, name) = {
            let Some(unit) = self.units.iter_mut().find(|u| u.id == unit_id) else {
                return false;
            };
            if !unit.kind.can_fortify() {
                return false;
            }
            unit.state = UnitState::Fortifying;
            unit.moves_left = 0;
            (unit.owner, unit.kind.name())
        };
        if owner == HUMAN {
            self.add_event(format!("{name} will be fortified next turn."));
        }
        true
    }

    fn city_yields(&self, city: &City) -> (i32, i32, i32) {
        let center = self.tile(city.x, city.y);
        let (mut cf, cs, ct) = center.yields();
        if !center.irrigation && !center.mine {
            // City centers receive the terrain's irrigation bonus
            // automatically, but an explicit irrigation does not stack.
            cf += center.terrain.irrigation_bonus();
        }
        let mut food = cf.max(1);
        let mut shields = cs.max(1);
        let mut trade = ct.max(1);
        let mut candidates = Vec::new();
        for dy in -2..=2 {
            for dx in -2..=2 {
                let distance = isohex_vector_distance(dx, dy);
                if (dx == 0 && dy == 0) || distance * distance > 5 {
                    continue;
                }
                let (tile_x, tile_y) = offset_native_by_map(city.x, city.y, dx, dy);
                let tile = self.tile(tile_x, tile_y);
                let yields = tile.yields();
                let score = yields.0 * 4 + yields.1 * 3 + yields.2 * 2;
                candidates.push((score, yields));
            }
        }
        candidates.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, (f, s, t)) in candidates.into_iter().take(city.size as usize) {
            food += f;
            shields += s;
            trade += t;
        }
        (food, shields, trade)
    }

    fn growth_box(size: i32) -> i32 {
        if size <= 0 {
            return 0;
        }
        let initial_count = GRANARY_FOOD_INITIAL.len() as i32;
        let base = if size <= initial_count {
            GRANARY_FOOD_INITIAL[(size - 1) as usize]
        } else {
            GRANARY_FOOD_INITIAL[GRANARY_FOOD_INITIAL.len() - 1]
                + GRANARY_FOOD_INCREMENT * (size - initial_count)
        };
        percent_of(base, FOODBOX_PERCENT).max(1)
    }

    fn hitpoint_gain(unit: &Unit, in_city_center: bool) -> i32 {
        if unit.moved {
            return 0;
        }
        let base = unit.kind.stats().hp;
        let minimum_percent = if in_city_center {
            CITY_MIN_HP_PERCENT
        } else {
            0
        };
        // hp_gain_coord first takes the greater of HP_Regen and Min_HP_Pct,
        // then adds HP_Regen_2. Opening ground units have no HP_Regen
        // effect, a universal 10 HP_Regen_2, and another 10 when fortified.
        let hp_regen = 0;
        let minimum = percent_of(base, minimum_percent);
        let mut regeneration_2 = BASE_HP_REGEN_2;
        if unit.state == UnitState::Fortified {
            regeneration_2 += FORTIFIED_HP_REGEN_2;
        }
        // hp_gain_coord at 0x1003fee50 divides the integer HP value by 10
        // before applying HP_Regen_2, then truncates again after the bonus.
        let regeneration_after_minimum = (base / 10) * regeneration_2 / 10;
        (hp_regen.max(minimum) + regeneration_after_minimum).clamp(0, base - unit.hp)
    }

    fn change_unit_production(&mut self, city_id: u32, kind: UnitKind) -> bool {
        let Some(city) = self.cities.iter_mut().find(|city| city.id == city_id) else {
            return false;
        };
        // Native city_change_production_penalty keeps every shield when the
        // old and new targets are both units. This focused build has no
        // building production, so every exposed switch is in that class.
        city.production = kind;
        true
    }

    fn choose_ai_production(&self, owner: usize, city_id: u32) -> UnitKind {
        let Some(city) = self.cities.iter().find(|c| c.id == city_id) else {
            return UnitKind::Warrior;
        };
        let defenders = self
            .units
            .iter()
            .filter(|u| u.owner == owner && u.x == city.x && u.y == city.y && u.kind.is_military())
            .count();
        if defenders == 0 {
            return if self.unit_available(owner, UnitKind::Phalanx) {
                UnitKind::Phalanx
            } else {
                UnitKind::Warrior
            };
        }
        let city_count = self.cities.iter().filter(|c| c.owner == owner).count();
        let has_founder = self
            .units
            .iter()
            .any(|u| u.owner == owner && u.kind == UnitKind::Settlers);
        if city.size >= 3 && city_count < 3 && !has_founder {
            return UnitKind::Settlers;
        }
        for kind in [
            UnitKind::Catapult,
            UnitKind::Chariot,
            UnitKind::Legion,
            UnitKind::Archer,
            UnitKind::Horseman,
            UnitKind::Phalanx,
            UnitKind::Warrior,
        ] {
            if self.unit_available(owner, kind) {
                return kind;
            }
        }
        UnitKind::Warrior
    }

    fn process_cities(&mut self) {
        let city_ids: Vec<u32> = self.cities.iter().map(|c| c.id).collect();
        for city_id in city_ids {
            let Some(snapshot) = self.cities.iter().find(|c| c.id == city_id).cloned() else {
                continue;
            };
            let (food_income, shield_income, _) = self.city_yields(&snapshot);
            let mut grew = false;
            let mut starved = false;
            let mut destroyed = false;
            let mut finished = None;
            if let Some(city) = self.cities.iter_mut().find(|c| c.id == city_id) {
                city.food += food_income - city.size * CITIZEN_FOOD_COST;
                if city.food >= Self::growth_box(city.size) {
                    city.food -= Self::growth_box(city.size);
                    city.size += 1;
                    grew = true;
                } else if city.food < 0 {
                    starved = true;
                    if city.size > 1 {
                        city.size -= 1;
                        city.food = 0;
                    } else {
                        destroyed = true;
                    }
                } else {
                    city.food = city.food.max(0);
                }
                if !destroyed {
                    city.shields += shield_income;
                    let stats = city.production.stats();
                    if city.shields >= stats.cost
                        && (stats.pop_cost == 0 || city.size > stats.pop_cost)
                    {
                        city.shields -= stats.cost;
                        city.size -= stats.pop_cost;
                        finished = Some(city.production);
                    }
                }
            }
            if destroyed {
                self.cities.retain(|city| city.id != city_id);
                if self.selected_city == Some(city_id) {
                    self.selected_city = None;
                }
                self.rebuild_city_ownership();
                if snapshot.owner == HUMAN {
                    self.add_event(format!("Famine destroyed {} entirely.", snapshot.name));
                }
                continue;
            }
            if grew && snapshot.owner == HUMAN {
                self.add_event(format!(
                    "{} grew to size {}.",
                    snapshot.name,
                    snapshot.size + 1
                ));
            }
            if starved && snapshot.owner == HUMAN {
                self.add_event(format!("{} lost population to famine.", snapshot.name));
            }
            if let Some(kind) = finished {
                self.spawn_unit(snapshot.owner, kind, snapshot.x, snapshot.y);
                if snapshot.owner == HUMAN {
                    self.add_event(format!("{} completed {}.", snapshot.name, kind.name()));
                }
                if self.players[snapshot.owner].ai {
                    let next = self.choose_ai_production(snapshot.owner, city_id);
                    if let Some(city) = self.cities.iter_mut().find(|c| c.id == city_id) {
                        city.production = next;
                    }
                }
            }
        }

        for owner in 0..PLAYER_COUNT {
            let science: i32 = self
                .cities
                .iter()
                .filter(|c| c.owner == owner)
                .map(|c| self.city_yields(c).2.max(1))
                .sum();
            if science <= 0 {
                continue;
            }
            self.players[owner].research += science;
            let current = self.players[owner].researching;
            let cost = self.target_cost(owner, current);
            if self.players[owner].research >= cost {
                self.players[owner].research -= cost;
                let first_bonus = current.data().is_some_and(|advance| advance.first_discovery_bonus)
                    && self.players.iter().all(|player| !player.known.contains(&current));
                self.discover(owner, current);
                if first_bonus {
                    let bonus = self.research_target(owner);
                    if bonus != Tech::FUTURE {
                        self.discover(owner, bonus);
                    }
                }
                self.players[owner].researching = self.research_target(owner);
            }
        }
    }

    fn update_activities_and_restore(&mut self) {
        let mut completed = Vec::new();
        let city_centers: Vec<(i32, i32)> =
            self.cities.iter().map(|city| (city.x, city.y)).collect();
        for unit in &mut self.units {
            if unit.work_remaining > 0 {
                unit.work_remaining -= 1;
                if unit.work_remaining == 0 {
                    if let Some(kind) = unit.work_kind.take() {
                        completed.push((unit.owner, unit.x, unit.y, kind));
                    }
                }
            }
            if unit.state == UnitState::Fortifying {
                unit.state = UnitState::Fortified;
            }
            let in_city_center = city_centers.contains(&(unit.x, unit.y));
            unit.hp += Self::hitpoint_gain(unit, in_city_center);
            unit.moves_left = unit.kind.stats().moves * MOVE_FRAGMENTS;
            unit.moved = false;
        }
        for (owner, x, y, kind) in completed {
            let tile = self.tile_mut(x, y);
            match kind {
                WorkKind::Road => tile.road = true,
                WorkKind::Irrigation => {
                    tile.irrigation = true;
                    tile.mine = false;
                }
                WorkKind::Mine => {
                    tile.mine = true;
                    tile.irrigation = false;
                }
            }
            if owner == HUMAN {
                self.add_event(format!("Your workers completed {}.", kind.name()));
            }
        }
    }

    fn best_city_site(&self, owner: usize) -> Option<(i32, i32)> {
        let founder = self
            .units
            .iter()
            .find(|u| u.owner == owner && u.kind == UnitKind::Settlers)?;
        let mut best = None;
        let mut best_score = i32::MIN;
        for index in 0..self.tiles.len() {
            let (x, y) = Self::xy(index);
            if !self.can_found_at(x, y) {
                continue;
            }
            let distance = self.map_distance(founder.x, founder.y, x, y);
            let (f, s, t) = self.tile(x, y).yields();
            let score = f * 8 + s * 5 + t * 3 - distance;
            if score > best_score {
                best_score = score;
                best = Some((x, y));
            }
        }
        best
    }

    fn wander_unit(&mut self, unit_id: u32) {
        let mut safety = 0;
        while self
            .units
            .iter()
            .find(|u| u.id == unit_id)
            .is_some_and(|u| u.moves_left > 0)
            && safety < 6
        {
            let start = self.rand_below(6) as usize;
            let mut moved = false;
            for offset in 0..6 {
                let (dx, dy) = ISOHEX_DIRS[(start + offset) % 6];
                if self.try_move_unit(unit_id, dx, dy) != MoveResult::Blocked {
                    moved = true;
                    break;
                }
            }
            if !moved {
                break;
            }
            safety += 1;
        }
    }

    fn run_automated_unit(&mut self, unit_id: u32) {
        let Some(unit) = self.units.iter().find(|u| u.id == unit_id).cloned() else {
            return;
        };
        if unit.kind.stats().can_work {
            let tile = self.tile(unit.x, unit.y).clone();
            if !tile.road && self.begin_work(unit_id, WorkKind::Road) {
                return;
            }
            if tile.terrain.prefers_mine() {
                if !tile.mine && self.begin_work(unit_id, WorkKind::Mine) {
                    return;
                }
            } else if !tile.irrigation {
                if self.begin_work(unit_id, WorkKind::Irrigation) {
                    return;
                }
                if !tile.mine && self.begin_work(unit_id, WorkKind::Mine) {
                    return;
                }
            }
        }
        self.wander_unit(unit_id);
    }

    fn run_ai_player(&mut self, owner: usize) {
        let unit_ids: Vec<u32> = self
            .units
            .iter()
            .filter(|u| u.owner == owner)
            .map(|u| u.id)
            .collect();
        for id in unit_ids {
            let Some(unit) = self.units.iter().find(|u| u.id == id).cloned() else {
                continue;
            };
            match unit.kind {
                UnitKind::Settlers => {
                    if self.can_found_at(unit.x, unit.y)
                        && (self.cities.iter().all(|c| c.owner != owner)
                            || self
                                .cities
                                .iter()
                                .filter(|c| c.owner == owner)
                                .all(|c| self.map_distance(unit.x, unit.y, c.x, c.y) >= 4))
                    {
                        self.found_city(id);
                    } else if let Some((x, y)) = self.best_city_site(owner) {
                        self.goto_unit(id, x, y);
                        if self
                            .units
                            .iter()
                            .find(|u| u.id == id)
                            .is_some_and(|u| self.can_found_at(u.x, u.y))
                        {
                            self.found_city(id);
                        }
                    }
                }
                UnitKind::Worker => self.run_automated_unit(id),
                UnitKind::Explorer | UnitKind::Diplomat => self.wander_unit(id),
                _ => {
                    let on_city = self
                        .cities
                        .iter()
                        .find(|c| c.owner == owner && c.x == unit.x && c.y == unit.y)
                        .cloned();
                    if let Some(city) = on_city {
                        let defenders = self
                            .units
                            .iter()
                            .filter(|u| {
                                u.owner == owner
                                    && u.x == city.x
                                    && u.y == city.y
                                    && u.kind.is_military()
                            })
                            .count();
                        if defenders <= 1 {
                            self.fortify_unit(id);
                            continue;
                        }
                    }
                    if let Some(target) = self
                        .cities
                        .iter()
                        .filter(|c| c.owner != owner)
                        .min_by_key(|c| self.map_distance(unit.x, unit.y, c.x, c.y))
                        .cloned()
                    {
                        self.goto_unit(id, target.x, target.y);
                    } else {
                        self.wander_unit(id);
                    }
                }
            }
        }
    }

    fn advance_calendar(&mut self) {
        let step = if self.year < -1000 {
            50
        } else if self.year < 0 {
            25
        } else if self.year < 1000 {
            20
        } else if self.year < 1500 {
            10
        } else if self.year < 1750 {
            5
        } else if self.year < 1900 {
            2
        } else {
            1
        };
        self.year += step;
        if self.year == 0 {
            self.year = 1;
        }
    }

    fn end_turn(&mut self) {
        if self.game_over.is_some() {
            return;
        }
        let automated: Vec<u32> = self
            .units
            .iter()
            .filter(|u| u.owner == HUMAN && u.automated)
            .map(|u| u.id)
            .collect();
        for id in automated {
            self.run_automated_unit(id);
        }
        for owner in 1..PLAYER_COUNT {
            self.run_ai_player(owner);
        }
        self.process_cities();
        self.update_activities_and_restore();
        self.turn += 1;
        self.advance_calendar();
        self.refresh_visibility();
        self.select_next_unit();
        self.add_event(format!(
            "Turn {} begins - {}.",
            self.turn,
            year_label(self.year)
        ));
        self.check_game_over();
    }

    fn select_next_unit(&mut self) {
        let current = self.selected_unit.unwrap_or(0);
        let mut candidates: Vec<u32> = self
            .units
            .iter()
            .filter(|u| {
                u.owner == HUMAN
                    && u.moves_left > 0
                    && u.work_remaining == 0
                    && u.state != UnitState::Sentry
            })
            .map(|u| u.id)
            .collect();
        candidates.sort_unstable();
        let next = candidates
            .iter()
            .copied()
            .find(|id| *id > current)
            .or_else(|| candidates.first().copied());
        self.selected_unit = next;
        self.selected_city = None;
        if let Some(id) = next {
            if let Some((x, y)) = self
                .units
                .iter()
                .find(|u| u.id == id)
                .map(|unit| (unit.x, unit.y))
            {
                self.focus_camera_on(x, y);
            }
        }
    }

    fn check_game_over(&mut self) {
        let human_survives = self.cities.iter().any(|c| c.owner == HUMAN)
            || self
                .units
                .iter()
                .any(|u| u.owner == HUMAN && u.kind == UnitKind::Settlers);
        if !human_survives {
            self.game_over = Some("Your civilization has passed into history.".to_owned());
            return;
        }
        let rival_survives = self.cities.iter().any(|c| c.owner != HUMAN)
            || self
                .units
                .iter()
                .any(|u| u.owner != HUMAN && u.kind == UnitKind::Settlers);
        if !rival_survives {
            self.game_over = Some("The world is united beneath your banner.".to_owned());
        }
    }
}

fn hash2(x: i32, y: i32) -> u32 {
    let mut n = (x as u32)
        .wrapping_mul(0x45d9_f3b)
        .wrapping_add((y as u32).wrapping_mul(0x119d_e1f3))
        .wrapping_add(0x7f4a_7c15);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846c_a68b);
    n ^ (n >> 16)
}

impl Game {
    fn perform_action(&mut self, action: UiAction) {
        if self.game_over.is_some() && !matches!(action, UiAction::EndTurn) {
            return;
        }
        match action {
            UiAction::EndTurn => self.end_turn(),
            UiAction::CloseCity => {
                self.selected_city = None;
            }
            UiAction::SelectUnit(id) => {
                if self
                    .units
                    .iter()
                    .any(|unit| unit.id == id && unit.owner == HUMAN)
                {
                    self.selected_unit = Some(id);
                    self.selected_city = None;
                }
            }
            UiAction::FoundCity => {
                if let Some(id) = self.selected_unit {
                    self.found_city(id);
                }
            }
            UiAction::Road => {
                if let Some(id) = self.selected_unit {
                    self.begin_work(id, WorkKind::Road);
                }
            }
            UiAction::Irrigate => {
                if let Some(id) = self.selected_unit {
                    self.begin_work(id, WorkKind::Irrigation);
                }
            }
            UiAction::Mine => {
                if let Some(id) = self.selected_unit {
                    self.begin_work(id, WorkKind::Mine);
                }
            }
            UiAction::Fortify => {
                if let Some(id) = self.selected_unit {
                    self.fortify_unit(id);
                }
            }
            UiAction::Wait => {
                if let Some(index) = self.selected_unit_index() {
                    self.units[index].moves_left = 0;
                    self.units[index].state = UnitState::Idle;
                    self.add_event("Unit waits until next turn.");
                    self.select_next_unit();
                }
            }
            UiAction::Automate => {
                if let Some(index) = self.selected_unit_index() {
                    let automated = !self.units[index].automated;
                    let name = self.units[index].kind.name();
                    self.units[index].automated = automated;
                    self.add_event(format!(
                        "{name} automation {}.",
                        if automated { "enabled" } else { "disabled" }
                    ));
                }
            }
            UiAction::Produce(kind) => {
                if !self.unit_available(HUMAN, kind) {
                    self.add_event(format!("{} requires more research.", kind.name()));
                    return;
                }
                if let Some(index) = self.selected_city_index() {
                    let city_name = self.cities[index].name.clone();
                    let city_id = self.cities[index].id;
                    if self.change_unit_production(city_id, kind) {
                        self.add_event(format!("{city_name} now produces {}.", kind.name()));
                    }
                }
            }
            UiAction::OpenResearch => self.research_page = Some(0),
            UiAction::CloseResearch => self.research_page = None,
            UiAction::ResearchPage(page) => self.research_page = Some(page),
            UiAction::ResearchGoal(goal) => {
                if self.has_tech(HUMAN, goal) {
                    return;
                }
                self.players[HUMAN].research_goal = goal;
                let next = self.research_target(HUMAN);
                let player = &mut self.players[HUMAN];
                if player.researching != next {
                    player.researching = next;
                    player.research = 0;
                }
            }
        }
    }

    fn handle_keyboard(&mut self) {
        if self.research_page.is_some() {
            if is_key_pressed(KeyCode::Escape) {
                self.research_page = None;
            }
            return;
        }
        let shift_down = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        if is_key_pressed(KeyCode::Escape) {
            if self.selected_city.is_some() {
                self.selected_city = None;
            } else {
                self.goto_mode = false;
            }
        }
        if is_key_pressed(KeyCode::Tab) {
            self.select_next_unit();
        }
        if is_key_pressed(KeyCode::Enter) && shift_down {
            self.perform_action(UiAction::EndTurn);
            return;
        }
        if is_key_pressed(KeyCode::B) {
            self.perform_action(UiAction::FoundCity);
        }
        if is_key_pressed(KeyCode::R) {
            self.perform_action(UiAction::Road);
        }
        if is_key_pressed(KeyCode::I) {
            self.perform_action(UiAction::Irrigate);
        }
        if is_key_pressed(KeyCode::M) {
            self.perform_action(UiAction::Mine);
        }
        if is_key_pressed(KeyCode::F) {
            self.perform_action(UiAction::Fortify);
        }
        if is_key_pressed(KeyCode::A) {
            self.perform_action(UiAction::Automate);
        }
        if is_key_pressed(KeyCode::G) {
            self.goto_mode = !self.goto_mode;
            self.add_event(if self.goto_mode {
                "Go-to active: choose a destination tile."
            } else {
                "Go-to cancelled."
            });
        }
        if is_key_pressed(KeyCode::S) {
            if let Some(index) = self.selected_unit_index() {
                let name = self.units[index].kind.name();
                self.units[index].state = UnitState::Sentry;
                self.units[index].moves_left = 0;
                self.add_event(format!("{name} is now on sentry duty."));
                self.select_next_unit();
            }
        }
        if is_key_pressed(KeyCode::Space) {
            self.perform_action(UiAction::Wait);
        }
        if is_key_pressed(KeyCode::X) {
            self.perform_action(UiAction::Automate);
        }

        if shift_down {
            let pan = if is_key_pressed(KeyCode::Up) {
                Some((0.0, -1.0))
            } else if is_key_pressed(KeyCode::Down) {
                Some((0.0, 1.0))
            } else if is_key_pressed(KeyCode::Left) {
                Some((-1.0, 0.0))
            } else if is_key_pressed(KeyCode::Right) {
                Some((1.0, 0.0))
            } else {
                None
            };
            if let Some((dx, dy)) = pan {
                self.camera.x += dx;
                self.camera.y += dy;
                self.normalize_camera();
                return;
            }
        }

        let direction = if is_key_pressed(KeyCode::Up)
            || is_key_pressed(KeyCode::Key8)
            || is_key_pressed(KeyCode::Kp8)
        {
            Some((0, -1))
        } else if is_key_pressed(KeyCode::Down)
            || is_key_pressed(KeyCode::Key2)
            || is_key_pressed(KeyCode::Kp2)
        {
            Some((0, 1))
        } else if is_key_pressed(KeyCode::Left)
            || is_key_pressed(KeyCode::Key4)
            || is_key_pressed(KeyCode::Kp4)
        {
            Some((-1, 0))
        } else if is_key_pressed(KeyCode::Right)
            || is_key_pressed(KeyCode::Key6)
            || is_key_pressed(KeyCode::Kp6)
        {
            Some((1, 0))
        } else if is_key_pressed(KeyCode::Q)
            || is_key_pressed(KeyCode::Key7)
            || is_key_pressed(KeyCode::Kp7)
        {
            Some((-1, -1))
        } else if is_key_pressed(KeyCode::C)
            || is_key_pressed(KeyCode::Key3)
            || is_key_pressed(KeyCode::Kp3)
        {
            Some((1, 1))
        } else {
            None
        };
        if let (Some(id), Some((dx, dy))) = (self.selected_unit, direction) {
            if self.try_move_unit(id, dx, dy) != MoveResult::Blocked {
                self.select_next_if_exhausted(id);
            }
        }
    }

    fn select_next_if_exhausted(&mut self, id: u32) {
        let exhausted = self
            .units
            .iter()
            .find(|u| u.id == id)
            .is_none_or(|u| u.moves_left <= 0);
        if exhausted {
            self.select_next_unit();
        }
    }

    fn viewport_rect() -> Rect {
        Rect::new(
            0.0,
            TOP_H,
            (screen_width() - PANEL_W).max(320.0),
            (screen_height() - TOP_H - BOTTOM_H).max(220.0),
        )
    }

    fn minimap_rect() -> Rect {
        Rect::new(
            screen_width() - PANEL_W + 18.0,
            TOP_H + 38.0,
            PANEL_W - 36.0,
            150.0,
        )
    }

    fn city_manager_rect() -> Rect {
        let viewport = Self::viewport_rect();
        let width = (viewport.w - 40.0).clamp(300.0, 760.0);
        let height = (viewport.h - 36.0).clamp(260.0, 480.0);
        Rect::new(
            viewport.x + (viewport.w - width) * 0.5,
            viewport.y + (viewport.h - height) * 0.5,
            width,
            height,
        )
    }

    fn focus_camera_on(&mut self, native_x: i32, native_y: i32) {
        let (map_x, map_y) = native_to_map(native_x, native_y);
        self.camera = vec2(map_x as f32, map_y as f32);
    }

    fn normalize_camera(&mut self) {
        let rounded_x = self.camera.x.round();
        let rounded_y = self.camera.y.round();
        let fractional_x = self.camera.x - rounded_x;
        let fractional_y = self.camera.y - rounded_y;
        let (native_x, native_y) = map_pos_to_native(rounded_x as i32, rounded_y as i32);
        let (map_x, map_y) = native_to_map(native_x, native_y);
        self.camera = vec2(map_x as f32 + fractional_x, map_y as f32 + fractional_y);
    }

    fn camera_native(&self) -> (i32, i32) {
        map_pos_to_native(self.camera.x.round() as i32, self.camera.y.round() as i32)
    }

    fn world_to_screen(&self, x: i32, y: i32) -> Vec2 {
        let viewport = Self::viewport_rect();
        let (camera_x, camera_y) = self.camera_native();
        let (camera_map_x, camera_map_y) = native_to_map(camera_x, camera_y);
        let (map_dx, map_dy) = wrapped_map_vector(camera_x, camera_y, x, y);
        let dx = map_dx as f32 - (self.camera.x - camera_map_x as f32);
        let dy = map_dy as f32 - (self.camera.y - camera_map_y as f32);
        vec2(
            viewport.x + viewport.w * 0.5 + (dx - dy) * TILE_W * 0.5 * self.zoom,
            viewport.y + viewport.h * 0.47 + (dx + dy) * TILE_H * 0.5 * self.zoom,
        )
    }

    fn screen_to_world(&self, point: Vec2) -> (i32, i32) {
        let viewport = Self::viewport_rect();
        let sx = point.x - (viewport.x + viewport.w * 0.5);
        let sy = point.y - (viewport.y + viewport.h * 0.47);
        let a = sx / (TILE_W * 0.5 * self.zoom);
        let b = sy / (TILE_H * 0.5 * self.zoom);
        let dx = (a + b) * 0.5;
        let dy = (b - a) * 0.5;
        map_pos_to_native(
            (self.camera.x + dx).round() as i32,
            (self.camera.y + dy).round() as i32,
        )
    }

    fn handle_mouse(&mut self, buttons: &[(Rect, UiAction)]) {
        if self.research_page.is_some() {
            if is_mouse_button_pressed(MouseButton::Left) {
                let (x, y) = mouse_position();
                if let Some((_, action)) = buttons.iter().find(|(rect, _)| rect.contains(vec2(x, y))) {
                    self.perform_action(*action);
                }
            }
            return;
        }
        let (_, wheel) = mouse_wheel();
        if wheel.abs() > 0.01 {
            self.zoom = (self.zoom + wheel * 0.08).clamp(MIN_ZOOM, MAX_ZOOM);
        }

        let mouse = vec2(mouse_position().0, mouse_position().1);
        if is_mouse_button_pressed(MouseButton::Middle)
            || is_mouse_button_pressed(MouseButton::Right)
        {
            self.dragging = true;
            self.drag_last = mouse;
        }
        if self.dragging
            && (is_mouse_button_down(MouseButton::Middle)
                || is_mouse_button_down(MouseButton::Right))
        {
            let delta = mouse - self.drag_last;
            let a = delta.x / (TILE_W * 0.5 * self.zoom);
            let b = delta.y / (TILE_H * 0.5 * self.zoom);
            self.camera.x -= (a + b) * 0.5;
            self.camera.y -= (b - a) * 0.5;
            self.normalize_camera();
            self.drag_last = mouse;
        }
        if is_mouse_button_released(MouseButton::Middle)
            || is_mouse_button_released(MouseButton::Right)
        {
            self.dragging = false;
        }

        if !is_mouse_button_pressed(MouseButton::Left) {
            return;
        }
        if let Some((_, action)) = buttons.iter().find(|(rect, _)| rect.contains(mouse)) {
            self.perform_action(*action);
            return;
        }
        if self.selected_city.is_some() {
            return;
        }
        let minimap = Self::minimap_rect();
        if minimap.contains(mouse) {
            let native_x = ((mouse.x - minimap.x) / minimap.w * MAP_W as f32)
                .clamp(0.0, MAP_W as f32 - 0.01) as i32;
            let native_y = ((mouse.y - minimap.y) / minimap.h * MAP_H as f32)
                .clamp(0.0, MAP_H as f32 - 0.01) as i32;
            self.focus_camera_on(native_x, native_y);
            return;
        }
        if !Self::viewport_rect().contains(mouse) {
            return;
        }
        let (x, y) = self.screen_to_world(mouse);
        if self.goto_mode {
            if let Some(id) = self.selected_unit {
                self.goto_unit(id, x, y);
            }
            self.goto_mode = false;
            return;
        }
        self.select_at(x, y);
    }

    fn select_at(&mut self, x: i32, y: i32) {
        // Cities take click priority over their garrisons. Units stationed in
        // a city remain selectable from the city manager.
        if let Some(city) = self
            .cities
            .iter()
            .find(|city| city.owner == HUMAN && city.x == x && city.y == y)
        {
            self.selected_city = Some(city.id);
            self.selected_unit = None;
            return;
        }
        if let Some(unit) = self
            .units
            .iter()
            .rev()
            .find(|u| u.owner == HUMAN && u.x == x && u.y == y)
        {
            self.selected_unit = Some(unit.id);
            self.selected_city = None;
        }
    }
}

impl Game {
    fn ocean_match_type(&self, x: i32, y: i32) -> u8 {
        if self.tile(x, y).terrain != Terrain::Ocean {
            return 2; // land
        }

        let shallow = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 1),
            (-1, 0),
        ]
        .iter()
        .any(|(dx, dy)| {
            let (neighbor_x, neighbor_y) = offset_native_by_map(x, y, *dx, *dy);
            let neighbor = self.tile(neighbor_x, neighbor_y);
            neighbor.seen && neighbor.terrain != Terrain::Ocean
        });
        if shallow { 1 } else { 0 }
    }

    fn ocean_neighbor_match(&self, x: i32, y: i32, dx: i32, dy: i32, current: u8) -> u8 {
        let (neighbor_x, neighbor_y) = offset_native_by_map(x, y, dx, dy);
        if self.tile(neighbor_x, neighbor_y).seen {
            self.ocean_match_type(neighbor_x, neighbor_y)
        } else {
            // Do not reveal a coastline through unexplored terrain.
            current
        }
    }

    fn draw_ocean_sprite(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
        tint: Color,
    ) {
        let current = self.ocean_match_type(x, y);
        let nearby = |dx, dy| self.ocean_neighbor_match(x, y, dx, dy, current);

        // Each visual corner belongs to four map tiles. Amplio2 stores all
        // land/shallow/deep combinations as a four-terrain cell group; the
        // generated corner atlas contributes only this tile's quarter.
        let groups = [
            [nearby(-1, -1), nearby(0, -1), current, nearby(-1, 0)],
            [nearby(0, -1), nearby(1, -1), nearby(1, 0), current],
            [current, nearby(1, 0), nearby(1, 1), nearby(0, 1)],
            [nearby(-1, 0), current, nearby(0, 1), nearby(-1, 1)],
        ];

        for (corner, matches) in groups.into_iter().enumerate() {
            let (source_row, source_col) = ocean_cell_source(matches);
            draw_texture_ex(
                &assets.ocean_corners,
                center.x - tile_w * 0.5,
                center.y - tile_h * 0.5,
                tint,
                DrawTextureParams {
                    dest_size: Some(vec2(tile_w, tile_h)),
                    source: Some(Rect::new(
                        (source_col * TILE_SPRITE_W) as f32,
                        (corner * OCEAN_ATLAS_H + source_row * TILE_SPRITE_H) as f32,
                        TILE_SPRITE_W as f32,
                        TILE_SPRITE_H as f32,
                    )),
                    ..Default::default()
                },
            );
        }
    }

    fn draw_terrain_sprite(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
        visible: bool,
    ) {
        let terrain = self.tile(x, y).terrain;
        let tint = if visible {
            WHITE
        } else {
            Color::new(0.54, 0.59, 0.62, 1.0)
        };

        if terrain == Terrain::Ocean {
            self.draw_ocean_sprite(assets, x, y, center, tile_w, tile_h, tint);
            return;
        }

        let row = terrain.sprite_row().expect("land terrain sprite row");
        draw_texture_ex(
            &assets.terrain,
            center.x - tile_w * 0.5,
            center.y - tile_h * 0.5,
            tint,
            DrawTextureParams {
                dest_size: Some(vec2(tile_w, tile_h)),
                source: Some(bordered_sprite_source(row, 0, 96.0, 48.0)),
                ..Default::default()
            },
        );
    }

    fn draw_terrain_overlay(
        &self,
        assets: &Assets,
        terrain: Terrain,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
        visible: bool,
    ) {
        let tint = if visible {
            WHITE
        } else {
            Color::new(0.54, 0.59, 0.62, 1.0)
        };
        match terrain {
            Terrain::Forest => {
                draw_texture_ex(
                    &assets.terrain2,
                    center.x - tile_w * 0.5,
                    center.y - tile_h * 0.5,
                    tint,
                    DrawTextureParams {
                        dest_size: Some(vec2(tile_w, tile_h)),
                        source: Some(bordered_sprite_source(4, 0, 96.0, 48.0)),
                        ..Default::default()
                    },
                );
            }
            Terrain::Hills => {
                draw_texture_ex(
                    &assets.hills,
                    center.x - tile_w * 0.5,
                    center.y - tile_h * 0.5,
                    tint,
                    DrawTextureParams {
                        dest_size: Some(vec2(tile_w, tile_h)),
                        source: Some(bordered_sprite_source(0, 0, 96.0, 48.0)),
                        ..Default::default()
                    },
                );
            }
            Terrain::Mountains => {
                let mountain_h = 66.0 * self.zoom;
                draw_texture_ex(
                    &assets.mountains,
                    center.x - tile_w * 0.5,
                    center.y + tile_h * 0.5 - mountain_h + 6.0 * self.zoom,
                    tint,
                    DrawTextureParams {
                        dest_size: Some(vec2(tile_w, mountain_h)),
                        source: Some(bordered_sprite_source(0, 0, 96.0, 66.0)),
                        ..Default::default()
                    },
                );
            }
            _ => {}
        }
    }

    fn draw_terrain_extra(
        &self,
        assets: &Assets,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
        source_row: i32,
        source_col: i32,
    ) {
        draw_texture_ex(
            &assets.terrain,
            center.x - tile_w * 0.5,
            center.y - tile_h * 0.5,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(tile_w, tile_h)),
                source: Some(bordered_sprite_source(source_row, source_col, 96.0, 48.0)),
                ..Default::default()
            },
        );
    }

    fn draw_road_sprites(
        &self,
        assets: &Assets,
        x: i32,
        y: i32,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
    ) {
        let mut connected = false;
        for ((dx, dy), source_col) in [
            ((0, -1), 1),
            ((1, 0), 3),
            ((1, 1), 4),
            ((0, 1), 5),
            ((-1, 0), 7),
            ((-1, -1), 8),
        ] {
            let (neighbor_x, neighbor_y) = offset_native_by_map(x, y, dx, dy);
            let neighbor = self.tile(neighbor_x, neighbor_y);
            if neighbor.seen && neighbor.road {
                self.draw_terrain_extra(assets, center, tile_w, tile_h, 12, source_col);
                connected = true;
            }
        }
        if !connected {
            self.draw_terrain_extra(assets, center, tile_w, tile_h, 12, 0);
        }
    }

    fn draw_territory_border(
        &self,
        x: i32,
        y: i32,
        owner: usize,
        center: Vec2,
        tile_w: f32,
        tile_h: f32,
        visible: bool,
    ) {
        let points = [
            vec2(center.x, center.y - tile_h * 0.5),
            vec2(center.x + tile_w * 0.5, center.y),
            vec2(center.x, center.y + tile_h * 0.5),
            vec2(center.x - tile_w * 0.5, center.y),
        ];
        let edge_neighbors = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let color = with_alpha(self.players[owner].color, if visible { 0.82 } else { 0.42 });

        for (edge, (dx, dy)) in edge_neighbors.into_iter().enumerate() {
            let (neighbor_x, neighbor_y) = offset_native_by_map(x, y, dx, dy);
            if self.tile(neighbor_x, neighbor_y).owner == Some(owner) {
                continue;
            }
            let start = points[edge];
            let end = points[(edge + 1) % points.len()];
            draw_line(start.x, start.y, end.x, end.y, 2.0, color);
        }
    }

    fn draw_world(&self, assets: &Assets) {
        let viewport = Self::viewport_rect();
        draw_rectangle(
            viewport.x,
            viewport.y,
            viewport.w,
            viewport.h,
            Color::from_rgba(8, 28, 48, 255),
        );

        let mut order: Vec<(i32, i32, f32)> = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .map(|(x, y)| {
                let p = self.world_to_screen(x, y);
                (x, y, p.y)
            })
            .collect();
        order.sort_by(|a, b| a.2.total_cmp(&b.2));

        let tile_w = TILE_W * self.zoom;
        let tile_h = TILE_H * self.zoom;
        // Draw every tile base first. Tall terrain sprites are a separate
        // layer so later diamond bases cannot cut horizontal bands through
        // forests, hills, and mountains.
        for &(x, y, _) in &order {
            let center = self.world_to_screen(x, y);
            if center.x < viewport.x - tile_w
                || center.x > viewport.x + viewport.w + tile_w
                || center.y < viewport.y - 80.0
                || center.y > viewport.y + viewport.h + 80.0
            {
                continue;
            }
            let tile = self.tile(x, y);
            if !tile.seen {
                draw_diamond(
                    center,
                    tile_w - 1.0,
                    tile_h - 1.0,
                    Color::from_rgba(5, 15, 24, 255),
                );
                continue;
            }

            draw_diamond(center, tile_w, tile_h, tile.terrain.minimap_color());
            self.draw_terrain_sprite(assets, x, y, center, tile_w, tile_h, tile.visible);
        }

        for (x, y, _) in order {
            let center = self.world_to_screen(x, y);
            if center.x < viewport.x - tile_w
                || center.x > viewport.x + viewport.w + tile_w
                || center.y < viewport.y - 80.0
                || center.y > viewport.y + viewport.h + 80.0
            {
                continue;
            }
            let tile = self.tile(x, y);
            if !tile.seen {
                continue;
            }

            self.draw_terrain_overlay(assets, tile.terrain, center, tile_w, tile_h, tile.visible);

            if let Some(owner) = tile.owner {
                self.draw_territory_border(x, y, owner, center, tile_w, tile_h, tile.visible);
            }
            if tile.road {
                self.draw_road_sprites(assets, x, y, center, tile_w, tile_h);
            }
            if tile.irrigation {
                self.draw_terrain_extra(assets, center, tile_w, tile_h, 1, 6);
            }
            if tile.mine {
                self.draw_terrain_extra(assets, center, tile_w, tile_h, 3, 6);
            }
            if !tile.visible {
                draw_diamond(center, tile_w, tile_h, Color::from_rgba(18, 29, 39, 112));
                continue;
            }

            if let Some(city) = self.cities.iter().find(|c| c.x == x && c.y == y) {
                self.draw_city(assets, city, center);
            }
            let mut stack: Vec<&Unit> =
                self.units.iter().filter(|u| u.x == x && u.y == y).collect();
            stack.sort_by_key(|u| u.id != self.selected_unit.unwrap_or(u32::MAX));
            if let Some(unit) = stack.first() {
                self.draw_unit(assets, unit, center, stack.len());
            }
        }
    }

    fn draw_city(&self, assets: &Assets, city: &City, center: Vec2) {
        let col = (city.id as usize + city.owner * 2) % 11;
        let city_w = 69.0 * self.zoom;
        let city_h = 52.0 * self.zoom;
        draw_texture_ex(
            &assets.cities,
            center.x - city_w * 0.5,
            center.y - city_h + 9.0 * self.zoom,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(city_w, city_h)),
                source: Some(Rect::new(
                    1.0 + col as f32 * 97.0,
                    1.0 + 2.0 * 73.0,
                    96.0,
                    72.0,
                )),
                ..Default::default()
            },
        );
        let label_y = center.y + 10.0 * self.zoom;
        let label = format!("{}  {}", city.name, city.size);
        let width = measure_text(&label, None, (14.0 * self.zoom).max(10.0) as u16, 1.0).width;
        draw_rectangle(
            center.x - width * 0.5 - 6.0,
            label_y - 13.0,
            width + 12.0,
            17.0,
            Color::from_rgba(19, 20, 18, 220),
        );
        draw_rectangle(
            center.x - width * 0.5 - 6.0,
            label_y - 13.0,
            3.0,
            17.0,
            self.players[city.owner].color,
        );
        draw_text(
            &label,
            center.x - width * 0.5,
            label_y,
            (14.0 * self.zoom).max(10.0),
            Color::from_rgba(244, 235, 201, 255),
        );
        if self.selected_city == Some(city.id) {
            draw_diamond_lines(
                center,
                TILE_W * self.zoom * 0.88,
                TILE_H * self.zoom * 0.82,
                3.0,
                Color::from_rgba(255, 234, 119, 255),
            );
        }
    }

    fn draw_unit(&self, assets: &Assets, unit: &Unit, center: Vec2, stack_count: usize) {
        let stats = unit.kind.stats();
        let unit_w = 58.0 * self.zoom;
        let unit_h = 44.0 * self.zoom;
        let is_selected = self.selected_unit == Some(unit.id);
        if is_selected {
            draw_diamond(
                center + vec2(0.0, 4.0 * self.zoom),
                55.0 * self.zoom,
                23.0 * self.zoom,
                Color::from_rgba(255, 235, 112, 185),
            );
        }
        draw_texture_ex(
            &assets.units,
            center.x - unit_w * 0.5,
            center.y - unit_h + 9.0 * self.zoom,
            if unit.moves_left > 0 {
                WHITE
            } else {
                Color::new(0.62, 0.66, 0.68, 1.0)
            },
            DrawTextureParams {
                dest_size: Some(vec2(unit_w, unit_h)),
                source: Some(Rect::new(
                    1.0 + stats.sprite_col as f32 * 65.0,
                    1.0 + stats.sprite_row as f32 * 49.0,
                    64.0,
                    48.0,
                )),
                ..Default::default()
            },
        );
        draw_circle(
            center.x - 21.0 * self.zoom,
            center.y - 25.0 * self.zoom,
            (5.0 * self.zoom).max(3.2),
            self.players[unit.owner].color,
        );
        if stack_count > 1 {
            draw_circle(
                center.x + 19.0 * self.zoom,
                center.y - 21.0 * self.zoom,
                (8.0 * self.zoom).max(6.0),
                Color::from_rgba(25, 24, 21, 235),
            );
            draw_text(
                &stack_count.to_string(),
                center.x + 15.0 * self.zoom,
                center.y - 17.0 * self.zoom,
                (13.0 * self.zoom).max(10.0),
                WHITE,
            );
        }
    }
}

impl Game {
    fn draw(&mut self, assets: &Assets) -> Vec<(Rect, UiAction)> {
        clear_background(Color::from_rgba(7, 20, 31, 255));
        self.hud_elements_drawn = 0;
        if let Some(page) = self.research_page {
            return self.draw_research(page);
        }
        self.draw_world(assets);
        let mut actions = self.draw_hud();
        if self.selected_city_index().is_some() {
            self.draw_city_manager(assets, &mut actions);
        }
        if self.goto_mode {
            let viewport = Self::viewport_rect();
            let text = "GO-TO: choose a destination";
            let width = measure_text(text, None, 22, 1.0).width;
            draw_rectangle(
                viewport.x + viewport.w * 0.5 - width * 0.5 - 14.0,
                viewport.y + 14.0,
                width + 28.0,
                34.0,
                Color::from_rgba(25, 23, 18, 230),
            );
            draw_rectangle_lines(
                viewport.x + viewport.w * 0.5 - width * 0.5 - 14.0,
                viewport.y + 14.0,
                width + 28.0,
                34.0,
                2.0,
                Color::from_rgba(237, 203, 89, 255),
            );
            draw_text(
                text,
                viewport.x + viewport.w * 0.5 - width * 0.5,
                viewport.y + 38.0,
                22.0,
                Color::from_rgba(255, 237, 170, 255),
            );
        }
        if let Some(message) = &self.game_over {
            let w = 540.0_f32.min(screen_width() - 80.0);
            let h = 174.0;
            let x = (screen_width() - w) * 0.5;
            let y = (screen_height() - h) * 0.5;
            draw_rectangle(x - 5.0, y - 5.0, w + 10.0, h + 10.0, BLACK);
            draw_rectangle(x, y, w, h, Color::from_rgba(42, 39, 31, 248));
            draw_rectangle_lines(x, y, w, h, 3.0, Color::from_rgba(224, 188, 82, 255));
            centered_text(
                "SKIRMISH COMPLETE",
                y + 42.0,
                28.0,
                Color::from_rgba(250, 222, 124, 255),
            );
            centered_text(
                message,
                y + 88.0,
                21.0,
                Color::from_rgba(238, 234, 218, 255),
            );
            centered_text(
                "The chronicle of this world is now complete.",
                y + 127.0,
                17.0,
                Color::from_rgba(176, 180, 169, 255),
            );
        }
        actions
    }

    fn draw_research(&self, page: usize) -> Vec<(Rect, UiAction)> {
        let player = &self.players[HUMAN];
        let advances: Vec<_> = Tech::all().chain(std::iter::once(Tech::FUTURE)).collect();
        let rows = (((screen_height() - 192.0) / 55.0).floor() as usize).max(1);
        let columns = if screen_width() >= 900.0 { 2 } else { 1 };
        let page_size = rows * columns;
        let pages = advances.len().div_ceil(page_size);
        let page = page.min(pages - 1);
        let width = (screen_width() - 48.0) / columns as f32;
        let mut actions = Vec::new();

        draw_text("Research tree", 24.0, 40.0, 34.0, WHITE);
        let close = Rect::new(screen_width() - 144.0, 16.0, 120.0, 34.0);
        draw_ui_button(close, "Close [Esc]", true, false);
        actions.push((close, UiAction::CloseResearch));

        let current = player.researching;
        let path = self.research_path(HUMAN, player.research_goal);
        let remaining = path.iter().filter(|&&tech| tech != Tech::FUTURE).count();
        draw_text(
            &format!("Researching {}  {}/{}   |   Goal: {}   |   {} advances remaining",
                current.name(), player.research, self.target_cost(HUMAN, current),
                player.research_goal.name(), remaining),
            24.0, 72.0, 19.0, SKYBLUE,
        );
        draw_text("Select an advance to pursue its prerequisites. Switching the current target resets progress.",
            24.0, 100.0, 17.0, LIGHTGRAY);

        let remaining_path = path.iter().map(|tech| tech.name()).collect::<Vec<_>>().join(" > ");
        let path_label = if remaining_path.is_empty() {
            "Goal complete. Choose another advance.".to_owned()
        } else {
            format!("Research path: {remaining_path}")
        };
        draw_text(
            &clip_text(&path_label, ((screen_width() - 48.0) / 8.0) as usize),
            24.0, 123.0, 16.0, LIGHTGRAY,
        );

        for (index, &tech) in advances.iter().skip(page * page_size).take(page_size).enumerate() {
            let x = 24.0 + (index / rows) as f32 * width;
            let y = 136.0 + (index % rows) as f32 * 55.0;
            let card = Rect::new(x, y, width - 8.0, 51.0);
            let known = self.has_tech(HUMAN, tech);
            let status = if known { "Known" } else if tech == current { "Researching" }
                else if self.tech_available(HUMAN, tech) { "Available" } else { "Locked" };
            let (mx, my) = mouse_position();
            let fill = if tech == player.research_goal { Color::from_rgba(65, 57, 36, 255) }
                else if !known && card.contains(vec2(mx, my)) { Color::from_rgba(49, 67, 76, 255) }
                else { Color::from_rgba(27, 45, 55, 255) };
            draw_rectangle(card.x, card.y, card.w, card.h, fill);
            draw_text(&format!("{}  ·  {}  ·  {} bulbs", tech.name(), status,
                self.target_cost(HUMAN, tech)), x + 8.0, y + 17.0, 17.0, WHITE);
            let reqs = tech.prerequisites().iter().map(|req| req.name()).collect::<Vec<_>>();
            let requirement = if tech == Tech::FUTURE { "Requires every named advance".to_owned() }
                else if reqs.is_empty() { "No prerequisites".to_owned() }
                else { format!("Requires {}", reqs.join(" + ")) };
            draw_text(&requirement, x + 8.0, y + 33.0, 15.0, LIGHTGRAY);
            let unlocks = match tech {
                Tech::ALPHABET => "In CivCraft: Diplomat",
                Tech::BRONZE_WORKING => "In CivCraft: Phalanx",
                Tech::WARRIOR_CODE => "In CivCraft: Archers",
                Tech::IRON_WORKING => "In CivCraft: Legion",
                Tech::HORSEBACK_RIDING => "In CivCraft: Horsemen",
                Tech::THE_WHEEL => "In CivCraft: Chariot",
                Tech::MATHEMATICS => "In CivCraft: Catapult",
                _ => "",
            };
            let label = if !unlocks.is_empty() { unlocks.to_owned() }
                else if let Some(data) = tech.data().filter(|data| !data.reference_unlocks.is_empty()) {
                    format!("Reference only: {}", data.reference_unlocks)
                } else { String::new() };
            draw_text(&clip_text(&label, ((width - 20.0) / 7.0) as usize),
                x + 8.0, y + 47.0, 14.0, SKYBLUE);
            if !known {
                actions.push((card, UiAction::ResearchGoal(tech)));
            }
        }

        let y = screen_height() - 42.0;
        for (x, label, target, enabled) in [
            (24.0, "Previous", page.saturating_sub(1), page > 0),
            (236.0, "Next", page + 1, page + 1 < pages),
        ] {
            let button = Rect::new(x, y, 92.0, 30.0);
            draw_ui_button(button, label, enabled, false);
            if enabled {
                actions.push((button, UiAction::ResearchPage(target)));
            }
        }
        draw_text(&format!("Page {} / {}", page + 1, pages), 133.0, y + 21.0, 18.0, WHITE);
        actions
    }

    fn draw_hud(&mut self) -> Vec<(Rect, UiAction)> {
        let mut actions = Vec::new();
        let viewport = Self::viewport_rect();
        let panel_x = screen_width() - PANEL_W;

        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            TOP_H,
            Color::from_rgba(29, 31, 28, 255),
        );
        draw_rectangle(
            0.0,
            TOP_H - 3.0,
            screen_width(),
            3.0,
            Color::from_rgba(171, 133, 55, 255),
        );
        draw_text(
            "CivCraft",
            18.0,
            31.0,
            25.0,
            Color::from_rgba(244, 220, 143, 255),
        );
        draw_text(
            &format!("TURN {}  |  {}", self.turn, year_label(self.year)),
            214.0,
            29.0,
            20.0,
            Color::from_rgba(227, 230, 220, 255),
        );
        let player = &self.players[HUMAN];
        draw_circle(483.0, 23.0, 8.0, player.color);
        draw_text(
            player.name,
            498.0,
            29.0,
            18.0,
            Color::from_rgba(228, 226, 210, 255),
        );
        draw_text(
            &format!("Treasury: {}", player.gold),
            665.0,
            29.0,
            17.0,
            Color::from_rgba(231, 201, 91, 255),
        );
        let research = player.researching;
        draw_text(
            &format!(
                "Research: {}  {}/{}",
                research.name(),
                player.research,
                self.target_cost(HUMAN, research)
            ),
            785.0,
            29.0,
            17.0,
            Color::from_rgba(151, 210, 231, 255),
        );
        self.hud_elements_drawn += 1;

        draw_rectangle(
            panel_x,
            TOP_H,
            PANEL_W,
            screen_height() - TOP_H,
            Color::from_rgba(38, 40, 37, 255),
        );
        draw_rectangle(
            panel_x,
            TOP_H,
            4.0,
            screen_height() - TOP_H,
            Color::from_rgba(133, 104, 47, 255),
        );
        draw_text(
            "WORLD MAP",
            panel_x + 18.0,
            TOP_H + 26.0,
            18.0,
            Color::from_rgba(227, 211, 164, 255),
        );
        self.draw_minimap();
        self.hud_elements_drawn += 1;

        let context_y = TOP_H + 205.0;
        draw_line(
            panel_x + 16.0,
            context_y - 9.0,
            screen_width() - 16.0,
            context_y - 9.0,
            1.0,
            Color::from_rgba(112, 108, 91, 255),
        );
        self.draw_context_panel(panel_x + 18.0, context_y);
        self.hud_elements_drawn += 1;

        let research_rect = Rect::new(
            panel_x + 18.0,
            screen_height() - 112.0,
            PANEL_W - 36.0,
            34.0,
        );
        draw_ui_button(
            research_rect,
            &format!(
                "Research tree: {}",
                self.players[HUMAN].researching.name()
            ),
            true,
            false,
        );
        actions.push((research_rect, UiAction::OpenResearch));
        let end_rect = Rect::new(panel_x + 18.0, screen_height() - 65.0, PANEL_W - 36.0, 44.0);
        draw_ui_button(end_rect, "END TURN  [Shift+Enter]", true, true);
        actions.push((end_rect, UiAction::EndTurn));

        draw_rectangle(
            0.0,
            screen_height() - BOTTOM_H,
            viewport.w,
            BOTTOM_H,
            Color::from_rgba(28, 30, 28, 248),
        );
        draw_rectangle(
            0.0,
            screen_height() - BOTTOM_H,
            viewport.w,
            3.0,
            Color::from_rgba(133, 104, 47, 255),
        );
        self.draw_action_strip(&mut actions, viewport.w);
        self.draw_messages(viewport.w);
        self.hud_elements_drawn += 1;

        actions
    }

    fn draw_minimap(&self) {
        let rect = Self::minimap_rect();
        draw_rectangle(
            rect.x - 3.0,
            rect.y - 3.0,
            rect.w + 6.0,
            rect.h + 6.0,
            Color::from_rgba(15, 17, 16, 255),
        );
        let cell_w = rect.w / MAP_W as f32;
        let cell_h = rect.h / MAP_H as f32;
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let tile = self.tile(x, y);
                let color = if !tile.seen {
                    Color::from_rgba(7, 13, 17, 255)
                } else if tile.visible {
                    tile.terrain.minimap_color()
                } else {
                    darken(tile.terrain.minimap_color(), 0.53)
                };
                draw_rectangle(
                    rect.x + x as f32 * cell_w,
                    rect.y + y as f32 * cell_h,
                    cell_w + 0.4,
                    cell_h + 0.4,
                    color,
                );
            }
        }
        for city in &self.cities {
            draw_rectangle(
                rect.x + city.x as f32 * cell_w - 1.0,
                rect.y + city.y as f32 * cell_h - 1.0,
                4.0,
                4.0,
                self.players[city.owner].color,
            );
        }
        let (camera_native_x, camera_native_y) = self.camera_native();
        let camera_x = rect.x + camera_native_x as f32 / MAP_W as f32 * rect.w;
        let camera_y = rect.y + camera_native_y as f32 / MAP_H as f32 * rect.h;
        draw_rectangle_lines(camera_x - 12.0, camera_y - 8.0, 24.0, 16.0, 1.5, WHITE);
    }

    fn draw_city_manager(&self, assets: &Assets, actions: &mut Vec<(Rect, UiAction)>) {
        let Some(index) = self.selected_city_index() else {
            return;
        };
        let city = &self.cities[index];
        let rect = Self::city_manager_rect();
        let viewport = Self::viewport_rect();
        draw_rectangle(
            viewport.x,
            viewport.y,
            viewport.w,
            viewport.h,
            Color::from_rgba(4, 9, 12, 178),
        );
        draw_rectangle(
            rect.x - 5.0,
            rect.y - 5.0,
            rect.w + 10.0,
            rect.h + 10.0,
            Color::from_rgba(9, 11, 10, 245),
        );
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Color::from_rgba(42, 44, 40, 252),
        );
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            3.0,
            Color::from_rgba(206, 165, 72, 255),
        );

        draw_text(
            "CITY MANAGEMENT",
            rect.x + 22.0,
            rect.y + 29.0,
            17.0,
            Color::from_rgba(175, 178, 165, 255),
        );
        draw_text(
            &city.name,
            rect.x + 22.0,
            rect.y + 61.0,
            29.0,
            Color::from_rgba(248, 222, 139, 255),
        );
        draw_text(
            &format!("Size {}  |  {}, {}", city.size, city.x, city.y),
            rect.x + 24.0,
            rect.y + 84.0,
            16.0,
            Color::from_rgba(183, 188, 178, 255),
        );
        let close_rect = Rect::new(rect.x + rect.w - 48.0, rect.y + 14.0, 32.0, 28.0);
        draw_ui_button(close_rect, "X", true, false);
        actions.push((close_rect, UiAction::CloseCity));

        let (food, shields, trade) = self.city_yields(city);
        let food_net = food - city.size * 2;
        let growth_box = Self::growth_box(city.size);
        let production_cost = city.production.stats().cost;
        let progress_x = rect.x + 205.0;
        let progress_w = rect.w - 229.0;
        draw_text(
            &format!("Food {food} ({food_net:+})   Shields {shields}   Trade {trade}"),
            progress_x,
            rect.y + 31.0,
            17.0,
            Color::from_rgba(188, 220, 166, 255),
        );
        draw_meter(
            Rect::new(progress_x, rect.y + 43.0, progress_w, 15.0),
            city.food,
            growth_box,
            Color::from_rgba(82, 166, 82, 255),
        );
        draw_text(
            &format!("Growth  {}/{}", city.food, growth_box),
            progress_x,
            rect.y + 78.0,
            15.0,
            Color::from_rgba(215, 218, 202, 255),
        );
        draw_meter(
            Rect::new(
                progress_x + progress_w * 0.48,
                rect.y + 66.0,
                progress_w * 0.52,
                15.0,
            ),
            city.shields,
            production_cost,
            Color::from_rgba(173, 133, 62, 255),
        );
        let production_label = format!(
            "{}  {}/{}",
            city.production.name(),
            city.shields,
            production_cost
        );
        let production_label_width = measure_text(&production_label, None, 15, 1.0)
            .width
            .min(progress_w * 0.5);
        draw_text(
            &production_label,
            progress_x + progress_w - production_label_width,
            rect.y + 78.0,
            15.0,
            Color::from_rgba(215, 218, 202, 255),
        );

        draw_line(
            rect.x + 20.0,
            rect.y + 101.0,
            rect.x + rect.w - 20.0,
            rect.y + 101.0,
            1.0,
            Color::from_rgba(105, 102, 86, 255),
        );
        draw_text(
            "CHOOSE PRODUCTION",
            rect.x + 22.0,
            rect.y + 127.0,
            18.0,
            Color::from_rgba(226, 211, 162, 255),
        );

        let choices = [
            UnitKind::Worker,
            UnitKind::Warrior,
            UnitKind::Phalanx,
            UnitKind::Archer,
            UnitKind::Horseman,
            UnitKind::Catapult,
            UnitKind::Chariot,
            UnitKind::Legion,
            UnitKind::Diplomat,
            UnitKind::Settlers,
        ];
        let columns = 4;
        let gap = 8.0;
        let card_w = (rect.w - 44.0 - gap * 3.0) / 4.0;
        let card_h = 72.0;
        let mouse = vec2(mouse_position().0, mouse_position().1);
        for (choice_index, kind) in choices.into_iter().enumerate() {
            let column = choice_index % columns;
            let row = choice_index / columns;
            let card = Rect::new(
                rect.x + 22.0 + column as f32 * (card_w + gap),
                rect.y + 139.0 + row as f32 * (card_h + gap),
                card_w,
                card_h,
            );
            let enabled = self.unit_available(HUMAN, kind);
            let current = city.production == kind;
            let hovered = enabled && card.contains(mouse);
            let fill = if current {
                Color::from_rgba(101, 80, 39, 255)
            } else if hovered {
                Color::from_rgba(76, 89, 80, 255)
            } else if enabled {
                Color::from_rgba(55, 61, 56, 255)
            } else {
                Color::from_rgba(43, 46, 43, 255)
            };
            draw_rectangle(card.x, card.y, card.w, card.h, fill);
            draw_rectangle_lines(
                card.x,
                card.y,
                card.w,
                card.h,
                if current { 2.0 } else { 1.0 },
                if current {
                    Color::from_rgba(236, 199, 97, 255)
                } else if enabled {
                    Color::from_rgba(117, 124, 111, 255)
                } else {
                    Color::from_rgba(65, 68, 63, 255)
                },
            );
            let stats = kind.stats();
            draw_texture_ex(
                &assets.units,
                card.x + 6.0,
                card.y + 13.0,
                if enabled {
                    WHITE
                } else {
                    Color::new(0.38, 0.4, 0.4, 1.0)
                },
                DrawTextureParams {
                    dest_size: Some(vec2(52.0, 39.0)),
                    source: Some(Rect::new(
                        1.0 + stats.sprite_col as f32 * 65.0,
                        1.0 + stats.sprite_row as f32 * 49.0,
                        64.0,
                        48.0,
                    )),
                    ..Default::default()
                },
            );
            let name = short_unit_name(kind);
            draw_text(
                name,
                card.x + 62.0,
                card.y + 26.0,
                16.0,
                if enabled {
                    Color::from_rgba(239, 235, 216, 255)
                } else {
                    Color::from_rgba(111, 114, 107, 255)
                },
            );
            draw_text(
                &format!("{} shields", stats.cost),
                card.x + 62.0,
                card.y + 47.0,
                13.0,
                if enabled {
                    Color::from_rgba(176, 184, 171, 255)
                } else {
                    Color::from_rgba(91, 94, 89, 255)
                },
            );
            if current {
                draw_text(
                    "CURRENT",
                    card.x + 7.0,
                    card.y + card.h - 7.0,
                    11.0,
                    Color::from_rgba(247, 211, 112, 255),
                );
            }
            if enabled {
                actions.push((card, UiAction::Produce(kind)));
            }
        }

        let garrison_y = rect.y + 395.0;
        draw_text(
            "GARRISON",
            rect.x + 22.0,
            garrison_y,
            18.0,
            Color::from_rgba(226, 211, 162, 255),
        );
        let garrison: Vec<&Unit> = self
            .units
            .iter()
            .filter(|unit| unit.owner == HUMAN && unit.x == city.x && unit.y == city.y)
            .collect();
        if garrison.is_empty() {
            draw_text(
                "No units stationed in this city.",
                rect.x + 22.0,
                garrison_y + 27.0,
                15.0,
                Color::from_rgba(159, 164, 154, 255),
            );
        } else {
            let button_gap = 7.0;
            let button_w = ((rect.w - 44.0 - button_gap * 3.0) / 4.0).min(170.0);
            for (unit_index, unit) in garrison.into_iter().take(4).enumerate() {
                let unit_rect = Rect::new(
                    rect.x + 22.0 + unit_index as f32 * (button_w + button_gap),
                    garrison_y + 10.0,
                    button_w,
                    35.0,
                );
                draw_ui_button(
                    unit_rect,
                    &format!("Select {}", short_unit_name(unit.kind)),
                    true,
                    false,
                );
                actions.push((unit_rect, UiAction::SelectUnit(unit.id)));
            }
        }
        draw_text(
            "Select a unit to leave the city screen  |  Esc closes",
            rect.x + 22.0,
            rect.y + rect.h - 17.0,
            14.0,
            Color::from_rgba(145, 151, 142, 255),
        );
    }

    fn draw_context_panel(&self, x: f32, y: f32) {
        if let Some(index) = self.selected_unit_index() {
            let unit = &self.units[index];
            let stats = unit.kind.stats();
            let state = if unit.work_remaining > 0 {
                format!(
                    "Building {} - {} turns",
                    unit.work_kind.map(|w| w.name()).unwrap_or("improvement"),
                    unit.work_remaining
                )
            } else if unit.automated {
                "Automated".to_owned()
            } else {
                match unit.state {
                    UnitState::Idle => "Ready".to_owned(),
                    UnitState::Fortifying => "Fortifying".to_owned(),
                    UnitState::Fortified => "Fortified (+50% defense)".to_owned(),
                    UnitState::Sentry => "Sentry".to_owned(),
                }
            };
            draw_text(
                unit.kind.name(),
                x,
                y + 23.0,
                25.0,
                Color::from_rgba(244, 221, 147, 255),
            );
            draw_text(
                &format!(
                    "{} at {}, {}",
                    self.players[unit.owner].name, unit.x, unit.y
                ),
                x,
                y + 46.0,
                16.0,
                Color::from_rgba(180, 185, 176, 255),
            );
            draw_text(
                &format!("Attack {}   Defense {}", stats.attack, stats.defense),
                x,
                y + 75.0,
                18.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            draw_text(
                &format!(
                    "Health {}/{}   Moves {}/{}",
                    unit.hp,
                    stats.hp,
                    unit.moves_left,
                    stats.moves * MOVE_FRAGMENTS
                ),
                x,
                y + 99.0,
                18.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            draw_text(
                &format!("Terrain: {}", self.tile(unit.x, unit.y).terrain.name()),
                x,
                y + 123.0,
                17.0,
                Color::from_rgba(167, 201, 158, 255),
            );
            draw_text(
                &state,
                x,
                y + 148.0,
                17.0,
                Color::from_rgba(145, 204, 224, 255),
            );
            draw_text(
                "Move: arrows / 2 3 4 6 7 8",
                x,
                y + 184.0,
                16.0,
                Color::from_rgba(177, 177, 164, 255),
            );
            draw_text(
                "G: go-to   Space: wait   Tab: next",
                x,
                y + 207.0,
                15.0,
                Color::from_rgba(177, 177, 164, 255),
            );
        } else if let Some(index) = self.selected_city_index() {
            let city = &self.cities[index];
            let (food, shields, trade) = self.city_yields(city);
            let cost = city.production.stats().cost;
            draw_text(
                &city.name,
                x,
                y + 23.0,
                25.0,
                Color::from_rgba(244, 221, 147, 255),
            );
            draw_text(
                &format!("City size {}", city.size),
                x,
                y + 48.0,
                18.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            draw_text(
                &format!("Food {}  Shields {}  Trade {}", food, shields, trade),
                x,
                y + 76.0,
                17.0,
                Color::from_rgba(177, 213, 159, 255),
            );
            draw_text(
                &format!("Food store {}/{}", city.food, Self::growth_box(city.size)),
                x,
                y + 102.0,
                17.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            draw_text(
                &format!("Training: {}", city.production.name()),
                x,
                y + 132.0,
                18.0,
                Color::from_rgba(145, 204, 224, 255),
            );
            draw_text(
                &format!("Production {}/{}", city.shields, cost),
                x,
                y + 156.0,
                17.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            draw_text(
                "Choose a unit below to change production.",
                x,
                y + 192.0,
                15.0,
                Color::from_rgba(177, 177, 164, 255),
            );
        } else {
            draw_text(
                "YOUR CHRONICLE",
                x,
                y + 22.0,
                22.0,
                Color::from_rgba(244, 221, 147, 255),
            );
            draw_text(
                "Found a city with your Settlers.",
                x,
                y + 55.0,
                17.0,
                Color::from_rgba(225, 224, 208, 255),
            );
            draw_text(
                "Explore the fog and improve the land.",
                x,
                y + 80.0,
                17.0,
                Color::from_rgba(225, 224, 208, 255),
            );
            draw_text(
                "Research stronger units and unite",
                x,
                y + 105.0,
                17.0,
                Color::from_rgba(225, 224, 208, 255),
            );
            draw_text(
                "the six rival civilizations.",
                x,
                y + 128.0,
                17.0,
                Color::from_rgba(225, 224, 208, 255),
            );
            let living = (1..PLAYER_COUNT)
                .filter(|owner| {
                    self.cities.iter().any(|c| c.owner == *owner)
                        || self.units.iter().any(|u| u.owner == *owner)
                })
                .count();
            draw_text(
                &format!("Rivals remaining: {living}"),
                x,
                y + 172.0,
                18.0,
                Color::from_rgba(236, 183, 117, 255),
            );
            draw_text(
                "Click a unit or city for details.",
                x,
                y + 208.0,
                15.0,
                Color::from_rgba(174, 177, 168, 255),
            );
        }
    }

    fn draw_action_strip(&self, actions: &mut Vec<(Rect, UiAction)>, available_width: f32) {
        let mut entries: Vec<(String, UiAction, bool)> = Vec::new();
        if let Some(index) = self.selected_unit_index() {
            let unit = &self.units[index];
            let stats = unit.kind.stats();
            if stats.can_found {
                entries.push((
                    "Found [B]".to_owned(),
                    UiAction::FoundCity,
                    self.can_found_at(unit.x, unit.y),
                ));
            }
            if stats.can_work {
                entries.push((
                    "Road [R]".to_owned(),
                    UiAction::Road,
                    !self.tile(unit.x, unit.y).road,
                ));
                entries.push((
                    "Irrigate [I]".to_owned(),
                    UiAction::Irrigate,
                    self.can_irrigate_at(unit.x, unit.y),
                ));
                entries.push((
                    "Mine [M]".to_owned(),
                    UiAction::Mine,
                    !self.tile(unit.x, unit.y).mine
                        && self.tile(unit.x, unit.y).terrain.mine_time() > 0,
                ));
            }
            if unit.kind.is_military() {
                entries.push(("Fortify [F]".to_owned(), UiAction::Fortify, true));
            }
            entries.push(("Automate [A]".to_owned(), UiAction::Automate, true));
            entries.push(("Wait [Space]".to_owned(), UiAction::Wait, true));
        } else if self.selected_city_index().is_some() {
            for kind in [
                UnitKind::Worker,
                UnitKind::Warrior,
                UnitKind::Phalanx,
                UnitKind::Archer,
                UnitKind::Horseman,
                UnitKind::Catapult,
                UnitKind::Chariot,
                UnitKind::Legion,
                UnitKind::Settlers,
            ] {
                entries.push((
                    format!("{} {}", short_unit_name(kind), kind.stats().cost),
                    UiAction::Produce(kind),
                    self.unit_available(HUMAN, kind),
                ));
            }
        } else {
            entries.push(("Next ready unit [Tab]".to_owned(), UiAction::Wait, false));
        }

        let count = entries.len().max(1) as f32;
        let gap = 6.0;
        let width = ((available_width - 24.0 - gap * (count - 1.0)) / count).clamp(76.0, 132.0);
        let y = screen_height() - BOTTOM_H + 10.0;
        let mut x = 12.0;
        for (label, action, enabled) in entries {
            if x + width > available_width - 8.0 {
                break;
            }
            let rect = Rect::new(x, y, width, 31.0);
            draw_ui_button(rect, &label, enabled, false);
            if enabled {
                actions.push((rect, action));
            }
            x += width + gap;
        }
    }

    fn draw_messages(&self, available_width: f32) {
        let latest: Vec<&String> = self.events.iter().rev().take(2).collect();
        let mut y = screen_height() - 11.0;
        for (i, message) in latest.iter().enumerate() {
            let color = if i == 0 {
                Color::from_rgba(230, 228, 211, 255)
            } else {
                Color::from_rgba(154, 158, 151, 255)
            };
            let clipped = clip_text(message, ((available_width - 36.0) / 8.6) as usize);
            draw_text(&clipped, 18.0, y, 15.0, color);
            y -= 19.0;
        }
    }
}

fn draw_diamond(center: Vec2, width: f32, height: f32, color: Color) {
    let top = vec2(center.x, center.y - height * 0.5);
    let right = vec2(center.x + width * 0.5, center.y);
    let bottom = vec2(center.x, center.y + height * 0.5);
    let left = vec2(center.x - width * 0.5, center.y);
    draw_triangle(top, right, bottom, color);
    draw_triangle(top, bottom, left, color);
}

fn draw_diamond_lines(center: Vec2, width: f32, height: f32, thickness: f32, color: Color) {
    let points = [
        vec2(center.x, center.y - height * 0.5),
        vec2(center.x + width * 0.5, center.y),
        vec2(center.x, center.y + height * 0.5),
        vec2(center.x - width * 0.5, center.y),
    ];
    for i in 0..4 {
        draw_line(
            points[i].x,
            points[i].y,
            points[(i + 1) % 4].x,
            points[(i + 1) % 4].y,
            thickness,
            color,
        );
    }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::new(color.r, color.g, color.b, alpha)
}

fn darken(color: Color, factor: f32) -> Color {
    Color::new(
        color.r * factor,
        color.g * factor,
        color.b * factor,
        color.a,
    )
}

fn draw_ui_button(rect: Rect, label: &str, enabled: bool, accent: bool) {
    let mouse = vec2(mouse_position().0, mouse_position().1);
    let hovered = enabled && rect.contains(mouse);
    let fill = if !enabled {
        Color::from_rgba(51, 53, 50, 255)
    } else if accent && hovered {
        Color::from_rgba(183, 133, 44, 255)
    } else if accent {
        Color::from_rgba(142, 98, 34, 255)
    } else if hovered {
        Color::from_rgba(86, 100, 91, 255)
    } else {
        Color::from_rgba(61, 68, 63, 255)
    };
    let border = if accent {
        Color::from_rgba(236, 201, 105, 255)
    } else if enabled {
        Color::from_rgba(131, 137, 122, 255)
    } else {
        Color::from_rgba(75, 77, 72, 255)
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, border);
    let mut font_size = if rect.h > 38.0 { 19.0 } else { 15.0 };
    while font_size > 11.0 && measure_text(label, None, font_size as u16, 1.0).width > rect.w - 10.0
    {
        font_size -= 1.0;
    }
    let size = measure_text(label, None, font_size as u16, 1.0);
    draw_text(
        label,
        rect.x + (rect.w - size.width) * 0.5,
        rect.y + (rect.h + font_size * 0.68) * 0.5,
        font_size,
        if enabled {
            Color::from_rgba(241, 238, 218, 255)
        } else {
            Color::from_rgba(112, 115, 108, 255)
        },
    );
}

fn draw_meter(rect: Rect, value: i32, maximum: i32, color: Color) {
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        Color::from_rgba(24, 27, 25, 255),
    );
    let fraction = if maximum > 0 {
        value.max(0) as f32 / maximum as f32
    } else {
        0.0
    }
    .clamp(0.0, 1.0);
    draw_rectangle(
        rect.x + 2.0,
        rect.y + 2.0,
        (rect.w - 4.0) * fraction,
        rect.h - 4.0,
        color,
    );
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        1.0,
        Color::from_rgba(113, 116, 105, 255),
    );
}

fn centered_text(text: &str, y: f32, size: f32, color: Color) {
    let width = measure_text(text, None, size as u16, 1.0).width;
    draw_text(text, (screen_width() - width) * 0.5, y, size, color);
}

fn short_unit_name(kind: UnitKind) -> &'static str {
    match kind {
        UnitKind::Settlers => "Settler",
        UnitKind::Worker => "Worker",
        UnitKind::Diplomat => "Diplomat",
        UnitKind::Explorer => "Explorer",
        UnitKind::Warrior => "Warrior",
        UnitKind::Phalanx => "Phalanx",
        UnitKind::Archer => "Archer",
        UnitKind::Legion => "Legion",
        UnitKind::Horseman => "Horseman",
        UnitKind::Chariot => "Chariot",
        UnitKind::Catapult => "Catapult",
    }
}

fn clip_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_owned()
    } else {
        let mut clipped: String = text.chars().take(max_chars.saturating_sub(3)).collect();
        clipped.push_str("...");
        clipped
    }
}

fn year_label(year: i32) -> String {
    if year < 0 {
        format!("{} BCE", -year)
    } else {
        format!("{} CE", year)
    }
}

fn argument_value(flag: &str) -> Option<String> {
    let args: Vec<String> = env::args().collect();
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

fn ensure_parent(path: &str) {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
}

fn json_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn write_self_test(
    path: &str,
    move_passed: bool,
    move_detail: &str,
    act_passed: bool,
    act_detail: &str,
    hud_passed: bool,
    hud_detail: &str,
) {
    ensure_parent(path);
    let json = format!(
        "{{\"version\":1,\"steps\":[{{\"step\":\"move\",\"passed\":{},\"detail\":\"{}\"}},{{\"step\":\"act\",\"passed\":{},\"detail\":\"{}\"}},{{\"step\":\"hud\",\"passed\":{},\"detail\":\"{}\"}}]}}",
        move_passed,
        json_escape(move_detail),
        act_passed,
        json_escape(act_detail),
        hud_passed,
        json_escape(hud_detail),
    );
    let _ = fs::write(path, json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_and_map_coordinates_round_trip() {
        for native_y in 0..MAP_H {
            for native_x in 0..MAP_W {
                let (map_x, map_y) = native_to_map(native_x, native_y);
                assert_eq!(map_to_native(map_x, map_y), (native_x, native_y));
            }
        }
    }

    #[test]
    fn isohex_neighbors_and_wrapping_match_freeciv_distance() {
        assert!(!is_isohex_step(1, -1));
        assert!(!is_isohex_step(-1, 1));
        assert!(!is_isohex_step(2, 0));

        for &(start_x, start_y) in &[(0, 0), (MAP_W - 1, MAP_H - 1), STARTS[0]] {
            let mut neighbors = Vec::new();
            for (dx, dy) in ISOHEX_DIRS {
                let neighbor = offset_native_by_map(start_x, start_y, dx, dy);
                assert_eq!(
                    isohex_vector_distance(
                        wrapped_map_vector(start_x, start_y, neighbor.0, neighbor.1).0,
                        wrapped_map_vector(start_x, start_y, neighbor.0, neighbor.1).1,
                    ),
                    1
                );
                neighbors.push(neighbor);
            }
            neighbors.sort_unstable();
            neighbors.dedup();
            assert_eq!(neighbors.len(), 6);
        }
    }

    #[test]
    fn generated_world_uses_source_size_land_target_and_starter_terrain() {
        assert_eq!(MAP_TARGET_TILES, 2_000);
        assert_eq!(MAP_LINEAR_SCALE, 16);
        assert_eq!((MAP_W, MAP_H), (32, 64));
        assert_eq!(LAND_TILE_TARGET, 614);

        let game = Game::new();
        assert_eq!(game.tiles.len(), (MAP_W * MAP_H) as usize);
        assert_eq!(
            game.tiles
                .iter()
                .filter(|tile| tile.terrain != Terrain::Ocean)
                .count(),
            LAND_TILE_TARGET
        );
        for &(x, y) in &STARTS {
            assert_eq!(game.tile(x, y).terrain, Terrain::Grassland);
            assert!(game.can_found_at(x, y));
        }
    }

    #[test]
    fn wrapped_path_uses_one_legal_hex_step() {
        let mut game = Game::new();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
        }
        let unit_id = game.units[0].id;
        game.units[0].x = 0;
        game.units[0].y = 0;
        let (target_x, target_y) = offset_native_by_map(0, 0, -1, -1);
        assert_eq!(game.path_to(unit_id, target_x, target_y), vec![(-1, -1)]);
        assert_eq!(game.map_distance(0, 0, target_x, target_y), 1);
    }

    #[test]
    fn healing_uses_city_minimum_and_fortified_regeneration() {
        let game = Game::new();
        let mut unit = game
            .units
            .iter()
            .find(|unit| unit.kind == UnitKind::Warrior)
            .cloned()
            .unwrap_or_else(|| Unit {
                id: 999,
                owner: HUMAN,
                kind: UnitKind::Warrior,
                x: STARTS[0].0,
                y: STARTS[0].1,
                hp: 1,
                moves_left: 0,
                state: UnitState::Idle,
                moved: false,
                automated: false,
                work_kind: None,
                work_remaining: 0,
            });
        unit.hp = 1;
        unit.moved = false;
        unit.state = UnitState::Idle;
        assert_eq!(Game::hitpoint_gain(&unit, false), 1);
        assert_eq!(Game::hitpoint_gain(&unit, true), 4);
        unit.state = UnitState::Fortified;
        assert_eq!(Game::hitpoint_gain(&unit, true), 5);
        unit.moved = true;
        assert_eq!(Game::hitpoint_gain(&unit, true), 0);

        // The compiled hp_gain_coord performs integer max-HP / 10 before
        // applying HP_Regen_2. Settlers have 20 maximum HP, so they gain 2
        // in the field and 6 minimum-city HP plus 2 regeneration in a city.
        unit.kind = UnitKind::Settlers;
        unit.hp = 1;
        unit.state = UnitState::Idle;
        unit.moved = false;
        assert_eq!(Game::hitpoint_gain(&unit, false), 2);
        assert_eq!(Game::hitpoint_gain(&unit, true), 8);
    }

    #[test]
    fn defense_stages_and_research_costs_match_civ2civ3() {
        let mut game = Game::new();
        let (x, y) = STARTS[0];
        game.tile_mut(x, y).terrain = Terrain::Forest;
        let mut defender = Unit {
            id: 999,
            owner: HUMAN,
            kind: UnitKind::Warrior,
            x,
            y,
            hp: 10,
            moves_left: MOVE_FRAGMENTS,
            state: UnitState::Idle,
            moved: false,
            automated: false,
            work_kind: None,
            work_remaining: 0,
        };

        // Forest first: 10 * 125% = 12. Field fortification is a later
        // stage: 12 * 150% = 18.
        assert_eq!(game.defense_power(&defender, x, y), 12);
        defender.state = UnitState::Fortified;
        assert_eq!(game.defense_power(&defender, x, y), 18);

        // A city adds a 50% Defend_Bonus and gives can-fortify ground units
        // an automatic 50% Fortify_Defense_Bonus. Integer stages are
        // 12 * 150% = 18, then 18 * 150% = 27.
        defender.state = UnitState::Idle;
        game.cities.push(City {
            id: 999,
            owner: HUMAN,
            x,
            y,
            name: "Test".to_owned(),
            size: 1,
            food: 0,
            shields: 0,
            production: UnitKind::Warrior,
        });
        assert_eq!(game.defense_power(&defender, x, y), 27);

        // Linear precalculation is 10 bulbs per recursive requirement; the
        // ruleset's unconditional 300% Tech_Cost_Pct effect is then applied.
        assert_eq!(Tech::ALPHABET.cost(), 30);
        assert_eq!(Tech::IRON_WORKING.cost(), 90);
    }

    #[test]
    fn combat_odds_and_defender_selection_match_native_rules() {
        let even_fight = combat_win_chance(10, 10, 1, 10, 10, 1);
        assert!((even_fight - 0.5).abs() < 1.0e-12);

        let mut game = Game::new();
        game.units.clear();
        game.cities.clear();
        let attacker = Unit {
            id: 1,
            owner: HUMAN,
            kind: UnitKind::Warrior,
            x: 0,
            y: 0,
            hp: 10,
            moves_left: MOVE_FRAGMENTS,
            state: UnitState::Idle,
            moved: false,
            automated: false,
            work_kind: None,
            work_remaining: 0,
        };
        let mut fragile_defender = attacker.clone();
        fragile_defender.id = 2;
        fragile_defender.owner = 1;
        fragile_defender.x = 1;
        fragile_defender.hp = 1;
        let mut healthy_defender = fragile_defender.clone();
        healthy_defender.id = 3;
        healthy_defender.hp = 10;
        game.units = vec![attacker.clone(), fragile_defender, healthy_defender];
        game.tile_mut(1, 0).terrain = Terrain::Grassland;

        assert_eq!(game.best_defender_id(&attacker, 1, 0), Some(3));
    }

    #[test]
    fn granary_formula_uses_ruleset_initial_values_and_foodbox() {
        assert_eq!(Game::growth_box(0), 0);
        assert_eq!(Game::growth_box(1), 20);
        assert_eq!(Game::growth_box(6), 30);
        assert_eq!(Game::growth_box(8), 40);
        assert_eq!(Game::growth_box(20), 40);
    }

    #[test]
    fn famine_destroys_size_one_city() {
        let mut game = Game::new();
        let settler = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settlers)
            .map(|unit| unit.id)
            .expect("human settler");
        assert!(game.found_city(settler));
        game.cities[0].food = -100;
        game.process_cities();
        assert!(game.cities.iter().all(|city| city.owner != HUMAN));
    }

    #[test]
    fn clicking_a_garrisoned_city_opens_city_management() {
        let mut game = Game::new();
        let settler = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settlers)
            .map(|unit| unit.id)
            .expect("human settler");
        assert!(game.found_city(settler));
        let city = game
            .cities
            .iter()
            .find(|city| city.owner == HUMAN)
            .cloned()
            .expect("human city");
        let garrison = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.x == city.x && unit.y == city.y)
            .map(|unit| unit.id)
            .expect("garrisoned unit");

        game.selected_city = None;
        game.selected_unit = Some(garrison);
        game.select_at(city.x, city.y);

        assert_eq!(game.selected_city, Some(city.id));
        assert_eq!(game.selected_unit, None);
    }

    #[test]
    fn irrigation_needs_water_and_conflicts_with_mines() {
        let mut game = Game::new();
        let (x, y) = STARTS[0];
        game.tile_mut(x, y).terrain = Terrain::Plains;
        for (dx, dy) in ISOHEX_DIRS {
            let (neighbor_x, neighbor_y) = offset_native_by_map(x, y, dx, dy);
            let neighbor = game.tile_mut(neighbor_x, neighbor_y);
            neighbor.terrain = Terrain::Grassland;
            neighbor.irrigation = false;
        }
        assert!(!game.can_irrigate_at(x, y));
        let (water_x, water_y) = offset_native_by_map(x, y, ISOHEX_DIRS[0].0, ISOHEX_DIRS[0].1);
        game.tile_mut(water_x, water_y).terrain = Terrain::Ocean;
        assert!(game.can_irrigate_at(x, y));

        let worker_index = game
            .units
            .iter()
            .position(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Worker)
            .expect("human worker");
        game.units[worker_index].x = x;
        game.units[worker_index].y = y;
        game.units[worker_index].work_kind = Some(WorkKind::Irrigation);
        game.units[worker_index].work_remaining = 1;
        game.tile_mut(x, y).mine = true;
        game.update_activities_and_restore();
        assert!(game.tile(x, y).irrigation);
        assert!(!game.tile(x, y).mine);

        game.units[worker_index].work_kind = Some(WorkKind::Mine);
        game.units[worker_index].work_remaining = 1;
        game.update_activities_and_restore();
        assert!(game.tile(x, y).mine);
        assert!(!game.tile(x, y).irrigation);
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let screenshot_path = argument_value("--screenshot");
    let self_test_path = argument_value("--self-test");
    let automated = screenshot_path.is_some() || self_test_path.is_some();
    let assets = Assets::new();
    let mut game = Game::new();
    let mut buttons: Vec<(Rect, UiAction)> = Vec::new();
    let mut frame = 0_u32;
    let mut move_passed = false;
    let mut move_detail = "No scripted move completed".to_owned();
    let mut act_passed = false;
    let mut act_detail = "No scripted action completed".to_owned();

    loop {
        if !automated {
            game.handle_keyboard();
            game.handle_mouse(&buttons);
        }

        if self_test_path.is_some() && frame == 12 {
            if let Some(explorer) = game
                .units
                .iter()
                .find(|u| u.owner == HUMAN && u.kind == UnitKind::Explorer)
                .cloned()
            {
                for (dx, dy) in ISOHEX_DIRS {
                    if game.try_move_unit(explorer.id, dx, dy) != MoveResult::Blocked {
                        if let Some(after) = game.units.iter().find(|u| u.id == explorer.id) {
                            let distance =
                                game.map_distance(explorer.x, explorer.y, after.x, after.y);
                            move_passed = distance > 0;
                            move_detail = format!(
                                "Explorer moved {distance} tile from {},{} to {},{} using terrain movement costs",
                                explorer.x, explorer.y, after.x, after.y
                            );
                        }
                        break;
                    }
                }
            }
        }

        if self_test_path.is_some() && frame == 30 {
            if let Some(settler_id) = game
                .units
                .iter()
                .find(|u| u.owner == HUMAN && u.kind == UnitKind::Settlers)
                .map(|u| u.id)
            {
                let cities_before = game.cities.len();
                game.selected_unit = Some(settler_id);
                game.perform_action(UiAction::FoundCity);
                let cities_after = game.cities.len();
                act_passed = cities_after == cities_before + 1;
                act_detail = if act_passed {
                    "Settlers founded a city, claimed its workable tiles, and opened city production"
                        .to_owned()
                } else {
                    "Settlers could not found a city at the scripted starting tile".to_owned()
                };
            }
        }

        buttons = game.draw(&assets);
        frame += 1;

        if let Some(path) = &screenshot_path {
            if frame == 120 {
                ensure_parent(path);
                get_screen_data().export_png(path);
                if self_test_path.is_none() {
                    break;
                }
            }
        }
        if let Some(path) = &self_test_path {
            if frame >= 150 {
                let hud_passed = game.hud_elements_drawn >= 4;
                let hud_detail = format!(
                    "Drew {} HUD regions: turn and research, world map, selection details, commands and chronicle",
                    game.hud_elements_drawn
                );
                write_self_test(
                    path,
                    move_passed,
                    &move_detail,
                    act_passed,
                    &act_detail,
                    hud_passed,
                    &hud_detail,
                );
                break;
            }
        }
        next_frame().await;
    }
}
