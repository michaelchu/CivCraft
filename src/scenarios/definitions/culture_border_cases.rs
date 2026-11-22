//! Case studies for the Civ III border and land-claim rules: how two or three
//! borders share a tile, what a border may claim, and what happens to a border
//! when its city is lost. Every scenario also re-derives the whole map's
//! ownership from the rule and compares it with the simulation on each turn.

use super::*;
use crate::{
    sim::{
        HUMAN, Simulation,
        drive::Drive,
        fixtures::{CivPreset, TileSurface, WorldBuilder},
        lifecycle::CityRemoval,
    },
    world::{GeneratedMap, Terrain, topology::MapWrap},
};
use serde_json::{Value, json};

const KOREA: usize = 1;
const EGYPT: usize = 2;

pub(super) const CONTESTS: Scenario = Scenario::new(
    "border-contests",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: setup_contests,
    },
);

pub(super) const LAND_CLAIMS: Scenario = Scenario::new(
    "border-land-claims",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: setup_land_claims,
    },
);

pub(super) const CITY_LOSS: Scenario = Scenario::new(
    "border-city-loss",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: setup_city_loss,
    },
);

/// Flat grassland with three civilizations. The first city of each owner gets
/// the Palace and so gains culture every turn; each scenario therefore puts
/// the owners' capitals out of the way and stages culture on later cities.
fn prepared(map: GeneratedMap, all_known: bool) -> Simulation {
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &["rome", "korea", "egypt"].map(|id| CivPreset {
            id,
            technologies: None,
        }),
    );
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(80, -1000);
    world.map_surface(TileSurface {
        terrain: Some(Terrain::Grassland),
        ..Default::default()
    });
    if all_known {
        for owner in [HUMAN, KOREA, EGYPT] {
            world.fertile_tiles(None, Some(owner));
        }
    }
    world.finish();
    game
}

fn capitals(world: &mut WorldBuilder<'_>) {
    for (owner, x) in [(HUMAN, 6), (KOREA, 16), (EGYPT, 26)] {
        world.city(owner, (x, 44), 3);
    }
}

/// The straight run of tiles `steps` long from `from` along a map axis
/// (direction 3 is south-east, 5 south-west).
fn axis(wrap: MapWrap, from: (i32, i32), direction: usize, steps: usize) -> Vec<(i32, i32)> {
    let mut path = vec![from];
    for _ in 0..steps {
        let previous = *path.last().unwrap();
        path.push(
            wrap.eight_way_neighbor(previous.0, previous.1, direction)
                .expect("scenario path stays on the map"),
        );
    }
    path
}

/// Re-derive every tile's owner from the Civ III rule alone and compare: a
/// tile goes to the nearest city (by culture distance) whose border shape
/// reaches it, then to the one with more culture, then to the older city. A
/// city's own tile is always its own, and no other city may take it.
fn rule_holds(game: &Simulation) -> bool {
    let wrap = game.map_wrap;
    let cities = game.cities();
    let radii: Vec<_> = cities
        .iter()
        .map(|city| {
            game.city_border_extent(city.id)
                .map_or(1, |(radius, _)| radius)
        })
        .collect();
    for y in 0..wrap.height() {
        for x in 0..wrap.width() {
            let occupied = cities.iter().any(|city| (city.x, city.y) == (x, y));
            let mut best: Option<((i32, i64, u32), u32)> = None;
            for (city, &radius) in cities.iter().zip(&radii) {
                if !wrap.border_shape_contains((city.x, city.y), (x, y), radius) {
                    continue;
                }
                let is_center = (city.x, city.y) == (x, y);
                if occupied && !is_center {
                    continue;
                }
                let rank = if is_center {
                    -1
                } else {
                    wrap.culture_rank_distance(city.x, city.y, x, y)
                };
                let key = (rank, -game.city_culture_score(city), city.id);
                if best.is_none_or(|(current, _)| key < current) {
                    best = Some((key, city.id));
                }
            }
            if game.tile(x, y).border_city != best.map(|(_, id)| id) {
                return false;
            }
        }
    }
    true
}

fn border_city(game: &Simulation, tile: (i32, i32)) -> Option<u32> {
    game.tile(tile.0, tile.1).border_city
}

fn all_true(values: &[bool]) -> bool {
    !values.is_empty() && values.iter().all(|value| *value)
}

// ---------------------------------------------------------------------------
// border-contests
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct Pair {
    a: u32,
    b: u32,
    path: Vec<(i32, i32)>,
}

