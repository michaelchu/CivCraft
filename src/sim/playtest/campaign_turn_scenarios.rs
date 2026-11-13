//! Cross-system deterministic scenarios for full campaign turns.

use super::*;
use crate::world::Terrain;

#[cfg(test)]
mod tests {
    use super::*;

    fn route_turn_game(roads_connected: bool) -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.trade_routes.clear();
        game.diplomacy.clear();
        game.diplomatic_contact_until.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.owner = None;
            tile.border_city = None;
            tile.road = roads_connected;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.oil_well = false;
            tile.oil_platform = false;
            tile.river = false;
            tile.pollution = false;
            tile.fallout = false;
        }
        game.create_city(HUMAN, 5, 20);
        game.create_city(1, 20, 20);
        game.players[HUMAN].gold = 0;
        game.players[HUMAN].rates = [0, 100, 0];
        game.set_random_seed(0xC1_7A_13);
        game
    }

    #[test]
    fn connected_trade_income_flows_through_city_processing_and_score_boundary() {
        let mut connected = route_turn_game(true);
        let mut disconnected = route_turn_game(false);
        let connected_start_gold = connected.players[HUMAN].gold;
        let disconnected_start_gold = disconnected.players[HUMAN].gold;
        let connected_start_turn = connected.turn;

        // Freeciv's actor-free adapter forms connected routes before city
        // activity; route trade must therefore reach tax income in this same
        // end_turn. Compare the same seeded campaign with only its road
        // network changed.
        connected.end_turn();
        disconnected.end_turn();

        assert_eq!(connected.trade_routes.len(), 1);
        assert!(disconnected.trade_routes.is_empty());
        assert!(connected.players[HUMAN].gold > connected_start_gold);
        assert!(
            connected.players[HUMAN].gold - connected_start_gold
                > disconnected.players[HUMAN].gold - disconnected_start_gold,
            "connected route trade should increase same-turn tax revenue"
        );

        // The score snapshot is refreshed at the next begin-turn boundary,
        // after city growth and end-of-turn world events have resolved.
        assert_eq!(connected.turn, connected_start_turn + 1);
        assert_eq!(
            connected.nation_scores[HUMAN].score_population,
            connected
                .cities_owned_by(HUMAN)
                .map(|city| city.size)
                .sum::<i32>()
        );
    }
}
