//! Dispatch of UI actions into simulation commands and client state changes.

use super::*;

impl Game {
    /// Enters `mode` when the selected unit offers it, replacing any other
    /// targeting mode; an unavailable action leaves plain tile selection.
    fn begin_targeting(
        &mut self,
        mode: TargetingMode,
        available: impl FnOnce(&crate::sim::UnitActionView) -> bool,
    ) {
        let available = self
            .selected_unit
            .and_then(|id| self.sim.unit_action_view(id))
            .is_some_and(|view| available(&view));
        self.targeting = mode.when(available);
    }

    /// Leaves `mode` once its target was chosen; another active mode stays.
    fn end_targeting(&mut self, mode: TargetingMode) {
        if self.targeting == mode {
            self.targeting = TargetingMode::None;
        }
    }

    pub(crate) fn perform_action(&mut self, action: UiAction) {
        if (self.turn_busy()
            && !matches!(
                action,
                UiAction::ToggleMusicPanel
                    | UiAction::ToggleMusicMute
                    | UiAction::SetMusicVolume(_)
                    | UiAction::ToggleSfxMute
                    | UiAction::SetSfxVolume(_)
            ))
            || self.pending_city_view.is_some()
        {
            return;
        }
        if self.sim.game_over_view().is_some()
            && !matches!(
                action,
                UiAction::EndTurn
                    | UiAction::ToggleMusicPanel
                    | UiAction::ToggleMusicMute
                    | UiAction::SetMusicVolume(_)
                    | UiAction::ToggleSfxMute
                    | UiAction::SetSfxVolume(_)
            )
        {
            return;
        }
        match action {
            UiAction::BlockMapClick => {}
            UiAction::DismissIntro => self.overlay.close(Overlay::Intro),
            UiAction::ToggleMusicPanel => self.music_open = !self.music_open,
            UiAction::ToggleMusicMute => self.music_muted = !self.music_muted,
            UiAction::SetMusicVolume(volume) => self.music_volume = volume.min(100),
            UiAction::ToggleSfxMute => self.sfx_muted = !self.sfx_muted,
            UiAction::SetSfxVolume(volume) => self.sfx_volume = volume.min(100),
            UiAction::SetRates(rates) => {
                self.sim.apply(Command::SetRates(rates));
            }
            UiAction::CityGovernorPage => {
                self.city_governor_page = !self.city_governor_page;
                self.city_governor_preview = self
                    .selected_city
                    .filter(|_| self.city_governor_page)
                    .and_then(|id| self.sim.city_governor_preview(id));
            }
            UiAction::SetCityGovernor(city_id, state) => {
                self.sim.apply(Command::SetCityGovernor { city_id, state });
                self.city_governor_preview = self.sim.city_governor_preview(city_id);
            }
            UiAction::WorkCityTile(city, x, y) => {
                self.sim.apply(Command::WorkCityTile {
                    city_id: city,
                    x,
                    y,
                });
            }
            UiAction::Specialist(city, index, delta) => {
                self.sim.apply(Command::Specialist {
                    city_id: city,
                    index,
                    delta,
                });
            }
            UiAction::Disband(city) => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::Disband {
                        unit_id: id,
                        city_id: city,
                    })
                {
                    self.select_next_unit();
                }
            }
            UiAction::UpgradeUnit(target, price) => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::UpgradeUnit {
                        unit_id: id,
                        target,
                        price,
                    });
                }
            }
            UiAction::FormArmy => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::FormArmy(id))
                {
                    self.select_next_unit();
                }
            }
            UiAction::CompleteProduction(city) => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::CompleteProduction {
                        unit_id: id,
                        city_id: city,
                    })
                {
                    self.select_next_unit();
                }
            }
            UiAction::BeginParadrop => {
                self.begin_targeting(TargetingMode::Paradrop, |view| view.paradrop);
            }
            UiAction::ParadropTo(x, y) => {
                if self.targeting == TargetingMode::Paradrop
                    && let Some(id) = self.selected_unit
                {
                    self.move_selected(Command::Paradrop(id, x, y), id);
                }
                self.end_targeting(TargetingMode::Paradrop);
            }
            UiAction::BeginCaptureUnits => {
                self.begin_targeting(TargetingMode::CaptureUnits, |view| view.capture_units);
            }
            UiAction::CaptureUnitsTo(x, y) => {
                if self.targeting == TargetingMode::CaptureUnits
                    && let Some(id) = self.selected_unit
                {
                    self.move_selected(Command::CaptureUnits(id, x, y), id);
                }
                self.end_targeting(TargetingMode::CaptureUnits);
            }
            UiAction::BeginBombard
            | UiAction::BeginAutoBombard
            | UiAction::BeginPrecisionBombard
            | UiAction::BeginAutoPrecisionBombard => {
                let precision = matches!(
                    action,
                    UiAction::BeginPrecisionBombard | UiAction::BeginAutoPrecisionBombard
                );
                let auto = matches!(
                    action,
                    UiAction::BeginAutoBombard | UiAction::BeginAutoPrecisionBombard
                );
                self.begin_targeting(TargetingMode::Bombard { precision, auto }, |view| {
                    if precision {
                        view.precision_bombard
                    } else {
                        view.bombard
                    }
                });
            }
            UiAction::BombardTo(x, y) => {
                if let TargetingMode::Bombard { precision, auto } = self.targeting {
                    if let Some(id) = self.selected_unit {
                        let command = match (precision, auto) {
                            (true, true) => Command::AutoPrecisionBombard(id, x, y),
                            (true, false) => Command::PrecisionBombard(id, x, y),
                            (false, true) => Command::AutoBombard(id, x, y),
                            (false, false) => Command::Bombard(id, x, y),
                        };
                        self.move_selected(command, id);
                    }
                    self.targeting = TargetingMode::None;
                }
            }
            UiAction::CancelUnitOrder => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::CancelOrder(id));
                }
            }
            UiAction::AirSuperiority => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::AirSuperiority(id));
                }
            }
            UiAction::BeginRebase => {
                self.begin_targeting(TargetingMode::Rebase, |view| view.rebase);
            }
            UiAction::RebaseTo(x, y) => {
                if self.targeting == TargetingMode::Rebase
                    && let Some(id) = self.selected_unit
                    && self.move_selected(Command::Rebase(id, x, y), id)
                {
                    self.focus_camera_on(x, y);
                }
                self.end_targeting(TargetingMode::Rebase);
            }
            UiAction::BeginRecon => {
                self.begin_targeting(TargetingMode::Recon, |view| view.recon);
            }
            UiAction::ReconTo(x, y) => {
                if self.targeting == TargetingMode::Recon
                    && let Some(id) = self.selected_unit
                {
                    self.move_selected(Command::Recon(id, x, y), id);
                }
                self.end_targeting(TargetingMode::Recon);
            }
            UiAction::NuclearStrikeTo(x, y) => {
                if self.targeting == TargetingMode::NuclearStrike
                    && let Some(id) = self.selected_unit
                    && self.sim.apply(Command::NuclearStrike(id, x, y))
                {
                    self.select_next_unit();
                }
                self.end_targeting(TargetingMode::NuclearStrike);
            }
            UiAction::Airlift(city) => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::Airlift(id, city))
                    && let Some(unit) = self.sim.unit_action_view(id)
                {
                    self.focus_camera_on(unit.x, unit.y);
                }
            }
            UiAction::JoinCity(city) => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::JoinCity {
                        unit_id: id,
                        city_id: city,
                    })
                {
                    self.select_next_unit();
                }
            }
            UiAction::BeginNuclearStrike => {
                self.begin_targeting(TargetingMode::NuclearStrike, |view| view.nuclear_strike);
            }
            UiAction::BeginConnect => {
                self.begin_targeting(TargetingMode::Connect, |view| view.can_connect);
            }
            UiAction::ConnectTo(x, y) => {
                if self.targeting == TargetingMode::Connect
                    && let Some(id) = self.selected_unit
                    && self.move_selected(Command::Connect(id, x, y), id)
                {
                    self.select_next_unit();
                }
                self.end_targeting(TargetingMode::Connect);
            }
            UiAction::BeginPatrol => {
                self.begin_targeting(TargetingMode::Patrol, |view| view.can_patrol);
            }
            UiAction::PatrolTo(x, y) => {
                if self.targeting == TargetingMode::Patrol
                    && let Some(id) = self.selected_unit
                    && self.move_selected(Command::Patrol(id, x, y), id)
                {
                    self.select_next_unit();
                }
                self.end_targeting(TargetingMode::Patrol);
            }
            UiAction::BoardOrDeboard => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::BoardOrDeboard(id));
                }
            }
            UiAction::UnloadAll => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::UnloadAll(id))
                {
                    self.select_next_unit();
                }
            }
            UiAction::HomeCity => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::HomeCity(id));
                }
            }
            UiAction::ToggleSpaceship => self.overlay.toggle(Overlay::Spaceship),
            UiAction::EndTurn => self.start_background_turn(),
            UiAction::CloseCity => {
                self.selected_city = None;
            }
            UiAction::SelectUnit(id) => {
                if self
                    .sim
                    .unit_action_view(id)
                    .is_some_and(|unit| unit.owner == HUMAN)
                {
                    self.select_unit_group(vec![id], Some(id));
                }
            }
            UiAction::ShowCityInfo(city_id) => {
                self.show_city_info(city_id);
            }
            UiAction::SelectUnitStack(ids) => {
                self.select_unit_group(ids, None);
            }
            UiAction::SelectSameUnitType(ids) => {
                let active = self.selected_unit.filter(|id| ids.contains(id));
                self.select_unit_group(ids, active);
            }
            UiAction::FoundCity => {
                if let Some(id) = self.selected_unit {
                    self.found_city(id);
                }
            }
            UiAction::Road => {
                if let Some(id) = self.selected_unit
                    && let Some(view) = self.sim.unit_action_view(id)
                {
                    self.sim.apply(Command::Work(id, view.road_work));
                }
            }
            UiAction::Irrigate => {
                if let Some(id) = self.selected_unit
                    && let Some(view) = self.sim.unit_action_view(id)
                {
                    self.sim.apply(Command::Work(id, view.irrigation_work));
                }
            }
            UiAction::TerrainWork(kind) => {
                if let Some(id) = self.selected_unit {
                    self.sim.apply(Command::Work(id, kind));
                }
            }
            UiAction::Mine => {
                if let Some(id) = self.selected_unit
                    && let Some(view) = self.sim.unit_action_view(id)
                {
                    self.sim.apply(Command::Work(id, view.mine_work));
                }
            }
            UiAction::Fortify => {
                if let Some(id) = self.selected_unit
                    && self.sim.apply(Command::Fortify(id))
                {
                    self.select_next_unit();
                }
            }
            UiAction::Wait => {
                self.wait_selected_unit();
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
            UiAction::CityBuildingsPage(page) => self.city_buildings_page = page,
            UiAction::ProductionPage(page) => self.production_page = page,
            UiAction::UnitProductionPage(page) => self.unit_production_page = page,
            UiAction::WorklistPage(page) => self.worklist_page = page,
            UiAction::EditWorklist(id, edit) => {
                self.sim.apply(Command::EditWorklist { city_id: id, edit });
            }
            UiAction::ToggleDiplomacy => {
                self.overlay.toggle(Overlay::Diplomacy);
                self.diplomacy_target = None;
                self.treaty_draft = None;
                self.treaty_feedback = None;
                self.diplomacy_page = 0;
                self.reviewed_offer = None;
            }
            UiAction::ToggleEspionage => {
                self.overlay.toggle(Overlay::Espionage);
                self.espionage_target = None;
                self.espionage_rival_page = 0;
                self.espionage_city = None;
                self.espionage_city_page = 0;
                self.espionage_military_page = 0;
            }
            UiAction::EspionageOpponent(owner) => {
                self.espionage_target = Some(owner);
                self.espionage_city = None;
                self.espionage_city_page = 0;
                self.espionage_military_page = 0;
            }
            UiAction::EspionageRivalPage(page) => {
                self.espionage_rival_page = page;
                self.espionage_target = None;
                self.espionage_city = None;
            }
            UiAction::EspionageCity(city) => self.espionage_city = Some(city),
            UiAction::EspionageCityPage(page) => {
                self.espionage_city_page = page;
                self.espionage_city = None;
            }
            UiAction::EspionageMilitaryPage(page) => self.espionage_military_page = page,
            UiAction::EspionageRisk(risk) => self.espionage_risk = risk,
            UiAction::PerformEspionage(owner, mission, city, risk) => {
                self.sim
                    .apply(Command::Espionage(owner, mission, city, risk));
            }
            UiAction::OpenDiplomacyWith(owner) => {
                self.overlay = Overlay::Diplomacy;
                self.diplomacy_target = Some(owner);
                self.treaty_draft = None;
                self.treaty_feedback = None;
                self.treaty_item_page = [0; MAX_CIVILIZATIONS];
                self.treaty_clause_page = [0; 2];
                self.treaty_item_group = [0; MAX_CIVILIZATIONS];
                self.diplomacy_page = 0;
                self.reviewed_offer = None;
            }
            UiAction::OpenDiplomacyOverview => {
                self.overlay = Overlay::Diplomacy;
                self.diplomacy_target = None;
                self.treaty_draft = None;
                self.treaty_feedback = None;
                self.diplomacy_page = 0;
                self.reviewed_offer = None;
            }
            UiAction::DiplomacyPage(page) => self.diplomacy_page = page,
            UiAction::EditTreatyDraft(owner, offer) => {
                let previous = self
                    .treaty_draft
                    .as_ref()
                    .filter(|(target, _)| *target == owner)
                    .map(|(_, draft)| draft.clone())
                    .or_else(|| {
                        self.sim
                            .diplomacy_meeting_view(owner, None)
                            .map(|view| view.offer)
                    });
                if let Some(previous) = previous {
                    for side in 0..offer.terms.len() {
                        for &city in offer.terms[side].cities() {
                            if !previous.terms[side].cities().contains(&city) {
                                self.sim.apply(Command::ShareTreatyCityMap(owner, city));
                            }
                        }
                    }
                }
                self.treaty_draft = Some((owner, offer));
                self.treaty_feedback = None;
            }
            UiAction::TreatyItemsPage(owner, page) => {
                if owner < self.treaty_item_page.len() {
                    self.treaty_item_page[owner] = page;
                }
            }
            UiAction::TreatyClausesPage(side, page) => {
                if let Some(current) = self.treaty_clause_page.get_mut(side) {
                    *current = page;
                }
            }
            UiAction::TreatyItemGroup(side, group) => {
                if side < self.treaty_item_group.len() {
                    self.treaty_item_group[side] = group;
                    self.treaty_item_page[side] = 0;
                }
            }
            UiAction::AdjustTreatyGold(owner, delta) => {
                if let Some(gold) = self.sim.treasury_view(owner) {
                    let treasury = gold.max(0);
                    self.treaty_gold_amount[owner] = self.treaty_gold_amount[owner]
                        .saturating_add(delta)
                        .clamp(0, treasury);
                    self.treaty_feedback = None;
                }
            }
            UiAction::AddTreatyGold(target, side, mut offer) => {
                let term_side = usize::from(side != HUMAN);
                if let Some(gold) = self.sim.treasury_view(side)
                    && term_side < offer.terms.len()
                {
                    let amount = self.treaty_gold_amount[side].min(gold.max(0));
                    offer.terms[term_side] = offer.terms[term_side].set_gold(amount);
                    self.treaty_draft = Some((target, offer));
                    self.treaty_feedback = None;
                }
            }
            UiAction::ProposeTreaty(owner, offer) => {
                let accepted = self.sim.apply(Command::ProposeTreaty(owner, offer.clone()));
                let message = self
                    .sim
                    .campaign_messages_view()
                    .last()
                    .cloned()
                    .unwrap_or_else(|| "The treaty proposal could not be delivered.".to_owned());
                self.treaty_feedback = Some((owner, message));
                if accepted {
                    self.treaty_draft = Some((owner, offer));
                }
            }
            UiAction::ReleaseVassal(other) => {
                self.sim.apply(Command::ReleaseVassal(other));
            }
            UiAction::CancelPact(other) => {
                self.sim.apply(Command::CancelPact(other));
            }
            UiAction::CancelSharedVision(other) => {
                self.sim.apply(Command::CancelSharedVision(other));
            }
            UiAction::ReviewOffer(other, offer) => {
                self.treaty_clause_page = [0; 2];
                self.overlay = Overlay::Diplomacy;
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
                            .campaign_messages_view()
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
            UiAction::ToggleReligion => {
                self.overlay.toggle(Overlay::Religion);
                self.religion_draft = None;
                self.religion_feedback.clear();
            }
            UiAction::ReligionCity(index) => self.religion_city = index,
            UiAction::ReligionFoundingChoice => {
                self.religion_choice =
                    (self.religion_choice + 1) % self.sim.religion_management_view().founding.len()
            }
            UiAction::ReligionFaith => {
                let view = self.sim.religion_management_view();
                let (faith, policy) = self
                    .religion_draft
                    .unwrap_or((view.current.declared, view.current.policy));
                let choices: Vec<_> = std::iter::once(None)
                    .chain(view.faiths.iter().map(|(id, _)| Some(*id)))
                    .collect();
                let i = choices.iter().position(|f| *f == faith).unwrap_or(0);
                self.religion_draft = Some((choices[(i + 1) % choices.len()], policy));
            }
            UiAction::ReligionPolicy(policy) => {
                let view = self.sim.religion_management_view();
                self.religion_draft = Some((
                    self.religion_draft.map_or(view.current.declared, |x| x.0),
                    policy,
                ));
            }
            UiAction::FoundReligion => {
                let view = self.sim.religion_management_view();
                if let Some((id, _)) = view
                    .cities
                    .get(self.religion_city.min(view.cities.len().saturating_sub(1)))
                {
                    let ok = self.sim.apply(Command::FoundReligion(
                        *id,
                        view.founding[self.religion_choice % view.founding.len()].id,
                    ));
                    self.religion_feedback = if ok {
                        "Religion founded. You can now declare it below."
                    } else {
                        "Cannot found: check required technology, founding city and availability."
                    }
                    .into();
                }
            }
            UiAction::ConfirmReligion => {
                if let Some((faith, policy)) = self.religion_draft {
                    let ok = self.sim.apply(Command::DeclareFaith(faith, policy));
                    self.religion_feedback = if ok {
                        "Declared faith and policy updated."
                    } else {
                        "No change applied. Check cooldown and faith presence."
                    }
                    .into();
                    if ok {
                        self.religion_draft = None;
                    }
                }
            }
            UiAction::ToggleGovernment => self.overlay.toggle(Overlay::Government),
            UiAction::ChangeGovernment(target) => {
                self.sim.apply(Command::Government(target));
            }
            UiAction::Mobilize => {
                self.sim.apply(Command::Mobilize);
            }
            UiAction::OpenResearch => {
                self.overlay = Overlay::Research;
                self.research_drag = None;
                self.dragging = false;
                self.center_research();
            }
            UiAction::CloseResearch => self.overlay.close(Overlay::Research),
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
