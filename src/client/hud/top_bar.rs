//! Top-of-screen turn, economy, and navigation controls.

use super::*;

/// The button row starts at the same top margin as the messages on the left.
const BUTTON_TOP: f32 = HUD_MARGIN;
const BUTTON_H: f32 = 33.0;

/// Strip covering the floating buttons and the gaps around them, so a near-miss
/// click is not delivered to the map underneath.
fn controls_row_rect(button_start: f32, width: f32) -> Rect {
    let left = button_start - 4.0;
    Rect::new(
        left,
        0.0,
        (width - left).max(0.0),
        BUTTON_TOP + BUTTON_H + 5.0,
    )
}

/// Left edge of the floating button row: five buttons, plus one while a
/// spaceship status is shown, right-aligned to the HUD margin.
pub(super) fn controls_start_x(screen_width: f32, has_spaceship: bool) -> f32 {
    let count = 5 + usize::from(has_spaceship);
    screen_width - HUD_MARGIN - count as f32 * 42.0 + 4.0
}

impl Game {
    /// Whether the spaceship button is currently part of the button row.
    pub(super) fn has_spaceship_button(&self) -> bool {
        self.sim
            .top_bar_view(HUMAN)
            .is_some_and(|view| view.spaceship_status.is_some())
    }

    pub(super) fn draw_top_bar(&self, actions: &mut Vec<(Rect, UiAction)>) {
        let width = screen_width();

        let Some(view) = self.sim.top_bar_view(HUMAN) else {
            return;
        };
        let status = view.spaceship_status;
        let button_count = 5 + usize::from(status.is_some());
        let button_width = 38.0;
        let step = button_width + 4.0;
        let button_start = controls_start_x(width, status.is_some());

        let mut controls = vec![
            (
                HudIcon::Government,
                UiAction::ToggleGovernment,
                "Government".to_owned(),
                true,
                false,
            ),
            (
                HudIcon::Diplomacy,
                UiAction::ToggleDiplomacy,
                "Diplomacy".to_owned(),
                true,
                false,
            ),
            (
                HudIcon::Target,
                UiAction::ToggleEspionage,
                "Espionage".to_owned(),
                !self.sim.espionage_rivals_view().is_empty(),
                false,
            ),
            (
                HudIcon::Research,
                UiAction::OpenResearch,
                "Research tree".to_owned(),
                true,
                false,
            ),
        ];
        if let Some(status) = status {
            controls.push((
                HudIcon::Spaceship,
                UiAction::ToggleSpaceship,
                status,
                true,
                view.can_launch_spaceship,
            ));
        }

        let mut tooltip = None;
        for (index, (icon, action, label, enabled, accent)) in controls.into_iter().enumerate() {
            let rect = Rect::new(
                button_start + index as f32 * step,
                BUTTON_TOP,
                button_width,
                BUTTON_H,
            );
            draw_ui_button(rect, "", enabled, accent);
            let color = if enabled {
                Color::from_rgba(241, 238, 218, 255)
            } else {
                GRAY
            };
            let center = vec2(rect.x + rect.w * 0.5, rect.y + 16.5);
            draw_hud_icon(icon, center, 21.0, color);
            if enabled {
                actions.push((rect, action));
            }
            if rect.contains(ui_pointer()) {
                tooltip = Some((rect, label));
            }
        }

        let music_rect = Rect::new(
            button_start + (button_count - 1) as f32 * step,
            BUTTON_TOP,
            button_width,
            BUTTON_H,
        );
        draw_ui_button(music_rect, "", true, self.music_open);
        let music_color = Color::from_rgba(241, 238, 218, 255);
        let music_center = vec2(music_rect.x + music_rect.w * 0.5, music_rect.y + 16.5);
        let music_icon = if self.music_muted {
            HudIcon::MusicMuted
        } else {
            HudIcon::Music
        };
        draw_hud_icon(music_icon, music_center, 21.0, music_color);
        actions.push((music_rect, UiAction::ToggleMusicPanel));
        // Listed after the buttons so they win; this only catches the gaps and margins.
        actions.push((
            controls_row_rect(button_start, width),
            UiAction::BlockMapClick,
        ));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_start_x_ends_at_the_margin_and_a_spaceship_button_widens_the_row() {
        let step = 42.0;
        let plain = controls_start_x(1280.0, false);
        assert_eq!(plain + 5.0 * step - 4.0, 1280.0 - HUD_MARGIN);
        assert_eq!(controls_start_x(1280.0, true), plain - step);
    }

    #[test]
    fn controls_row_blocks_clicks_in_gaps_and_margins_but_not_beside_it() {
        let row = controls_row_rect(1000.0, 1280.0);
        // Gap between two 38px buttons that sit 42px apart, plus the strips above and below.
        assert!(row.contains(vec2(1000.0 + 40.0, 20.0)));
        assert!(row.contains(vec2(1100.0, 1.0)));
        assert!(row.contains(vec2(1100.0, BUTTON_TOP + BUTTON_H + 4.0)));
        assert!(!row.contains(vec2(1100.0, BUTTON_TOP + BUTTON_H + 8.0)));
        assert!(!row.contains(vec2(900.0, 20.0)));
    }

    #[test]
    fn buttons_share_the_top_margin_with_the_messages() {
        assert_eq!(BUTTON_TOP, HUD_MARGIN);
    }
}
