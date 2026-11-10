//! Dispatch of UI actions into simulation commands and client state changes.

use super::*;

impl Game {
    pub(crate) fn perform_action(&mut self, action: UiAction) {
        if self.pending_city_view.is_some() {
            return;
        }
        if self.sim.game_over().is_some() && !matches!(action, UiAction::EndTurn) {
            return;
        }
        match action {
            UiAction::BlockMapClick => {}
            UiAction::DismissIntro => self.intro_open = false,
            UiAction::RaiseRate(index) => {
                self.sim.apply(Command::RaiseRate(index));
            }
            UiAction::CityTiles => self.city_tiles = !self.city_tiles,
            UiAction::WorkCityTile(city, x, y) => {
                self.sim.apply(Command::WorkCityTile(city, x, y));
            }
            UiAction::Specialist(city, index, delta) => {
                self.sim.apply(Command::Specialist(city, index, delta));
            }
            UiAction::Disband(city) => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::Disband(id, city))
                {
                    self.select_next_unit();
                }
            }
            UiAction::UpgradeUnit(target, price) => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::UpgradeUnit(id, target, price));
                }
            }
            UiAction::HelpWonder(city) => {
                if let Some(id) = self.selected_unit {
                    if self.sim.apply(Command::HelpWonder(id, city)) {
                        self.select_next_unit();
                    }
                }
            }
            UiAction::BeginParadrop => {
                self.paradrop_mode = self
                    .selected_unit_index()
                    .is_some_and(|index| self.sim.can_begin_paradrop(&self.sim.units()[index]));
                self.goto_mode = false;
                self.bombard_mode = false;
                self.nuclear_mode = false;
            }
            UiAction::ParadropTo(x, y) => {
                if self.paradrop_mode
                    && let Some(id) = self.selected_unit
                {
                    self.move_selected(Command::Paradrop(id, x, y), id);
                }
                self.paradrop_mode = false;
            }
            UiAction::BeginBombard => {
                self.goto_mode = false;
                self.paradrop_mode = false;
                self.nuclear_mode = false;
                self.bombard_mode = self
                    .selected_unit_index()
                    .is_some_and(|index| self.sim.can_begin_bombard(&self.sim.units()[index]));
            }
            UiAction::BombardTo(x, y) => {
                if self.bombard_mode
                    && let Some(id) = self.selected_unit
                {
                    self.move_selected(Command::Bombard(id, x, y), id);
                }
                self.bombard_mode = false;
            }
            UiAction::NuclearStrikeTo(x, y) => {
                if self.nuclear_mode
                    && let Some(id) = self.selected_unit
                    && self.sim.apply(Command::NuclearStrike(id, x, y))
                {
                    self.select_next_unit();
                }
                self.nuclear_mode = false;
            }
            UiAction::Airlift(city) => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::Airlift(id, city))
                    && let Some(unit) = self.sim.unit_by_id(id)
                {
                    self.focus_camera_on(unit.x, unit.y);
                }
            }
            UiAction::AirSuperiority => {
                if let Some(id) = self.selected_unit {
                    self.move_selected(Command::AirSuperiority(id), id);
                }
            }
            UiAction::BeginRebase => {
                self.goto_mode = false;
                self.paradrop_mode = false;
                self.bombard_mode = false;
                self.nuclear_mode = false;
                self.rebase_mode = self.selected_unit_index().is_some_and(|index| {
                    self.sim.can_begin_rebase(&self.sim.units()[index])
                });
            }
            UiAction::RebaseTo(x, y) => {
                if self.rebase_mode && let Some(id) = self.selected_unit {
                    self.move_selected(Command::Rebase(id, x, y), id);
                }
                self.rebase_mode = false;
            }
            UiAction::JoinCity(city) => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::JoinCity(id, city))
                {
                    self.select_next_unit();
                }
            }
            UiAction::BeginNuclearStrike => {
                self.goto_mode = false;
                self.paradrop_mode = false;
                self.bombard_mode = false;
                self.nuclear_mode = self
                    .selected_unit_index()
                    .is_some_and(|index| self.sim.can_begin_nuclear_strike(&self.sim.units()[index]));
            }
            UiAction::BoardOrDeboard => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::BoardOrDeboard(id));
                }
            }
            UiAction::HomeCity => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::HomeCity(id));
                }
            }
            UiAction::LaunchSpaceship => {
                if self.sim.apply(Command::LaunchSpaceship) {
                    self.spaceship_open = false;
                }
            }
            UiAction::ToggleSpaceship => self.spaceship_open = !self.spaceship_open,
            UiAction::ToggleManualAssembly => {
                self.sim.apply(Command::ToggleManualAssembly);
            }
            UiAction::PlaceShipPart(part) => {
                self.sim.apply(Command::PlaceShipPart(part));
            }
            UiAction::EndTurn => {
                if self.sim.apply(Command::EndTurn)
                    && self.autosave_enabled
                    && let Err(error) = self
                        .sim
                        .save_campaign(std::path::Path::new(crate::AUTOSAVE_PATH))
                {
                    self.sim.add_event(format!("Autosave failed. {error}"));
                }
                self.select_next_unit();
            }
            UiAction::CloseCity => {
                self.selected_city = None;
            }
            UiAction::SelectUnit(id) => {
                if self
                    .sim
                    .units()
                    .iter()
                    .any(|unit| unit.id == id && unit.owner == HUMAN)
                {
                    self.action_page = 0;
                    self.selected_unit = Some(id);
                    self.selected_city = None;
                }
            }
            UiAction::FoundCity => {
                if let Some(id) = self.selected_unit {
                    self.found_city(id);
                }
            }
            UiAction::Road => {
                if let Some(index) = self.selected_unit_index() {
                    let unit = &self.sim.units()[index];
                    let id = unit.id;
                    let kind = if self.sim.tile(unit.x, unit.y).road {
                        WorkKind::Railroad
                    } else {
                        WorkKind::Road
                    };
                    self.sim.apply(Command::Work(id, kind));
                }
            }
            UiAction::Irrigate => {
                if let Some(index) = self.selected_unit_index() {
                    let unit = &self.sim.units()[index];
                    let kind = if self.sim.tile(unit.x, unit.y).irrigation {
                        WorkKind::Farmland
                    } else {
                        WorkKind::Irrigation
                    };
                    self.sim.apply(Command::Work(unit.id, kind));
                }
            }
            UiAction::TerrainWork(kind) => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::Work(id, kind));
                }
            }
            UiAction::Mine => {
                if let Some(index) = self.selected_unit_index() {
                    let unit = &self.sim.units()[index];
                    let kind = self.sim.oil_work(unit).unwrap_or(WorkKind::Mine);
                    self.sim.apply(Command::Work(unit.id, kind));
                }
            }
            UiAction::Fortify => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::Fortify(id));
                }
            }
            UiAction::Wait => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::Wait(id));
                    self.select_next_unit();
                }
            }
            UiAction::Automate => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::Automate(id));
                }
            }
            UiAction::RushProduction(id) => {
                self.sim.apply(Command::RushProduction(id));
            }
            UiAction::SellBuilding(id, building) => {
                self.sim.apply(Command::SellBuilding(id, building));
            }
            UiAction::Produce(kind) => {
                if let Some(id) = self.selected_city {
                    self.sim.apply(Command::Produce(id, kind));
                }
            }
            UiAction::ActionPage(page) => self.action_page = page,
            UiAction::ProductionPage(page) => self.production_page = page,
            UiAction::UnitProductionPage(page) => self.unit_production_page = page,
            UiAction::WorklistPage(page) => self.worklist_page = page,
            UiAction::EditWorklist(id, edit) => {
                self.sim.apply(Command::EditWorklist(id, edit));
            }
            UiAction::ToggleDiplomacy => {
                self.diplomacy_open = !self.diplomacy_open;
                self.diplomacy_target = None;
                self.treaty_draft = None;
                self.treaty_feedback = None;
                self.diplomacy_page = 0;
                self.city_deals = None;
                self.reviewed_offer = None;
            }
            UiAction::OpenDiplomacyWith(owner) => {
                self.diplomacy_open = true;
                self.diplomacy_target = Some(owner);
                self.treaty_draft = None;
                self.treaty_feedback = None;
                self.treaty_item_page = [0; MAX_CIVILIZATIONS];
                self.treaty_item_group = [0; MAX_CIVILIZATIONS];
                self.diplomacy_page = 0;
                self.city_deals = None;
                self.reviewed_offer = None;
            }
            UiAction::OpenDiplomacyOverview => {
                self.diplomacy_open = true;
                self.diplomacy_target = None;
                self.treaty_draft = None;
                self.treaty_feedback = None;
                self.diplomacy_page = 0;
                self.city_deals = None;
                self.reviewed_offer = None;
            }
            UiAction::DiplomacyPage(page) => self.diplomacy_page = page,
            UiAction::EditTreatyDraft(owner, offer) => {
                self.treaty_draft = Some((owner, offer));
                self.treaty_feedback = None;
            }
            UiAction::TreatyItemsPage(owner, page) => {
                if owner < self.treaty_item_page.len() {
                    self.treaty_item_page[owner] = page;
                }
            }
            UiAction::TreatyItemGroup(side, group) => {
                if side < self.treaty_item_group.len() {
                    self.treaty_item_group[side] = group;
                    self.treaty_item_page[side] = 0;
                }
            }
            UiAction::AdjustTreatyGold(owner, delta) => {
                if owner < self.sim.players().len() {
                    let treasury = self.sim.players()[owner].gold.max(0);
                    self.treaty_gold_amount[owner] = self.treaty_gold_amount[owner]
                        .saturating_add(delta)
                        .clamp(0, treasury);
                    self.treaty_feedback = None;
                }
            }
            UiAction::AddTreatyGold(target, side, mut offer) => {
                let term_side = usize::from(side != HUMAN);
                if side < self.sim.players().len() && term_side < offer.terms.len() {
                    let amount =
                        self.treaty_gold_amount[side].min(self.sim.players()[side].gold.max(0));
                    offer.terms[term_side] = offer.terms[term_side].set_gold(amount);
                    self.treaty_draft = Some((target, offer));
                    self.treaty_feedback = None;
                }
            }
            UiAction::ProposeTreaty(owner, offer) => {
                let accepted = self.sim.apply(Command::ProposeTreaty(owner, offer));
                let message =
                    self.sim.events().last().cloned().unwrap_or_else(|| {
                        "The treaty proposal could not be delivered.".to_owned()
                    });
                self.treaty_feedback = Some((owner, message));
                if accepted {
                    self.treaty_draft = Some((owner, offer));
                }
            }
            UiAction::CityDeals(selection) => self.city_deals = selection,
            UiAction::BuyCity(city, price) => {
                self.sim.apply(Command::BuyCity(city, price));
            }
            UiAction::BuyMap(owner, map) => {
                self.sim.apply(Command::BuyMap(owner, map));
            }
            UiAction::CancelPact(other) => {
                self.sim.apply(Command::CancelPact(other));
            }
            UiAction::ReviewOffer(other, offer) => {
                self.diplomacy_open = true;
                self.diplomacy_target = Some(other);
                self.treaty_draft = Some((other, offer));
                self.treaty_feedback = None;
                self.reviewed_offer = None;
            }
            UiAction::CloseOffer => self.reviewed_offer = None,
            UiAction::AcceptOffer(other, offer) => {
                let accepted = self.sim.apply(Command::AcceptOffer(other, offer));
                self.reviewed_offer = None;
                self.treaty_draft = None;
                self.treaty_feedback = accepted.then(|| {
                    (
                        other,
                        self.sim
                            .events()
                            .last()
                            .cloned()
                            .unwrap_or_else(|| "Treaty terms agreed.".to_owned()),
                    )
                });
            }
            UiAction::DeclineOffer(other) => {
                self.reviewed_offer = None;
                self.sim.apply(Command::DeclineOffer(other));
            }
            UiAction::ToggleGovernment => self.government_open = !self.government_open,
            UiAction::ChangeGovernment(target) => {
                self.sim.apply(Command::Government(target));
            }
            UiAction::OpenResearch => {
                self.research_open = true;
                self.research_drag = None;
                self.dragging = false;
                self.center_research();
            }
            UiAction::CloseResearch => self.research_open = false,
            UiAction::CenterResearch => self.center_research(),
            UiAction::FitResearch => self.fit_research(),
            UiAction::ResearchAge(age) => {
                self.research_age = age;
                self.research_inspected = None;
                self.fit_research();
            }
            UiAction::InspectResearch(tech) => {
                self.research_inspected = Some(tech);
                self.focus_research(tech);
            }
            UiAction::ResearchGoal(goal) => {
                self.research_inspected = Some(goal);
                self.sim.apply(Command::Research(goal));
            }
        }
    }
}
