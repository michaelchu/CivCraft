//! Automated gameplay scenarios covering regression behavior.
//!
//! Scenarios that need the window (real input routing or HUD state) are
//! scripted in the submodules. Window-free scenarios are defined once in
//! [`regression`] and replayed here on their historical frames so the
//! self-test report keeps the same checks in the same order.

use super::*;
use crate::sim::scenarios::regression;

#[path = "regression_scenarios/early.rs"]
mod early;
#[path = "regression_scenarios/military.rs"]
mod military;
#[path = "regression_scenarios/native_rules.rs"]
mod native_rules;
#[path = "regression_scenarios/research.rs"]
mod research;

/// The frame on which each registered window-free scenario reports.
const REGISTERED_SCENARIO_FRAMES: &[(u32, &str)] = &[
    (416, "barbarian-uprisings"),
    (422, "ai-expansion"),
    (425, "overseas-settlement"),
    (428, "settlement-ferry-production"),
    (431, "inland-overseas-settlement"),
    (434, "ferry-research"),
    (437, "offshore-settlement-pickup"),
    (440, "inland-ferry-pickup"),
    (443, "rendezvous-route-selection"),
    (446, "city-border-claims"),
    (448, "culture-accumulation"),
    (449, "fogged-borders"),
    (452, "ai-city-defense"),
    (455, "ai-fortifications"),
    (458, "ai-reinforcements"),
    (461, "ai-counterattacks"),
    (470, "ai-fighter-strike"),
    (476, "campaign-save-load"),
    (484, "ai-paradrops"),
    (487, "paradrop-follow-up"),
    (490, "government-research"),
    (493, "government-worker-preview"),
    (496, "ai-domestic-buildings"),
    (499, "ai-ground-target"),
    (502, "ai-overseas-assault"),
    (505, "ai-defended-overseas-assault"),
    (519, "disband-and-recycle"),
    (521, "transport-cargo-rescue"),
    (523, "nested-transport-rescue"),
    (525, "movement-cost-routing"),
    (527, "fog-of-war-routing"),
    (529, "remembered-map-routing"),
    (531, "ai-emergency-defense-purchase"),
    (533, "ai-emergency-defense-sale"),
    (535, "ai-civilian-upgrade-budget"),
    (537, "ai-purchase-reserve-tax"),
    (553, "diplomacy-map-exchange"),
    (557, "diplomacy-city-clause"),
    (559, "diplomacy-city-clause-bundle"),
    (563, "ai-defense-route-cost"),
    (565, "ai-road-reinforcements"),
    (569, "ai-map-observation-transfer"),
    (571, "ai-war-preparation"),
    (574, "ai-growth-expansion"),
    (577, "ai-space-strategy"),
    (580, "ai-research-switching"),
    (583, "ai-aircraft-advisor"),
    (586, "nuclear-ai-role-mapping"),
    (592, "ai-ground-invasion"),
    (601, "ai-settlement-value"),
    (604, "ai-settlement-goal-government"),
    (607, "ai-settlement-reservations"),
    (610, "ai-future-defenders"),
    (625, "ai-government-reevaluation"),
    (628, "ai-embarked-paradrop-danger"),
    (634, "ai-ferry-assignment-state"),
    (637, "ai-happiness-effects"),
    (640, "industrial-shield-output"),
    (646, "ai-city-manager"),
    (649, "ai-treaty-policy"),
    (655, "ai-worker-remote-work"),
    (658, "ai-exploration"),
    (661, "ai-founder-work-arbitration"),
    (667, "city-output-rules"),
    (673, "ai-idle-exploration"),
    (676, "exploration-legality"),
    (679, "ai-guard-selection"),
    (682, "ai-guard-ferry-estimate"),
    (685, "damaged-movement"),
    (688, "ai-bodyguard-production"),
    (691, "settlement-worker-order"),
    (694, "barbarian-output-waste"),
    (697, "civilian-peace-borders"),
    (703, "native-ai-tax-manager"),
    (706, "military-training-choice"),
    (709, "zero-attack-carrier-path"),
    (712, "ai-tax-before-city-advice"),
    (715, "native-infrastructure-costs"),
    (718, "source-ai-goto-profiles"),
    (724, "native-player-and-tile-order"),
    (727, "native-city-activity-order"),
    (733, "native-defense-quiet-city"),
    (736, "native-worker-allocation-founding"),
    (739, "native-gameplay-random-stream"),
    (742, "native-cargo-boarding-order"),
    (748, "native-repair-before-construction"),
    (751, "native-city-healing"),
    (754, "native-defense-effect-baseline"),
    (757, "native-per-unit-road-completion"),
    (763, "native-complete-unit-catalog-stats"),
    (772, "native-transport-oil-platform"),
    (775, "native-land-hunter-dispatch-and-scan"),
    (781, "native-counter-research-quiet-city"),
    (784, "native-research-positive-float-accumulation"),
    (787, "native-research-retained-phase-snapshot"),
    (793, "native-offensive-known-target-policy"),
    (796, "native-research-gift-clears-matching-goal"),
    (799, "native-unset-goal-save-and-command-rules"),
];

/// The reference map every registered scenario runs on in the self-test,
/// independent of the campaign seed under test.
const REGISTERED_SCENARIO_MAP_SEED: u64 = 42;

impl Playtest {
    pub(super) fn advance_regression_scenarios(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
    ) {
        self.advance_registered_scenarios(frame);
        self.advance_early_scenarios(frame, game, buttons);
        self.advance_military_scenarios(frame, game, buttons);
        self.advance_native_rule_scenarios(frame, game);
        self.advance_research_scenarios(frame, game, buttons);
    }

    fn advance_registered_scenarios(&mut self, frame: u32) {
        for &(_, id) in REGISTERED_SCENARIO_FRAMES
            .iter()
            .filter(|(scheduled, _)| *scheduled == frame)
        {
            let scenario = regression::find(id)
                .unwrap_or_else(|| panic!("self-test schedules unregistered scenario {id}"));
            let mut campaign =
                sim::Simulation::new(crate::mapgen::generate(REGISTERED_SCENARIO_MAP_SEED));
            self.results.extend(
                scenario
                    .run(&mut campaign)
                    .into_iter()
                    .map(|check| (check.name, check.passed, check.detail)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_scheduled_scenario_is_registered_once_in_frame_order() {
        let ids: BTreeSet<_> = REGISTERED_SCENARIO_FRAMES
            .iter()
            .map(|(_, id)| *id)
            .collect();
        assert_eq!(ids.len(), REGISTERED_SCENARIO_FRAMES.len());
        for id in &ids {
            assert!(regression::find(id).is_some(), "{id} is not registered");
        }
        assert!(
            REGISTERED_SCENARIO_FRAMES
                .windows(2)
                .all(|pair| pair[0].0 < pair[1].0)
        );
    }

    #[test]
    fn every_new_campaign_scenario_is_replayed_by_the_self_test() {
        let scheduled: BTreeSet<_> = REGISTERED_SCENARIO_FRAMES
            .iter()
            .map(|(_, id)| *id)
            .collect();
        for scenario in regression::all() {
            if scenario.fixture == regression::Fixture::NewCampaign {
                assert!(
                    scheduled.contains(scenario.id),
                    "{} is registered but not scheduled",
                    scenario.id
                );
            }
        }
    }
}
