//! Diplomatic state, contact, offers, treaties, and war planning.

use super::*;

impl Game {
    pub(in crate::client) fn draw_diplomacy(&self, assets: &Assets) -> Vec<(Rect, UiAction)> {
        if let Some(owner) = self.diplomacy_target {
            return self.draw_diplomacy_meeting(owner, assets);
        }
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
        let close = panel_close_rect(panel);
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
                &format!(
                    "{} · {}",
                    self.sim.players()[owner].name,
                    self.sim.players()[owner].leader_name()
                ),
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
            } else if offer.is_some() {
                buttons.push((
                    "Review offer".to_owned(),
                    UiAction::OpenDiplomacyWith(owner),
                ));
            } else if self.sim.next_pact(HUMAN, owner).is_some() {
                buttons.push(("Negotiate".to_owned(), UiAction::OpenDiplomacyWith(owner)));
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

    fn draw_diplomacy_meeting(&self, owner: usize, assets: &Assets) -> Vec<(Rect, UiAction)> {
        if owner >= self.sim.players().len() || owner == HUMAN {
            return vec![];
        }
        let width = (screen_width() - 32.0).min(930.0);
        let height = (screen_height() - 64.0).min(560.0);
        let panel = Rect::new(
            (screen_width() - width) / 2.0,
            (screen_height() - height) / 2.0,
            width,
            height,
        );
        panel_frame(panel);
        let draw_text = panel_text(panel);
        draw_text("Diplomacy", panel.x + 20.0, panel.y + 31.0, 24.0, WHITE);
        let close = Rect::new(panel.right() - 46.0, panel.y + 12.0, 32.0, 26.0);
        draw_ui_button(close, "X", true, false);
        let mut actions = vec![(close, UiAction::ToggleDiplomacy)];
        let can_negotiate = self.sim.can_negotiate(owner);
        let ai_offer = self.sim.treaty_offer(owner);
        let offer = self
            .treaty_draft
            .filter(|(target, _)| *target == owner)
            .map(|(_, offer)| offer)
            .or(ai_offer)
            .unwrap_or_else(|| TreatyOffer {
                pact: self.sim.next_pact(HUMAN, owner),
                ..TreatyOffer::default()
            });
        let attitude = self.sim.diplomatic_attitude(owner, HUMAN);
        let content_y = panel.y + 49.0;
        let side_width = (panel.w * 0.22).clamp(150.0, 196.0);
        let gap = 10.0;
        let left_x = panel.x + 16.0;
        let center_x = left_x + side_width + gap;
        let center_width = panel.w - 32.0 - side_width * 2.0 - gap * 2.0;
        let right_x = center_x + center_width + gap;
        let bottom = panel.bottom() - 52.0;
        let side_height = bottom - content_y;
        let side_rects = [
            Rect::new(left_x, content_y, side_width, side_height),
            Rect::new(right_x, content_y, side_width, side_height),
        ];

        for (side_index, side) in [HUMAN, owner].into_iter().enumerate() {
            let rect = side_rects[side_index];
            let side_name = &self.sim.players()[side].name;
            draw_text(
                &format!("{side_name} · {}g", self.sim.players()[side].gold),
                rect.x,
                rect.y + 16.0,
                16.0,
                Color::from_rgba(230, 208, 151, 255),
            );
            let gold_y = rect.y + 23.0;
            let amount =
                self.treaty_gold_amount[side].clamp(0, self.sim.players()[side].gold.max(0));
            let step_width = ((rect.w - 12.0) / 3.0).min(48.0);
            let step_gap = (rect.w - step_width * 3.0) / 2.0;
            let minus = Rect::new(rect.x, gold_y, step_width, 24.0);
            let amount_rect = Rect::new(rect.x + step_width + step_gap, gold_y, step_width, 24.0);
            let plus = Rect::new(amount_rect.right() + step_gap, gold_y, step_width, 24.0);
            let add_gold = Rect::new(rect.x, gold_y + 28.0, rect.w, 24.0);
            draw_ui_button(minus, "-25", amount > 0, false);
            let amount_label = format!("{amount}g");
            let amount_label_width = measure_text(&amount_label, None, 13, 1.0).width;
            draw_text(
                &amount_label,
                amount_rect.x + (amount_rect.w - amount_label_width) / 2.0,
                amount_rect.y + 17.0,
                13.0,
                WHITE,
            );
            draw_ui_button(plus, "+25", amount < self.sim.players()[side].gold, false);
            draw_ui_button(
                add_gold,
                "Add gold",
                can_negotiate && self.sim.players()[side].gold > 0,
                false,
            );
            if can_negotiate {
                if amount > 0 {
                    actions.push((minus, UiAction::AdjustTreatyGold(side, -25)));
                }
                if amount < self.sim.players()[side].gold {
                    actions.push((plus, UiAction::AdjustTreatyGold(side, 25)));
                }
                if self.sim.players()[side].gold > 0 {
                    actions.push((add_gold, UiAction::AddTreatyGold(owner, side, offer)));
                }
            }

            let mut groups: Vec<(String, Vec<(String, TreatyOffer)>)> = vec![
                ("Technologies".to_owned(), Vec::new()),
                ("Maps".to_owned(), Vec::new()),
                ("Diplomatic access".to_owned(), Vec::new()),
                ("Cities".to_owned(), Vec::new()),
            ];
            let after = offer.pact.unwrap_or_else(|| self.sim.pact(HUMAN, owner));
            for tech in self
                .sim
                .treaty_trade_technologies(side, if side == HUMAN { owner } else { HUMAN })
            {
                let mut changed = offer;
                changed.terms[side_index] = changed.terms[side_index].set_technology(Some(tech));
                groups[0].1.push((tech.name().to_owned(), changed));
            }
            for map in [TreatyMap::World, TreatyMap::Sea] {
                let mut changed = offer;
                changed.terms[side_index] = changed.terms[side_index].set_map(Some(map));
                groups[1].1.push((
                    format!(
                        "{} map",
                        if map == TreatyMap::World {
                            "World"
                        } else {
                            "Sea"
                        }
                    ),
                    changed,
                ));
            }
            if self
                .sim
                .treaty_can_offer_embassy(side, if side == HUMAN { owner } else { HUMAN })
            {
                let mut changed = offer;
                changed.terms[side_index] = changed.terms[side_index].set_embassy(true);
                groups[2].1.push(("Embassy access".to_owned(), changed));
            }
            if self.sim.treaty_can_offer_vision(
                side,
                if side == HUMAN { owner } else { HUMAN },
                after,
            ) {
                let mut changed = offer;
                changed.terms[side_index] = changed.terms[side_index].set_shared_vision(true);
                groups[2].1.push(("Shared vision".to_owned(), changed));
            }
            for city_id in self.sim.treaty_trade_cities(side, after) {
                if let Some(city) = self.sim.city_by_id(city_id) {
                    let mut changed = offer;
                    changed.terms[side_index] = changed.terms[side_index].set_city(Some(city_id));
                    groups[3].1.push((city.name.clone(), changed));
                }
            }

            let item_top = gold_y + 57.0;
            let list_bottom = rect.bottom() - 30.0;
            let list = Rect::new(rect.x, item_top, rect.w, (list_bottom - item_top).max(45.0));
            draw_rectangle(
                list.x,
                list.y,
                list.w,
                list.h,
                Color::from_rgba(35, 38, 35, 255),
            );
            draw_rectangle_lines(
                list.x,
                list.y,
                list.w,
                list.h,
                1.0,
                Color::from_rgba(112, 108, 91, 255),
            );
            draw_text(
                if side == HUMAN { "Offer" } else { "Request" },
                list.x + 6.0,
                list.y + 16.0,
                12.0,
                LIGHTGRAY,
            );
            let mut group = self.treaty_item_group[side].min(groups.len() - 1);
            if groups[group].1.is_empty() {
                group = groups
                    .iter()
                    .position(|(_, items)| !items.is_empty())
                    .unwrap_or(group);
            }
            let heading_y = list.y + 23.0;
            let heading_h = 22.0;
            for (index, (name, group_items)) in groups.iter().enumerate() {
                let y = heading_y + index as f32 * heading_h;
                let label = format!(
                    "{} {} ({})",
                    if index == group { "v" } else { ">" },
                    name,
                    group_items.len()
                );
                text_in(
                    &label,
                    list.x + 7.0,
                    y + 15.0,
                    12.0,
                    list.w - 14.0,
                    if index == group {
                        GOLD
                    } else if group_items.is_empty() {
                        GRAY
                    } else {
                        LIGHTGRAY
                    },
                );
                if index != group && can_negotiate && !group_items.is_empty() {
                    actions.push((
                        Rect::new(list.x + 4.0, y, list.w - 8.0, heading_h),
                        UiAction::TreatyItemGroup(side, index),
                    ));
                }
            }
            let items = &groups[group].1;
            let page_size = 10usize;
            let last_page = items.len().saturating_sub(1) / page_size;
            let page = self.treaty_item_page[side].min(last_page);
            for (row, (label, changed)) in items
                .iter()
                .skip(page * page_size)
                .take(page_size)
                .enumerate()
            {
                let y = heading_y + groups.len() as f32 * heading_h + row as f32 * 20.0;
                let row_rect = Rect::new(list.x + 14.0, y - 2.0, list.w - 20.0, 19.0);
                let item_label = format!("· {label}");
                text_in(&item_label, row_rect.x, y + 12.0, 12.0, row_rect.w, WHITE);
                if can_negotiate {
                    actions.push((row_rect, UiAction::EditTreatyDraft(owner, *changed)));
                }
            }
            if items.is_empty() {
                draw_text(
                    "No items available",
                    list.x + 14.0,
                    heading_y + groups.len() as f32 * heading_h + 15.0,
                    12.0,
                    LIGHTGRAY,
                );
            }
            if last_page > 0 {
                let previous = Rect::new(rect.x, rect.bottom() - 24.0, 68.0, 22.0);
                let next = Rect::new(rect.right() - 68.0, rect.bottom() - 24.0, 68.0, 22.0);
                draw_ui_button(previous, "Prev", page > 0, false);
                draw_ui_button(next, "Next", page < last_page, false);
                let page_label = format!("{} / {}", page + 1, last_page + 1);
                let page_width = measure_text(&page_label, None, 12, 1.0).width;
                draw_text(
                    &page_label,
                    rect.x + (rect.w - page_width) / 2.0,
                    rect.bottom() - 8.0,
                    12.0,
                    LIGHTGRAY,
                );
                if page > 0 {
                    actions.push((previous, UiAction::TreatyItemsPage(side, page - 1)));
                }
                if page < last_page {
                    actions.push((next, UiAction::TreatyItemsPage(side, page + 1)));
                }
            }
        }

        let center = Rect::new(center_x, content_y, center_width, side_height);
        let portrait = assets.leader_portrait(
            self.sim.players()[owner].civilization_id.as_deref(),
            attitude,
        );
        let max_portrait_w = (center_width * 0.5).clamp(116.0, 168.0);
        let max_portrait_h = (side_height * 0.35).clamp(148.0, 182.0);
        let portrait_aspect = portrait.map_or(0.75, |texture| texture.width() / texture.height());
        let (portrait_w, portrait_h) = if max_portrait_w / max_portrait_h > portrait_aspect {
            (max_portrait_h * portrait_aspect, max_portrait_h)
        } else {
            (max_portrait_w, max_portrait_w / portrait_aspect)
        };
        let portrait_rect = Rect::new(
            center.x + (center.w - portrait_w) / 2.0,
            center.y,
            portrait_w,
            portrait_h,
        );
        draw_rectangle(
            portrait_rect.x - 2.0,
            portrait_rect.y - 2.0,
            portrait_rect.w + 4.0,
            portrait_rect.h + 4.0,
            Color::from_rgba(16, 17, 16, 255),
        );
        if let Some(portrait) = portrait {
            draw_texture_ex(
                portrait,
                portrait_rect.x,
                portrait_rect.y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(portrait_rect.w, portrait_rect.h)),
                    ..Default::default()
                },
            );
        }
        draw_rectangle_lines(
            portrait_rect.x - 2.0,
            portrait_rect.y - 2.0,
            portrait_rect.w + 4.0,
            portrait_rect.h + 4.0,
            2.0,
            Color::from_rgba(194, 160, 82, 255),
        );
        let leader_y = portrait_rect.bottom() + 19.0;
        let leader_name = format!(
            "{} · {}",
            self.sim.players()[owner].name,
            self.sim.players()[owner].leader_name()
        );
        let leader_width = measure_text(&leader_name, None, 17, 1.0).width;
        draw_text(
            &leader_name,
            center.x + (center.w - leader_width) / 2.0,
            leader_y,
            17.0,
            WHITE,
        );
        let pact = self.sim.pact(HUMAN, owner);
        let default_dialogue = match pact {
            Pact::War => "Our nations are at war. We can discuss a ceasefire.",
            Pact::Ceasefire | Pact::Armistice => {
                "Our forces are holding position. What terms do you propose?"
            }
            Pact::Peace => "Our diplomats are ready to discuss our relations.",
            Pact::Alliance => "Our alliance joins our fortunes. What shall we discuss?",
        };
        let dialogue = self
            .treaty_feedback
            .as_ref()
            .filter(|(target, _)| *target == owner)
            .map_or(default_dialogue, |(_, message)| message.as_str());
        let speech = Rect::new(center.x, leader_y + 7.0, center.w, 38.0);
        draw_rectangle(
            speech.x,
            speech.y,
            speech.w,
            speech.h,
            Color::from_rgba(35, 38, 35, 255),
        );
        draw_rectangle_lines(
            speech.x,
            speech.y,
            speech.w,
            speech.h,
            1.0,
            Color::from_rgba(112, 108, 91, 255),
        );
        text_in(
            dialogue,
            speech.x + 8.0,
            speech.y + 23.0,
            12.0,
            speech.w - 16.0,
            WHITE,
        );

