//! Automated city worker assignment and production management.

use super::*;

const CITY_OUTER_PAD: f32 = 18.0;
const CITY_PANEL_PAD: f32 = 12.0;
const CITY_PANEL_GAP: f32 = 12.0;
const CITY_SIDEBAR_TOP_H: f32 = 144.0;

/// Shared geometry keeps the map, local religion and production areas disjoint.
struct CityViewLayout {
    buildings: Rect,
    map: Rect,
    religion: Rect,
    production: Rect,
}
impl CityViewLayout {
    fn new(rect: Rect) -> Self {
        let inner = Rect::new(
            rect.x + CITY_OUTER_PAD,
            rect.y + 130.0,
            rect.w - 2.0 * CITY_OUTER_PAD,
            rect.h - 130.0 - CITY_OUTER_PAD,
        );
        let production_h = 172.0;
        let production = Rect::new(
            inner.x,
            inner.bottom() - production_h,
            inner.w,
            production_h,
        );
        let sidebar_w = (inner.w * 0.19).clamp(146.0, 220.0).round();
        let buildings = Rect::new(
            inner.x,
            inner.y,
            sidebar_w,
            production.y - inner.y - CITY_PANEL_GAP,
        );
        let map = Rect::new(
            buildings.right() + CITY_PANEL_GAP,
            inner.y,
            inner.w - 2.0 * sidebar_w - 2.0 * CITY_PANEL_GAP,
            production.y - inner.y - CITY_PANEL_GAP,
        );
        let religion = Rect::new(map.right() + CITY_PANEL_GAP, inner.y, sidebar_w, map.h);
        Self {
            buildings,
            map,
            religion,
            production,
        }
    }
}

fn city_sidebar_sections(column: Rect) -> (Rect, Rect) {
    let top = Rect::new(column.x, column.y, column.w, CITY_SIDEBAR_TOP_H);
    let bottom = Rect::new(
        column.x,
        top.bottom() + CITY_PANEL_GAP,
        column.w,
        column.h - CITY_SIDEBAR_TOP_H - CITY_PANEL_GAP,
    );
    (top, bottom)
}

