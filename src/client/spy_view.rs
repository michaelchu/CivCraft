use super::*;

impl Game {
    pub(super) fn draw_spy_targets(
        &self,
        city_id: u32,
        requested_page: usize,
    ) -> Vec<(Rect, UiAction)> {
        let Some(unit) = self
            .selected_unit_index()
            .map(|index| &self.sim.units()[index])
        else {
            return Vec::new();
        };
        let Some(city) = self.sim.cities().iter().find(|city| city.id == city_id) else {
            return Vec::new();
        };
        let w = (screen_width() - 24.0).min(500.0);
        let h = (screen_height() - 24.0).clamp(200.0, 400.0);
        let panel = Rect::new(
            (screen_width() - w) * 0.5,
            (screen_height() - h) * 0.5,
            w,
            h,
        );
        draw_rectangle(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            Color::from_rgba(42, 44, 40, 255),
        );
        draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 2.0, GOLD);
        draw_text(
            &format!("Spy targets in {}", city.name),
            panel.x + 18.0,
            panel.y + 32.0,
            22.0,
            WHITE,
        );
        draw_text(
            "Targeted missions are riskier than random missions.",
            panel.x + 18.0,
            panel.y + 54.0,
            15.0,
            LIGHTGRAY,
        );
        let close = Rect::new(panel.right() - 45.0, panel.y + 12.0, 32.0, 26.0);
        draw_ui_button(close, "X", true, false);
        let mut actions = vec![(close, UiAction::SpyTargets(None))];
        let mut targets = Vec::new();
        if self.sim.can_poison_city(unit, city) {
            targets.push((
                "Poison water supply".to_owned(),
                UiAction::PoisonCity(city_id),
            ));
        }
        if unit.kind == UnitKind::Spy && self.sim.can_steal_technology(unit, city) {
            targets.extend(
                Tech::all()
                    .chain(std::iter::once(Tech::FUTURE))
                    .filter(|&tech| self.sim.stealable_technology(unit.owner, city.owner, tech))
                    .map(|tech| {
                        (
                            format!("Steal {}", tech.name()),
                            UiAction::TargetedTheft(city_id, tech),
                        )
                    }),
            );
        }
        if unit.kind == UnitKind::Spy && self.sim.can_sabotage_city(unit, city) {
            targets.push((
                "Sabotage production".to_owned(),
                UiAction::TargetedSabotage(city_id, SabotageTarget::Production),
            ));
            targets.extend(
                city.buildings
                    .iter()
                    .copied()
                    .filter(|&building| Simulation::sabotageable_building(building))
                    .map(|building| {
                        (
                            format!("Sabotage {}", building.data().name),
                            UiAction::TargetedSabotage(city_id, SabotageTarget::Building(building)),
                        )
                    }),
            );
        }
        let rows = ((h - 107.0) / 31.0).floor().max(1.0) as usize;
        let last = targets.len().saturating_sub(1) / rows;
        let page = requested_page.min(last);
        if targets.is_empty() {
            draw_text(
                "No eligible targets.",
                panel.x + 18.0,
                panel.y + 85.0,
                16.0,
                LIGHTGRAY,
            );
        }
        for (row, (label, action)) in targets.into_iter().skip(page * rows).take(rows).enumerate() {
            let button = Rect::new(
                panel.x + 18.0,
                panel.y + 65.0 + row as f32 * 31.0,
                panel.w - 36.0,
                26.0,
            );
            draw_ui_button(button, &label, true, false);
            actions.push((button, action));
        }
        for (enabled, x, label, next) in [
            (page > 0, panel.x + 18.0, "Previous", page.saturating_sub(1)),
            (page < last, panel.right() - 96.0, "Next", page + 1),
        ] {
            if enabled {
                let button = Rect::new(x, panel.bottom() - 35.0, 78.0, 25.0);
                draw_ui_button(button, label, true, false);
                actions.push((button, UiAction::SpyTargets(Some((city_id, next)))));
            }
        }
        actions
    }
}
