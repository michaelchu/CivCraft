//! AI use of Great Leaders: walk to a city whose building production a Leader
//! can complete, then complete it. See the Great Leader design spec.

use super::*;
use crate::rules::Genus;

impl Simulation {
    /// The owner's city whose current building a Leader should complete: the
    /// wonder city when it is building a wonder, otherwise the city with the
    /// most shields still to go, ties to the nearest city, then the lowest id.
    pub(in crate::sim) fn great_leader_target(&self, unit: &Unit) -> Option<u32> {
        let candidates: Vec<&City> = self
            .cities
            .iter()
            .filter(|city| city.owner == unit.owner && self.leader_can_complete_in(city))
            .collect();
        if let Some(wonder_city) = self.ai_wonder_city(unit.owner)
            && let Some(city) = candidates.iter().find(|city| city.id == wonder_city)
            && matches!(city.production, Production::Building(building)
                if matches!(building.data().genus, Genus::SmallWonder | Genus::GreatWonder))
        {
            return Some(city.id);
        }
        candidates
            .into_iter()
            .max_by_key(|city| {
                (
                    city.production.cost() - city.shields,
                    std::cmp::Reverse(self.map_distance(unit.x, unit.y, city.x, city.y)),
                    std::cmp::Reverse(city.id),
                )
            })
            .map(|city| city.id)
    }

    /// Moves a Leader toward its target city and completes production on
    /// arrival. With no target it waits in a city of its owner. Returns true
    /// because a Leader never takes part in other unit handling.
    pub(in crate::sim) fn run_ai_great_leader(&mut self, unit: &Unit) -> bool {
        let Some(city_id) = self.great_leader_target(unit) else {
            self.shelter_great_leader(unit);
            return true;
        };
        let Some((x, y)) = self.city_by_id(city_id).map(|city| (city.x, city.y)) else {
            return true;
        };
        if (unit.x, unit.y) != (x, y) {
            let result = self.ai_goto_unit(unit.id, (x, y), GotoTask::None);
            if !result.alive || !result.at_goal {
                return true;
            }
        }
        self.complete_production(unit.id, city_id);
        true
    }

