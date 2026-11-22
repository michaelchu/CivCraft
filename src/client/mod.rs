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
mod autosave;
mod turn_processing;
pub(crate) use autosave::AutosaveWriter;
#[cfg(feature = "desktop-audio")]
mod amb;
mod asset_error;
mod assets;
#[cfg(feature = "desktop-audio")]
mod audio;
#[cfg(not(feature = "desktop-audio"))]
#[path = "audio_stub.rs"]
mod audio;
#[cfg(feature = "desktop-audio")]
mod audio_assets;
#[cfg(feature = "desktop-audio")]
mod audio_codecs;
mod media;
mod sprite_metadata;
mod unit_media;
pub(crate) use assets::{AssetContext, Assets, window_conf};
pub(crate) use audio::MusicPlayer;
use sprite_metadata::{minimap_color, player_color};
pub(crate) use unit_sound::UnitSoundPlayer;

mod city_bar;
mod city_manager;
mod city_map;
#[allow(dead_code)] // Standalone projection helpers are retained for geometry regression fixtures.
mod diamond_geometry;
mod diamond_projection;
mod espionage_view;
mod flc;
mod fog;
mod font;
mod government_view;
mod hud;
mod input;
mod map_layers;
mod map_overlays;
mod map_view;
mod modes;
mod presentation;
mod research_tree;
mod research_view;
mod rivers;
mod routes;
mod spaceship_view;
mod stack_menu;
mod tileset;
pub(crate) use tileset::Tileset;
mod ui;
mod ui_review;
mod unit_actions;
mod unit_animation_pacing;
#[cfg(feature = "desktop-audio")]
mod unit_sound;
#[cfg(not(feature = "desktop-audio"))]
#[path = "unit_sound_stub.rs"]
mod unit_sound;
mod world;
use modes::{Overlay, TargetingMode};
pub(crate) use ui::set_review_pointer;
use ui::*;
pub(crate) use ui_review::{UI_REVIEW_SCENES, UiReviewSequence, UiReviewSources};

use crate::rules::{Building, Requirement, Tech};
use crate::sim::{
    Base, CityManagerView, ClientSimulation, Command, EspionageMission, EspionageRisk, GotoPoint,
    GotoPreview, Government, HUMAN, MAX_CIVILIZATIONS, Pact, PresentationEventView, Production,
    RenderedCityView, RenderedUnitView, Resource, Terrain, TreatyMap, TreatyOffer, UnitKind,
    UnitState, WORKLIST_LIMIT, WorkKind, WorklistEdit, year_label,
};
#[cfg(test)]
use crate::sim::{CombatVisual, VisualEvent};
use crate::world::topology::*;
use font::draw_text;
use macroquad::prelude::*;
use research_tree::{NODE_SIZE, RESEARCH_AGES, ResearchTree};

const TILE_W: f32 = 96.0;
const TILE_H: f32 = 48.0;
/// Height of the floating button row; the map itself fills the whole window.
const CONTROLS_H: f32 = HUD_MARGIN + 33.0 + 5.0;
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

#[derive(Clone)]
pub(crate) enum UiAction {
    BlockMapClick,
    DismissIntro,
    ToggleMusicPanel,
    ToggleMusicMute,
    SetMusicVolume(u8),
    ToggleSfxMute,
    SetSfxVolume(u8),
    ActionPage(usize),
    EndTurn,
    ToggleSpaceship,
    /// Sets the science, tax and luxury split from a rate slider.
    SetRates([i32; 3]),
    Specialist(u32, usize, i32),
    CityGovernorPage,
    SetCityGovernor(u32, crate::sim::CityGovernor),
    WorkCityTile(u32, i32, i32),
    HomeCity,
    BoardOrDeboard,
    UnloadAll,
    Disband(Option<u32>),
    BeginNuclearStrike,
    BeginConnect,
    ConnectTo(i32, i32),
    BeginPatrol,
    PatrolTo(i32, i32),
    CompleteProduction(u32),
    FormArmy,
    JoinCity(u32),
    Airlift(u32),
    BeginParadrop,
    ParadropTo(i32, i32),
    BeginCaptureUnits,
    CaptureUnitsTo(i32, i32),
    BeginBombard,
    BeginAutoBombard,
    BeginPrecisionBombard,
    BeginAutoPrecisionBombard,
    AirSuperiority,
    BeginRebase,
    RebaseTo(i32, i32),
    BeginRecon,
    ReconTo(i32, i32),
    CancelUnitOrder,
    BombardTo(i32, i32),
    NuclearStrikeTo(i32, i32),
    UpgradeUnit(UnitKind, i32),
    ToggleGovernment,
    ToggleReligion,
    ReligionCity(usize),
    ReligionFoundingChoice,
    ReligionFaith,
    ReligionPolicy(crate::sim::FaithPolicy),
    FoundReligion,
    ConfirmReligion,

