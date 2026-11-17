//! Government selection and government-dependent rules.

use super::*;

impl Game {
    pub(in crate::client) fn draw_government(&self) -> Vec<(Rect, UiAction)> {
        let Some(view) = self.sim.government_view(HUMAN) else {
            return vec![];
        };
        let width = (screen_width() - 24.0).min(620.0);
        // Government options, then the three rate sliders, then mobilization.
        let rows = view.options.len().div_ceil(2) as f32;
        let rates_top_offset = 88.0 + rows * 67.0 + 6.0;
        let height = rates_top_offset + 162.0;
        let panel = Rect::new(
            (screen_width() - width) / 2.0,
            (screen_height() - height) / 2.0,
            width,
            height,
        );
        panel_frame(panel);
        let draw_text = panel_text(panel);
        let close = panel_close_rect(panel);
        draw_ui_button(close, "X", true, false);
        let mut actions = vec![(close, UiAction::ToggleGovernment)];

        text_in(
            &view.leader_line,
            panel.x + 18.0,
            panel.y + 31.0,
            22.0,
            panel.w - 81.0,
            WHITE,
        );
        draw_text(
            &view.status_line,
            panel.x + 18.0,
            panel.y + 55.0,
            17.0,
            GOLD,
        );
        draw_text(
            "Choose a government to start a revolution.",
            panel.x + 18.0,
            panel.y + 77.0,
            16.0,
            LIGHTGRAY,
        );
        let col = (panel.w - 46.0) / 2.0;
        for (index, option) in view.options.iter().enumerate() {
            let x = panel.x + 18.0 + (index % 2) as f32 * (col + 10.0);
            let y = panel.y + 88.0 + (index / 2) as f32 * 67.0;
            let button = Rect::new(x, y, col, 28.0);
            draw_ui_button(button, option.name, option.enabled, option.current);
            if option.enabled {
                actions.push((button, UiAction::ChangeGovernment(option.government)));
            }
            draw_text(
                &option.support_description,
                x + 3.0,
                y + 43.0,
                13.0,
                LIGHTGRAY,
            );
            if let Some(label) = &option.requirement_description {
                draw_text(label, x + 3.0, y + 57.0, 13.0, GOLD);
            }
        }
        // Trade rate sliders sit under the options, above the mobilization button.
        let rates_top = panel.y + rates_top_offset;
        draw_text(
            "Trade rates: drag a slider; the others rebalance.",
            panel.x + 18.0,
            rates_top + 14.0,
            16.0,
            LIGHTGRAY,
        );
        let rate_rows = [
            (hud::HudIcon::Science, "Science"),
            (hud::HudIcon::Tax, "Tax"),
            (hud::HudIcon::Luxury, "Luxury"),
        ];
        let track_left = panel.x + 190.0;
        let track_right = panel.right() - 30.0;
        let stop = (track_right - track_left) / 10.0;
        let usable = !view.rates_locked;
        for (index, (icon, name)) in rate_rows.into_iter().enumerate() {
            let row_y = rates_top + 22.0 + index as f32 * 30.0;
            let color = if usable {
                Color::from_rgba(241, 238, 218, 255)
            } else {
                GRAY
            };
            hud::draw_hud_icon(icon, vec2(panel.x + 30.0, row_y + 13.0), 17.0, color);
            text_in(
                &format!("{name} {}%", view.rates[index]),
                panel.x + 46.0,
                row_y + 18.0,
                16.0,
                130.0,
                color,
            );
            let value_x = track_left + stop * (view.rates[index] / 10) as f32;
            let cap_x = track_left + stop * (view.max_rate / 10) as f32;
            let track_y = row_y + 11.0;
            draw_rectangle(
                track_left,
                track_y,
                track_right - track_left,
                4.0,
                Color::from_rgba(28, 32, 28, 255),
            );
            draw_rectangle(
                track_left,
                track_y,
                cap_x - track_left,
                4.0,
                Color::from_rgba(61, 69, 60, 255),
            );
            draw_rectangle(
                track_left,
                track_y,
                value_x - track_left,
                4.0,
                if usable {
                    Color::from_rgba(171, 133, 55, 255)
                } else {
                    GRAY
                },
            );
            for tick in 0..=10 {
                let x = track_left + stop * tick as f32;
                draw_line(
                    x,
                    row_y + 8.0,
                    x,
                    row_y + 18.0,
                    1.0,
                    Color::from_rgba(90, 98, 88, 255),
                );
            }
            let handle = Rect::new(value_x - 5.0, row_y + 3.0, 10.0, 20.0);
            draw_rectangle(handle.x, handle.y, handle.w, handle.h, color);
            if usable {
                for tick in 0..=(view.max_rate / 10) {
                    let x = track_left + stop * tick as f32;
                    let hit = Rect::new(x - stop * 0.5, row_y, stop, 26.0);
                    let rates =
                        rates::rebalanced_rates(view.rates, view.max_rate, index, tick * 10);
                    actions.push((hit, UiAction::SetRates(rates)));
                }
            }
        }
        let mobilization = Rect::new(panel.x + 18.0, panel.bottom() - 42.0, panel.w - 36.0, 28.0);
        draw_ui_button(
            mobilization,
            view.mobilization_label,
            view.can_mobilize,
            false,
        );
        if view.can_mobilize {
            actions.push((mobilization, UiAction::Mobilize));
        }
        actions
    }
}
