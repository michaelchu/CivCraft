//! Top-of-screen turn, economy, and navigation controls.

use super::*;

impl Game {
    pub(super) fn draw_top_bar(&self, actions: &mut Vec<(Rect, UiAction)>) {
        let width = screen_width();

        let Some(view) = self.sim.top_bar_view(HUMAN) else {
            return;
        };
        let status = view.spaceship_status;
        let button_count = 8 + usize::from(status.is_some());
        let button_width = 38.0;
        let step = button_width + 4.0;
        let button_start = width - HUD_MARGIN - button_count as f32 * step + 4.0;

        let in_anarchy = view.anarchy;
        let rates = if in_anarchy { [0, 0, 100] } else { view.rates };
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
                HudIcon::Target,
                UiAction::ToggleEspionage,
                "Espionage".to_owned(),
                None,
                !self.sim.espionage_rivals_view().is_empty(),
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
            let enabled = !in_anarchy && rate < view.max_rate;
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
                view.can_launch_spaceship,
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

        let music_rect = Rect::new(
            button_start + (button_count - 1) as f32 * step,
            5.0,
            button_width,
            33.0,
        );
        draw_ui_button(music_rect, "", true, self.music_open);
        let music_color = Color::from_rgba(241, 238, 218, 255);
        let music_center = vec2(music_rect.x + music_rect.w * 0.5, music_rect.y + 16.5);
        draw_hud_icon(HudIcon::Music, music_center, 21.0, music_color);
        if self.music_muted {
            draw_line(
                music_center.x - 7.0,
                music_center.y + 7.0,
                music_center.x + 7.0,
                music_center.y - 7.0,
                2.0,
                music_color,
            );
        }
        actions.push((music_rect, UiAction::ToggleMusicPanel));
        if music_rect.contains(ui_pointer()) {
            tooltip = Some((music_rect, "Audio controls".to_owned()));
        }

        if self.music_open {
            let panel = Rect::new(width - HUD_MARGIN - 190.0, CONTROLS_H + 4.0, 190.0, 132.0);
            draw_hud_card(panel);
            draw_text("Music", panel.x + 10.0, panel.y + 19.0, 14.0, GOLD);

            let mute = Rect::new(panel.x + 9.0, panel.y + 25.0, 66.0, 30.0);
            draw_ui_button(
                mute,
                if self.music_muted { "Muted" } else { "On" },
                true,
                false,
            );
            actions.push((mute, UiAction::ToggleMusicMute));

            let down = Rect::new(panel.x + 83.0, panel.y + 25.0, 28.0, 30.0);
            draw_ui_button(down, "−", self.music_volume > 0, false);
            if self.music_volume > 0 {
                actions.push((down, UiAction::AdjustMusicVolume(-1)));
            }
            let volume = format!("{}%", self.music_volume);
            let volume_width = measure_text(&volume, None, 13, 1.0).width;
            draw_text(
                &volume,
                panel.x + 117.0 - volume_width * 0.5,
                panel.y + 44.0,
                13.0,
                WHITE,
            );
            let up = Rect::new(panel.x + 133.0, panel.y + 25.0, 28.0, 30.0);
            draw_ui_button(up, "+", self.music_volume < 100, false);
            if self.music_volume < 100 {
                actions.push((up, UiAction::AdjustMusicVolume(1)));
            }

            draw_text("Sound effects", panel.x + 10.0, panel.y + 77.0, 14.0, GOLD);
            let sfx_mute = Rect::new(panel.x + 9.0, panel.y + 84.0, 66.0, 30.0);
            draw_ui_button(
                sfx_mute,
                if self.sfx_muted { "Muted" } else { "On" },
                true,
                false,
            );
            actions.push((sfx_mute, UiAction::ToggleSfxMute));
            let sfx_down = Rect::new(panel.x + 83.0, panel.y + 84.0, 28.0, 30.0);
            draw_ui_button(sfx_down, "−", self.sfx_volume > 0, false);
            if self.sfx_volume > 0 {
                actions.push((sfx_down, UiAction::AdjustSfxVolume(-1)));
            }
            let sfx_volume = format!("{}%", self.sfx_volume);
            let sfx_width = measure_text(&sfx_volume, None, 13, 1.0).width;
            draw_text(
                &sfx_volume,
                panel.x + 117.0 - sfx_width * 0.5,
                panel.y + 103.0,
                13.0,
                WHITE,
            );
            let sfx_up = Rect::new(panel.x + 133.0, panel.y + 84.0, 28.0, 30.0);
            draw_ui_button(sfx_up, "+", self.sfx_volume < 100, false);
            if self.sfx_volume < 100 {
                actions.push((sfx_up, UiAction::AdjustSfxVolume(1)));
            }
            actions.push((panel, UiAction::BlockMapClick));
        }
        if let Some((button, label)) = tooltip {
            let width = measure_text(&label, None, 15, 1.0).width + 16.0;
            let rect = Rect::new(
                (button.right() - width).max(HUD_MARGIN),
                button.bottom() + 3.0,
                width,
                24.0,
            );
            draw_hud_card(rect);
            draw_text(&label, rect.x + 8.0, rect.y + 17.0, 15.0, WHITE);
        }
    }
}
