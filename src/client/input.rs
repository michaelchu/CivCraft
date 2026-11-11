//! Keyboard and pointer input handling for the active game screen.

use super::*;

impl Game {
    pub(crate) fn handle_keyboard(&mut self) {
        if self.intro_open {
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Escape) {
                self.perform_action(UiAction::DismissIntro);
            }
            return;
        }
        if is_key_pressed(KeyCode::F5) {
            let message = match self
                .sim
                .save_campaign(std::path::Path::new(crate::QUICKSAVE_PATH))
            {
                Ok(()) => "Campaign saved.".to_owned(),
                Err(error) => format!("Save failed. {error}"),
            };
            self.notify(message);
            return;
        }
        if is_key_pressed(KeyCode::F9) {
            match Simulation::load_campaign(std::path::Path::new(crate::QUICKSAVE_PATH)) {
                Ok(simulation) => {
                    let autosave_enabled = self.autosave_enabled;
                    let audio_settings = (
                        self.music_open,
                        self.music_muted,
                        self.music_volume,
                        self.sfx_muted,
                        self.sfx_volume,
                    );
                    *self = Self::from_simulation(simulation);
                    self.autosave_enabled = autosave_enabled;
                    self.music_open = audio_settings.0;
                    self.music_muted = audio_settings.1;
                    self.music_volume = audio_settings.2;
                    self.sfx_muted = audio_settings.3;
                    self.sfx_volume = audio_settings.4;
                    self.notify("Campaign loaded.");
                }
                Err(error) => self.notify(format!("Load failed. {error}")),
            }
            return;
        }
        if self.government_open || self.diplomacy_open {
            if is_key_pressed(KeyCode::Escape) {
                if self.reviewed_offer.take().is_some() || self.city_deals.take().is_some() {
                    return;
                }
                self.government_open = false;
                self.diplomacy_open = false;
            }
            return;
        }
        if self.spaceship_open {
            if is_key_pressed(KeyCode::Escape) {
                self.spaceship_open = false;
            }
            return;
        }
        if self.research_open {
            if is_key_pressed(KeyCode::Escape) {
                self.research_open = false;
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
                self.goto_mode = false;
                self.paradrop_mode = false;
                self.bombard_mode = false;
                self.nuclear_mode = false;
                self.rebase_mode = false;
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
                .selected_unit_index()
                .is_some_and(|index| self.sim.can_begin_bombard(&self.sim.units()[index]));
            self.perform_action(if bombard {
                UiAction::BeginBombard
            } else {
                UiAction::FoundCity
            });
        }
        if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::R) {
            self.perform_action(UiAction::BeginRebase);
        } else if is_key_pressed(KeyCode::R) {
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
            self.paradrop_mode = false;
            self.bombard_mode = false;
            self.nuclear_mode = false;
            self.goto_mode = !self.goto_mode;
            self.notify(if self.goto_mode {
                "Go-to active: choose a destination tile."
            } else {
                "Go-to cancelled."
            });
        }
        if is_key_pressed(KeyCode::S)
            && let Some(index) = self.selected_unit_index()
        {
            if self
                .sim
                .can_assign_air_superiority(&self.sim.units()[index])
            {
                self.perform_action(UiAction::AirSuperiority);
            }
        }
        if is_key_pressed(KeyCode::Y)
            && let Some(index) = self.selected_unit_index()
        {
            let name = self.sim.units()[index].kind.name();
            self.sim.apply(Command::Sentry(self.sim.units()[index].id));
            self.notify(format!("{name} is now on sentry duty."));
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
            .units()
            .iter()
            .find(|u| u.id == id)
            .is_none_or(|u| u.moves_left <= 0);
        if exhausted {
            self.select_next_unit();
        }
    }

    pub(super) fn viewport_rect() -> Rect {
        Rect::new(
            0.0,
            TOP_H,
            screen_width(),
            (screen_height() - TOP_H).max(0.0),
        )
    }

    pub(super) fn context_rect(&self) -> Rect {
        let width = Self::corner_width();
        let height = if self.selected_unit_index().is_some() {
            140.0
        } else if self.selected_city_index().is_some() {
            174.0
        } else {
            54.0
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
        let aspect = (2 * self.sim.map_wrap.width()) as f32 / self.sim.map_wrap.height() as f32;
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
        let scale = rect.h / self.sim.map_wrap.height() as f32;
        vec2(
            rect.x + (2 * x + y.rem_euclid(2)) as f32 * scale,
            rect.y + y as f32 * scale,
        )
    }

    pub(super) fn city_manager_rect() -> Rect {
        let width = (screen_width() - 48.0).min(920.0);
        let height = (screen_height() - 80.0).min(640.0);
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
                .rem_euclid(2.0 * self.sim.map_wrap.width() as f32),
            self.camera.x + self.camera.y - MAP_W as f32,
        )
    }

