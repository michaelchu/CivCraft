//! Top-of-screen turn, economy, and navigation controls.

use super::*;

impl Game {
    pub(super) fn draw_top_bar(&self, actions: &mut Vec<(Rect, UiAction)>) {
        let width = screen_width();
        draw_rectangle(0.0, 0.0, width, TOP_H, Color::from_rgba(29, 31, 28, 255));
        draw_rectangle(
            0.0,
            TOP_H - 3.0,
            width,
            3.0,
            Color::from_rgba(171, 133, 55, 255),
        );

        let player = &self.sim.players()[HUMAN];
        let status = self.sim.spaceship_status(HUMAN);
        let button_count = 6 + usize::from(status.is_some());
        let button_width = 38.0;
        let step = button_width + 4.0;
        let button_start = width - HUD_MARGIN - button_count as f32 * step + 4.0;
        let research = player.researching;
        let displays = [
            (
                format!("Turn {} · {}", self.sim.turn(), year_label(self.sim.year())),
                WHITE,
            ),
            (format!("Gold {}", player.gold), GOLD),
            (player.government.name().to_owned(), LIGHTGRAY),
            (
                format!(
                    "{} {}/{}",
                    research.name(),
                    player.research,
                    self.sim.target_cost(HUMAN, research)
                ),
                Color::from_rgba(151, 210, 231, 255),
            ),
        ];
        let mut x = HUD_MARGIN;
        if width >= 950.0 {
            text_in("CivCraft", x, 29.0, 24.0, 100.0, GOLD);
            x += 112.0;
        }
        let available = (button_start - x - HUD_MARGIN).max(0.0);
        let measured = displays
            .each_ref()
            .map(|(label, _)| measure_text(label, None, 16, 1.0).width);
        let scale = ((available - 36.0) / measured.iter().sum::<f32>()).clamp(0.0, 1.0);
        for ((label, color), measured) in displays.iter().zip(measured) {
            let slot = (measured * scale).ceil();
            text_in(label, x, 27.0, 16.0, slot, *color);
            x += slot + 11.0;
        }

        let in_anarchy = player.government == Government::Anarchy;
        let rates = if in_anarchy {
            [0, 0, 100]
        } else {
            player.rates
        };
        let mut controls = vec![
            (
                HudIcon::Government,
                UiAction::ToggleGovernment,
                "Government".to_owned(),
                None,
                true,
                false,
            ),
            (
                HudIcon::Diplomacy,
                UiAction::ToggleDiplomacy,
                "Diplomacy".to_owned(),
                None,
                true,
                false,
            ),
            (
                HudIcon::Research,
                UiAction::OpenResearch,
                "Research tree".to_owned(),
                None,
                true,
                false,
            ),
        ];
        for (index, (icon, label)) in [
            (HudIcon::Science, "Science"),
            (HudIcon::Tax, "Tax"),
            (HudIcon::Luxury, "Luxury"),
        ]
        .into_iter()
        .enumerate()
        {
            let rate = rates[index];
            let enabled = !in_anarchy && rate < player.government.max_rate();
            controls.push((
                icon,
                UiAction::RaiseRate(index),
                format!("{label} {rate}%"),
                Some(rate),
                enabled,
                false,
            ));
        }
        if let Some(status) = status {
            controls.push((
                HudIcon::Spaceship,
                UiAction::ToggleSpaceship,
                status,
                None,
                true,
                self.sim.can_launch_spaceship(HUMAN),
            ));
        }

        let mut tooltip = None;
        for (index, (icon, action, label, rate, enabled, accent)) in
            controls.into_iter().enumerate()
        {
            let rect = Rect::new(button_start + index as f32 * step, 5.0, button_width, 33.0);
            draw_ui_button(rect, "", enabled, accent);
            let color = if enabled {
                Color::from_rgba(241, 238, 218, 255)
            } else {
                GRAY
            };
            let center = vec2(
                rect.x + rect.w * 0.5,
                rect.y + if rate.is_some() { 11.0 } else { 16.5 },
            );
            draw_hud_icon(
                icon,
                center,
                if rate.is_some() { 15.0 } else { 21.0 },
                color,
            );
            if let Some(rate) = rate {
                let label = format!("{rate}%");
                let w = measure_text(&label, None, 12, 1.0).width;
                draw_text(&label, center.x - w * 0.5, rect.bottom() - 4.0, 12.0, color);
            }
            if enabled {
                actions.push((rect, action));
            }
            if rect.contains(ui_pointer()) {
                tooltip = Some((rect, label));
            }
        }
        if let Some((button, label)) = tooltip {
            let width = measure_text(&label, None, 15, 1.0).width + 16.0;
            let rect = Rect::new(
                (button.right() - width).max(HUD_MARGIN),
                TOP_H + 3.0,
                width,
                24.0,
            );
            draw_hud_card(rect);
            draw_text(&label, rect.x + 8.0, rect.y + 17.0, 15.0, WHITE);
        }
    }

    pub(super) fn draw_leader_portraits(
        &self,
        assets: &Assets,
        actions: &mut Vec<(Rect, UiAction)>,
    ) -> f32 {
        let portraits: Vec<_> = self
            .sim
            .contacted_civilizations(HUMAN)
            .into_iter()
            .filter_map(|owner| Some((owner, assets.leader_avatar_badges.get(owner)?)))
            .collect();
        let size = 56.0;
        let step = size + 5.0;
        let left = screen_width() - HUD_MARGIN - portraits.len() as f32 * step;
        for (index, (owner, badge)) in portraits.into_iter().enumerate() {
            let rect = Rect::new(left + 5.0 + index as f32 * step, TOP_H + 10.0, size, size);
            draw_texture_ex(
                badge,
                rect.x,
                rect.y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(size, size)),
                    ..Default::default()
                },
            );
            actions.push((rect, UiAction::OpenDiplomacyWith(owner)));
        }
        left
    }
}
