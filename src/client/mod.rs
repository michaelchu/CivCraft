//! Presentation layer between the simulation and the Macroquad window.
//!
//! [`Game`] owns transient client state such as selection, camera position, and
//! open panels. It reads the campaign from [`crate::sim::Simulation`], draws
//! map and interface layers, and translates input into [`UiAction`] values.
//! Actions are resolved through simulation commands so the client does not
//! implement gameplay rules itself.
//!
//! Rendering is split by screen responsibility: `world` draws map contents,
//! `hud` and the view modules draw controls and dialogs, and `assets` plus
//! `tileset` provide Civ III artwork. `render_backend` owns the supported map
//! projection and painter. UI review helpers exercise these same screens
//! through the normal frame path.

mod actions;
mod assets;
mod audio;
mod sprite_metadata;
pub(crate) use assets::{Assets, window_conf};
pub(crate) use audio::MusicPlayer;
use sprite_metadata::{player_color, unit_art_role};

pub(crate) fn native_campaign_unit_art_playtest() -> (bool, bool, bool) {
    let roster = UnitKind::ALL_NATIVE;
    let roles = roster.into_iter().map(unit_art_role).collect::<Vec<_>>();
    let mapped = roster.iter().all(|kind| {
        kind.conquests_id()
            .is_some_and(|id| unit_art_role(*kind) == format!("unit-{}", id.replace('_', "-")))
    });
    let distinct = roles
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        == 92;
    let labels = roster
        .into_iter()
        .all(|kind| !short_unit_name(kind).is_empty());
    (mapped, distinct, labels)
}
mod city_bar;
mod city_manager;
mod city_map;
#[allow(dead_code)] // Standalone projection helpers are retained for geometry regression fixtures.
mod civ3_geometry;
mod flc;
mod fog;
mod government_view;
mod hud;
mod input;
mod map_layers;
mod map_overlays;
mod map_view;
mod presentation;
mod render_backend;
mod research_tree;
mod research_view;
mod rivers;
mod routes;
mod spaceship_view;
mod tileset;
pub(crate) use tileset::Tileset;
mod territory;
mod ui;
mod ui_review;
mod unit_animation_pacing;
mod unit_actions;
mod world;
pub(crate) use render_backend::{RenderBackend, create as create_renderer};
pub(crate) use ui::set_review_pointer;
use ui::*;
pub(crate) use ui_review::{UI_REVIEW_SCENES, UiReviewSequence, UiReviewSources};

use crate::rules::{Building, Genus, Requirement, Tech};
use crate::sim::{topology::*, *};
use macroquad::prelude::*;
use research_tree::{NODE_SIZE, RESEARCH_AGES, ResearchTree};

const TILE_W: f32 = 96.0;
const TILE_H: f32 = 48.0;
const TOP_H: f32 = 46.0;
const HUD_MARGIN: f32 = 14.0;
const START_ZOOM: f32 = 0.82;
const MIN_ZOOM: f32 = 0.55;
const MAX_ZOOM: f32 = 1.35;

