//! Keyboard and pointer input handling for the active game screen.

use super::*;

impl Game {
    pub(crate) fn handle_keyboard(&mut self) {
        if self.turn_busy() {
            return;
        }
        if self.overlay == Overlay::Intro {
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Escape) {
                self.perform_action(UiAction::DismissIntro);
            }
            return;
        }
        if is_key_pressed(KeyCode::F5) {
            match self
                .sim
                .save_campaign(std::path::Path::new(crate::QUICKSAVE_PATH))
            {
                Ok(()) => self.notify("Campaign saved."),
                Err(error) => self.notify_important(format!("Save failed. {error}")),
            }
            return;
        }
        if is_key_pressed(KeyCode::F9) {
            match ClientSimulation::load_campaign(std::path::Path::new(crate::QUICKSAVE_PATH)) {
                Ok(simulation) => {
                    let autosave_enabled = self.autosave_enabled;
                    let audio_settings = (
                        self.music_open,
                        self.music_muted,
                        self.music_volume,
                        self.sfx_muted,
                        self.sfx_volume,
                    );
                    *self = Self::from_client_simulation(simulation);
                    self.autosave_enabled = autosave_enabled;
                    self.music_open = audio_settings.0;
                    self.music_muted = audio_settings.1;
                    self.music_volume = audio_settings.2;
                    self.sfx_muted = audio_settings.3;
                    self.sfx_volume = audio_settings.4;
                    self.notify("Campaign loaded.");
                }
                Err(error) => self.notify_important(format!("Load failed. {error}")),
            }
            return;
        }
        if self.overlay.is_council() {
            if is_key_pressed(KeyCode::Escape) {
                if self.reviewed_offer.take().is_some() {
                    return;
                }
                self.overlay = Overlay::None;
            }
            return;
        }
        if self.overlay.hides_map() {
            if is_key_pressed(KeyCode::Escape) {
                self.overlay = Overlay::None;
            }
            return;
        }
        if self.stack_menu.is_some() {
            if is_key_pressed(KeyCode::Escape) {
                self.stack_menu = None;
            }
            return;
        }
        if self.pending_city_view.is_some() {
            return;
        }
        if is_key_pressed(KeyCode::Z) {
            self.zoom = if (self.zoom - ZOOMED_OUT).abs() < 0.01 {
                ZOOMED_IN
            } else {
                ZOOMED_OUT
            };
            self.normalize_camera();
        }
        let shift_down = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        if is_key_pressed(KeyCode::Escape) {
            if self.selected_city.is_some() {
                self.selected_city = None;
            } else {
                self.targeting = TargetingMode::None;
            }
        }
        if is_key_pressed(KeyCode::Tab) {
            self.select_next_unit();
        }
        if is_key_pressed(KeyCode::Enter) && shift_down {
            self.perform_action(UiAction::EndTurn);
            return;
        }
        if is_key_pressed(KeyCode::L) {
            self.perform_action(UiAction::BoardOrDeboard);
        }
        if is_key_pressed(KeyCode::B) {
            let bombard = self
                .selected_unit
                .and_then(|id| self.sim.unit_action_view(id))
                .is_some_and(|view| view.bombard);
            self.perform_action(if bombard {
                UiAction::BeginBombard
            } else {
                UiAction::FoundCity
            });
        }
        if is_key_pressed(KeyCode::R) {
            self.perform_action(UiAction::Road);
        }
        if is_key_pressed(KeyCode::I) {
            self.perform_action(UiAction::Irrigate);
        }
        if is_key_pressed(KeyCode::H) {
            self.perform_action(UiAction::HomeCity);
        }
        for (key, kind) in [
            (KeyCode::V, WorkKind::Cultivate),
            (KeyCode::P, WorkKind::Plant),
        ] {
            if is_key_pressed(key) {
                self.perform_action(UiAction::TerrainWork(kind));
            }
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
            self.targeting = if self.targeting == TargetingMode::Goto {
                TargetingMode::None
            } else {
                TargetingMode::Goto
            };
            self.notify(if self.targeting == TargetingMode::Goto {
                "Go-to active: choose a destination tile."
            } else {
                "Go-to cancelled."
            });
        }
        if is_key_pressed(KeyCode::Y)
            && let Some(id) = self.selected_unit
            && let Some(view) = self.sim.unit_action_view(id)
            && self.sim.apply(Command::Sentry(id))
        {
            self.notify(format!("{} is now on sentry duty.", view.unit_name));
            self.select_next_unit();
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
                self.camera += vec2(dx + dy, dy - dx) * 0.5;
                self.normalize_camera();
                return;
            }
        }

        let direction = if is_key_pressed(KeyCode::Up)
            || is_key_pressed(KeyCode::Key8)
            || is_key_pressed(KeyCode::Kp8)
        {
            Some(0)
        } else if is_key_pressed(KeyCode::Down)
            || is_key_pressed(KeyCode::Key2)
            || is_key_pressed(KeyCode::Kp2)
        {
            Some(4)
        } else if is_key_pressed(KeyCode::Left)
            || is_key_pressed(KeyCode::Key4)
            || is_key_pressed(KeyCode::Kp4)
        {
            Some(6)
        } else if is_key_pressed(KeyCode::Right)
            || is_key_pressed(KeyCode::Key6)
            || is_key_pressed(KeyCode::Kp6)
        {
            Some(2)
        } else if is_key_pressed(KeyCode::Q)
            || is_key_pressed(KeyCode::Key7)
            || is_key_pressed(KeyCode::Kp7)
        {
            Some(7)
        } else if is_key_pressed(KeyCode::C)
            || is_key_pressed(KeyCode::Key3)
            || is_key_pressed(KeyCode::Kp3)
        {
            Some(3)
        } else if is_key_pressed(KeyCode::Key9) || is_key_pressed(KeyCode::Kp9) {
            Some(1)
        } else if is_key_pressed(KeyCode::Key1) || is_key_pressed(KeyCode::Kp1) {
            Some(5)
        } else {
            None
        };
        if let (Some(id), Some(direction)) = (self.selected_unit, direction)
            && self.move_selected(Command::MoveCiv3(id, direction), id)
        {
            self.select_next_if_exhausted(id);
        }
    }

    fn select_next_if_exhausted(&mut self, id: u32) {
        let exhausted = self
            .sim
            .unit_moves_left_view(id)
            .is_none_or(|moves_left| moves_left <= 0);
        if exhausted {
            self.select_next_unit();
        }
    }

    pub(super) fn viewport_rect() -> Rect {
        Rect::new(0.0, 0.0, screen_width(), screen_height())
    }

    pub(super) fn context_rect(&self) -> Rect {
        let width = Self::corner_width();
        let height = if self.selected_unit_index().is_some() {
            let inline = self
                .selected_unit
                .and_then(|id| self.sim.unit_context_view_by_id(id))
                .is_none_or(|view| hud::unit_status_inline(&view, width));
            hud::unit_card_height(inline)
        } else if self.selected_city_index().is_some() {
            174.0 + hud::INFO_BLOCK_H + 4.0
        } else {
            hud::EMPTY_CARD_H
        };
        Rect::new(
            screen_width() - HUD_MARGIN - width,
            screen_height() - HUD_MARGIN - height,
            width,
            height,
        )
    }

    pub(super) fn corner_width() -> f32 {
        (screen_width() * 0.27).min(252.0)
    }

    pub(super) fn minimap_rect(&self) -> Rect {
        let aspect = (2 * self.sim.map_wrap().width()) as f32 / self.sim.map_wrap().height() as f32;
        let width = Self::corner_width().min(174.0 * aspect);
        let height = width / aspect;
        Rect::new(
            HUD_MARGIN,
            screen_height() - HUD_MARGIN - height,
            width,
            height,
        )
    }

    pub(super) fn minimap_tile_origin(&self, rect: Rect, x: i32, y: i32) -> Vec2 {
        let scale = rect.h / self.sim.map_wrap().height() as f32;
        vec2(
            rect.x + (2 * x + y.rem_euclid(2)) as f32 * scale,
            rect.y + y as f32 * scale,
        )
    }

    pub(super) fn city_manager_rect() -> Rect {
        let width = (screen_width() - 32.0).min(1440.0);
        let height = (screen_height() - 32.0).min(960.0);
        Rect::new(
            (screen_width() - width) * 0.5,
            (screen_height() - height) * 0.5,
            width,
            height,
        )
    }

    pub(super) fn focus_camera_on(&mut self, native_x: i32, native_y: i32) {
        let (map_x, map_y) = native_to_map(native_x, native_y);
        self.camera = vec2(map_x as f32, map_y as f32);
    }

    pub(super) fn camera_overview_position(&self) -> Vec2 {
        vec2(
            (self.camera.x - self.camera.y + MAP_W as f32)
                .rem_euclid(2.0 * self.sim.map_wrap().width() as f32),
            self.camera.x + self.camera.y - MAP_W as f32,
        )
    }

    pub(super) fn normalize_camera(&mut self) {
        let position = self.camera_overview_position();
        let y = map_view::normalize_camera_y(
            self.sim.map_wrap(),
            position.y,
            Self::viewport_rect().h,
            self.zoom,
        );
        let map_x = (position.x + y) * 0.5;
        self.camera = vec2(map_x, y + MAP_W as f32 - map_x);
    }

    pub(super) fn screen_to_world(&self, point: Vec2) -> Option<(i32, i32)> {
        civ3_projection::screen_to_world(
            point,
            Self::viewport_rect(),
            self.camera,
            self.zoom,
            self.sim.map_wrap(),
        )
    }

    pub(super) fn world_screen_position(&self, x: i32, y: i32) -> Vec2 {
        civ3_projection::world_screen_position(
            self.sim.map_wrap(),
            Self::viewport_rect(),
            self.camera_overview_position(),
            self.zoom,
            x,
            y,
        )
    }

    pub(crate) fn ui_action_at(point: Vec2, buttons: &[(Rect, UiAction)]) -> Option<UiAction> {
        buttons
            .iter()
            .rev()
            .find(|(rect, action)| {
                rect.contains(point)
                    && (!matches!(action, UiAction::WorkCityTile(..))
                        || super::city_map::tile_hit(*rect, point))
            })
            .map(|(_, action)| action.clone())
    }

    pub(crate) fn handle_mouse(&mut self, buttons: &[(Rect, UiAction)]) {
        if self.turn_busy() {
            if is_mouse_button_pressed(MouseButton::Left) {
                let pointer = ui_pointer();
                if let Some(action) = Self::ui_action_at(pointer, buttons) {
                    self.perform_action(action);
                }
            }
            return;
        }
        if self.pending_city_view.is_some() {
            return;
        }
        if self.overlay == Overlay::Research {
            self.handle_research_mouse(buttons);
            return;
        }
        let mouse = vec2(mouse_position().0, mouse_position().1);

        if self.music_open
            && is_mouse_button_down(MouseButton::Left)
            && let Some(action) = Self::ui_action_at(mouse, buttons)
            && matches!(
                action,
                UiAction::SetMusicVolume(_) | UiAction::SetSfxVolume(_)
            )
        {
            self.perform_action(action);
            return;
        }

        if let Some(menu) = &mut self.stack_menu {
            let (_, wheel) = mouse_wheel();
            if wheel < 0.0 {
                menu.scroll_offset = menu.scroll_offset.saturating_add(1);
            } else if wheel > 0.0 {
                menu.scroll_offset = menu.scroll_offset.saturating_sub(1);
            }
        }

        if self.overlay != Overlay::None {
            let pressed = is_mouse_button_pressed(MouseButton::Left);
            // Holding the button down on a rate slider keeps following the pointer.
            let dragging_slider =
                self.overlay == Overlay::Government && is_mouse_button_down(MouseButton::Left);
            if (pressed || dragging_slider)
                && let Some(action) = Self::ui_action_at(mouse, buttons)
                && (pressed || matches!(action, UiAction::SetRates(_)))
            {
                self.perform_action(action);
            }
            return;
        }

        if self.stack_menu.is_some() && is_mouse_button_pressed(MouseButton::Left) {
            if let Some(action) = Self::ui_action_at(mouse, buttons) {
                if matches!(
                    &action,
                    UiAction::SelectUnit(_)
                        | UiAction::SelectUnitStack(_)
                        | UiAction::SelectSameUnitType(_)
                        | UiAction::ShowCityInfo(_)
                ) {
                    self.perform_action(action);
                } else {
                    self.stack_menu = None;
                }
            } else {
                self.stack_menu = None;
            }
            return;
        }

        if self.stack_menu.is_some() && is_mouse_button_pressed(MouseButton::Middle) {
            self.stack_menu = None;
        }

        if is_mouse_button_pressed(MouseButton::Right) {
            self.left_map_press = None;
            self.goto_drag_unit = None;
            self.dragging = false;
            self.stack_menu = None;
            if self.targeting == TargetingMode::None
                && Self::viewport_rect().contains(mouse)
                && !self.minimap_rect().contains(mouse)
                && !self.context_rect().contains(mouse)
                && Self::ui_action_at(mouse, buttons).is_none()
                && let Some((x, y)) = self.screen_to_world(mouse)
            {
                if let Some(city_id) = self.sim.human_city_at_view(x, y) {
                    self.stack_menu = Some(StackMenu {
                        position: mouse,
                        units: self.garrison_unit_ids_at(x, y),
                        city_id: Some(city_id),
                        scroll_offset: 0,
                    });
                    return;
                }
                let units = self.sim.human_units_at_view(x, y);
                if units.len() > 1 {
                    self.stack_menu = Some(StackMenu {
                        position: mouse,
                        units: units.into_iter().map(|unit| unit.id).collect(),
                        city_id: None,
                        scroll_offset: 0,
                    });
                    return;
                }
            }
        }

        if self.selected_city.is_some() {
            if is_mouse_button_pressed(MouseButton::Left)
                && let Some(action) = Self::ui_action_at(mouse, buttons)
            {
                self.perform_action(action);
            }
            return;
        }

        if is_mouse_button_pressed(MouseButton::Middle) {
            self.dragging = true;
            self.drag_last = mouse;
        }
        if self.dragging && is_mouse_button_down(MouseButton::Middle) {
            let delta = mouse - self.drag_last;
            let a = delta.x / (TILE_W * 0.5 * self.zoom);
            let b = delta.y / (TILE_H * 0.5 * self.zoom);
            self.camera.x -= (a + b) * 0.5;
            self.camera.y -= (b - a) * 0.5;
            self.normalize_camera();
            self.drag_last = mouse;
        }
        if is_mouse_button_released(MouseButton::Middle) {
            self.dragging = false;
        }

        if let Some(press) = self.left_map_press.as_ref() {
            if is_mouse_button_down(MouseButton::Left)
                && press.unit_id.is_some()
                && (mouse - press.screen).length() > 8.0
            {
                self.goto_drag_unit = press.unit_id;
            }
            if is_mouse_button_released(MouseButton::Left) {
                let press = self.left_map_press.take().unwrap();
                if let Some(unit_id) = self.goto_drag_unit.take() {
                    if Self::viewport_rect().contains(mouse)
                        && !self.minimap_rect().contains(mouse)
                        && !self.context_rect().contains(mouse)
                        && Self::ui_action_at(mouse, buttons).is_none()
                        && let Some((x, y)) = self.screen_to_world(mouse)
                    {
                        if !self.selected_units.contains(&unit_id) {
                            self.set_unit_selection(vec![unit_id], Some(unit_id));
                        }
                        let group_order = self.selected_units.len() > 1;
                        if self.move_selected_to(x, y, unit_id) && !group_order {
                            self.select_next_unit();
                        }
                    }
                } else {
                    self.select_at(press.tile.0, press.tile.1);
                }
                return;
            }
        }

        if !is_mouse_button_pressed(MouseButton::Left) {
            return;
        }
        if let Some(action) = Self::ui_action_at(mouse, buttons) {
            self.perform_action(action);
            return;
        }
        if self.selected_city.is_some() {
            return;
        }
        let minimap = self.minimap_rect();
        if minimap.contains(mouse) {
            let scale = minimap.h / self.sim.map_wrap().height() as f32;
            let natural_x = ((mouse.x - minimap.x) / scale).floor() as i32;
            let native_y = (((mouse.y - minimap.y) / scale).floor() as i32)
                .min(self.sim.map_wrap().height() - 1);
            let native_x = (natural_x - native_y.rem_euclid(2)).div_euclid(2);
            self.focus_camera_on(native_x.rem_euclid(self.sim.map_wrap().width()), native_y);
            return;
        }
        if !Self::viewport_rect().contains(mouse) {
            return;
        }
        let Some((x, y)) = self.screen_to_world(mouse) else {
            return;
        };
        match self.targeting {
            TargetingMode::None => {
                let stack = self.sim.human_units_at_view(x, y);
                let unit_id = stack
                    .iter()
                    .find(|unit| Some(unit.id) == self.selected_unit)
                    .or_else(|| {
                        (self.sim.human_city_at_view(x, y).is_none())
                            .then(|| stack.first())
                            .flatten()
                    })
                    .map(|unit| unit.id);
                self.left_map_press = Some(LeftMapPress {
                    screen: mouse,
                    tile: (x, y),
                    unit_id,
                });
            }
            TargetingMode::Paradrop => self.perform_action(UiAction::ParadropTo(x, y)),
            TargetingMode::NuclearStrike => self.perform_action(UiAction::NuclearStrikeTo(x, y)),
            TargetingMode::Connect => self.perform_action(UiAction::ConnectTo(x, y)),
            TargetingMode::Patrol => self.perform_action(UiAction::PatrolTo(x, y)),
            TargetingMode::CaptureUnits => self.perform_action(UiAction::CaptureUnitsTo(x, y)),
            TargetingMode::Bombard { .. } => self.perform_action(UiAction::BombardTo(x, y)),
            TargetingMode::Goto => {
                let group_order = self.selected_units.len() > 1;
                if let Some(id) = self.selected_unit
                    && self.move_selected_to(x, y, id)
                    && !group_order
                {
                    self.select_next_unit();
                }
                self.targeting = TargetingMode::None;
            }
        }
    }

    fn move_selected_to(&mut self, x: i32, y: i32, active_id: u32) -> bool {
        let command = if self.selected_units.len() > 1 {
            Command::GoToGroup(self.selected_units.clone(), x, y)
        } else {
            Command::GoTo(active_id, x, y)
        };
        self.move_selected(command, active_id)
    }

    pub(crate) fn select_at(&mut self, x: i32, y: i32) {
        let clicked_selected_unit = self
            .selected_unit
            .and_then(|unit_id| self.sim.unit_position_view(unit_id))
            == Some((x, y));
        if !clicked_selected_unit && let Some(unit_id) = self.selected_unit.take() {
            self.waiting_units.insert(unit_id);
        }

        let city_id = self.sim.human_city_at_view(x, y);
        let stack = if city_id.is_some() {
            self.garrison_unit_ids_at(x, y)
        } else {
            self.sim
                .human_units_at_view(x, y)
                .into_iter()
                .map(|unit| unit.id)
                .collect()
        };
        let current = stack.iter().position(|id| Some(*id) == self.selected_unit);
        let next = current
            .and_then(|index| stack.get(index + 1))
            .or_else(|| stack.first());
        if let Some(unit_id) = next {
            self.waiting_units.remove(unit_id);
            self.action_page = 0;
            self.set_unit_selection(vec![*unit_id], Some(*unit_id));
            self.selected_city = None;
            return;
        }
        if let Some(city_id) = city_id {
            self.show_city_info(city_id);
            return;
        }
        if let Some(unit_id) = self.selected_unit.take() {
            self.waiting_units.insert(unit_id);
            self.set_unit_selection(Vec::new(), None);
        }
    }

    fn garrison_unit_ids_at(&self, x: i32, y: i32) -> Vec<u32> {
        self.sim
            .human_units_at_view(x, y)
            .into_iter()
            .filter(|unit| !unit.transported)
            .map(|unit| unit.id)
            .collect()
    }
}
