//! Research selection and progress display.

use super::*;

impl Game {
    fn research_viewport() -> Rect {
        Rect::new(
            0.0,
            141.0,
            screen_width(),
            (screen_height() - 232.0).max(1.0),
        )
    }

    pub(super) fn center_research(&mut self) {
        self.research_inspected = None;
        if let Some(progress) = self.sim.research_progress_view(HUMAN) {
            self.focus_research(progress.goal);
        }
    }

    pub(super) fn focus_research(&mut self, tech: Tech) {
        let current = self
            .sim
            .research_progress_view(HUMAN)
            .map_or(tech, |progress| progress.researching);
        let tech = focused_research_tech(tech, current);
        self.research_center = self.research_tree.center(tech);
        self.research_age = self.research_tree.age(tech);
        self.research_zoom = 1.0;
        self.clamp_research_center();
    }

    pub(super) fn fit_research(&mut self) {
        let viewport = Self::research_viewport();
        let bounds = self.research_tree.bounds(self.research_age);
        self.research_center = bounds.center();
        self.research_zoom = (viewport.w / (bounds.w + 40.0))
            .min(viewport.h / (bounds.h + 40.0))
            .min(1.0);
    }

    fn clamp_research_center(&mut self) {
        let viewport = Self::research_viewport();
        let bounds = self.research_tree.bounds(self.research_age);
        let padding = vec2(20.0, 20.0);
        let size = vec2(bounds.w, bounds.h) + padding * 2.0;
        let margin = (vec2(viewport.w, viewport.h) / (2.0 * self.research_zoom)).min(size * 0.5);
        let start = vec2(bounds.x, bounds.y) - padding;
        self.research_center = self
            .research_center
            .clamp(start + margin, start + size - margin);
    }

    pub(super) fn handle_research_mouse(&mut self, buttons: &[(Rect, UiAction)]) {
        let (x, y) = mouse_position();
        let mouse = vec2(x, y);
        let viewport = Self::research_viewport();
        let panning =
            is_mouse_button_down(MouseButton::Middle) || is_mouse_button_down(MouseButton::Right);
        if panning {
            if let Some(previous) = self.research_drag {
                self.research_center -= (mouse - previous) / self.research_zoom;
            } else if !viewport.contains(mouse) {
                return;
            }
            self.research_drag = Some(mouse);
        } else {
            self.research_drag = None;
            let (_, wheel) = mouse_wheel();
            if viewport.contains(mouse) && wheel != 0.0 {
                let old_zoom = self.research_zoom;
                self.research_zoom = (old_zoom * 1.15_f32.powf(wheel)).clamp(0.02, 1.7);
                self.research_center +=
                    (mouse - viewport.center()) * (1.0 / old_zoom - 1.0 / self.research_zoom);
            } else if is_mouse_button_pressed(MouseButton::Left)
                && let Some((_, action)) = buttons.iter().find(|(rect, _)| rect.contains(mouse))
            {
                self.perform_action(*action);
            }
        }
        self.clamp_research_center();
    }