fn pair(
    world: &mut WorldBuilder<'_>,
    path: Vec<(i32, i32)>,
    owners: (usize, usize),
    sizes: (i32, i32),
    culture: (i32, i32),
) -> Pair {
    let last = path.len() - 1;
    let a = world.city(owners.0, path[0], sizes.0);
    let b = world.city(owners.1, path[last], sizes.1);
    world.city_history(a, culture.0);
    world.city_history(b, culture.1);
    Pair { a, b, path }
}

fn setup_contests(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = prepared(map, true);
    let wrap = game.map_wrap;
    let mut world = WorldBuilder::new(&mut game);
    capitals(&mut world);

    // A size-12 and a size-1 city five steps apart: the nearer city wins each
    // contested tile, whatever the sizes.
    let size = pair(
        &mut world,
        axis(wrap, (4, 10), 3, 5),
        (HUMAN, KOREA),
        (12, 1),
        (1_000, 1_000),
    );
    // Four steps apart with 200 against 100 culture: the middle tile follows
    // culture, and flips back and forth as culture is staged.
    let flip = pair(
        &mut world,
        axis(wrap, (15, 10), 3, 4),
        (HUMAN, KOREA),
        (3, 3),
        (200, 100),
    );
    // Equal culture: the older city keeps the middle tile, whoever owns it.
    let older_human = pair(
        &mut world,
        axis(wrap, (26, 10), 3, 4),
        (HUMAN, KOREA),
        (3, 3),
        (300, 300),
    );
    let older_korea = {
        let path = axis(wrap, (4, 26), 3, 4);
        let last = path.len() - 1;
        let b = world.city(KOREA, path[0], 3);
        let a = world.city(HUMAN, path[last], 3);
        world.city_history(a, 300);
        world.city_history(b, 300);
        Pair { a: b, b: a, path }
    };
    // Three civilizations meeting at one junction, equal culture.
    let origin = (15, 24);
    let arm_b = axis(wrap, origin, 3, 4);
    let arm_c = axis(wrap, origin, 5, 4);
    let triangle = [
        world.city(HUMAN, origin, 3),
        world.city(KOREA, arm_b[4], 3),
        world.city(EGYPT, arm_c[4], 3),
    ];
    for id in triangle {
        world.city_history(id, 300);
    }
    // Two cities only two steps apart at full radius: each keeps its centre.
    let adjacent = pair(
        &mut world,
        axis(wrap, (26, 28), 3, 2),
        (HUMAN, KOREA),
        (3, 3),
        (1_000, 1_000),
    );
    world.settle_borders();
    world.finish();

    (
        game,
        Box::new(ContestAudit {
            size,
            flip,
            older_human,
            older_korea,
            triangle,
            adjacent,
            ..Default::default()
        }),
    )
}

#[derive(Default)]
struct ContestAudit {
    size: Pair,
    flip: Pair,
    older_human: Pair,
    older_korea: Pair,
    triangle: [u32; 3],
    adjacent: Pair,
    rule: Vec<bool>,
    nearer_wins: Vec<bool>,
    flips: Vec<(u32, Option<u32>)>,
    older: Vec<bool>,
    junction: Vec<bool>,
    centres: Vec<bool>,
    culture_exact: Vec<bool>,
}

impl ContestAudit {
    /// Culture staged on the flip pair before the given completed turn.
    fn flip_culture(turn: u32) -> (i32, i32) {
        match turn {
            ..=2 => (200, 100),
            3..=4 => (200, 600),
            _ => (700, 600),
        }
    }
}

impl ScenarioRun for ContestAudit {
    fn before_turn(&mut self, game: &mut Simulation, completed: u32) {
        match completed {
            2 => game.stage_city_culture(self.flip.b, 600),
            4 => game.stage_city_culture(self.flip.a, 700),
            _ => {}
        }
    }

    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        self.rule.push(rule_holds(game));

        let size = &self.size;
        self.nearer_wins.push(
            border_city(game, size.path[2]) == Some(size.a)
                && border_city(game, size.path[3]) == Some(size.b),
        );

        let flip_middle = border_city(game, self.flip.path[2]);
        self.flips.push((turn, flip_middle));

        let older = [&self.older_human, &self.older_korea]
            .iter()
            .all(|pair| border_city(game, pair.path[2]) == Some(pair.a));
        self.older.push(older);

        self.junction.push(self.triangle.iter().all(|&id| {
            game.city_border_extent(id)
                .is_some_and(|(_, tiles)| tiles > 9)
        }));

        let adjacent = &self.adjacent;
        self.centres.push(
            border_city(game, adjacent.path[0]) == Some(adjacent.a)
                && border_city(game, adjacent.path[2]) == Some(adjacent.b)
                && border_city(game, adjacent.path[1]) == Some(adjacent.a),
        );

