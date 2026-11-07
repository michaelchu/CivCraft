use super::hud::{HudIcon, draw_hud_card, draw_hud_icon};
use super::*;

impl Game {
    pub(super) fn draw_action_strip(
        &self,
        actions: &mut Vec<(Rect, UiAction)>,
        available_width: f32,
    ) {
        let mut entries: Vec<(String, UiAction, bool)> = Vec::new();
        if let Some(index) = self.selected_unit_index() {
            let unit = &self.sim.units()[index];
            let stats = unit.kind.stats();
            if let Some((target, price)) = self.sim.paid_upgrade(unit) {
                entries.push((
                    format!("Upgrade {price}g"),
                    UiAction::UpgradeUnit(target, price),
                    self.sim.players()[unit.owner].gold >= price,
                ));
            }
            for city in self
                .sim
                .cities()
                .iter()
                .filter(|city| self.sim.can_establish_embassy(unit, city))
            {
                entries.push((
                    format!("Embassy: {}", city.name),
                    UiAction::EstablishEmbassy(city.id),
                    true,
                ));
            }
            for city in self
                .sim
                .cities()
                .iter()
                .filter(|city| self.sim.can_investigate_city(unit, city))
            {
                entries.push((
                    format!("Investigate {}", city.name),
                    UiAction::InvestigateCity(city.id),
                    true,
                ));
            }
            if unit.kind == UnitKind::Spy {
                for city in self.sim.cities().iter().filter(|city| {
                    self.sim.can_steal_technology(unit, city)
                        || self.sim.can_sabotage_city(unit, city)
                }) {
                    entries.push((
                        format!("Targets: {}", city.name),
                        UiAction::SpyTargets(Some((city.id, 0))),
                        true,
                    ));
                }
            }
            if unit.kind.is_diplomatic() {
                for city in self
                    .sim
                    .cities()
                    .iter()
                    .filter(|city| self.sim.can_steal_technology(unit, city))
                {
                    entries.push((
                        format!("Steal tech: {}", city.name),
                        UiAction::StealTechnology(city.id),
                        true,
                    ));
                }
                for city in self
                    .sim
                    .cities()
                    .iter()
                    .filter(|city| self.sim.can_sabotage_city(unit, city))
                {
                    entries.push((
                        format!("Sabotage {}", city.name),
                        UiAction::SabotageCity(city.id),
                        true,
                    ));
                }
                for city in self.sim.cities() {
                    if let Some(price) = self.sim.incite_price(unit, city) {
                        entries.push((
                            format!("Incite {} {price}g", city.name),
                            UiAction::InciteCity(city.id, price),
                            self.sim.players()[unit.owner].gold >= price,
                        ));
                    }
                }
                for target in self.sim.units() {
                    if !self.sim.unit_visible_to_human(target) {
                        continue;
                    }
                    if self.sim.can_sabotage_unit(unit, target) {
                        entries.push((
                            format!("Sabotage {}", target.kind.name()),
                            UiAction::SabotageUnit(target.id),
                            true,
                        ));
                    }
                    if let Some(price) = self.sim.bribe_price(unit, target) {
                        entries.push((
                            format!("Bribe {} {price}g", target.kind.name()),
                            UiAction::BribeUnit(target.id, price),
                            self.sim.players()[unit.owner].gold >= price,
                        ));
                    }
                }
            }
            if self.sim.can_begin_paradrop(unit) {
                entries.insert(0, ("Paradrop".to_owned(), UiAction::BeginParadrop, true));
            }
            for city in self.sim.cities() {
                if self.sim.can_airlift(unit, city) {
                    entries.push((
                        format!("Airlift to {}", city.name),
                        UiAction::Airlift(city.id),
                        true,
                    ));
                }
                if self.sim.can_join_city(unit, city) {
                    entries.push((
                        format!("Join {}", city.name),
                        UiAction::JoinCity(city.id),
                        true,
                    ));
                }
                if self.sim.can_deliver_goods(unit, city) && self.sim.tile(city.x, city.y).visible {
                    entries.push((
                        format!("Trade: {}", city.name),
                        UiAction::DeliverGoods(city.id),
                        true,
                    ));
                }
                if self.sim.can_help_wonder(unit, city) && self.sim.tile(city.x, city.y).visible {
                    entries.push((
                        format!("Help wonder: {}", city.name),
                        UiAction::HelpWonder(city.id),
                        true,
                    ));
                }
            }
            if unit.kind == UnitKind::Nuclear {
                entries.push(("Detonate here".to_owned(), UiAction::Detonate, true));
            }
            if let Some(label) = self.sim.transport_action(unit) {
                entries.push((format!("{label} [L]"), UiAction::BoardOrDeboard, true));
            }
            if stats.can_found && unit.transport.is_none() {
                entries.push((
                    "Found city [B]".to_owned(),
                    UiAction::FoundCity,
                    self.sim.can_found_at(unit.x, unit.y),
                ));
            }
            if stats.can_work && unit.transport.is_none() {
                let tile = self.sim.tile(unit.x, unit.y);
                let (label, enabled) = if tile.road && self.sim.has_tech(unit.owner, Tech::RAILROAD)
                {
                    (
                        "Build railroad [R]",
                        self.sim.can_railroad_at(unit.owner, unit.x, unit.y),
                    )
                } else {
                    (
                        "Build road [R]",
                        !tile.road
                            && tile.terrain.road_time() > 0
                            && (!tile.river
                                || self.sim.has_tech(unit.owner, Tech::BRIDGE_BUILDING)),
                    )
                };
                entries.push((label.to_owned(), UiAction::Road, enabled));
            }
            if stats.can_work && unit.transport.is_none() && !unit.kind.is_naval() {
                let tile = self.sim.tile(unit.x, unit.y);
                let (label, enabled) = if tile.irrigation {
                    (
                        "Build farmland [I]",
                        self.sim.can_farm_at(unit.owner, unit.x, unit.y),
                    )
                } else {
                    (
                        "Irrigate [I]",
                        self.sim.can_irrigate_at(unit.owner, unit.x, unit.y),
                    )
                };
                entries.push((label.to_owned(), UiAction::Irrigate, enabled));
            }
            if let Some(kind) = self.sim.oil_work(unit) {
                entries.push((
                    format!("Build {} [M]", kind.name()),
                    UiAction::Mine,
                    unit.work_left == 0,
                ));
            } else if stats.can_work && unit.transport.is_none() && !unit.kind.is_naval() {
                let tile = self.sim.tile(unit.x, unit.y);
                entries.push((
                    "Mine [M]".to_owned(),
                    UiAction::Mine,
                    !tile.mine && tile.terrain.mine_time() > 0,
                ));
            }
            for (kind, label) in [
                (WorkKind::Cultivate, "Cultivate [V]"),
                (WorkKind::Plant, "Plant [P]"),
                (WorkKind::Transform, "Transform [T]"),
            ] {
                if self.sim.terrain_work(unit, kind).is_some() {
                    entries.push((
                        label.to_owned(),
                        UiAction::TerrainWork(kind),
                        unit.work_left == 0,
                    ));
                }
            }
            if unit.kind.can_clean() && unit.transport.is_none() {
                let tile = self.sim.tile(unit.x, unit.y);
                for target in [Contamination::Pollution, Contamination::Fallout] {
                    if tile.has_contamination(target) {
                        entries.insert(
                            0,
                            (
                                format!("Clean {}", target.name()),
                                UiAction::TerrainWork(WorkKind::Clean(target)),
                                unit.work_left == 0,
                            ),
                        );
                    }
                }
            }
            for base in [Base::Fort, Base::Fortress, Base::Airstrip, Base::Airbase] {
                if self.sim.base_work_time(unit, base).is_some() {
                    entries.push((
                        format!("Build {}", base.name()),
                        UiAction::TerrainWork(WorkKind::BuildBase(base)),
                        unit.work_left == 0,
                    ));
                }
            }
            let base = self.sim.tile(unit.x, unit.y).base.map(PillageTarget::Base);
            for target in PillageTarget::ALL.into_iter().chain(base) {
                if self.sim.can_pillage(unit, target) {
                    entries.push((
                        format!("Pillage {}", target.name()),
                        UiAction::TerrainWork(WorkKind::Pillage(target)),
                        unit.work_left == 0,
                    ));
                }
            }
            if unit.kind.is_military() && unit.transport.is_none() {
                entries.push(("Fortify [F]".to_owned(), UiAction::Fortify, true));
            }
            if self.sim.home_city_target(unit).is_some() {
                entries.push(("Set home city [H]".to_owned(), UiAction::HomeCity, true));
            }
            if unit.automated
                || (unit.kind.fuel_capacity() == 0 && unit.kind != UnitKind::Helicopter)
            {
                entries.push(("Automate [A / X]".to_owned(), UiAction::Automate, true));
            }
            entries.push(("Wait [Space]".to_owned(), UiAction::Wait, true));
            for city in self
                .sim
                .cities()
                .iter()
                .filter(|city| self.sim.can_disband(unit, Some(city)))
            {
                entries.push((
                    format!("Recycle: {}", city.name),
                    UiAction::Disband(Some(city.id)),
                    true,
                ));
            }
            if self.sim.can_disband(unit, None) {
                entries.push(("Disband".to_owned(), UiAction::Disband(None), true));
            }
        } else if let Some(index) = self.selected_city_index() {
            let city = &self.sim.cities()[index];
            for kind in [
                if self.sim.has_tech(city.owner, Tech::EXPLOSIVES) {
                    UnitKind::Engineer
                } else {
                    UnitKind::Worker
                },
                UnitKind::Warrior,
                UnitKind::Phalanx,
                UnitKind::Archer,
                UnitKind::Horseman,
                UnitKind::Catapult,
                UnitKind::Chariot,
                UnitKind::Legion,
                UnitKind::Settlers,
            ] {
                entries.push((
                    format!("{} {}", short_unit_name(kind), kind.stats().cost),
                    UiAction::Produce(Production::Unit(kind)),
                    self.sim.can_change_production(city, Production::Unit(kind)),
                ));
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
        UiAction::TerrainWork(WorkKind::Transform) | UiAction::Disband(Some(_)) => HudIcon::Cycle,
        UiAction::TerrainWork(WorkKind::Clean(_)) => HudIcon::Broom,
        UiAction::TerrainWork(WorkKind::BuildBase(_)) | UiAction::Fortify => HudIcon::Shield,
        UiAction::TerrainWork(WorkKind::Pillage(_))
        | UiAction::Detonate
        | UiAction::SabotageCity(_)
        | UiAction::SabotageUnit(_) => HudIcon::Burst,
        UiAction::HomeCity => HudIcon::Home,
        UiAction::Automate => HudIcon::Gear,
        UiAction::Wait => HudIcon::Pause,
        UiAction::UpgradeUnit(..) => HudIcon::Upgrade,
        UiAction::EstablishEmbassy(_) => HudIcon::Government,
        UiAction::InvestigateCity(_) => HudIcon::Search,
        UiAction::SpyTargets(_) => HudIcon::Target,
        UiAction::StealTechnology(_) => HudIcon::Research,
        UiAction::BribeUnit(..) | UiAction::InciteCity(..) => HudIcon::Tax,
        UiAction::BeginParadrop => HudIcon::Parachute,
        UiAction::Airlift(_) => HudIcon::Plane,
        UiAction::BoardOrDeboard => HudIcon::Boat,
        UiAction::DeliverGoods(_) => HudIcon::Crate,
        UiAction::HelpWonder(_) => HudIcon::Wonder,
        UiAction::Disband(None) => HudIcon::Cross,
        _ => HudIcon::More,
    }
}
