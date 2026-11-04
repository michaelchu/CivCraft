use super::*;

impl Game {
    pub(crate) fn perform_action(&mut self, action: UiAction) {
        if self.sim.game_over().is_some() && !matches!(action, UiAction::EndTurn) {
            return;
        }
        match action {
            UiAction::RaiseRate(index) => {
                self.sim.apply(Command::RaiseRate(index));
            }
            UiAction::Specialist(city, index, delta) => {
                self.sim.apply(Command::Specialist(city, index, delta));
            }
            UiAction::HomeCity => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::HomeCity(id));
                }
            }
            UiAction::EndTurn => {
                self.sim.apply(Command::EndTurn);
                self.select_next_unit();
            }
            UiAction::CloseCity => {
                self.selected_city = None;
            }
            UiAction::SelectUnit(id) => {
                if self
                    .sim
                    .units()
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
                if let Some(index) = self.selected_unit_index() {
                    let unit = &self.sim.units()[index];
                    let id = unit.id;
                    let kind = if self.sim.tile(unit.x, unit.y).road {
                        WorkKind::Railroad
                    } else {
                        WorkKind::Road
                    };
                    self.sim.apply(Command::Work(id, kind));
                }
            }
            UiAction::Irrigate => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::Work(id, WorkKind::Irrigation));
                }
            }
            UiAction::Mine => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::Work(id, WorkKind::Mine));
                }
            }
            UiAction::Fortify => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::Fortify(id));
                }
            }
            UiAction::Wait => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::Wait(id));
                    self.select_next_unit();
                }
            }
            UiAction::Automate => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::Automate(id));
                }
            }
            UiAction::Produce(kind) => {
                if let Some(id) = self.selected_city {
                    self.sim.apply(Command::Produce(id, kind));
                }
            }
            UiAction::ProductionPage(page) => self.production_page = page,
            UiAction::ToggleDiplomacy => {
                self.diplomacy_open = !self.diplomacy_open;
                self.diplomacy_page = 0;
                self.reviewed_offer = None;
            }
            UiAction::DiplomacyPage(page) => self.diplomacy_page = page,
            UiAction::Negotiate(other, pact) => {
                self.sim.apply(Command::Negotiate(other, pact));
            }
            UiAction::CancelPact(other) => {
                self.sim.apply(Command::CancelPact(other));
            }
            UiAction::ReviewOffer(other, offer) => self.reviewed_offer = Some((other, offer)),
            UiAction::CloseOffer => self.reviewed_offer = None,
            UiAction::AcceptOffer(other, offer) => {
                self.sim.apply(Command::AcceptOffer(other, offer));
                self.reviewed_offer = None;
            }
            UiAction::DeclineOffer(other) => {
                self.reviewed_offer = None;
                self.sim.apply(Command::DeclineOffer(other));
            }
            UiAction::ToggleGovernment => self.government_open = !self.government_open,
            UiAction::ChangeGovernment(target) => {
                self.sim.apply(Command::Government(target));
            }
            UiAction::OpenResearch => {
                self.research_open = true;
                self.research_drag = None;
                self.dragging = false;
                self.center_research();
            }
            UiAction::CloseResearch => self.research_open = false,
            UiAction::CenterResearch => self.center_research(),
            UiAction::FitResearch => self.fit_research(),
            UiAction::ResearchAge(age) => {
                self.research_age = age;
                self.research_inspected = None;
                self.fit_research();
            }
            UiAction::InspectResearch(tech) => {
                self.research_inspected = Some(tech);
                self.focus_research(tech);
            }
            UiAction::ResearchGoal(goal) => {
                self.research_inspected = Some(goal);
                self.sim.apply(Command::Research(goal));
            }
        }
    }

    pub(crate) fn handle_keyboard(&mut self) {
        if self.government_open || self.diplomacy_open {
            if is_key_pressed(KeyCode::Escape) {
                if self.reviewed_offer.take().is_some() {
                    return;
                }
                self.government_open = false;
                self.diplomacy_open = false;
            }
            return;
        }
        if self.research_open {
            if is_key_pressed(KeyCode::Escape) {
                self.research_open = false;
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
        if is_key_pressed(KeyCode::H) {
            self.perform_action(UiAction::HomeCity);
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
            self.sim.add_event(if self.goto_mode {
                "Go-to active: choose a destination tile."
            } else {
                "Go-to cancelled."
            });
        }
        if is_key_pressed(KeyCode::S) {
            if let Some(index) = self.selected_unit_index() {
                let name = self.sim.units()[index].kind.name();
                self.sim.apply(Command::Sentry(self.sim.units()[index].id));
                self.sim.add_event(format!("{name} is now on sentry duty."));
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
                self.camera += vec2(dx + dy, dy - dx) * 0.5;
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
            if self.move_selected(Command::Move(id, dx, dy), id) {
                self.select_next_if_exhausted(id);
            }
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
            (screen_width() - PANEL_W).max(320.0),
            (screen_height() - TOP_H - BOTTOM_H).max(220.0),
        )
    }

    pub(super) fn minimap_rect() -> Rect {
        let top = TOP_H + 38.0;
        let available_width = PANEL_W - 36.0;
        // Leave room for the context text and the two bottom buttons.
        let available_height = (screen_height() - top - 17.0 - 207.0 - 122.0).max(1.0);
        let natural_size = vec2((2 * MAP_W) as f32, MAP_H as f32);
        let scale = (available_width / natural_size.x).min(available_height / natural_size.y);
        let size = natural_size * scale;
        let left = screen_width() - PANEL_W + (PANEL_W - size.x) * 0.5;
        Rect::new(left, top, size.x, size.y)
    }

    pub(super) fn minimap_tile_origin(rect: Rect, x: i32, y: i32) -> Vec2 {
        let scale = rect.h / MAP_H as f32;
        vec2(
            rect.x + (2 * x + y.rem_euclid(2)) as f32 * scale,
            rect.y + y as f32 * scale,
        )
    }

    pub(super) fn city_manager_rect() -> Rect {
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

    pub(super) fn focus_camera_on(&mut self, native_x: i32, native_y: i32) {
        let (map_x, map_y) = native_to_map(native_x, native_y);
        self.camera = vec2(map_x as f32, map_y as f32);
    }

    pub(super) fn camera_overview_position(&self) -> Vec2 {
        vec2(
            (self.camera.x - self.camera.y + MAP_W as f32).rem_euclid(2.0 * MAP_W as f32),
            self.camera.x + self.camera.y - MAP_W as f32,
        )
    }

    fn normalize_camera(&mut self) {
        let position = self.camera_overview_position();
        let y = position.y.clamp(0.0, (MAP_H - 1) as f32);
        let map_x = (position.x + y) * 0.5;
        self.camera = vec2(map_x, y + MAP_W as f32 - map_x);
    }

    pub(super) fn world_to_screen(&self, x: i32, y: i32) -> Vec2 {
        let viewport = Self::viewport_rect();
        let camera = self.camera_overview_position();
        let tile_x = (2 * x + y.rem_euclid(2)) as f32;
        let half_width = MAP_W as f32;
        let dx = (tile_x - camera.x + half_width).rem_euclid(2.0 * half_width) - half_width;
        let dy = y as f32 - camera.y;
        vec2(
            viewport.x + viewport.w * 0.5 + dx * TILE_W * 0.5 * self.zoom,
            viewport.y + viewport.h * 0.47 + dy * TILE_H * 0.5 * self.zoom,
        )
    }

    fn screen_to_world(&self, point: Vec2) -> Option<(i32, i32)> {
        let viewport = Self::viewport_rect();
        let sx = point.x - (viewport.x + viewport.w * 0.5);
        let sy = point.y - (viewport.y + viewport.h * 0.47);
        let a = sx / (TILE_W * 0.5 * self.zoom);
        let b = sy / (TILE_H * 0.5 * self.zoom);
        let dx = (a + b) * 0.5;
        let dy = (b - a) * 0.5;
        let (x, y) = map_to_native(
            (self.camera.x + dx).round() as i32,
            (self.camera.y + dy).round() as i32,
        );
        (0..MAP_H).contains(&y).then_some((x.rem_euclid(MAP_W), y))
    }

    pub(crate) fn handle_mouse(&mut self, buttons: &[(Rect, UiAction)]) {
        if self.government_open || self.diplomacy_open {
            let (x, y) = mouse_position();
            if is_mouse_button_pressed(MouseButton::Left)
                && let Some((_, action)) =
                    buttons.iter().find(|(rect, _)| rect.contains(vec2(x, y)))
            {
                self.perform_action(*action);
            }
            return;
        }
        if self.research_open {
            self.handle_research_mouse(buttons);
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
            let scale = minimap.h / MAP_H as f32;
            let natural_x = ((mouse.x - minimap.x) / scale).floor() as i32;
            let native_y = (((mouse.y - minimap.y) / scale).floor() as i32).min(MAP_H - 1);
            let native_x = (natural_x - native_y.rem_euclid(2)).div_euclid(2);
            self.focus_camera_on(native_x.rem_euclid(MAP_W), native_y);
            return;
        }
        if !Self::viewport_rect().contains(mouse) {
            return;
        }
        let Some((x, y)) = self.screen_to_world(mouse) else {
            return;
        };
        if self.goto_mode {
            if let Some(id) = self.selected_unit {
                self.move_selected(Command::GoTo(id, x, y), id);
            }
            self.goto_mode = false;
            return;
        }
        self.select_at(x, y);
    }

    pub(super) fn select_at(&mut self, x: i32, y: i32) {
        // Cities take click priority over their garrisons. Units stationed in
        // a city remain selectable from the city manager.
        if let Some(city) = self
            .sim
            .cities()
            .iter()
            .find(|city| city.owner == HUMAN && city.x == x && city.y == y)
        {
            self.production_page = 0;
            self.selected_city = Some(city.id);
            self.selected_unit = None;
            return;
        }
        if let Some(unit) = self
            .sim
            .units()
            .iter()
            .rev()
            .find(|u| u.owner == HUMAN && u.x == x && u.y == y)
        {
            self.selected_unit = Some(unit.id);
            self.selected_city = None;
        }
    }
}
