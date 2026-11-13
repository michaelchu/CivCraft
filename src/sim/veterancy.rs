//! Unit veteran levels and combat experience.

use super::*;
use crate::rules::freeciv::{RulesetData, VeteranLevel, VeteranSystem, effect_value};
use std::sync::OnceLock;

fn selected_veteran_systems() -> &'static [(UnitKind, VeteranSystem)] {
    static SYSTEMS: OnceLock<Vec<(UnitKind, VeteranSystem)>> = OnceLock::new();
    SYSTEMS.get_or_init(|| {
        let rules = RulesetData::campaign_default();
        UnitKind::ALL_NATIVE
            .into_iter()
            .map(|kind| {
                let section = crate::sim::action_rules::freeciv_unit_type_section(kind);
                (
                    kind,
                    rules
                        .veteran_system(section.as_deref())
                        .unwrap_or_else(|error| {
                            panic!("invalid selected C2C3 veteran system for {kind:?}: {error}")
                        }),
                )
            })
            .collect()
    })
}

pub(super) fn selected_veteran_system(kind: UnitKind) -> &'static VeteranSystem {
    selected_veteran_systems()
        .iter()
        .find_map(|(candidate, system)| (*candidate == kind).then_some(system))
        .unwrap_or_else(|| {
            panic!("selected C2C3 veterancy has no explicit unit identity for {kind:?}")
        })
}

fn selected_veteran_level(unit: &Unit) -> &'static VeteranLevel {
    let system = selected_veteran_system(unit.kind);
    system
        .level(unit.c2c3_veteran)
        .or_else(|| system.levels.last())
        .expect("selected C2C3 veteran system has at least one level")
}

pub(super) fn combat_veterancy_chances(
    attacker_strength: i64,
    defender_strength: i64,
    scaled: bool,
) -> (i32, i32) {
    if !scaled {
        return (100, 100);
    }
    let total = attacker_strength + defender_strength;
    if total <= 0 {
        return (0, 0);
    }
    let attacker = (200 * defender_strength / total).clamp(0, 200) as i32;
    let defender = (200 * attacker_strength / total).clamp(0, 200) as i32;
    (attacker, defender)
}

impl Unit {
    pub(super) fn activity_rate(&self) -> i32 {
        ACTIVITY_FACTOR * self.kind.stats().moves.max(1) * self.veteran_power() / 100
    }

    pub(crate) fn work_turns_left(&self) -> i32 {
        let rate = self.activity_rate();
        (self.work_left + rate - 1) / rate
    }

    pub(crate) fn rank_name(&self) -> &'static str {
        ["conscript", "regular", "veteran", "elite"][self.veteran.min(3)]
    }

    pub(super) fn veteran_power(&self) -> i32 {
        selected_veteran_level(self).power_factor
    }

    pub(super) fn veteran_move_bonus(&self) -> i32 {
        selected_veteran_level(self).move_bonus
    }
}

impl Simulation {
    /// Calculate the Freeciv veteran level granted to a unit built in a city.
    /// This is separate from the retained Civ III rank, which continues to
    /// supply the Conquests hitpoint bonus and display identity.
    pub(super) fn trained_c2c3_veteran(&self, city: &City, kind: UnitKind) -> usize {
        let system = selected_veteran_system(kind);
        let unit = Unit::new(u32::MAX, city.owner, kind, city.x, city.y);
        let effect = self.city_rule_effect_for_unit(
            city,
            &unit,
            "Veteran_Build",
            self.players[city.owner].government,
        );
        effect.clamp(0, system.levels.len().saturating_sub(1) as i32) as usize
    }

    pub(super) fn promote_combat_veterans(
        &mut self,
        attacker_id: u32,
        defender_id: u32,
        powerless: bool,
        attacker_base_chance: i32,
        defender_base_chance: i32,
    ) {
        let settings = super::combat_rules::selected();
        if powerless && settings.only_real_fight_makes_veteran {
            return;
        }
        let attacker = self.unit_by_id(attacker_id).cloned();
        let defender = self.unit_by_id(defender_id).cloned();
        let Some(attacker) = attacker else {
            return;
        };
        let Some(defender) = defender else {
            return;
        };
        if settings.only_killing_makes_veteran && attacker.hp > 0 && defender.hp > 0 {
            return;
        }
        if attacker.hp > 0 {
            self.promote_unit_after_combat(attacker.id, attacker_base_chance);
        }
        if defender.hp > 0 {
            self.promote_unit_after_combat(defender.id, defender_base_chance);
        }
    }

