//! Focused gameplay scenarios for the parent subsystem.

use super::report::PlaytestReport;
use crate::{
    client::{Game, UiAction},
    ensure_parent,
    rules::Building,
    sim::{self, Command, HUMAN, Production, UnitKind, WorklistEdit, topology::CIV3_DIRS},
};
use macroquad::prelude::Rect;
use std::fs;

#[path = "opening.rs"]
mod opening;
#[path = "regression_scenarios.rs"]
mod regression_scenarios;
#[path = "ui_script.rs"]
mod ui_script;

pub(crate) struct Playtest {
    path: String,
    move_passed: bool,
    move_detail: String,
    act_passed: bool,
    act_detail: String,
    city_tile_reassigned: Option<bool>,
    city_tiles_complete: bool,
    city_manager_closed: bool,
    tax_step_complete: bool,
    city_focus_selected: bool,
    diplomacy_probe: Option<(u32, i32, i32, usize)>,
    civil_war_attack_direction: Option<usize>,
    vision_probe: Option<(i32, i32)>,
    stealth_aircraft: Option<(u32, u32)>,
    airlift: Option<(u32, u32, u32)>,
    work_team: Option<(u32, u32)>,
    pillage_probe: Option<(i32, i32)>,
    transport_probe: Option<(u32, u32, (i32, i32))>,
    results: PlaytestReport,
    previous_passed: bool,
    detail_offset: usize,
}

impl Playtest {
    pub(crate) fn new(path: String, seed: u64) -> Self {
        Self {
            path,
            move_passed: false,
            move_detail: "No scripted move completed".to_owned(),
            act_passed: false,
            act_detail: "No scripted action completed".to_owned(),
            city_tile_reassigned: None,
            city_tiles_complete: false,
            city_manager_closed: false,
            tax_step_complete: false,
            city_focus_selected: false,
            diplomacy_probe: None,
            civil_war_attack_direction: None,
            vision_probe: None,
            transport_probe: None,
            pillage_probe: None,
            work_team: None,
            stealth_aircraft: None,
            airlift: None,
            results: PlaytestReport::new(seed),
            previous_passed: true,
            detail_offset: 0,
        }
    }

    pub(crate) fn advance(&mut self, frame: u32, game: &mut Game, buttons: &[(Rect, UiAction)]) {
        self.advance_opening(frame, game, buttons);
        self.advance_ui_script(frame, game, buttons);
        self.advance_regression_scenarios(frame, game, buttons);
        if let Some(step) = match frame {
            30 => Some("foundation"),
            65 => Some("economy"),
            90 => Some("government"),
            115 => Some("civil_war"),
            148 => Some("diplomacy"),
            _ => None,
        } {
            let detail = if !self.previous_passed {
                "Skipped because a prerequisite scenario failed".to_owned()
            } else {
                self.act_detail[self.detail_offset..]
                    .trim_start_matches(['.', ' '])
                    .to_owned()
            };
            if self.previous_passed {
                self.results.push((step, self.act_passed, detail));
            } else {
                self.results.skip(step, detail);
            }
            self.previous_passed = self.act_passed;
            self.detail_offset = self.act_detail.len();
        }
    }

    pub(crate) fn finish(&mut self, hud_count: usize) -> bool {
        let passed =
            self.move_passed && self.act_passed && hud_count >= 4 && self.results.all_passed();
        ensure_parent(&self.path);
        let hud_detail = format!("Drew {hud_count} HUD regions");
        self.results
            .push(("move", self.move_passed, self.move_detail.clone()));
        self.results
            .push(("act", self.act_passed, self.act_detail.clone()));
        self.results.push(("hud", hud_count >= 4, hud_detail));
        let json = self.results.to_json();
        if let Err(error) = fs::write(&self.path, json) {
            eprintln!("Could not write playtest report: {error}");
            return false;
        }
        passed
    }
}

#[cfg(test)]
mod report_tests {
    use super::*;
    #[test]
    fn failed_checks_return_failure_even_when_report_writes_successfully() {
        let path = std::env::temp_dir().join(format!(
            "civcraft-failed-playtest-{}.json",
            std::process::id()
        ));
        let mut test = Playtest::new(path.to_str().unwrap().into(), 42);
        assert!(!test.finish(4));
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("\"passed\":false")
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod opening_scenario_tests {
    use super::*;

    #[test]
    fn seed_42_moves_a_starting_unit_then_founds_a_city() {
        let map = crate::mapgen::generate_for_civ3_size(
            42,
            crate::mapgen::Civ3MapSize::Standard,
            crate::mapgen::Civ3WorldSettings::default(),
        );
        let mut game = Game::new(sim::Simulation::new(map));
        let mut playtest = Playtest::new("unused-report.json".to_owned(), 42);

        playtest.advance_opening(12, &mut game, &[]);
        assert!(playtest.move_passed, "{}", playtest.move_detail);
        playtest.advance_opening(30, &mut game, &[]);
        assert!(playtest.act_passed, "{}", playtest.act_detail);
    }
}
