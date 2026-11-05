//! Integration checks for Civ2Civ3 unit requirements and action flags.
use super::*;

impl Simulation {
    pub(crate) fn transport_hud_playtest_setup(&mut self) -> u32 {
        self.units.clear();
        self.cities.clear();
        let position = (20, 24);
        let tile = self.tile_mut(position.0, position.1);
        tile.terrain = Terrain::DeepOcean;
        tile.oil_platform = false;
        tile.pollution = true;
        self.players[HUMAN].known.push(Tech::MINIATURIZATION);
        let id = self.spawn_unit(HUMAN, UnitKind::Transport, position.0, position.1);
        self.refresh_visibility();
        id
    }

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

    pub(crate) fn native_roster_actions_playtest(&self) -> (bool, bool, bool, bool) {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.base = None;
        }
        game.players[HUMAN].known.clear();
        game.players[HUMAN].government = Government::Despotism;
        game.create_city(HUMAN, 10, 20);
        let city = game.cities[0].clone();
        let unavailable = [
            UnitKind::Pikemen,
            UnitKind::Riflemen,
            UnitKind::Armor,
            UnitKind::Awacs,
            UnitKind::Fanatics,
            UnitKind::Explorer,
            UnitKind::Leader,
            UnitKind::Storm,
        ]
        .into_iter()
        .all(|kind| !game.can_produce(&city, Production::Unit(kind)));
        game.players[HUMAN].known.extend([
            Tech::all().find(|tech| tech.name() == "Feudalism").unwrap(),
            Tech::BRONZE_WORKING,
        ]);
        let pikemen = game.can_produce(&city, Production::Unit(UnitKind::Pikemen))
            && !game.can_produce(&city, Production::Unit(UnitKind::Phalanx));
        game.players[HUMAN].known.push(Tech::AMPHIBIOUS_WARFARE);
        // A later successor obsoletes the entire chain even when intermediate
        // technologies are absent (scenario grants and traded technologies).
        let successor = game.available_unit_successor(HUMAN, UnitKind::Phalanx)
            == Some(UnitKind::Marines)
            && !game.can_produce(&city, Production::Unit(UnitKind::Pikemen));
        game.players[HUMAN].known.extend([
            Tech::GUERILLA_WARFARE,
            Tech::all().find(|tech| tech.name() == "Seafaring").unwrap(),
        ]);
        let gov_gate = !game.unit_available(HUMAN, UnitKind::Fanatics);
        game.players[HUMAN].government = Government::Fundamentalism;
        let requirements = unavailable
            && pikemen
            && successor
            && gov_gate
            && game.unit_available(HUMAN, UnitKind::Fanatics)
            && game.unit_available(HUMAN, UnitKind::Explorer);

        let defender = game.make_unit(0, HUMAN, UnitKind::Warrior, city.x, city.y);
        let firepower = [
            UnitKind::Artillery,
            UnitKind::Howitzer,
            UnitKind::CruiseMissile,
        ]
        .into_iter()
        .all(|kind| {
            let attacker = game.make_unit(1, 1, kind, city.x + 1, city.y);
            let city_fp = game.combat_firepower(&attacker, &defender).0;
            let mut field = defender.clone();
            field.x += 4;
            city_fp == 2 * kind.stats().firepower
                && game.combat_firepower(&attacker, &field).0 == kind.stats().firepower
        });
        let pos = (14, 24);
        game.tile_mut(pos.0, pos.1).terrain = Terrain::Mountains;
        game.tile_mut(pos.0, pos.1).river = true;
        let big_land = [
            UnitKind::MechInf,
            UnitKind::Armor,
            UnitKind::Artillery,
            UnitKind::Howitzer,
        ]
        .into_iter()
        .all(|kind| {
            let unit = game.make_unit(0, HUMAN, kind, pos.0, pos.1);
            !game.pf_native_tile(kind, pos.0, pos.1)
                && game.pf_extra_class_defense(kind, pos.0, pos.1) == 0
                && game.unit_move_rate(&unit)
                    == ((kind.stats().moves - 1) * MOVE_FRAGMENTS).max(MOVE_FRAGMENTS)
        });
        let combat = firepower && big_land;

        game.players[HUMAN].government = Government::Monarchy;
        for _ in 0..5 {
            let id = game.spawn_unit(HUMAN, UnitKind::Fanatics, city.x, city.y);
            let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
            unit.home_city = Some(city.id);
        }
        let support = game.city_unit_upkeep(&city);
        let normal_food = support.food == 1 && support.gold == 0 && support.shields == 0;
        game.players[HUMAN].government = Government::Fundamentalism;
        let support = game.city_unit_upkeep(&city);
        let free = support.food == 0 && support.gold == 0 && support.shields == 0;
        let id = game.units[0].id;
        let joined = game.join_city(id, city.id)
            && game.cities[0].size == city.size + 1
            && !game.units.iter().any(|unit| unit.id == id);
        let upkeep = normal_food && free && joined;

        let flags = [
            UnitKind::Legion,
            UnitKind::Musketeers,
            UnitKind::Riflemen,
            UnitKind::Marines,
        ]
        .into_iter()
        .all(UnitKind::has_marine_attack)
            && !UnitKind::Riflemen.can_helicopter_field_load()
            && [
                UnitKind::AlpineTroops,
                UnitKind::Fanatics,
                UnitKind::Partisan,
                UnitKind::Marines,
                UnitKind::Paratroopers,
            ]
            .into_iter()
            .all(UnitKind::can_helicopter_field_load)
            && [
                UnitKind::Pikemen,
                UnitKind::Riflemen,
                UnitKind::AlpineTroops,
                UnitKind::Fanatics,
            ]
            .into_iter()
            .all(UnitKind::can_build_fortress)
            && [UnitKind::Leader, UnitKind::Storm, UnitKind::BarbarianLeader]
                .into_iter()
                .all(UnitKind::is_unbribable);
        (requirements, combat, upkeep, flags)
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
