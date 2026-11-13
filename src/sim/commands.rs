//! Player actions represented as commands and validated by the simulation.
//!
//! The client constructs a [`Command`] after interpreting a click or keypress;
//! [`Simulation::apply`] checks ownership and game-over state, then delegates
//! to the rule system that owns the requested action. This keeps input code
//! from changing campaign state directly and gives desktop automation the same
//! command path as a human player. AI decisions commonly call the lower-level
//! rule methods after choosing their actions.

use super::{
    HUMAN, MoveResult, Production, Simulation, UnitKind, UnitState, WorkKind, WorklistEdit,
};
use crate::rules::{Building, Tech};

pub(crate) enum Command {
    Move(u32, i32, i32),
    MoveCiv3(u32, usize),
    Bombard(u32, i32, i32),
    GoTo(u32, i32, i32),
    FoundCity(u32),
    Work(u32, WorkKind),
    Fortify(u32),
    Wait(u32),
    Sentry(u32),
    Automate(u32),
    Produce(u32, Production),
    RushProduction(u32),
    EditWorklist(u32, WorklistEdit),
    SellBuilding(u32, Building),
    Research(Tech),
    RaiseRate(usize),
    Government(super::Government),
    Mobilize,
    Negotiate(usize, super::Pact),
    ProposeTreaty(usize, super::TreatyOffer),
    BuyMap(usize, super::TreatyMap),
    BuyCity(u32, i32),
    CancelPact(usize),
    DeclineOffer(usize),
    AcceptOffer(usize, super::TreatyOffer),
    Specialist(u32, usize, i32),
    WorkCityTile(u32, i32, i32),
    HomeCity(u32),
    BoardOrDeboard(u32),
    Disband(u32, Option<u32>),
    NuclearStrike(u32, i32, i32),
    HelpWonder(u32, u32),
    JoinCity(u32, u32),
    Airlift(u32, u32),
    AirSuperiority(u32),
    Rebase(u32, i32, i32),
    Paradrop(u32, i32, i32),
    UpgradeUnit(u32, UnitKind, i32),
    LaunchSpaceship,
    ToggleManualAssembly,
    PlaceShipPart(super::ShipPart),
    EndTurn,
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
            Move(id, ..)
            | MoveCiv3(id, _)
            | Bombard(id, ..)
            | GoTo(id, ..)
            | FoundCity(id)
            | Work(id, ..)
            | Fortify(id)
            | Wait(id)
            | Sentry(id)
            | Automate(id)
            | HomeCity(id)
            | UpgradeUnit(id, _, _)
            | Disband(id, _)
            | BoardOrDeboard(id)
            | NuclearStrike(id, ..)
            | HelpWonder(id, _)
            | JoinCity(id, _)
            | Paradrop(id, ..)
            | Airlift(id, _) => Some(id),
            AirSuperiority(id) => Some(id),
            Rebase(id, ..) => Some(id),
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
        match command {
            Move(id, dx, dy) => self.try_move_unit(id, dx, dy) != MoveResult::Blocked,
            MoveCiv3(id, direction) => {
                self.try_move_unit_civ3(id, direction) != MoveResult::Blocked
            }
            Bombard(id, x, y) => self.try_ranged_bombard(id, x, y) != MoveResult::Blocked,
            GoTo(id, x, y) => {
                (0..self.map_wrap.width()).contains(&x)
                    && (0..self.map_wrap.height()).contains(&y)
                    && self.player_goto(id, x, y)
            }
            FoundCity(id) => self.found_city(id),
            LaunchSpaceship => self.launch_spaceship(HUMAN),
            ToggleManualAssembly => self.toggle_manual_assembly(),
            PlaceShipPart(part) => self.place_ship_part(part),
            Disband(id, city) => self.disband_unit(id, city),
            NuclearStrike(id, x, y) => self.launch_nuclear_strike(id, x, y),
            HelpWonder(id, city) => self.help_wonder(id, city),
            JoinCity(id, city) => self.join_city(id, city),
            Paradrop(id, x, y) => self.paradrop(id, x, y),
            Airlift(id, city) => self.airlift(id, city),
            AirSuperiority(id) => self.assign_air_superiority(id),
            Rebase(id, x, y) => self.rebase_aircraft(id, x, y),
            Work(id, kind) => self.begin_work(id, kind),
            Fortify(id) => self.fortify_unit(id),
            Wait(_) | Sentry(_) => {
                let unit = &mut self.units[unit_index.unwrap()];
                unit.moves_left = 0;
                unit.state = if matches!(command, Wait(_)) {
                    UnitState::Idle
                } else {
                    UnitState::Sentry
                };
                if matches!(command, Wait(_)) {
                    self.add_event("Unit waits until next turn.");
                }
                true
            }
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
                let name = unit.kind.name();
                let state = if unit.automated {
                    "enabled"
                } else {
                    "disabled"
                };
                self.add_event(format!("{name} automation {state}."));
                true
            }
            EditWorklist(id, edit) => self.edit_worklist(id, edit),
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
            BuyCity(city, price) => self.buy_city(city, price),
            BuyMap(other, map) => self.buy_map(other, map),
            Negotiate(other, pact) => self.negotiate(other, pact),
            ProposeTreaty(other, offer) => self.propose_treaty(other, offer),
            CancelPact(other) => self.cancel_pact(HUMAN, other),
            AcceptOffer(other, offer) => self.accept_offer(other, offer),
            DeclineOffer(other) => self.decline_offer(other),
            Government(target) => self.change_government(HUMAN, target),
            Mobilize => self.mobilize(HUMAN),
            RaiseRate(index) => self.raise_rate(HUMAN, index),
            WorkCityTile(id, x, y) => self.toggle_city_tile(id, x, y),
            Specialist(id, index, delta) => self.assign_specialist(id, index, delta),
            BoardOrDeboard(id) => self.board_or_deboard(id),
            UpgradeUnit(id, target, price) => self.upgrade_unit(id, target, price),
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
                true
            }
        }
    }
}