    ToggleDiplomacy,
    ToggleEspionage,
    EspionageOpponent(usize),
    EspionageRivalPage(usize),
    EspionageCity(u32),
    EspionageCityPage(usize),
    EspionageMilitaryPage(usize),
    EspionageRisk(EspionageRisk),
    PerformEspionage(usize, EspionageMission, Option<u32>, EspionageRisk),
    OpenDiplomacyWith(usize),
    OpenDiplomacyOverview,
    DiplomacyPage(usize),
    EditTreatyDraft(usize, TreatyOffer),
    TreatyItemsPage(usize, usize),
    TreatyClausesPage(usize, usize),
    TreatyItemGroup(usize, usize),
    AdjustTreatyGold(usize, i32),
    AddTreatyGold(usize, usize, TreatyOffer),
    ProposeTreaty(usize, TreatyOffer),
    CancelPact(usize),
    ReleaseVassal(usize),
    CancelSharedVision(usize),
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
    CityBuildingsPage(usize),
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
    SelectUnitStack(Vec<u32>),
    SelectSameUnitType(Vec<u32>),
    ShowCityInfo(u32),
}

/// Preserve ascending-ID cycling without allocating and sorting candidate lists.
fn next_unit_id(
    ids: impl Iterator<Item = u32>,
    current: u32,
    waiting: &std::collections::BTreeSet<u32>,
) -> (Option<u32>, bool) {
    let mut first = None;
    let mut after = None;
    let mut available_first = None;
    let mut available_after = None;
    for id in ids {
        first = Some(first.map_or(id, |old: u32| old.min(id)));
        if id > current {
            after = Some(after.map_or(id, |old: u32| old.min(id)));
        }
        if !waiting.contains(&id) {
            available_first = Some(available_first.map_or(id, |old: u32| old.min(id)));
            if id > current {
                available_after = Some(available_after.map_or(id, |old: u32| old.min(id)));
            }
        }
    }
    let available = available_after.or(available_first);
    (available.or(after).or(first), available.is_none())
}

pub(crate) struct Game {
    pub(crate) sim: ClientSimulation,
    background_turn: Option<crate::background::Task<crate::sim::CompletedTurn>>,
    preparing_turn: Option<turn_processing::TurnPreparation>,
    prepared_events: std::collections::VecDeque<PresentationEventView>,
    pending_autosave: Option<Vec<u8>>,
    pending_notices: Vec<(String, bool)>,
    minimap_cache: std::cell::RefCell<hud::MinimapCache>,
    religion_city: usize,
    religion_choice: usize,
    religion_draft: Option<(Option<u32>, crate::sim::FaithPolicy)>,
    religion_feedback: String,
    last_notification_id: u64,
    active_toast: Option<(String, f64)>,
    pub(crate) autosave_enabled: bool,
    pub(crate) music_open: bool,
    pub(crate) music_muted: bool,
    pub(crate) music_volume: u8,
    pub(crate) sfx_muted: bool,
    pub(crate) sfx_volume: u8,
    pub(crate) selected_unit: Option<u32>,
    pub(crate) selected_units: Vec<u32>,
    selected_city: Option<u32>,
    waiting_units: std::collections::BTreeSet<u32>,
    pending_city_view: Option<u32>,
    action_page: usize,
    city_buildings_page: usize,
    production_page: usize,
    unit_production_page: usize,
    worklist_page: Option<usize>,
    city_governor_page: bool,
    city_governor_preview: Option<[i32; 6]>,
    camera: Vec2,
    zoom: f32,
    targeting: TargetingMode,
    overlay: Overlay,
    espionage_target: Option<usize>,
    espionage_rival_page: usize,
    espionage_city: Option<u32>,
    espionage_city_page: usize,
    espionage_military_page: usize,
    espionage_risk: EspionageRisk,
    diplomacy_target: Option<usize>,
    reviewed_offer: Option<(usize, TreatyOffer)>,
    treaty_draft: Option<(usize, TreatyOffer)>,
    treaty_gold_amount: [i32; MAX_CIVILIZATIONS],
    treaty_item_page: [usize; MAX_CIVILIZATIONS],
    treaty_clause_page: [usize; 2],
    treaty_item_group: [usize; MAX_CIVILIZATIONS],
    treaty_feedback: Option<(usize, String)>,
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
    fortify_animation_started_at: std::collections::HashMap<u32, f64>,
    dragging: bool,
    drag_last: Vec2,
    left_map_press: Option<LeftMapPress>,
    goto_drag_unit: Option<u32>,
    stack_menu: Option<StackMenu>,
}

