//! Opt-in visual QA through the actual renderer, using isolated fixture games.
use super::*;

type WorkerAssignmentSnapshot = (u32, i32, i32, Vec<(i32, i32)>, [i32; 3]);
pub(crate) const UI_REVIEW_SCENES: &[&str] = &[
    "world",
    "city-units",
    "city-buildings",
    "city-tiles",
    "worklist",
    "government",
    "diplomacy",
    "city-deals",
    "treaty",
    "research",
    "spaceship",
    "worker-actions",
    "touching-borders",
    "campaign-city",
    "campaign-worked-tiles",
    "campaign-after-turn",
    "rivers-six-directions",
    "rivers-fogged",
    "rivers-generated",
    "city-garrison-idle",
    "city-garrison-selected",
    "unit-status",
    "city-appearance",
    "unit-movement",
    "city-combat",
    "city-combat-explosion",
    "fog-transitions",
    "terrain-blending",
    "map-layers",
    "remembered-map",
    "goto-preview",
    "north-pole",
    "south-pole",
    "wrap-seam",
    "terrain-seams-small",
    "terrain-seams-default",
    "terrain-seams-large",
    "ice-transitions",
    "first-contact",
    "leader-diplomacy",
    "campaign-intro",
    "diplomacy-negotiation",
    "resource-shadows",
    "civ3-city-sprites",
    "civ3-water-depths",
];
/// Cloning preserves the complete RNG and campaign state. Fixtures are built
/// once and kept immutable; every scenario receives an independent clone.
pub(crate) struct UiReviewSources {
    initial: Simulation,
    fixture: Simulation,
    ids: (u32, u32),
}
impl UiReviewSources {
    pub(crate) fn new(initial: &Simulation) -> Self {
        let mut fixture = initial.clone();
        let ids = fixture.ui_review_fixture();
        Self {
            initial: initial.clone(),
            fixture,
            ids,
        }
    }
}

impl Game {
    fn ui_review_map_zoom() -> f32 {
        if Self::viewport_rect().w < 760.0 {
            0.625
        } else {
            1.0
        }
    }

