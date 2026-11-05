//! Opt-in visual QA through the actual renderer, using isolated fixture games.
use super::*;
pub(crate) const UI_REVIEW_SCENES: &[&str] = &[
    "world",
    "city-units",
    "city-buildings",
    "city-tiles",
    "worklist",
    "government",
    "diplomacy",
    "city-deals",
    "treaty",
    "research",
    "spaceship",
    "spy",
    "investigation",
    "worker-actions",
];
impl Game {
    pub(crate) fn prepare_ui_review(&mut self, scene: usize, seed: u64) {
        *self = Game::new(crate::mapgen::generate(seed));
        self.autosave_enabled = false;
        if scene == 0 {
            return;
        }
        let (city, spy, rival) = self.sim.ui_review_fixture();
        self.selected_unit = None;
        match scene {
            1..=4 => {
                self.selected_city = Some(city);
                self.production_page = usize::from(scene == 2);
                self.city_tiles = scene == 3;
                self.worklist_page = (scene == 4).then_some(0);
            }
            5 => self.government_open = true,
            6 => self.diplomacy_open = true,
            7 => {
                self.diplomacy_open = true;
                self.city_deals = Some((1, 0));
            }
            8 => {
                self.diplomacy_open = true;
                self.reviewed_offer = Some((1, self.sim.ui_review_offer()));
            }
            9 => {
                self.research_open = true;
                self.center_research();
            }
            10 => {
                self.sim.manual_spaceship_playtest_setup();
                self.spaceship_open = true;
            }
            11 => {
                self.sim.ui_review_spy_war();
                self.selected_unit = Some(spy);
                self.spy_targets = Some((rival, 0));
            }
            12 => self.investigation = self.sim.investigate_city(spy, rival),
            13 => {
                self.selected_unit = self
                    .sim
                    .units()
                    .iter()
                    .find(|u| u.owner == HUMAN && u.kind == UnitKind::Engineer)
                    .map(|u| u.id);
            }
            _ => {}
        }
        self.focus_camera_on(20, 24);
    }
}

impl Game {
    /// Exercise real page actions after the first screenshot, then render the new state.
    pub(crate) fn advance_ui_review(&mut self, scene: usize, buttons: &[(Rect, UiAction)]) {
        let next = buttons
            .iter()
            .find(|(_, action)| match (scene, action) {
                (1, UiAction::UnitProductionPage(page)) => *page > 0,
                (2, UiAction::ProductionPage(page)) => *page > 1,
                (6, UiAction::DiplomacyPage(page)) => *page > 0,
                (7, UiAction::CityDeals(Some((_, page)))) => *page > 0,
                (11, UiAction::SpyTargets(Some((_, page)))) => *page > 0,
                (12, UiAction::InvestigationPage(page)) => *page > 0,
                (13, UiAction::ActionPage(page)) => *page > 0,
                _ => false,
            })
            .map(|(_, action)| *action);
        if let Some(action) = next {
            self.perform_action(action);
        }
    }
}