struct LeftMapPress {
    screen: Vec2,
    tile: (i32, i32),
    unit_id: Option<u32>,
}

struct StackMenu {
    position: Vec2,
    units: Vec<u32>,
    city_id: Option<u32>,
    scroll_offset: usize,
}

impl Game {
    /// Creates client state around a simulation.
    #[cfg(test)]
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
        let last_notification_id = sim
            .campaign_notifications_view()
            .iter()
            .map(|item| item.id)
            .max()
            .unwrap_or(0);
        Self {
            sim,
            background_turn: None,
            preparing_turn: None,
            prepared_events: Default::default(),
            pending_autosave: None,
            pending_notices: Vec::new(),
            minimap_cache: Default::default(),
            religion_city: 0,
            religion_choice: 0,
            religion_draft: None,
            religion_feedback: String::new(),
            last_notification_id,
            active_toast: None,
            autosave_enabled: false,
            music_open: false,
            music_muted: false,
            music_volume: 50,
            sfx_muted: false,
            sfx_volume: 50,
            selected_unit,
            selected_units: selected_unit.into_iter().collect(),
            selected_city: None,
            waiting_units: Default::default(),
            pending_city_view: None,
            action_page: 0,
            city_buildings_page: 0,
            production_page: 0,
            unit_production_page: 0,
            worklist_page: None,
            city_governor_page: false,
            city_governor_preview: None,
            camera: vec2(x as f32, y as f32),
            zoom: START_ZOOM,
            targeting: TargetingMode::None,
            overlay: Overlay::None,
            espionage_target: None,
            espionage_rival_page: 0,
            espionage_city: None,
            espionage_city_page: 0,
            espionage_military_page: 0,
            espionage_risk: EspionageRisk::Immediate,
            diplomacy_target: None,
            reviewed_offer: None,
            treaty_draft: None,
            treaty_gold_amount: [50; MAX_CIVILIZATIONS],
            treaty_item_page: [0; MAX_CIVILIZATIONS],
            treaty_clause_page: [0; 2],
            treaty_item_group: [0; MAX_CIVILIZATIONS],
            treaty_feedback: None,
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
            fortify_animation_started_at: Default::default(),
            dragging: false,
            drag_last: vec2(0.0, 0.0),
            left_map_press: None,
            goto_drag_unit: None,
            stack_menu: None,
        }
    }

    pub(crate) fn world_seed(&self) -> u64 {
        self.sim.world_seed()
    }

    pub(crate) fn turn_for_smoke_test(&self) -> i32 {
        self.sim.turn_view()
    }

    pub(crate) fn map_wrap(&self) -> MapWrap {
        self.sim.map_wrap()
    }

    pub(crate) fn ui_review_sources(&self) -> UiReviewSources {
        UiReviewSources::new(&self.sim)
    }

    pub(super) fn notify(&mut self, message: impl Into<String>) {
        if self.turn_busy() {
            self.pending_notices.push((message.into(), false));
        } else {
            self.sim.notify(message);
        }
    }

    pub(super) fn notify_important(&mut self, message: impl Into<String>) {
        if self.turn_busy() {
            self.pending_notices.push((message.into(), true));
        } else {
            self.sim.notify_important(message);
        }
    }

    fn found_city(&mut self, id: u32) -> bool {
        if self.sim.apply(Command::FoundCity(id)) {
            if self.selected_unit == Some(id) {
                self.pending_sound_unit = Some(id);
            }
            self.set_unit_selection(Vec::new(), None);
            self.city_buildings_page = 0;
            self.production_page = 0;
            self.unit_production_page = 0;
            self.worklist_page = None;
            self.city_governor_page = false;
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
            let active = self.selected_unit;
            let remaining = self
                .selected_units
                .iter()
                .copied()
                .filter(|selected| *selected != id)
                .collect();
            self.set_unit_selection(remaining, active.filter(|selected| *selected != id));
        }
        true
    }

    pub(super) fn set_unit_selection(&mut self, ids: Vec<u32>, active: Option<u32>) {
        let mut unique = Vec::new();
        for id in ids {
            if !unique.contains(&id) && self.sim.is_human_unit(id) {
                unique.push(id);
            }
        }
        self.selected_unit = active
            .filter(|id| unique.contains(id))
            .or_else(|| unique.first().copied());
        self.selected_units = unique;
    }

    pub(super) fn select_unit_group(&mut self, ids: Vec<u32>, active: Option<u32>) {
        self.set_unit_selection(ids, active);
        for id in &self.selected_units {
            self.waiting_units.remove(id);
        }
        self.action_page = 0;
        self.selected_city = None;
        self.stack_menu = None;
    }

    pub(super) fn show_city_info(&mut self, city_id: u32) {
        if let Some(unit_id) = self.selected_unit.take() {
            self.waiting_units.insert(unit_id);
        }
        self.city_buildings_page = 0;
        self.production_page = 0;
        self.unit_production_page = 0;
        self.worklist_page = None;
        self.city_governor_page = false;
        self.action_page = 0;
        self.selected_city = Some(city_id);
        self.set_unit_selection(Vec::new(), None);
        self.stack_menu = None;
    }

    fn sync_unit_selection(&mut self) {
        self.selected_units.retain(|id| self.sim.is_human_unit(*id));
        if let Some(active) = self.selected_unit {
            if !self.selected_units.contains(&active) {
                if self.sim.is_human_unit(active) {
                    self.selected_units = vec![active];
                } else {
                    self.selected_unit = self.selected_units.first().copied();
                }
            }
        } else {
            self.selected_unit = self.selected_units.first().copied();
        }
    }

    fn selected_unit_index(&self) -> Option<usize> {
        let id = self.selected_unit?;
        self.sim.is_human_unit(id).then_some(0)
    }

    fn selected_city_index(&self) -> Option<usize> {
        let id = self.selected_city?;
        self.sim.city_exists(id).then_some(0)
    }

    fn select_next_unit(&mut self) {
        let current = self.selected_unit.unwrap_or(0);
        let (next, reset_waiting) = next_unit_id(
            self.sim.selectable_unit_ids_view(),
            current,
            &self.waiting_units,
        );
        if reset_waiting {
            self.waiting_units.clear();
        }
        self.action_page = 0;
        self.set_unit_selection(next.into_iter().collect(), next);
        self.selected_city = None;
        if let Some(id) = next
            && let Some((x, y)) = self.sim.unit_position_view(id)
        {
            self.focus_camera_on(x, y);
        }
    }

    fn wait_selected_unit(&mut self) {
        if let Some(id) = self.selected_unit {
            self.waiting_units.insert(id);
            self.select_next_unit();
        }
    }
}