    pub(super) fn normalize_camera(&mut self) {
        let position = self.camera_overview_position();
        let y = map_view::normalize_camera_y(
            self.sim.map_wrap,
            position.y,
            Self::viewport_rect().h,
            self.zoom,
        );
        let map_x = (position.x + y) * 0.5;
        self.camera = vec2(map_x, y + MAP_W as f32 - map_x);
    }

    pub(super) fn screen_to_world(&self, point: Vec2) -> Option<(i32, i32)> {
        self.renderer.screen_to_world(
            point,
            Self::viewport_rect(),
            self.camera,
            self.zoom,
            self.sim.map_wrap,
        )
    }

    pub(super) fn world_screen_position(&self, x: i32, y: i32) -> Vec2 {
        self.renderer.world_screen_position(
            self.sim.map_wrap,
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
            .find(|(rect, action)| {
                rect.contains(point)
                    && (!matches!(action, UiAction::WorkCityTile(..))
                        || super::city_map::tile_hit(*rect, point))
            })
            .map(|(_, action)| *action)
    }

    pub(crate) fn handle_mouse(&mut self, buttons: &[(Rect, UiAction)]) {
        if self.pending_city_view.is_some() {
            return;
        }
        if self.intro_open
            || self.spaceship_open
            || self.government_open
            || self.diplomacy_open
            || self.selected_city.is_some()
        {
            let (x, y) = mouse_position();
            if is_mouse_button_pressed(MouseButton::Left)
                && let Some(action) = Self::ui_action_at(vec2(x, y), buttons)
            {
                self.perform_action(action);
            }
            return;
        }
        if self.research_open {
            self.handle_research_mouse(buttons);
            return;
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
        if let Some(action) = Self::ui_action_at(mouse, buttons) {
            self.perform_action(action);
            return;
        }
        if self.selected_city.is_some() {
            return;
        }
        let minimap = self.minimap_rect();
        if minimap.contains(mouse) {
            let scale = minimap.h / self.sim.map_wrap.height() as f32;
            let natural_x = ((mouse.x - minimap.x) / scale).floor() as i32;
            let native_y = (((mouse.y - minimap.y) / scale).floor() as i32)
                .min(self.sim.map_wrap.height() - 1);
            let native_x = (natural_x - native_y.rem_euclid(2)).div_euclid(2);
            self.focus_camera_on(native_x.rem_euclid(self.sim.map_wrap.width()), native_y);
            return;
        }
        if !Self::viewport_rect().contains(mouse) {
            return;
        }
        let Some((x, y)) = self.screen_to_world(mouse) else {
            return;
        };
        if self.paradrop_mode {
            self.perform_action(UiAction::ParadropTo(x, y));
            return;
        }
        if self.nuclear_mode {
            self.perform_action(UiAction::NuclearStrikeTo(x, y));
            return;
        }
        if self.bombard_mode {
            self.perform_action(UiAction::BombardTo(x, y));
            return;
        }
        if self.rebase_mode {
            self.perform_action(UiAction::RebaseTo(x, y));
            return;
        }
        if self.goto_mode {
            if let Some(id) = self.selected_unit {
                self.move_selected(Command::GoTo(id, x, y), id);
            }
            self.goto_mode = false;
            return;
        }
        self.select_at(x, y);
    }

    pub(crate) fn select_at(&mut self, x: i32, y: i32) {
        // Cities take click priority over their garrisons. Units stationed in
        // a city remain selectable from the city manager.
        if let Some(city) = self
            .sim
            .cities()
            .iter()
            .find(|city| city.owner == HUMAN && city.x == x && city.y == y)
        {
            self.production_page = 0;
            self.unit_production_page = 0;
            self.worklist_page = None;
            self.city_tiles = false;
            self.action_page = 0;
            self.selected_city = Some(city.id);
            self.selected_unit = None;
            return;
        }
        let mut stack = self
            .sim
            .units()
            .iter()
            .rev()
            .filter(|unit| unit.owner == HUMAN && (unit.x, unit.y) == (x, y))
            .collect::<Vec<_>>();
        stack.sort_by_key(|unit| unit.transport.is_some());
        let current = stack
            .iter()
            .position(|unit| Some(unit.id) == self.selected_unit);
        let next = current
            .and_then(|index| stack.get(index + 1))
            .or_else(|| stack.first());
        if let Some(unit) = next {
            self.action_page = 0;
            self.selected_unit = Some(unit.id);
            self.selected_city = None;
        }
    }
}
