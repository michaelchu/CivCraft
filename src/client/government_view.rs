use super::*;

impl Game {
    pub(super) fn draw_government(&self) -> Vec<(Rect, UiAction)> {
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

impl Game {
    pub(super) fn draw_diplomacy(&self) -> Vec<(Rect, UiAction)> {
        if let Some((owner, offer)) = self.reviewed_offer {
            return self.draw_treaty_offer(owner, offer);
        }
        if let Some((owner, page)) = self.city_deals {
            return self.draw_city_deals(owner, page);
        }
        let width = (screen_width() - 24.0).min(850.0);
        let panel = Rect::new(
            (screen_width() - width) / 2.0,
            (screen_height() - 480.0) / 2.0,
            width,
            480.0,
        );
        panel_frame(panel);
        let draw_text = panel_text(panel);
        draw_text("Diplomacy", panel.x + 18.0, panel.y + 31.0, 24.0, WHITE);
        let close = Rect::new(panel.right() - 45.0, panel.y + 12.0, 32.0, 26.0);
        draw_ui_button(close, "X", true, false);
        let mut actions = vec![(close, UiAction::ToggleDiplomacy)];
        let rivals: Vec<_> = (1..self.sim.players().len())
            .filter(|&owner| {
                self.sim.can_negotiate(owner) || self.sim.treaty_offer(owner).is_some()
            })
            .collect();
        let page = self.diplomacy_page.min(rivals.len().saturating_sub(1) / 4);
        for (row, &owner) in rivals.iter().skip(page * 4).take(4).enumerate() {
            let y = panel.y + 53.0 + row as f32 * 94.0;
            let pact = self.sim.pact(HUMAN, owner);
            let turns = self.sim.pact_turns(HUMAN, owner);
            let offer = self.sim.pact_offer(owner);
            let mut status = pact.name().to_owned();
            if self.sim.has_embassy(HUMAN, owner) {
                status.push_str(" · Embassy");
            }
            if turns > 0 {
                status.push_str(&format!(" ({turns})"));
            }
            if pact != Pact::War && self.sim.senate_blocks_cancellation(HUMAN, owner) {
                status.push_str(" · Senate veto");
            }
            text_in(
                self.sim.players()[owner].name.as_str(),
                panel.x + 18.0,
                y + 14.0,
                17.0,
                panel.w - 36.0,
                WHITE,
            );
            draw_text(&status, panel.x + 18.0, y + 30.0, 14.0, LIGHTGRAY);
            let mut buttons = Vec::new();
            if let Some(bundle) = self
                .sim
                .treaty_offer(owner)
                .filter(|offer| offer.bundled() || !self.sim.can_negotiate(owner))
            {
                buttons.push((
                    "Review offer".to_owned(),
                    UiAction::ReviewOffer(owner, bundle),
                ));
            } else if let Some(next) = offer.or_else(|| self.sim.next_pact(HUMAN, owner)) {
                let verb = if offer.is_some() { "Accept" } else { "Propose" };
                buttons.push((
                    format!("{verb} {}", next.name().to_lowercase()),
                    UiAction::Negotiate(owner, next),
                ));
            }
            if self.sim.treaty_offer(owner).is_some() {
                buttons.push(("Decline".to_owned(), UiAction::DeclineOffer(owner)));
            }
            if pact != Pact::War {
                buttons.push(("Cancel pact".to_owned(), UiAction::CancelPact(owner)));
            }
            if self.sim.treaty_offer(owner).is_none() {
                for (label, map) in [("World", TreatyMap::World), ("Sea", TreatyMap::Sea)] {
                    let price = self.sim.map_trade_price(owner, HUMAN, map, pact);
                    buttons.push((
                        format!("{label} map {price}g"),
                        UiAction::BuyMap(owner, map),
                    ));
                }
            }
            if self
                .sim
                .cities()
                .iter()
                .any(|city| city.owner == owner && self.sim.city_trade_price(city).is_some())
            {
                buttons.push((
                    "City deals".to_owned(),
                    UiAction::CityDeals(Some((owner, 0))),
                ));
            }
            let columns = grid_columns(panel.w - 36.0, 120.0);
            let button_w = (panel.w - 36.0 - UI_GAP * (columns - 1) as f32) / columns as f32;
            for (index, (label, action)) in buttons.into_iter().enumerate() {
                let button = Rect::new(
                    panel.x + 18.0 + (index % columns) as f32 * (button_w + UI_GAP),
                    y + 39.0 + (index / columns) as f32 * 32.0,
                    button_w,
                    26.0,
                );
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
        actions.extend(pager(
            panel,
            page,
            rivals.len().saturating_sub(1) / 4,
            UiAction::DiplomacyPage,
        ));
        actions
    }
}

impl Game {
    fn draw_city_deals(&self, owner: usize, page: usize) -> Vec<(Rect, UiAction)> {
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
        let cities: Vec<_> = self
            .sim
            .cities()
            .iter()
            .filter(|city| city.owner == owner)
            .filter_map(|city| self.sim.city_trade_price(city).map(|price| (city, price)))
            .collect();
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
        for (row, &(city, price)) in cities.iter().skip(page * 7).take(7).enumerate() {
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
            let affordable = self.sim.players()[HUMAN].gold >= price;
            draw_ui_button(button, &format!("Buy {price}g"), affordable, false);
            if affordable {
                actions.push((button, UiAction::BuyCity(city.id, price)));
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

    fn draw_treaty_offer(&self, owner: usize, offer: TreatyOffer) -> Vec<(Rect, UiAction)> {
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
            &format!("Treaty offer from {}", self.sim.players()[owner].name),
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
                let name = self
                    .sim
                    .cities()
                    .iter()
                    .find(|city| city.id == id)
                    .map_or("Unavailable city", |city| city.name.as_str());
                lines.push(name.to_owned());
            }
            if lines.is_empty() {
                lines.push("No concessions".to_owned());
            }
            for line in lines {
                y += 24.0;
                text_in(&line, x, y, 17.0, column_w, LIGHTGRAY);
            }
        }
        let valid = self.sim.treaty_offer(owner) == Some(offer);
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
