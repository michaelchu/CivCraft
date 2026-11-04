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

impl Game {
    pub(super) fn draw_diplomacy(&self) -> Vec<(Rect, UiAction)> {
        let width = (screen_width() - 24.0).min(580.0);
        let panel = Rect::new(
            (screen_width() - width) / 2.0,
            (screen_height() - 360.0) / 2.0,
            width,
            360.0,
        );
        draw_rectangle(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            Color::from_rgba(42, 44, 40, 255),
        );
        draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 2.0, GOLD);
        draw_text("Diplomacy", panel.x + 18.0, panel.y + 31.0, 24.0, WHITE);
        let close = Rect::new(panel.right() - 45.0, panel.y + 12.0, 32.0, 26.0);
        draw_ui_button(close, "X", true, false);
        let mut actions = vec![(close, UiAction::ToggleDiplomacy)];
        let rivals: Vec<_> = (1..self.sim.players().len())
            .filter(|&owner| self.sim.can_negotiate(owner))
            .collect();
        let page = self.diplomacy_page.min(rivals.len().saturating_sub(1) / 7);
        for (row, &owner) in rivals.iter().skip(page * 7).take(7).enumerate() {
            let y = panel.y + 53.0 + row as f32 * 37.0;
            let pact = self.sim.pact(HUMAN, owner);
            let turns = self.sim.pact_turns(HUMAN, owner);
            let offer = self.sim.pact_offer(owner);
            let mut status = pact.name().to_owned();
            if turns > 0 {
                status.push_str(&format!(" ({turns})"));
            }
            if pact != Pact::War && self.sim.senate_blocks_cancellation(HUMAN, owner) {
                status.push_str(" · Senate veto");
            }
            draw_text(
                self.sim.players()[owner].name,
                panel.x + 18.0,
                y + 14.0,
                17.0,
                WHITE,
            );
            draw_text(&status, panel.x + 18.0, y + 30.0, 14.0, LIGHTGRAY);
            let mut buttons = Vec::new();
            if let Some(next) = offer.or_else(|| self.sim.next_pact(HUMAN, owner)) {
                let verb = if offer.is_some() { "Accept" } else { "Propose" };
                buttons.push((
                    format!("{verb} {}", next.name().to_lowercase()),
                    UiAction::Negotiate(owner, next),
                ));
            }
            if offer.is_some() {
                buttons.push(("Decline".to_owned(), UiAction::DeclineOffer(owner)));
            }
            if pact != Pact::War {
                buttons.push(("Cancel pact".to_owned(), UiAction::CancelPact(owner)));
            }
            for (index, (label, action)) in buttons.into_iter().rev().enumerate() {
                let button =
                    Rect::new(panel.right() - 129.0 - index as f32 * 116.0, y, 110.0, 30.0);
                draw_ui_button(button, &label, true, false);
                actions.push((button, action));
            }
        }
        if rivals.is_empty() {
            draw_text(
                "Meet a rival civilization to negotiate.",
                panel.x + 18.0,
                panel.y + 76.0,
                17.0,
                LIGHTGRAY,
            );
        }
        for (enabled, x, label, target) in [
            (page > 0, panel.x + 18.0, "Previous", page.saturating_sub(1)),
            (
                (page + 1) * 7 < rivals.len(),
                panel.right() - 95.0,
                "Next",
                page + 1,
            ),
        ] {
            if enabled {
                let button = Rect::new(x, panel.bottom() - 36.0, 77.0, 25.0);
                draw_ui_button(button, label, true, false);
                actions.push((button, UiAction::DiplomacyPage(target)));
            }
        }
        actions
    }
}
