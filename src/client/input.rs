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
            UiAction::CityTiles => self.city_tiles = !self.city_tiles,
            UiAction::WorkCityTile(city, x, y) => {
                self.sim.apply(Command::WorkCityTile(city, x, y));
            }
            UiAction::Specialist(city, index, delta) => {
                self.sim.apply(Command::Specialist(city, index, delta));
            }
            UiAction::Disband(city) => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::Disband(id, city))
                {
                    self.select_next_unit();
                }
            }
            UiAction::UpgradeUnit(target, price) => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::UpgradeUnit(id, target, price));
                }
            }
            UiAction::DeliverGoods(city) | UiAction::HelpWonder(city) => {
                if let Some(id) = self.selected_unit {
                    let command = if matches!(action, UiAction::DeliverGoods(_)) {
                        Command::DeliverGoods(id, city)
                    } else {
                        Command::HelpWonder(id, city)
                    };
                    if self.sim.apply(command) {
                        self.select_next_unit();
                    }
                }
            }
            UiAction::BeginParadrop => {
                self.paradrop_mode = self
                    .selected_unit_index()
                    .is_some_and(|index| self.sim.can_begin_paradrop(&self.sim.units()[index]));
                self.goto_mode = false;
            }
            UiAction::ParadropTo(x, y) => {
                if self.paradrop_mode
                    && let Some(id) = self.selected_unit
                {
                    self.move_selected(Command::Paradrop(id, x, y), id);
                }
                self.paradrop_mode = false;
            }
            UiAction::Airlift(city) => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::Airlift(id, city))
                    && let Some(unit) = self.sim.units().iter().find(|unit| unit.id == id)
                {
                    self.focus_camera_on(unit.x, unit.y);
                }
            }
            UiAction::JoinCity(city) => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::JoinCity(id, city))
                {
                    self.select_next_unit();
                }
            }
            UiAction::Detonate => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::Detonate(id))
                {
                    self.select_next_unit();
                }
            }
            UiAction::BoardOrDeboard => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::BoardOrDeboard(id));
                }
            }
            UiAction::HomeCity => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::HomeCity(id));
                }
            }
            UiAction::LaunchSpaceship => {
                if self.sim.apply(Command::LaunchSpaceship) {
                    self.spaceship_open = false;
                }
            }
            UiAction::ToggleSpaceship => self.spaceship_open = !self.spaceship_open,
            UiAction::ToggleManualAssembly => {
                self.sim.apply(Command::ToggleManualAssembly);
            }
            UiAction::PlaceShipPart(part) => {
                self.sim.apply(Command::PlaceShipPart(part));
            }
            UiAction::EndTurn => {
                if self.sim.apply(Command::EndTurn)
                    && self.autosave_enabled
                    && let Err(error) = self
                        .sim
                        .save_campaign(std::path::Path::new(crate::AUTOSAVE_PATH))
                {
                    self.sim.add_event(format!("Autosave failed. {error}"));
                }
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
                    self.action_page = 0;
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
                if let Some(index) = self.selected_unit_index() {
                    let unit = &self.sim.units()[index];
                    let kind = if self.sim.tile(unit.x, unit.y).irrigation {
                        WorkKind::Farmland
                    } else {
                        WorkKind::Irrigation
                    };
                    self.sim.apply(Command::Work(unit.id, kind));
                }
            }
            UiAction::TerrainWork(kind) => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::Work(id, kind));
                }
            }
            UiAction::Mine => {
                if let Some(index) = self.selected_unit_index() {
                    let unit = &self.sim.units()[index];
                    let kind = self.sim.oil_work(unit).unwrap_or(WorkKind::Mine);
                    self.sim.apply(Command::Work(unit.id, kind));
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
            UiAction::RushProduction(id) => {
                self.sim.apply(Command::RushProduction(id));
            }
            UiAction::SellBuilding(id, building) => {
                self.sim.apply(Command::SellBuilding(id, building));
            }
            UiAction::Produce(kind) => {
                if let Some(id) = self.selected_city {
                    self.sim.apply(Command::Produce(id, kind));
                }
            }
            UiAction::ActionPage(page) => self.action_page = page,
            UiAction::ProductionPage(page) => self.production_page = page,
            UiAction::UnitProductionPage(page) => self.unit_production_page = page,
            UiAction::WorklistPage(page) => self.worklist_page = page,
            UiAction::EditWorklist(id, edit) => {
                self.sim.apply(Command::EditWorklist(id, edit));
            }
            UiAction::ToggleDiplomacy => {
                self.diplomacy_open = !self.diplomacy_open;
                self.diplomacy_page = 0;
                self.city_deals = None;
                self.reviewed_offer = None;
            }
            UiAction::DiplomacyPage(page) => self.diplomacy_page = page,
            UiAction::SpyTargets(targets) => self.spy_targets = targets,
            UiAction::TargetedTheft(..)
            | UiAction::TargetedSabotage(..)
            | UiAction::StealTechnology(..)
            | UiAction::SabotageUnit(..)
            | UiAction::PoisonCity(..)
            | UiAction::InciteCity(..)
            | UiAction::SabotageCity(..) => {
                if let Some(id) = self.selected_unit {
                    let command = match action {
                        UiAction::SabotageUnit(target) => Command::SabotageUnit(id, target),
                        UiAction::InciteCity(city, price) => Command::InciteCity(id, city, price),
                        UiAction::PoisonCity(city) => Command::PoisonCity(id, city),
                        UiAction::SabotageCity(city) => Command::SabotageCity(id, city),
                        UiAction::StealTechnology(city) => Command::StealTechnology(id, city),
                        UiAction::TargetedTheft(city, tech) => {
                            Command::TargetedTheft(id, city, tech)
                        }
                        UiAction::TargetedSabotage(city, target) => {
                            Command::TargetedSabotage(id, city, target)
                        }
                        _ => unreachable!(),
                    };
                    if self.sim.apply(command) {
                        self.spy_targets = None;
                        if self.sim.units().iter().all(|unit| unit.id != id) {
                            self.select_next_unit();
                        }
                    }
                }
            }
            UiAction::BribeUnit(target, price) => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::BribeUnit(id, target, price));
                }
            }
            UiAction::InvestigateCity(city) => {
                if let Some(id) = self.selected_unit
                    && let Some(report) = self.sim.investigate_city(id, city)
                {
                    self.investigation = Some(report);
                    self.investigation_page = 0;
                }
            }
            UiAction::CloseInvestigation => self.investigation = None,
            UiAction::InvestigationPage(page) => self.investigation_page = page,
            UiAction::EstablishEmbassy(city) => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::EstablishEmbassy(id, city))
                    && self.sim.units().iter().all(|unit| unit.id != id)
                {
                    self.select_next_unit();
                }
            }
            UiAction::CityDeals(selection) => self.city_deals = selection,
            UiAction::BuyCity(city, price) => {
                self.sim.apply(Command::BuyCity(city, price));
            }
            UiAction::BuyMap(owner, map) => {
                self.sim.apply(Command::BuyMap(owner, map));
            }
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
        if is_key_pressed(KeyCode::F5) {
            let message = match self
                .sim
                .save_campaign(std::path::Path::new(crate::QUICKSAVE_PATH))
            {
                Ok(()) => "Campaign saved.".to_owned(),
                Err(error) => format!("Save failed. {error}"),
            };
            self.sim.add_event(message);
            return;
        }
        if is_key_pressed(KeyCode::F9) {
            match Simulation::load_campaign(std::path::Path::new(crate::QUICKSAVE_PATH)) {
                Ok(simulation) => {
                    let autosave_enabled = self.autosave_enabled;
                    *self = Self::from_simulation(simulation);
                    self.autosave_enabled = autosave_enabled;
                    self.sim.add_event("Campaign loaded.");
                }
                Err(error) => self.sim.add_event(format!("Load failed. {error}")),
            }
            return;
        }
        if self.spy_targets.is_some() {
            if is_key_pressed(KeyCode::Escape) {
                self.spy_targets = None;
            }
            return;
        }
        if self.investigation.is_some() {
            if is_key_pressed(KeyCode::Escape) {
                self.investigation = None;
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
        let shift_down = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        if is_key_pressed(KeyCode::Escape) {
            if self.selected_city.is_some() {
                self.selected_city = None;
            } else {
                self.goto_mode = false;
                self.paradrop_mode = false;
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
        for (key, kind) in [
            (KeyCode::V, WorkKind::Cultivate),
            (KeyCode::P, WorkKind::Plant),
            (KeyCode::T, WorkKind::Transform),
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

    pub(super) fn minimap_rect(&self) -> Rect {
        let top = TOP_H + 58.0;
        let available_width = PANEL_W - 36.0;
        // Leave room for the context text and the two bottom buttons.
        let space_row = if self.sim.spaceship_status(HUMAN).is_some() {
            34.0
        } else {
            0.0
        };
        let context_height = if screen_height() < 700.0 {
            168.0
        } else {
            214.0
        };
        let available_height =
            (screen_height() - top - 17.0 - context_height - 160.0 - space_row).max(1.0);
        let natural_size = vec2(
            (2 * self.sim.map_wrap.width()) as f32,
            self.sim.map_wrap.height() as f32,
        );
        let scale = (available_width / natural_size.x).min(available_height / natural_size.y);
        let size = natural_size * scale;
        let left = screen_width() - PANEL_W + (PANEL_W - size.x) * 0.5;
        Rect::new(left, top, size.x, size.y)
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
        if self.spaceship_open
            || self.government_open
            || self.diplomacy_open
            || self.investigation.is_some()
            || self.spy_targets.is_some()
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
