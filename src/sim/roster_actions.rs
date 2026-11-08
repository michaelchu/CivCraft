//! Integration checks for Civ2Civ3 unit requirements and action flags.
use super::*;

impl Simulation {
    pub(crate) fn native_roster_stats_oracle(&self) -> bool {
        // Snapshot extracted independently from the supplied ruleset, rather
        // than from UnitStats or the generated capability catalog.
        let rows: Vec<_> = include_str!("../../tests/fixtures/civ2civ3_unit_stats.tsv")
            .lines()
            .skip(2)
            .collect();
        rows.len() == 57
            && UnitKind::ALL_NATIVE
                .into_iter()
                .take(rows.len())
                .zip(rows)
                .all(|(kind, row)| {
                    let expected: Vec<i32> = row
                        .split('\t')
                        .skip(1)
                        .map(|value| value.parse().unwrap())
                        .collect();
                    let stats = kind.stats();
                    expected
                        == [
                            stats.cost,
                            stats.pop_cost,
                            stats.attack,
                            stats.defense,
                            stats.hp,
                            stats.firepower,
                            stats.moves,
                            stats.vision_sq,
                        ]
                })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_57_unit_statistics_match_independent_reference_snapshot() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.native_roster_stats_oracle());
    }

    #[test]
    fn reference_roster_requirements_actions_and_upkeep() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.native_roster_actions_playtest(),
            (true, true, true, true)
        );
    }

    #[test]
    fn no_home_flag_blocks_rehoming_even_malformed_or_transferred_units() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.create_city(HUMAN, 10, 20);
        let mut leader = game.make_unit(0, HUMAN, UnitKind::Leader, 10, 20);
        leader.home_city = Some(u32::MAX);
        assert_eq!(game.home_city_target(&leader), None);
        let mut warrior = game.make_unit(1, HUMAN, UnitKind::Warrior, 10, 20);
        warrior.home_city = Some(u32::MAX);
        assert_eq!(game.home_city_target(&warrior), Some(game.cities[0].id));
    }

    #[test]
    fn marine_flag_permits_real_amphibious_attacks_for_all_flagged_types() {
        for kind in [
            UnitKind::Legion,
            UnitKind::Musketeers,
            UnitKind::Riflemen,
            UnitKind::Marines,
            UnitKind::Pikemen,
        ] {
            let mut game = Simulation::new(crate::mapgen::generate(42));
            game.units.clear();
            game.cities.clear();
            let origin = (10, 20);
            let target = offset_native_by_map(origin.0, origin.1, 1, 0);
            game.tile_mut(origin.0, origin.1).terrain = Terrain::Ocean;
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
    fn caravel_without_shield_to_gold_still_obeys_government_shield_effect() {
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
        assert_eq!(game.city_unit_upkeep(&city).shields, 1);
        game.players[HUMAN].government = Government::Monarchy;
        let support = game.city_unit_upkeep(&city);
        assert_eq!((support.shields, support.gold), (0, 0));
    }
}
