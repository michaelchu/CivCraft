use super::*;

impl Game {
    pub(super) fn draw_hud(&mut self, assets: &Assets) -> Vec<(Rect, UiAction)> {
        let mut actions = Vec::new();
        self.draw_minimap();
        self.hud_elements_drawn += 1;

        let context = self.context_rect();
        draw_hud_card(context);
        let end = Rect::new(
            context.x + 10.0,
            context.bottom() - 38.0,
            context.w - 20.0,
            28.0,
        );
        self.draw_context_panel(assets, context, end.y);
        draw_ui_button(end, "", true, true);
        let shortcut = "Shift+Enter";
        let shortcut_width = measure_text(shortcut, None, 11, 1.0).width;
        draw_text("End Turn", end.x + 8.0, end.y + 18.0, 13.0, WHITE);
        draw_text(
            shortcut,
            end.right() - 8.0 - shortcut_width,
            end.y + 18.0,
            11.0,
            Color::from_rgba(241, 223, 189, 255),
        );
        actions.push((end, UiAction::EndTurn));
        actions.push((context, UiAction::BlockMapClick));
        self.hud_elements_drawn += 1;

        let action_width = screen_width() - 2.0 * (Self::corner_width() + 2.0 * HUD_MARGIN);
        self.draw_action_strip(&mut actions, action_width);
        let portraits_left = self.draw_leader_portraits(assets, &mut actions);
        self.draw_messages(&mut actions, portraits_left - 2.0 * HUD_MARGIN);
        self.draw_top_bar(&mut actions);
        self.hud_elements_drawn += 1;
        self.hud_elements_drawn += 1;
        actions
    }

    fn draw_top_bar(&self, actions: &mut Vec<(Rect, UiAction)>) {
        let width = screen_width();
        draw_rectangle(0.0, 0.0, width, TOP_H, Color::from_rgba(29, 31, 28, 255));
        draw_rectangle(
            0.0,
            TOP_H - 3.0,
            width,
            3.0,
            Color::from_rgba(171, 133, 55, 255),
        );

        let player = &self.sim.players()[HUMAN];
        let status = self.sim.spaceship_status(HUMAN);
        let button_count = 6 + usize::from(status.is_some());
        let button_width = 38.0;
        let step = button_width + 4.0;
        let button_start = width - HUD_MARGIN - button_count as f32 * step + 4.0;
        let research = player.researching;
        let displays = [
            (
                format!("Turn {} · {}", self.sim.turn(), year_label(self.sim.year())),
                WHITE,
            ),
            (format!("Gold {}", player.gold), GOLD),
            (player.government.name().to_owned(), LIGHTGRAY),
            (
                format!(
                    "{} {}/{}",
                    research.name(),
                    player.research,
                    self.sim.target_cost(HUMAN, research)
                ),
                Color::from_rgba(151, 210, 231, 255),
            ),
        ];
        let mut x = HUD_MARGIN;
        if width >= 950.0 {
            text_in("CivCraft", x, 29.0, 24.0, 100.0, GOLD);
            x += 112.0;
        }
        let available = (button_start - x - HUD_MARGIN).max(0.0);
        let measured = displays
            .each_ref()
            .map(|(label, _)| measure_text(label, None, 16, 1.0).width);
        let scale = ((available - 36.0) / measured.iter().sum::<f32>()).clamp(0.0, 1.0);
        for ((label, color), measured) in displays.iter().zip(measured) {
            let slot = (measured * scale).ceil();
            text_in(label, x, 27.0, 16.0, slot, *color);
            x += slot + 11.0;
        }

        let in_anarchy = player.government == Government::Anarchy;
        let rates = if in_anarchy {
            [0, 0, 100]
        } else {
            player.rates
        };
        let mut controls = vec![
            (
                HudIcon::Government,
                UiAction::ToggleGovernment,
                "Government".to_owned(),
                None,
                true,
                false,
            ),
            (
                HudIcon::Diplomacy,
                UiAction::ToggleDiplomacy,
                "Diplomacy".to_owned(),
                None,
                true,
                false,
            ),
            (
                HudIcon::Research,
                UiAction::OpenResearch,
                "Research tree".to_owned(),
                None,
                true,
                false,
            ),
        ];
        for (index, (icon, label)) in [
            (HudIcon::Science, "Science"),
            (HudIcon::Tax, "Tax"),
            (HudIcon::Luxury, "Luxury"),
        ]
        .into_iter()
        .enumerate()
        {
            let rate = rates[index];
            let enabled = !in_anarchy && rate < player.government.max_rate();
            controls.push((
                icon,
                UiAction::RaiseRate(index),
                format!("{label} {rate}%"),
                Some(rate),
                enabled,
                false,
            ));
        }
        if let Some(status) = status {
            controls.push((
                HudIcon::Spaceship,
                UiAction::ToggleSpaceship,
                status,
                None,
                true,
                self.sim.can_launch_spaceship(HUMAN),
            ));
        }

        let mut tooltip = None;
        for (index, (icon, action, label, rate, enabled, accent)) in
            controls.into_iter().enumerate()
        {
            let rect = Rect::new(button_start + index as f32 * step, 5.0, button_width, 33.0);
            draw_ui_button(rect, "", enabled, accent);
            let color = if enabled {
                Color::from_rgba(241, 238, 218, 255)
            } else {
                GRAY
            };
            let center = vec2(
                rect.x + rect.w * 0.5,
                rect.y + if rate.is_some() { 11.0 } else { 16.5 },
            );
            draw_hud_icon(
                icon,
                center,
                if rate.is_some() { 15.0 } else { 21.0 },
                color,
            );
            if let Some(rate) = rate {
                let label = format!("{rate}%");
                let w = measure_text(&label, None, 12, 1.0).width;
                draw_text(&label, center.x - w * 0.5, rect.bottom() - 4.0, 12.0, color);
            }
            if enabled {
                actions.push((rect, action));
            }
            if rect.contains(ui_pointer()) {
                tooltip = Some((rect, label));
            }
        }
        if let Some((button, label)) = tooltip {
            let width = measure_text(&label, None, 15, 1.0).width + 16.0;
            let rect = Rect::new(
                (button.right() - width).max(HUD_MARGIN),
                TOP_H + 3.0,
                width,
                24.0,
            );
            draw_hud_card(rect);
            draw_text(&label, rect.x + 8.0, rect.y + 17.0, 15.0, WHITE);
        }
    }