    fn promote_unit_after_combat(&mut self, id: u32, base_chance: i32) {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return;
        };
        let system = selected_veteran_system(unit.kind);
        let Some(level) = system.level(unit.c2c3_veteran) else {
            return;
        };
        if unit.c2c3_veteran + 1 >= system.levels.len()
            || !self
                .action_available_for_unit(&unit, "Gain Veterancy", (unit.x, unit.y))
                .unwrap_or(false)
        {
            return;
        }
        let rules = RulesetData::campaign_default();
        let context = crate::sim::action_rules::SimulationRequirementContext::for_unit(self, &unit);
        let bonus = effect_value(&rules.effects, "Veteran_Combat", &context)
            .expect("campaign C2C3 Veteran_Combat requirements are supported");
        let modifier =
            base_chance + i32::try_from(bonus).expect("campaign C2C3 Veteran_Combat fits in i32");
        let chance = level.base_raise_chance * modifier / 100;
        if self.rand_below(100) >= chance {
            return;
        }

        let promoted_level = unit.c2c3_veteran + 1;
        let promoted_name = system
            .level(promoted_level)
            .map(|level| level.name.clone())
            .unwrap_or_else(|| promoted_level.to_string());
        if let Some(index) = self.unit_index_by_id(id) {
            self.units[index].c2c3_veteran = promoted_level;
        }
        if unit.owner == HUMAN {
            let message = format!(
                "Your {} gained {promoted_name} experience.",
                unit.kind.name()
            );
            self.add_event(message);
        }
    }

    pub(super) fn active_barracks(&self, city: &City) -> bool {
        city.has_building(Building::BARRACKS)
    }

    pub(super) fn trained_rank(&self, city: &City, kind: UnitKind) -> usize {
        let default_rank = crate::sim::civ3_unit_rules::default_experience_rank();
        if kind.no_veteran() || kind.max_veteran_levels() <= 1 {
            return default_rank;
        }
        let rank = if kind.unit_class().has_barracks() && !kind.is_civilian() {
            usize::from(
                self.active_barracks(city)
                    || self.has_active_wonder(city.owner, Building::SUN_TZUS_WAR_ACADEMY),
            )
        } else {
            0
        };
        (default_rank + rank).min(kind.max_veteran_levels() - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civ3_rank_changes_hit_points_without_changing_combat_power() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        game.cities[0]
            .buildings
            .extend([Building::BARRACKS, Building::SUN_TZUS_WAR_ACADEMY]);
        assert_eq!(game.trained_rank(&game.cities[0], UnitKind::Warrior), 2);
        game.players[HUMAN].government = Government::Communism;
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        let regular = game.unit_by_id(id).unwrap().clone();
        let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
        unit.veteran = 2;
        unit.hp = unit.max_hit_points();
        let veteran = game.unit_by_id(id).unwrap();
        assert_eq!(veteran.rank_name(), "veteran");
        assert_eq!(veteran.max_hit_points(), 4);
        assert_eq!(veteran.hp, 4);
        assert_eq!(
            Simulation::attack_power(veteran),
            Simulation::attack_power(&regular)
        );
        let mut freeciv_veteran = regular.clone();
        freeciv_veteran.c2c3_veteran = 1;
        assert_eq!(
            Simulation::attack_power(&freeciv_veteran),
            UnitKind::Warrior.stats().attack * POWER_FACTOR * 150 / 100
        );
    }

    #[test]
    fn barracks_production_completes_as_veteran_after_a_turn() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let (city, expected_rank) = game.veteran_playtest_setup();
        let city_state = game.city_by_id(city).unwrap();
        assert_eq!(game.trained_c2c3_veteran(city_state, UnitKind::Warrior), 1);
        assert!(game.unit_buildable(city_state, UnitKind::Warrior));
        assert!(
            game.cities
                .iter()
                .find(|candidate| candidate.id == city)
                .is_some_and(
                    |candidate| candidate.production == Production::Unit(UnitKind::Warrior)
                )
        );

        assert!(game.apply(Command::EndTurn));
        assert!(game.units.iter().any(|unit| {
            unit.home_city == Some(city)
                && unit.kind == UnitKind::Warrior
                && unit.veteran == expected_rank
                && unit.c2c3_veteran == 1
        }));
    }

    #[test]
    fn combat_veterancy_odds_scale_with_opponent_strength() {
        assert_eq!(combat_veterancy_chances(10, 10, true), (100, 100));
        assert_eq!(combat_veterancy_chances(10, 30, true), (150, 50));
        assert_eq!(combat_veterancy_chances(30, 10, true), (50, 150));
        assert_eq!(combat_veterancy_chances(0, 10, true), (200, 0));
        assert_eq!(combat_veterancy_chances(0, 0, true), (0, 0));
        assert_eq!(combat_veterancy_chances(10, 30, false), (100, 100));
    }

    #[test]
    fn a_real_combat_can_promote_both_survivors_without_changing_civ3_hp_rank() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        let attacker_id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
        let defender_id = game.spawn_unit(1, UnitKind::Warrior, 11, 24);
        let original_attacker = game.unit_by_id(attacker_id).unwrap().clone();
        let original_defender = game.unit_by_id(defender_id).unwrap().clone();

        game.promote_combat_veterans(attacker_id, defender_id, false, 200, 200);

        let attacker = game.unit_by_id(attacker_id).unwrap();
        let defender = game.unit_by_id(defender_id).unwrap();
        assert_eq!(attacker.c2c3_veteran, 1);
        assert_eq!(defender.c2c3_veteran, 1);
        assert_eq!(attacker.veteran, original_attacker.veteran);
        assert_eq!(defender.veteran, original_defender.veteran);
        assert_eq!(attacker.hp, original_attacker.hp);
        assert_eq!(defender.hp, original_defender.hp);
    }

    #[test]
    fn real_combat_rolls_veterancy_before_the_occupation_chance() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        let (x, y) = (10, 24);
        let (tx, ty) = game.map_wrap.offset(x, y, 1, 0).unwrap();
        for (tile_x, tile_y) in [(x, y), (tx, ty)] {
            let tile = game.tile_mut(tile_x, tile_y);
            tile.terrain = super::super::Terrain::Grassland;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
            tile.base = None;
            tile.hut = false;
        }
        game.players[HUMAN].government = super::super::Government::Despotism;
        game.players[1].government = super::super::Government::Despotism;
        game.sign_pact(HUMAN, 1, super::super::Pact::War);
        let attacker_id = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        let defender_id = game.spawn_unit(1, UnitKind::Warrior, tx, ty);
        let attacker = game.unit_by_id(attacker_id).unwrap().clone();
        let defender = game.unit_by_id(defender_id).unwrap().clone();

        let attack_power = game.attack_power_against(&attacker, tx, ty);
        let defense_power = game.defense_power_against(&defender, tx, ty, attacker.kind);
        let (attack_firepower, defense_firepower) = game.combat_firepower(&attacker, &defender);
        let (attacker_vet_base_chance, _) = combat_veterancy_chances(
            i64::from(attack_power) * i64::from(attacker.hp) * i64::from(attack_firepower),
            i64::from(defense_power) * i64::from(defender.hp) * i64::from(defense_firepower),
            super::super::combat_rules::selected().combat_odds_scaled_veterancy,
        );
        let attacker_level = selected_veteran_system(attacker.kind)
            .level(attacker.c2c3_veteran)
            .expect("Warrior starts below the selected veteran cap");
        let veteran_bonus = effect_value(
            &RulesetData::campaign_default().effects,
            "Veteran_Combat",
            &crate::sim::action_rules::SimulationRequirementContext::for_unit(&game, &attacker),
        )
        .expect("campaign C2C3 Veteran_Combat requirements are supported");
        let attacker_promotion_chance = attacker_level.base_raise_chance
            * (attacker_vet_base_chance
                + i32::try_from(veteran_bonus).expect("Veteran_Combat fits in i32"))
            / 100;
        assert!((1..100).contains(&attacker_promotion_chance));
        assert_eq!(super::super::combat_rules::selected().occupychance, 100);

        // common/unittools.c::unit_versus_unit consumes combat-round draws;
        // combat_veterans then tests the surviving attacker; unithand.c draws
        // Occupy_Chance only after those veteran checks. Select a seed where
        // swapping the latter two draws would change the promotion result.
        let expected = (1_u64..10_000)
            .find_map(|seed| {
                let mut probe = game.clone();
                probe.native_random = None;
                probe.rng = seed;
                let mut attacker_hp = attacker.hp;
                let mut defender_hp = defender.hp;
                while attacker_hp > 0 && defender_hp > 0 {
                    if probe.rand_below(attack_power + defense_power) >= defense_power {
                        defender_hp -= attack_firepower;
                    } else {
                        attacker_hp -= defense_firepower;
                    }
                }
                if attacker_hp <= 0 || defender_hp > 0 {
                    return None;
                }
                let veteran_roll = probe.rand_below(100);
                let occupation_roll = probe.rand_below(100);
                let promotion = veteran_roll < attacker_promotion_chance;
                (promotion != (occupation_roll < attacker_promotion_chance)).then_some((
                    seed,
                    promotion,
                    probe.random_snapshot(),
                ))
            })
            .expect("find an attacker win that distinguishes veteran and occupation draw order");
        let (seed, expected_promotion, expected_random) = expected;
        game.native_random = None;
        game.rng = seed;

        game.resolve_combat(attacker_id, defender_id, tx, ty, None);

        assert_eq!(game.random_snapshot(), expected_random);
        assert!(game.unit_by_id(defender_id).is_none());
        assert_eq!(
            game.unit_by_id(attacker_id).unwrap().c2c3_veteran,
            usize::from(expected_promotion),
            "the first post-round draw belongs to the surviving attacker"
        );
    }

    #[test]
    fn powerless_fights_cannot_promote_under_selected_c2c3_setting() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        let attacker_id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
        let defender_id = game.spawn_unit(1, UnitKind::Warrior, 11, 24);

        game.promote_combat_veterans(attacker_id, defender_id, true, 200, 200);

        assert_eq!(game.unit_by_id(attacker_id).unwrap().c2c3_veteran, 0);
        assert_eq!(game.unit_by_id(defender_id).unwrap().c2c3_veteran, 0);
    }
}