        let treaty_y = speech.bottom() + 8.0;
        let treaty_label = if let Some(pact) = offer.pact {
            format!("Treaty: {}", pact.name())
        } else {
            "No treaty clause".to_owned()
        };
        text_in(
            &treaty_label,
            center.x,
            treaty_y + 14.0,
            14.0,
            center.w - 95.0,
            GOLD,
        );
        if offer.pact.is_some() {
            let remove = Rect::new(center.right() - 88.0, treaty_y - 2.0, 88.0, 22.0);
            draw_ui_button(remove, "Remove", can_negotiate, false);
            if can_negotiate {
                let mut changed = offer;
                changed.pact = None;
                actions.push((remove, UiAction::EditTreatyDraft(owner, changed)));
            }
        } else if let Some(next_pact) = self.sim.next_pact(HUMAN, owner) {
            let add = Rect::new(center.right() - 88.0, treaty_y - 2.0, 88.0, 22.0);
            draw_ui_button(
                add,
                format!("Add {}", next_pact.name()).as_str(),
                can_negotiate,
                false,
            );
            if can_negotiate {
                let mut changed = offer;
                changed.pact = Some(next_pact);
                actions.push((add, UiAction::EditTreatyDraft(owner, changed)));
            }
        }

        let table_y = treaty_y + 22.0;
        draw_text("Offer on the table", center.x, table_y + 15.0, 14.0, WHITE);
        let table_top = table_y + 20.0;
        let col_gap = 7.0;
        let col_w = (center.w - col_gap) / 2.0;
        let terms_actions = self.draw_treaty_terms_column(
            owner,
            0,
            offer,
            Rect::new(
                center.x,
                table_top,
                col_w,
                center.bottom() - table_top - 35.0,
            ),
            "You offer",
            can_negotiate,
        );
        actions.extend(terms_actions);
        let terms_actions = self.draw_treaty_terms_column(
            owner,
            1,
            offer,
            Rect::new(
                center.x + col_w + col_gap,
                table_top,
                col_w,
                center.bottom() - table_top - 35.0,
            ),
            "They offer",
            can_negotiate,
        );
        actions.extend(terms_actions);

