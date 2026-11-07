use crate::{
    client::{Game, UiAction},
    ensure_parent,
    rules::Building,
    sim::{self, Command, HUMAN, Production, UnitKind, WorklistEdit, topology::ISOHEX_DIRS},
};
use macroquad::prelude::Rect;
use std::fs;

mod opening;
mod regression_scenarios;
mod ui_script;

fn json_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

pub(crate) struct Playtest {
    path: String,
    move_passed: bool,
    move_detail: String,
    act_passed: bool,
    act_detail: String,
    diplomacy_probe: Option<(u32, i32, i32)>,
    vision_probe: Option<(i32, i32)>,
    stealth_aircraft: Option<(u32, u32)>,
    merchant: Option<(u32, u32, u32, u32)>,
    migrants: Option<(u32, u32)>,
    airlift: Option<(u32, u32, u32)>,
    work_team: Option<(u32, u32)>,
    pillage_probe: Option<(i32, i32)>,
    transport_probe: Option<(u32, u32, (i32, i32))>,
    results: Vec<(&'static str, bool, String)>,
    previous_passed: bool,
    detail_offset: usize,
}

impl Playtest {
    pub(crate) fn new(path: String) -> Self {
        Self {
            path,
            move_passed: false,
            move_detail: "No scripted move completed".to_owned(),
            act_passed: false,
            act_detail: "No scripted action completed".to_owned(),
            diplomacy_probe: None,
            vision_probe: None,
            transport_probe: None,
            pillage_probe: None,
            work_team: None,
            stealth_aircraft: None,
            merchant: None,
            migrants: None,
            airlift: None,
            results: Vec::new(),
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
            self.results.push((step, self.act_passed, detail));
            self.previous_passed = self.act_passed;
            self.detail_offset = self.act_detail.len();
        }
    }

    pub(crate) fn finish(&self, hud_count: usize) -> bool {
        let passed = self.move_passed
            && self.act_passed
            && hud_count >= 4
            && self.results.iter().all(|(_, passed, _)| *passed);
        ensure_parent(&self.path);
        let hud_detail = format!("Drew {hud_count} HUD regions");
        let steps = [
            ("move", self.move_passed, self.move_detail.as_str()),
            ("act", self.act_passed, self.act_detail.as_str()),
            ("hud", hud_count >= 4, hud_detail.as_str()),
        ]
        .into_iter()
        .chain(
            self.results
                .iter()
                .map(|(step, passed, detail)| (*step, *passed, detail.as_str())),
        )
        .map(|(step, passed, detail)| {
            format!(
                "{{\"step\":\"{step}\",\"passed\":{passed},\"detail\":\"{}\"}}",
                json_escape(detail)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
        let json = format!("{{\"version\":1,\"steps\":[{steps}]}}");
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
        let test = Playtest::new(path.to_str().unwrap().into());
        assert!(!test.finish(4));
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("\"passed\":false")
        );
        std::fs::remove_file(path).unwrap();
    }
}