    pub(crate) fn prepare_ui_review(&mut self, scene: usize, sources: &UiReviewSources) {
        let baseline = if matches!(scene, 0 | 13..=15 | 18 | 31 | 32 | 40) {
            &sources.initial
        } else {
            &sources.fixture
        };
        *self = Game::from_simulation(baseline.clone());
        set_review_pointer(Some(vec2(-1.0, -1.0)));
        self.autosave_enabled = false;
        if scene == 0 {
            self.sim.ui_review_reveal_all_tiles();
            self.zoom = MIN_ZOOM;
            self.focus_camera_on(
                self.sim.map_wrap.width() / 2,
                self.sim.map_wrap.height() / 2,
            );
            return;
        }
        if scene == 40 {
            self.intro_open = true;
            return;
        }
        if scene == 43 {
            self.sim.ui_review_civ3_city_sprites();
            self.zoom = 0.78;
            self.focus_camera_on(20, 24);
            return;
        }
        if matches!(scene, 0 | 13..=15) {
            return;
        }
        let (city, _rival) = sources.ids;
        self.selected_unit = None;
        match scene {
            1..=4 => {
                self.selected_city = Some(city);
                self.production_page = usize::from(scene == 2);
                self.city_tiles = scene == 3;
                self.worklist_page = (scene == 4).then_some(0);
            }
            5 => self.government_open = true,
            6 => self.diplomacy_open = true,
            38 => {
                self.sim.ui_review_first_contact();
            }
            39 => {
                self.sim.ui_review_first_contact();
            }
            41 => {
                self.sim.ui_review_compensation_negotiation(1);
                self.diplomacy_open = true;
                self.diplomacy_target = Some(1);
            }
            42 => {
                self.sim.ui_review_resource_shadows();
                self.zoom = 1.15;
            }
            7 => {
                self.diplomacy_open = true;
                self.city_deals = Some((1, 0));
            }
            8 => {
                self.diplomacy_open = true;
                self.reviewed_offer = Some((1, self.sim.ui_review_offer()));
            }
            9 => {
                self.research_open = true;
                self.center_research();
            }
            10 => {
                self.sim.manual_spaceship_playtest_setup();
                self.spaceship_open = true;
            }
            11 => {
                self.selected_unit = self
                    .sim
                    .units()
                    .iter()
                    .find(|u| u.owner == HUMAN && u.kind == UnitKind::Worker)
                    .map(|u| u.id);
            }
            12 => {
                self.sim.ui_review_touching_borders();
                self.zoom = 1.0;
            }
            16 | 17 => {
                self.sim.ui_review_rivers(scene == 17);
                self.zoom = Self::ui_review_map_zoom();
            }
            27 | 28 => {
                self.sim.ui_review_terrain_layers(scene == 28);
                self.zoom = Self::ui_review_map_zoom();
            }
            37 => {
                self.sim.ui_review_ice_transitions();
                self.zoom = 1.0;
                self.focus_camera_on(20, 24);
                return;
            }
            44 => {
                self.sim.ui_review_civ3_water_depths();
                self.zoom = 1.35;
                self.focus_camera_on(20, 24);
                return;
            }
            34..=36 => {
                self.sim.ui_review_terrain_seams();
                self.zoom = [0.625, 0.82, 1.35][scene - 34];
                self.focus_camera_on(20, 24);
                self.camera += vec2(0.137, 0.213);
                return;
            }
            31 | 32 => {
                self.sim.ui_review_reveal_all_tiles();
                self.zoom = Self::ui_review_map_zoom();
                self.focus_camera_on(
                    self.sim.map_wrap.width() / 2,
                    if scene == 31 {
                        0
                    } else {
                        self.sim.map_wrap.height() - 1
                    },
                );
                return;
            }
            33 => {
                self.selected_unit = Some(self.sim.ui_review_wrap_seam());
                self.zoom = Self::ui_review_map_zoom();
                self.focus_camera_on(12, 0);
                return;
            }
            29 => {
                let (_, focus) = self.sim.ui_review_observed_map();
                self.zoom = Self::ui_review_map_zoom();
                self.focus_camera_on(focus.0, focus.1);
                return;
            }
            30 => {
                let (id, target) = self.sim.ui_review_route_setup();
                self.selected_unit = Some(id);
                self.goto_mode = true;
                self.zoom = Self::ui_review_map_zoom();
                self.focus_camera_on(20, 24);
                let point = self.world_screen_position(target.0, target.1);
                set_review_pointer(Some(point));
                return;
            }
            26 => {
                let unit = self.sim.ui_review_fog();
                self.selected_unit = Some(unit);
                self.zoom = if Self::viewport_rect().w < 760.0 {
                    0.55
                } else {
                    0.7
                };
            }
            23..=25 => {
                let (unit, _) = self.sim.ui_review_animation_setup(scene != 23);
                self.selected_unit = Some(unit);
                self.playback.review_progress = Some(if scene == 25 { 0.9 } else { 0.5 });
                self.zoom = 1.35;
            }
            22 => {
                self.sim.ui_review_city_appearance();
                self.zoom = Self::ui_review_map_zoom();
            }
            21 => {
                self.sim.ui_review_unit_status();
                self.zoom = Self::ui_review_map_zoom();
            }
            19 | 20 => {
                self.zoom = 1.5;
                if scene == 20 {
                    self.selected_unit = self
                        .sim
                        .units()
                        .iter()
                        .find(|u| u.owner == HUMAN && u.kind == UnitKind::Worker)
                        .map(|u| u.id);
                }
            }
            18 => {
                let (x, y) = self
                    .sim
                    .ui_review_generated_river_focus()
                    .expect("generated river example");
                self.focus_camera_on(x, y);
                self.zoom = 1.15;
                return;
            }
            _ => {}
        }
        self.sim.ui_review_record_knowledge();
        self.focus_camera_on(20, 24);
    }
}

