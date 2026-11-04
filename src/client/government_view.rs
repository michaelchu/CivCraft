use super::*;

impl Game {
    pub(super) fn draw_government(&self) -> Vec<(Rect, UiAction)> {
        let player = &self.sim.players()[HUMAN];
        draw_rectangle(
            0.0,
            TOP_H,
            screen_width(),
            screen_height() - TOP_H,
            Color::from_rgba(4, 9, 12, 185),
        );
        let width = (screen_width() - 24.0).min(620.0);
        let panel = Rect::new(
            (screen_width() - width) / 2.0,
            (screen_height() - 378.0) / 2.0,
            width,
            378.0,
        );
        draw_rectangle(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            Color::from_rgba(42, 44, 40, 255),
        );
        draw_rectangle_lines(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            2.0,
            Color::from_rgba(206, 165, 72, 255),
        );
        let close = Rect::new(panel.right() - 45.0, panel.y + 12.0, 32.0, 26.0);
        draw_ui_button(close, "X", true, false);
        let mut actions = vec![(close, UiAction::ToggleGovernment)];

        draw_text("Government", panel.x + 18.0, panel.y + 31.0, 24.0, WHITE);
        let status = if let (Some(target), Some(turn)) =
            (player.revolution_target, player.revolution_turn)
        {
            format!(
                "Anarchy  |  {} in {} turns",
                target.name(),
                turn - self.sim.turn()
            )
        } else {
            format!(
                "{}  |  Rate limit {}%",
                player.government.name(),
                player.government.max_rate()
            )
        };
        draw_text(&status, panel.x + 18.0, panel.y + 55.0, 17.0, GOLD);
        draw_text(
            "Choose a government to start a revolution.",
            panel.x + 18.0,
            panel.y + 77.0,
            16.0,
            LIGHTGRAY,
        );
        let col = (panel.w - 46.0) / 2.0;
        for (index, government) in Government::TARGETS.into_iter().enumerate() {
            let x = panel.x + 18.0 + (index % 2) as f32 * (col + 10.0);
            let y = panel.y + 88.0 + (index / 2) as f32 * 67.0;
            let button = Rect::new(x, y, col, 28.0);
            let enabled = self.sim.can_change_government(HUMAN, government);
            let current = player.government == government;
            draw_ui_button(button, government.name(), enabled, current);
            if enabled {
                actions.push((button, UiAction::ChangeGovernment(government)));
            }
            let (gold, free_shields, free_gold) = government.unit_support();
            let support = if gold > 0 {
                format!("Eligible units: {gold} gold, {free_gold} free gold")
            } else {
                format!("Unit support: {free_shields} free shields")
            };
            draw_text(&support, x + 3.0, y + 43.0, 13.0, LIGHTGRAY);
            if let Some(tech) = government.requirement()
                && !player.known.iter().any(|known| known.name() == tech)
            {
                let label = if enabled {
                    "Statue of Liberty".to_owned()
                } else {
                    format!("Requires {tech}")
                };
                draw_text(&label, x + 3.0, y + 57.0, 13.0, GOLD);
            }
        }
        actions
    }
}
