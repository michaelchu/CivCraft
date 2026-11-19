//! Complete research, treaty transfer and upkeep loss through engine operations.
use super::{Checks, Scenario};
use crate::{
    rules::Tech,
    sim::{
        Simulation,
        fixtures::WorldBuilder,
        research_observation::{TechnologyAction, TechnologySource},
    },
};
pub(super) const SCENARIO: Scenario = Scenario::all_ai("research-history", execute);
fn execute(game: &mut Simulation, checks: &mut Checks) {
    game.ensure_research_observation();
    let tech = Tech::all()
        .find(|t| {
            t.is_civ3_advance()
                && !game.has_tech(0, *t)
                && !game.has_tech(1, *t)
                && game.tech_available(0, *t)
                && game.tech_available(1, *t)
                && !t.data().unwrap().first_discovery_bonus
        })
        .expect("fixture has a common available advance");
    WorldBuilder::new(game).research_project(0, tech, 0);
    let science = game.target_cost(0, tech) + game.technology_upkeep(0);
    game.resolve_research_turn(0, science);
    let traded = game.transfer_treaty_technology(0, 1, tech);
    WorldBuilder::new(game).research_progress(0, 0);
    game.apply_research_upkeep(0, -1);
    game.reconcile_research_observation();
    let s = game.observe_research();
    checks.extend([
        (
            "research_completed",
            s.changes.iter().any(|c| {
                c.owner == 0 && c.technology_id == tech.0 && c.source == TechnologySource::Research
            }),
            "Completed science identifies its technology and owner".into(),
        ),
        (
            "technology_traded",
            traded
                && s.changes.iter().any(|c| {
                    c.owner == 1 && c.source == TechnologySource::Trade && c.other_owner == Some(0)
                }),
            "Technology transfer is distinct from research".into(),
        ),
        (
            "technology_lost",
            s.changes.iter().any(|c| {
                c.owner == 0
                    && c.source == TechnologySource::Upkeep
                    && c.action == TechnologyAction::Lost
            }),
            "Negative science uses the engine's upkeep-driven technology loss".into(),
        ),
        (
            "research_history_complete",
            s.complete_since_start,
            "Knowledge changes reconcile with live player knowledge".into(),
        ),
    ]);
}
