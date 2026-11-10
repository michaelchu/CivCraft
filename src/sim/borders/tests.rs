use super::*;

#[test]
fn contested_border_claims_follow_civilization_culture() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(game.city_borders_playtest(), (true, true, true));
}
