//! Heads-up display composition and its map, overview, and status panels.

use super::*;
mod context_panel;
mod icons;
mod info_block;
mod minimap;
mod top_bar;

pub(super) use context_panel::{EMPTY_CARD_H, unit_card_height, unit_status_inline};
pub(super) use icons::{HudIcon, draw_hud_icon};
pub(super) use info_block::INFO_BLOCK_H;

impl Game {
    pub(super) fn draw_hud(&mut self, assets: &Assets) -> Vec<(Rect, UiAction)> {
        let mut actions = Vec::new();
        self.draw_minimap();
        self.hud_elements_drawn += 1;

        let context = self.context_rect();
        draw_hud_card(context);
        let end = Rect::new(
            context.x + 10.0,
            context.bottom() - 38.0,
            context.w - 20.0,
            28.0,
        );
        self.draw_context_panel(assets, context, end.y);
        draw_ui_button(end, "", true, true);
        let shortcut = "Shift+Enter";
        let shortcut_width = measure_text(shortcut, None, 11, 1.0).width;
        draw_text("End Turn", end.x + 8.0, end.y + 18.0, 13.0, WHITE);
        draw_text(
            shortcut,
            end.right() - 8.0 - shortcut_width,
            end.y + 18.0,
            11.0,
            Color::from_rgba(241, 223, 189, 255),
        );
        actions.push((end, UiAction::EndTurn));
        actions.push((context, UiAction::BlockMapClick));
        self.hud_elements_drawn += 1;

        let action_width = screen_width() - 2.0 * (Self::corner_width() + 2.0 * HUD_MARGIN);
        self.draw_action_strip(&mut actions, action_width);
        // Messages sit in the top-left corner and stop short of the floating buttons.
        let message_room = top_bar::controls_start_x(screen_width(), self.has_spaceship_button())
            - 2.0 * HUD_MARGIN;
        self.draw_messages(&mut actions, message_room);
        self.draw_top_bar(&mut actions);
        self.hud_elements_drawn += 1;
        self.hud_elements_drawn += 1;
        actions
    }

    pub(super) fn draw_intro(&self) -> Vec<(Rect, UiAction)> {
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::from_rgba(4, 9, 12, 178),
        );
        let width = (screen_width() - 2.0 * HUD_MARGIN).min(470.0);
        let height = 254.0;
        let panel = Rect::new(
            (screen_width() - width) * 0.5,
            (screen_height() - height) * 0.5,
            width,
            height,
        );
        panel_frame(panel);
        text_in(
            "Your Chronicle",
            panel.x + 22.0,
            panel.y + 41.0,
            27.0,
            width - 44.0,
            GOLD,
        );
        for (index, line) in [
            "Found a city with your Settlers.",
            "Explore the fog and improve the land.",
            "Research stronger units.",
            "Unite the rival nations.",
            "Click a unit or city for details.",
        ]
        .into_iter()
        .enumerate()
        {
            text_in(
                line,
                panel.x + 22.0,
                panel.y + 75.0 + index as f32 * 25.0,
                17.0,
                width - 44.0,
                WHITE,
            );
        }
        let button = Rect::new(panel.right() - 172.0, panel.bottom() - 52.0, 150.0, 34.0);
        draw_ui_button(button, "Begin [Enter]", true, true);
        vec![(button, UiAction::DismissIntro)]
    }

    fn draw_messages(&mut self, actions: &mut Vec<(Rect, UiAction)>, available: f32) {
        let width = (screen_width() * 0.65).min(620.0).min(available);
        let sim_messages = self.sim.campaign_messages_view();
        if sim_messages != self.last_sim_messages {
            self.client_messages.clear();
            self.last_sim_messages.clone_from(&sim_messages);
        }
        let latest = self
            .client_messages
            .iter()
            .rev()
            .chain(sim_messages.iter().rev())
            .take(2);
        for (i, message) in latest.enumerate() {
            let color = if i == 0 {
                Color::from_rgba(230, 228, 211, 255)
            } else {
                Color::from_rgba(174, 177, 171, 255)
            };
            let clipped = fit_text(message, 15.0, width - 16.0);
            let rect = Rect::new(
                HUD_MARGIN,
                HUD_MARGIN + (1 - i) as f32 * 21.0,
                measure_text(&clipped, None, 15, 1.0).width + 16.0,
                21.0,
            );
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                Color::from_rgba(16, 20, 20, 218),
            );
            draw_text(&clipped, rect.x + 8.0, rect.y + 15.0, 15.0, color);
            actions.push((rect, UiAction::BlockMapClick));
        }
    }
}

pub(super) fn draw_hud_card(card: Rect) {
    draw_rectangle(
        card.x,
        card.y,
        card.w,
        card.h,
        Color::from_rgba(26, 29, 26, 237),
    );
    draw_rectangle_lines(
        card.x,
        card.y,
        card.w,
        card.h,
        2.0,
        Color::from_rgba(160, 126, 61, 255),
    );
}
