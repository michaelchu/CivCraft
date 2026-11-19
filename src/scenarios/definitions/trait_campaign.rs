//! Matched campaigns comparing Commercial/Militaristic and Commercial/Scientific.

use super::*;
use crate::{
    rules::{Building, Tech},
    sim::{
        Pact, Production, Simulation, UnitKind,
        fixtures::{CivPreset, TileSurface, WorldBuilder},
        trace::{Collector, Entity, Event},
    },
    world::{GeneratedMap, Terrain, topology::offset_native_by_map},
};
use serde_json::{Value, json};

const RIVAL: usize = 0;
const SUBJECT: usize = 1;
const PARTNER: usize = 2;
const CAMPAIGN_TURNS: u32 = 20;
const EVENTS: &[&str] = &[
    "production.completed",
    "production.rushed",
    "diplomacy.transition",
];

pub(super) const SCENARIO: Scenario = Scenario::new(
    "trait-campaign",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup,
    },
);

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let technologies = [Tech::ALPHABET, Tech::BRONZE_WORKING, Tech::WARRIOR_CODE];
    let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &["france", "rome", "egypt"].map(|id| CivPreset {
            id,
            technologies: Some(&technologies),
        }),
    );
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(40, -2000);
    world.map_surface(TileSurface {
        terrain: Some(Terrain::GrasslandShield),
        road: Some(true),
        ..Default::default()
    });
    let home = (10, 24);
    let front = offset_native_by_map(home.0, home.1, 8, 0);
    let rear = offset_native_by_map(home.0, home.1, -5, 0);
    let market = (26, 34);
    for (owner, position, population) in [
        (SUBJECT, home, 4),
        (SUBJECT, rear, 4),
        (RIVAL, front, 2),
        (PARTNER, market, 3),
    ] {
        let city = world.city(owner, position, population);
        world.city_workers(city, population as usize);
        world.production_stock(city, Production::Building(Building::COINAGE), 0);
        world.unit(owner, UnitKind::Spearman, position);
    }
    world.add_technology(PARTNER, writing);
    world.treasury(PARTNER, 0);
    world.treasury(SUBJECT, 500);
    world.last_science(SUBJECT, 20);
    world.diplomacy(SUBJECT, PARTNER, Pact::Alliance, 1, [6000; 2]);
    world.embassy(SUBJECT, PARTNER);
    // After the first peaceful attitude update, trust falls between the two
    // optional-war thresholds.
    world.diplomacy(SUBJECT, RIVAL, Pact::Peace, 1, [-2520; 2]);
    for dx in 0..=8 {
        for dy in -3..=3 {
            world.mark_known(SUBJECT, offset_native_by_map(home.0, home.1, dx, dy));
        }
    }
    world.finish();
    let mut scientific = game.clone();
    WorldBuilder::new(&mut scientific).civilization_identity(SUBJECT, "korea");
    scientific.trace = Collector::enabled(EVENTS.iter().map(|kind| (*kind).into()).collect(), 1024);
    (
        game,
        Box::new(Comparison {
            scientific,
            rome: Outcomes::default(),
            korea: Outcomes::default(),
            synchronized: true,
            reached_game_over: false,
        }),
    )
}

#[derive(Default)]
struct Outcomes {
    first_preparation: Option<u32>,
    first_war: Option<u32>,
    completed: usize,
    rushed: usize,
    trades: usize,
    dropped_events: bool,
}

