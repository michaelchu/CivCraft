//! Map context menus for city garrisons and unit stacks.

use super::*;

const MENU_WIDTH: f32 = 224.0;
const MENU_HEADER_HEIGHT: f32 = 28.0;
const MENU_ROW_HEIGHT: f32 = 34.0;

impl Game {
    pub(super) fn draw_stack_menu(&self) -> Vec<(Rect, UiAction)> {
        let Some(menu) = &self.stack_menu else {
            return Vec::new();
        };
        let width = MENU_WIDTH.min((screen_width() - 16.0).max(1.0));
        let is_city = menu.city_id.is_some();
        let same_kind = if is_city {
            None
        } else {
            self.selected_unit.and_then(|selected_id| {
                let selected_kind = self.sim.unit_kind_view(selected_id)?;
                menu.units.contains(&selected_id).then(|| {
                    menu.units
                        .iter()
                        .copied()
                        .filter(|id| {
                            self.sim
                                .unit_kind_view(*id)
                                .is_some_and(|kind| kind == selected_kind)
                        })
                        .collect::<Vec<_>>()
                })
            })
        };
        let footer_rows = if is_city {
            1
        } else {
            1 + usize::from(same_kind.is_some())
        };
        let padding = if is_city { 10.0 } else { 14.0 };
        let max_height = (screen_height() - 8.0).max(1.0);
        let max_unit_rows =
            ((max_height - MENU_HEADER_HEIGHT - MENU_ROW_HEIGHT * footer_rows as f32 - padding)
                / MENU_ROW_HEIGHT)
                .floor()
                .max(0.0) as usize;
        let visible_rows = menu.units.len().min(max_unit_rows);
        let max_offset = menu.units.len().saturating_sub(visible_rows);
        let offset = menu.scroll_offset.min(max_offset);
        let height =
            (MENU_HEADER_HEIGHT + MENU_ROW_HEIGHT * (visible_rows + footer_rows) as f32 + padding)
                .min(max_height);
        let x = menu
            .position
            .x
            .clamp(4.0, (screen_width() - width - 4.0).max(4.0));
        let y = menu
            .position
            .y
            .clamp(4.0, (screen_height() - height - 4.0).max(4.0));
        let title = if let Some(city_id) = menu.city_id {
            self.sim
                .city_name_view(city_id)
                .unwrap_or_else(|| "CITY".to_owned())
        } else {
            format!("UNIT STACK · {}", menu.units.len())
        };
        let title = if menu.units.len() > visible_rows {
            format!("{title} (scroll)")
        } else {
            title
        };
        let panel = self.draw_menu_panel(Rect::new(x, y, width, height), title);

        let mut actions = self.draw_unit_rows(
            panel,
            panel.y + MENU_HEADER_HEIGHT,
            offset,
            visible_rows,
            &menu.units,
        );
        let divider_y = panel.y + MENU_HEADER_HEIGHT + MENU_ROW_HEIGHT * visible_rows as f32;
        self.draw_menu_divider(panel, divider_y);
        let button = Rect::new(
            panel.x + 5.0,
            divider_y + 6.0,
            panel.w - 10.0,
            MENU_ROW_HEIGHT,
        );
        if let Some(city_id) = menu.city_id {
            draw_ui_button(button, "Show city info", true, true);
            actions.push((button, UiAction::ShowCityInfo(city_id)));
        } else {
            draw_ui_button(button, "Select all units in stack", true, true);
            actions.push((button, UiAction::SelectUnitStack(menu.units.clone())));
            if let Some(ids) = same_kind
                && let Some(active) = self
                    .selected_unit
                    .and_then(|id| self.sim.unit_kind_view(id))
            {
                let button = Rect::new(
                    panel.x + 5.0,
                    divider_y + 6.0 + MENU_ROW_HEIGHT,
                    panel.w - 10.0,
                    MENU_ROW_HEIGHT,
                );
                draw_ui_button(
                    button,
                    &format!("Select all {}", Self::plural_unit_label(active)),
                    true,
                    true,
                );
                actions.push((button, UiAction::SelectSameUnitType(ids)));
            }
        }
        actions
    }

    fn draw_menu_panel(&self, panel: Rect, title: String) -> Rect {
        draw_rectangle(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            Color::from_rgba(28, 31, 28, 248),
        );
        draw_rectangle_lines(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            2.0,
            Color::from_rgba(205, 175, 91, 255),
        );
        draw_text(
            &title,
            panel.x + 9.0,
            panel.y + 19.0,
            14.0,
            Color::from_rgba(242, 218, 145, 255),
        );
        panel
    }

    fn draw_menu_divider(&self, panel: Rect, y: f32) {
        draw_line(
            panel.x + 8.0,
            y + 3.0,
            panel.right() - 8.0,
            y + 3.0,
            1.0,
            Color::from_rgba(150, 132, 93, 230),
        );
    }

    fn draw_unit_rows(
        &self,
        panel: Rect,
        top: f32,
        offset: usize,
        visible_rows: usize,
        units: &[u32],
    ) -> Vec<(Rect, UiAction)> {
        units
            .iter()
            .copied()
            .skip(offset)
            .take(visible_rows)
            .enumerate()
            .filter_map(|(index, id)| {
                let kind = self.sim.unit_kind_view(id)?;
                let button = Rect::new(
                    panel.x + 5.0,
                    top + MENU_ROW_HEIGHT * index as f32,
                    panel.w - 10.0,
                    MENU_ROW_HEIGHT,
                );
                draw_ui_button(button, kind.name(), true, true);
                Some((button, UiAction::SelectUnit(id)))
            })
            .collect()
    }

    fn plural_unit_label(kind: UnitKind) -> String {
        let name = kind.name();
        match kind {
            UnitKind::Cavalry | UnitKind::Infantry | UnitKind::Artillery => {
                format!("{name} units")
            }
            _ if name.ends_with("man") => {
                format!("{}men", name.strip_suffix("man").unwrap())
            }
            _ if name.ends_with('y')
                && !name.ends_with("ey")
                && !name.ends_with("ay")
                && !name.ends_with("oy")
                && !name.ends_with("uy") =>
            {
                format!("{}ies", name.strip_suffix('y').unwrap())
            }
            _ if name.ends_with('s') || name.ends_with('x') => format!("{name}es"),
            _ => format!("{name}s"),
        }
    }
}