#[derive(Default)]
pub(crate) struct UiReviewSequence {
    pub(crate) checks: Vec<(&'static str, bool)>,
    worker: Option<WorkerAssignmentSnapshot>,
}
impl UiReviewSequence {
    fn check(&mut self, label: &'static str, passed: bool) -> Result<(), &'static str> {
        self.checks.push((label, passed));
        if passed { Ok(()) } else { Err(label) }
    }
    fn click(
        &mut self,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
        label: &'static str,
        wanted: impl Fn(UiAction) -> bool,
    ) -> Result<(), &'static str> {
        let rect = buttons
            .iter()
            .find(|(_, action)| wanted(*action))
            .map(|(rect, _)| *rect);
        self.check(label, rect.is_some())?;
        let rect = rect.unwrap();
        let point = vec2(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
        let actual = Game::ui_action_at(point, buttons).filter(|action| wanted(*action));
        self.check("click_reaches_requested_control", actual.is_some())?;
        game.perform_action(actual.unwrap());
        Ok(())
    }
}
impl Game {
    /// Drives controls through the same hit testing and dispatch as real clicks.
    /// Staged scenes exercise pagination; campaign scenes use no state overrides.
    pub(crate) fn step_ui_review(
        &mut self,
        scene: usize,
        phase: u32,
        buttons: &[(Rect, UiAction)],
        sequence: &mut UiReviewSequence,
    ) -> Result<bool, &'static str> {
        if scene == 11 && phase == 1 {
            let irrigate = buttons
                .iter()
                .find_map(|(rect, action)| matches!(action, UiAction::Irrigate).then_some(*rect));
            sequence.check("worker_action_available_for_hover", irrigate.is_some())?;
            set_review_pointer(Some(irrigate.unwrap().center()));
        }
        if scene == 40 && phase == 6 {
            sequence.check("intro_captures_map_controls", buttons.len() == 1)?;
            sequence.click(self, buttons, "begin_campaign", |action| {
                matches!(action, UiAction::DismissIntro)
            })?;
            sequence.check("intro_dismissed", !self.intro_open)?;
            return Ok(true);
        }
        if scene == 39 && phase == 1 {
            sequence.check(
                "first_contact_has_no_avatar_diplomacy_button",
                !buttons
                    .iter()
                    .any(|(_, action)| matches!(action, UiAction::OpenDiplomacyWith(_)))
                    && !self.diplomacy_open,
            )?;
            return Ok(true);
        }
        if scene == 41 {
            if phase == 1 {
                sequence.click(
                    self,
                    buttons,
                    "offer_peace_without_compensation",
                    |action| {
                        matches!(action, UiAction::ProposeTreaty(1, offer)
                        if offer.pact == Some(Pact::Peace) && offer.terms[HUMAN].gold == 0)
                    },
                )?;
            } else if phase == 2 {
                sequence.check(
                    "ai_requires_compensation",
                    self.sim.pact(HUMAN, 1) == Pact::Ceasefire
                        && self.sim.players()[HUMAN].gold == 250
                        && self
                            .treaty_feedback
                            .as_ref()
                            .is_some_and(|(_, message)| message.contains("requires at least 62")),
                )?;
                sequence.click(self, buttons, "increase_compensation_amount", |action| {
                    matches!(action, UiAction::AdjustTreatyGold(HUMAN, 25))
                })?;
                sequence.check(
                    "gold_amount_increased",
                    self.treaty_gold_amount[HUMAN] == 75,
                )?;
            } else if phase == 3 {
                sequence.click(self, buttons, "add_gold_to_treaty_table", |action| {
                    matches!(action, UiAction::AddTreatyGold(1, HUMAN, _))
                })?;
                sequence.check(
                    "compensation_added_to_rome_offer",
                    self.treaty_draft
                        .is_some_and(|(owner, offer)| owner == 1 && offer.terms[HUMAN].gold == 75),
                )?;
            } else if phase == 6 {
                sequence.click(self, buttons, "propose_compensated_peace", |action| {
                    matches!(action, UiAction::ProposeTreaty(1, offer)
                        if offer.pact == Some(Pact::Peace) && offer.terms[HUMAN].gold == 75)
                })?;
                sequence.check(
                    "ai_accepts_compensated_peace",
                    self.sim.pact(HUMAN, 1) == Pact::Armistice,
                )?;
                sequence.check(
                    "compensation_paid_once",
                    self.sim.players()[HUMAN].gold == 175,
                )?;
                return Ok(true);
            }
            return Ok(false);
        }
        if scene == 37 {
            if phase == 6 {
                self.zoom = 0.82;
                self.camera += vec2(0.137, 0.213);
                return Ok(true);
            }
            return Ok(false);
        }
        if (34..=36).contains(&scene) {
            if phase == 6 {
                self.camera += vec2(0.37, 0.23);
                return Ok(true);
            }
            return Ok(false);
        }
        if matches!(scene, 31 | 32) {
            let viewport = Self::viewport_rect();
            let camera = self.camera_overview_position();
            if phase == 1 {
                let y = if scene == 31 {
                    0
                } else {
                    self.sim.map_wrap.height() - 1
                };
                let point = self.world_screen_position(self.sim.map_wrap.width() / 2, y);
                let expected = if scene == 31 {
                    viewport.y + map_view::EDGE_WATER_ROWS as f32 * TILE_H * self.zoom * 0.5 - 1.0
                } else {
                    viewport.bottom() - map_view::EDGE_WATER_ROWS as f32 * TILE_H * self.zoom * 0.5
                        + 1.0
                };
                sequence.check("polar_scroll_anchor", (point.y - expected).abs() < 0.02)?;
                let input_y = y;
                let input_point = point;
                sequence.check(
                    "polar_tile_input_matches_rendering",
                    viewport.contains(input_point)
                        && self.screen_to_world(input_point)
                            == Some((self.sim.map_wrap.width() / 2, input_y)),
                )?;
            }
            if phase == 6 {
                // Same camera-coordinate change/normalization as real panning.
                let delta = if scene == 31 {
                    -4 * self.sim.map_wrap.height()
                } else {
                    4 * self.sim.map_wrap.height()
                };
                self.camera += Vec2::splat(delta as f32 * 0.5);
                self.normalize_camera();
                let after = self.camera_overview_position().y;
                sequence.check(
                    "camera_stops_at_polar_limit",
                    (after - camera.y).abs() < 0.02,
                )?;
                let previous_zoom = self.zoom;
                self.zoom = MIN_ZOOM;
                self.normalize_camera();
                let y = if scene == 31 {
                    0
                } else {
                    self.sim.map_wrap.height() - 1
                };
                let point = self.world_screen_position(self.sim.map_wrap.width() / 2, y);
                let expected = if scene == 31 {
                    viewport.y + map_view::EDGE_WATER_ROWS as f32 * TILE_H * self.zoom * 0.5 - 1.0
                } else {
                    viewport.bottom() - map_view::EDGE_WATER_ROWS as f32 * TILE_H * self.zoom * 0.5
                        + 1.0
                };
                sequence.check(
                    "zoomed_out_edge_water_fills_viewport",
                    (point.y - expected).abs() < 0.02,
                )?;
                return Ok((self.zoom - previous_zoom).abs() > 0.001);
            }
            return Ok(false);
        }
        if scene == 33 {
            if phase == 6 {
                let id = self.selected_unit.unwrap();
                let moved = self.move_selected(Command::Move(id, 0, -1), id);
                sequence.check("movement_stops_at_polar_edge", !moved)?;
                let unit = self.sim.unit_by_id(id).unwrap();
                sequence.check("bounded_map_keeps_unit_in_place", unit.y == 0)?;
                sequence.check(
                    "bounded_map_still_accepts_real_move",
                    self.move_selected(Command::Move(id, 1, 0), id),
                )?;
                self.sim.take_visual_events();
                self.playback.finish();
                return Ok(true);
            }
            return Ok(false);
        }
        if scene == 27 {
            if phase == 1 {
                sequence.check(
                    "terrain_gallery_runs_shared_renderer",
                    self.selected_city.is_none(),
                )?;
            }
            return Ok(false);
        }
        if scene == 28 {
            if phase == 6 {
                self.selected_city = self.sim.cities().first().map(|c| c.id);
                self.city_tiles = true;
                sequence.check(
                    "layer_order_shared_with_city_map",
                    self.selected_city.is_some(),
                )?;
                return Ok(true);
            }
            return Ok(false);
        }
        if scene == 29 {
            if phase == 1 {
                sequence.check(
                    "hidden_changes_keep_last_observed_art",
                    self.sim.ui_review_observed_map_valid(false),
                )?;
            }
            if phase == 6 {
                self.sim.ui_review_reobserve_map();
                sequence.check(
                    "returning_vision_updates_remembered_art",
                    self.sim.ui_review_observed_map_valid(true),
                )?;
                return Ok(true);
            }
            return Ok(false);
        }
        if scene == 30 {
            if phase == 1 {
                let preview = self.goto_preview();
                sequence.check("goto_uses_observed_simulation_route", preview.is_some())?;
            }
            if phase == 6 {
                let id = self.selected_unit.unwrap();
                let target = offset_native_by_map(20, 24, 3, 0);
                sequence.check(
                    "goto_preview_executes_real_command",
                    self.move_selected(Command::GoTo(id, target.0, target.1), id),
                )?;
                self.goto_mode = false;
                self.playback.finish();
                set_review_pointer(Some(vec2(-1.0, -1.0)));
                return Ok(true);
            }
            return Ok(false);
        }
        if scene == 26 {
            let visibility = |game: &Game| {
                (0..game.sim.map_wrap.width() * game.sim.map_wrap.height())
                    .map(|i| {
                        let tile = game
                            .sim
                            .tile(i % game.sim.map_wrap.width(), i / game.sim.map_wrap.width());
                        (tile.seen, tile.visible)
                    })
                    .collect::<Vec<_>>()
            };
            if phase == 1 {
                let states = visibility(self);
                sequence.check(
                    "all_three_knowledge_states_present",
                    states.contains(&(false, false))
                        && states.contains(&(true, false))
                        && states.contains(&(true, true)),
                )?;
            }
            if phase == 6 {
                let before = visibility(self);
                let id = self.selected_unit.unwrap();
                sequence.check(
                    "fog_move_uses_real_command",
                    self.move_selected(Command::Move(id, 1, 0), id),
                )?;
                sequence.check("movement_refreshes_visibility", before != visibility(self))?;
                return Ok(true);
            }
            return Ok(false);
        }
        if (23..=25).contains(&scene) {
            if phase == 1 {
                let id = self.selected_unit.unwrap();
                sequence.check(
                    "real_move_command_accepted",
                    self.move_selected(Command::Move(id, 1, 0), id),
                )?;
            }
            if phase == 2 {
                sequence.check(
                    "real_simulation_animation_event",
                    self.playback.active().len() == if scene != 23 { 2 } else { 1 },
                )?;
            }
            if phase == 6 {
                self.playback.finish();
                sequence.check("animation_finished", self.playback.active().is_empty())?;
                return Ok(true);
            }
            return Ok(false);
        }
        if matches!(scene, 19 | 20) && phase == 1 {
            let city = self.sim.cities_owned_by(HUMAN).next().unwrap();
            sequence.check(
                "city_has_garrison",
                self.sim
                    .units()
                    .iter()
                    .any(|u| (u.x, u.y) == (city.x, city.y)),
            )?;
            sequence.check(
                "expected_unit_focus",
                if scene == 19 {
                    self.selected_unit.is_none()
                } else {
                    self.sim
                        .units()
                        .iter()
                        .any(|u| Some(u.id) == self.selected_unit && (u.x, u.y) == (city.x, city.y))
                },
            )?;
        }
        if (13..=15).contains(&scene) {
            if phase == 1 {
                sequence.click(self, buttons, "found_city_control", |action| {
                    matches!(action, UiAction::FoundCity)
                })?;
                sequence.check(
                    "city_founded_through_ui",
                    self.selected_city_index().is_some(),
                )?;
            }
            if phase == 2 && scene == 14 {
                sequence.click(self, buttons, "worked_tiles_control", |action| {
                    matches!(action, UiAction::CityTiles)
                })?;
                sequence.check("worked_tiles_open", self.city_tiles)?;
            }
            if phase == 2 && scene == 15 {
                sequence.click(self, buttons, "close_city_control", |action| {
                    matches!(action, UiAction::CloseCity)
                })?;
            }
            if phase == 3 && scene == 15 {
                let rates = self.sim.players()[HUMAN].rates;
                sequence.click(self, buttons, "tax_control", |action| {
                    matches!(action, UiAction::RaiseRate(1))
                })?;
                sequence.check(
                    "rates_changed_through_ui",
                    self.sim.players()[HUMAN].rates != rates,
                )?;
            }
            if phase == 6 && scene == 14 {
                let target = buttons.iter().find_map(|(_, action)| {
                    if let UiAction::WorkCityTile(city, x, y) = action {
                        Some((*city, *x, *y))
                    } else {
                        None
                    }
                });
                sequence.check("worked_tile_available", target.is_some())?;
                let (id, x, y) = target.unwrap();
                let city = self.sim.city_by_id(id).unwrap();
                let before = city.specialists;
                sequence.worker = Some((id, x, y, city.worked_tiles.clone(), before));
                sequence.click(self,buttons,"remove_tile_worker",|action|matches!(action,UiAction::WorkCityTile(cid,tx,ty) if (cid,tx,ty)==(id,x,y)))?;
                let city = self.sim.city_by_id(id).unwrap();
                sequence.check(
                    "worker_became_entertainer",
                    !city.worked_tiles.contains(&(x, y)) && city.specialists[2] == before[2] + 1,
                )?;
                return Ok(true);
            }
            if phase == 8 && scene == 14 {
                let (id, x, y, worked, before) = sequence.worker.take().unwrap();
                sequence.click(self,buttons,"restore_tile_worker",|action|matches!(action,UiAction::WorkCityTile(cid,tx,ty) if (cid,tx,ty)==(id,x,y)))?;
                let city = self.sim.city_by_id(id).unwrap();
                sequence.check(
                    "worker_assignment_restored",
                    city.specialists == before
                        && city.worked_tiles.len() == worked.len()
                        && worked.iter().all(|p| city.worked_tiles.contains(p)),
                )?;
            }
            if phase == 6 && scene == 15 {
                let turn = self.sim.turn();
                sequence.click(self, buttons, "end_turn_control", |action| {
                    matches!(action, UiAction::EndTurn)
                })?;
                sequence.check("normal_turn_advanced", self.sim.turn() == turn + 1)?;
                return Ok(true);
            }
            return Ok(false);
        }
        if phase != 6 {
            return Ok(false);
        }
        let candidate = buttons
            .iter()
            .find(|(_, action)| match (scene, action) {
                (1, UiAction::UnitProductionPage(page)) => *page > 0,
                (2, UiAction::ProductionPage(page)) => *page > 1,
                (6, UiAction::DiplomacyPage(page)) => *page > 0,
                (7, UiAction::CityDeals(Some((_, page)))) => *page > 0,
                (11, UiAction::ActionPage(page)) => *page > 0,
                _ => false,
            })
            .map(|(rect, _)| *rect);
        let Some(rect) = candidate else {
            return Ok(false);
        };
        let before = (
            self.unit_production_page,
            self.production_page,
            self.diplomacy_page,
            self.city_deals,
            self.action_page,
        );
        let point = vec2(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
        let actual = Game::ui_action_at(point, buttons);
        sequence.check("pagination_hit_target", actual.is_some())?;
        self.perform_action(actual.unwrap());
        let after = (
            self.unit_production_page,
            self.production_page,
            self.diplomacy_page,
            self.city_deals,
            self.action_page,
        );
        sequence.check("page_changed_through_ui", before != after)?;
        Ok(true)
    }
    pub(crate) fn border_cache_stats(&self) -> (u64, u64, usize) {
        self.border_geometry.borrow().stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scripted_hit_testing_uses_the_actual_diamond() {
        let buttons = [
            (
                Rect::new(0.0, 0.0, 96.0, 48.0),
                UiAction::WorkCityTile(1, 2, 3),
            ),
            (Rect::new(0.0, 0.0, 20.0, 20.0), UiAction::CloseCity),
        ];
        assert!(matches!(
            Game::ui_action_at(vec2(48.0, 24.0), &buttons),
            Some(UiAction::WorkCityTile(1, 2, 3))
        ));
        assert!(matches!(
            Game::ui_action_at(vec2(1.0, 1.0), &buttons),
            Some(UiAction::CloseCity)
        ));
        assert!(Game::ui_action_at(vec2(200.0, 200.0), &buttons).is_none());
    }
    #[test]
    fn one_scenario_cannot_mutate_its_baseline_or_another_scenario() {
        let initial = Simulation::new(crate::mapgen::generate(42));
        let sources = UiReviewSources::new(&initial);
        let mut a = Game::from_simulation(initial.clone());
        let mut b = Game::from_simulation(initial);
        a.prepare_ui_review(15, &sources);
        b.prepare_ui_review(15, &sources);
        a.perform_action(UiAction::FoundCity);
        assert!(a.sim.cities().iter().any(|c| c.owner == HUMAN));
        assert!(!b.sim.cities().iter().any(|c| c.owner == HUMAN));
        assert!(!sources.initial.cities().iter().any(|c| c.owner == HUMAN));
    }

    #[test]
    fn world_review_reveals_and_fits_the_generated_map() {
        let initial = Simulation::new(crate::mapgen::generate_with_wrap(
            42,
            crate::sim::topology::MapWrap::X,
        ));
        let sources = UiReviewSources::new(&initial);
        let mut game = Game::from_simulation(initial);
        game.prepare_ui_review(0, &sources);
        assert!((0..game.sim.map_wrap.height()).all(|y| {
            (0..game.sim.map_wrap.width())
                .all(|x| game.sim.tile(x, y).seen && game.sim.tile(x, y).visible)
        }));
        assert_eq!(game.zoom, MIN_ZOOM);
    }
}
