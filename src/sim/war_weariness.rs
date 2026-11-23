//! Civ III war weariness.
//!
//! The Conquests Civilopedia says citizens under a Republic, Feudalism or
//! Democracy grow unhappy during war; a nation that starts the conflict
//! tolerates it for a while and then suffers more, a nation that is attacked
//! is more forgiving, and only ending the war removes it. The per-government
//! rating comes from the BIQ (`NativeGovernment::war_weariness`). The Pedia
//! gives no numbers, so the pacing below is provisional and lives in one
//! place for tuning. Police Stations and Universal Suffrage reduce it.

use super::*;

/// Turns of war a nation that declared it tolerates before weariness builds.
const AGGRESSOR_GRACE_TURNS: i32 = 5;
/// Turns of war a nation that was attacked tolerates before weariness builds.
const DEFENDER_GRACE_TURNS: i32 = 10;
/// Weariness points per turn per rating point when the nation declared the war.
const AGGRESSOR_RATE: i32 = 2;
/// Weariness points per turn per rating point when the nation was attacked.
const DEFENDER_RATE: i32 = 1;
/// Points at which every citizen of an unprotected city is affected.
const FULL_UNREST_POINTS: i32 = 40;
/// Percentage a Police Station removes from its own city's weariness.
const POLICE_STATION_REDUCTION_PCT: i32 = 50;
/// Percentage Universal Suffrage removes from every city of its owner.
const UNIVERSAL_SUFFRAGE_REDUCTION_PCT: i32 = 50;

impl Simulation {
    /// Weariness points of a nation: its worst current war, before any
    /// building reduction. Zero in peace and under ratings of zero.
    pub(crate) fn war_weariness_points(&self, owner: usize) -> i32 {
        let rating = self.players[owner].government.war_weariness();
        if rating <= 0 {
            return 0;
        }
        self.diplomacy
            .iter()
            .filter(|((a, b), relation)| relation.pact == Pact::War && (*a == owner || *b == owner))
            .filter_map(|(_, relation)| {
                let started = relation.war_since?;
                let aggressor = relation.war_aggressor == Some(owner);
                let (grace, rate) = if aggressor {
                    (AGGRESSOR_GRACE_TURNS, AGGRESSOR_RATE)
                } else {
                    (DEFENDER_GRACE_TURNS, DEFENDER_RATE)
                };
                Some((self.turn - started - grace).max(0) * rate * rating)
            })
            .max()
            .unwrap_or(0)
    }

