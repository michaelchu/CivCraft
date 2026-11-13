//! Presentation layer between the simulation and the Macroquad window.
//!
//! [`Game`] owns transient client state such as selection, camera position, and
//! open panels. It reads display projections and submits commands through
//! [`crate::sim::ClientSimulation`], draws map and interface layers, and
//! translates input into [`UiAction`] values. The simulation remains the
//! authority for campaign state and gameplay rules.
//!
//! Rendering is split by screen responsibility: `world` draws map contents,
//! `hud` and the view modules draw controls and dialogs, and `assets` plus
//! `tileset` provide Civ III artwork. Civ III projection helpers and the world
//! painter own map rendering. UI review helpers exercise these same screens
//! through the normal frame path.

mod actions;
mod amb;
mod assets;
mod audio;
mod sprite_metadata;
pub(crate) use assets::{Assets, window_conf};
pub(crate) use audio::MusicPlayer;
use sprite_metadata::{player_color, unit_art_role};
pub(crate) use unit_sound::UnitSoundPlayer;

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
mod civ3_projection;
mod flc;
mod fog;
mod government_view;
mod hud;
mod input;
mod map_layers;
mod map_overlays;
mod map_view;
mod presentation;
mod research_tree;
mod research_view;
mod rivers;
mod routes;
mod spaceship_view;
mod tileset;
pub(crate) use tileset::Tileset;
mod ui;
mod ui_review;
mod unit_actions;
mod unit_animation_pacing;
mod unit_sound;
mod world;
pub(crate) use ui::set_review_pointer;
use ui::*;
pub(crate) use ui_review::{UI_REVIEW_SCENES, UiReviewSequence, UiReviewSources};

use crate::rules::{Building, Requirement, Tech};
use crate::sim::{
    Base, CityManagerView, ClientSimulation, Command, GotoPoint, GotoPreview, Government, HUMAN,
    MAX_CIVILIZATIONS, Pact, PresentationEventView, Production, RenderedCityView, RenderedUnitView,
    Resource, ShipPart, Terrain, TreatyMap, TreatyOffer, UnitKind, UnitState, WORKLIST_LIMIT,
    WorkKind, WorklistEdit, year_label,
};
#[cfg(test)]
use crate::sim::{CombatVisual, VisualEvent};
use crate::world::topology::*;
use macroquad::prelude::*;
use research_tree::{NODE_SIZE, RESEARCH_AGES, ResearchTree};

const TILE_W: f32 = 96.0;
const TILE_H: f32 = 48.0;
const TOP_H: f32 = 46.0;
const HUD_MARGIN: f32 = 14.0;
const WORLD_ART_SCALE: f32 = 0.75;
const ZOOMED_OUT: f32 = 1.0;
const ZOOMED_IN: f32 = 1.25;
const START_ZOOM: f32 = ZOOMED_IN;
#[cfg(test)]
const MAX_ZOOM: f32 = 1.25;

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
    ToggleSfxMute,
    AdjustSfxVolume(i8),
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
    BeginParadrop,
    ParadropTo(i32, i32),
    BeginCaptureUnits,
    CaptureUnitsTo(i32, i32),
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
    Mobilize,
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
    pub(crate) sim: ClientSimulation,
    client_messages: Vec<String>,
    last_sim_messages: Vec<String>,
    pub(crate) autosave_enabled: bool,
    pub(crate) intro_open: bool,
    pub(crate) music_open: bool,
    pub(crate) music_muted: bool,
    pub(crate) music_volume: u8,
    pub(crate) sfx_muted: bool,
    pub(crate) sfx_volume: u8,
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
    capture_units_mode: bool,
    bombard_mode: bool,
    nuclear_mode: bool,
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
    research_center: Vec2,
    research_zoom: f32,
    research_drag: Option<Vec2>,
    pub(crate) hud_elements_drawn: usize,
    playback: presentation::Playback,
    unit_sound_requests: std::collections::VecDeque<unit_sound::UnitSoundRequest>,
    pending_sound_unit: Option<u32>,
    fidget_sound_cycle: Option<(u32, i64)>,
    dragging: bool,
    drag_last: Vec2,
}

impl Game {
    /// Creates client state around a simulation.
    pub(crate) fn new(sim: crate::sim::Simulation) -> Self {
        Self::from_simulation(sim)
    }

    /// Restores client state around a loaded simulation.
    pub(crate) fn from_simulation(sim: crate::sim::Simulation) -> Self {
        Self::from_client_simulation(ClientSimulation::new(sim))
    }