impl Game {
    /// Draws the current frame and returns the interactive control hit boxes.
    pub(crate) fn draw(&mut self, assets: &Assets) -> Vec<(Rect, UiAction)> {
        self.sync_unit_selection();
        self.normalize_camera();
        let sound_unit = if self.turn_busy() {
            self.selected_unit
        } else {
            self.pending_sound_unit.take().or(self.selected_unit)
        };
        let mut events = std::mem::take(&mut self.prepared_events);
        if !self.turn_busy() {
            let mut incoming = self.sim.take_presentation_event_views();
            presentation::configure_combat_timing(&mut incoming, &assets.art_tileset);
            events.extend(incoming);
        }
        let now = get_time();
        for event in &events {
            if let PresentationEventView::Fortify { unit, .. } = event {
                self.fortify_animation_started_at.insert(unit.id, now);
            }
        }
        self.playback.update(events, now, sound_unit);
        self.unit_sound_requests
            .extend(self.playback.take_sound_requests());
        if !self.turn_busy() && !self.overlay.hides_map() && self.overlay != Overlay::Intro {
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
        match self.overlay {
            Overlay::Research => return self.draw_research(),
            Overlay::Spaceship => return self.draw_spaceship(),
            _ => {}
        }
        self.draw_world_map(assets);
        let mut actions = self.draw_hud(assets);
        if self.overlay == Overlay::Intro {
            actions.clear();
            actions.extend(self.draw_intro());
            return actions;
        }
        if self.overlay.is_council() {
            draw_rectangle(
                0.0,
                0.0,
                screen_width(),
                screen_height(),
                Color::from_rgba(4, 9, 12, 178),
            );
        }
        match self.overlay {
            Overlay::Religion => return self.draw_religion(),
            Overlay::Government => return self.draw_government(),
            Overlay::Diplomacy => return self.draw_diplomacy(),
            Overlay::Espionage => return self.draw_espionage(),
            _ => {}
        }
        if self.selected_city_index().is_some() {
            actions.clear();
            self.draw_city_manager(assets, &mut actions);
        }
        actions.extend(self.draw_stack_menu());
        if let Some(text) = self.targeting.banner() {
            let viewport = Self::viewport_rect();
            let width = measure_cached(text, 22).width;
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
        let pacing = assets.art_tileset.fidget_pacing(unit.kind);
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
                    #[cfg(test)]
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
    while font_size > 13.0 && measure_cached(label, font_size as u16).width > rect.w - 10.0 {
        font_size -= 1.0;
    }
    let fitted = fit_text(
        label,
        font_size,
        rect.w - if rect.w < 40.0 { 6.0 } else { 16.0 },
    );
    let label = fitted.as_str();
    let size = measure_cached(label, font_size as u16);
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
    let width = measure_cached(text, size as u16).width;
    draw_text(text, (screen_width() - width) * 0.5, y, size, color);
}

fn short_unit_name(kind: UnitKind) -> &'static str {
    kind.name()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_unit_selection_preserves_id_order_waiting_and_wraparound() {
        for ids in [vec![], vec![7], vec![7, 2, 10, 4], vec![10, 7, 4, 2]] {
            for mask in 0..(1 << ids.len()) {
                let waiting: std::collections::BTreeSet<_> = ids
                    .iter()
                    .copied()
                    .enumerate()
                    .filter_map(|(index, id)| (mask & (1 << index) != 0).then_some(id))
                    .collect();
                for current in [0, 2, 3, 4, 7, 10, 99] {
                    let mut ordered = ids.clone();
                    ordered.sort_unstable();
                    let available: Vec<_> = ordered
                        .iter()
                        .copied()
                        .filter(|id| !waiting.contains(id))
                        .collect();
                    let reset_waiting = available.is_empty();
                    let candidates = if reset_waiting { &ordered } else { &available };
                    let expected = candidates
                        .iter()
                        .copied()
                        .find(|id| *id > current)
                        .or_else(|| candidates.first().copied());
                    assert_eq!(
                        next_unit_id(ids.iter().copied(), current, &waiting),
                        (expected, reset_waiting)
                    );
                }
            }
        }
    }

    #[test]
    fn client_notices_are_campaign_notifications() {
        let mut game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
        let before = game.sim.campaign_notifications_view().len();
        for index in 0..11 {
            game.notify(format!("notice {index}"));
        }
        let notifications = game.sim.campaign_notifications_view();
        assert_eq!(notifications.len(), before + 11);
        assert_eq!(
            notifications.last().map(|item| item.text.as_str()),
            Some("notice 10")
        );
    }

    #[test]
    fn clicking_a_garrisoned_city_selects_its_unit() {
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

        assert_eq!(game.selected_city, None);
        assert_eq!(game.selected_unit, Some(garrison));
        let waited = game
            .sim
            .review_simulation()
            .units()
            .iter()
            .find(|unit| unit.id == garrison)
            .unwrap();
        assert!(waited.moves_left > 0);
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
        assert!(waited.moves_left > 0);
        assert_eq!(waited.state, UnitState::Idle);
        assert!(game.waiting_units.contains(&unit_id));
    }

    #[test]
    fn waiting_defers_focus_until_other_active_units_are_handled() {
        let mut game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
        let mut units: Vec<_> = game.sim.selectable_unit_ids_view().collect();
        units.sort_unstable();
        assert!(units.len() >= 2);
        game.selected_unit = Some(units[0]);

        game.wait_selected_unit();
        assert_eq!(game.selected_unit, Some(units[1]));
        assert!(game.waiting_units.contains(&units[0]));
        assert!(
            game.sim
                .unit_moves_left_view(units[0])
                .is_some_and(|moves_left| moves_left > 0)
        );

        game.wait_selected_unit();
        assert_eq!(game.selected_unit, Some(units[0]));
        assert!(game.waiting_units.is_empty());
    }
}