        let culture = |id: u32| {
            game.city_by_id(id)
                .map(|city| game.city_culture_score(city))
        };
        let staged = [
            (self.size.a, 1_000),
            (self.size.b, 1_000),
            (self.older_human.a, 300),
            (self.older_human.b, 300),
            (self.older_korea.a, 300),
            (self.older_korea.b, 300),
            (self.adjacent.a, 1_000),
            (self.adjacent.b, 1_000),
        ];
        let (seen_a, seen_b) = Self::flip_culture(turn);
        self.culture_exact.push(
            staged
                .iter()
                .all(|&(id, expected)| culture(id) == Some(expected))
                && culture(self.flip.a) == Some(i64::from(seen_a))
                && culture(self.flip.b) == Some(i64::from(seen_b)),
        );

        json!({
            "nearer_wins": self.nearer_wins.last(),
            "flip_middle_city": flip_middle,
            "older_wins": older,
            "rule_holds": self.rule.last(),
        })
    }

    fn finish(self: Box<Self>, turns: u32) -> Vec<Check> {
        let expected_flips: Vec<_> = self
            .flips
            .iter()
            .map(|&(turn, _)| {
                let (a, b) = Self::flip_culture(turn);
                (turn, Some(if b > a { self.flip.b } else { self.flip.a }))
            })
            .collect();
        vec![
            Check {
                name: "contest_rule_holds_for_every_tile",
                passed: all_true(&self.rule) && turns >= 6,
                detail: "Every tile on the map belongs to the nearest city whose border reaches it, then the city with more culture, then the older city".into(),
            },
            Check {
                name: "nearer_city_wins_regardless_of_size",
                passed: all_true(&self.nearer_wins),
                detail: "A size-1 city keeps the tile two steps from it against a size-12 city three steps away, and loses the tile it is three steps from".into(),
            },
            Check {
                name: "culture_decides_equidistant_tile_and_flips",
                passed: self.flips == expected_flips && turns >= 6,
                detail: format!(
                    "Middle tile by turn (city id) {:?}; expected {:?}",
                    self.flips, expected_flips
                ),
            },
            Check {
                name: "equal_culture_goes_to_older_city",
                passed: all_true(&self.older),
                detail: "The older city keeps the middle tile at equal culture, whether it is the human's or the AI's".into(),
            },
            Check {
                name: "three_city_junction_follows_nearest",
                passed: all_true(&self.junction),
                detail: "Each of three civilizations' cities holds more than its own square where their borders meet".into(),
            },
            Check {
                name: "adjacent_cities_keep_their_centres",
                passed: all_true(&self.centres),
                detail: "Two cities two steps apart each keep their own tile; the tile between them goes to the older city".into(),
            },
            Check {
                name: "staged_culture_stays_exact",
                passed: all_true(&self.culture_exact),
                detail: "No Palace or other culture source disturbs the staged values".into(),
            },
        ]
    }
}

// ---------------------------------------------------------------------------
// border-land-claims
// ---------------------------------------------------------------------------

fn set_ocean(
    game: &mut Simulation,
    x_range: std::ops::RangeInclusive<i32>,
    y_range: std::ops::RangeInclusive<i32>,
    keep: &[(i32, i32)],
) {
    let mut world = WorldBuilder::new(game);
    for y in y_range {
        for x in x_range.clone() {
            if !keep.contains(&(x, y)) {
                world.terrain((x, y), Terrain::Ocean);
            }
        }
    }
}

fn setup_land_claims(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    // Nothing is known at the start, so claims can be seen to reveal terrain.
    let mut game = prepared(map, false);
    let wrap = game.map_wrap;
    let coast = (6, 14);
    let island = (20, 14);
    let unseen = (14, 28);
    let seam = (0, 28);
    let pole = (24, 3);
    set_ocean(&mut game, 7..=13, 8..=20, &[]);
    set_ocean(&mut game, 15..=25, 8..=20, &[island]);
    let mut world = WorldBuilder::new(&mut game);
    capitals(&mut world);
    let ids = [coast, island, unseen, seam, pole].map(|position| world.city(HUMAN, position, 3));
    // The unseen-tile city starts one point short of its radius-3 expansion.
    for (id, culture) in ids.into_iter().zip([100, 10, 99, 100, 100]) {
        world.city_history(id, culture);
    }
    world.settle_borders();
    world.finish();
    let far_corner = axis(wrap, unseen, 3, 3)[3];
    let unknown_before = !game.tile(far_corner.0, far_corner.1).known_by[HUMAN];
    (
        game,
        Box::new(LandAudit {
            ids,
            far_corner,
            unknown_before,
            ..Default::default()
        }),
    )
}

