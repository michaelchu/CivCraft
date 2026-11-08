//! Regression tests for the behavior of the parent module.

use super::*;
use crate::rules::Tech;

#[test]
fn baseline_clones_preserve_rng_and_normal_campaign_results() {
    let initial = Simulation::new(crate::mapgen::generate(42));
    let original = bincode::serialize(&initial).unwrap();
    let mut a = initial.clone();
    let mut b = initial.clone();
    let founder = initial
        .units
        .iter()
        .find(|u| u.owner == HUMAN && u.kind == UnitKind::Settlers)
        .unwrap()
        .id;
    assert!(a.apply(Command::FoundCity(founder)));
    assert!(b.apply(Command::FoundCity(founder)));
    assert!(a.apply(Command::EndTurn));
    assert!(b.apply(Command::EndTurn));
    assert_eq!(
        bincode::serialize(&a).unwrap(),
        bincode::serialize(&b).unwrap()
    );
    for _ in 0..32 {
        assert_eq!(a.rand_below(1_000_000), b.rand_below(1_000_000));
    }
    assert_eq!(bincode::serialize(&initial).unwrap(), original);
}

#[test]
fn resource_shadow_review_scene_contains_every_civ3_resource() {
    use Resource::*;

    let mut sim = Simulation::new(crate::mapgen::generate(42));
    sim.players[HUMAN].known = Tech::all().collect();
    sim.ui_review_resource_shadows();
    let placed: Vec<_> = sim
        .tiles
        .iter()
        .filter_map(|tile| tile.active_resource())
        .collect();
    let expected = [
        Aluminum,
        Coal,
        Horses,
        Iron,
        Oil,
        Rubber,
        Saltpeter,
        Uranium,
        Dyes,
        Furs,
        Gems,
        Incense,
        Ivory,
        Silk,
        Spice,
        Wine,
        Cattle,
        Fish,
        Gold,
        Game,
        Oasis,
        Sugar,
        Tobacco,
        TropicalFruit,
        Whales,
        Wheat,
    ];
    assert_eq!(placed.len(), expected.len());
    for resource in expected {
        assert!(placed.contains(&resource), "{resource:?}");
        assert!(sim.resource_visible_to(HUMAN, resource), "{resource:?}");
    }
    for resource in [Fish, Whales] {
        let tile = sim
            .tiles
            .iter()
            .find(|tile| tile.resource == Some(resource))
            .unwrap();
        assert_eq!(tile.terrain, Terrain::Ocean, "{resource:?}");
    }
}
