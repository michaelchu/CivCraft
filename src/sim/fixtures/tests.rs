use super::*;
use crate::sim::{Pact, UnitKind};

#[test]
fn presets_initialize_roster_knowledge_calendar_entities_and_diplomacy() {
    let mut game = WorldBuilder::ocean_campaign(
        crate::test_support::seeded_map(),
        &[
            CivPreset {
                id: "england",
                technologies: Some(&[Tech::COMBUSTION]),
            },
            CivPreset {
                id: "germany",
                technologies: Some(&[]),
            },
        ],
    );
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(120, 1900);
    world.grassland((10, 24));
    let city = world.city(0, (10, 24), 3);
    let ship = world.unit(1, UnitKind::Battleship, (10, 25));
    world.diplomacy(1, 0, Pact::Peace, 42, [3000, 5000]);
    world.finish();

    assert_eq!(game.civilization_count, 2);
    assert_eq!(game.players[0].civilization_id.as_deref(), Some("england"));
    assert_eq!(game.players[1].civilization_id.as_deref(), Some("germany"));
    assert_eq!(game.players[0].known, [Tech::COMBUSTION]);
    assert!(game.players[1].known.is_empty());
    assert_eq!(game.global_advances, [Tech::COMBUSTION]);
    assert_ne!(game.players[0].researching, Tech::COMBUSTION);
    assert_eq!((game.turn, game.year), (120, 1900));
    assert_eq!(game.city_by_id(city).unwrap().founded_turn, 120);
    assert_eq!(game.nation_scores[0].score_population, 3);
    assert_eq!(game.unit_by_id(ship).unwrap().owner, 1);
    assert_eq!(game.pact(0, 1), Pact::Peace);
    assert_eq!(game.diplomacy[&(0, 1)].trust, [5000, 3000]);
}