    fn draw_leader_portraits(&self, assets: &Assets, actions: &mut Vec<(Rect, UiAction)>) -> f32 {
        let portraits: Vec<_> = self
            .sim
            .contacted_civilizations(HUMAN)
            .into_iter()
            .filter_map(|owner| Some((owner, assets.leader_avatar_badges.get(owner)?)))
            .collect();
        let size = 56.0;
        let step = size + 5.0;
        let left = screen_width() - HUD_MARGIN - portraits.len() as f32 * step;
        for (index, (owner, badge)) in portraits.into_iter().enumerate() {
            let rect = Rect::new(left + 5.0 + index as f32 * step, TOP_H + 10.0, size, size);
            draw_texture_ex(
                badge,
                rect.x,
                rect.y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(size, size)),
                    ..Default::default()
                },
            );
            actions.push((rect, UiAction::OpenDiplomacyWith(owner)));
        }
        left
    }

    pub(super) fn draw_intro(&self) -> Vec<(Rect, UiAction)> {
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::from_rgba(4, 9, 12, 178),
        );
        let width = (screen_width() - 2.0 * HUD_MARGIN).min(470.0);
        let height = 254.0;
        let panel = Rect::new(
            (screen_width() - width) * 0.5,
            (screen_height() - height) * 0.5,
            width,
            height,
        );
        panel_frame(panel);
        text_in(
            "Your Chronicle",
            panel.x + 22.0,
            panel.y + 41.0,
            27.0,
            width - 44.0,
            GOLD,
        );
        for (index, line) in [
            "Found a city with your Settlers.",
            "Explore the fog and improve the land.",
            "Research stronger units.",
            "Unite the rival nations.",
            "Click a unit or city for details.",
        ]
        .into_iter()
        .enumerate()
        {
            text_in(
                line,
                panel.x + 22.0,
                panel.y + 75.0 + index as f32 * 25.0,
                17.0,
                width - 44.0,
                WHITE,
            );
        }
        let button = Rect::new(panel.right() - 172.0, panel.bottom() - 52.0, 150.0, 34.0);
        draw_ui_button(button, "Begin [Enter]", true, true);
        vec![(button, UiAction::DismissIntro)]
    }

    fn draw_minimap(&self) {
        let rect = self.minimap_rect();
        let scale = rect.h / self.sim.map_wrap.height() as f32;
        for y in 0..self.sim.map_wrap.height() {
            for x in 0..self.sim.map_wrap.width() {
                let tile = self.sim.render_tile(x, y);
                let color = if !tile.seen {
                    Color::from_rgba(7, 13, 17, 255)
                } else if tile.visible {
                    tile.terrain.minimap_color()
                } else {
                    darken(tile.terrain.minimap_color(), 0.53)
                };
                let origin = self.minimap_tile_origin(rect, x, y);
                let width = (rect.right() - origin.x).min(2.0 * scale);
                draw_rectangle(origin.x, origin.y, width, scale, color);
                if y % 2 == 1 && x == self.sim.map_wrap.width() - 1 {
                    draw_rectangle(rect.x, origin.y, scale, scale, color);
                }
            }
        }
        let city_overrides = self.playback.city_overrides();
        let observed = self.sim.render_city_markers();
        for city in observed
            .into_iter()
            .filter(|c| !city_overrides.contains_key(&c.id))
            .chain(city_overrides.values())
        {
            let origin = self.minimap_tile_origin(rect, city.x, city.y);
            let marker = Rect::new(origin.x, origin.y, 2.0 * scale, scale);
            let color = player_color(city.owner);
            if let Some(part) = marker.intersect(rect) {
                draw_rectangle(part.x, part.y, part.w, part.h, color);
            }
            if marker.right() > rect.right() {
                draw_rectangle(
                    rect.x,
                    marker.y,
                    marker.right() - rect.right(),
                    marker.h,
                    color,
                );
            }
        }

        let viewport = Self::viewport_rect();
        let camera =
            vec2(rect.x, rect.y) + (self.camera_overview_position() + vec2(1.0, 0.5)) * scale;
        let width = viewport.w * scale / (TILE_W * 0.5 * self.zoom);
        let height = viewport.h * scale / (TILE_H * 0.5 * self.zoom);
        let shifts_y = if self.sim.map_wrap.wraps_y() {
            vec![-rect.h, 0.0, rect.h]
        } else {
            vec![0.0]
        };
        for &wrap_y in &shifts_y {
            for wrap_x in [-rect.w, 0.0, rect.w] {
                let left = camera.x + wrap_x - width * 0.5;
                let top = camera.y + wrap_y - height * 0.5;
                let right = left + width;
                let bottom = top + height;
                for y in [top, bottom] {
                    if (rect.y..=rect.bottom()).contains(&y)
                        && left < rect.right()
                        && right > rect.x
                    {
                        draw_line(left.max(rect.x), y, right.min(rect.right()), y, 1.0, WHITE);
                    }
                }
                for x in [left, right] {
                    if (rect.x..=rect.right()).contains(&x)
                        && top < rect.bottom()
                        && bottom > rect.y
                    {
                        draw_line(x, top.max(rect.y), x, bottom.min(rect.bottom()), 1.0, WHITE);
                    }
                }
            }
        }
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            2.0,
            Color::from_rgba(160, 126, 61, 255),
        );
    }

    pub(super) fn draw_city_tiles(
        &self,
        assets: &Assets,
        city: &City,
        panel: Rect,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        self.draw_city_tiles_map(assets, city, panel, actions);
    }

    pub(super) fn draw_worklist(
        &self,
        city: &City,
        panel: Rect,
        requested_page: usize,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        let rows = ((panel.h - 208.0) / 34.0).floor().max(1.0) as usize;
        let last_page = city.worklist.len().saturating_sub(1) / rows;
        let page = requested_page.min(last_page);
        draw_text(
            &format!("WORKLIST  {}/{}", city.worklist.len(), WORKLIST_LIMIT),
            panel.x + 22.0,
            panel.y + 187.0,
            18.0,
            WHITE,
        );
        for (offset, width, label, target, enabled) in [
            (185.0, 28.0, "<", Some(page.saturating_sub(1)), page > 0),
            (150.0, 28.0, ">", Some(page + 1), page < last_page),
            (114.0, 92.0, "Production", None, true),
        ] {
            let button = Rect::new(panel.x + panel.w - offset, panel.y + 169.0, width, 25.0);
            draw_ui_button(button, label, enabled, false);
            if enabled {
                actions.push((button, UiAction::WorklistPage(target)));
            }
        }
        if city.worklist.is_empty() {
            draw_text(
                "Use + on a production card to add it to the worklist.",
                panel.x + 22.0,
                panel.y + 224.0,
                16.0,
                LIGHTGRAY,
            );
        }
        for (index, &entry) in city
            .worklist
            .iter()
            .enumerate()
            .skip(page * rows)
            .take(rows)
        {
            let y = panel.y + 201.0 + (index % rows) as f32 * 34.0;
            let target = self.sim.worklist_replacement(city, entry);
            let status = if !self.sim.can_produce(city, target) {
                "  (unavailable)".to_owned()
            } else if target != entry {
                format!("  (upgrades to {})", target.name())
            } else {
                String::new()
            };
            text_in(
                &format!("{}. {}{status}", index + 1, entry.name()),
                panel.x + 22.0,
                y + 19.0,
                16.0,
                panel.w - 158.0,
                WHITE,
            );
            for (offset, label, edit, enabled) in [
                (
                    122.0,
                    "^",
                    WorklistEdit::Move(index, index.saturating_sub(1)),
                    index > 0,
                ),
                (
                    87.0,
                    "v",
                    WorklistEdit::Move(index, index + 1),
                    index + 1 < city.worklist.len(),
                ),
                (48.0, "X", WorklistEdit::Remove(index), true),
            ] {
                let button = Rect::new(panel.x + panel.w - offset, y, 32.0, 24.0);
                draw_ui_button(button, label, enabled, false);
                if enabled {
                    actions.push((button, UiAction::EditWorklist(city.id, edit)));
                }
            }
        }
    }

    pub(super) fn draw_queue_button(
        &self,
        city: &City,
        card: Rect,
        target: Production,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        if city.worklist.len() < WORKLIST_LIMIT && self.sim.can_produce(city, target) {
            let button = Rect::new(card.x + card.w - 29.0, card.y + card.h - 23.0, 22.0, 18.0);
            draw_ui_button(button, "+", true, false);
            actions.push((
                button,
                UiAction::EditWorklist(city.id, WorklistEdit::Append(target)),
            ));
        }
    }

    fn draw_context_panel(&self, assets: &Assets, card: Rect, button_y: f32) {
        let x = card.x + 10.0;
        let y = card.y + 4.0;
        let spacing = ((button_y - y - 6.0) / 156.0).clamp(0.0, 1.0);
        let context_text = |text: &str, tx: f32, ty: f32, size: f32, color: Color| {
            text_in(
                text,
                tx,
                y + (ty - y) * spacing,
                size.min(17.0),
                card.w - 20.0,
                color,
            );
        };
        if let Some(index) = self.selected_unit_index() {
            let unit = &self.sim.units()[index];
            let animated = self
                .playback
                .active()
                .into_iter()
                .map(|(unit, _)| unit)
                .find(|shown| shown.id == unit.id);
            let unit = animated.as_ref().unwrap_or(unit);
            let stats = unit.kind.stats();
            let state = if unit.work_left > 0 {
                format!(
                    "Building {} - {} turns",
                    unit.work_kind.map(|w| w.name()).unwrap_or("improvement"),
                    self.sim.project_turns_left(unit)
                )
            } else if let Some(carrier) = unit.transport {
                self.sim
                    .units()
                    .iter()
                    .find(|ship| ship.id == carrier)
                    .map_or_else(
                        || "Aboard transport".to_owned(),
                        |ship| format!("Aboard {}", ship.kind.name()),
                    )
            } else if unit.kind.transport_capacity() > 0 {
                format!(
                    "Cargo {}/{}",
                    self.sim.cargo_count(unit.id),
                    unit.kind.transport_capacity()
                )
            } else if unit.automated {
                "Automated".to_owned()
            } else {
                match unit.state {
                    UnitState::Idle => "Ready".to_owned(),
                    UnitState::Fortifying => "Fortifying".to_owned(),
                    UnitState::Fortified => "Fortified (+50% defense)".to_owned(),
                    UnitState::Sentry => "Sentry".to_owned(),
                }
            };
            let (sprite_row, sprite_col) = unit_sprite(unit.kind);
            draw_texture_ex(
                &assets.units,
                x,
                card.y + 8.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(36.0, 27.0)),
                    source: Some(isolated_sprite_source(sprite_row, sprite_col, 64.0, 48.0)),
                    ..Default::default()
                },
            );

            let width = card.w - 20.0;
            let right = x + width;
            let identity_x = x + 41.0;
            let muted = Color::from_rgba(181, 190, 174, 255);
            let status_color = Color::from_rgba(188, 219, 175, 255);
            let state = if unit.kind.is_aircraft() {
                format!("{state} · Fuel {}/{}", unit.fuel, unit.kind.fuel_capacity())
            } else {
                state
            };
            let state = if unit.veteran > 0 {
                format!("{state} · {}", unit.rank_name())
            } else {
                state
            };
            let state_width = measure_text(&state, None, 12, 1.0).width;
            let name_width = measure_text(unit.kind.name(), None, 14, 1.0).width;
            let inline_status = identity_x + name_width + state_width + 8.0 <= right;
            let title_y = card.y + 20.0;
            text_in(
                unit.kind.name(),
                identity_x,
                title_y,
                14.0,
                right - identity_x,
                Color::from_rgba(244, 221, 147, 255),
            );
            text_in(
                &format!(
                    "{} · {}",
                    self.sim.players()[unit.owner].name,
                    self.sim.tile(unit.x, unit.y).terrain.name(),
                ),
                identity_x,
                title_y + 14.0,
                12.0,
                right - identity_x,
                muted,
            );
            if inline_status {
                draw_text(&state, right - state_width, title_y, 12.0, status_color);
            } else {
                text_in(&state, x, title_y + 29.0, 12.0, width, status_color);
            }

            let gap = 4.0;
            let cell_width = (width - gap) * 0.5;
            let moves = self.sim.unit_move_allowance(unit.owner, unit.kind);
            let metrics = [
                ("Health", format!("{}/{}", unit.hp, stats.hp)),
                ("Moves", format!("{}/{moves}", unit.moves_left)),
                ("Attack", stats.attack.to_string()),
                ("Defense", stats.defense.to_string()),
            ];
            for (index, (label, value)) in metrics.iter().enumerate() {
                let cell_x = x + (index % 2) as f32 * (cell_width + gap);
                let grid_y = if inline_status {
                    card.y + 44.0
                } else {
                    card.y + 56.0
                };
                let cell_y = grid_y + (index / 2) as f32 * 21.0;
                let cell = Rect::new(cell_x, cell_y, cell_width, 18.0);
                draw_rectangle(
                    cell.x,
                    cell.y,
                    cell.w,
                    cell.h,
                    Color::from_rgba(41, 48, 42, 255),
                );

                let value_size = measure_text(value, None, 12, 1.0);
                let label = fit_text(label, 12.0, cell.w - value_size.width - 13.0);
                let label_size = measure_text(&label, None, 12, 1.0);
                let baseline =
                    |size: TextDimensions| cell.y + (cell.h - size.height) * 0.5 + size.offset_y;
                draw_text(&label, cell.x + 5.0, baseline(label_size), 12.0, muted);
                draw_text(
                    value,
                    cell.right() - value_size.width - 5.0,
                    baseline(value_size),
                    12.0,
                    WHITE,
                );
            }
        } else if let Some(index) = self.selected_city_index() {
            let city = &self.sim.cities()[index];
            let forecast = self.sim.city_forecast(city);
            let (food, shields, trade) = (forecast.food, forecast.shields, forecast.trade);
            let foreign = city.size - city.domestic_citizens();
            let cost = city.production.cost();
            context_text(
                &city.name,
                x,
                y + 23.0,
                25.0,
                Color::from_rgba(244, 221, 147, 255),
            );
            context_text(
                &format!(
                    "City size {} ({foreign} foreign)  Plague {:.1}%",
                    city.size,
                    f64::from(self.sim.city_illness(city, shields)) / 10.0
                ),
                x,
                y + 48.0,
                18.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            context_text(
                &format!("Food {}  Shields {}  Trade {}", food, shields, trade),
                x,
                y + 76.0,
                17.0,
                Color::from_rgba(177, 213, 159, 255),
            );
            context_text(
                &format!(
                    "Food store {}/{}",
                    city.food,
                    Simulation::growth_box(city.size)
                ),
                x,
                y + 102.0,
                17.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            context_text(
                &format!("Producing: {}", city.production.name()),
                x,
                y + 132.0,
                18.0,
                Color::from_rgba(145, 204, 224, 255),
            );
            context_text(
                &format!("Production {}/{}", city.shields, cost),
                x,
                y + 156.0,
                17.0,
                Color::from_rgba(229, 226, 209, 255),
            );
        }
    }

    fn draw_action_strip(&self, actions: &mut Vec<(Rect, UiAction)>, available_width: f32) {
        let mut entries: Vec<(String, UiAction, bool)> = Vec::new();
        if let Some(index) = self.selected_unit_index() {
            let unit = &self.sim.units()[index];
            let stats = unit.kind.stats();
            if let Some((target, price)) = self.sim.paid_upgrade(unit) {
                entries.push((
                    format!("Upgrade {price}g"),
                    UiAction::UpgradeUnit(target, price),
                    self.sim.players()[unit.owner].gold >= price,
                ));
            }
            for city in self
                .sim
                .cities()
                .iter()
                .filter(|city| self.sim.can_establish_embassy(unit, city))
            {
                entries.push((
                    format!("Embassy: {}", city.name),
                    UiAction::EstablishEmbassy(city.id),
                    true,
                ));
            }
            for city in self
                .sim
                .cities()
                .iter()
                .filter(|city| self.sim.can_investigate_city(unit, city))
            {
                entries.push((
                    format!("Investigate {}", city.name),
                    UiAction::InvestigateCity(city.id),
                    true,
                ));
            }
            if unit.kind == UnitKind::Spy {
                for city in self.sim.cities().iter().filter(|city| {
                    self.sim.can_steal_technology(unit, city)
                        || self.sim.can_sabotage_city(unit, city)
                }) {
                    entries.push((
                        format!("Targets: {}", city.name),
                        UiAction::SpyTargets(Some((city.id, 0))),
                        true,
                    ));
                }
            }
            if unit.kind.is_diplomatic() {
                for city in self
                    .sim
                    .cities()
                    .iter()
                    .filter(|city| self.sim.can_steal_technology(unit, city))
                {
                    entries.push((
                        format!("Steal tech: {}", city.name),
                        UiAction::StealTechnology(city.id),
                        true,
                    ));
                }
                for city in self
                    .sim
                    .cities()
                    .iter()
                    .filter(|city| self.sim.can_sabotage_city(unit, city))
                {
                    entries.push((
                        format!("Sabotage {}", city.name),
                        UiAction::SabotageCity(city.id),
                        true,
                    ));
                }
                for city in self.sim.cities() {
                    if let Some(price) = self.sim.incite_price(unit, city) {
                        entries.push((
                            format!("Incite {} {price}g", city.name),
                            UiAction::InciteCity(city.id, price),
                            self.sim.players()[unit.owner].gold >= price,
                        ));
                    }
                }
                for target in self.sim.units() {
                    if !self.sim.unit_visible_to_human(target) {
                        continue;
                    }
                    if self.sim.can_sabotage_unit(unit, target) {
                        entries.push((
                            format!("Sabotage {}", target.kind.name()),
                            UiAction::SabotageUnit(target.id),
                            true,
                        ));
                    }
                    if let Some(price) = self.sim.bribe_price(unit, target) {
                        entries.push((
                            format!("Bribe {} {price}g", target.kind.name()),
                            UiAction::BribeUnit(target.id, price),
                            self.sim.players()[unit.owner].gold >= price,
                        ));
                    }
                }
            }
            if self.sim.can_begin_paradrop(unit) {
                entries.insert(0, ("Paradrop".to_owned(), UiAction::BeginParadrop, true));
            }
            for city in self.sim.cities() {
                if self.sim.can_airlift(unit, city) {
                    entries.push((
                        format!("Airlift to {}", city.name),
                        UiAction::Airlift(city.id),
                        true,
                    ));
                }
                if self.sim.can_join_city(unit, city) {
                    entries.push((
                        format!("Join {}", city.name),
                        UiAction::JoinCity(city.id),
                        true,
                    ));
                }
                if self.sim.can_deliver_goods(unit, city) && self.sim.tile(city.x, city.y).visible {
                    entries.push((
                        format!("Trade: {}", city.name),
                        UiAction::DeliverGoods(city.id),
                        true,
                    ));
                }
                if self.sim.can_help_wonder(unit, city) && self.sim.tile(city.x, city.y).visible {
                    entries.push((
                        format!("Help wonder: {}", city.name),
                        UiAction::HelpWonder(city.id),
                        true,
                    ));
                }
            }
            if unit.kind == UnitKind::Nuclear {
                entries.push(("Detonate here".to_owned(), UiAction::Detonate, true));
            }
            if let Some(label) = self.sim.transport_action(unit) {
                entries.push((format!("{label} [L]"), UiAction::BoardOrDeboard, true));
            }
            if stats.can_found && unit.transport.is_none() {
                entries.push((
                    "Found city [B]".to_owned(),
                    UiAction::FoundCity,
                    self.sim.can_found_at(unit.x, unit.y),
                ));
            }
            if stats.can_work && unit.transport.is_none() {
                let tile = self.sim.tile(unit.x, unit.y);
                let (label, enabled) = if tile.road && self.sim.has_tech(unit.owner, Tech::RAILROAD)
                {
                    (
                        "Build railroad [R]",
                        self.sim.can_railroad_at(unit.owner, unit.x, unit.y),
                    )
                } else {
                    (
                        "Build road [R]",
                        !tile.road
                            && tile.terrain.road_time() > 0
                            && (!tile.river
                                || self.sim.has_tech(unit.owner, Tech::BRIDGE_BUILDING)),
                    )
                };
                entries.push((label.to_owned(), UiAction::Road, enabled));
            }
            if stats.can_work && unit.transport.is_none() && !unit.kind.is_naval() {
                let tile = self.sim.tile(unit.x, unit.y);
                let (label, enabled) = if tile.irrigation {
                    (
                        "Build farmland [I]",
                        self.sim.can_farm_at(unit.owner, unit.x, unit.y),
                    )
                } else {
                    (
                        "Irrigate [I]",
                        self.sim.can_irrigate_at(unit.owner, unit.x, unit.y),
                    )
                };
                entries.push((label.to_owned(), UiAction::Irrigate, enabled));
            }
            if let Some(kind) = self.sim.oil_work(unit) {
                entries.push((
                    format!("Build {} [M]", kind.name()),
                    UiAction::Mine,
                    unit.work_left == 0,
                ));
            } else if stats.can_work && unit.transport.is_none() && !unit.kind.is_naval() {
                let tile = self.sim.tile(unit.x, unit.y);
                entries.push((
                    "Mine [M]".to_owned(),
                    UiAction::Mine,
                    !tile.mine && tile.terrain.mine_time() > 0,
                ));
            }
            for (kind, label) in [
                (WorkKind::Cultivate, "Cultivate [V]"),
                (WorkKind::Plant, "Plant [P]"),
                (WorkKind::Transform, "Transform [T]"),
            ] {
                if self.sim.terrain_work(unit, kind).is_some() {
                    entries.push((
                        label.to_owned(),
                        UiAction::TerrainWork(kind),
                        unit.work_left == 0,
                    ));
                }
            }
            if unit.kind.can_clean() && unit.transport.is_none() {
                let tile = self.sim.tile(unit.x, unit.y);
                for target in [Contamination::Pollution, Contamination::Fallout] {
                    if tile.has_contamination(target) {
                        entries.insert(
                            0,
                            (
                                format!("Clean {}", target.name()),
                                UiAction::TerrainWork(WorkKind::Clean(target)),
                                unit.work_left == 0,
                            ),
                        );
                    }
                }
            }
            for base in [Base::Fort, Base::Fortress, Base::Airstrip, Base::Airbase] {
                if self.sim.base_work_time(unit, base).is_some() {
                    entries.push((
                        format!("Build {}", base.name()),
                        UiAction::TerrainWork(WorkKind::BuildBase(base)),
                        unit.work_left == 0,
                    ));
                }
            }
            let base = self.sim.tile(unit.x, unit.y).base.map(PillageTarget::Base);
            for target in PillageTarget::ALL.into_iter().chain(base) {
                if self.sim.can_pillage(unit, target) {
                    entries.push((
                        format!("Pillage {}", target.name()),
                        UiAction::TerrainWork(WorkKind::Pillage(target)),
                        unit.work_left == 0,
                    ));
                }
            }
            if unit.kind.is_military() && unit.transport.is_none() {
                entries.push(("Fortify [F]".to_owned(), UiAction::Fortify, true));
            }
            if self.sim.home_city_target(unit).is_some() {
                entries.push(("Set home city [H]".to_owned(), UiAction::HomeCity, true));
            }
            if unit.automated
                || (unit.kind.fuel_capacity() == 0 && unit.kind != UnitKind::Helicopter)
            {
                entries.push(("Automate [A / X]".to_owned(), UiAction::Automate, true));
            }
            entries.push(("Wait [Space]".to_owned(), UiAction::Wait, true));
            for city in self
                .sim
                .cities()
                .iter()
                .filter(|city| self.sim.can_disband(unit, Some(city)))
            {
                entries.push((
                    format!("Recycle: {}", city.name),
                    UiAction::Disband(Some(city.id)),
                    true,
                ));
            }
            if self.sim.can_disband(unit, None) {
                entries.push(("Disband".to_owned(), UiAction::Disband(None), true));
            }
        } else if let Some(index) = self.selected_city_index() {
            let city = &self.sim.cities()[index];
            for kind in [
                if self.sim.has_tech(city.owner, Tech::EXPLOSIVES) {
                    UnitKind::Engineer
                } else {
                    UnitKind::Worker
                },
                UnitKind::Warrior,
                UnitKind::Phalanx,
                UnitKind::Archer,
                UnitKind::Horseman,
                UnitKind::Catapult,
                UnitKind::Chariot,
                UnitKind::Legion,
                UnitKind::Settlers,
            ] {
                entries.push((
                    format!("{} {}", short_unit_name(kind), kind.stats().cost),
                    UiAction::Produce(Production::Unit(kind)),
                    self.sim.can_change_production(city, Production::Unit(kind)),
                ));
            }
        }

        if entries.is_empty() {
            return;
        }
        let width = 34.0_f32.min(available_width.max(1.0));
        let gap = 5.0;
        let step = width + gap;
        let capacity = (((available_width + gap) / step).floor() as usize).max(2);
        let paginated = entries.len() > capacity;
        let page_size = capacity - usize::from(paginated);
        let pages = entries.len().div_ceil(page_size);
        let page = self.action_page.min(pages - 1);
        let mut visible: Vec<_> = entries
            .into_iter()
            .skip(page * page_size)
            .take(page_size)
            .collect();
        if paginated {
            visible.push((
                format!("More actions ({}/{})", page + 1, pages),
                UiAction::ActionPage((page + 1) % pages),
                true,
            ));
        }
        let row_width = visible.len() as f32 * step - gap;
        let x = (screen_width() - row_width) * 0.5;
        let y = screen_height() - HUD_MARGIN - 34.0;
        let mut tooltip = None;
        for (column, (label, action, enabled)) in visible.into_iter().enumerate() {
            let rect = Rect::new(x + column as f32 * step, y, width, 34.0);
            draw_ui_button(rect, "", enabled, false);
            let color = if enabled {
                Color::from_rgba(241, 238, 218, 255)
            } else {
                GRAY
            };
            draw_hud_icon(unit_action_icon(action), rect.center(), 21.0, color);
            if rect.contains(ui_pointer()) {
                tooltip = Some((rect, label));
            }
            actions.push((
                rect,
                if enabled {
                    action
                } else {
                    UiAction::BlockMapClick
                },
            ));
        }
        if let Some((button, label)) = tooltip {
            let label = fit_text(
                &label,
                15.0,
                (screen_width() - 2.0 * HUD_MARGIN - 16.0).max(0.0),
            );
            let width = measure_text(&label, None, 15, 1.0).width + 16.0;
            let x = (button.center().x - width * 0.5)
                .max(HUD_MARGIN)
                .min(screen_width() - HUD_MARGIN - width);
            let rect = Rect::new(x, button.y - 30.0, width, 24.0);
            draw_hud_card(rect);
            draw_text(&label, rect.x + 8.0, rect.y + 17.0, 15.0, WHITE);
        }
    }

    fn draw_messages(&self, actions: &mut Vec<(Rect, UiAction)>, available: f32) {
        let width = (screen_width() * 0.65).min(620.0).min(available);
        let latest = self.sim.events().iter().rev().take(2);
        for (i, message) in latest.enumerate() {
            let color = if i == 0 {
                Color::from_rgba(230, 228, 211, 255)
            } else {
                Color::from_rgba(174, 177, 171, 255)
            };
            let clipped = fit_text(message, 15.0, width - 16.0);
            let rect = Rect::new(
                HUD_MARGIN,
                TOP_H + 10.0 + (1 - i) as f32 * 21.0,
                measure_text(&clipped, None, 15, 1.0).width + 16.0,
                21.0,
            );
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                Color::from_rgba(16, 20, 20, 218),
            );
            draw_text(&clipped, rect.x + 8.0, rect.y + 15.0, 15.0, color);
            actions.push((rect, UiAction::BlockMapClick));
        }
    }
}

