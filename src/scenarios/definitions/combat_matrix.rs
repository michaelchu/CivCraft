//! Actual movement-triggered combat across terrain and city defenses.
use super::{Checks, Scenario};
use crate::{
    sim::{HUMAN, MoveResult, Pact, Simulation, UnitKind, fixtures::WorldBuilder},
    world::Terrain,
};
pub(super) const SCENARIO: Scenario = Scenario::all_ai("combat-matrix", execute);
fn execute(game: &mut Simulation, checks: &mut Checks) {
    let cases = [
        (
            "open_grassland",
            UnitKind::Warrior,
            UnitKind::Warrior,
            Terrain::Grassland,
            false,
        ),
        (
            "forest_defense",
            UnitKind::Swordsman,
            UnitKind::Spearman,
            Terrain::Forest,
            false,
        ),
        (
            "hill_defense",
            UnitKind::Cavalry,
            UnitKind::Rifleman,
            Terrain::Hills,
            false,
        ),
        (
            "city_defense",
            UnitKind::Cavalry,
            UnitKind::Rifleman,
            Terrain::Grassland,
            true,
        ),
    ];

    for (case_index, (name, attacker_kind, defender_kind, terrain, city)) in
        cases.into_iter().enumerate()
    {
        WorldBuilder::new(game).combat_world();

        let x = 5 + case_index as i32 * 4;
        let y = 20;
        let (tx, ty) = game.map_wrap.civ3_neighbors(x, y).next().unwrap();
        WorldBuilder::new(game).terrain((tx, ty), terrain);
        game.sign_pact(HUMAN, 1, Pact::War);
        if city {
            WorldBuilder::new(game).founding_city(1, (tx, ty));
        }
        let attacker_id = WorldBuilder::new(game).unit(HUMAN, attacker_kind, (x, y));
        let defender_id = WorldBuilder::new(game).unit(1, defender_kind, (tx, ty));
        let defender = game.unit_by_id(defender_id).unwrap().clone();
        let base_defense = game.defense_power_against(&defender, x, y, attacker_kind);
        let terrain_defense = game.defense_power_against(&defender, tx, ty, attacker_kind);
        let modifier_ok = if terrain == Terrain::Grassland && !city {
            terrain_defense == base_defense
        } else {
            terrain_defense > base_defense
        };
        let mut world = WorldBuilder::new(game);
        world.legacy_combat_rank(attacker_id, u8::MAX as usize);
        world.legacy_combat_rank(defender_id, u8::MAX as usize);
        world.legacy_random_seed(0x0C13 + case_index as u64);
        world.refresh_visibility();

        let direction = game
            .map_wrap
            .civ3_neighbors(x, y)
            .position(|position| position == (tx, ty))
            .unwrap();
        let result = game.try_move_unit_civ3(attacker_id, direction);
        let attacker = game.unit_by_id(attacker_id);
        let defender = game.unit_by_id(defender_id);
        let attacker_hp = attacker.map_or(0, |unit| unit.hp);
        let defender_hp = defender.map_or(0, |unit| unit.hp);
        let terminal =
            attacker.is_none_or(|unit| unit.hp <= 0) || defender.is_none_or(|unit| unit.hp <= 0);
        let passed = result == MoveResult::Fought && terminal && modifier_ok;
        let outcome = match (attacker.is_some(), defender.is_some()) {
            (true, false) => format!("{} won with {attacker_hp} HP", attacker_kind.name()),
            (false, true) => format!("{} won with {defender_hp} HP", defender_kind.name()),
            (false, false) => "both units were destroyed".into(),
            (true, true) => format!("combat did not end ({attacker_hp} vs {defender_hp} HP)"),
        };
        checks.push((
                name,
                passed,
                format!(
                    "{} {} vs {} on {:?}{}: defense {base_defense}->{terrain_defense}; {result:?}; {outcome}",
                    attacker_kind.name(),
                    attacker_kind.stats().attack,
                    defender_kind.name(),
                    terrain,
                    if city { " in a city" } else { "" },
                ),
            ));
    }
    let mut world = WorldBuilder::new(game);
    world.clear_units_and_cities();
    world.clear_outcome();
    for (owner, x, y) in [(HUMAN, 2, 2), (1, 5, 2)] {
        world.founding_city(owner, (x, y));
    }
}
