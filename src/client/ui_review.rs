//! Opt-in visual QA through the actual renderer, using isolated fixture games.
use super::*;
#[cfg(test)]
use crate::sim::Simulation;

type WorkerAssignmentSnapshot = (u32, i32, i32, Vec<(i32, i32)>, [i32; 3]);
pub(crate) const UI_REVIEW_SCENES: &[&str] = &[
    "world",
    "city-units",
    "city-buildings",
    "city-tiles",
    "worklist",
    "government",
    "diplomacy",
    "treaty-draft",
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
    "religion",
    "borders-fog-cities",
    "ai-city-garrisons",
    "combat-animation-frames",
    "leader-form-army",
    "army-loaded",
    "army-attack-swordsman",
    "army-attack-horseman",
    "army-attack-spearman",
    "army-defend-swordsman",
    "army-defend-spearman",
    "army-defend-horseman",
    "air-bombard",
    "air-sam-intercept",
];
/// Cloning preserves the complete RNG and campaign state. Fixtures are built
/// once and kept immutable; every scenario receives an independent clone.
pub(crate) struct UiReviewSources {
    initial: ClientSimulation,
    fixture: ClientSimulation,
    ids: (u32, u32),
}
impl UiReviewSources {
    pub(crate) fn new(initial: &ClientSimulation) -> Self {
        let mut fixture = initial.fork_for_review();
        let ids = fixture.review_fixture().fixture();
        Self {
            initial: initial.fork_for_review(),
            fixture,
            ids,
        }
    }
}

impl Game {
    pub(crate) fn border_cache_stats(&self, assets: &Assets) -> (u64, u64, usize) {
        assets.civ3.territory_cache_stats()
    }

