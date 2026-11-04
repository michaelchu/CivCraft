use super::{HUMAN, MoveResult, Production, Simulation, UnitState, WorkKind};
use crate::rules::Tech;

pub(crate) enum Command {
    Move(u32, i32, i32),
    GoTo(u32, i32, i32),
    FoundCity(u32),
    Work(u32, WorkKind),
    Fortify(u32),
    Wait(u32),
    Sentry(u32),
    Automate(u32),
    Produce(u32, Production),
    Research(Tech),
    RaiseRate(usize),
    Government(super::Government),
    Specialist(u32, usize, i32),
    HomeCity(u32),
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
            | GoTo(id, ..)
            | FoundCity(id)
            | Work(id, ..)
            | Fortify(id)
            | Wait(id)
            | Sentry(id)
            | Automate(id)
            | HomeCity(id) => Some(id),
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
            GoTo(id, x, y) => {
                (0..super::MAP_W).contains(&x)
                    && (0..super::MAP_H).contains(&y)
                    && self.goto_unit(id, x, y)
            }
            FoundCity(id) => self.found_city(id),
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
                unit.automated = !unit.automated;
                let name = unit.kind.name();
                let state = if unit.automated {
                    "enabled"
                } else {
                    "disabled"
                };
                self.add_event(format!("{name} automation {state}."));
                true
            }
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
            Government(target) => self.change_government(HUMAN, target),
            RaiseRate(index) => self.raise_rate(HUMAN, index),
            Specialist(id, index, delta) => self.assign_specialist(id, index, delta),
            HomeCity(_) => self.change_home_city(unit_index.unwrap()),
            Research(goal) => {
                if self.has_tech(HUMAN, goal) {
                    return false;
                }
                self.players[HUMAN].research_goal = goal;
                let next = self.research_target(HUMAN);
                let player = &mut self.players[HUMAN];
                if player.researching != next {
                    player.researching = next;
                    player.research = 0;
                }
                true
            }
            EndTurn => {
                self.end_turn();
                true
            }
        }
    }
}