    /// Citizens of `city` made unhappier by war weariness.
    pub(crate) fn city_war_weariness(&self, city: &City) -> i32 {
        let points = self.war_weariness_points(city.owner);
        if points <= 0 {
            return 0;
        }
        let mut reduction = 0;
        if city.has_building(Building::POLICE_STATION) {
            reduction += POLICE_STATION_REDUCTION_PCT;
        }
        if self.has_active_wonder(city.owner, Building::UNIVERSAL_SUFFRAGE) {
            reduction += UNIVERSAL_SUFFRAGE_REDUCTION_PCT;
        }
        let points = points.min(FULL_UNREST_POINTS) * (100 - reduction).max(0) / 100;
        // Round up so that any remaining weariness is felt by a citizen.
        (city.size * points + FULL_UNREST_POINTS - 1) / FULL_UNREST_POINTS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_war(game: &mut Simulation, aggressor: Option<usize>, since: i32) {
        let relation = game.diplomacy.entry((HUMAN, 1)).or_default();
        relation.pact = Pact::War;
        relation.war_since = Some(since);
        relation.war_aggressor = aggressor;
    }

    fn game_with_city(government: Government) -> Simulation {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        game.players[HUMAN].government = government;
        game.create_city(HUMAN, 10, 24);
        game.cities[0].size = 8;
        game.cities[0].buildings.clear();
        game
    }

    fn human_city(game: &Simulation) -> City {
        game.cities
            .iter()
            .find(|city| city.owner == HUMAN)
            .cloned()
            .unwrap()
    }

    #[test]
    fn governments_carry_the_biq_rating() {
        assert_eq!(Government::REPUBLIC.war_weariness(), 1);
        assert_eq!(Government::DEMOCRACY.war_weariness(), 2);
        assert_eq!(Government::FEUDALISM.war_weariness(), 1);
        assert_eq!(Government::MONARCHY.war_weariness(), 0);
        assert_eq!(Government::FASCISM.war_weariness(), 0);
    }

    #[test]
    fn aggressor_tolerates_then_suffers_more_than_a_defender() {
        let mut game = game_with_city(Government::DEMOCRACY);
        game.turn = 20;
        at_war(&mut game, Some(HUMAN), 10);
        let attacker = game.war_weariness_points(HUMAN);
        at_war(&mut game, Some(1), 10);
        let defender = game.war_weariness_points(HUMAN);
        assert_eq!(attacker, (10 - AGGRESSOR_GRACE_TURNS) * AGGRESSOR_RATE * 2);
        assert_eq!(defender, 0);
        game.turn = 30;
        assert!(game.war_weariness_points(HUMAN) > 0);
        at_war(&mut game, Some(HUMAN), 25);
        assert_eq!(
            game.war_weariness_points(HUMAN),
            0,
            "inside the grace period"
        );
    }

    #[test]
    fn rating_zero_governments_and_peace_feel_nothing() {
        let mut game = game_with_city(Government::MONARCHY);
        game.turn = 60;
        at_war(&mut game, Some(HUMAN), 1);
        assert_eq!(game.war_weariness_points(HUMAN), 0);
        game.players[HUMAN].government = Government::DEMOCRACY;
        assert!(game.war_weariness_points(HUMAN) > 0);
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
        assert_eq!(game.war_weariness_points(HUMAN), 0);
    }

    #[test]
    fn police_station_and_universal_suffrage_reduce_unrest() {
        let mut game = game_with_city(Government::DEMOCRACY);
        game.turn = 30;
        at_war(&mut game, Some(HUMAN), 1);
        let bare = game.city_war_weariness(&human_city(&game));
        assert_eq!(bare, 8, "40+ points affect every citizen");
        let index = game.cities.iter().position(|c| c.owner == HUMAN).unwrap();
        game.cities[index].buildings.push(Building::POLICE_STATION);
        let policed = game.city_war_weariness(&human_city(&game));
        assert_eq!(policed, 4);
        game.cities[index]
            .buildings
            .push(Building::UNIVERSAL_SUFFRAGE);
        assert_eq!(game.city_war_weariness(&human_city(&game)), 0);
    }

    #[test]
    fn declaring_war_records_start_turn_and_declarer() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.players[HUMAN].government = Government::DESPOTISM;
        game.set_scenario_diplomacy(HUMAN, 1, Pact::Peace, 0, [0, 0]);
        game.turn = 7;
        assert!(game.cancel_pact(HUMAN, 1));
        let relation = &game.diplomacy[&(HUMAN, 1)];
        assert_eq!(relation.pact, Pact::War);
        assert_eq!(
            (relation.war_since, relation.war_aggressor),
            (Some(7), Some(HUMAN))
        );
    }

    /// A war that resumes when a ceasefire runs out is still a war: it must
    /// have a start turn, or weariness silently never applies to it.
    #[test]
    fn war_resuming_after_ceasefire_expiry_still_wears_citizens_down() {
        let mut game = game_with_city(Government::DEMOCRACY);
        game.turn = 1;
        at_war(&mut game, Some(HUMAN), 1);
        game.sign_pact(HUMAN, 1, Pact::Ceasefire);
        assert_eq!(
            game.war_weariness_points(HUMAN),
            0,
            "no weariness in a ceasefire"
        );
        let until = game.diplomacy[&(HUMAN, 1)].until;
        game.turn = until;
        game.advance_diplomacy();
        assert_eq!(game.pact(HUMAN, 1), Pact::War, "the ceasefire expired");
        let resumed = game.diplomacy[&(HUMAN, 1)].war_since;
        assert_eq!(
            resumed,
            Some(until),
            "the resumed war records its start turn"
        );
        game.turn = until + 60;
        assert!(
            game.war_weariness_points(HUMAN) > 0,
            "a long resumed war must cause weariness"
        );
    }

    #[test]
    fn weariness_makes_citizens_unhappy_and_luxury_cannot_cure_it() {
        let mut game = game_with_city(Government::DEMOCRACY);
        game.turn = 40;
        let calm = game.city_feelings(&game.cities[0], 0);
        at_war(&mut game, Some(HUMAN), 1);
        let weary = game.city_feelings(&game.cities[0], 0);
        assert_ne!(calm, weary);
        let lavish = game.city_feelings(&game.cities[0], 1000);
        let calm_lavish = {
            game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
            game.city_feelings(&game.cities[0], 1000)
        };
        assert_ne!(lavish, calm_lavish, "weariness survives luxury");
    }
}
