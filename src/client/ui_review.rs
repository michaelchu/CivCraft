//! Opt-in visual QA through the actual renderer, using isolated fixture games.
use super::*;
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
    "spy",
    "investigation",
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
];
/// Cloning preserves the complete RNG and campaign state. Fixtures are built
/// once and kept immutable; every scenario receives an independent clone.
pub(crate) struct UiReviewSources {
    initial: Simulation,
    fixture: Simulation,
    ids: (u32, u32, u32),
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
    pub(crate) fn prepare_ui_review(&mut self, scene: usize, sources: &UiReviewSources) {
        let baseline = if matches!(scene, 0 | 15..=17 | 20 | 33 | 34) {
            &sources.initial
        } else {
            &sources.fixture
        };
        *self = Game::from_simulation(baseline.clone());
        set_review_pointer(Some(vec2(-1.0, -1.0)));
        self.autosave_enabled = false;
        if matches!(scene, 0 | 15..=17) {
            return;
        }
        let (city, spy, rival) = sources.ids;
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
                self.sim.ui_review_spy_war();
                self.selected_unit = Some(spy);
                self.spy_targets = Some((rival, 0));
            }
            12 => self.investigation = self.sim.investigate_city(spy, rival),
            13 => {
                self.selected_unit = self
                    .sim
                    .units()
                    .iter()
                    .find(|u| u.owner == HUMAN && u.kind == UnitKind::Engineer)
                    .map(|u| u.id);
            }
            14 => {
                self.sim.ui_review_touching_borders();
                self.zoom = 1.0;
            }
            18 | 19 => {
                self.sim.ui_review_rivers(scene == 19);
                self.zoom = if Self::viewport_rect().w < 760.0 {
                    0.625
                } else {
                    1.0
                };
            }
            29 | 30 => {
                self.sim.ui_review_terrain_layers(scene == 30);
                self.zoom = if Self::viewport_rect().w < 760.0 {
                    0.625
                } else {
                    1.0
                };
            }
            33 | 34 => {
                self.sim.ui_review_reveal_poles();
                self.zoom = if Self::viewport_rect().w < 760.0 {
                    0.625
                } else {
                    1.0
                };
                self.focus_camera_on(MAP_W / 2, if scene == 33 { 0 } else { MAP_H - 1 });
                return;
            }
            35 => {
                self.selected_unit = Some(self.sim.ui_review_wrap_seam());
                self.zoom = if Self::viewport_rect().w < 760.0 {
                    0.625
                } else {
                    1.0
                };
                self.focus_camera_on(12, 0);
                return;
            }
            31 => {
                let (_, focus) = self.sim.ui_review_observed_map();
                self.zoom = if Self::viewport_rect().w < 760.0 {
                    0.625
                } else {
                    1.0
                };
                self.focus_camera_on(focus.0, focus.1);
                return;
            }
            32 => {
                let (id, target) = self.sim.ui_review_route_setup();
                self.selected_unit = Some(id);
                self.goto_mode = true;
                self.zoom = if Self::viewport_rect().w < 760.0 {
                    0.625
                } else {
                    1.0
                };
                self.focus_camera_on(20, 24);
                let point = self.world_screen_position(target.0, target.1);
                set_review_pointer(Some(point));
                return;
            }
            28 => {
                let unit = self.sim.ui_review_fog();
                self.selected_unit = Some(unit);
                self.zoom = if Self::viewport_rect().w < 760.0 {
                    0.625
                } else {
                    1.0
                };
            }
            25..=27 => {
                let (unit, _) = self.sim.ui_review_animation_setup(scene != 25);
                self.selected_unit = Some(unit);
                self.playback.review_progress = Some(if scene == 27 { 0.9 } else { 0.5 });
                self.zoom = 1.35;
            }
            24 => {
                self.sim.ui_review_city_appearance();
                self.zoom = if Self::viewport_rect().w < 760.0 {
                    0.625
                } else {
                    1.0
                };
            }
            23 => {
                self.sim.ui_review_unit_status();
                self.zoom = if Self::viewport_rect().w < 760.0 {
                    0.625
                } else {
                    1.0
                };
            }
            21 | 22 => {
                self.zoom = 1.5;
                if scene == 22 {
                    self.selected_unit = self
                        .sim
                        .units()
                        .iter()
                        .find(|u| u.owner == HUMAN && u.kind == UnitKind::Engineer)
                        .map(|u| u.id);
                }
            }
            20 => {
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
    worker: Option<(u32, i32, i32, Vec<(i32, i32)>, [i32; 3])>,
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
        if matches!(scene, 33 | 34) {
            let viewport = Self::viewport_rect();
            let camera = self.camera_overview_position();
            let wrapping = self.sim.map_wrap.wraps_y();
            if phase == 1 {
                let y = if scene == 33 { 0 } else { MAP_H - 1 };
                let point = self.world_screen_position(MAP_W / 2, y);
                let expected = if wrapping {
                    viewport.y + viewport.h * 0.5
                } else if scene == 33 {
                    viewport.y + TILE_H * self.zoom * 0.5
                } else {
                    viewport.bottom() - TILE_H * self.zoom * 0.5
                };
                sequence.check(
                    "native_polar_scroll_anchor",
                    (point.y - expected).abs() < 0.02,
                )?;
                sequence.check(
                    "polar_tile_input_matches_rendering",
                    self.screen_to_world(point) == Some((MAP_W / 2, y)),
                )?;
            }
            if phase == 6 {
                // Same camera-coordinate change/normalization as real panning.
                let delta = if wrapping {
                    if scene == 33 { -(MAP_H + 1) } else { MAP_H + 1 }
                } else if scene == 33 {
                    -4 * MAP_H
                } else {
                    4 * MAP_H
                };
                self.camera += Vec2::splat(delta as f32 * 0.5);
                self.normalize_camera();
                let after = self.camera_overview_position().y;
                let expected = if wrapping {
                    camera.y + if scene == 33 { -1.0 } else { 1.0 }
                } else {
                    camera.y
                };
                sequence.check(
                    "camera_clamps_or_wraps_like_native",
                    (after - expected).abs() < 0.02,
                )?;
                return Ok(wrapping);
            }
            return Ok(false);
        }
        if scene == 35 {
            if phase == 6 {
                let id = self.selected_unit.unwrap();
                let wrapping = self.sim.map_wrap.wraps_y();
                let moved = self.move_selected(Command::Move(id, 0, -1), id);
                sequence.check("movement_obeys_configured_vertical_seam", moved == wrapping)?;
                let unit = self.sim.units().iter().find(|u| u.id == id).unwrap();
                sequence.check(
                    "vertical_seam_has_expected_destination",
                    unit.y == if wrapping { MAP_H - 1 } else { 0 },
                )?;
                if !wrapping {
                    sequence.check(
                        "bounded_map_still_accepts_real_move",
                        self.move_selected(Command::Move(id, 1, 0), id),
                    )?;
                }
                self.sim.take_visual_events();
                self.playback.finish();
                return Ok(true);
            }
            return Ok(false);
        }
        if scene == 29 {
            if phase == 1 {
                sequence.check(
                    "terrain_gallery_runs_shared_renderer",
                    self.selected_city.is_none(),
                )?;
            }
            return Ok(false);
        }
        if scene == 30 {
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
        if scene == 31 {
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
        if scene == 32 {
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
        if scene == 28 {
            let visibility = |game: &Game| {
                (0..MAP_W * MAP_H)
                    .map(|i| {
                        let tile = game.sim.tile(i % MAP_W, i / MAP_W);
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
        if (25..=27).contains(&scene) {
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
                    self.playback.active().len() == if scene != 25 { 2 } else { 1 },
                )?;
            }
            if phase == 6 {
                self.playback.finish();
                sequence.check("animation_finished", self.playback.active().is_empty())?;
                return Ok(true);
            }
            return Ok(false);
        }
        if matches!(scene, 21 | 22) && phase == 1 {
            let city = self.sim.cities().iter().find(|c| c.owner == HUMAN).unwrap();
            sequence.check(
                "city_has_garrison",
                self.sim
                    .units()
                    .iter()
                    .any(|u| (u.x, u.y) == (city.x, city.y)),
            )?;
            sequence.check(
                "expected_unit_focus",
                if scene == 21 {
                    self.selected_unit.is_none()
                } else {
                    self.sim
                        .units()
                        .iter()
                        .any(|u| Some(u.id) == self.selected_unit && (u.x, u.y) == (city.x, city.y))
                },
            )?;
        }
        if (15..=17).contains(&scene) {
            if phase == 1 {
                sequence.click(self, buttons, "found_city_control", |action| {
                    matches!(action, UiAction::FoundCity)
                })?;
                sequence.check(
                    "city_founded_through_ui",
                    self.selected_city_index().is_some(),
                )?;
            }
            if phase == 2 && scene == 16 {
                sequence.click(self, buttons, "worked_tiles_control", |action| {
                    matches!(action, UiAction::CityTiles)
                })?;
                sequence.check("worked_tiles_open", self.city_tiles)?;
            }
            if phase == 2 && scene == 17 {
                sequence.click(self, buttons, "close_city_control", |action| {
                    matches!(action, UiAction::CloseCity)
                })?;
            }
            if phase == 3 && scene == 17 {
                let rates = self.sim.players()[HUMAN].rates;
                sequence.click(self, buttons, "tax_control", |action| {
                    matches!(action, UiAction::RaiseRate(1))
                })?;
                sequence.check(
                    "rates_changed_through_ui",
                    self.sim.players()[HUMAN].rates != rates,
                )?;
            }
            if phase == 6 && scene == 16 {
                let target = buttons.iter().find_map(|(_, action)| {
                    if let UiAction::WorkCityTile(city, x, y) = action {
                        Some((*city, *x, *y))
                    } else {
                        None
                    }
                });
                sequence.check("worked_tile_available", target.is_some())?;
                let (id, x, y) = target.unwrap();
                let city = self.sim.cities().iter().find(|c| c.id == id).unwrap();
                let before = city.specialists;
                sequence.worker = Some((id, x, y, city.worked_tiles.clone(), before));
                sequence.click(self,buttons,"remove_tile_worker",|action|matches!(action,UiAction::WorkCityTile(cid,tx,ty) if (cid,tx,ty)==(id,x,y)))?;
                let city = self.sim.cities().iter().find(|c| c.id == id).unwrap();
                sequence.check(
                    "worker_became_entertainer",
                    !city.worked_tiles.contains(&(x, y)) && city.specialists[2] == before[2] + 1,
                )?;
                return Ok(true);
            }
            if phase == 8 && scene == 16 {
                let (id, x, y, worked, before) = sequence.worker.take().unwrap();
                sequence.click(self,buttons,"restore_tile_worker",|action|matches!(action,UiAction::WorkCityTile(cid,tx,ty) if (cid,tx,ty)==(id,x,y)))?;
                let city = self.sim.cities().iter().find(|c| c.id == id).unwrap();
                sequence.check(
                    "worker_assignment_restored",
                    city.specialists == before
                        && city.worked_tiles.len() == worked.len()
                        && worked.iter().all(|p| city.worked_tiles.contains(p)),
                )?;
            }
            if phase == 6 && scene == 17 {
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
                (11, UiAction::SpyTargets(Some((_, page)))) => *page > 0,
                (12, UiAction::InvestigationPage(page)) => *page > 0,
                (13, UiAction::ActionPage(page)) => *page > 0,
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
            self.spy_targets,
            self.investigation_page,
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
            self.spy_targets,
            self.investigation_page,
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
}
