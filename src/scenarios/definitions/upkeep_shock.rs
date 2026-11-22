//! Loss of a major tax center while the surviving cities defend a frontier.

use super::*;
use crate::{
    rules::{Building, Tech},
    sim::{
        Government, HUMAN, Pact, Production, Simulation, UnitKind,
        drive::Drive,
        fixtures::{CivPreset, TileSurface, WorldBuilder},
        lifecycle::CityRemoval,
    },
    world::{GeneratedMap, Terrain, topology::offset_native_by_map},
};
use serde_json::{Value, json};

const OWNER: usize = 1;
const SHOCK_TURN: u32 = 5;
const DEADLINE: u32 = 25;

pub(super) const SCENARIO: Scenario = Scenario::new(
    "wartime-upkeep-shock",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup,
    },
);

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let techs: Vec<_> = Tech::all()
        .filter(|tech| tech.is_researchable_advance())
        .take(25)
        .collect();
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &["rome", "korea"].map(|id| CivPreset {
            id,
            technologies: Some(&techs),
        }),
    );
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(80, -1000);
    world.map_surface(TileSurface {
        terrain: Some(Terrain::GrasslandShield),
        road: Some(true),
        ..Default::default()
    });
    let front = (14, 22);
    let enemy = offset_native_by_map(front.0, front.1, 5, 0);
    let capital = world.city(OWNER, (7, 20), 5);
    world.city_buildings(capital, &[Building::PALACE, Building::MARKETPLACE]);
    world.city_workers(capital, 5);
    let revenue = world.city(OWNER, (8, 34), 12);
    world.city_buildings(
        revenue,
        &[Building::AQUEDUCT, Building::MARKETPLACE, Building::BANK],
    );
    world.city_workers(revenue, 12);
    let frontier = world.city(OWNER, front, 4);
    world.city_workers(frontier, 4);
    let rival = world.city(HUMAN, enemy, 5);
    world.production_stock(rival, Production::Building(Building::COINAGE), 0);
    world.unit(HUMAN, UnitKind::Pikeman, enemy);
    world.unit(
        HUMAN,
        UnitKind::Swordsman,
        offset_native_by_map(front.0, front.1, 2, 0),
    );
    for position in [front, (7, 20), (8, 34)] {
        for _ in 0..3 {
            world.unit(OWNER, UnitKind::Pikeman, position);
        }
    }
    for _ in 0..4 {
        world.unit(OWNER, UnitKind::Pikeman, (8, 34));
    }
    world.government_state(OWNER, Government::REPUBLIC);
    world.economy(OWNER, 50, [50, 50, 0]);
    world.diplomacy(OWNER, HUMAN, Pact::War, 1, [-10_000; 2]);
    world.mark_known(OWNER, enemy);
    world.finish();
    (
        game,
        Box::new(Audit {
            revenue,
            frontier,
            ..Default::default()
        }),
    )
}

#[derive(Default)]
struct Audit {
    revenue: u32,
    frontier: u32,
    shock: Option<(i32, i32, i32)>,
    baseline_science: Option<i32>,
    deficit: Option<(u32, i32)>,
    lowest_post_shock_science: Option<i32>,
    restored: Option<u32>,
    forced_losses: usize,
    trace_dropped: bool,
    defended_turns: u32,
    observed_shock_turns: u32,
}

