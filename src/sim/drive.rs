//! Scenario-facing entry points to private simulation rules.
//!
//! The wrapped methods are scoped to `crate::sim`. Scenarios import [`Drive`]
//! to invoke a rule step directly, so the production crate cannot reach them
//! by accident. Read-only queries stay `pub(crate)` on `Simulation`; only
//! state-mutating internals belong here. Prefer `Command` or a normal turn when
//! a scenario can use one.

use super::advisors::AiBuildChoice;
use super::{Pact, Simulation, TreatyOffer, lifecycle};
use crate::{
    rules::Tech,
    sim::{MoveResult, Unit},
};

pub(crate) trait Drive {
    fn found_religion(&mut self, owner: usize, city: u32, faith: u32) -> bool;
    fn declare_faith(
        &mut self,
        owner: usize,
        faith: Option<u32>,
        policy: super::FaithPolicy,
    ) -> bool;
    fn advance_diplomacy(&mut self);
    fn advance_diplomatic_offers_for(&mut self, owner: usize);
    fn advance_proactive_trades_for(&mut self, owner: usize);
    fn apply_research_upkeep(&mut self, owner: usize, income: i32);
    fn balance_unit_gold_upkeep(&mut self, owner: usize, paid: &[(u32, i32)]);
    fn cancel_pact(&mut self, owner: usize, other: usize) -> bool;
    fn capture_city_if_needed(&mut self, unit_id: u32, x: i32, y: i32);
    fn declare_ai_war_now(&mut self, owner: usize, target: usize);
    fn disband_unit(&mut self, id: u32, city_id: Option<u32>) -> bool;
    fn execute_offer(&mut self, a: usize, b: usize, offer: &TreatyOffer) -> bool;
    fn found_city(&mut self, unit_id: u32) -> bool;
    fn make_diplomatic_contact(&mut self, a: usize, b: usize);
    fn manage_ai_taxes(&mut self, owner: usize);
    fn process_player_cities(&mut self, owner: usize) -> (i32, Vec<(u32, i32)>);
    fn propose_treaty(&mut self, other: usize, offer: TreatyOffer) -> bool;
    fn reconcile_city_workers(&mut self);
    fn remove_city_because(
        &mut self,
        city_id: u32,
        reason: lifecycle::CityRemoval,
        by_owner: Option<usize>,
    );
    fn remove_units_with_cargo_because(&mut self, losses: Vec<u32>, reason: lifecycle::UnitRemoval);
    fn resolve_research_turn(&mut self, owner: usize, science: i32);
    fn restore_player_units(&mut self, owner: usize);
    fn run_ai_great_leader(&mut self, unit: &Unit) -> bool;
    fn select_ai_research(&mut self, owner: usize) -> (Tech, i32, i32);
    fn sign_pact(&mut self, a: usize, b: usize, proposed: Pact);
    fn spend_ai_defense_gold_with_choices(
        &mut self,
        owner: usize,
        choices: &std::collections::BTreeMap<u32, AiBuildChoice>,
    );
    fn transfer_traded_city(&mut self, index: usize, buyer: usize);
    fn transfer_treaty_technology(&mut self, donor: usize, receiver: usize, tech: Tech) -> bool;
    fn try_move_unit_civ3(&mut self, unit_id: u32, direction: usize) -> MoveResult;
}