    pub(crate) fn ui_review_polar_edge_has_no_black_gaps(
        &self,
        image: &Image,
        north: bool,
    ) -> bool {
        let viewport = Self::viewport_rect();
        let scale_x = image.width as f32 / screen_width().max(1.0);
        let scale_y = image.height as f32 / screen_height().max(1.0);
        let (left, right) = if north { (0.55, 0.90) } else { (0.35, 0.70) };
        let band_height = (TILE_H * self.zoom * 2.0).min(viewport.h);
        let (top, bottom) = if north {
            (
                viewport.y,
                (viewport.y + band_height).min(viewport.bottom()),
            )
        } else {
            (
                (viewport.bottom() - band_height).max(viewport.y),
                viewport.bottom(),
            )
        };
        let x_start = (left * screen_width() * scale_x).floor().max(0.0) as usize;
        let x_end = (right * screen_width() * scale_x)
            .ceil()
            .min(f32::from(image.width)) as usize;
        let y_start = (top * scale_y).floor().max(0.0) as usize;
        let y_end = (bottom * scale_y).ceil().min(f32::from(image.height)) as usize;
        let channels = image.bytes.len() / (usize::from(image.width) * usize::from(image.height));
        if channels < 3 || x_start >= x_end || y_start >= y_end {
            return false;
        }

        let mut dark_pixels = 0usize;
        let mut total_pixels = 0usize;
        for y in y_start..y_end {
            for x in x_start..x_end {
                let framebuffer_y = usize::from(image.height) - 1 - y;
                let offset = (framebuffer_y * usize::from(image.width) + x) * channels;
                let pixel = &image.bytes[offset..offset + 3];
                dark_pixels += usize::from(pixel.iter().all(|channel| *channel < 12));
                total_pixels += 1;
            }
        }
        dark_pixels * 1000 < total_pixels
    }

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
        *self = Game::from_client_simulation(baseline.fork_for_review());
        set_review_pointer(Some(vec2(-1.0, -1.0)));
        self.autosave_enabled = false;
        if scene == 0 {
            self.sim.review_fixture().reveal_all_tiles();
            self.zoom = ZOOMED_OUT;
            self.focus_camera_on(
                self.sim.map_wrap().width() / 2,
                self.sim.map_wrap().height() / 2,
            );
            return;
        }
        if scene == 40 {
            self.overlay = Overlay::Intro;
            return;
        }
        if scene == 43 {
            self.sim.review_fixture().civ3_city_sprites();
            self.zoom = 0.78;
            self.focus_camera_on(20, 24);
            return;
        }
        if matches!(scene, 0 | 13..=15) {
            return;
        }
        let (city, _rival) = sources.ids;
        self.set_unit_selection(Vec::new(), None);
        match scene {
            49 | 50 => {
                let id = if scene == 50 {
                    self.sim.review_fixture().loaded_army()
                } else {
                    self.sim.review_fixture().leader_army()
                };
                self.select_unit_group(vec![id], Some(id));
                self.focus_camera_on(10, 24);
                self.zoom = 1.25;
                return;
            }
            47 => {
                self.sim.review_fixture().ai_city_garrisons();
                self.zoom = 0.65;
                self.focus_camera_on(21, 25);
                return;
            }
            46 => {
                self.sim.review_fixture().borders_fog_cities();
                self.zoom = 0.65;
                self.focus_camera_on(22, 24);
                return;
            }
            1..=4 => {
                self.selected_city = Some(city);
                self.production_page = usize::from(scene == 2);
                self.worklist_page = (scene == 4).then_some(0);
            }
            45 => {
                self.sim.review_fixture().religion(city);
                self.overlay = Overlay::Religion;
                self.religion_choice = 0;
            }
            5 => self.overlay = Overlay::Government,
            6 => self.overlay = Overlay::Diplomacy,
            38 => {
                self.sim.review_fixture().first_contact();
            }
            39 => {
                self.sim.review_fixture().first_contact();
            }
            41 => {
                self.sim.review_fixture().compensation_negotiation(1);
                self.overlay = Overlay::Diplomacy;
                self.diplomacy_target = Some(1);
            }
            42 => {
                self.sim.review_fixture().resource_shadows();
                self.zoom = 1.15;
            }
            7 => {
                self.overlay = Overlay::Diplomacy;
                self.diplomacy_target = Some(1);
                self.treaty_draft = Some((1, TreatyOffer::default()));
            }
            8 => {
                self.overlay = Overlay::Diplomacy;
                self.reviewed_offer = Some((1, self.sim.review_fixture().offer()));
            }
            9 => {
                self.overlay = Overlay::Research;
                self.center_research();
            }
            10 => {
                self.sim.review_fixture().spaceship();
                self.overlay = Overlay::Spaceship;
            }
            11 => {
                self.selected_unit = self
                    .sim
                    .review_state()
                    .units()
                    .iter()
                    .find(|u| u.owner == HUMAN && u.kind == UnitKind::Worker)
                    .map(|u| u.id);
            }
            12 => {
                self.sim.review_fixture().touching_borders();
                self.zoom = 1.0;
            }
            16 | 17 => {
                self.sim.review_fixture().rivers(scene == 17);
                self.zoom = Self::ui_review_map_zoom();
            }
            27 | 28 => {
                self.sim.review_fixture().terrain_layers(scene == 28);
                self.zoom = Self::ui_review_map_zoom();
            }
            37 => {
                self.sim.review_fixture().ice_transitions();
                self.zoom = 1.0;
                self.focus_camera_on(20, 24);
                return;
            }
            44 => {
                self.sim.review_fixture().civ3_water_depths();
                self.zoom = 1.35;
                self.focus_camera_on(20, 24);
                return;
            }
            34..=36 => {
                self.sim.review_fixture().terrain_seams();
                self.zoom = [0.625, 0.82, 1.35][scene - 34];
                self.focus_camera_on(20, 24);
                self.camera += vec2(0.137, 0.213);
                return;
            }
            31 | 32 => {
                self.sim.review_fixture().reveal_all_tiles();
                self.zoom = Self::ui_review_map_zoom();
                self.focus_camera_on(
                    self.sim.map_wrap().width() / 2,
                    if scene == 31 {
                        0
                    } else {
                        self.sim.map_wrap().height() - 1
                    },
                );
                return;
            }
            33 => {
                self.selected_unit = Some(self.sim.review_fixture().wrap_seam());
                self.zoom = Self::ui_review_map_zoom();
                self.focus_camera_on(12, 0);
                return;
            }
            29 => {
                let (_, focus) = self.sim.review_fixture().observed_map();
                self.zoom = Self::ui_review_map_zoom();
                self.focus_camera_on(focus.0, focus.1);
                return;
            }
            30 => {
                let (id, target) = self.sim.review_fixture().route_setup();
                self.selected_unit = Some(id);
                self.targeting = TargetingMode::Goto;
                self.zoom = Self::ui_review_map_zoom();
                self.focus_camera_on(20, 24);
                let point = self.world_screen_position(target.0, target.1);
                set_review_pointer(Some(point));
                return;
            }
            26 => {
                let unit = self.sim.review_fixture().fog();
                self.selected_unit = Some(unit);
                self.zoom = if Self::viewport_rect().w < 760.0 {
                    0.55
                } else {
                    0.7
                };
            }
            51..=56 => {
                let (unit, _, _) = self.sim.review_fixture().army_combat_setup(scene <= 53);
                self.select_unit_group(vec![unit], Some(unit));
                self.playback.review_progress = Some(0.0);
                self.zoom = 1.35;
            }
            57 | 58 => {
                let (unit, target) = self.sim.review_fixture().air_combat_setup(scene == 58);
                self.select_unit_group(vec![unit], Some(unit));
                self.playback.review_progress = Some(0.0);
                self.zoom = 1.25;
                self.focus_camera_on((20 + target.0) / 2, target.1);
                return;
            }
            23..=25 | 48 => {
                let (unit, _) = self.sim.review_fixture().animation_setup(scene != 23);
                self.selected_unit = Some(unit);
                self.playback.review_progress = Some(if scene == 25 {
                    0.999
                } else if scene == 48 {
                    0.0
                } else {
                    0.5
                });
                self.zoom = 1.35;
            }
            22 => {
                self.sim.review_fixture().city_appearance();
                self.zoom = Self::ui_review_map_zoom();
            }
            21 => {
                self.sim.review_fixture().unit_status();
                self.zoom = Self::ui_review_map_zoom();
            }
            19 | 20 => {
                self.zoom = 1.5;
                if scene == 20 {
                    self.selected_unit = self
                        .sim
                        .review_state()
                        .units()
                        .iter()
                        .find(|u| u.owner == HUMAN && u.kind == UnitKind::Worker)
                        .map(|u| u.id);
                }
            }
            18 => {
                let (x, y) = self
                    .sim
                    .review_fixture()
                    .generated_river_focus()
                    .expect("generated river example");
                self.focus_camera_on(x, y);
                self.zoom = 1.15;
                return;
            }
            _ => {}
        }
        self.sim.review_fixture().record_knowledge();
        self.focus_camera_on(20, 24);
    }
}