impl ScenarioRun for Audit {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["economy.upkeep_shortfall"]
    }

    fn before_turn(&mut self, game: &mut Simulation, completed: u32) {
        if completed == 0 && !game.observe_research().history_available {
            game.start_research_observation();
        }
        if completed + 1 == SHOCK_TURN
            && game
                .city_by_id(self.revenue)
                .is_some_and(|city| city.owner == OWNER)
        {
            let before = game.ai_projected_income(OWNER);
            let gold = game.players()[OWNER].gold;
            game.remove_city_because(self.revenue, CityRemoval::Disaster, None);
            self.shock = Some((before, game.ai_projected_income(OWNER), gold));
        }
    }

    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        let shortfalls: Vec<_> = game
            .trace
            .pending_events()
            .iter()
            .filter(|event| {
                event.kind == "economy.upkeep_shortfall" && event.payload["owner"] == OWNER
            })
            .map(|event| &event.payload)
            .collect();
        self.trace_dropped |= game.trace.dropped() > 0;
        let guarded = game.city_by_id(self.frontier).is_some_and(|city| {
            city.owner == OWNER
                && game.units().iter().any(|unit| {
                    unit.owner == OWNER
                        && unit.kind.is_military()
                        && !unit.kind.is_naval()
                        && unit.kind.stats().defense > 0
                        && (unit.x, unit.y) == (city.x, city.y)
                })
        });
        let strategy = game.strategic_priorities(OWNER);
        let science = game
            .observe_research()
            .civilizations
            .into_iter()
            .find(|nation| nation.owner == OWNER)
            .and_then(|nation| nation.science_output)
            .map(|science| science.net);
        let rates = game.players()[OWNER].rates;
        if turn + 1 == SHOCK_TURN {
            self.baseline_science = science;
        }
        if self.shock.is_some() && turn <= DEADLINE {
            self.forced_losses += shortfalls.len();
            self.observed_shock_turns += 1;
            if guarded && game.at_war(OWNER, HUMAN) {
                self.defended_turns += 1;
            }
            if turn < SHOCK_TURN + 3 && strategy.projected_income < 0 {
                self.deficit
                    .get_or_insert((turn, strategy.projected_income));
            }
            if let Some(science) = science {
                self.lowest_post_shock_science = Some(
                    self.lowest_post_shock_science
                        .map_or(science, |lowest| lowest.min(science)),
                );
            }
            if self.deficit.is_some()
                && strategy.projected_income >= 0
                && science
                    .zip(self.baseline_science)
                    .is_some_and(|(now, before)| before > 0 && now >= before)
            {
                self.restored.get_or_insert(turn);
            }
        }
        json!({"shock_income_before_after_and_treasury": self.shock, "rates": rates,
            "net_science": science, "projected_income": strategy.projected_income,
            "first_post_shock_deficit": self.deficit,
            "treasury": game.players()[OWNER].gold, "frontier_guarded": guarded,
            "at_war": game.at_war(OWNER, HUMAN), "upkeep_shortfalls": shortfalls})
    }

    fn finish(self: Box<Self>, turns: u32) -> Vec<Check> {
        vec![
            Check {
                name: "solvent_empire_loses_income",
                passed: self
                    .shock
                    .is_some_and(|(before, after, _)| before >= 0 && after < before)
                    && self.deficit.is_some(),
                detail: format!(
                    "Projected income before and after, with treasury at loss {:?}. First deficit (turn, income) {:?}.",
                    self.shock, self.deficit
                ),
            },
            Check {
                name: "shock_preserves_units_and_frontier",
                passed: turns >= DEADLINE
                    && self.observed_shock_turns > 0
                    && self.forced_losses == 0
                    && !self.trace_dropped
                    && self.defended_turns == self.observed_shock_turns,
                detail: format!(
                    "{} upkeep shortfalls, defended during war for {}/{} post-shock turns",
                    self.forced_losses, self.defended_turns, self.observed_shock_turns
                ),
            },
            Check {
                name: "shock_restores_research",
                passed: self
                    .lowest_post_shock_science
                    .zip(self.baseline_science)
                    .is_some_and(|(lowest, before)| lowest < before)
                    && self.restored.is_some(),
                detail: format!(
                    "Pre-shock net science {:?}, post-shock low {:?}, restored under solvent rates on {:?} by turn {DEADLINE}",
                    self.baseline_science, self.lowest_post_shock_science, self.restored
                ),
            },
        ]
    }
}
