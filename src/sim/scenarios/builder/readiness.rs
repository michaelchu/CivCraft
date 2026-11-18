//! Presets for cities and units already in operation and projects in progress.

use std::collections::BTreeSet;

use super::WorldBuilder;
use crate::{
    rules::Tech,
    sim::{Production, UnitState, worklist::WORKLIST_LIMIT},
};

/// Movement is in the simulation's movement fragments.
pub(crate) struct UnitReadiness {
    pub moves_left: i32,
    pub fuel: u8,
    pub state: UnitState,
}

impl WorldBuilder<'_> {
    /// Install the current production, its stock, and the ordered worklist.
    /// Later items go through normal eligibility checks when reached.
    pub(crate) fn city_production(
        &mut self,
        city_id: u32,
        current: Production,
        shields: i32,
        worklist: &[Production],
    ) {
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city must exist");
        assert!(shields >= 0, "scenario shield stock cannot be negative");
        assert!(
            worklist.len() <= WORKLIST_LIMIT,
            "scenario worklist is too long"
        );
        assert!(
            self.game.can_produce(&self.game.cities[index], current),
            "city {city_id} cannot produce {}",
            current.name(),
        );
        let city = &mut self.game.cities[index];
        city.production = current;
        city.shields = shields;
        city.worklist = worklist.to_vec();
        city.bought_production = false;
        city.protected_shields = 0;
        city.production_before_switch = None;
    }

    /// Assign the citizens explicitly. The city center is worked automatically.
    pub(crate) fn city_workforce(
        &mut self,
        city_id: u32,
        worked: &[(i32, i32)],
        specialists: [i32; 3],
    ) {
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city must exist");
        let city = &self.game.cities[index];
        assert!(
            specialists.iter().all(|&count| count >= 0)
                && specialists
                    .iter()
                    .map(|&count| i64::from(count))
                    .sum::<i64>()
                    + worked.len() as i64
                    == i64::from(city.size),
            "scenario workforce must account for every citizen in city {city_id}"
        );
        let mut distinct = BTreeSet::new();
        for &(x, y) in worked {
            let is_in_radius = self
                .game
                .map_city_radius(city)
                .any(|(_, _, tx, ty)| (x, y) == (tx, ty));
            assert!(
                is_in_radius
                    && distinct.insert((x, y))
                    && self.game.tile_can_support_worker(city, x, y)
                    && self.game.cities.iter().all(|other| {
                        other.id == city_id || !other.worked_tiles.contains(&(x, y))
                    }),
                "city {city_id} cannot work scenario tile ({x}, {y})"
            );
        }

        let city = &mut self.game.cities[index];
        city.worked_tiles = worked.to_vec();
        city.specialists = specialists;
    }

    /// Board co-located cargo through the same transport checks as a normal order.
    pub(crate) fn load_cargo(&mut self, passenger: u32, carrier: u32) {
        assert!(
            self.game.board_specific_ship(passenger, carrier),
            "scenario unit {passenger} cannot board carrier {carrier}"
        );
    }

    pub(crate) fn unit_readiness(&mut self, id: u32, readiness: UnitReadiness) {
        let unit = self.game.unit_by_id(id).expect("fixture unit must exist");
        let allowance = self.game.unit_move_rate(unit);
        assert!(
            (0..=allowance).contains(&readiness.moves_left),
            "invalid scenario moves for unit {id}"
        );
        assert!(
            readiness.fuel <= unit.kind.fuel_capacity(),
            "invalid scenario fuel for unit {id}"
        );
        if matches!(
            readiness.state,
            UnitState::Fortified | UnitState::Fortifying
        ) {
            assert!(
                unit.transport.is_none()
                    && (matches!(unit.state, UnitState::Fortified | UnitState::Fortifying)
                        || self
                            .game
                            .action_available_fail_closed(id, "Fortify", (unit.x, unit.y))),
                "scenario unit {id} cannot fortify here"
            );
        }
        let unit = self.unit_mut(id);
        unit.moves_left = readiness.moves_left;
        unit.moved = readiness.moves_left < allowance;
        unit.fuel = readiness.fuel;
        unit.state = readiness.state;
    }

    /// Rates are ordered science, tax, luxury and use the current government's cap.
    pub(crate) fn economy(&mut self, owner: usize, treasury: i32, rates: [i32; 3]) {
        assert!(
            owner < self.game.civilization_count,
            "inactive scenario economy owner {owner}"
        );
        assert!(treasury >= 0, "scenario treasury cannot be negative");
        assert!(
            rates.iter().all(|rate| (0..=100).contains(rate)) && self.game.set_rates(owner, rates),
            "invalid starting rates {rates:?} for scenario slot {owner}"
        );
        self.game.players[owner].gold = treasury;
    }

    /// Start a legal research project already in progress.
    pub(crate) fn research(
        &mut self,
        owner: usize,
        target: Tech,
        progress: i32,
        goal: Option<Tech>,
    ) {
        assert!(
            owner < self.game.civilization_count,
            "inactive scenario research owner {owner}"
        );
        assert!(
            self.game.tech_available(owner, target),
            "scenario research target {target:?} is unavailable"
        );
        assert!(
            (0..self.game.target_cost(owner, target)).contains(&progress),
            "invalid scenario research progress for {target:?}"
        );
        if let Some(goal) = goal {
            assert!(
                goal == target || !self.game.shared_research_path(owner, goal).is_empty(),
                "unreachable scenario research goal {goal:?}"
            );
        }
        let player = &mut self.game.players[owner];
        player.researching = target;
        player.research = progress;
        player.research_goal = goal.unwrap_or(Tech::UNSET);
        player.researching_saved = None;
        player.research_saved = 0;
    }
}
