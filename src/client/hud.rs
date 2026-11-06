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
        let player = &self.sim.players()[HUMAN];
        let government_button = Rect::new(118.0, 8.0, 108.0, 30.0);
        draw_ui_button(government_button, player.government.name(), true, false);
        actions.push((government_button, UiAction::ToggleGovernment));
        text_in(
            &format!("Turn {} · {}", self.sim.turn(), year_label(self.sim.year())),
            238.0,
            29.0,
            17.0,
            178.0,
            WHITE,
        );
        let diplomacy = Rect::new(424.0, 8.0, 90.0, 30.0);
        draw_ui_button(diplomacy, "Diplomacy", true, false);
        actions.push((diplomacy, UiAction::ToggleDiplomacy));
        let treasury = format!("Gold {}", player.gold);
        let treasury_w = measure_text(&treasury, None, 16, 1.0).width.min(180.0);
        text_in(&treasury, 530.0, 29.0, 16.0, treasury_w, GOLD);
        let research = player.researching;
        text_in(
            &format!(
                "{}  {}/{}",
                research.name(),
                player.research,
                self.sim.target_cost(HUMAN, research)
            ),
            546.0 + treasury_w,
            29.0,
            16.0,
            screen_width() - 564.0 - treasury_w,
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
        let seed_label = format!("Seed {} · Wrap {}", self.sim.world_seed, self.sim.map_wrap);
        text_in(
            &seed_label,
            panel_x + UI_PAD,
            TOP_H + 46.0,
            14.0,
            PANEL_W - 2.0 * UI_PAD,
            LIGHTGRAY,
        );
        if let Some(status) = self.sim.spaceship_status(HUMAN) {
            let rect = Rect::new(
                panel_x + 18.0,
                screen_height() - 178.0,
                PANEL_W - 36.0,
                27.0,
            );
            let ready = self.sim.can_launch_spaceship(HUMAN);
            draw_ui_button(rect, &status, true, ready);
            actions.push((rect, UiAction::ToggleSpaceship));
        }
        self.draw_minimap();
        self.hud_elements_drawn += 1;

        let context_y = self.minimap_rect().bottom() + 17.0;
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

        for (index, label) in ["Science", "Tax", "Luxury"].into_iter().enumerate() {
            let player = &self.sim.players()[HUMAN];
            let in_anarchy = player.government == Government::Anarchy;
            let rate = if in_anarchy {
                [0, 0, 100][index]
            } else {
                player.rates[index]
            };
            let enabled = !in_anarchy && rate < player.government.max_rate();
            let rect = Rect::new(
                panel_x + UI_PAD + index as f32 * ((PANEL_W - 2.0 * UI_PAD + UI_GAP) / 3.0),
                screen_height() - 146.0,
                (PANEL_W - 2.0 * UI_PAD - 2.0 * UI_GAP) / 3.0,
                28.0,
            );
            draw_ui_button(rect, &format!("{label} {rate}%"), enabled, false);
            if enabled {
                actions.push((rect, UiAction::RaiseRate(index)));
            }
        }
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
        let rect = self.minimap_rect();
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
                let tile = self.sim.render_tile(x, y);
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
        let city_overrides = self.playback.city_overrides();
        let observed = self.sim.render_city_markers();
        for city in observed
            .into_iter()
            .filter(|c| !city_overrides.contains_key(&c.id))
            .chain(city_overrides.values())
        {
            let origin = Self::minimap_tile_origin(rect, city.x, city.y);
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
    }

    fn draw_city_tiles(
        &self,
        assets: &Assets,
        city: &City,
        panel: Rect,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        self.draw_city_tiles_map(assets, city, panel, actions);
    }

    fn draw_worklist(
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

    fn draw_queue_button(
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

    pub(super) fn draw_city_manager(&self, assets: &Assets, actions: &mut Vec<(Rect, UiAction)>) {
        let Some(index) = self.selected_city_index() else {
            return;
        };
        let city = &self.sim.cities()[index];
        let rect = Self::city_manager_rect();
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
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

        text_in(
            &city.name,
            rect.x + 22.0,
            rect.y + 35.0,
            26.0,
            rect.w - 100.0,
            Color::from_rgba(248, 222, 139, 255),
        );
        let close_rect = Rect::new(rect.right() - 50.0, rect.y + 14.0, 28.0, 28.0);
        draw_ui_button(close_rect, "X", true, false);
        actions.push((close_rect, UiAction::CloseCity));
        let forecast = self.sim.city_forecast(city);
        let support = &forecast.support;
        let food_net = forecast.food_surplus(city.size, support.food);
        let shields = forecast.shield_surplus();
        let science = forecast.science;
        let luxury = forecast.luxury;
        let gold = forecast.gold_surplus(support.gold);
        text_in(
            &format!(
                "Size {} · Food {food_net:+} · Shields {shields} · Science {science} · Gold {gold:+} · Luxury {luxury}",
                city.size
            ),
            rect.x + 22.0,
            rect.y + 61.0,
            16.0,
            rect.w - 44.0,
            Color::from_rgba(188, 220, 166, 255),
        );
        let growth_box = Simulation::growth_box(city.size);
        let production_cost = city.production.cost();
        let half = (rect.w - 60.0) / 2.0;
        let growth = if forecast.disorder {
            "Civil disorder".to_owned()
        } else if city.size >= Simulation::city_size_limit(city) {
            "Growth limit reached".to_owned()
        } else {
            format!("Growth {}/{}", city.food, growth_box)
        };
        text_in(&growth, rect.x + 22.0, rect.y + 87.0, 15.0, half, LIGHTGRAY);
        text_in(
            &format!(
                "{} {}/{}",
                city.production.name(),
                city.shields,
                production_cost
            ),
            rect.x + 38.0 + half,
            rect.y + 87.0,
            15.0,
            half,
            LIGHTGRAY,
        );
        draw_meter(
            Rect::new(rect.x + 22.0, rect.y + 96.0, half, 10.0),
            city.food,
            growth_box,
            Color::from_rgba(82, 166, 82, 255),
        );
        draw_meter(
            Rect::new(rect.x + 38.0 + half, rect.y + 96.0, half, 10.0),
            city.shields,
            production_cost,
            Color::from_rgba(173, 133, 62, 255),
        );
        let specialist_w = (rect.w - 44.0) / 3.0;
        for (index, label) in ["Scientists", "Taxmen", "Entertainers"]
            .into_iter()
            .enumerate()
        {
            let x = rect.x + 22.0 + index as f32 * specialist_w;
            let count = city.specialists[index];
            for (delta, offset, symbol, enabled) in [
                (-1, 0.0, "-", count > 0),
                (1, specialist_w - 36.0, "+", city.tile_workers() > 0),
            ] {
                let button = Rect::new(x + offset, rect.y + 118.0, 28.0, 26.0);
                draw_ui_button(button, symbol, enabled, false);
                if enabled {
                    actions.push((button, UiAction::Specialist(city.id, index, delta)));
                }
            }
            text_in(
                &format!("{label} {count}"),
                x + 34.0,
                rect.y + 136.0,
                14.0,
                specialist_w - 76.0,
                LIGHTGRAY,
            );
        }
        draw_line(
            rect.x + 22.0,
            rect.y + 154.0,
            rect.right() - 22.0,
            rect.y + 154.0,
            1.0,
            Color::from_rgba(105, 102, 86, 255),
        );
        if let Some(page) = self.worklist_page {
            self.draw_worklist(city, rect, page, actions);
            return;
        }
        let tiles_button = Rect::new(rect.x + rect.w - 277.0, rect.y + 169.0, 82.0, 25.0);
        draw_ui_button(
            tiles_button,
            if self.city_tiles {
                "Production"
            } else {
                "City tiles"
            },
            true,
            self.city_tiles,
        );
        actions.push((tiles_button, UiAction::CityTiles));
        if self.city_tiles {
            self.draw_city_tiles(assets, city, rect, actions);
            return;
        }
        draw_text(
            "CHOOSE PRODUCTION",
            rect.x + 22.0,
            rect.y + 187.0,
            18.0,
            Color::from_rgba(226, 211, 162, 255),
        );

        let choices: Vec<_> = UnitKind::ALL_NATIVE
            .into_iter()
            .filter(|&kind| self.sim.can_produce(city, Production::Unit(kind)))
            .collect();
        let columns = grid_columns(rect.w - 44.0, 180.0);
        let gap = 8.0;
        let card_w = (rect.w - 44.0 - gap * (columns - 1) as f32) / columns as f32;
        let card_h = 72.0;
        let rows = ((rect.h - 199.0 - 112.0) / (card_h + gap)).floor().max(1.0) as usize;
        let capacity = columns * rows;
        let mouse = ui_pointer();
        let buildings: Vec<_> = if self.production_page == 0 {
            Vec::new()
        } else {
            Building::all()
                .filter_map(|building| {
                    let blocker = self.sim.building_blocker(city, building);
                    (blocker.is_none() || city.buildings.contains(&building))
                        .then_some((building, blocker))
                })
                .collect()
        };
        let last_page = buildings.len().div_ceil(capacity).max(1);
        let page = self.production_page.min(last_page);
        let unit_pages = choices.len().div_ceil(capacity).max(1);
        let unit_page = self.unit_production_page.min(unit_pages - 1);
        for (offset, width, label, action, enabled) in [
            (
                0.0,
                72.0,
                if page == 0 { "Buildings" } else { "Units" },
                UiAction::ProductionPage(usize::from(page == 0)),
                true,
            ),
            (
                79.0,
                39.0,
                "<",
                if page == 0 {
                    UiAction::UnitProductionPage(unit_page.saturating_sub(1))
                } else {
                    UiAction::ProductionPage(page - 1)
                },
                if page == 0 { unit_page > 0 } else { page > 1 },
            ),
            (
                125.0,
                39.0,
                ">",
                if page == 0 {
                    UiAction::UnitProductionPage(unit_page + 1)
                } else {
                    UiAction::ProductionPage(page + 1)
                },
                if page == 0 {
                    unit_page + 1 < unit_pages
                } else {
                    page < last_page
                },
            ),
        ] {
            let button = Rect::new(
                rect.x + rect.w - 186.0 + offset,
                rect.y + 169.0,
                width,
                25.0,
            );
            draw_ui_button(button, label, enabled, false);
            if enabled {
                actions.push((button, action));
            }
        }
        if page > 0 {
            draw_text(
                &format!("Buildings {page}/{last_page}"),
                rect.x + 216.0,
                rect.y + 187.0,
                15.0,
                LIGHTGRAY,
            );
            if buildings.is_empty() {
                draw_text(
                    "No buildings available in this city.",
                    rect.x + 22.0,
                    rect.y + 230.0,
                    16.0,
                    LIGHTGRAY,
                );
            }
            for (index, (building, blocker)) in buildings
                .into_iter()
                .skip((page - 1) * capacity)
                .take(capacity)
                .enumerate()
            {
                let card = Rect::new(
                    rect.x + 22.0 + (index % columns) as f32 * (card_w + gap),
                    rect.y + 199.0 + (index / columns) as f32 * (card_h + gap),
                    card_w,
                    card_h,
                );
                let data = building.data();
                let target = Production::Building(building);
                let enabled = blocker.is_none() && self.sim.can_change_production(city, target);
                let built = city.buildings.contains(&building);
                let current = city.production == target;
                let fill = if current {
                    Color::from_rgba(101, 80, 39, 255)
                } else if enabled && card.contains(mouse) {
                    Color::from_rgba(76, 89, 80, 255)
                } else {
                    Color::from_rgba(50, 54, 49, 255)
                };
                draw_rectangle(card.x, card.y, card.w, card.h, fill);
                draw_rectangle_lines(card.x, card.y, card.w, card.h, 1.0, GRAY);
                text_in(
                    data.name,
                    card.x + 8.0,
                    card.y + 20.0,
                    15.0,
                    card.w - 16.0,
                    if enabled || built { WHITE } else { LIGHTGRAY },
                );
                let cost = if data.genus == Genus::Convert {
                    "Converts shields to gold".to_owned()
                } else {
                    format!("{} shields  /  ref. upkeep {}", data.cost, data.upkeep)
                };
                text_in(
                    &cost,
                    card.x + 8.0,
                    card.y + 38.0,
                    13.0,
                    card.w - 16.0,
                    LIGHTGRAY,
                );
                let state = blocker.unwrap_or_else(|| {
                    if current {
                        "Current".into()
                    } else {
                        "Available".into()
                    }
                });
                text_in(
                    &state,
                    card.x + 8.0,
                    card.y + 57.0,
                    13.0,
                    card.w - 92.0,
                    LIGHTGRAY,
                );
                if let Some(price) = Simulation::building_sale_price(city, building) {
                    let button = Rect::new(card.x + card.w - 69.0, card.y + 44.0, 62.0, 21.0);
                    draw_ui_button(button, &format!("Sell {price}g"), true, false);
                    actions.push((button, UiAction::SellBuilding(city.id, building)));
                }
                self.draw_queue_button(city, card, target, actions);
                if enabled {
                    actions.push((card, UiAction::Produce(target)));
                }
            }
        } else {
            draw_text(
                &format!("Units {}/{}", unit_page + 1, unit_pages),
                rect.x + 216.0,
                rect.y + 187.0,
                15.0,
                LIGHTGRAY,
            );
            for (choice_index, kind) in choices
                .into_iter()
                .skip(unit_page * capacity)
                .take(capacity)
                .enumerate()
            {
                let column = choice_index % columns;
                let row = choice_index / columns;
                let card = Rect::new(
                    rect.x + 22.0 + column as f32 * (card_w + gap),
                    rect.y + 199.0 + row as f32 * (card_h + gap),
                    card_w,
                    card_h,
                );
                let enabled = self.sim.can_change_production(city, Production::Unit(kind));
                let current = city.production == Production::Unit(kind);
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
                text_in(
                    name,
                    card.x + 62.0,
                    card.y + 26.0,
                    16.0,
                    card.w - 70.0,
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
                self.draw_queue_button(city, card, Production::Unit(kind), actions);
                if enabled {
                    actions.push((card, UiAction::Produce(Production::Unit(kind))));
                }
            }
        }

        let garrison_y = rect.bottom() - 77.0;
        let worklist_button = Rect::new(rect.x + rect.w - 251.0, garrison_y - 20.0, 104.0, 24.0);
        draw_ui_button(
            worklist_button,
            &format!("Worklist ({})", city.worklist.len()),
            true,
            false,
        );
        actions.push((worklist_button, UiAction::WorklistPage(Some(0))));
        let buy_button = Rect::new(rect.x + rect.w - 139.0, garrison_y - 20.0, 115.0, 24.0);
        let cost = self.sim.rush_cost(city);
        let affordable = cost.is_some_and(|price| price <= self.sim.players()[city.owner].gold);
        let buy_label = cost.map_or_else(
            || {
                if city.bought_production {
                    "Rushed".to_owned()
                } else {
                    "Rush".to_owned()
                }
            },
            |price| format!("Rush {price}g"),
        );
        draw_ui_button(buy_button, &buy_label, affordable, city.bought_production);
        if affordable {
            actions.push((buy_button, UiAction::RushProduction(city.id)));
        }
        draw_text(
            &format!(
                "Upkeep: {} food / {} shields / {} gold",
                support.food, support.shields, support.gold
            ),
            rect.x + 22.0,
            garrison_y + 22.0,
            14.0,
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
                    garrison_y + 31.0,
                    button_w,
                    28.0,
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
    }

    fn draw_context_panel(&self, x: f32, y: f32) {
        let compact = screen_height() < 700.0;
        let context_text = |text: &str, tx: f32, ty: f32, size: f32, color: Color| {
            let baseline = y + (ty - y) * if compact { 0.72 } else { 1.0 };
            text_in(
                text,
                tx,
                baseline,
                if compact { size.min(17.0) } else { size },
                PANEL_W - 36.0,
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
            context_text(
                unit.kind.name(),
                x,
                y + 23.0,
                25.0,
                Color::from_rgba(244, 221, 147, 255),
            );
            context_text(
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
            context_text(
                &format!("Attack {}   Defense {}", stats.attack, stats.defense),
                x,
                y + 75.0,
                18.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            context_text(
                &format!(
                    "Health {}/{}   Moves {}/{}",
                    unit.hp,
                    stats.hp,
                    unit.moves_left,
                    self.sim.unit_move_allowance(unit.owner, unit.kind)
                ),
                x,
                y + 99.0,
                18.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            context_text(
                &format!(
                    "{}   Elevation {}",
                    self.sim.tile(unit.x, unit.y).terrain.name(),
                    self.sim.tile(unit.x, unit.y).altitude,
                ),
                x,
                y + 123.0,
                17.0,
                Color::from_rgba(167, 201, 158, 255),
            );
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
            context_text(
                &state,
                x,
                y + 148.0,
                17.0,
                Color::from_rgba(145, 204, 224, 255),
            );
            let home = self
                .sim
                .cities()
                .iter()
                .find(|city| Some(city.id) == unit.home_city);
            let support = home.map_or("No home city".to_owned(), |city| {
                format!("Home city: {}", city.name)
            });
            context_text(
                &support,
                x,
                y + 168.0,
                15.0,
                Color::from_rgba(177, 177, 164, 255),
            );
            context_text(
                "Move: arrows / 2 3 4 6 7 8",
                x,
                y + 184.0,
                16.0,
                Color::from_rgba(177, 177, 164, 255),
            );
            context_text(
                "G: go-to   Space: wait   Tab: next",
                x,
                y + 207.0,
                15.0,
                Color::from_rgba(177, 177, 164, 255),
            );
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
            context_text(
                "Choose a unit below to change production.",
                x,
                y + 192.0,
                15.0,
                Color::from_rgba(177, 177, 164, 255),
            );
        } else {
            context_text(
                "YOUR CHRONICLE",
                x,
                y + 22.0,
                22.0,
                Color::from_rgba(244, 221, 147, 255),
            );
            context_text(
                "Found a city with your Settlers.",
                x,
                y + 55.0,
                17.0,
                Color::from_rgba(225, 224, 208, 255),
            );
            context_text(
                "Explore the fog and improve the land.",
                x,
                y + 80.0,
                17.0,
                Color::from_rgba(225, 224, 208, 255),
            );
            context_text(
                "Research stronger units and unite",
                x,
                y + 105.0,
                17.0,
                Color::from_rgba(225, 224, 208, 255),
            );
            context_text(
                "the rival civilizations.",
                x,
                y + 128.0,
                17.0,
                Color::from_rgba(225, 224, 208, 255),
            );
            let living = (1..self.sim.players().len())
                .filter(|&owner| owner != BARBARIANS)
                .filter(|owner| {
                    self.sim.cities().iter().any(|c| c.owner == *owner)
                        || self.sim.units().iter().any(|u| u.owner == *owner)
                })
                .count();
            context_text(
                &format!("Rivals remaining: {living}"),
                x,
                y + 172.0,
                18.0,
                Color::from_rgba(236, 183, 117, 255),
            );
            context_text(
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
                    "Found [B]".to_owned(),
                    UiAction::FoundCity,
                    self.sim.can_found_at(unit.x, unit.y),
                ));
            }
            if stats.can_work && unit.transport.is_none() {
                let tile = self.sim.tile(unit.x, unit.y);
                let (label, enabled) = if tile.road && self.sim.has_tech(unit.owner, Tech::RAILROAD)
                {
                    (
                        "Railroad [R]",
                        self.sim.can_railroad_at(unit.owner, unit.x, unit.y),
                    )
                } else {
                    (
                        "Road [R]",
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
                        "Farmland [I]",
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
                entries.push(("Set home [H]".to_owned(), UiAction::HomeCity, true));
            }
            if unit.automated
                || (unit.kind.fuel_capacity() == 0 && unit.kind != UnitKind::Helicopter)
            {
                entries.push(("Automate [A]".to_owned(), UiAction::Automate, true));
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
        } else {
            entries.push(("Next ready unit [Tab]".to_owned(), UiAction::Wait, false));
        }

        let columns = grid_columns(available_width - 24.0, 112.0);
        let capacity = columns * 2;
        let paginated = entries.len() > capacity;
        let page_size = capacity - usize::from(paginated);
        let pages = entries.len().div_ceil(page_size).max(1);
        let page = self.action_page.min(pages - 1);
        let width = (available_width - 24.0 - UI_GAP * (columns - 1) as f32) / columns as f32;
        let origin_y = screen_height() - BOTTOM_H + 8.0;
        for (slot, (label, action, enabled)) in entries
            .into_iter()
            .skip(page * page_size)
            .take(page_size)
            .enumerate()
        {
            let rect = Rect::new(
                12.0 + (slot % columns) as f32 * (width + UI_GAP),
                origin_y + (slot / columns) as f32 * 36.0,
                width,
                28.0,
            );
            draw_ui_button(rect, &label, enabled, false);
            if enabled {
                actions.push((rect, action));
            }
        }
        if paginated {
            let slot = capacity - 1;
            let rect = Rect::new(
                12.0 + (slot % columns) as f32 * (width + UI_GAP),
                origin_y + 36.0,
                width,
                28.0,
            );
            draw_ui_button(rect, &format!("More {}/{} >", page + 1, pages), true, false);
            actions.push((rect, UiAction::ActionPage((page + 1) % pages)));
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
            let clipped = fit_text(message, 15.0, available_width - 36.0);
            draw_text(&clipped, 18.0, y, 15.0, color);
            y -= 19.0;
        }
    }
}
