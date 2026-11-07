use super::*;

impl Game {
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
                format!("Buildings {page}/{last_page}"),
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
                format!("Units {}/{}", unit_page + 1, unit_pages),
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
                        source: Some(isolated_sprite_source(sprite_row, sprite_col, 64.0, 48.0)),
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
                    format!("{} shields", stats.cost),
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
            format!(
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
            format!("WORKLIST  {}/{}", city.worklist.len(), WORKLIST_LIMIT),
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
}