impl Drive for Simulation {
    fn found_religion(&mut self, owner: usize, city: u32, faith: u32) -> bool {
        Simulation::found_religion(self, owner, city, faith)
    }
    fn declare_faith(
        &mut self,
        owner: usize,
        faith: Option<u32>,
        policy: super::FaithPolicy,
    ) -> bool {
        Simulation::declare_faith(self, owner, faith, policy)
    }
    fn advance_diplomacy(&mut self) {
        Simulation::advance_diplomacy(self)
    }
    fn advance_diplomatic_offers_for(&mut self, owner: usize) {
        Simulation::advance_diplomatic_offers_for(self, owner)
    }
    fn advance_proactive_trades_for(&mut self, owner: usize) {
        Simulation::advance_proactive_trades_for(self, owner)
    }
    fn apply_research_upkeep(&mut self, owner: usize, income: i32) {
        Simulation::apply_research_upkeep(self, owner, income)
    }
    fn balance_unit_gold_upkeep(&mut self, owner: usize, paid: &[(u32, i32)]) {
        Simulation::balance_unit_gold_upkeep(self, owner, paid)
    }
    fn cancel_pact(&mut self, owner: usize, other: usize) -> bool {
        Simulation::cancel_pact(self, owner, other)
    }
    fn capture_city_if_needed(&mut self, unit_id: u32, x: i32, y: i32) {
        Simulation::capture_city_if_needed(self, unit_id, x, y)
    }
    fn declare_ai_war_now(&mut self, owner: usize, target: usize) {
        Simulation::declare_ai_war_now(self, owner, target)
    }
    fn disband_unit(&mut self, id: u32, city_id: Option<u32>) -> bool {
        Simulation::disband_unit(self, id, city_id)
    }
    fn execute_offer(&mut self, a: usize, b: usize, offer: &TreatyOffer) -> bool {
        Simulation::execute_offer(self, a, b, offer)
    }
    fn found_city(&mut self, unit_id: u32) -> bool {
        Simulation::found_city(self, unit_id)
    }
    fn make_diplomatic_contact(&mut self, a: usize, b: usize) {
        Simulation::make_diplomatic_contact(self, a, b)
    }
    fn manage_ai_taxes(&mut self, owner: usize) {
        Simulation::manage_ai_taxes(self, owner)
    }
    fn process_player_cities(&mut self, owner: usize) -> (i32, Vec<(u32, i32)>) {
        Simulation::process_player_cities(self, owner)
    }
    fn propose_treaty(&mut self, other: usize, offer: TreatyOffer) -> bool {
        Simulation::propose_treaty(self, other, offer)
    }
    fn reconcile_city_workers(&mut self) {
        Simulation::reconcile_city_workers(self)
    }
    fn remove_city_because(
        &mut self,
        city_id: u32,
        reason: lifecycle::CityRemoval,
        by_owner: Option<usize>,
    ) {
        Simulation::remove_city_because(self, city_id, reason, by_owner)
    }
    fn remove_units_with_cargo_because(
        &mut self,
        losses: Vec<u32>,
        reason: lifecycle::UnitRemoval,
    ) {
        Simulation::remove_units_with_cargo_because(self, losses, reason)
    }
    fn resolve_research_turn(&mut self, owner: usize, science: i32) {
        Simulation::resolve_research_turn(self, owner, science)
    }
    fn restore_player_units(&mut self, owner: usize) {
        Simulation::restore_player_units(self, owner)
    }
    fn run_ai_great_leader(&mut self, unit: &Unit) -> bool {
        Simulation::run_ai_great_leader(self, unit)
    }
    fn select_ai_research(&mut self, owner: usize) -> (Tech, i32, i32) {
        Simulation::select_ai_research(self, owner)
    }
    fn sign_pact(&mut self, a: usize, b: usize, proposed: Pact) {
        Simulation::sign_pact(self, a, b, proposed)
    }
    fn spend_ai_defense_gold_with_choices(
        &mut self,
        owner: usize,
        choices: &std::collections::BTreeMap<u32, AiBuildChoice>,
    ) {
        Simulation::spend_ai_defense_gold_with_choices(self, owner, choices)
    }
    fn transfer_traded_city(&mut self, index: usize, buyer: usize) {
        Simulation::transfer_traded_city(self, index, buyer)
    }
    fn transfer_treaty_technology(&mut self, donor: usize, receiver: usize, tech: Tech) -> bool {
        Simulation::transfer_treaty_technology(self, donor, receiver, tech)
    }
    fn try_move_unit_civ3(&mut self, unit_id: u32, direction: usize) -> MoveResult {
        Simulation::try_move_unit_civ3(self, unit_id, direction)
    }
}
