use super::*;

impl Game {
    pub(in crate::client) fn draw_government(&self) -> Vec<(Rect, UiAction)> {
        let player = &self.sim.players()[HUMAN];
        let width = (screen_width() - 24.0).min(620.0);
        let panel = Rect::new(
            (screen_width() - width) / 2.0,
            (screen_height() - 378.0) / 2.0,
            width,
            378.0,
        );
        panel_frame(panel);
        let draw_text = panel_text(panel);
        let close = panel_close_rect(panel);
        draw_ui_button(close, "X", true, false);
        let mut actions = vec![(close, UiAction::ToggleGovernment)];

        text_in(
            &format!("{} · {}", player.name, player.leader_name()),
            panel.x + 18.0,
            panel.y + 31.0,
            22.0,
            panel.w - 81.0,
            WHITE,
        );
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
                && !player.known.contains(&tech)
            {
                let label = if enabled {
                    "Statue of Liberty".to_owned()
                } else {
                    format!("Requires {}", tech.name())
                };
                draw_text(&label, x + 3.0, y + 57.0, 13.0, GOLD);
            }
        }
        actions
    }
}