impl Outcomes {
    fn observe(&mut self, game: &Simulation, turn: u32) -> Value {
        self.dropped_events |= game.trace.dropped() > 0;
        let player = &game.players()[SUBJECT];
        let border = game
            .observe_diplomacy()
            .into_iter()
            .find(|pair| pair.players == [RIVAL, SUBJECT]);
        let preparation = border.as_ref().and_then(|pair| pair.war_plans[1].as_ref());
        let objective = preparation.and_then(|plan| plan.objective);
        let war = game.at_war(SUBJECT, RIVAL);
        if preparation.is_some_and(|plan| plan.countdown >= 0) {
            self.first_preparation.get_or_insert(turn);
        }
        if war {
            self.first_war.get_or_insert(turn);
        }
        let mut completed = Vec::new();
        let mut rushed = Vec::new();
        let mut trades = Vec::new();
        for event in game
            .trace
            .pending_events()
            .iter()
            .filter(|event| event.entities.contains(&Entity::Civilization(SUBJECT)))
        {
            match event.kind.as_str() {
                "production.completed" => completed.push(city_event(event)),
                "production.rushed" => rushed.push(city_event(event)),
                "diplomacy.transition"
                    if event.payload["action"] == "offer_accepted"
                        && event.payload["offer"]["terms"]
                            .as_array()
                            .is_some_and(|terms| {
                                terms.iter().any(|term| {
                                    !term["technology"].is_null()
                                        || term["additional_technologies"]
                                            .as_array()
                                            .is_some_and(|techs| !techs.is_empty())
                                })
                            }) =>
                {
                    trades.push(&event.payload);
                }
                _ => {}
            }
        }
        self.completed += completed.len();
        self.rushed += rushed.len();
        self.trades += trades.len();
        let queue: Vec<_> = game
            .cities()
            .iter()
            .filter(|city| city.owner == SUBJECT)
            .map(|city| json!({"city": city.id, "item": city.production.name()}))
            .collect();
        json!({
            "identity": player.civilization_id,
            "gold": player.gold,
            "rates": player.rates,
            "research": player.researching.name(),
            "research_goal": player.research_goal.name(),
            "research_progress": player.research,
            "known_technologies": player.known.iter().map(|tech| tech.name()).collect::<Vec<_>>(),
            "production_queue": queue,
            "production_completed": completed,
            "rush_purchases": rushed,
            "technology_trades": trades,
            "rival_trust": border.as_ref().map(|pair| pair.trust[1]),
            "war_preparation": preparation,
            "war_objective": objective,
            "at_war_with_rival": war,
            "game_over": game.game_over().is_some(),
        })
    }
}

fn city_event(event: &Event) -> Value {
    json!({"entities": event.entities, "details": event.payload})
}

struct Comparison {
    scientific: Simulation,
    rome: Outcomes,
    korea: Outcomes,
    synchronized: bool,
    reached_game_over: bool,
}

impl ScenarioRun for Comparison {
    fn trace_kinds(&self) -> &'static [&'static str] {
        EVENTS
    }

    fn companion_campaigns(&mut self) -> &mut [Simulation] {
        std::slice::from_mut(&mut self.scientific)
    }

    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        self.synchronized &= game.turn() == self.scientific.turn();
        self.reached_game_over = game.game_over().is_some();
        let rome = self.rome.observe(game, turn);
        let korea = self.korea.observe(&self.scientific, turn);
        self.scientific.trace.drain();
        json!({"rome": rome, "korea": korea})
    }

    fn finish(self: Box<Self>, turns: u32) -> Vec<Check> {
        let rome = self.rome;
        let korea = self.korea;
        vec![
            Check {
                name: "matched_campaign_turns",
                passed: (turns >= CAMPAIGN_TURNS || self.reached_game_over)
                    && self.synchronized
                    && !rome.dropped_events
                    && !korea.dropped_events
                    && rome.completed > 0
                    && korea.completed > 0
                    && rome.rushed > 0
                    && korea.rushed > 0
                    && rome.trades > 0
                    && korea.trades > 0,
                detail: format!(
                    "{turns} paired turns (synchronized {}, no dropped events {}). Rome/Korea completions {}/{}, rushes {}/{}, trades {}/{}.",
                    self.synchronized,
                    !rome.dropped_events && !korea.dropped_events,
                    rome.completed,
                    korea.completed,
                    rome.rushed,
                    korea.rushed,
                    rome.trades,
                    korea.trades,
                ),
            },
            Check {
                name: "militaristic_prepares_earlier",
                passed: rome.first_preparation.is_some_and(|rome_turn| {
                    korea
                        .first_preparation
                        .is_none_or(|korea_turn| rome_turn < korea_turn)
                }),
                detail: format!(
                    "Rome/Korea first preparation {:?}/{:?}, first war {:?}/{:?}",
                    rome.first_preparation,
                    korea.first_preparation,
                    rome.first_war,
                    korea.first_war,
                ),
            },
        ]
    }
}