    pub(super) fn from_client_simulation(sim: ClientSimulation) -> Self {
        let (selected_unit, position) = sim.initial_client_focus_view();
        let position = position.unwrap_or((0, 0));
        let (x, y) = native_to_map(position.0, position.1);
        let last_sim_messages = sim.campaign_messages_view();
        Self {
            sim,
            client_messages: Vec::new(),
            last_sim_messages,
            autosave_enabled: false,
            intro_open: false,
            music_open: false,
            music_muted: false,
            music_volume: 50,
            sfx_muted: false,
            sfx_volume: 50,
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
            capture_units_mode: false,
            bombard_mode: false,
            nuclear_mode: false,
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
            research_center: Vec2::ZERO,
            research_zoom: 1.0,
            research_drag: None,
            hud_elements_drawn: 0,
            playback: Default::default(),
            unit_sound_requests: Default::default(),
            pending_sound_unit: None,
            fidget_sound_cycle: None,
            dragging: false,
            drag_last: vec2(0.0, 0.0),
        }
    }

    pub(crate) fn world_seed(&self) -> u64 {
        self.sim.world_seed()
    }

    pub(crate) fn map_wrap(&self) -> MapWrap {
        self.sim.map_wrap()
    }

    pub(crate) fn ui_review_sources(&self) -> UiReviewSources {
        UiReviewSources::new(self.sim.review_simulation())
    }

    pub(super) fn notify(&mut self, message: impl Into<String>) {
        self.client_messages.push(message.into());
        if self.client_messages.len() > 10 {
            self.client_messages.remove(0);
        }
    }

    fn found_city(&mut self, id: u32) -> bool {
        if self.sim.apply(Command::FoundCity(id)) {
            if self.selected_unit == Some(id) {
                self.pending_sound_unit = Some(id);
            }
            self.selected_unit = None;
            self.production_page = 0;
            self.unit_production_page = 0;
            self.worklist_page = None;
            self.city_tiles = false;
            self.selected_city = None;
            self.pending_city_view = self.sim.newest_city_id_view();
            true
        } else {
            false
        }
    }