#[derive(Default)]
struct LandAudit {
    ids: [u32; 5],
    far_corner: (i32, i32),
    unknown_before: bool,
    rule: Vec<bool>,
    coastal: Vec<bool>,
    island: Vec<bool>,
    unseen: Vec<bool>,
    seam: Vec<bool>,
    pole: Vec<bool>,
}

/// Map tiles inside a city's border shape, with wrapping applied and tiles
/// beyond the poles dropped.
fn shape_tiles(game: &Simulation, center: (i32, i32), radius: i32) -> Vec<(i32, i32)> {
    let wrap = game.map_wrap;
    let mut tiles = Vec::new();
    for y in 0..wrap.height() {
        for x in 0..wrap.width() {
            if wrap.border_shape_contains(center, (x, y), radius) {
                tiles.push((x, y));
            }
        }
    }
    tiles
}

impl ScenarioRun for LandAudit {
    fn before_turn(&mut self, game: &mut Simulation, completed: u32) {
        if completed == 0 {
            // Cross the threshold on the first turn: the new border claims
            // tiles beyond the city's sight and reveals them to its owner.
            game.stage_city_culture(self.ids[2], 100);
        }
    }

    fn observe_turn(&mut self, game: &Simulation, _turn: u32) -> Value {
        self.rule.push(rule_holds(game));
        let city = |index: usize| game.city_by_id(self.ids[index]).map(|c| (c.x, c.y));
        let claimed = |index: usize, radius: i32| -> Option<(Vec<(i32, i32)>, bool)> {
            let center = city(index)?;
            let tiles = shape_tiles(game, center, radius);
            let all = tiles
                .iter()
                .all(|&tile| border_city(game, tile) == Some(self.ids[index]));
            Some((tiles, all))
        };

        let coastal = claimed(0, 3).is_some_and(|(tiles, all)| {
            let water = tiles
                .iter()
                .filter(|&&(x, y)| game.tile(x, y).terrain.is_water())
                .count();
            all && water > 0 && water < tiles.len()
        });
        self.coastal.push(coastal);

        let island = claimed(1, 2).is_some_and(|(tiles, all)| {
            all && tiles.len() == 21
                && tiles
                    .iter()
                    .filter(|&&(x, y)| game.tile(x, y).terrain.is_water())
                    .count()
                    == 20
        });
        self.island.push(island);

        let unseen = claimed(2, 3).is_some_and(|(tiles, all)| {
            all && tiles.iter().all(|&(x, y)| game.tile(x, y).known_by[HUMAN])
                && border_city(game, self.far_corner) == Some(self.ids[2])
        });
        self.unseen.push(unseen);

        let seam = claimed(3, 3).is_some_and(|(tiles, all)| {
            all && tiles.len() == 45
                && tiles.iter().any(|&(x, _)| x >= game.map_wrap.width() - 3)
                && tiles.iter().any(|&(x, _)| x <= 2)
        });
        self.seam.push(seam);

        let pole =
            claimed(4, 3).is_some_and(|(tiles, all)| all && !tiles.is_empty() && tiles.len() < 45);
        self.pole.push(pole);

        json!({"coastal": coastal, "island": island, "unseen": unseen,
               "seam": seam, "pole": pole, "rule_holds": self.rule.last()})
    }

    fn finish(self: Box<Self>, turns: u32) -> Vec<Check> {
        vec![
            Check {
                name: "claims_cover_exactly_the_shape_everywhere",
                passed: all_true(&self.rule) && turns >= 2,
                detail: "Every claimed tile is in a border shape, and every uncontested shape tile is claimed".into(),
            },
            Check {
                name: "coastal_border_claims_land_and_sea",
                passed: all_true(&self.coastal),
                detail: "A radius-3 coastal city holds all 45 shape tiles, ocean included".into(),
            },
            Check {
                name: "island_border_is_all_sea",
                passed: all_true(&self.island),
                detail: "A city on a one-tile island holds its 21-tile fat X: itself and 20 ocean tiles".into(),
            },
            Check {
                name: "unseen_tiles_are_claimed_and_revealed",
                passed: self.unknown_before && all_true(&self.unseen),
                detail: format!(
                    "The tile three steps out at {:?} was unknown before the expansion ({}); once culture reaches 100 every shape tile is claimed and known",
                    self.far_corner, self.unknown_before
                ),
            },
            Check {
                name: "border_wraps_across_the_map_seam",
                passed: all_true(&self.seam),
                detail: "A city at the map's west edge holds all 45 tiles, including tiles at its east edge".into(),
            },
            Check {
                name: "polar_border_is_clipped_not_shifted",
                passed: all_true(&self.pole),
                detail: "A city near the north pole holds only the shape tiles that are on the map".into(),
            },
        ]
    }
}

