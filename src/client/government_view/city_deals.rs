//! City deal and trade negotiation controls in the diplomacy view.

use super::*;

impl Game {
    pub(super) fn draw_city_deals(&self, owner: usize, page: usize) -> Vec<(Rect, UiAction)> {
        let width = (screen_width() - 24.0).min(580.0);
        let panel = Rect::new(
            (screen_width() - width) / 2.0,
            (screen_height() - 360.0) / 2.0,
            width,
            360.0,
        );
        panel_frame(panel);
        let draw_text = panel_text(panel);
        draw_text("City deals", panel.x + 18.0, panel.y + 31.0, 24.0, WHITE);
        let back = Rect::new(panel.right() - 88.0, panel.y + 12.0, 72.0, 26.0);
        draw_ui_button(back, "Back", true, false);
        let mut actions = vec![(back, UiAction::CityDeals(None))];
        let cities = self.sim.city_deals_view(owner);
        if cities.is_empty() {
            draw_text(
                "No cities are available for purchase.",
                panel.x + 18.0,
                panel.y + 76.0,
                17.0,
                LIGHTGRAY,
            );
        }
        let page = page.min(cities.len().saturating_sub(1) / 7);
        for (row, city) in cities.iter().skip(page * 7).take(7).enumerate() {
            let y = panel.y + 53.0 + row as f32 * 37.0;
            text_in(
                &city.name,
                panel.x + 18.0,
                y + 20.0,
                17.0,
                panel.w - 185.0,
                WHITE,
            );
            let button = Rect::new(panel.right() - 145.0, y, 129.0, 28.0);
            draw_ui_button(
                button,
                &format!("Buy {}g", city.price),
                city.affordable,
                false,
            );
            if city.affordable {
                actions.push((button, UiAction::BuyCity(city.id, city.price)));
            }
        }
        actions.extend(pager(
            panel,
            page,
            cities.len().saturating_sub(1) / 7,
            |page| UiAction::CityDeals(Some((owner, page))),
        ));
        actions
    }

    pub(super) fn draw_treaty_offer(
        &self,
        owner: usize,
        offer: TreatyOffer,
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
        for (index, terms) in offer.terms.into_iter().enumerate() {
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
            if let Some(map) = terms.map() {
                lines.push(format!("{map:?} map"));
            }
            if let Some(id) = terms.city() {
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
            ("Accept", UiAction::AcceptOffer(owner, offer), valid),
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