    fn move_selected(&mut self, command: Command, id: u32) -> bool {
        let previous = self.sim.unit_position_view(id);
        if !self.sim.apply(command) {
            return false;
        }
        if let Some((x, y)) = self.sim.unit_position_view(id) {
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
        self.sim.unit_action_view(id).map(|_| 0)
    }

    fn selected_city_index(&self) -> Option<usize> {
        let id = self.selected_city?;
        self.sim.city_context_view(id).map(|_| 0)
    }

    fn select_next_unit(&mut self) {
        let current = self.selected_unit.unwrap_or(0);
        let mut candidates: Vec<u32> = self
            .sim
            .selectable_units_view()
            .into_iter()
            .map(|unit| unit.id)
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
            && let Some((x, y)) = self.sim.unit_position_view(id)
        {
            self.focus_camera_on(x, y);
        }
    }
}

impl Game {
    /// Draws the current frame and returns the interactive control hit boxes.
    pub(crate) fn draw(&mut self, assets: &Assets) -> Vec<(Rect, UiAction)> {
        self.normalize_camera();
        let sound_unit = self.pending_sound_unit.take().or(self.selected_unit);
        let events = self.sim.take_presentation_event_views();
        self.playback.update(events, get_time(), sound_unit);
        self.unit_sound_requests
            .extend(self.playback.take_sound_requests());
        if !self.research_open && !self.spaceship_open && !self.intro_open {
            self.queue_fidget_sound(assets);
        } else {
            self.fidget_sound_cycle = None;
        }
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
        self.draw_civ3_world_map(assets);
        let mut actions = self.draw_hud(assets);
        if self.intro_open {
            actions.clear();
            actions.extend(self.draw_intro());
            return actions;
        }
        if self.government_open || self.diplomacy_open {
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
            return self.draw_diplomacy();
        }
        if self.selected_city_index().is_some() {
            actions.clear();
            self.draw_city_manager(assets, &mut actions);
        }
        if self.goto_mode
            || self.paradrop_mode
            || self.capture_units_mode
            || self.bombard_mode
            || self.nuclear_mode
        {
            let viewport = Self::viewport_rect();
            let text = if self.paradrop_mode {
                "PARADROP: choose a landing tile"
            } else if self.capture_units_mode {
                "CAPTURE UNITS: choose an adjacent enemy stack"
            } else if self.bombard_mode {
                "BOMBARD: choose an enemy tile"
            } else if self.nuclear_mode {
                "NUCLEAR STRIKE: choose a target"
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
        if let Some(game_over) = self.sim.game_over_view()
            && !self.playback.pending()
        {
            let w = 540.0_f32.min(screen_width() - 80.0);
            let rows = ((screen_height() - 165.0) / 24.0).max(1.0) as usize;
            let h = 129.0 + 24.0 * game_over.standings.len().min(rows) as f32;
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
                &game_over.message,
                y + 88.0,
                21.0,
                Color::from_rgba(238, 234, 218, 255),
            );
            for (index, standing) in game_over.standings.iter().take(rows).enumerate() {
                centered_text(
                    &format!("{}  ·  {} points", standing.name, standing.score),
                    y + 117.0 + index as f32 * 24.0,
                    17.0,
                    Color::from_rgba(176, 180, 169, 255),
                );
            }
        }
        actions
    }

    fn queue_fidget_sound(&mut self, assets: &Assets) {
        let Some(id) = self.selected_unit else {
            self.fidget_sound_cycle = None;
            return;
        };
        let Some(unit) = self.sim.unit_fidget_view(id) else {
            self.fidget_sound_cycle = None;
            return;
        };
        if self.playback.is_active(id)
            || unit.working
            || !matches!(unit.state, UnitState::Idle | UnitState::Sentry)
        {
            self.fidget_sound_cycle = None;
            return;
        }
        let pacing = assets.civ3.fidget_pacing(unit.kind);
        let period = pacing.play_seconds + pacing.rest_seconds;
        if period <= 0.0 {
            return;
        }
        let now = get_time() as f32;
        let cycle = (now / period).floor() as i64;
        let phase = now.rem_euclid(period);
        let cycle_key = (unit.id, cycle);
        if phase < pacing.play_seconds && self.fidget_sound_cycle != Some(cycle_key) {
            self.unit_sound_requests
                .push_back(unit_sound::UnitSoundRequest {
                    unit_id: unit.id,
                    kind: unit.kind,
                    era_index: unit.era_index,
                    action: "FIDGET".to_owned(),
                    delay_seconds: 0.0,
                });
            self.fidget_sound_cycle = Some(cycle_key);
        }
    }

    pub(crate) fn take_unit_sound_requests(&mut self) -> Vec<unit_sound::UnitSoundRequest> {
        self.unit_sound_requests.drain(..).collect()
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
    fn client_notices_are_transient_and_capped_without_mutating_campaign_events() {
        let mut game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
        let campaign_events = game.sim.review_simulation().events().to_vec();
        for index in 0..11 {
            game.notify(format!("notice {index}"));
        }
        assert_eq!(game.client_messages.len(), 10);
        assert_eq!(
            game.client_messages.first().map(String::as_str),
            Some("notice 1")
        );
        assert_eq!(game.sim.review_simulation().events(), campaign_events);
    }

    #[test]
    fn clicking_a_garrisoned_city_opens_city_management() {
        let mut game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
        let settler = game
            .sim
            .review_simulation()
            .units()
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
            .map(|unit| unit.id)
            .expect("human settler");
        assert!(game.found_city(settler));
        let city = game
            .sim
            .review_simulation()
            .cities()
            .iter()
            .find(|city| city.owner == HUMAN)
            .cloned()
            .expect("human city");
        let garrison = game
            .sim
            .review_simulation()
            .units()
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.x == city.x && unit.y == city.y)
            .map(|unit| unit.id)
            .expect("garrisoned unit");

        game.selected_city = None;
        game.selected_unit = Some(garrison);
        assert!(
            game.sim
                .review_simulation()
                .units()
                .iter()
                .find(|unit| unit.id == garrison)
                .unwrap()
                .moves_left
                > 0
        );
        game.select_at(city.x, city.y);

        assert_eq!(game.selected_city, Some(city.id));
        assert_eq!(game.selected_unit, None);
        let waited = game
            .sim
            .review_simulation()
            .units()
            .iter()
            .find(|unit| unit.id == garrison)
            .unwrap();
        assert_eq!(waited.moves_left, 0);
        assert_eq!(waited.state, UnitState::Idle);
    }

    #[test]
    fn clicking_an_empty_map_tile_waits_and_deselects_the_active_unit() {
        let mut game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
        let unit_id = game
            .sim
            .review_simulation()
            .units()
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.moves_left > 0)
            .map(|unit| unit.id)
            .expect("human unit with movement remaining");
        game.selected_unit = Some(unit_id);

        let wrap = game.map_wrap();
        let empty = (0..wrap.height())
            .flat_map(|y| (0..wrap.width()).map(move |x| (x, y)))
            .find(|&(x, y)| {
                game.sim.human_city_at_view(x, y).is_none()
                    && game.sim.human_units_at_view(x, y).is_empty()
            })
            .expect("empty map tile");

        game.select_at(empty.0, empty.1);

        assert_eq!(game.selected_unit, None);
        let waited = game
            .sim
            .review_simulation()
            .units()
            .iter()
            .find(|unit| unit.id == unit_id)
            .unwrap();
        assert_eq!(waited.moves_left, 0);
        assert_eq!(waited.state, UnitState::Idle);
    }
}
