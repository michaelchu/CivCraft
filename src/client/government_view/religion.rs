//! Religion management lives beside government, with independent commands.
use super::*;
use crate::sim::FaithPolicy;

impl Game {
    pub(in crate::client) fn draw_religion(&self) -> Vec<(Rect, UiAction)> {
        let view = self.sim.religion_management_view();
        let (faith, policy) = self
            .religion_draft
            .unwrap_or((view.current.declared, view.current.policy));
        let panel = Rect::new(
            (screen_width() - 680.0).max(0.0) / 2.0,
            (screen_height() - 620.0).max(0.0) / 2.0,
            screen_width().min(680.0),
            screen_height().min(620.0),
        );
        panel_frame(panel);
        let text = panel_text(panel);
        let x = panel.x + 18.0;
        let y = panel.y;
        text("Religion", x, y + 30.0, 24.0, WHITE);
        let close = panel_close_rect(panel);
        draw_ui_button(close, "X", true, false);
        let mut actions = vec![(close, UiAction::ToggleReligion)];
        text(
            "Choose an unclaimed religion. Each civilization may found one.",
            x,
            y + 57.0,
            15.0,
            LIGHTGRAY,
        );
        let choice = &view.founding[self.religion_choice % view.founding.len()];
        let field = Rect::new(x, y + 68.0, panel.w - 36.0, 30.0);
        draw_ui_button(
            field,
            &format!(
                "{} — {} (click to browse)",
                choice.name,
                if choice.founded {
                    "Already founded"
                } else if choice.eligible {
                    "Available"
                } else {
                    "Locked"
                }
            ),
            true,
            false,
        );
        actions.push((field, UiAction::ReligionFoundingChoice));
        let index = self.religion_city.min(view.cities.len().saturating_sub(1));
        let city = view.cities.get(index);
        let select = Rect::new(x, y + 108.0, panel.w - 190.0, 29.0);
        draw_ui_button(
            select,
            &city.map_or("No owned city".into(), |c| {
                format!("Founding city: {} (click to cycle)", c.1)
            }),
            !view.cities.is_empty(),
            false,
        );
        if !view.cities.is_empty() {
            actions.push((
                select,
                UiAction::ReligionCity((index + 1) % view.cities.len()),
            ));
        }
        let found = Rect::new(panel.right() - 157.0, y + 108.0, 139.0, 29.0);
        let can_found = city.is_some() && choice.eligible;
        draw_ui_button(found, "Found religion", can_found, false);
        if can_found {
            actions.push((found, UiAction::FoundReligion));
        }
        text(
            &format!(
                "Requires {} · {} where dominant",
                choice.requirement, choice.bonus
            ),
            x,
            y + 159.0,
            14.0,
            LIGHTGRAY,
        );
        let faith_label = faith
            .and_then(|id| view.faiths.iter().find(|f| f.0 == id))
            .map_or("No declared faith", |f| f.1.as_str());
        let current = view
            .current
            .declared
            .and_then(|id| view.faiths.iter().find(|f| f.0 == id))
            .map_or("No declared faith", |f| f.1.as_str());
        text(
            &format!(
                "Current: {current} / {} · Change available in {} turns",
                view.current.policy.name(),
                view.cooldown
            ),
            x,
            y + 186.0,
            15.0,
            GOLD,
        );
        let faith_button = Rect::new(x, y + 199.0, panel.w - 36.0, 29.0);
        draw_ui_button(
            faith_button,
            &format!("Declare: {faith_label} (click to cycle)"),
            view.cooldown == 0,
            false,
        );
        if view.cooldown == 0 {
            actions.push((faith_button, UiAction::ReligionFaith));
        }
        for (i, p) in FaithPolicy::ALL.into_iter().enumerate() {
            let row = y + 241.0 + i as f32 * 58.0;
            let button = Rect::new(x, row, 130.0, 27.0);
            draw_ui_button(button, p.name(), view.cooldown == 0, p == policy);
            if view.cooldown == 0 {
                actions.push((button, UiAction::ReligionPolicy(p)));
            }
            text_in(
                &p.description(),
                x + 142.0,
                row + 19.0,
                13.0,
                panel.w - 180.0,
                LIGHTGRAY,
            );
        }
        text(
            "Declared faiths: matching +4 attitude; different faiths depend on policy.",
            x,
            y + 428.0,
            14.0,
            LIGHTGRAY,
        );
        text(
            "Devout/Missionary need a declared faith. Changes share a 10-turn cooldown.",
            x,
            y + 448.0,
            14.0,
            LIGHTGRAY,
        );
        let confirm = Rect::new(x, y + 462.0, panel.w - 36.0, 30.0);
        let changed = (faith, policy) != (view.current.declared, view.current.policy);
        let enabled = view.cooldown == 0 && changed;
        draw_ui_button(confirm, "Confirm declared faith and policy", enabled, false);
        if enabled {
            actions.push((confirm, UiAction::ConfirmReligion));
        }
        text_in(
            &self.religion_feedback,
            x,
            y + 513.0,
            14.0,
            panel.w - 36.0,
            GOLD,
        );
        text(
            "City screens show local shares. Conversions appear in Notifications.",
            x,
            y + 543.0,
            14.0,
            LIGHTGRAY,
        );
        if let Some((id, _)) = city {
            text_in(
                &self.sim.city_religion_summary(*id),
                x,
                y + 570.0,
                14.0,
                panel.w - 36.0,
                GOLD,
            );
        }
        text_in(
            "Dominance: 50% and a 10-point lead. Established faith stays until below 40%.",
            x,
            y + 596.0,
            13.0,
            panel.w - 36.0,
            LIGHTGRAY,
        );
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn faith_and_policy_drafts_require_confirmation() {
        let mut sim = crate::sim::Simulation::new(crate::test_support::seeded_map());
        let mut world = crate::sim::fixtures::WorldBuilder::new(&mut sim);
        world.clear_units_and_cities();
        let city = world.founding_city(HUMAN, (10, 24));
        world.known_technologies(HUMAN, &[crate::rules::Tech::MEDITATION]);
        world.finish();
        let mut game = Game::new(sim);
        game.perform_action(UiAction::ToggleReligion);
        game.religion_choice = 0;
        game.perform_action(UiAction::FoundReligion);
        game.perform_action(UiAction::ReligionFaith);
        game.perform_action(UiAction::ReligionPolicy(FaithPolicy::Devout));
        assert_eq!(game.sim.religion_management_view().current.declared, None);
        game.perform_action(UiAction::ConfirmReligion);
        let view = game.sim.religion_management_view();
        assert_eq!(view.current.declared, Some(1));
        assert_eq!(view.current.policy, FaithPolicy::Devout);
        assert!(game.sim.city_religion_summary(city).contains("Buddhism"));
        game.perform_action(UiAction::ReligionPolicy(FaithPolicy::Tolerant));
        game.perform_action(UiAction::ConfirmReligion);
        assert_eq!(
            game.sim.religion_management_view().current.policy,
            FaithPolicy::Devout
        );
    }
}