    /// Without a legal target a Leader waits inside the nearest city of its
    /// owner and uses the first target that appears.
    fn shelter_great_leader(&mut self, unit: &Unit) {
        if self
            .cities
            .iter()
            .any(|city| city.owner == unit.owner && (city.x, city.y) == (unit.x, unit.y))
        {
            return;
        }
        let nearest = self
            .cities
            .iter()
            .filter(|city| city.owner == unit.owner)
            .min_by_key(|city| (self.map_distance(unit.x, unit.y, city.x, city.y), city.id))
            .map(|city| (city.x, city.y));
        if let Some(goal) = nearest {
            self.ai_goto_unit(unit.id, goal, GotoTask::None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Simulation {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        game.players[1].ai = true;
        game.players[1].known = Tech::all().collect();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Forest;
            tile.hut = false;
        }
        game
    }

    fn city_with(game: &mut Simulation, x: i32, y: i32, production: Production) -> u32 {
        game.create_city(1, x, y);
        let index = game.cities.len() - 1;
        game.cities[index].production = production;
        game.cities[index].shields = 0;
        game.cities[index].id
    }

    fn leader_at(game: &mut Simulation, x: i32, y: i32) -> Unit {
        let id = game.spawn_unit(1, UnitKind::Leader, x, y);
        game.unit_by_id(id).unwrap().clone()
    }

    #[test]
    fn great_leader_target_prefers_a_wonder_in_the_wonder_city() {
        let mut game = game();
        let a = city_with(
            &mut game,
            10,
            24,
            Production::Building(Building::MARKETPLACE),
        );
        let b = city_with(&mut game, 20, 24, Production::Building(Building::PYRAMIDS));
        let c = city_with(
            &mut game,
            30,
            24,
            Production::Building(Building::MARKETPLACE),
        );
        game.reconcile_city_workers();
        let wonder_city = game.ai_wonder_city(1).expect("three cities host one");
        let wonder = game
            .cities
            .iter_mut()
            .find(|city| city.id == wonder_city)
            .unwrap();
        wonder.production = Production::Building(Building::PYRAMIDS);
        // Almost finished, so only the wonder-city rule can pick it over the
        // Marketplaces that still need most of their shields.
        wonder.shields = Production::Building(Building::PYRAMIDS).cost() - 10;
        let leader = leader_at(&mut game, 10, 24);
        assert!([a, b, c].contains(&wonder_city));
        assert_eq!(game.great_leader_target(&leader), Some(wonder_city));
    }

    #[test]
    fn great_leader_target_falls_back_to_the_most_expensive_building_then_nearest() {
        let mut game = game();
        let cheap = city_with(&mut game, 10, 24, Production::Building(Building::TEMPLE));
        let dear = city_with(
            &mut game,
            30,
            24,
            Production::Building(Building::MARKETPLACE),
        );
        let leader = leader_at(&mut game, 10, 24);
        assert_eq!(game.great_leader_target(&leader), Some(dear));
        assert!(
            Production::Building(Building::MARKETPLACE).cost()
                > Production::Building(Building::TEMPLE).cost()
        );

        let mut game = self::game();
        let far = city_with(&mut game, 30, 24, Production::Building(Building::TEMPLE));
        let near = city_with(&mut game, 12, 24, Production::Building(Building::TEMPLE));
        let leader = leader_at(&mut game, 10, 24);
        assert_eq!(game.great_leader_target(&leader), Some(near));
        assert_ne!(far, near);
        let _ = cheap;
    }

    #[test]
    fn great_leader_no_legal_target_means_no_target() {
        let mut game = game();
        city_with(&mut game, 10, 24, Production::Unit(UnitKind::Warrior));
        city_with(&mut game, 20, 24, Production::Building(Building::COINAGE));
        let leader = leader_at(&mut game, 10, 24);
        assert_eq!(game.great_leader_target(&leader), None);
    }

    #[test]
    fn great_leader_in_its_target_city_completes_production() {
        let mut game = game();
        let city = city_with(&mut game, 10, 24, Production::Building(Building::TEMPLE));
        let leader = leader_at(&mut game, 10, 24);
        assert!(game.run_ai_great_leader(&leader));
        assert!(game.unit_by_id(leader.id).is_none());
        assert!(game.city_by_id(city).unwrap().bought_production);
    }

    #[test]
    fn great_leader_walks_toward_its_target() {
        let mut game = game();
        let city = city_with(&mut game, 10, 24, Production::Building(Building::TEMPLE));
        let (cx, cy) = (
            game.city_by_id(city).unwrap().x,
            game.city_by_id(city).unwrap().y,
        );
        let leader = leader_at(&mut game, 16, 24);
        let before = game.map_distance(leader.x, leader.y, cx, cy);
        assert!(game.run_ai_great_leader(&leader));
        let after = game
            .unit_by_id(leader.id)
            .map_or(0, |unit| game.map_distance(unit.x, unit.y, cx, cy));
        assert!(after < before, "{before} -> {after}");
    }

    #[test]
    fn great_leader_without_target_stays_put_and_is_never_a_defender() {
        let mut game = game();
        city_with(&mut game, 10, 24, Production::Unit(UnitKind::Warrior));
        let leader = leader_at(&mut game, 10, 24);
        game.refresh_ai_guards();
        let defenders = game.assign_ai_city_defenders(1);
        assert!(!defenders.contains(&leader.id));
        assert!(!game.ai_guard_targets.contains_key(&leader.id));
        assert!(game.run_ai_great_leader(&leader));
        let after = game.unit_by_id(leader.id).unwrap();
        assert_eq!((after.x, after.y), (10, 24));
    }
}
