//! Technology availability, cost, and research progression.

use super::*;

impl Playtest {
    pub(super) fn advance_research_scenarios(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
    ) {
        if frame == 779 && self.act_passed {
            let oil = buttons
                .iter()
                .any(|(_, action)| matches!(action, UiAction::Mine));
            let cleanup = buttons.iter().any(|(_, action)| {
                matches!(
                    action,
                    UiAction::TerrainWork(crate::sim::WorkKind::Clean(_))
                )
            });
            let land = buttons.iter().any(|(_, action)| {
                matches!(
                    action,
                    UiAction::Irrigate
                        | UiAction::TerrainWork(
                            crate::sim::WorkKind::Cultivate | crate::sim::WorkKind::Plant
                        )
                )
            });
            self.results.push(("native_transport_worker_hud", oil && cleanup && !land,
                format!("Actual Transport HUD offered offshore oil work: {oil}; cleanup: {cleanup}; excluded land-only work: {}.", !land)));
        }

        if frame == 781 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (quiet, first, context) = sim.counter_defender_research_playtest();
            for (name, passed) in [
                ("native_counter_research_quiet_city", quiet),
                ("native_counter_research_first_missing_defender", first),
                ("native_counter_research_threat_context", context),
            ] {
                self.results.push((name, passed, "A quiet city requested no counter research; a reachable threat requested the native missing defender technology with post-specific urgency.".to_owned()));
            }
        }

        if frame == 784 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (positive, signed, large) = sim.native_research_arithmetic_playtest();
            for (name, passed) in [
                ("native_research_positive_float_accumulation", positive),
                ("native_research_signed_float_accumulation", signed),
                ("native_research_large_float_rounding", large),
            ] {
                self.results.push((name, passed, "Research selection matched independent C vectors with truncation at every int-plus-float assignment.".to_owned()));
            }
        }

        if frame == 787 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (retained, order, copied, loaded) = sim.native_research_phase_playtest();
            for (name, passed) in [
                ("native_research_retained_phase_snapshot", retained),
                ("native_research_ordered_feedback", order),
                ("native_research_civil_war_copy", copied),
                ("native_research_load_zero_snapshot", loaded),
            ] {
                self.results.push((name, passed, "AI research feedback persisted between phases, accumulated in order, copied on player splits and reset on load.".to_owned()));
            }
        }

        if frame == 793 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (hidden, learned) = sim.offensive_research_playtest();
            for (name, passed) in [
                ("native_offensive_known_target_policy", hidden),
                ("native_offensive_learned_tech_feedback_ends", learned),
            ] {
                self.results.push((name, passed, "Offensive target research feedback is limited to targets known by the player, and stops after its technology is learned.".to_owned()));
            }
        }

        if frame == 796 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (gift, negative, valid) = sim.native_unset_research_goal_playtest();
            for (name, passed) in [
                ("native_research_gift_clears_matching_goal", gift),
                ("native_research_negative_wants_clear_goal", negative),
                ("native_unset_goal_stays_out_of_inventions", valid),
            ] {
                self.results.push((name, passed, "Unset research goals remained distinct from Future Tech and could not become inventions or active research targets.".to_owned()));
            }
        }

        if frame == 799 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("native_unset_goal_save_and_command_rules", sim.native_unset_goal_save_playtest(),
                "An unset goal roundtripped in the existing save shape; unset current/known technologies and explicit invalid goal commands were rejected.".to_owned()));
        }

        if frame == 802 && self.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            game.sim
                .playtest_simulation_mut()
                .unset_goal_hud_playtest_setup();
            game.perform_action(UiAction::OpenResearch);
        }

        if frame == 803 && self.act_passed {
            let close = buttons.iter().find_map(|(_, action)| {
                matches!(action, UiAction::CloseResearch).then_some(*action)
            });
            let current = buttons.iter().any(|(_, action)| {
                matches!(action,
                UiAction::ResearchGoal(tech) if *tech == crate::rules::Tech::ALPHABET)
            });
            let phantom = buttons.iter().any(|(_, action)| {
                matches!(action,
                UiAction::ResearchGoal(tech) if *tech == crate::rules::Tech::UNSET)
            });
            let unset = game.sim.playtest_simulation().players()[HUMAN].research_goal
                == crate::rules::Tech::UNSET;
            self.results.push(("native_unset_goal_research_screen", unset && close.is_some() && current && !phantom,
                format!("Actual research screen opened with an unset goal: {unset}; current-project node visible: {current}; phantom unset node: {phantom}.")));
            if let Some(action) = close {
                game.perform_action(action);
            }
        }
    }
}