        let footer_y = panel.bottom() - 40.0;
        let mut footer = vec![("All civilizations", UiAction::OpenDiplomacyOverview)];
        if let Some(ai_offer) = ai_offer {
            footer.push(("Accept offer", UiAction::AcceptOffer(owner, ai_offer)));
            footer.push(("Decline", UiAction::DeclineOffer(owner)));
        }
        if can_negotiate && (offer.pact.is_some() || offer.bundled()) {
            footer.push(("Propose terms", UiAction::ProposeTreaty(owner, offer)));
        }
        let mut x = panel.x + 16.0;
        for (label, action) in footer {
            let button_width = match action {
                UiAction::ProposeTreaty(..) => 116.0,
                UiAction::OpenDiplomacyOverview => 145.0,
                _ => 94.0,
            };
            let button = Rect::new(x, footer_y, button_width, 27.0);
            let primary = matches!(action, UiAction::ProposeTreaty(..));
            draw_ui_button(button, label, true, primary);
            actions.push((button, action));
            x += button_width + 7.0;
        }
        if !can_negotiate && ai_offer.is_none() {
            text_in(
                "Diplomatic contact has lapsed. Meet them again to negotiate.",
                center.x,
                center.y + center.h - 12.0,
                12.0,
                center.w,
                LIGHTGRAY,
            );
        }
        actions
    }

    fn draw_treaty_terms_column(
        &self,
        target: usize,
        side: usize,
        offer: TreatyOffer,
        rect: Rect,
        title: &str,
        editable: bool,
    ) -> Vec<(Rect, UiAction)> {
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Color::from_rgba(35, 38, 35, 255),
        );
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            1.0,
            Color::from_rgba(112, 108, 91, 255),
        );
        draw_text(
            title,
            rect.x + 6.0,
            rect.y + 15.0,
            12.0,
            Color::from_rgba(230, 208, 151, 255),
        );
        let terms = offer.terms[side];
        let mut rows: Vec<(String, TreatyOffer)> = Vec::new();
        if terms.gold > 0 {
            let mut changed = offer;
            changed.terms[side] = terms.set_gold(0);
            rows.push((format!("{} gold", terms.gold), changed));
        }
        if let Some(technology) = terms.technology {
            let mut changed = offer;
            changed.terms[side] = terms.set_technology(None);
            rows.push((technology.name().to_owned(), changed));
        }
        if let Some(map) = terms.map() {
            let mut changed = offer;
            changed.terms[side] = terms.set_map(None);
            rows.push((format!("{:?} map", map), changed));
        }
        if terms.shared_vision() {
            let mut changed = offer;
            changed.terms[side] = terms.set_shared_vision(false);
            rows.push(("Shared vision".to_owned(), changed));
        }
        if terms.embassy() {
            let mut changed = offer;
            changed.terms[side] = terms.set_embassy(false);
            rows.push(("Embassy access".to_owned(), changed));
        }
        if let Some(city_id) = terms.city() {
            let name = self
                .sim
                .cities()
                .iter()
                .find(|city| city.id == city_id)
                .map_or("Unavailable city", |city| city.name.as_str());
            let mut changed = offer;
            changed.terms[side] = terms.set_city(None);
            rows.push((name.to_owned(), changed));
        }
        let mut actions = Vec::new();
        if rows.is_empty() {
            draw_text(
                "No concessions",
                rect.x + 6.0,
                rect.y + 38.0,
                11.0,
                LIGHTGRAY,
            );
        }
        for (index, (label, changed)) in rows.into_iter().enumerate() {
            let y = rect.y + 35.0 + index as f32 * 23.0;
            text_in(&label, rect.x + 6.0, y + 11.0, 11.0, rect.w - 34.0, WHITE);
            let remove = Rect::new(rect.right() - 25.0, y - 4.0, 19.0, 20.0);
            draw_ui_button(remove, "X", editable, false);
            if editable {
                actions.push((remove, UiAction::EditTreatyDraft(target, changed)));
            }
        }
        actions
    }
}
