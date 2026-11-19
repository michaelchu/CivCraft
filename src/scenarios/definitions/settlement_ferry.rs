//! Production and transport phases build a ferry, then a founder, then settle.
use super::{Checks, Scenario};
use crate::{
    rules::Tech,
    sim::{
        Production, Simulation, UnitKind,
        fixtures::{TileSurface, WorldBuilder},
    },
    world::{Resource, Terrain, topology::offset_native_by_map},
};
pub(super) const SCENARIO: Scenario = Scenario::all_ai("settlement-ferry", execute);

/// A productive coastal home and a known overseas island.
fn setup(game: &mut Simulation) -> (u32, (i32, i32)) {
    let map = game.map_wrap;
    let mut world = WorldBuilder::new(game);
    world.clear_units();
    world.completed_great_wonders();
    world.map_surface(TileSurface {
        terrain: Some(Terrain::Sea),
        resource: Some(None),
        hut: Some(false),
        ..Default::default()
    });
    let home = (10, 24);
    let island = offset_native_by_map(home.0, home.1, 6, 0);
    world.terrain(home, Terrain::Forest);
    world.terrain(island, Terrain::Grassland);
    let productive = offset_native_by_map(island.0, island.1, 1, 0);
    world.terrain(productive, Terrain::Grassland);
    world.mark_known(1, productive);
    world.mark_known(1, island);
    let fertile = TileSurface {
        terrain: Some(Terrain::Grassland),
        resource: Some(Some(Resource::Wheat)),
        irrigation: Some(true),
        river: Some(true),
        ..Default::default()
    };
    for (_, _, x, y) in map.city_radius(island.0, island.1) {
        world.tile_surface((x, y), fertile);
        world.mark_known(1, (x, y));
    }
    for (_, _, x, y) in map.city_radius(home.0, home.1) {
        if (x, y) != home {
            world.tile_surface((x, y), fertile);
        }
    }
    let city = world.founding_city_on(1, home, Terrain::Forest);
    world.city_population(city, 3, None);
    world.city_food(city, 20);
    game.reconcile_city_workers();
    let worked = game.city_by_id(city).unwrap().worked_tiles.clone();
    let mut world = WorldBuilder::new(game);
    for (_, _, x, y) in map.city_radius(home.0, home.1) {
        if (x, y) == home || worked.contains(&(x, y)) {
            continue;
        }
        world.tile_surface(
            (x, y),
            TileSurface {
                terrain: Some(Terrain::Sea),
                resource: Some(None),
                irrigation: Some(false),
                river: Some(false),
                ..Default::default()
            },
        );
    }
    game.reconcile_city_workers();
    WorldBuilder::new(game).unit(1, UnitKind::Warrior, home);
    (city, island)
}

fn execute(game: &mut Simulation, checks: &mut Checks) {
    let (city, island) = setup(game);
    let before = game.choose_ai_production(1, city);
    WorldBuilder::new(game).add_technology(1, Tech::MAP_MAKING);
    let production = game.choose_ai_production(1, city);
    let boat = Production::Unit(UnitKind::Galley);
    let queued = before != boat && production == boat;
    let mut sequenced = false;
    let mut founded = false;
    if queued {
        WorldBuilder::new(game).production_stock(city, production, production.cost());
        game.process_cities();
        let settler = Production::Unit(UnitKind::Settler);
        sequenced = game
            .units()
            .iter()
            .any(|unit| unit.kind == UnitKind::Galley)
            && game.cities()[0].production == settler;
        if sequenced {
            WorldBuilder::new(game).production_stock(city, settler, settler.cost());
            game.process_cities();
            for _ in 0..8 {
                game.update_activities_and_restore();
                game.run_ai_player(1);
                if game.cities().len() > 1 {
                    break;
                }
            }
            founded = game.cities().iter().any(|city| {
                city.owner == 1
                    && game
                        .map_wrap
                        .civ3_distance(city.x, city.y, island.0, island.1)
                        <= 2
            });
        }
    }
    checks.extend([
        (
            "ferry_queued",
            queued,
            "AI selected a ferry for an overseas settlement plan".into(),
        ),
        (
            "settler_sequenced",
            sequenced,
            "AI sequenced a Settler after completing the ferry".into(),
        ),
        (
            "island_founded",
            founded,
            "AI transported its founder and established the island city".into(),
        ),
    ]);
}
