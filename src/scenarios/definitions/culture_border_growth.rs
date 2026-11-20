//! Culture-only borders growing over several turns: the square, the fat X and
//! each later expansion, plus a strength contest where two borders meet.

use super::*;
use crate::{
    rules::Building,
    sim::{
        HUMAN, Simulation,
        drive::Drive,
        fixtures::{CivPreset, TileSurface, WorldBuilder},
    },
    world::{GeneratedMap, Terrain},
};
use serde_json::{Value, json};

const OWNER: usize = 1;
/// Tiles held by a lone city at border radius 1 through 5 (square, fat X, ...).
const TILES_BY_RADIUS: [usize; 5] = [9, 21, 45, 77, 117];
/// Culture staged one point below each threshold, injected before the turn on
/// which the Palace's single point carries the city across it.
const STAGED_CULTURE: [(u32, i32); 3] = [(3, 99), (5, 999), (7, 9_999)];

pub(super) const SCENARIO: Scenario = Scenario::new(
    "culture-border-growth",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup,
    },
);

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &["rome", "korea"].map(|id| CivPreset {
            id,
            technologies: None,
        }),
    );
    let wrap = game.map_wrap;
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(80, -1000);
    world.map_surface(TileSurface {
        terrain: Some(Terrain::Grassland),
        ..Default::default()
    });
    world.fertile_tiles(None, Some(HUMAN));
    world.fertile_tiles(None, Some(OWNER));

    // A lone capital: 8 culture at the start, then staged jumps to each tier.
    let lone = world.city(HUMAN, (8, 24), 3);
    world.city_buildings(lone, &[Building::PALACE]);
    world.city_history(lone, 8);

    // Two cities four steps apart along a map axis, both already at radius 3.
    // The larger city's border strength wins the tile between them.
    let weak_position = (22, 34);
    let mut path = vec![weak_position];
    for _ in 0..4 {
        let previous = *path.last().unwrap();
        path.push(wrap.civ3_neighbor(previous.0, previous.1, 3).unwrap());
    }
    let weak = world.city(OWNER, path[0], 1);
    let strong = world.city(HUMAN, path[4], 8);
    world.city_history(weak, 100);
    world.city_history(strong, 100);
    world.finish();

    (
        game,
        Box::new(Audit {
            lone,
            weak,
            strong,
            weak_position,
            midpoint: path[2],
            ..Default::default()
        }),
    )
}

#[derive(Default)]
struct Audit {
    lone: u32,
    weak: u32,
    strong: u32,
    weak_position: (i32, i32),
    midpoint: (i32, i32),
    /// (completed turn, radius, tiles) for the lone capital.
    ladder: Vec<(u32, i32, usize)>,
    midpoint_held_by_stronger: Vec<bool>,
    weaker_ring_held: Vec<bool>,
}

impl ScenarioRun for Audit {
    fn before_turn(&mut self, game: &mut Simulation, completed: u32) {
        if let Some(&(_, history)) = STAGED_CULTURE.iter().find(|(turn, _)| *turn == completed) {
            // Direct fixture-style injection; the next turn's Palace point
            // carries the city across the threshold and rebuilds borders.
            game.stage_city_culture(self.lone, history);
        }
    }

    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        let (radius, tiles) = game.city_border_extent(self.lone).unwrap_or((0, 0));
        self.ladder.push((turn, radius, tiles));
        let strong_city = game.tile(self.midpoint.0, self.midpoint.1).border_city;
        self.midpoint_held_by_stronger
            .push(strong_city == Some(self.strong));
        let ring: Vec<_> = game
            .map_wrap
            .civ3_neighbors(self.weak_position.0, self.weak_position.1)
            .collect();
        let ring_held = !ring.is_empty()
            && ring
                .iter()
                .all(|&(x, y)| game.tile(x, y).border_city == Some(self.weak));
        self.weaker_ring_held.push(ring_held);
        let (_, weak_tiles) = game.city_border_extent(self.weak).unwrap_or((0, 0));
        let (_, strong_tiles) = game.city_border_extent(self.strong).unwrap_or((0, 0));
        json!({
            "lone_city": {"radius": radius, "tiles": tiles,
                "culture": game.city_by_id(self.lone).map(|c| game.city_culture_score(c))},
            "contest": {"weak_tiles": weak_tiles, "strong_tiles": strong_tiles,
                "midpoint_owner": game.tile(self.midpoint.0, self.midpoint.1).owner,
                "weaker_ring_held": ring_held},
        })
    }

    fn finish(self: Box<Self>, turns: u32) -> Vec<Check> {
        let exact = !self.ladder.is_empty()
            && self
                .ladder
                .iter()
                .all(|&(_, radius, tiles)| TILES_BY_RADIUS.get((radius - 1) as usize) == Some(&tiles));
        let radii: Vec<_> = self.ladder.iter().map(|&(_, radius, _)| radius).collect();
        let reached_every_tier = (1..=5).all(|radius| radii.contains(&radius));
        let monotonic = self
            .ladder
            .windows(2)
            .all(|pair| pair[1].2 >= pair[0].2 && pair[1].1 >= pair[0].1);
        vec![
            Check {
                name: "border_tiers_follow_culture",
                passed: exact && reached_every_tier && turns >= 8,
                detail: format!(
                    "Lone city (turn, radius, tiles) {:?}; expected tiles by radius {TILES_BY_RADIUS:?}",
                    self.ladder
                ),
            },
            Check {
                name: "border_never_shrinks_without_loss",
                passed: monotonic,
                detail: "Radius and tile count never decrease while culture only accumulates".into(),
            },
            Check {
                name: "stronger_city_wins_contested_tile",
                passed: !self.midpoint_held_by_stronger.is_empty()
                    && self.midpoint_held_by_stronger.iter().all(|held| *held),
                detail: format!(
                    "Midpoint held by the size-8 city on every observed turn: {:?}",
                    self.midpoint_held_by_stronger
                ),
            },
            Check {
                name: "weaker_city_keeps_adjacent_tiles",
                passed: !self.weaker_ring_held.is_empty()
                    && self.weaker_ring_held.iter().all(|held| *held),
                detail: format!(
                    "All eight neighbors of the size-1 city stay its own despite the stronger border: {:?}",
                    self.weaker_ring_held
                ),
            },
        ]
    }
}
