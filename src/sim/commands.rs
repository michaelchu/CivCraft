//! Player actions represented as commands and validated by the simulation.
//!
//! The client constructs a [`Command`] after interpreting a click or keypress;
//! [`Simulation::apply`] checks ownership and game-over state, then delegates
//! to the rule system that owns the requested action. This keeps input code
//! from changing campaign state directly and gives desktop automation the same
//! command path as a human player. AI decisions commonly call the lower-level
//! rule methods after choosing their actions.

use super::{
    CityGovernor, HUMAN, MoveResult, Production, Simulation, UnitKind, UnitState, WorkKind,
    WorklistEdit,
};
use crate::rules::{Building, Tech};

pub(crate) enum Command {
    MoveCiv3(u32, usize),
    CaptureUnits(u32, i32, i32),
    Bombard(u32, i32, i32),
    AutoBombard(u32, i32, i32),
    PrecisionBombard(u32, i32, i32),
    AutoPrecisionBombard(u32, i32, i32),
    AirSuperiority(u32),
    CancelOrder(u32),
    GoTo(u32, i32, i32),
    GoToGroup(Vec<u32>, i32, i32),
    Connect(u32, i32, i32),
    Patrol(u32, i32, i32),
    FoundCity(u32),
    Work(u32, WorkKind),
    Fortify(u32),
    FormArmy(u32),
    Sentry(u32),
    Automate(u32),
    Produce(u32, Production),
    RushProduction(u32),
    EditWorklist {
        city_id: u32,
        edit: WorklistEdit,
    },
    SellBuilding(u32, Building),
    Research(Tech),
    /// Sets the science, tax and luxury split; see `Simulation::set_rates`.
    SetRates([i32; 3]),
    Government(super::Government),
    Mobilize,
    FoundReligion(u32, u32),
    DeclareFaith(Option<u32>, super::FaithPolicy),
    Espionage(
        usize,
        super::EspionageMission,
        Option<u32>,
        super::EspionageRisk,
    ),
    #[allow(dead_code)]
    Negotiate(usize, super::Pact),
    ProposeTreaty(usize, super::TreatyOffer),
    ShareTreatyCityMap(usize, u32),
    CancelPact(usize),
    ReleaseVassal(usize),
    CancelSharedVision(usize),
    DeclineOffer(usize),
    AcceptOffer(usize, super::TreatyOffer),
    Specialist {
        city_id: u32,
        index: usize,
        delta: i32,
    },
    SetCityGovernor {
        city_id: u32,
        state: CityGovernor,
    },
    WorkCityTile {
        city_id: u32,
        x: i32,
        y: i32,
    },
    HomeCity(u32),
    BoardOrDeboard(u32),
    UnloadAll(u32),
    Disband {
        unit_id: u32,
        city_id: Option<u32>,
    },
    NuclearStrike(u32, i32, i32),
    CompleteProduction {
        unit_id: u32,
        city_id: u32,
    },
    JoinCity {
        unit_id: u32,
        city_id: u32,
    },
    Airlift(u32, u32),
    Paradrop(u32, i32, i32),
    UpgradeUnit {
        unit_id: u32,
        target: UnitKind,
        price: i32,
    },
    LaunchSpaceship,
    ToggleManualAssembly,
    PlaceShipPart {
        part: super::ShipPart,
    },
    EndTurn,
}

impl Command {
    /// Build a Civ III direction command for an adjacent map-space offset.
    /// This keeps coordinate-based fixtures on the eight-direction command path.
    pub(crate) fn move_civ3_by_offset(
        simulation: &Simulation,
        unit_id: u32,
        dx: i32,
        dy: i32,
    ) -> Self {
        let direction = simulation
            .civ3_direction_for_unit_offset(unit_id, dx, dy)
            .unwrap_or(usize::MAX);
        Self::MoveCiv3(unit_id, direction)
    }
}