    pub(super) fn draw_research(&self) -> Vec<(Rect, UiAction)> {
        let Some(progress) = self.sim.research_progress_view(HUMAN) else {
            return Vec::new();
        };
        let goal = progress.goal;
        let current = progress.researching;
        let ancestors = self.research_tree.ancestors(goal);
        let viewport = Self::research_viewport();
        let in_age = |tech: Tech| self.research_tree.age(tech) == self.research_age;
        let zoom = self.research_zoom;
        let project = |point: Vec2| viewport.center() + (point - self.research_center) * zoom;
        let (mx, my) = mouse_position();
        let mouse = vec2(mx, my);
        let background = Color::from_rgba(7, 20, 31, 255);
        let gold = Color::from_rgba(245, 193, 87, 255);
        let muted_gold = Color::from_rgba(152, 124, 71, 255);
        let mut actions = Vec::new();
        let mut hovered = None;

        // Draw the selected branch over unrelated links so it stays visible
        // wherever the routes cross.
        for highlighted in [false, true] {
            for edge in &self.research_tree.edges {
                if ancestors.contains(&edge.to) != highlighted
                    || !in_age(edge.from)
                    || !in_age(edge.to)
                {
                    continue;
                }
                let color = if !highlighted {
                    Color::from_rgba(61, 84, 99, 255)
                } else if self
                    .sim
                    .research_technology_known(HUMAN, edge.from)
                    .unwrap_or(false)
                {
                    muted_gold
                } else {
                    gold
                };
                let thickness = if highlighted { 2.5 } else { 1.0 };
                for pair in edge.points.windows(2) {
                    let a = project(pair[0]);
                    let b = project(pair[1]);
                    draw_line(a.x, a.y, b.x, b.y, thickness, color);
                }
            }
        }

        for &(tech, position) in &self.research_tree.nodes {
            if !in_age(tech) {
                continue;
            }
            let center = project(position);
            let size = NODE_SIZE * zoom;
            let card = Rect::new(
                center.x - size.x / 2.0,
                center.y - size.y / 2.0,
                size.x,
                size.y,
            );
            let Some(hitbox) = card.intersect(viewport) else {
                continue;
            };
            let Some(technology) = self.sim.technology_research_view(HUMAN, tech) else {
                continue;
            };
            let known = technology.known;
            let available = technology.available;
            let on_path = ancestors.contains(&tech);
            let hover = hitbox.contains(mouse);
            let status = if known {
                "Known"
            } else if tech == current {
                "Researching"
            } else if available {
                "Available"
            } else {
                "Locked"
            };
            let fill = if tech == goal {
                Color::from_rgba(91, 66, 28, 255)
            } else if hover {
                Color::from_rgba(57, 78, 92, 255)
            } else if known {
                Color::from_rgba(32, 61, 48, 255)
            } else if tech == current {
                Color::from_rgba(28, 66, 84, 255)
            } else if available {
                Color::from_rgba(31, 51, 66, 255)
            } else {
                Color::from_rgba(21, 35, 46, 255)
            };
            let outline = if on_path {
                gold
            } else if tech == current {
                SKYBLUE
            } else {
                fill
            };
            draw_rectangle(card.x, card.y, card.w, card.h, fill);
            if on_path || tech == current {
                draw_rectangle_lines(card.x, card.y, card.w, card.h, 2.0 * zoom, outline);
            }
            if zoom >= 0.5 {
                draw_text(
                    tech.name(),
                    card.x + 8.0 * zoom,
                    card.y + 22.0 * zoom,
                    17.0 * zoom,
                    WHITE,
                );
                draw_text(
                    format!("{status} · {} bulbs", technology.target_cost),
                    card.x + 8.0 * zoom,
                    card.y + 43.0 * zoom,
                    15.0 * zoom,
                    LIGHTGRAY,
                );
            }
            if hover {
                hovered = Some(tech);
            }
            actions.push((hitbox, UiAction::ResearchGoal(tech)));
        }

        // Paint the fixed controls over the graph and keep hitboxes within the viewport.
        draw_rectangle(0.0, 0.0, screen_width(), viewport.y, background);
        draw_rectangle(0.0, viewport.bottom(), screen_width(), 91.0, background);
        let draw_text = |text: &str, x: f32, y: f32, size: f32, color: Color| {
            text_in(text, x, y, size, screen_width() - 24.0 - x, color);
        };
        draw_text("Research tree", 24.0, 38.0, 32.0, WHITE);
        for (offset, width, label, action) in [
            (308.0, 84.0, "Fit tree", UiAction::FitResearch),
            (214.0, 86.0, "Find goal", UiAction::CenterResearch),
            (120.0, 100.0, "Close [Esc]", UiAction::CloseResearch),
        ] {
            let button = Rect::new(screen_width() - offset, 15.0, width, 30.0);
            draw_ui_button(button, label, true, false);
            actions.push((button, action));
        }
        draw_text(
            &format!(
                "Researching {}  {}/{}   |   Goal: {}",
                current.name(),
                progress.points,
                progress.target_cost,
                goal.name()
            ),
            24.0,
            68.0,
            19.0,
            SKYBLUE,
        );
        draw_text(
            "Click a goal · Right or middle drag to pan · Scroll to zoom · Gold shows the goal branch",
            24.0,
            93.0,
            16.0,
            LIGHTGRAY,
        );

        let tab_width = ((screen_width() - 48.0) / RESEARCH_AGES.len() as f32).min(190.0);
        for (age, &(name, _)) in RESEARCH_AGES.iter().enumerate() {
            let button = Rect::new(
                24.0 + age as f32 * tab_width,
                105.0,
                tab_width - UI_GAP,
                29.0,
            );
            draw_ui_button(button, name, true, self.research_age == age);
            actions.push((button, UiAction::ResearchAge(age)));
        }

        let inspected = self
            .research_inspected
            .unwrap_or_else(|| focused_research_tech(goal, current));
        let tech = hovered.unwrap_or(inspected);
        let requirements = if tech == Tech::FUTURE {
            "Requires all 82 Civ III advances".to_owned()
        } else if let Some(era) = tech.era_gate() {
            format!("Opens after all required {} advances are known", era.name())
        } else if tech.prerequisites().is_empty() {
            "No prerequisites".to_owned()
        } else {
            let names: Vec<_> = tech.prerequisites().iter().map(|req| req.name()).collect();
            format!("Requires {}", names.join(" + "))
        };
        let y = viewport.bottom();
        draw_text(
            &format!("{}  ·  {requirements}", tech.name()),
            24.0,
            y + 23.0,
            18.0,
            WHITE,
        );
        let detail = research_unlock_detail(tech);
        draw_text(&detail, 24.0, y + 45.0, 16.0, SKYBLUE);
        let mut x = 24.0;
        for &req in inspected.prerequisites() {
            if in_age(req) {
                continue;
            }
            let age = self.research_tree.age(req);
            let age_name = RESEARCH_AGES[age].0;
            let label = format!("{} ({})", req.name(), age_name);
            let width = measure_text(&label, None, 15, 1.0).width + 22.0;
            let button = Rect::new(x, y + 53.0, width, 29.0);
            draw_ui_button(button, &label, true, false);
            actions.push((button, UiAction::InspectResearch(req)));
            x += width + 8.0;
        }
        actions
    }
}