#[derive(Clone, Copy)]
pub(crate) struct TileGeometry {
    pub(crate) center: Vec2,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl TileGeometry {
    /// Creates diamond tile geometry around a screen-space center.
    pub(crate) fn new(center: Vec2, width: f32, height: f32) -> Self {
        Self {
            center,
            width,
            height,
        }
    }

    /// Returns the tile's width and height as a vector.
    pub(crate) fn size(self) -> Vec2 {
        vec2(self.width, self.height)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum UiAction {
    BlockMapClick,
    DismissIntro,
    ToggleMusicPanel,
    ToggleMusicMute,
    AdjustMusicVolume(i8),
    ActionPage(usize),
    EndTurn,
    LaunchSpaceship,
    ToggleSpaceship,
    ToggleManualAssembly,
    PlaceShipPart(ShipPart),
    RaiseRate(usize),
    Specialist(u32, usize, i32),
    CityTiles,
    WorkCityTile(u32, i32, i32),
    HomeCity,
    BoardOrDeboard,
    Disband(Option<u32>),
    BeginNuclearStrike,
    HelpWonder(u32),
    JoinCity(u32),
    Airlift(u32),
    AirSuperiority,
    BeginRebase,
    RebaseTo(i32, i32),
    BeginParadrop,
    ParadropTo(i32, i32),
    BeginBombard,
    BombardTo(i32, i32),
    NuclearStrikeTo(i32, i32),
    UpgradeUnit(UnitKind, i32),
    ToggleGovernment,
    ToggleDiplomacy,
    OpenDiplomacyWith(usize),
    OpenDiplomacyOverview,
    DiplomacyPage(usize),
    EditTreatyDraft(usize, TreatyOffer),
    TreatyItemsPage(usize, usize),
    TreatyItemGroup(usize, usize),
    AdjustTreatyGold(usize, i32),
    AddTreatyGold(usize, usize, TreatyOffer),
    ProposeTreaty(usize, TreatyOffer),
    BuyMap(usize, TreatyMap),
    CityDeals(Option<(usize, usize)>),
    BuyCity(u32, i32),
    CancelPact(usize),
    DeclineOffer(usize),
    ReviewOffer(usize, TreatyOffer),
    CloseOffer,
    AcceptOffer(usize, TreatyOffer),
    ChangeGovernment(Government),
    CloseCity,
    SelectUnit(u32),
    FoundCity,
    Road,
    Irrigate,
    TerrainWork(WorkKind),
    Mine,
    Fortify,
    Wait,
    Automate,
    Produce(Production),
    RushProduction(u32),
    SellBuilding(u32, Building),
    ProductionPage(usize),
    UnitProductionPage(usize),
    WorklistPage(Option<usize>),
    EditWorklist(u32, WorklistEdit),
    OpenResearch,
    CloseResearch,
    CenterResearch,
    FitResearch,
    ResearchAge(usize),
    InspectResearch(Tech),
    ResearchGoal(Tech),
}

pub(crate) struct Game {
    pub(crate) sim: Simulation,
    renderer: std::rc::Rc<dyn RenderBackend>,
    pub(crate) autosave_enabled: bool,
    pub(crate) intro_open: bool,
    pub(crate) music_open: bool,
    pub(crate) music_muted: bool,
    pub(crate) music_volume: u8,
    pub(crate) selected_unit: Option<u32>,
    selected_city: Option<u32>,
    pending_city_view: Option<u32>,
    action_page: usize,
    production_page: usize,
    unit_production_page: usize,
    worklist_page: Option<usize>,
    city_tiles: bool,
    camera: Vec2,
    zoom: f32,
    goto_mode: bool,
    paradrop_mode: bool,
    bombard_mode: bool,
    nuclear_mode: bool,
    rebase_mode: bool,
    research_open: bool,
    government_open: bool,
    spaceship_open: bool,
    diplomacy_open: bool,
    diplomacy_target: Option<usize>,
    reviewed_offer: Option<(usize, TreatyOffer)>,
    treaty_draft: Option<(usize, TreatyOffer)>,
    treaty_gold_amount: [i32; MAX_CIVILIZATIONS],
    treaty_item_page: [usize; MAX_CIVILIZATIONS],
    treaty_item_group: [usize; MAX_CIVILIZATIONS],
    treaty_feedback: Option<(usize, String)>,
    city_deals: Option<(usize, usize)>,
    diplomacy_page: usize,
    research_age: usize,
    research_inspected: Option<Tech>,
    research_tree: &'static ResearchTree,
    border_geometry: std::cell::RefCell<territory::BorderGeometryCache>,
    research_center: Vec2,
    research_zoom: f32,
    research_drag: Option<Vec2>,
    pub(crate) hud_elements_drawn: usize,
    playback: presentation::Playback,
    dragging: bool,
    drag_last: Vec2,
}

impl Game {
    /// Creates a new interactive game with the default renderer.
    pub(crate) fn new(map: crate::mapgen::GeneratedMap) -> Self {
        Self::new_with_renderer(map, render_backend::default_renderer())
    }

    /// Creates a new game using the selected rendering backend.
    pub(crate) fn new_with_renderer(
        map: crate::mapgen::GeneratedMap,
        renderer: std::rc::Rc<dyn RenderBackend>,
    ) -> Self {
        Self::from_simulation_with_renderer(Simulation::new(map), renderer)
    }

    /// Restores client state around a loaded simulation using the default renderer.
    pub(crate) fn from_simulation(sim: Simulation) -> Self {
        Self::from_simulation_with_renderer(sim, render_backend::default_renderer())
    }

    /// Restores client state around a loaded simulation and selected renderer.
    pub(crate) fn from_simulation_with_renderer(
        mut sim: Simulation,
        renderer: std::rc::Rc<dyn RenderBackend>,
    ) -> Self {
        sim.enable_visual_events();
        let focus = sim
            .units_owned_by(HUMAN)
            .find(|unit| unit.kind == UnitKind::Settler)
            .or_else(|| sim.units_owned_by(HUMAN).next());
        let selected_unit = focus.map(|unit| unit.id);
        let position = focus
            .map(|unit| (unit.x, unit.y))
            .or_else(|| {
                sim.cities_owned_by(HUMAN)
                    .next()
                    .map(|city| (city.x, city.y))
            })
            .unwrap_or((0, 0));
        let (x, y) = native_to_map(position.0, position.1);
        Self {
            sim,
            renderer,
            autosave_enabled: false,
            intro_open: false,
            music_open: false,
            music_muted: false,
            music_volume: 50,
            selected_unit,
            selected_city: None,
            pending_city_view: None,
            action_page: 0,
            production_page: 0,
            unit_production_page: 0,
            worklist_page: None,
            city_tiles: false,
            camera: vec2(x as f32, y as f32),
            zoom: START_ZOOM,
            goto_mode: false,
            paradrop_mode: false,
            bombard_mode: false,
            nuclear_mode: false,
            rebase_mode: false,
            research_open: false,
            government_open: false,
            spaceship_open: false,
            diplomacy_open: false,
            diplomacy_target: None,
            reviewed_offer: None,
            treaty_draft: None,
            treaty_gold_amount: [50; MAX_CIVILIZATIONS],
            treaty_item_page: [0; MAX_CIVILIZATIONS],
            treaty_item_group: [0; MAX_CIVILIZATIONS],
            treaty_feedback: None,
            city_deals: None,
            diplomacy_page: 0,
            research_age: 0,
            research_inspected: None,
            research_tree: research_tree::shared_tree(),
            border_geometry: Default::default(),
            research_center: Vec2::ZERO,
            research_zoom: 1.0,
            research_drag: None,
            hud_elements_drawn: 0,
            playback: Default::default(),
            dragging: false,
            drag_last: vec2(0.0, 0.0),
        }
    }

    fn found_city(&mut self, id: u32) -> bool {
        if self.sim.apply(Command::FoundCity(id)) {
            self.selected_unit = None;
            self.production_page = 0;
            self.unit_production_page = 0;
            self.worklist_page = None;
            self.city_tiles = false;
            self.selected_city = None;
            self.pending_city_view = self.sim.cities().last().map(|city| city.id);
            true
        } else {
            false
        }
    }

    fn move_selected(&mut self, command: Command, id: u32) -> bool {
        let previous = self
            .sim
            .units()
            .iter()
            .find(|u| u.id == id)
            .map(|u| (u.x, u.y));
        if !self.sim.apply(command) {
            return false;
        }
        if let Some((x, y)) = self
            .sim
            .units()
            .iter()
            .find(|u| u.id == id)
            .map(|u| (u.x, u.y))
        {
            if Some((x, y)) != previous {
                self.focus_camera_on(x, y);
            }
        } else if self.selected_unit == Some(id) {
            self.selected_unit = None;
        }
        true
    }

    fn selected_unit_index(&self) -> Option<usize> {
        let id = self.selected_unit?;
        self.sim.unit_index_by_id(id)
    }

    fn selected_city_index(&self) -> Option<usize> {
        let id = self.selected_city?;
        self.sim.city_index_by_id(id)
    }

    fn select_next_unit(&mut self) {
        let current = self.selected_unit.unwrap_or(0);
        let mut candidates: Vec<u32> = self
            .sim
            .units()
            .iter()
            .filter(|u| {
                u.owner == HUMAN
                    && u.moves_left > 0
                    && u.work_left == 0
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
        self.action_page = 0;
        self.selected_unit = next;
        self.selected_city = None;
        if let Some(id) = next
            && let Some((x, y)) = self
                .sim
                .units()
                .iter()
                .find(|u| u.id == id)
                .map(|unit| (unit.x, unit.y))
        {
            self.focus_camera_on(x, y);
        }
    }
}

impl Game {
    /// Draws the current frame and returns the interactive control hit boxes.
    pub(crate) fn draw(&mut self, assets: &Assets) -> Vec<(Rect, UiAction)> {
        self.normalize_camera();
        self.playback.update(&mut self.sim, get_time());
        if !self.playback.pending()
            && let Some(city) = self.pending_city_view.take()
        {
            self.selected_city = Some(city);
        }
        clear_background(Color::from_rgba(7, 20, 31, 255));
        self.hud_elements_drawn = 0;
        if self.research_open {
            return self.draw_research();
        }
        if self.spaceship_open {
            return self.draw_spaceship();
        }
        let renderer = std::rc::Rc::clone(&self.renderer);
        renderer.draw_world(self, assets);
        let mut actions = self.draw_hud(assets);
        if self.intro_open {
            actions.extend(self.draw_intro());
            return actions;
        }
        if self.government_open
            || self.diplomacy_open
        {
            draw_rectangle(
                0.0,
                0.0,
                screen_width(),
                screen_height(),
                Color::from_rgba(4, 9, 12, 178),
            );
        }
        if self.government_open {
            return self.draw_government();
        }
        if self.diplomacy_open {
            return self.draw_diplomacy(assets);
        }
        if self.selected_city_index().is_some() {
            actions.clear();
            self.draw_city_manager(assets, &mut actions);
        }
        if self.goto_mode || self.paradrop_mode || self.bombard_mode || self.nuclear_mode || self.rebase_mode {
            let viewport = Self::viewport_rect();
            let text = if self.paradrop_mode {
                "PARADROP: choose a landing tile"
            } else if self.bombard_mode {
                "BOMBARD: choose an enemy tile"
            } else if self.nuclear_mode {
                "NUCLEAR STRIKE: choose a target"
            } else if self.rebase_mode {
                "RE-BASE: choose a city, airfield, or carrier"
            } else {
                "GO-TO: choose a destination"
            };
            let width = measure_text(text, None, 22, 1.0).width;
            draw_rectangle(
                viewport.x + viewport.w * 0.5 - width * 0.5 - 14.0,
                viewport.y + 66.0,
                width + 28.0,
                34.0,
                Color::from_rgba(25, 23, 18, 230),
            );
            draw_rectangle_lines(
                viewport.x + viewport.w * 0.5 - width * 0.5 - 14.0,
                viewport.y + 66.0,
                width + 28.0,
                34.0,
                2.0,
                Color::from_rgba(237, 203, 89, 255),
            );
            draw_text(
                text,
                viewport.x + viewport.w * 0.5 - width * 0.5,
                viewport.y + 90.0,
                22.0,
                Color::from_rgba(255, 237, 170, 255),
            );
        }
        if let Some(message) = self.sim.game_over()
            && !self.playback.pending()
        {
            let w = 540.0_f32.min(screen_width() - 80.0);
            let standings = self.sim.score_standings();
            let rows = ((screen_height() - 165.0) / 24.0).max(1.0) as usize;
            let h = 129.0 + 24.0 * standings.len().min(rows) as f32;
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
            for (index, (owner, score)) in standings.into_iter().take(rows).enumerate() {
                centered_text(
                    &format!("{}  ·  {score} points", self.sim.players()[owner].name),
                    y + 117.0 + index as f32 * 24.0,
                    17.0,
                    Color::from_rgba(176, 180, 169, 255),
                );
            }
        }
        actions
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
    let mouse = ui_pointer();
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
    while font_size > 13.0 && measure_text(label, None, font_size as u16, 1.0).width > rect.w - 10.0
    {
        font_size -= 1.0;
    }
    let fitted = fit_text(
        label,
        font_size,
        rect.w - if rect.w < 40.0 { 6.0 } else { 16.0 },
    );
    let label = fitted.as_str();
    let size = measure_text(label, None, font_size as u16, 1.0);
    draw_text(
        label,
        rect.x + (rect.w - size.width) * 0.5,
        // draw_text uses a baseline; center the measured glyph bounds, not an
        // estimate based on font size (which places symbols such as + too low).
        rect.y + (rect.h - size.height) * 0.5 + size.offset_y,
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
    kind.name()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clicking_a_garrisoned_city_opens_city_management() {
        let mut game = Game::new(crate::mapgen::generate(42));
        let settler = game
            .sim
            .units()
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
            .map(|unit| unit.id)
            .expect("human settler");
        assert!(game.found_city(settler));
        let city = game
            .sim
            .cities()
            .iter()
            .find(|city| city.owner == HUMAN)
            .cloned()
            .expect("human city");
        let garrison = game
            .sim
            .units()
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
}