impl Simulation {
    /// Apply an action from the human player. AI calls the same rule methods
    /// after choosing actions for its own units.
    pub(crate) fn apply(&mut self, command: Command) -> bool {
        use Command::*;
        if self.game_over.is_some() {
            return false;
        }
        let unit_id = match command {
            MoveCiv3(id, _)
            | CaptureUnits(id, ..)
            | Bombard(id, ..)
            | AutoBombard(id, ..)
            | PrecisionBombard(id, ..)
            | AutoPrecisionBombard(id, ..)
            | AirSuperiority(id)
            | CancelOrder(id)
            | GoTo(id, ..)
            | Connect(id, ..)
            | Patrol(id, ..)
            | FoundCity(id)
            | Work(id, ..)
            | Fortify(id)
            | FormArmy(id)
            | Sentry(id)
            | Automate(id)
            | HomeCity(id)
            | UpgradeUnit { unit_id: id, .. }
            | Disband { unit_id: id, .. }
            | BoardOrDeboard(id)
            | UnloadAll(id)
            | NuclearStrike(id, ..)
            | CompleteProduction { unit_id: id, .. }
            | JoinCity { unit_id: id, .. }
            | Paradrop(id, ..)
            | Airlift(id, _) => Some(id),
            _ => None,
        };
        let unit_index = unit_id.and_then(|id| {
            self.units
                .iter()
                .position(|unit| unit.id == id && unit.owner == HUMAN)
        });
        if unit_id.is_some() && unit_index.is_none() {
            return false;
        }
        if unit_index.is_some_and(|index| self.is_army_member(&self.units[index])) {
            return false;
        }
        let applied = match command {
            MoveCiv3(id, direction) => {
                self.try_move_unit_civ3(id, direction) != MoveResult::Blocked
            }
            CaptureUnits(id, x, y) => self.try_capture_units_by_id(id, x, y),
            Bombard(id, x, y) => {
                let succeeded = self.try_ranged_bombard(id, x, y) != MoveResult::Blocked;
                if succeeded {
                    self.cancel_unit_order(id);
                }
                succeeded
            }
            AutoBombard(id, x, y) => self.start_repeated_bombard(id, x, y),
            PrecisionBombard(id, x, y) => {
                let succeeded = self.try_precision_bombard(id, x, y) != MoveResult::Blocked;
                if succeeded {
                    self.cancel_unit_order(id);
                }
                succeeded
            }
            AutoPrecisionBombard(id, x, y) => self.start_repeated_precision_bombard(id, x, y),
            AirSuperiority(id) => self.start_air_superiority(id),
            CancelOrder(id) => self.cancel_unit_order(id),
            GoTo(id, x, y) => self.issue_player_goto(id, x, y),
            GoToGroup(ids, x, y) => self.issue_player_goto_group(ids, x, y),
            Connect(id, x, y) => self.start_connect_order(id, x, y),
            Patrol(id, x, y) => self.start_patrol_order(id, x, y),
            FoundCity(id) => self.found_city(id),
            LaunchSpaceship => self.launch_spaceship(HUMAN),
            ToggleManualAssembly => self.toggle_manual_assembly(),
            PlaceShipPart { part } => self.place_ship_part(part),
            Disband { unit_id, city_id } => self.disband_unit(unit_id, city_id),
            NuclearStrike(id, x, y) => self.launch_nuclear_strike(id, x, y),
            CompleteProduction { unit_id, city_id } => self.complete_production(unit_id, city_id),
            JoinCity { unit_id, city_id } => self.join_city(unit_id, city_id),
            Paradrop(id, x, y) => self.paradrop(id, x, y),
            Airlift(id, city) => self.airlift(id, city),
            Work(id, kind) => self.begin_work(id, kind),
            Fortify(id) => self.fortify_unit(id),
            FormArmy(id) => self.form_army(id),
            Sentry(id) => self.set_unit_sentry(id),
            Automate(_) => {
                let unit = &mut self.units[unit_index.unwrap()];
                // common/unit.c: ACTIVITY_EXPLORE excludes fuelled types and
                // helicopter-class units that lose HP away from a base.
                if !unit.automated
                    && (unit.kind.fuel_capacity() > 0 || unit.kind == UnitKind::Helicopter)
                {
                    return false;
                }
                unit.automated = !unit.automated;
                if unit.automated && !unit.kind.stats().can_work {
                    unit.clear_work();
                    unit.state = UnitState::Idle;
                }
                true
            }
            EditWorklist { city_id, edit } => self.edit_worklist(city_id, edit),
            RushProduction(id) => self.rush_production(HUMAN, id),
            SellBuilding(id, building) => self.sell_building(HUMAN, id, building),
            Produce(id, kind) => {
                let Some(city) = self
                    .cities
                    .iter()
                    .find(|city| city.id == id && city.owner == HUMAN)
                else {
                    return false;
                };
                let name = city.name.clone();
                if !self.change_production(id, kind) {
                    return false;
                }
                self.add_event(format!("{name} now produces {}.", kind.name()));
                true
            }
            Negotiate(other, pact) => self.negotiate(other, pact),
            ProposeTreaty(other, offer) => self.propose_treaty(other, offer),
            ShareTreatyCityMap(other, city) => self.share_treaty_city_map(other, city),
            ReleaseVassal(other) => self.release_vassal(HUMAN, other),
            CancelPact(other) => self.cancel_pact(HUMAN, other),
            CancelSharedVision(other) => self.cancel_shared_vision(HUMAN, other),
            AcceptOffer(other, offer) => self.accept_offer(other, offer),
            DeclineOffer(other) => self.decline_offer(other),
            FoundReligion(city, faith) => self.found_religion(HUMAN, city, faith),
            DeclareFaith(faith, policy) => self.declare_faith(HUMAN, faith, policy),
            Government(target) => self.change_government(HUMAN, target),
            Mobilize => self.mobilize(HUMAN),
            Espionage(owner, mission, city, risk) => {
                self.perform_espionage(owner, mission, city, risk)
            }
            SetRates(rates) => self.set_rates(HUMAN, rates),
            SetCityGovernor { city_id, state } => self.set_city_governor(city_id, state),
            WorkCityTile { city_id, x, y } => self.toggle_city_tile(city_id, x, y),
            Specialist {
                city_id,
                index,
                delta,
            } => self.assign_specialist(city_id, index, delta),
            BoardOrDeboard(id) => self.board_or_deboard(id),
            UnloadAll(id) => self.unload_all(id),
            UpgradeUnit {
                unit_id,
                target,
                price,
            } => self.upgrade_unit(unit_id, target, price),
            HomeCity(_) => self.change_home_city(unit_index.unwrap()),
            Research(goal) => {
                if ((goal.data().is_none() || !goal.is_civ3_advance()) && goal != Tech::FUTURE)
                    || self.has_tech(HUMAN, goal)
                {
                    return false;
                }
                self.players[HUMAN].research_goal = goal;
                let next = self.choose_research_target(HUMAN);
                self.choose_research_tech(HUMAN, next, 0);
                true
            }
            EndTurn => {
                self.end_turn();
                return true;
            }
        };
        if applied {
            self.refresh_city_governors();
        }
        applied
    }
}