fn draw_hud_card(card: Rect) {
    draw_rectangle(
        card.x,
        card.y,
        card.w,
        card.h,
        Color::from_rgba(26, 29, 26, 237),
    );
    draw_rectangle_lines(
        card.x,
        card.y,
        card.w,
        card.h,
        2.0,
        Color::from_rgba(160, 126, 61, 255),
    );
}

fn unit_action_icon(action: UiAction) -> HudIcon {
    match action {
        UiAction::FoundCity | UiAction::JoinCity(_) | UiAction::Produce(_) => HudIcon::City,
        UiAction::Road => HudIcon::Road,
        UiAction::Irrigate => HudIcon::Water,
        UiAction::Mine => HudIcon::Pickaxe,
        UiAction::TerrainWork(WorkKind::Cultivate) => HudIcon::Wheat,
        UiAction::TerrainWork(WorkKind::Plant) => HudIcon::Tree,
        UiAction::TerrainWork(WorkKind::Transform) | UiAction::Disband(Some(_)) => HudIcon::Cycle,
        UiAction::TerrainWork(WorkKind::Clean(_)) => HudIcon::Broom,
        UiAction::TerrainWork(WorkKind::BuildBase(_)) | UiAction::Fortify => HudIcon::Shield,
        UiAction::TerrainWork(WorkKind::Pillage(_))
        | UiAction::Detonate
        | UiAction::SabotageCity(_)
        | UiAction::SabotageUnit(_) => HudIcon::Burst,
        UiAction::HomeCity => HudIcon::Home,
        UiAction::Automate => HudIcon::Gear,
        UiAction::Wait => HudIcon::Pause,
        UiAction::UpgradeUnit(..) => HudIcon::Upgrade,
        UiAction::EstablishEmbassy(_) => HudIcon::Government,
        UiAction::InvestigateCity(_) => HudIcon::Search,
        UiAction::SpyTargets(_) => HudIcon::Target,
        UiAction::StealTechnology(_) => HudIcon::Research,
        UiAction::BribeUnit(..) | UiAction::InciteCity(..) => HudIcon::Tax,
        UiAction::BeginParadrop => HudIcon::Parachute,
        UiAction::Airlift(_) => HudIcon::Plane,
        UiAction::BoardOrDeboard => HudIcon::Boat,
        UiAction::DeliverGoods(_) => HudIcon::Crate,
        UiAction::HelpWonder(_) => HudIcon::Wonder,
        UiAction::Disband(None) => HudIcon::Cross,
        _ => HudIcon::More,
    }
}

