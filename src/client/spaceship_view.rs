//! Spaceship construction and launch interface.

use super::*;

impl Game {
    pub(super) fn draw_spaceship(&self) -> Vec<(Rect, UiAction)> {
        let width = (screen_width() - 32.0).min(700.0);
        let x = (screen_width() - width) / 2.0;
        let y = ((screen_height() - 384.0) / 2.0).max(12.0);
        let panel = Rect::new(x, y, width, 384.0);
        panel_frame(panel);
        let draw_text = panel_text(panel);
        draw_text("Spaceship assembly", x + 16.0, y + 32.0, 24.0, GOLD);
        let close = Rect::new(x + width - 52.0, y + 12.0, 36.0, 27.0);
        draw_ui_button(close, "X", true, false);
        let mut actions = vec![(close, UiAction::ToggleSpaceship)];

        let (unplaced, structure, components, modules, manual) = self.sim.ship_assembly();
        let status = self.sim.spaceship_status(HUMAN).unwrap_or_default();
        let launch = Rect::new(x + 16.0, y + 49.0, width - 32.0, 28.0);
        let ready = self.sim.can_launch_spaceship(HUMAN);
        draw_ui_button(launch, &status, ready, ready);
        if ready {
            actions.push((launch, UiAction::LaunchSpaceship));
        }
        let mode = Rect::new(x + 16.0, y + 85.0, width - 32.0, 27.0);
        let can_toggle = !status.starts_with("Spaceship arrives");
        draw_ui_button(
            mode,
            if manual {
                "Manual assembly • Enable automatic placement"
            } else {
                "Automatic assembly • Enable manual placement"
            },
            can_toggle,
            false,
        );
        if can_toggle {
            actions.push((mode, UiAction::ToggleManualAssembly));
        }

        draw_text(
            &format!(
                "Unplaced: {} structural / {} component / {} module",
                unplaced[0], unplaced[1], unplaced[2]
            ),
            x + 16.0,
            y + 137.0,
            17.0,
            LIGHTGRAY,
        );
        draw_text(
            "Structural slots  (slot < required slot)",
            x + 16.0,
            y + 160.0,
            16.0,
            LIGHTGRAY,
        );
        let cell = (width - 32.0) / 8.0;
        for (index, &placed) in structure.iter().enumerate() {
            let rect = Rect::new(
                x + 16.0 + (index % 8) as f32 * cell,
                y + 170.0 + (index / 8) as f32 * 29.0,
                cell - 4.0,
                25.0,
            );
            let part = ShipPart::Structural(index);
            let enabled = self.sim.can_place_ship_part(part);
            let label = if index == 0 || placed {
                format!("{}", index + 1)
            } else {
                format!("{} < {}", index + 1, STRUCTURE_PARENT[index] + 1)
            };
            draw_ui_button(rect, &label, enabled, placed);
            if enabled {
                actions.push((rect, UiAction::PlaceShipPart(part)));
            }
        }
        let cell = (width - 32.0) / 3.0;
        for (index, label) in [
            "Fuel",
            "Propulsion",
            "Habitation",
            "Life support",
            "Solar panels",
        ]
        .into_iter()
        .enumerate()
        {
            let (part, count, limit) = if index < 2 {
                (ShipPart::Component(index), components[index], 8)
            } else {
                (ShipPart::Module(index - 2), modules[index - 2], 4)
            };
            let rect = Rect::new(
                x + 16.0 + (index % 3) as f32 * cell,
                y + 297.0 + (index / 3) as f32 * 33.0,
                cell - 4.0,
                28.0,
            );
            let enabled = self.sim.can_place_ship_part(part);
            draw_ui_button(rect, &format!("{label} {count}/{limit}"), enabled, false);
            if enabled {
                actions.push((rect, UiAction::PlaceShipPart(part)));
            }
        }
        actions
    }
}
