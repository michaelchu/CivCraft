//! Spaceship progress panel: the ten Civ III components, built once each.

use super::*;

impl Game {
    pub(super) fn draw_spaceship(&self) -> Vec<(Rect, UiAction)> {
        let view = self.sim.spaceship_assembly_view(HUMAN);
        let width = (screen_width() - 32.0).min(560.0);
        let height = 120.0 + view.parts.len() as f32 * 26.0;
        let x = (screen_width() - width) / 2.0;
        let y = ((screen_height() - height) / 2.0).max(12.0);
        let panel = Rect::new(x, y, width, height);
        panel_frame(panel);
        let draw_text = panel_text(panel);
        draw_text("Spaceship", x + 16.0, y + 32.0, 24.0, GOLD);
        let close = Rect::new(x + width - 52.0, y + 12.0, 36.0, 27.0);
        draw_ui_button(close, "X", true, false);
        draw_text(&view.status, x + 16.0, y + 62.0, 17.0, LIGHTGRAY);
        draw_text(
            "The first civilization to build all ten parts wins the space race.",
            x + 16.0,
            y + 84.0,
            15.0,
            LIGHTGRAY,
        );
        for (index, (name, built)) in view.parts.iter().enumerate() {
            let color = if *built { GOLD } else { LIGHTGRAY };
            let mark = if *built { "[x]" } else { "[ ]" };
            draw_text(
                &format!("{mark} {name}"),
                x + 16.0,
                y + 114.0 + index as f32 * 26.0,
                18.0,
                color,
            );
        }
        vec![(close, UiAction::ToggleSpaceship)]
    }
}
