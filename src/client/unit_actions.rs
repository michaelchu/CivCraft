//! Unit-specific action availability and control construction.

use super::hud::{HudIcon, draw_hud_card, draw_hud_icon};
use super::*;

impl Game {
    pub(super) fn draw_action_strip(
        &self,
        actions: &mut Vec<(Rect, UiAction)>,
        available_width: f32,
    ) {
        let mut entries: Vec<(String, UiAction, bool)> = Vec::new();
        if let Some(unit_id) = self.selected_unit {
            if let Some(view) = self.sim.unit_action_view(unit_id) {
                if let Some((target, price, affordable)) = view.upgrade {
                    entries.push((
                        format!("Upgrade {price}g"),
                        UiAction::UpgradeUnit(target, price),
                        affordable,
                    ));
                }
                if view.air_superiority {
                    entries.push((
                        "Air Superiority [S]".to_owned(),
                        UiAction::AirSuperiority,
                        true,
                    ));
                }
                if view.rebase {
                    entries.push(("Re-base [Ctrl-R]".to_owned(), UiAction::BeginRebase, true));
                }
                if view.paradrop {
                    entries.insert(0, ("Paradrop".to_owned(), UiAction::BeginParadrop, true));
                }
                if view.bombard {
                    entries.insert(0, ("Bombard".to_owned(), UiAction::BeginBombard, true));
                }
                for city in &view.cities {
                    if city.can_airlift {
                        entries.push((
                            format!("Airlift to {}", city.name),
                            UiAction::Airlift(city.id),
                            true,
                        ));
                    }
                    if city.can_join {
                        entries.push((
                            format!("Join {}", city.name),
                            UiAction::JoinCity(city.id),
                            true,
                        ));
                    }
                    if city.can_help_wonder {
                        entries.push((
                            format!("Help wonder: {}", city.name),
                            UiAction::HelpWonder(city.id),
                            true,
                        ));
                    }
                }
                if view.nuclear_weapon {
                    entries.push((
                        "Launch nuclear strike".to_owned(),
                        UiAction::BeginNuclearStrike,
                        view.nuclear_strike,
                    ));
                }
                if let Some(label) = view.transport_action {
                    entries.push((format!("{label} [L]"), UiAction::BoardOrDeboard, true));
                }
                if let Some(enabled) = view.found_city {
                    entries.push(("Found city [B]".to_owned(), UiAction::FoundCity, enabled));
                }
                if let Some((label, enabled)) = view.road_action {
                    entries.push((label, UiAction::Road, enabled));
                }
                if let Some((label, enabled)) = view.irrigation_action {
                    entries.push((label, UiAction::Irrigate, enabled));
                }
                if let Some((label, enabled)) = view.mine_action {
                    entries.push((label, UiAction::Mine, enabled));
                }
                for kind in view.terrain_work {
                    let label = match kind {
                        WorkKind::Cultivate => "Cultivate [V]",
                        WorkKind::Plant => "Plant [P]",
                        _ => continue,
                    };
                    entries.push((
                        label.to_owned(),
                        UiAction::TerrainWork(kind),
                        view.work_left == 0,
                    ));
                }
                for (target, label) in view.clean_targets {
                    entries.insert(
                        0,
                        (
                            format!("Clean {label}"),
                            UiAction::TerrainWork(WorkKind::Clean(target)),
                            view.work_left == 0,
                        ),
                    );
                }
                for (base, name) in view.base_work {
                    entries.push((
                        format!("Build {name}"),
                        UiAction::TerrainWork(WorkKind::BuildBase(base)),
                        view.work_left == 0,
                    ));
                }
                for (target, name) in view.pillage_targets {
                    entries.push((
                        format!("Pillage {name}"),
                        UiAction::TerrainWork(WorkKind::Pillage(target)),
                        view.work_left == 0,
                    ));
                }
                if view.fortify {
                    entries.push(("Fortify [F]".to_owned(), UiAction::Fortify, true));
                }
                if view.set_home_city {
                    entries.push(("Set home city [H]".to_owned(), UiAction::HomeCity, true));
                }
                if view.automate {
                    entries.push(("Automate [A / X]".to_owned(), UiAction::Automate, true));
                }
                entries.push(("Wait [Space]".to_owned(), UiAction::Wait, true));
                for (city_id, name) in view.disband_cities {
                    entries.push((
                        format!("Recycle: {name}"),
                        UiAction::Disband(Some(city_id)),
                        true,
                    ));
                }
                if view.can_disband {
                    entries.push(("Disband".to_owned(), UiAction::Disband(None), true));
                }
            }
        } else if let Some(city_id) = self.selected_city
            && let Some(city) = self.sim.city_manager_view(city_id)
        {
            for kind in [
                UnitKind::Worker,
                UnitKind::Warrior,
                UnitKind::Spearman,
                UnitKind::Archer,
                UnitKind::Horseman,
                UnitKind::Catapult,
                UnitKind::Chariot,
                UnitKind::Swordsman,
                UnitKind::Settler,
            ] {
                if let Some(unit) = city.units.iter().find(|unit| unit.kind == kind) {
                    entries.push((
                        format!("{} {}", short_unit_name(kind), unit.cost),
                        UiAction::Produce(Production::Unit(kind)),
                        unit.can_change,
                    ));
                }
            }
        }

        if entries.is_empty() {
            return;
        }
        let width = 34.0_f32.min(available_width.max(1.0));
        let gap = 5.0;
        let step = width + gap;
        let capacity = (((available_width + gap) / step).floor() as usize).max(2);
        let paginated = entries.len() > capacity;
        let page_size = capacity - usize::from(paginated);
        let pages = entries.len().div_ceil(page_size);
        let page = self.action_page.min(pages - 1);
        let mut visible: Vec<_> = entries
            .into_iter()
            .skip(page * page_size)
            .take(page_size)
            .collect();
        if paginated {
            visible.push((
                format!("More actions ({}/{})", page + 1, pages),
                UiAction::ActionPage((page + 1) % pages),
                true,
            ));
        }
        let row_width = visible.len() as f32 * step - gap;
        let x = (screen_width() - row_width) * 0.5;
        let y = screen_height() - HUD_MARGIN - 34.0;
        let mut tooltip = None;
        for (column, (label, action, enabled)) in visible.into_iter().enumerate() {
            let rect = Rect::new(x + column as f32 * step, y, width, 34.0);
            draw_ui_button(rect, "", enabled, false);
            let color = if enabled {
                Color::from_rgba(241, 238, 218, 255)
            } else {
                GRAY
            };
            draw_hud_icon(unit_action_icon(action), rect.center(), 21.0, color);
            if rect.contains(ui_pointer()) {
                tooltip = Some((rect, label));
            }
            actions.push((
                rect,
                if enabled {
                    action
                } else {
                    UiAction::BlockMapClick
                },
            ));
        }
        if let Some((button, label)) = tooltip {
            let label = fit_text(
                &label,
                15.0,
                (screen_width() - 2.0 * HUD_MARGIN - 16.0).max(0.0),
            );
            let width = measure_text(&label, None, 15, 1.0).width + 16.0;
            let x = (button.center().x - width * 0.5)
                .max(HUD_MARGIN)
                .min(screen_width() - HUD_MARGIN - width);
            let rect = Rect::new(x, button.y - 30.0, width, 24.0);
            draw_hud_card(rect);
            draw_text(&label, rect.x + 8.0, rect.y + 17.0, 15.0, WHITE);
        }
    }
}