enum HudIcon {
    Government,
    Diplomacy,
    Research,
    Science,
    Tax,
    Luxury,
    Spaceship,
    City,
    Road,
    Water,
    Pickaxe,
    Wheat,
    Tree,
    Cycle,
    Broom,
    Shield,
    Burst,
    Home,
    Gear,
    Pause,
    Upgrade,
    Search,
    Target,
    Parachute,
    Plane,
    Boat,
    Crate,
    Wonder,
    Cross,
    More,
}

fn draw_hud_icon(icon: HudIcon, center: Vec2, size: f32, color: Color) {
    // Coordinates use a shared 24 by 24 canvas so the icons scale together.
    let paths: &[&[(f32, f32)]] = match icon {
        HudIcon::Government => &[
            &[(2., 8.), (12., 2.), (22., 8.), (2., 8.)],
            &[(5., 11.), (5., 19.)],
            &[(12., 11.), (12., 19.)],
            &[(19., 11.), (19., 19.)],
            &[(2., 22.), (22., 22.)],
        ],
        HudIcon::Diplomacy => &[
            &[
                (2., 3.),
                (17., 3.),
                (17., 12.),
                (8., 12.),
                (4., 16.),
                (4., 12.),
                (2., 12.),
                (2., 3.),
            ],
            &[
                (20., 8.),
                (22., 8.),
                (22., 18.),
                (20., 18.),
                (20., 22.),
                (15., 18.),
                (10., 18.),
                (10., 15.),
            ],
        ],
        HudIcon::Research => &[&[
            (12., 6.),
            (8., 3.),
            (2., 3.),
            (2., 18.),
            (8., 18.),
            (12., 21.),
            (16., 18.),
            (22., 18.),
            (22., 3.),
            (16., 3.),
            (12., 6.),
            (12., 21.),
        ]],
        HudIcon::Science => &[
            &[(8., 2.), (16., 2.)],
            &[
                (9., 2.),
                (9., 10.),
                (3., 20.),
                (5., 22.),
                (19., 22.),
                (21., 20.),
                (15., 10.),
                (15., 2.),
            ],
            &[(6., 15.), (18., 15.)],
        ],
        HudIcon::Tax => &[
            &[
                (3., 6.),
                (7., 3.),
                (17., 3.),
                (21., 6.),
                (17., 9.),
                (7., 9.),
                (3., 6.),
                (3., 18.),
                (7., 21.),
                (17., 21.),
                (21., 18.),
                (21., 6.),
            ],
            &[(3., 12.), (7., 15.), (17., 15.), (21., 12.)],
        ],
        HudIcon::Luxury => &[
            &[
                (2., 8.),
                (6., 3.),
                (18., 3.),
                (22., 8.),
                (12., 22.),
                (2., 8.),
                (22., 8.),
            ],
            &[(9., 3.), (7., 8.), (12., 22.), (17., 8.), (15., 3.)],
        ],
        HudIcon::Spaceship => &[
            &[
                (12., 2.),
                (16., 7.),
                (17., 15.),
                (20., 19.),
                (15., 18.),
                (9., 18.),
                (4., 19.),
                (7., 15.),
                (8., 7.),
                (12., 2.),
            ],
            &[(10., 9.), (14., 9.)],
            &[(12., 19.), (12., 23.)],
        ],
        HudIcon::City => &[
            &[
                (2., 21.),
                (2., 9.),
                (7., 9.),
                (7., 14.),
                (10., 14.),
                (10., 4.),
                (15., 4.),
                (15., 11.),
                (21., 11.),
                (21., 21.),
                (2., 21.),
            ],
            &[(6., 17.), (6., 18.)],
            &[(16., 15.), (16., 18.)],
        ],
        HudIcon::Road => &[
            &[(3., 22.), (9., 2.)],
            &[(21., 22.), (15., 2.)],
            &[(12., 3.), (12., 7.)],
            &[(12., 10.), (12., 14.)],
            &[(12., 17.), (12., 22.)],
        ],
        HudIcon::Water => &[&[
            (12., 2.),
            (6., 10.),
            (5., 15.),
            (7., 20.),
            (12., 22.),
            (17., 20.),
            (19., 15.),
            (18., 10.),
            (12., 2.),
        ]],
        HudIcon::Pickaxe => &[
            &[(4., 21.), (16., 7.)],
            &[(5., 4.), (11., 3.), (17., 6.), (21., 12.)],
        ],
        HudIcon::Wheat => &[
            &[(12., 22.), (12., 3.)],
            &[(12., 9.), (7., 5.)],
            &[(12., 13.), (17., 8.)],
            &[(12., 17.), (6., 12.)],
            &[(12., 19.), (18., 14.)],
        ],
        HudIcon::Tree => &[
            &[
                (12., 2.),
                (5., 11.),
                (8., 11.),
                (3., 18.),
                (21., 18.),
                (16., 11.),
                (19., 11.),
                (12., 2.),
            ],
            &[(12., 18.), (12., 23.)],
        ],
        HudIcon::Cycle => &[
            &[(3., 10.), (6., 5.), (15., 4.), (20., 8.)],
            &[(15., 8.), (20., 8.), (20., 3.)],
            &[(21., 14.), (18., 19.), (9., 20.), (4., 16.)],
            &[(9., 16.), (4., 16.), (4., 21.)],
        ],
        HudIcon::Broom => &[
            &[(11., 13.), (20., 3.)],
            &[(8., 10.), (14., 15.), (10., 22.), (3., 16.), (8., 10.)],
            &[(6., 19.), (10., 14.)],
        ],
        HudIcon::Shield => &[
            &[
                (3., 4.),
                (21., 4.),
                (21., 13.),
                (18., 18.),
                (12., 22.),
                (6., 18.),
                (3., 13.),
                (3., 4.),
            ],
            &[(12., 7.), (12., 17.)],
        ],
        HudIcon::Burst => &[&[
            (12., 2.),
            (14., 8.),
            (21., 5.),
            (18., 11.),
            (23., 13.),
            (17., 15.),
            (20., 22.),
            (13., 18.),
            (9., 22.),
            (8., 16.),
            (2., 17.),
            (6., 12.),
            (2., 7.),
            (9., 9.),
            (12., 2.),
        ]],
        HudIcon::Home => &[
            &[(2., 11.), (12., 3.), (22., 11.)],
            &[(5., 9.), (5., 21.), (19., 21.), (19., 9.)],
            &[(10., 21.), (10., 15.), (14., 15.), (14., 21.)],
        ],
        HudIcon::Gear => &[
            &[
                (9., 2.),
                (15., 2.),
                (16., 6.),
                (21., 6.),
                (22., 11.),
                (19., 14.),
                (20., 19.),
                (15., 22.),
                (12., 19.),
                (8., 22.),
                (4., 19.),
                (5., 14.),
                (2., 11.),
                (3., 6.),
                (8., 6.),
                (9., 2.),
            ],
            &[(9., 12.), (12., 9.), (15., 12.), (12., 15.), (9., 12.)],
        ],
        HudIcon::Pause => &[&[(9., 5.), (9., 19.)], &[(15., 5.), (15., 19.)]],
        HudIcon::Upgrade => &[
            &[(4., 10.), (12., 3.), (20., 10.)],
            &[(4., 17.), (12., 10.), (20., 17.)],
            &[(12., 11.), (12., 22.)],
        ],
        HudIcon::Search => &[
            &[
                (4., 4.),
                (10., 2.),
                (16., 5.),
                (18., 11.),
                (15., 16.),
                (9., 18.),
                (3., 15.),
                (2., 9.),
                (4., 4.),
            ],
            &[(16., 16.), (22., 22.)],
        ],
        HudIcon::Target => &[
            &[
                (12., 4.),
                (18., 6.),
                (20., 12.),
                (18., 18.),
                (12., 20.),
                (6., 18.),
                (4., 12.),
                (6., 6.),
                (12., 4.),
            ],
            &[(12., 1.), (12., 8.)],
            &[(12., 16.), (12., 23.)],
            &[(1., 12.), (8., 12.)],
            &[(16., 12.), (23., 12.)],
        ],
        HudIcon::Parachute => &[
            &[
                (2., 10.),
                (4., 6.),
                (8., 3.),
                (16., 3.),
                (20., 6.),
                (22., 10.),
                (2., 10.),
                (12., 21.),
                (22., 10.),
            ],
            &[(8., 10.), (12., 21.), (16., 10.)],
        ],
        HudIcon::Plane => &[&[
            (12., 2.),
            (14., 4.),
            (14., 9.),
            (22., 15.),
            (22., 17.),
            (14., 14.),
            (14., 19.),
            (17., 22.),
            (7., 22.),
            (10., 19.),
            (10., 14.),
            (2., 17.),
            (2., 15.),
            (10., 9.),
            (10., 4.),
            (12., 2.),
        ]],
        HudIcon::Boat => &[
            &[(2., 15.), (22., 15.), (18., 21.), (6., 21.), (2., 15.)],
            &[(12., 15.), (12., 3.), (19., 12.), (12., 12.)],
        ],
        HudIcon::Crate => &[
            &[
                (3., 7.),
                (12., 3.),
                (21., 7.),
                (21., 17.),
                (12., 21.),
                (3., 17.),
                (3., 7.),
                (12., 11.),
                (21., 7.),
            ],
            &[(12., 11.), (12., 21.)],
            &[(7., 5.), (16., 9.)],
        ],
        HudIcon::Wonder => &[
            &[(2., 21.), (12., 3.), (22., 21.), (2., 21.)],
            &[(6., 14.), (18., 14.)],
            &[(4., 18.), (20., 18.)],
        ],
        HudIcon::Cross => &[&[(5., 5.), (19., 19.)], &[(19., 5.), (5., 19.)]],
        HudIcon::More => &[
            &[(5., 11.), (5., 13.)],
            &[(12., 11.), (12., 13.)],
            &[(19., 11.), (19., 13.)],
        ],
    };
    let point = |(x, y)| center + vec2(x - 12.0, y - 12.0) * (size / 24.0);
    for path in paths {
        for pair in path.windows(2) {
            let a = point(pair[0]);
            let b = point(pair[1]);
            draw_line(a.x, a.y, b.x, b.y, 1.6, color);
        }
    }
}
