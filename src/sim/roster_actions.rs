//! Unit action and upkeep integration checks.
#[cfg(test)]
use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_home_ids_rehome_at_a_friendly_city_when_source_flags_allow_it() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.create_city(HUMAN, 10, 20);
        let mut leader = game.make_unit(0, HUMAN, UnitKind::Leader, 10, 20);
        leader.home_city = Some(u32::MAX);
        assert!(!UnitKind::Leader.has_no_home());
        assert_eq!(game.home_city_target(&leader), Some(game.cities[0].id));
        let mut warrior = game.make_unit(1, HUMAN, UnitKind::Warrior, 10, 20);
        warrior.home_city = Some(u32::MAX);
        assert_eq!(game.home_city_target(&warrior), Some(game.cities[0].id));
    }

    #[test]
    fn campaign_marine_identity_allows_real_amphibious_attacks() {
        {
            let kind = UnitKind::Marine;
            let mut game = Simulation::new(crate::mapgen::generate(42));
            game.units.clear();
            game.cities.clear();
            let origin = (10, 20);
            let target = offset_native_by_map(origin.0, origin.1, 1, 0);
            game.tile_mut(origin.0, origin.1).terrain = Terrain::Sea;
            game.tile_mut(target.0, target.1).terrain = Terrain::Grassland;
            let boat = game.spawn_unit(HUMAN, UnitKind::Galleon, origin.0, origin.1);
            let actor = game.spawn_unit(HUMAN, kind, origin.0, origin.1);
            game.units
                .iter_mut()
                .find(|unit| unit.id == actor)
                .unwrap()
                .transport = Some(boat);
            game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
            let result = game.try_move_unit(actor, 1, 0);
            assert_eq!(
                result,
                if kind.has_marine_attack() {
                    MoveResult::Fought
                } else {
                    MoveResult::Blocked
                },
                "{kind:?}"
            );
        }
    }

    #[test]
    fn caravel_upkeep_uses_the_selected_c2c3_record_and_government_effects() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.create_city(HUMAN, 10, 20);
        let city = game.cities[0].clone();
        for _ in 0..3 {
            game.spawn_unit(HUMAN, UnitKind::Caravel, 10, 20);
            game.units.last_mut().unwrap().home_city = Some(city.id);
        }
        game.players[HUMAN].government = Government::Anarchy;
        let anarchy = game.city_unit_upkeep(&city);
        // C2C3 Caravels have one shield upkeep; Anarchy supports two
        // shields per city, so three cost one shield in aggregate.
        assert_eq!((anarchy.shields, anarchy.gold), (1, 0));
        game.players[HUMAN].government = Government::Monarchy;
        let support = game.city_unit_upkeep(&city);
        // Monarchy disables shield upkeep, and Caravels have no direct gold
        // upkeep in the selected C2C3 unit record.
        assert_eq!((support.shields, support.gold), (0, 0));
    }
}
