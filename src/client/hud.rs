use super::*;

impl Game {
    pub(super) fn draw_hud(&mut self) -> Vec<(Rect, UiAction)> {
        let mut actions = Vec::new();
        let viewport = Self::viewport_rect();
        let panel_x = screen_width() - PANEL_W;

        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            TOP_H,
            Color::from_rgba(29, 31, 28, 255),
        );
        draw_rectangle(
            0.0,
            TOP_H - 3.0,
            screen_width(),
            3.0,
            Color::from_rgba(171, 133, 55, 255),
        );
        draw_text(
            "CivCraft",
            18.0,
            31.0,
            25.0,
            Color::from_rgba(244, 220, 143, 255),
        );
        draw_text(
            &format!(
                "TURN {}  |  {}",
                self.sim.turn(),
                year_label(self.sim.year())
            ),
            214.0,
            29.0,
            20.0,
            Color::from_rgba(227, 230, 220, 255),
        );
        let player = &self.sim.players()[HUMAN];
        draw_circle(483.0, 23.0, 8.0, PLAYER_COLORS[HUMAN]);
        draw_text(
            player.name,
            498.0,
            29.0,
            18.0,
            Color::from_rgba(228, 226, 210, 255),
        );
        draw_text(
            &format!("Treasury: {}", player.gold),
            665.0,
            29.0,
            17.0,
            Color::from_rgba(231, 201, 91, 255),
        );
        let research = player.researching;
        draw_text(
            &format!(
                "Research: {}  {}/{}",
                research.name(),
                player.research,
                self.sim.target_cost(HUMAN, research)
            ),
            785.0,
            29.0,
            17.0,
            Color::from_rgba(151, 210, 231, 255),
        );
        self.hud_elements_drawn += 1;

        draw_rectangle(
            panel_x,
            TOP_H,
            PANEL_W,
            screen_height() - TOP_H,
            Color::from_rgba(38, 40, 37, 255),
        );
        draw_rectangle(
            panel_x,
            TOP_H,
            4.0,
            screen_height() - TOP_H,
            Color::from_rgba(133, 104, 47, 255),
        );
        draw_text(
            "WORLD MAP",
            panel_x + 18.0,
            TOP_H + 26.0,
            18.0,
            Color::from_rgba(227, 211, 164, 255),
        );
        self.draw_minimap();
        self.hud_elements_drawn += 1;

        let context_y = Self::minimap_rect().bottom() + 17.0;
        draw_line(
            panel_x + 16.0,
            context_y - 9.0,
            screen_width() - 16.0,
            context_y - 9.0,
            1.0,
            Color::from_rgba(112, 108, 91, 255),
        );
        self.draw_context_panel(panel_x + 18.0, context_y);
        self.hud_elements_drawn += 1;

        let research_rect = Rect::new(
            panel_x + 18.0,
            screen_height() - 112.0,
            PANEL_W - 36.0,
            34.0,
        );
        draw_ui_button(
            research_rect,
            &format!(
                "Research tree: {}",
                self.sim.players()[HUMAN].researching.name()
            ),
            true,
            false,
        );
        actions.push((research_rect, UiAction::OpenResearch));
        let end_rect = Rect::new(panel_x + 18.0, screen_height() - 65.0, PANEL_W - 36.0, 44.0);
        draw_ui_button(end_rect, "END TURN  [Shift+Enter]", true, true);
        actions.push((end_rect, UiAction::EndTurn));

        draw_rectangle(
            0.0,
            screen_height() - BOTTOM_H,
            viewport.w,
            BOTTOM_H,
            Color::from_rgba(28, 30, 28, 248),
        );
        draw_rectangle(
            0.0,
            screen_height() - BOTTOM_H,
            viewport.w,
            3.0,
            Color::from_rgba(133, 104, 47, 255),
        );
        self.draw_action_strip(&mut actions, viewport.w);
        self.draw_messages(viewport.w);
        self.hud_elements_drawn += 1;