// ---------------------------------------------------------------------------
// border-city-loss
// ---------------------------------------------------------------------------

fn setup_city_loss(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = prepared(map, true);
    let wrap = game.map_wrap;
    let mut world = WorldBuilder::new(&mut game);
    capitals(&mut world);
    // Both pairs: the human city has more culture and owns the middle tile.
    let destroyed = pair(
        &mut world,
        axis(wrap, (6, 14), 3, 4),
        (HUMAN, KOREA),
        (3, 3),
        (300, 200),
    );
    let captured = pair(
        &mut world,
        axis(wrap, (22, 14), 3, 4),
        (HUMAN, KOREA),
        (3, 3),
        (300, 200),
    );
    world.settle_borders();
    world.finish();
    (
        game,
        Box::new(LossAudit {
            destroyed,
            captured,
            ..Default::default()
        }),
    )
}

#[derive(Default)]
struct LossAudit {
    destroyed: Pair,
    captured: Pair,
    rule: Vec<bool>,
    middle_before: Vec<bool>,
    destroyed_released: Vec<bool>,
    middle_inherited: Vec<bool>,
    captured_owner: Vec<bool>,
    captured_square: Vec<bool>,
}

const LOSS_TURN: u32 = 3;

impl ScenarioRun for LossAudit {
    fn before_turn(&mut self, game: &mut Simulation, completed: u32) {
        if completed + 1 != LOSS_TURN {
            return;
        }
        game.remove_city_because(self.destroyed.a, CityRemoval::Disaster, None);
        if let Some(index) = game.city_index_by_id(self.captured.a) {
            game.transfer_traded_city(index, KOREA);
        }
    }

    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        self.rule.push(rule_holds(game));
        let before = turn < LOSS_TURN;
        let destroyed = &self.destroyed;
        let captured = &self.captured;
        let owner = |tile: (i32, i32)| game.tile(tile.0, tile.1).owner;
        if before {
            self.middle_before.push(
                border_city(game, destroyed.path[2]) == Some(destroyed.a)
                    && border_city(game, captured.path[2]) == Some(captured.a),
            );
        } else {
            // The destroyed city's own tile is nobody's (the other border is
            // four steps away) and the middle tile passes to the other city.
            self.destroyed_released.push(
                game.city_by_id(destroyed.a).is_none()
                    && border_city(game, destroyed.path[0]).is_none()
                    && owner(destroyed.path[0]).is_none(),
            );
            self.middle_inherited
                .push(border_city(game, destroyed.path[2]) == Some(destroyed.b));
            // The captured city keeps its tiles but they now belong to Korea.
            self.captured_owner.push(
                owner(captured.path[0]) == Some(KOREA)
                    && border_city(game, captured.path[0]) == Some(captured.a),
            );
            // Fresh culture under its new owner: back to the 3×3 square.
            self.captured_square
                .push(game.city_border_extent(captured.a) == Some((1, 9)));
        }
        json!({"rule_holds": self.rule.last(), "before_loss": before,
               "destroyed_middle": border_city(game, destroyed.path[2]),
               "captured_owner": owner(captured.path[0])})
    }

    fn finish(self: Box<Self>, turns: u32) -> Vec<Check> {
        vec![
            Check {
                name: "rule_holds_after_every_change",
                passed: all_true(&self.rule) && turns > LOSS_TURN,
                detail: "Ownership matches the rule before and after a city is destroyed or captured".into(),
            },
            Check {
                name: "human_city_owns_middle_before_loss",
                passed: all_true(&self.middle_before),
                detail: "With 300 against 200 culture the human city holds the equidistant tile".into(),
            },
            Check {
                name: "destroyed_city_releases_its_tiles",
                passed: all_true(&self.destroyed_released) && all_true(&self.middle_inherited),
                detail: "The destroyed city's own tile becomes unowned and the middle tile passes to the other city".into(),
            },
            Check {
                name: "captured_city_shrinks_to_its_square",
                passed: all_true(&self.captured_square),
                detail: "A captured city starts fresh culture, so its border falls back to the radius-1 square of nine tiles".into(),
            },
            Check {
                name: "captured_city_keeps_tiles_for_new_owner",
                passed: all_true(&self.captured_owner),
                detail: "A captured city's tiles keep its border but belong to the capturing civilization".into(),
            },
        ]
    }
}
