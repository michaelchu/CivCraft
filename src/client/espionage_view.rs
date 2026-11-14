//! Conquests espionage screen. All mission effects are submitted as commands.

use super::*;

impl Game {
    pub(super) fn draw_espionage(&self) -> Vec<(Rect, UiAction)> {
        let width = (screen_width() - 28.0).min(900.0);
        let height = (screen_height() - 48.0).min(540.0);
        let panel = Rect::new(
            (screen_width() - width) / 2.0,
            (screen_height() - height) / 2.0,
            width,
            height,
        );
        panel_frame(panel);
        let draw_text = panel_text(panel);
        draw_text("Espionage", panel.x + 18.0, panel.y + 30.0, 24.0, WHITE);
        let close = panel_close_rect(panel);
        draw_ui_button(close, "X", true, false);
        let mut actions = vec![(close, UiAction::ToggleEspionage)];
        let rivals = self.sim.espionage_rivals_view();
        let list = Rect::new(
            panel.x + 16.0,
            panel.y + 46.0,
            panel.w * 0.30,
            panel.h - 62.0,
        );
        let selected = self
            .espionage_target
            .and_then(|owner| rivals.iter().find(|rival| rival.owner == owner));
        let rival_pages = rivals.len().saturating_sub(1) / 7;
        let rival_page = self.espionage_rival_page.min(rival_pages);
        let rival_start = rival_page * 7;
        for (index, rival) in rivals.iter().skip(rival_start).take(7).enumerate() {
            let button = Rect::new(list.x, list.y + index as f32 * 54.0, list.w, 46.0);
            let status = if rival.at_war {
                "At war"
            } else if rival.spy {
                "Spy planted"
            } else if rival.embassy {
                "Embassy"
            } else if rival.contacted {
                "Contacted"
            } else {
                "Unmet"
            };
            draw_ui_button(
                button,
                &format!("{} · {status}", rival.name),
                true,
                self.espionage_target == Some(rival.owner),
            );
            actions.push((button, UiAction::EspionageOpponent(rival.owner)));
        }
        if rival_page > 0 {
            let button = Rect::new(list.x, list.bottom() - 28.0, 44.0, 24.0);
            draw_ui_button(button, "<", true, false);
            actions.push((button, UiAction::EspionageRivalPage(rival_page - 1)));
        }
        if rival_page < rival_pages {
            let button = Rect::new(list.right() - 44.0, list.bottom() - 28.0, 44.0, 24.0);
            draw_ui_button(button, ">", true, false);
            actions.push((button, UiAction::EspionageRivalPage(rival_page + 1)));
        }
        if let Some(rival) = selected {
            let x = list.right() + 18.0;
            let right = panel.right() - 18.0;
            let top = panel.y + 55.0;
            draw_text(
                &format!(
                    "{} · {} gold",
                    rival.name,
                    self.sim.treasury_view(HUMAN).unwrap_or(0)
                ),
                x,
                top,
                17.0,
                WHITE,
            );
            draw_text(
                &format!(
                    "{} · {}",
                    if rival.embassy {
                        "Embassy established"
                    } else {
                        "No embassy"
                    },
                    if rival.spy { "Spy planted" } else { "No Spy" }
                ),
                x,
                top + 20.0,
                13.0,
                LIGHTGRAY,
            );
            if !rival.military_units.is_empty() {
                let pages = rival.military_units.len().saturating_sub(1) / 6;
                let page = self.espionage_military_page.min(pages);
                let intel = rival
                    .military_units
                    .iter()
                    .skip(page * 6)
                    .take(6)
                    .map(|(name, x, y)| format!("{name} ({x},{y})"))
                    .collect::<Vec<_>>()
                    .join(" · ");
                draw_text(
                    &format!("Military plans {}/{}: {intel}", page + 1, pages + 1),
                    x,
                    top + 35.0,
                    12.0,
                    LIGHTGRAY,
                );
                if page > 0 {
                    let button = Rect::new(right - 42.0, top + 19.0, 20.0, 20.0);
                    draw_ui_button(button, "<", true, false);
                    actions.push((button, UiAction::EspionageMilitaryPage(page - 1)));
                }
                if page < pages {
                    let button = Rect::new(right - 20.0, top + 19.0, 20.0, 20.0);
                    draw_ui_button(button, ">", true, false);
                    actions.push((button, UiAction::EspionageMilitaryPage(page + 1)));
                }
            }
            let risk_y = top
                + if rival.military_units.is_empty() {
                    32.0
                } else {
                    50.0
                };
            for (index, risk) in [
                EspionageRisk::Immediate,
                EspionageRisk::Carefully,
                EspionageRisk::Safely,
            ]
            .into_iter()
            .enumerate()
            {
                let button = Rect::new(x + index as f32 * 104.0, risk_y, 98.0, 25.0);
                draw_ui_button(button, risk.name(), true, self.espionage_risk == risk);
                actions.push((button, UiAction::EspionageRisk(risk)));
            }
            let city_y = risk_y + 34.0;
            draw_text("Target city", x, city_y + 12.0, 13.0, LIGHTGRAY);
            let city_pages = rival.cities.len().saturating_sub(1) / 6;
            let city_page = self.espionage_city_page.min(city_pages);
            if city_page > 0 {
                let button = Rect::new(x + 80.0, city_y - 4.0, 27.0, 24.0);
                draw_ui_button(button, "<", true, false);
                actions.push((button, UiAction::EspionageCityPage(city_page - 1)));
            }
            if city_page < city_pages {
                let button = Rect::new(x + 110.0, city_y - 4.0, 27.0, 24.0);
                draw_ui_button(button, ">", true, false);
                actions.push((button, UiAction::EspionageCityPage(city_page + 1)));
            }
            let city_start = city_page * 6;
            let city_w = ((right - x - 16.0) / 3.0).max(70.0);
            for (index, (id, name)) in rival.cities.iter().skip(city_start).take(6).enumerate() {
                let row = index / 3;
                let col = index % 3;
                let button = Rect::new(
                    x + col as f32 * (city_w + 8.0),
                    city_y + row as f32 * 27.0 - 4.0,
                    city_w,
                    23.0,
                );
                draw_ui_button(button, name, true, self.espionage_city == Some(*id));
                actions.push((button, UiAction::EspionageCity(*id)));
            }
            let mission_y = city_y + 60.0;
            let mission_w = ((right - x - 8.0) / 2.0).max(120.0);
            for (index, mission) in EspionageMission::ALL.into_iter().enumerate() {
                let row = index / 2;
                let col = index % 2;
                let button = Rect::new(
                    x + col as f32 * (mission_w + 8.0),
                    mission_y + row as f32 * 38.0,
                    mission_w,
                    31.0,
                );
                let has_city = self
                    .espionage_city
                    .is_some_and(|id| rival.cities.iter().any(|(city, _)| *city == id));
                let enabled = match mission {
                    EspionageMission::EstablishEmbassy => rival.can_establish_embassy,
                    EspionageMission::InvestigateCity => rival.embassy && has_city && !rival.at_war,
                    EspionageMission::StealTechnology => {
                        rival.espionage_ready && (rival.embassy || rival.spy)
                    }
                    EspionageMission::PlantSpy => rival.can_plant_spy,
                    _ => rival.espionage_ready && rival.spy,
                };
                let enabled = enabled && (!mission.needs_city() || has_city);
                draw_ui_button(button, mission.name(), enabled, false);
                if enabled {
                    actions.push((
                        button,
                        UiAction::PerformEspionage(
                            rival.owner,
                            mission,
                            mission
                                .needs_city()
                                .then_some(self.espionage_city.unwrap_or_default()),
                            self.espionage_risk,
                        ),
                    ));
                }
            }
            draw_text(
                "Missions that target a city use the selected target above.",
                x,
                panel.bottom() - 16.0,
                12.0,
                LIGHTGRAY,
            );
        } else {
            draw_text(
                "Select a contacted civilization to view embassy, Spy, and mission options.",
                list.right() + 18.0,
                panel.y + 90.0,
                15.0,
                LIGHTGRAY,
            );
        }
        actions
    }
}