        actions
    }

    fn draw_minimap(&self) {
        let rect = Self::minimap_rect();
        draw_rectangle(
            rect.x - 3.0,
            rect.y - 3.0,
            rect.w + 6.0,
            rect.h + 6.0,
            Color::from_rgba(15, 17, 16, 255),
        );
        let scale = rect.h / MAP_H as f32;
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let tile = self.sim.tile(x, y);
                let color = if !tile.seen {
                    Color::from_rgba(7, 13, 17, 255)
                } else if tile.visible {
                    tile.terrain.minimap_color()
                } else {
                    darken(tile.terrain.minimap_color(), 0.53)
                };
                let origin = Self::minimap_tile_origin(rect, x, y);
                let width = (rect.right() - origin.x).min(2.0 * scale);
                draw_rectangle(origin.x, origin.y, width, scale, color);
                if y % 2 == 1 && x == MAP_W - 1 {
                    draw_rectangle(rect.x, origin.y, scale, scale, color);
                }
            }
        }
        for city in self.sim.cities() {
            if self.sim.tile(city.x, city.y).visible {
                let origin = Self::minimap_tile_origin(rect, city.x, city.y);
                let marker = Rect::new(origin.x, origin.y, 2.0 * scale, scale);
                let color = PLAYER_COLORS[city.owner];
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
        }

        let viewport = Self::viewport_rect();
        let camera =
            vec2(rect.x, rect.y) + (self.camera_overview_position() + vec2(1.0, 0.5)) * scale;
        let width = viewport.w * scale / (TILE_W * 0.5 * self.zoom);
        let height = viewport.h * scale / (TILE_H * 0.5 * self.zoom);
        for wrap_x in [-rect.w, 0.0, rect.w] {
            let left = camera.x + wrap_x - width * 0.5;
            let top = camera.y - height * 0.47;
            let right = left + width;
            let bottom = top + height;
            for y in [top, bottom] {
                if (rect.y..=rect.bottom()).contains(&y) && left < rect.right() && right > rect.x {
                    draw_line(left.max(rect.x), y, right.min(rect.right()), y, 1.0, WHITE);
                }
            }
            for x in [left, right] {
                if (rect.x..=rect.right()).contains(&x) && top < rect.bottom() && bottom > rect.y {
                    draw_line(x, top.max(rect.y), x, bottom.min(rect.bottom()), 1.0, WHITE);
                }
            }
        }
    }

    pub(super) fn draw_city_manager(&self, assets: &Assets, actions: &mut Vec<(Rect, UiAction)>) {
        let Some(index) = self.selected_city_index() else {
            return;
        };
        let city = &self.sim.cities()[index];
        let rect = Self::city_manager_rect();
        let viewport = Self::viewport_rect();
        draw_rectangle(
            viewport.x,
            viewport.y,
            viewport.w,
            viewport.h,
            Color::from_rgba(4, 9, 12, 178),
        );
        draw_rectangle(
            rect.x - 5.0,
            rect.y - 5.0,
            rect.w + 10.0,
            rect.h + 10.0,
            Color::from_rgba(9, 11, 10, 245),
        );
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Color::from_rgba(42, 44, 40, 252),
        );
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            3.0,
            Color::from_rgba(206, 165, 72, 255),
        );

        draw_text(
            "CITY MANAGEMENT",
            rect.x + 22.0,
            rect.y + 29.0,
            17.0,
            Color::from_rgba(175, 178, 165, 255),
        );
        draw_text(
            &city.name,
            rect.x + 22.0,
            rect.y + 61.0,
            29.0,
            Color::from_rgba(248, 222, 139, 255),
        );
        draw_text(
            &format!("Size {}  |  {}, {}", city.size, city.x, city.y),
            rect.x + 24.0,
            rect.y + 84.0,
            16.0,
            Color::from_rgba(183, 188, 178, 255),
        );
        let close_rect = Rect::new(rect.x + rect.w - 48.0, rect.y + 14.0, 32.0, 28.0);
        draw_ui_button(close_rect, "X", true, false);
        actions.push((close_rect, UiAction::CloseCity));

        let (food, shields, trade) = self.sim.city_yields(city);
        let food_net = food - city.size * 2;
        let growth_box = Simulation::growth_box(city.size);
        let production_cost = city.production.stats().cost;
        let progress_x = rect.x + 205.0;
        let progress_w = rect.w - 229.0;
        draw_text(
            &format!("Food {food} ({food_net:+})   Shields {shields}   Trade {trade}"),
            progress_x,
            rect.y + 31.0,
            17.0,
            Color::from_rgba(188, 220, 166, 255),
        );
        draw_meter(
            Rect::new(progress_x, rect.y + 43.0, progress_w, 15.0),
            city.food,
            growth_box,
            Color::from_rgba(82, 166, 82, 255),
        );
        draw_text(
            &format!("Growth  {}/{}", city.food, growth_box),
            progress_x,
            rect.y + 78.0,
            15.0,
            Color::from_rgba(215, 218, 202, 255),
        );
        draw_meter(
            Rect::new(
                progress_x + progress_w * 0.48,
                rect.y + 66.0,
                progress_w * 0.52,
                15.0,
            ),
            city.shields,
            production_cost,
            Color::from_rgba(173, 133, 62, 255),
        );
        let production_label = format!(
            "{}  {}/{}",
            city.production.name(),
            city.shields,
            production_cost
        );
        let production_label_width = measure_text(&production_label, None, 15, 1.0)
            .width
            .min(progress_w * 0.5);
        draw_text(
            &production_label,
            progress_x + progress_w - production_label_width,
            rect.y + 78.0,
            15.0,
            Color::from_rgba(215, 218, 202, 255),
        );

        draw_line(
            rect.x + 20.0,
            rect.y + 101.0,
            rect.x + rect.w - 20.0,
            rect.y + 101.0,
            1.0,
            Color::from_rgba(105, 102, 86, 255),
        );
        draw_text(
            "CHOOSE PRODUCTION",
            rect.x + 22.0,
            rect.y + 127.0,
            18.0,
            Color::from_rgba(226, 211, 162, 255),
        );

        let choices = [
            UnitKind::Worker,
            UnitKind::Warrior,
            UnitKind::Phalanx,
            UnitKind::Archer,
            UnitKind::Horseman,
            UnitKind::Catapult,
            UnitKind::Chariot,
            UnitKind::Legion,
            UnitKind::Diplomat,
            UnitKind::Settlers,
        ];
        let columns = 4;
        let gap = 8.0;
        let card_w = (rect.w - 44.0 - gap * 3.0) / 4.0;
        let card_h = 72.0;
        let mouse = vec2(mouse_position().0, mouse_position().1);
        for (choice_index, kind) in choices.into_iter().enumerate() {
            let column = choice_index % columns;
            let row = choice_index / columns;
            let card = Rect::new(
                rect.x + 22.0 + column as f32 * (card_w + gap),
                rect.y + 139.0 + row as f32 * (card_h + gap),
                card_w,
                card_h,
            );
            let enabled = self.sim.unit_available(HUMAN, kind);
            let current = city.production == kind;
            let hovered = enabled && card.contains(mouse);
            let fill = if current {
                Color::from_rgba(101, 80, 39, 255)
            } else if hovered {
                Color::from_rgba(76, 89, 80, 255)
            } else if enabled {
                Color::from_rgba(55, 61, 56, 255)
            } else {
                Color::from_rgba(43, 46, 43, 255)
            };
            draw_rectangle(card.x, card.y, card.w, card.h, fill);
            draw_rectangle_lines(
                card.x,
                card.y,
                card.w,
                card.h,
                if current { 2.0 } else { 1.0 },
                if current {
                    Color::from_rgba(236, 199, 97, 255)
                } else if enabled {
                    Color::from_rgba(117, 124, 111, 255)
                } else {
                    Color::from_rgba(65, 68, 63, 255)
                },
            );
            let (sprite_row, sprite_col) = unit_sprite(kind);
            let stats = kind.stats();
            draw_texture_ex(
                &assets.units,
                card.x + 6.0,
                card.y + 13.0,
                if enabled {
                    WHITE
                } else {
                    Color::new(0.38, 0.4, 0.4, 1.0)
                },
                DrawTextureParams {
                    dest_size: Some(vec2(52.0, 39.0)),
                    source: Some(Rect::new(
                        1.0 + sprite_col as f32 * 65.0,
                        1.0 + sprite_row as f32 * 49.0,
                        64.0,
                        48.0,
                    )),
                    ..Default::default()
                },
            );
            let name = short_unit_name(kind);
            draw_text(
                name,
                card.x + 62.0,
                card.y + 26.0,
                16.0,
                if enabled {
                    Color::from_rgba(239, 235, 216, 255)
                } else {
                    Color::from_rgba(111, 114, 107, 255)
                },
            );
            draw_text(
                &format!("{} shields", stats.cost),
                card.x + 62.0,
                card.y + 47.0,
                13.0,
                if enabled {
                    Color::from_rgba(176, 184, 171, 255)
                } else {
                    Color::from_rgba(91, 94, 89, 255)
                },
            );
            if current {
                draw_text(
                    "CURRENT",
                    card.x + 7.0,
                    card.y + card.h - 7.0,
                    11.0,
                    Color::from_rgba(247, 211, 112, 255),
                );
            }
            if enabled {
                actions.push((card, UiAction::Produce(kind)));
            }
        }

        let garrison_y = rect.y + 395.0;
        draw_text(
            "GARRISON",
            rect.x + 22.0,
            garrison_y,
            18.0,
            Color::from_rgba(226, 211, 162, 255),
        );
        let garrison: Vec<&Unit> = self
            .sim
            .units()
            .iter()
            .filter(|unit| unit.owner == HUMAN && unit.x == city.x && unit.y == city.y)
            .collect();
        if garrison.is_empty() {
            draw_text(
                "No units stationed in this city.",
                rect.x + 22.0,
                garrison_y + 27.0,
                15.0,
                Color::from_rgba(159, 164, 154, 255),
            );
        } else {
            let button_gap = 7.0;
            let button_w = ((rect.w - 44.0 - button_gap * 3.0) / 4.0).min(170.0);
            for (unit_index, unit) in garrison.into_iter().take(4).enumerate() {
                let unit_rect = Rect::new(
                    rect.x + 22.0 + unit_index as f32 * (button_w + button_gap),
                    garrison_y + 10.0,
                    button_w,
                    35.0,
                );
                draw_ui_button(
                    unit_rect,
                    &format!("Select {}", short_unit_name(unit.kind)),
                    true,
                    false,
                );
                actions.push((unit_rect, UiAction::SelectUnit(unit.id)));
            }
        }
        draw_text(
            "Select a unit to leave the city screen  |  Esc closes",
            rect.x + 22.0,
            rect.y + rect.h - 17.0,
            14.0,
            Color::from_rgba(145, 151, 142, 255),
        );
    }

    fn draw_context_panel(&self, x: f32, y: f32) {
        if let Some(index) = self.selected_unit_index() {
            let unit = &self.sim.units()[index];
            let stats = unit.kind.stats();
            let state = if unit.work_remaining > 0 {
                format!(
                    "Building {} - {} turns",
                    unit.work_kind.map(|w| w.name()).unwrap_or("improvement"),
                    unit.work_remaining
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
            draw_text(
                unit.kind.name(),
                x,
                y + 23.0,
                25.0,
                Color::from_rgba(244, 221, 147, 255),
            );
            draw_text(
                &format!(
                    "{} at {}, {}",
                    self.sim.players()[unit.owner].name,
                    unit.x,
                    unit.y
                ),
                x,
                y + 46.0,
                16.0,
                Color::from_rgba(180, 185, 176, 255),
            );
            draw_text(
                &format!("Attack {}   Defense {}", stats.attack, stats.defense),
                x,
                y + 75.0,
                18.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            draw_text(
                &format!(
                    "Health {}/{}   Moves {}/{}",
                    unit.hp,
                    stats.hp,
                    unit.moves_left,
                    stats.moves * MOVE_FRAGMENTS
                ),
                x,
                y + 99.0,
                18.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            draw_text(
                &format!("Terrain: {}", self.sim.tile(unit.x, unit.y).terrain.name()),
                x,
                y + 123.0,
                17.0,
                Color::from_rgba(167, 201, 158, 255),
            );
            draw_text(
                &state,
                x,
                y + 148.0,
                17.0,
                Color::from_rgba(145, 204, 224, 255),
            );
            draw_text(
                "Move: arrows / 2 3 4 6 7 8",
                x,
                y + 184.0,
                16.0,
                Color::from_rgba(177, 177, 164, 255),
            );
            draw_text(
                "G: go-to   Space: wait   Tab: next",
                x,
                y + 207.0,
                15.0,
                Color::from_rgba(177, 177, 164, 255),
            );
        } else if let Some(index) = self.selected_city_index() {
            let city = &self.sim.cities()[index];
            let (food, shields, trade) = self.sim.city_yields(city);
            let cost = city.production.stats().cost;
            draw_text(
                &city.name,
                x,
                y + 23.0,
                25.0,
                Color::from_rgba(244, 221, 147, 255),
            );
            draw_text(
                &format!("City size {}", city.size),
                x,
                y + 48.0,
                18.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            draw_text(
                &format!("Food {}  Shields {}  Trade {}", food, shields, trade),
                x,
                y + 76.0,
                17.0,
                Color::from_rgba(177, 213, 159, 255),
            );
            draw_text(
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
            draw_text(
                &format!("Training: {}", city.production.name()),
                x,
                y + 132.0,
                18.0,
                Color::from_rgba(145, 204, 224, 255),
            );
            draw_text(
                &format!("Production {}/{}", city.shields, cost),
                x,
                y + 156.0,
                17.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            draw_text(
                "Choose a unit below to change production.",
                x,
                y + 192.0,
                15.0,
                Color::from_rgba(177, 177, 164, 255),
            );
        } else {
            draw_text(
                "YOUR CHRONICLE",
                x,
                y + 22.0,
                22.0,
                Color::from_rgba(244, 221, 147, 255),
            );
            draw_text(
                "Found a city with your Settlers.",
                x,
                y + 55.0,
                17.0,
                Color::from_rgba(225, 224, 208, 255),
            );
            draw_text(
                "Explore the fog and improve the land.",
                x,
                y + 80.0,
                17.0,
                Color::from_rgba(225, 224, 208, 255),
            );
            draw_text(
                "Research stronger units and unite",
                x,
                y + 105.0,
                17.0,
                Color::from_rgba(225, 224, 208, 255),
            );
            draw_text(
                "the six rival civilizations.",
                x,
                y + 128.0,
                17.0,
                Color::from_rgba(225, 224, 208, 255),
            );
            let living = (1..PLAYER_COUNT)
                .filter(|owner| {
                    self.sim.cities().iter().any(|c| c.owner == *owner)
                        || self.sim.units().iter().any(|u| u.owner == *owner)
                })
                .count();
            draw_text(
                &format!("Rivals remaining: {living}"),
                x,
                y + 172.0,
                18.0,
                Color::from_rgba(236, 183, 117, 255),
            );
            draw_text(
                "Click a unit or city for details.",
                x,
                y + 208.0,
                15.0,
                Color::from_rgba(174, 177, 168, 255),
            );
        }
    }

    fn draw_action_strip(&self, actions: &mut Vec<(Rect, UiAction)>, available_width: f32) {
        let mut entries: Vec<(String, UiAction, bool)> = Vec::new();
        if let Some(index) = self.selected_unit_index() {
            let unit = &self.sim.units()[index];
            let stats = unit.kind.stats();
            if stats.can_found {
                entries.push((
                    "Found [B]".to_owned(),
                    UiAction::FoundCity,
                    self.sim.can_found_at(unit.x, unit.y),
                ));
            }
            if stats.can_work {
                entries.push((
                    "Road [R]".to_owned(),
                    UiAction::Road,
                    !self.sim.tile(unit.x, unit.y).road,
                ));
                entries.push((
                    "Irrigate [I]".to_owned(),
                    UiAction::Irrigate,
                    self.sim.can_irrigate_at(unit.x, unit.y),
                ));
                entries.push((
                    "Mine [M]".to_owned(),
                    UiAction::Mine,
                    !self.sim.tile(unit.x, unit.y).mine
                        && self.sim.tile(unit.x, unit.y).terrain.mine_time() > 0,
                ));
            }
            if unit.kind.is_military() {
                entries.push(("Fortify [F]".to_owned(), UiAction::Fortify, true));
            }
            entries.push(("Automate [A]".to_owned(), UiAction::Automate, true));
            entries.push(("Wait [Space]".to_owned(), UiAction::Wait, true));
        } else if self.selected_city_index().is_some() {
            for kind in [
                UnitKind::Worker,
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
                    UiAction::Produce(kind),
                    self.sim.unit_available(HUMAN, kind),
                ));
            }
        } else {
            entries.push(("Next ready unit [Tab]".to_owned(), UiAction::Wait, false));
        }

        let count = entries.len().max(1) as f32;
        let gap = 6.0;
        let width = ((available_width - 24.0 - gap * (count - 1.0)) / count).clamp(76.0, 132.0);
        let y = screen_height() - BOTTOM_H + 10.0;
        let mut x = 12.0;
        for (label, action, enabled) in entries {
            if x + width > available_width - 8.0 {
                break;
            }
            let rect = Rect::new(x, y, width, 31.0);
            draw_ui_button(rect, &label, enabled, false);
            if enabled {
                actions.push((rect, action));
            }
            x += width + gap;
        }
    }

    fn draw_messages(&self, available_width: f32) {
        let latest: Vec<&String> = self.sim.events().iter().rev().take(2).collect();
        let mut y = screen_height() - 11.0;
        for (i, message) in latest.iter().enumerate() {
            let color = if i == 0 {
                Color::from_rgba(230, 228, 211, 255)
            } else {
                Color::from_rgba(154, 158, 151, 255)
            };
            let clipped = clip_text(message, ((available_width - 36.0) / 8.6) as usize);
            draw_text(&clipped, 18.0, y, 15.0, color);
            y -= 19.0;
        }
    }
}