impl Game {
    pub(super) fn draw_city_manager(&self, assets: &Assets, actions: &mut Vec<(Rect, UiAction)>) {
        let Some(city_id) = self.selected_city else {
            return;
        };
        let Some(city) = self.sim.city_manager_view(city_id) else {
            return;
        };
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
            rect.x + CITY_OUTER_PAD,
            rect.y + 35.0,
            26.0,
            (rect.w - 196.0).max(0.0),
            Color::from_rgba(248, 222, 139, 255),
        );
        let close_rect = Rect::new(
            rect.right() - CITY_OUTER_PAD - 28.0,
            rect.y + CITY_OUTER_PAD,
            28.0,
            28.0,
        );
        draw_ui_button(close_rect, "X", true, false);
        actions.push((close_rect, UiAction::CloseCity));
        text_in(
            &format!(
                "Size {} · Food {:+} · Shields {} · Science {} · Gold {:+} · Luxury {}",
                city.size, city.food_net, city.shield_net, city.science, city.gold_net, city.luxury
            ),
            rect.x + CITY_OUTER_PAD,
            rect.y + 61.0,
            16.0,
            rect.w - 2.0 * CITY_OUTER_PAD,
            Color::from_rgba(188, 220, 166, 255),
        );
        let growth_box = city.growth_box;
        let half = (rect.w - 2.0 * CITY_OUTER_PAD - CITY_PANEL_GAP) / 2.0;
        let growth = if city.disorder {
            "Civil disorder".to_owned()
        } else if city.growth_limited {
            "Growth limit reached".to_owned()
        } else {
            format!("Growth {}/{}", city.food, growth_box)
        };
        text_in(
            &growth,
            rect.x + CITY_OUTER_PAD,
            rect.y + 87.0,
            15.0,
            half,
            LIGHTGRAY,
        );
        let culture_label = match city.next_border_threshold {
            Some(target) => format!(
                "Culture {}/{} (+{}/turn)",
                city.culture, target, city.culture_per_turn
            ),
            None => format!(
                "Culture {} (+{}/turn) · Maximum borders",
                city.culture, city.culture_per_turn
            ),
        };
        text_in(
            &culture_label,
            rect.x + CITY_OUTER_PAD + CITY_PANEL_GAP + half,
            rect.y + 87.0,
            15.0,
            half,
            Color::from_rgba(206, 177, 234, 255),
        );
        draw_meter(
            Rect::new(rect.x + CITY_OUTER_PAD, rect.y + 96.0, half, 10.0),
            city.food,
            growth_box,
            Color::from_rgba(82, 166, 82, 255),
        );
        let culture_fraction = city.next_border_threshold.map_or(1.0, |target| {
            (city.culture as f64 / target.max(1) as f64).clamp(0.0, 1.0)
        });
        draw_meter(
            Rect::new(
                rect.x + CITY_OUTER_PAD + CITY_PANEL_GAP + half,
                rect.y + 96.0,
                half,
                10.0,
            ),
            (culture_fraction * 1000.0).round() as i32,
            1000,
            Color::from_rgba(159, 109, 191, 255),
        );
        draw_line(
            rect.x + CITY_OUTER_PAD,
            rect.y + 118.0,
            rect.right() - CITY_OUTER_PAD,
            rect.y + 118.0,
            1.0,
            Color::from_rgba(105, 102, 86, 255),
        );
        if city.owner == HUMAN {
            let button = Rect::new(
                close_rect.x - CITY_PANEL_GAP - 109.0,
                rect.y + CITY_OUTER_PAD,
                109.0,
                28.0,
            );
            draw_ui_button(
                button,
                if city.governor.active {
                    "Governor: On"
                } else {
                    "Governor"
                },
                true,
                self.city_governor_page,
            );
            actions.push((button, UiAction::CityGovernorPage));
            if self.city_governor_page {
                self.draw_city_governor(&city, rect, actions);
                return;
            }
        }
        let CityViewLayout {
            buildings,
            map,
            religion,
            production,
        } = CityViewLayout::new(rect);
        let (specialist_panel, built_panel) = city_sidebar_sections(buildings);
        self.draw_city_specialists(&city, specialist_panel, actions);
        self.draw_city_buildings(&city, built_panel, actions);
        self.draw_city_tiles_map(assets, &city, map, actions);
        let (faith_panel, luxury_panel) = city_sidebar_sections(religion);
        self.draw_city_religion(&city, faith_panel);
        self.draw_city_luxuries(assets, &city, luxury_panel);
        draw_rectangle(
            production.x,
            production.y,
            production.w,
            production.h,
            Color::from_rgba(31, 35, 31, 255),
        );
        draw_rectangle_lines(
            production.x,
            production.y,
            production.w,
            production.h,
            1.0,
            Color::from_rgba(105, 102, 86, 255),
        );
        if let Some(page) = self.worklist_page {
            // The queue replaces only the production strip; the map stays visible.
            self.draw_worklist(&city, production, page, actions);
        } else {
            self.draw_city_production(assets, &city, production, actions);
        }
    }

    fn draw_city_specialists(
        &self,
        city: &CityManagerView,
        panel: Rect,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        draw_rectangle(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            Color::from_rgba(31, 35, 31, 255),
        );
        draw_rectangle_lines(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            1.0,
            Color::from_rgba(105, 102, 86, 255),
        );
        text_in(
            "SPECIALISTS",
            panel.x + CITY_PANEL_PAD,
            panel.y + 21.0,
            14.0,
            panel.w - 2.0 * CITY_PANEL_PAD,
            GOLD,
        );
        for (index, label) in ["Scientists", "Taxmen", "Entertainers"]
            .into_iter()
            .enumerate()
        {
            let y = panel.y + 23.0 + index as f32 * 36.0;
            text_in(
                label,
                panel.x + CITY_PANEL_PAD,
                y + 10.0,
                13.0,
                panel.w - 2.0 * CITY_PANEL_PAD,
                LIGHTGRAY,
            );
            let count = city.specialists[index];
            for (delta, x, label, enabled) in [
                (-1, panel.x + CITY_PANEL_PAD, "-", count > 0),
                (
                    1,
                    panel.right() - CITY_PANEL_PAD - 26.0,
                    "+",
                    city.tile_workers > 0,
                ),
            ] {
                let enabled = enabled && !city.governor.active;
                let button = Rect::new(x, y + 15.0, 26.0, 22.0);
                draw_ui_button(button, label, enabled, false);
                if enabled {
                    actions.push((button, UiAction::Specialist(city.id, index, delta)));
                }
            }
            let count = count.to_string();
            let width = measure_text(&count, None, 13, 1.0).width;
            draw_text(
                &count,
                panel.x + (panel.w - width) * 0.5,
                y + 30.0,
                13.0,
                LIGHTGRAY,
            );
        }
    }

    fn draw_city_buildings(
        &self,
        city: &CityManagerView,
        panel: Rect,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        draw_rectangle(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            Color::from_rgba(31, 35, 31, 255),
        );
        draw_rectangle_lines(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            1.0,
            Color::from_rgba(105, 102, 86, 255),
        );
        let built: Vec<_> = city
            .buildings
            .iter()
            .filter(|building| building.built)
            .collect();
        text_in(
            &format!("BUILDINGS ({})", built.len()),
            panel.x + CITY_PANEL_PAD,
            panel.y + 21.0,
            14.0,
            panel.w - 2.0 * CITY_PANEL_PAD,
            GOLD,
        );
        if built.is_empty() {
            text_in(
                "None built yet",
                panel.x + CITY_PANEL_PAD,
                panel.y + 48.0,
                13.0,
                panel.w - 2.0 * CITY_PANEL_PAD,
                LIGHTGRAY,
            );
            return;
        }
        let row_h = if panel.h < 90.0 { 22.0 } else { 28.0 };
        let rows = ((panel.h - 72.0) / row_h).floor().max(1.0) as usize;
        let pages = built.len().div_ceil(rows);
        let page = self.city_buildings_page.min(pages - 1);
        for (index, building) in built.iter().skip(page * rows).take(rows).enumerate() {
            let y = panel.y + if panel.h < 90.0 { 26.0 } else { 36.0 } + index as f32 * row_h;
            let sell_width = if building.sale_price.is_some() {
                52.0
            } else {
                0.0
            };
            let text_width = panel.w
                - 2.0 * CITY_PANEL_PAD
                - sell_width
                - if sell_width > 0.0 {
                    CITY_PANEL_GAP
                } else {
                    0.0
                };
            text_in(
                building.name,
                panel.x + CITY_PANEL_PAD,
                y + 14.0,
                13.0,
                text_width,
                LIGHTGRAY,
            );
            if building.sale_price.is_some() {
                let button = Rect::new(
                    panel.right() - CITY_PANEL_PAD - sell_width,
                    y - 2.0,
                    sell_width,
                    20.0,
                );
                draw_ui_button(button, "Sell", true, false);
                actions.push((button, UiAction::SellBuilding(city.id, building.building)));
            }
        }
        if pages > 1 {
            let button_h = if panel.h < 90.0 { 18.0 } else { 24.0 };
            let y = panel.bottom() - CITY_PANEL_PAD - button_h;
            for (x, label, target, enabled) in [
                (
                    panel.x + CITY_PANEL_PAD,
                    "<",
                    page.saturating_sub(1),
                    page > 0,
                ),
                (
                    panel.right() - CITY_PANEL_PAD - 26.0,
                    ">",
                    page + 1,
                    page + 1 < pages,
                ),
            ] {
                let button = Rect::new(x, y, 26.0, button_h);
                draw_ui_button(button, label, enabled, false);
                if enabled {
                    actions.push((button, UiAction::CityBuildingsPage(target)));
                }
            }
            let label = format!("{}/{}", page + 1, pages);
            let width = measure_text(&label, None, 12, 1.0).width;
            draw_text(
                &label,
                panel.x + (panel.w - width) * 0.5,
                y + button_h * 0.5 + 4.0,
                12.0,
                LIGHTGRAY,
            );
        }
    }

    fn draw_city_luxuries(&self, assets: &Assets, city: &CityManagerView, panel: Rect) {
        draw_rectangle(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            Color::from_rgba(31, 35, 31, 255),
        );
        draw_rectangle_lines(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            1.0,
            Color::from_rgba(105, 102, 86, 255),
        );
        text_in(
            "LUXURIES",
            panel.x + CITY_PANEL_PAD,
            panel.y + 21.0,
            14.0,
            panel.w - 2.0 * CITY_PANEL_PAD,
            GOLD,
        );
        if city.luxuries.is_empty() {
            text_in(
                "None connected",
                panel.x + CITY_PANEL_PAD,
                panel.y + 65.0,
                13.0,
                panel.w - 2.0 * CITY_PANEL_PAD,
                LIGHTGRAY,
            );
            return;
        }
        let cell_w = (panel.w - 2.0 * CITY_PANEL_PAD) / 4.0;
        let row_h = (panel.h - 48.0) / 2.0;
        let mut tooltip = None;
        for (index, &(resource, name)) in city.luxuries.iter().enumerate() {
            let cell = Rect::new(
                panel.x + CITY_PANEL_PAD + (index % 4) as f32 * cell_w,
                panel.y + 48.0 + (index / 4) as f32 * row_h,
                cell_w,
                row_h,
            );
            assets.civ3.draw_resource(
                resource,
                cell.center(),
                vec2((cell.w.min(cell.h) - 4.0).clamp(12.0, 30.0) / 0.44, 40.0),
            );
            if cell.contains(ui_pointer()) {
                tooltip = Some(name);
            }
        }
        text_in(
            tooltip.unwrap_or("Connected to city"),
            panel.x + CITY_PANEL_PAD,
            panel.y + 39.0,
            12.0,
            panel.w - 2.0 * CITY_PANEL_PAD,
            LIGHTGRAY,
        );
    }

    fn draw_city_religion(&self, city: &CityManagerView, panel: Rect) {
        draw_rectangle(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            Color::from_rgba(31, 35, 31, 255),
        );
        draw_rectangle_lines(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            1.0,
            Color::from_rgba(105, 102, 86, 255),
        );
        text_in(
            "RELIGION",
            panel.x + CITY_PANEL_PAD,
            panel.y + 21.0,
            14.0,
            panel.w - 2.0 * CITY_PANEL_PAD,
            GOLD,
        );
        let shares = self.sim.city_faith_shares(city.id);
        if shares.is_empty() {
            text_in(
                "Unavailable",
                panel.x + CITY_PANEL_PAD,
                panel.y + 46.0,
                13.0,
                panel.w - 2.0 * CITY_PANEL_PAD,
                LIGHTGRAY,
            );
            return;
        }
        let row_h = ((panel.h - 40.0) / shares.len() as f32).min(40.0);
        let font_size = (row_h - 2.0).clamp(9.0, 13.0);
        for (index, share) in shares.iter().enumerate() {
            let y = panel.y + 38.0 + index as f32 * row_h;
            let color = if share.dominant { GOLD } else { LIGHTGRAY };
            text_in(
                &share.name,
                panel.x + CITY_PANEL_PAD,
                y + font_size,
                font_size,
                panel.w - 68.0,
                color,
            );
            let percentage = format!("{}%", share.percent);
            let metrics = measure_text(&percentage, None, font_size as u16, 1.0);
            draw_text(
                &percentage,
                panel.right() - CITY_PANEL_PAD - metrics.width,
                y + font_size,
                font_size,
                color,
            );
            if row_h >= 26.0 {
                draw_meter(
                    Rect::new(
                        panel.x + CITY_PANEL_PAD,
                        y + 18.0,
                        panel.w - 2.0 * CITY_PANEL_PAD,
                        4.0,
                    ),
                    share.percent,
                    100,
                    color,
                );
            }
        }
    }

    fn draw_city_production(
        &self,
        assets: &Assets,
        city: &CityManagerView,
        rect: Rect,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        let choices: Vec<_> = city.units.iter().filter(|unit| unit.available).collect();
        let columns = grid_columns(rect.w - 2.0 * CITY_PANEL_PAD, 180.0);
        let gap = CITY_PANEL_GAP;
        let card_w = (rect.w - 2.0 * CITY_PANEL_PAD - gap * (columns - 1) as f32) / columns as f32;
        let card_h = 72.0;
        let rows = ((rect.h - 78.0) / (card_h + gap)).floor().max(1.0) as usize;
        let capacity = columns * rows;
        let mouse = ui_pointer();
        let buildings: Vec<_> = if self.production_page == 0 {
            Vec::new()
        } else {
            city.buildings
                .iter()
                .filter(|building| !building.built && building.blocker.is_none())
                .collect()
        };
        let last_page = buildings.len().div_ceil(capacity).max(1);
        let page = self.production_page.min(last_page);
        let unit_pages = choices.len().div_ceil(capacity).max(1);
        let unit_page = self.unit_production_page.min(unit_pages - 1);
        for (offset, label, tab, selected) in [
            (CITY_PANEL_PAD, "Units", 0, page == 0),
            (
                CITY_PANEL_PAD + 78.0 + CITY_PANEL_GAP,
                "Buildings",
                1,
                page > 0,
            ),
        ] {
            let button = Rect::new(rect.x + offset, rect.y + CITY_PANEL_PAD, 78.0, 26.0);
            draw_ui_button(button, label, true, selected);
            actions.push((button, UiAction::ProductionPage(tab)));
        }
        text_in(
            &format!(
                "{} · {}/{} shields",
                city.production_name, city.shields, city.production_cost
            ),
            rect.x + 2.0 * CITY_PANEL_PAD + 2.0 * 78.0 + CITY_PANEL_GAP,
            rect.y + 30.0,
            13.0,
            (rect.w - 320.0).max(0.0),
            LIGHTGRAY,
        );
        let page_label = if page == 0 {
            format!("{}/{}", unit_page + 1, unit_pages)
        } else {
            format!("{page}/{last_page}")
        };
        text_in(
            &page_label,
            rect.right() - 128.0,
            rect.y + 30.0,
            13.0,
            44.0,
            LIGHTGRAY,
        );
        for (offset, label, action, enabled) in [
            (
                CITY_PANEL_PAD + 2.0 * 26.0 + CITY_PANEL_GAP,
                "<",
                if page == 0 {
                    UiAction::UnitProductionPage(unit_page.saturating_sub(1))
                } else {
                    UiAction::ProductionPage(page.saturating_sub(1))
                },
                if page == 0 { unit_page > 0 } else { page > 1 },
            ),
            (
                CITY_PANEL_PAD + 26.0,
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
            let button = Rect::new(rect.right() - offset, rect.y + CITY_PANEL_PAD, 26.0, 26.0);
            draw_ui_button(button, label, enabled, false);
            if enabled {
                actions.push((button, action));
            }
        }
        if page > 0 {
            if buildings.is_empty() {
                draw_text(
                    "No buildings available in this city.",
                    rect.x + CITY_PANEL_PAD,
                    rect.y + 67.0,
                    16.0,
                    LIGHTGRAY,
                );
            }
            for (index, building_view) in buildings
                .into_iter()
                .skip((page - 1) * capacity)
                .take(capacity)
                .enumerate()
            {
                let building = building_view.building;
                let card = Rect::new(
                    rect.x + CITY_PANEL_PAD + (index % columns) as f32 * (card_w + gap),
                    rect.y + 50.0 + (index / columns) as f32 * (card_h + gap),
                    card_w,
                    card_h,
                );
                let target = Production::Building(building);
                let enabled = building_view.can_change;
                let current = building_view.current;
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
                    building_view.name,
                    card.x + 8.0,
                    card.y + 20.0,
                    15.0,
                    card.w - 16.0,
                    if enabled { WHITE } else { LIGHTGRAY },
                );
                text_in(
                    &building_view.cost_label,
                    card.x + 8.0,
                    card.y + 38.0,
                    13.0,
                    card.w - 16.0,
                    LIGHTGRAY,
                );
                let state = building_view.blocker.clone().unwrap_or_else(|| {
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
                if enabled {
                    actions.push((card, UiAction::Produce(target)));
                }
                self.draw_queue_button(city, card, target, building_view.can_queue, actions);
            }
        } else {
            for (choice_index, unit_view) in choices
                .into_iter()
                .skip(unit_page * capacity)
                .take(capacity)
                .enumerate()
            {
                let kind = unit_view.kind;
                let column = choice_index % columns;
                let row = choice_index / columns;
                let card = Rect::new(
                    rect.x + CITY_PANEL_PAD + column as f32 * (card_w + gap),
                    rect.y + 50.0 + row as f32 * (card_h + gap),
                    card_w,
                    card_h,
                );
                let enabled = unit_view.can_change;
                let current = unit_view.current;
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
                let cost = unit_view.cost;
                assets.civ3.draw_unit_icon(
                    kind,
                    city.owner,
                    self.sim.player_team_palette_view(city.owner),
                    city.era_index,
                    vec2(card.x + 34.0, card.y + 32.5),
                    vec2(52.0, 39.0),
                    enabled,
                );
                let name = short_unit_name(kind);
                text_in(
                    name,
                    card.x + 68.0,
                    card.y + 26.0,
                    16.0,
                    card.w - 76.0,
                    if enabled {
                        Color::from_rgba(239, 235, 216, 255)
                    } else {
                        Color::from_rgba(111, 114, 107, 255)
                    },
                );
                draw_text(
                    format!("{} shields", cost),
                    card.x + 68.0,
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
                        card.x + 8.0,
                        card.bottom() - 8.0,
                        11.0,
                        Color::from_rgba(247, 211, 112, 255),
                    );
                }
                if enabled {
                    actions.push((card, UiAction::Produce(Production::Unit(kind))));
                }
                self.draw_queue_button(
                    city,
                    card,
                    Production::Unit(kind),
                    unit_view.can_queue,
                    actions,
                );
            }
        }

        let footer_y = rect.bottom() - CITY_PANEL_PAD - 26.0;
        let worklist_button = Rect::new(
            rect.right() - CITY_PANEL_PAD - 115.0 - CITY_PANEL_GAP - 104.0,
            footer_y,
            104.0,
            26.0,
        );
        draw_ui_button(
            worklist_button,
            &format!("Worklist ({})", city.worklist.len()),
            true,
            false,
        );
        actions.push((worklist_button, UiAction::WorklistPage(Some(0))));
        let buy_button = Rect::new(rect.right() - CITY_PANEL_PAD - 115.0, footer_y, 115.0, 26.0);
        let affordable = city.rush_cost.is_some_and(|price| price <= city.treasury);
        let buy_label = city.rush_cost.map_or_else(
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
    }

    fn draw_worklist(
        &self,
        city: &CityManagerView,
        panel: Rect,
        requested_page: usize,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        let rows = ((panel.h - 62.0) / 34.0).floor().max(1.0) as usize;
        let last_page = city.worklist.len().saturating_sub(1) / rows;
        let page = requested_page.min(last_page);
        draw_text(
            format!("WORKLIST  {}/{}", city.worklist.len(), WORKLIST_LIMIT),
            panel.x + CITY_PANEL_PAD,
            panel.y + 30.0,
            18.0,
            WHITE,
        );
        for (offset, width, label, target, enabled) in [
            (184.0, 28.0, "<", Some(page.saturating_sub(1)), page > 0),
            (144.0, 28.0, ">", Some(page + 1), page < last_page),
            (104.0, 92.0, "Production", None, true),
        ] {
            let button = Rect::new(
                panel.x + panel.w - offset,
                panel.y + CITY_PANEL_PAD,
                width,
                26.0,
            );
            draw_ui_button(button, label, enabled, false);
            if enabled {
                actions.push((button, UiAction::WorklistPage(target)));
            }
        }
        if city.worklist.is_empty() {
            draw_text(
                "Use + on a production card to add it to the worklist.",
                panel.x + CITY_PANEL_PAD,
                panel.y + 63.0,
                16.0,
                LIGHTGRAY,
            );
        }
        for (index, entry) in city
            .worklist
            .iter()
            .enumerate()
            .skip(page * rows)
            .take(rows)
        {
            let y = panel.y + 50.0 + (index % rows) as f32 * 34.0;
            let target = entry.target;
            let status = if !entry.available {
                "  (unavailable)".to_owned()
            } else if target != entry.production {
                format!("  (upgrades to {})", entry.target_name)
            } else {
                String::new()
            };
            text_in(
                &format!("{}. {}{status}", index + 1, entry.name),
                panel.x + CITY_PANEL_PAD,
                y + 19.0,
                16.0,
                panel.w - 158.0,
                WHITE,
            );
            for (offset, label, edit, enabled) in [
                (
                    132.0,
                    "^",
                    WorklistEdit::Move(index, index.saturating_sub(1)),
                    index > 0,
                ),
                (
                    88.0,
                    "v",
                    WorklistEdit::Move(index, index + 1),
                    index + 1 < city.worklist.len(),
                ),
                (44.0, "X", WorklistEdit::Remove(index), true),
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
        city: &CityManagerView,
        card: Rect,
        target: Production,
        can_queue: bool,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        if can_queue {
            let button = Rect::new(
                card.right() - 8.0 - 22.0,
                card.bottom() - 8.0 - 18.0,
                22.0,
                18.0,
            );
            draw_ui_button(button, "+", true, false);
            actions.push((
                button,
                UiAction::EditWorklist(city.id, WorklistEdit::Append(target)),
            ));
        }
    }
}

impl Game {
    fn draw_city_governor(
        &self,
        city: &CityManagerView,
        rect: Rect,
        actions: &mut Vec<(Rect, UiAction)>,
    ) {
        let state = city.governor;
        let x = rect.x + 22.0;
        let right = rect.right() - 22.0;
        let toggle = Rect::new(right - 126.0, rect.y + 166.0, 126.0, 26.0);
        let can_control = state.active || self.city_governor_preview.is_some();
        draw_ui_button(
            toggle,
            if state.active {
                "Release city"
            } else {
                "Control city"
            },
            can_control,
            state.active,
        );
        if can_control {
            actions.push((
                toggle,
                UiAction::SetCityGovernor(
                    city.id,
                    crate::sim::CityGovernor {
                        active: !state.active,
                        ..state
                    },
                ),
            ));
        }
        text_in(
            if state.active {
                "Automatic citizen assignments"
            } else if self.city_governor_preview.is_none() {
                "No assignment meets these requirements"
            } else {
                "Manual citizen assignments"
            },
            x,
            rect.y + 185.0,
            16.0,
            (toggle.x - x - 8.0).max(0.0),
            LIGHTGRAY,
        );

        let presets = crate::sim::GovernorSettings::PRESETS;
        let preset_w = (right - x - 24.0) / presets.len() as f32;
        for (i, (label, settings)) in presets.into_iter().enumerate() {
            let button = Rect::new(
                x + i as f32 * (preset_w + 6.0),
                rect.y + 200.0,
                preset_w,
                25.0,
            );
            draw_ui_button(button, label, true, state.settings == settings);
            actions.push((
                button,
                UiAction::SetCityGovernor(city.id, crate::sim::CityGovernor { settings, ..state }),
            ));
        }

        let column = (right - x) / 4.0;
        text_in(
            "Projected",
            x + 3.0 * column,
            rect.y + 249.0,
            15.0,
            column,
            LIGHTGRAY,
        );
        text_in(
            "Minimum surplus",
            x + column,
            rect.y + 249.0,
            15.0,
            column,
            LIGHTGRAY,
        );
        text_in(
            "Priority",
            x + 2.0 * column,
            rect.y + 249.0,
            15.0,
            column,
            LIGHTGRAY,
        );
        let row_h = ((rect.h - 270.0) / 8.0).clamp(16.0, 36.0);
        for (i, label) in [
            "Food",
            "Production",
            "Trade",
            "Gold",
            "Luxury",
            "Science",
            "Celebration",
        ]
        .into_iter()
        .enumerate()
        {
            let y = rect.y + 255.0 + i as f32 * row_h;
            let button_h = (row_h - 2.0).min(25.0);
            text_in(label, x, y + 16.0, 15.0, column, LIGHTGRAY);
            if i < 6 {
                let projected = self
                    .city_governor_preview
                    .map_or_else(|| "—".to_owned(), |outputs| format!("{:+}", outputs[i]));
                text_in(
                    &projected,
                    x + 3.0 * column,
                    y + 16.0,
                    15.0,
                    column,
                    LIGHTGRAY,
                );
            }
            for (is_weight, cell_x) in [(false, x + column), (true, x + 2.0 * column)] {
                if i == 6 && !is_weight {
                    let button = Rect::new(cell_x, y, column.min(150.0) - 6.0, button_h);
                    let mut next = state;
                    next.settings.require_celebration = !state.settings.require_celebration;
                    draw_ui_button(
                        button,
                        if state.settings.require_celebration {
                            "Required"
                        } else {
                            "Optional"
                        },
                        true,
                        state.settings.require_celebration,
                    );
                    actions.push((button, UiAction::SetCityGovernor(city.id, next)));
                    continue;
                }
                let (value, min, max) = if i == 6 {
                    (state.settings.celebration_weight, 0, 50)
                } else if is_weight {
                    (state.settings.weight[i], 0, 25)
                } else {
                    (state.settings.minimum[i], -20, 20)
                };
                let width = column.min(150.0) - 6.0;
                let button_w = (width / 4.0).min(30.0);
                for (delta, button_x, symbol) in
                    [(-1, cell_x, "-"), (1, cell_x + width - button_w, "+")]
                {
                    let button = Rect::new(button_x, y, button_w, button_h);
                    let changed = value + delta;
                    let enabled = (min..=max).contains(&changed)
                        && (i != 0 || is_weight || !state.settings.fastest_growth);
                    draw_ui_button(button, symbol, enabled, false);
                    if enabled {
                        let mut next = state;
                        if i == 6 {
                            next.settings.celebration_weight = changed;
                        } else if is_weight {
                            next.settings.weight[i] = changed;
                        } else {
                            next.settings.minimum[i] = changed;
                        }
                        actions.push((button, UiAction::SetCityGovernor(city.id, next)));
                    }
                }
                let label = if i == 0 && !is_weight && state.settings.fastest_growth {
                    "Auto".to_owned()
                } else {
                    value.to_string()
                };
                text_in(
                    &label,
                    cell_x + button_w + 5.0,
                    y + 16.0,
                    15.0,
                    width - 2.0 * button_w - 8.0,
                    LIGHTGRAY,
                );
            }
        }
        let growth = Rect::new(
            x,
            rect.y + 255.0 + 7.0 * row_h,
            175.0,
            (row_h - 2.0).min(25.0),
        );
        let mut next = state;
        next.settings.fastest_growth = !state.settings.fastest_growth;
        draw_ui_button(
            growth,
            "Fastest growth",
            true,
            state.settings.fastest_growth,
        );
        actions.push((growth, UiAction::SetCityGovernor(city.id, next)));
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn city_regions_fit_without_overlap_at_supported_window_sizes() {
        for (width, height) in [(768.0, 568.0), (1248.0, 688.0), (1440.0, 868.0)] {
            let panel = Rect::new(16.0, 16.0, width, height);
            let layout = CityViewLayout::new(panel);
            for region in [
                layout.buildings,
                layout.map,
                layout.religion,
                layout.production,
            ] {
                assert!(region.w > 0.0 && region.h > 0.0);
                assert!(region.x >= panel.x && region.right() <= panel.right());
                assert!(region.y >= panel.y && region.bottom() <= panel.bottom());
            }
            assert!(layout.buildings.right() < layout.map.x);
            assert!(layout.map.right() < layout.religion.x);
            assert!(layout.map.bottom() < layout.production.y);
            assert!(layout.religion.bottom() < layout.production.y);
            assert!(layout.map.w > layout.religion.w * 2.0);
            assert_eq!(layout.production.x, layout.buildings.x);
            assert_eq!(layout.production.right(), layout.religion.right());
            assert_eq!(layout.map.y, layout.buildings.y);
            assert_eq!(layout.map.y, layout.religion.y);
            assert_eq!(layout.map.bottom(), layout.religion.bottom());
            let (left_top, left_bottom) = city_sidebar_sections(layout.buildings);
            let (right_top, right_bottom) = city_sidebar_sections(layout.religion);
            assert_eq!(left_top.bottom(), right_top.bottom());
            assert_eq!(left_bottom.y, right_bottom.y);
            assert_eq!(left_bottom.bottom(), layout.map.bottom());
            assert!(left_bottom.h >= 64.0 && right_bottom.h >= 64.0);
            assert!(layout.production.h >= 172.0);
        }
    }
}