fn focused_research_tech(goal: Tech, current: Tech) -> Tech {
    if goal == Tech::UNSET { current } else { goal }
}

fn research_unlock_detail(tech: Tech) -> String {
    let mut unlocks = ClientSimulation::research_unit_unlocks(tech);
    unlocks.extend(Building::all().filter_map(|building| {
        let data = building.data();
        (!matches!(building, Building::BARRACKS_II | Building::BARRACKS_III)
            && data
                .requirements
                .contains(&(Requirement::Tech(tech.0), true)))
        .then_some(data.name)
    }));
    unlocks.extend(
        Government::targets()
            .iter()
            .copied()
            .filter(|government| government.requirement() == Some(tech))
            .map(|government| government.name()),
    );
    let art = match tech {
        Tech::STEAM_POWER => Some("Industrial"),
        Tech::MOTORIZED_TRANSPORTATION => Some("Electric Age"),
        Tech::ROCKETRY => Some("Modern"),
        Tech::SUPERCONDUCTORS => Some("Postmodern"),
        _ => None,
    };
    let mut detail = if unlocks.is_empty() {
        String::new()
    } else {
        format!("Unlocks: {}", unlocks.join(", "))
    };
    if let Some(age) = art {
        if !detail.is_empty() {
            detail.push_str(" · ");
        }
        detail.push_str(&format!("City art: {age}"));
    }
    if detail.is_empty() {
        "No direct unit or building unlock in CivCraft.".to_owned()
    } else {
        detail
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::TechEra;

    #[test]
    fn cleared_goal_focuses_current_research_without_a_phantom_future_branch() {
        let tree = ResearchTree::new();
        let focus = focused_research_tech(Tech::UNSET, Tech::BRONZE_WORKING);
        assert_eq!(focus, Tech::BRONZE_WORKING);
        assert_eq!(tree.center(focus), tree.center(Tech::BRONZE_WORKING));
        assert!(tree.ancestors(Tech::UNSET).is_empty());
        assert!(tree.nodes.iter().all(|(tech, _)| *tech != Tech::UNSET));
        assert_eq!(
            focused_research_tech(Tech::FUTURE, Tech::BRONZE_WORKING),
            Tech::FUTURE
        );
    }

    #[test]
    fn research_tree_goal_branch_includes_critical_advances_from_earlier_eras() {
        let tree = ResearchTree::new();
        let branch = tree.ancestors(Tech::COMPUTERS);

        assert!(Tech::all()
            .filter(|tech| tech.era() < TechEra::Modern && tech.required_for_civ3_era())
            .all(|tech| branch.contains(&tech)));
        assert!(branch.contains(&Tech::COMPUTERS));
    }

    #[test]
    fn research_details_include_new_units_and_buildings_together() {
        let feudalism = Tech::all().find(|tech| tech.name() == "Feudalism").unwrap();
        let detail = research_unlock_detail(feudalism);
        assert!(detail.contains("Pikeman") && detail.contains("Sun Tzu"));
        assert!(
            research_unlock_detail(Tech::ADVANCED_FLIGHT).contains("Helicopter")
                && research_unlock_detail(Tech::ADVANCED_FLIGHT).contains("Paratrooper")
        );
        let detail = research_unlock_detail(Tech::ROCKETRY);
        assert!(detail.contains("Cruise Missile") && detail.contains("City art: Modern"));
    }
}
