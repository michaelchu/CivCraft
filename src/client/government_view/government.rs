//! Government selection and government-dependent rules.

use super::*;

impl Game {
    pub(in crate::client) fn draw_government(&self) -> Vec<(Rect, UiAction)> {
        let Some(view) = self.sim.government_view(HUMAN) else {
            return vec![];
        };
        let width = (screen_width() - 24.0).min(620.0);
        let panel = Rect::new(
            (screen_width() - width) / 2.0,
            (screen_height() - 430.0) / 2.0,
            width,
            430.0,
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
        let mobilization = Rect::new(panel.x + 18.0, panel.y + 388.0, panel.w - 36.0, 28.0);
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