pub(super) fn unit_action_icon(action: UiAction) -> HudIcon {
    match action {
        UiAction::FoundCity | UiAction::JoinCity(_) | UiAction::Produce(_) => HudIcon::City,
        UiAction::Road => HudIcon::Road,
        UiAction::Irrigate => HudIcon::Water,
        UiAction::Mine => HudIcon::Pickaxe,
        UiAction::TerrainWork(WorkKind::Cultivate) => HudIcon::Wheat,
        UiAction::TerrainWork(WorkKind::Plant) => HudIcon::Tree,
        UiAction::Disband(Some(_)) => HudIcon::Cycle,
        UiAction::TerrainWork(WorkKind::Clean(_)) => HudIcon::Broom,
        UiAction::TerrainWork(WorkKind::BuildBase(_)) | UiAction::Fortify => HudIcon::Shield,
        UiAction::TerrainWork(WorkKind::Pillage(_)) | UiAction::BeginNuclearStrike => {
            HudIcon::Burst
        }
        UiAction::HomeCity => HudIcon::Home,
        UiAction::Automate => HudIcon::Gear,
        UiAction::Wait => HudIcon::Pause,
        UiAction::UpgradeUnit(..) => HudIcon::Upgrade,
        UiAction::BeginParadrop => HudIcon::Parachute,
        UiAction::BeginBombard => HudIcon::Target,
        UiAction::Airlift(_) => HudIcon::Plane,
        UiAction::BeginRebase => HudIcon::Plane,
        UiAction::BoardOrDeboard => HudIcon::Boat,
        UiAction::HelpWonder(_) => HudIcon::Wonder,
        UiAction::Disband(None) => HudIcon::Cross,
        _ => HudIcon::More,
    }
}