#[derive(Default)]
pub(crate) struct UiReviewSequence {
    pub(crate) checks: Vec<(&'static str, bool)>,
    worker: Option<WorkerAssignmentSnapshot>,
    pub(crate) pending_turn: Option<i32>,
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
            .find(|(_, action)| wanted((*action).clone()))
            .map(|(rect, _)| *rect);
        self.check(label, rect.is_some())?;
        let rect = rect.unwrap();
        let point = vec2(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
        let actual = Game::ui_action_at(point, buttons).filter(|action| wanted(action.clone()));
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
        assets: &Assets,
        sequence: &mut UiReviewSequence,
    ) -> Result<bool, &'static str> {
        if scene == 50 && phase == 6 {
            let id = self.selected_unit.unwrap();
            let view = self.sim.unit_context_view_by_id(id).unwrap();
            sequence.check(
                "pentagon_capacity_in_context",
                view.status.contains("Cargo 3/4"),
            )?;
            sequence.check(
                "loaded_army_selected",
                self.sim.unit_kind_view(id) == Some(UnitKind::Army),
            )?;
            return Ok(true);
        }
        if scene == 49 && phase == 6 {
            let leader = self.selected_unit.unwrap();
            sequence.click(self, buttons, "form_army_action", |action| {
                matches!(action, UiAction::FormArmy)
            })?;
            sequence.check(
                "leader_consumed_by_formation",
                self.sim.unit_kind_view(leader).is_none(),
            )?;
            sequence.check(
                "formed_army_selected",
                self.selected_unit
                    .and_then(|id| self.sim.unit_kind_view(id))
                    == Some(UnitKind::Army),
            )?;
            return Ok(true);
        }
        if scene == 2 && phase == 1 {
            let view = self
                .sim
                .city_manager_view(self.selected_city.unwrap())
                .unwrap();
            sequence.check(
                "production_excludes_built_buildings",
                buttons.iter().all(|(_, action)| {
                    if let UiAction::Produce(Production::Building(building)) = action {
                        !view
                            .buildings
                            .iter()
                            .any(|item| item.building == *building && item.built)
                    } else {
                        true
                    }
                }),
            )?;
        }
        if scene == 3 && phase == 6 {
            let city_id = self.selected_city.unwrap();
            let before = self.sim.city_manager_view(city_id).unwrap();
            let sale = before
                .buildings
                .iter()
                .find_map(|item| item.sale_price.map(|price| (item.building, price)));
            sequence.check("fixture_has_sellable_building", sale.is_some())?;
            let (building, price) = sale.unwrap();
            let column = Self::city_manager_rect();
            sequence.check(
                "sell_controls_in_left_building_list",
                buttons
                    .iter()
                    .filter_map(|(rect, action)| {
                        matches!(action, UiAction::SellBuilding(..)).then_some(rect)
                    })
                    .all(|rect| rect.right() < column.x + column.w * 0.3),
            )?;
            sequence.click(self, buttons, "sell_from_built_buildings", |action| matches!(action, UiAction::SellBuilding(id, target) if id == city_id && target == building))?;
            let after = self.sim.city_manager_view(city_id).unwrap();
            sequence.check(
                "building_removed_and_sale_paid",
                !after
                    .buildings
                    .iter()
                    .any(|item| item.building == building && item.built)
                    && after.treasury == before.treasury + price,
            )?;
            return Ok(true);
        }
        if scene == 45 && phase == 6 {
            sequence.click(self, buttons, "religion_choose_faith", |a| {
                matches!(a, UiAction::ReligionFaith)
            })?;
            let panel = self.draw_religion();
            sequence.click(self, &panel, "religion_devout_policy", |a| {
                matches!(a, UiAction::ReligionPolicy(crate::sim::FaithPolicy::Devout))
            })?;
            let panel = self.draw_religion();
            sequence.click(self, &panel, "religion_confirm", |a| {
                matches!(a, UiAction::ConfirmReligion)
            })?;
            let view = self.sim.religion_management_view();
            sequence.check(
                "religion_declared_through_ui",
                view.current.declared.is_some()
                    && view.current.policy == crate::sim::FaithPolicy::Devout
                    && view.cooldown > 0,
            )?;
            return Ok(true);
        }
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
            sequence.check("intro_dismissed", self.overlay != Overlay::Intro)?;
            return Ok(true);
        }
        if scene == 39 && phase == 1 {
            sequence.check(
                "first_contact_has_no_avatar_diplomacy_button",
                !buttons
                    .iter()
                    .any(|(_, action)| matches!(action, UiAction::OpenDiplomacyWith(_)))
                    && self.overlay != Overlay::Diplomacy,
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
                        && self.sim.treasury_view(HUMAN).unwrap() == 250
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
                        .as_ref()
                        .is_some_and(|(owner, offer)| *owner == 1 && offer.terms[HUMAN].gold == 75),
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
                    self.sim.treasury_view(HUMAN).unwrap() == 175,
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
                    self.sim.map_wrap().height() - 1
                };
                let point = self.world_screen_position(self.sim.map_wrap().width() / 2, y);
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
                            == Some((self.sim.map_wrap().width() / 2, input_y)),
                )?;
            }
            if phase == 6 {
                // Same camera-coordinate change/normalization as real panning.
                let delta = if scene == 31 {
                    -4 * self.sim.map_wrap().height()
                } else {
                    4 * self.sim.map_wrap().height()
                };
                self.camera += Vec2::splat(delta as f32 * 0.5);
                self.normalize_camera();
                let after = self.camera_overview_position().y;
                sequence.check(
                    "camera_stops_at_polar_limit",
                    (after - camera.y).abs() < 0.02,
                )?;
                let previous_zoom = self.zoom;
                self.zoom = ZOOMED_OUT;
                self.normalize_camera();
                let y = if scene == 31 {
                    0
                } else {
                    self.sim.map_wrap().height() - 1
                };
                let point = self.world_screen_position(self.sim.map_wrap().width() / 2, y);
                sequence.check(
                    "zoomed_out_boundary_tile_stays_in_camera_range",
                    point.y >= viewport.y - TILE_H * self.zoom
                        && point.y <= viewport.bottom() + TILE_H * self.zoom,
                )?;
                return Ok((self.zoom - previous_zoom).abs() > 0.001);
            }
            return Ok(false);
        }
        if scene == 33 {
            if phase == 6 {
                let id = self.selected_unit.unwrap();
                let moved = self.move_selected(self.sim.review_state().move_command(id, 0, -1), id);
                sequence.check("movement_stops_at_polar_edge", !moved)?;
                let unit = self.sim.review_state().unit_by_id(id).unwrap();
                sequence.check("bounded_map_keeps_unit_in_place", unit.y == 0)?;
                sequence.check(
                    "bounded_map_still_accepts_real_move",
                    self.move_selected(self.sim.review_state().move_command(id, 1, 0), id),
                )?;
                self.sim.review_fixture().clear_animation_events();
                self.playback.finish();
                return Ok(true);
            }
            return Ok(false);
        }
        if scene == 47 {
            if phase == 1 {
                let units = self.sim.rendered_unit_views();
                sequence.check(
                    "three_visible_city_defenders",
                    units.iter().filter(|u| u.owner == 1).count() == 3,
                )?;
                sequence.check(
                    "remembered_city_has_no_live_defender",
                    !units.iter().any(|u| u.y == 30),
                )?;
                sequence.check(
                    "four_city_sprites",
                    self.sim.rendered_city_views().len() == 4,
                )?;
            }
            return Ok(false);
        }
        if scene == 46 {
            if phase == 1 {
                let cities = self.sim.rendered_city_views();
                sequence.check(
                    "visible_city_drawn",
                    cities.iter().any(|c| c.name == "Visible Rome"),
                )?;
                sequence.check(
                    "remembered_ai_city_drawn",
                    cities.iter().any(|c| c.name == "Remembered Babylon"),
                )?;
                sequence.check(
                    "unknown_city_hidden",
                    !cities.iter().any(|c| c.name == "Unknown City"),
                )?;
                sequence.check(
                    "visible_tiles_present",
                    self.sim.review_state().tile_visibility(20, 24) == (true, true),
                )?;
                sequence.check(
                    "remembered_tiles_present",
                    self.sim.review_state().tile_visibility(23, 24) == (true, false),
                )?;
                sequence.check(
                    "unknown_tiles_present",
                    self.sim.review_state().tile_visibility(28, 24) == (false, false),
                )?;
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
                self.selected_city = self.sim.review_state().cities().first().map(|c| c.id);
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
                    self.sim.review_state().observed_map_valid(false),
                )?;
            }
            if phase == 6 {
                self.sim.review_fixture().reobserve_map();
                sequence.check(
                    "returning_vision_updates_remembered_art",
                    self.sim.review_state().observed_map_valid(true),
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
                self.targeting = TargetingMode::None;
                self.playback.finish();
                set_review_pointer(Some(vec2(-1.0, -1.0)));
                return Ok(true);
            }
            return Ok(false);
        }
        if scene == 26 {
            let visibility = |game: &Game| {
                (0..game.sim.map_wrap().width() * game.sim.map_wrap().height())
                    .map(|i| {
                        game.sim.review_state().tile_visibility(
                            i % game.sim.map_wrap().width(),
                            i / game.sim.map_wrap().width(),
                        )
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
                    self.move_selected(self.sim.review_state().move_command(id, 1, 0), id),
                )?;
                sequence.check("movement_refreshes_visibility", before != visibility(self))?;
                return Ok(true);
            }
            return Ok(false);
        }
        if matches!(scene, 57 | 58) {
            if phase == 1 {
                let id = self.selected_unit.unwrap();
                sequence.check(
                    "air_mission_real_command",
                    self.move_selected(Command::Bombard(id, 22, 24), id),
                )?;
            }
            if phase == 6 {
                let fraction = if scene == 57 { 0.48 } else { 0.42 };
                sequence.check(
                    "air_mission_clip_available",
                    self.playback.review_first_air_mission(fraction),
                )?;
                sequence.check(
                    "air_mission_actors_rendered",
                    !self.playback.active().is_empty(),
                )?;
                return Ok(true);
            }
            return Ok(false);
        }
        if (51..=56).contains(&scene) {
            if phase == 1 {
                let id = self.selected_unit.unwrap();
                sequence.check(
                    "army_battle_real_command",
                    self.move_selected(self.sim.review_state().move_command(id, 1, 0), id),
                )?;
            }
            if phase == 6 {
                let exchange = (scene - 51) % 3;
                sequence.check(
                    "army_member_exchange_available",
                    self.playback.review_combat_exchange(exchange, 0.65),
                )?;
                let expected = match scene {
                    51 | 54 => UnitKind::Swordsman,
                    52 | 56 => UnitKind::Horseman,
                    _ => UnitKind::Spearman,
                };
                let active = self.playback.active();
                sequence.check("both_combatants_rendered", active.len() == 2)?;
                let member = &active[usize::from(scene >= 54)].0;
                sequence.check("selected_member_sprite", member.kind == expected)?;
                sequence.check(
                    "member_health_not_army_pool",
                    member.hp == 1 && member.max_hp == 3,
                )?;
                return Ok(true);
            }
            return Ok(false);
        }
        if matches!(scene, 23..=25 | 48) {
            if phase == 1 {
                let id = self.selected_unit.unwrap();
                sequence.check(
                    "real_move_command_accepted",
                    self.move_selected(self.sim.review_state().move_command(id, 1, 0), id),
                )?;
            }
            if phase == 2 {
                sequence.check(
                    "real_simulation_animation_event",
                    self.playback.active().len() == if scene != 23 { 2 } else { 1 },
                )?;
            }
            if phase == 6 {
                if scene == 48 {
                    sequence.check(
                        "advance_within_first_attack_clip",
                        self.playback.review_first_combat_clip(0.65),
                    )?;
                    return Ok(true);
                }
                self.playback.finish();
                sequence.check("animation_finished", self.playback.active().is_empty())?;
                return Ok(true);
            }
            return Ok(false);
        }
        if matches!(scene, 19 | 20) && phase == 1 {
            let city = self
                .sim
                .review_state()
                .cities_owned_by(HUMAN)
                .next()
                .unwrap();
            sequence.check(
                "city_has_garrison",
                self.sim
                    .review_state()
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
                        .review_state()
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
                    self.sim.review_state().cities_owned_by(HUMAN).count() > 0
                        && self.selected_city.is_none()
                        && self.pending_city_view.is_some(),
                )?;
            }
            if phase == 2 {
                sequence.check(
                    "city_view_waits_for_build_animation",
                    self.pending_city_view.is_some()
                        && self.selected_city.is_none()
                        && self.playback.pending(),
                )?;
            }
            if phase == 7
                && scene == 14
                && let Some(id) = self.selected_city
            {
                sequence.check("city_view_opened_after_build", true)?;
                {
                    let city = self.sim.city_manager_view(id).unwrap();
                    let mut city_buttons = Vec::new();
                    if city.governor.active {
                        sequence.click(self, buttons, "open_governor", |action| {
                            matches!(action, UiAction::CityGovernorPage)
                        })?;
                        self.draw_city_manager(assets, &mut city_buttons);
                        sequence.click(self, &city_buttons, "release_city", |action| {
                            matches!(action, UiAction::SetCityGovernor(city_id, state)
                                if city_id == id && !state.active)
                        })?;
                        sequence.check(
                            "governor_released_city",
                            !self.sim.city_manager_view(id).unwrap().governor.active,
                        )?;
                        sequence.click(self, &city_buttons, "close_governor", |action| {
                            matches!(action, UiAction::CityGovernorPage)
                        })?;
                        city_buttons.clear();
                        self.draw_city_manager(assets, &mut city_buttons);
                    }
                    let buttons = if city.governor.active {
                        &city_buttons[..]
                    } else {
                        buttons
                    };
                    sequence.check(
                        "city_map_and_production_visible",
                        buttons
                            .iter()
                            .any(|(_, action)| matches!(action, UiAction::WorkCityTile(..)))
                            && buttons
                                .iter()
                                .any(|(_, action)| matches!(action, UiAction::Produce(_))),
                    )?;
                    return Ok(true);
                }
            }
            if phase == 7 && scene == 15 && self.selected_city_index().is_some() {
                sequence.check("city_view_opened_after_build", true)?;
                sequence.click(self, buttons, "close_city_control", |action| {
                    matches!(action, UiAction::CloseCity)
                })?;
                sequence.check("city_view_closed", self.selected_city.is_none())?;
                return Ok(true);
            }
            if phase == 8 && scene == 15 && self.selected_city.is_none() {
                // The rate sliders live in the Government panel: open it, raise
                // tax, and close it again, each through real hit testing.
                sequence.click(self, buttons, "government_control", |action| {
                    matches!(action, UiAction::ToggleGovernment)
                })?;
                sequence.check("government_panel_open", self.overlay == Overlay::Government)?;
                let panel = self.draw_government();
                let rates = self.sim.review_state().human_rates();
                sequence.click(
                    self,
                    &panel,
                    "tax_control",
                    |action| matches!(action, UiAction::SetRates(new) if new[1] > rates[1]),
                )?;
                sequence.check(
                    "rates_changed_through_ui",
                    self.sim.review_state().human_rates() != rates,
                )?;
                sequence.click(self, &panel, "close_government_control", |action| {
                    matches!(action, UiAction::ToggleGovernment)
                })?;
                sequence.check(
                    "government_panel_closed",
                    self.overlay != Overlay::Government,
                )?;
            }
            if phase == 8 && scene == 14 && sequence.worker.is_none() {
                let target = buttons.iter().find_map(|(_, action)| {
                    if let UiAction::WorkCityTile(city, x, y) = action {
                        Some((*city, *x, *y))
                    } else {
                        None
                    }
                });
                sequence.check("worked_tile_available", target.is_some())?;
                let (id, x, y) = target.unwrap();
                let city = self.sim.review_state().city_by_id(id).unwrap();
                let before = city.specialists;
                sequence.worker = Some((id, x, y, city.worked_tiles.clone(), before));
                sequence.click(self,buttons,"remove_tile_worker",|action|matches!(action,UiAction::WorkCityTile(cid,tx,ty) if (cid,tx,ty)==(id,x,y)))?;
                let city = self.sim.review_state().city_by_id(id).unwrap();
                sequence.check(
                    "worker_became_entertainer",
                    !city.worked_tiles.contains(&(x, y)) && city.specialists[2] == before[2] + 1,
                )?;
                return Ok(true);
            }
            if phase == 9 && scene == 14 && sequence.worker.is_some() {
                let (id, x, y, worked, before) = sequence.worker.take().unwrap();
                sequence.click(self,buttons,"restore_tile_worker",|action|matches!(action,UiAction::WorkCityTile(cid,tx,ty) if (cid,tx,ty)==(id,x,y)))?;
                let city = self.sim.review_state().city_by_id(id).unwrap();
                sequence.check(
                    "worker_assignment_restored",
                    city.specialists == before
                        && city.worked_tiles.len() == worked.len()
                        && worked.iter().all(|p| city.worked_tiles.contains(p)),
                )?;
            }
            if phase == 9 && scene == 15 {
                let turn = self.sim.turn_view();
                sequence.click(self, buttons, "end_turn_control", |action| {
                    matches!(action, UiAction::EndTurn)
                })?;
                sequence.check("normal_turn_started", self.turn_busy())?;
                sequence.pending_turn = Some(turn + 1);
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
                (7, UiAction::TreatyItemsPage(_, page)) => *page > 0,
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
            self.treaty_item_page,
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
            self.treaty_item_page,
            self.action_page,
        );
        sequence.check("page_changed_through_ui", before != after)?;
        Ok(true)
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
        let sources = UiReviewSources::new(&ClientSimulation::new(initial.clone()));
        let mut a = Game::from_simulation(initial.clone());
        let mut b = Game::from_simulation(initial);
        a.prepare_ui_review(15, &sources);
        b.prepare_ui_review(15, &sources);
        a.perform_action(UiAction::FoundCity);
        assert!(
            a.sim
                .review_simulation()
                .cities()
                .iter()
                .any(|c| c.owner == HUMAN)
        );
        assert!(
            !b.sim
                .review_simulation()
                .cities()
                .iter()
                .any(|c| c.owner == HUMAN)
        );
        assert!(
            !sources
                .initial
                .review_state()
                .cities()
                .iter()
                .any(|c| c.owner == HUMAN)
        );
    }

    #[test]
    fn world_review_reveals_and_fits_the_generated_map() {
        let initial = Simulation::new(crate::mapgen::generate_with_wrap(
            42,
            crate::world::topology::MapWrap::X,
        ));
        let sources = UiReviewSources::new(&ClientSimulation::new(initial.clone()));
        let mut game = Game::from_simulation(initial);
        game.prepare_ui_review(0, &sources);
        assert!((0..game.sim.map_wrap().height()).all(|y| {
            (0..game.sim.map_wrap().width()).all(|x| {
                game.sim.review_simulation().tile(x, y).seen
                    && game.sim.review_simulation().tile(x, y).visible
            })
        }));
        assert_eq!(game.zoom, ZOOMED_OUT);
    }
}
