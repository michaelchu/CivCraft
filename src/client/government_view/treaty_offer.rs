//! Treaty offer review controls in the diplomacy view.

use super::*;

impl Game {
    pub(super) fn draw_treaty_offer(
        &self,
        owner: usize,
        offer: &TreatyOffer,
    ) -> Vec<(Rect, UiAction)> {
        let width = (screen_width() - 24.0).min(560.0);
        let panel = Rect::new(
            (screen_width() - width) / 2.0,
            (screen_height() - 340.0) / 2.0,
            width,
            340.0,
        );
        panel_frame(panel);
        let draw_text = panel_text(panel);
        let x = panel.x + 18.0;
        draw_text(
            &format!(
                "Treaty offer from {}",
                self.sim.player_name_view(owner).unwrap_or_default()
            ),
            x,
            panel.y + 32.0,
            22.0,
            WHITE,
        );
        let pact = offer.pact.map_or("Exchange", Pact::name);
        draw_text(pact, x, panel.y + 60.0, 18.0, GOLD);
        for (index, terms) in offer.terms.iter().enumerate() {
            let mut y = panel.y + 90.0;
            let column_w = (width - 54.0) / 2.0;
            let x = x + index as f32 * (column_w + 18.0);
            draw_text(
                if index == 0 { "You give" } else { "They give" },
                x,
                y,
                19.0,
                WHITE,
            );
            let mut lines = Vec::new();
            if terms.gold > 0 {
                lines.push(format!("{} gold", terms.gold));
            }
            if let Some(tech) = terms.technology {
                lines.push(tech.name().to_owned());
            }
            if terms.shared_vision() {
                lines.push("Shared vision".to_owned());
            }
            if terms.embassy() {
                lines.push("Embassy access".to_owned());
            }
            for map in terms.maps() {
                lines.push(format!("{map:?} map"));
            }
            for &id in terms.cities() {
                lines.push(
                    self.sim
                        .city_name_view(id)
                        .unwrap_or_else(|| "Unavailable city".to_owned()),
                );
            }
            if lines.is_empty() {
                lines.push("No concessions".to_owned());
            }
            for line in lines {
                y += 24.0;
                text_in(&line, x, y, 17.0, column_w, LIGHTGRAY);
            }
        }
        let valid = self.sim.treaty_offer_current_view(owner, offer);
        let buttons = [
            ("Accept", UiAction::AcceptOffer(owner, offer.clone()), valid),
            ("Decline", UiAction::DeclineOffer(owner), valid),
            ("Back", UiAction::CloseOffer, true),
        ];
        let mut actions = Vec::new();
        for (index, (label, action, enabled)) in buttons.into_iter().enumerate() {
            let rect = Rect::new(x + index as f32 * 91.0, panel.bottom() - 46.0, 85.0, 28.0);
            draw_ui_button(rect, label, enabled, index == 0);
            if enabled {
                actions.push((rect, action));
            }
        }
        actions
    }
}
