//! AI Leader eligibility, consumption and wonder completion.
use super::{Checks, Scenario};
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::drive::Drive;
use crate::{
    rules::{Building, Tech},
    sim::{Production, Simulation, UnitKind, fixtures::WorldBuilder},
};
pub(super) const SCENARIO: Scenario = Scenario::new(
    "great-leader",
    2,
    ScenarioKind::Actions {
        control: Control::AllAi,
        execute,
    },
);

fn execute(game: &mut Simulation, checks: &mut Checks) {
    let mut world = WorldBuilder::new(game);
    world.grassland_campaign(false);
    world.ai_control(1, true);
    world.known_technologies(1, &Tech::all().collect::<Vec<_>>());
    let city = world.founding_city(1, (10, 24));
    let wonder_city = world.founding_city(1, (20, 24));
    let unbuildable = !game.can_produce(
        game.city_by_id(city).unwrap(),
        Production::Unit(UnitKind::Leader),
    );

    // A military Leader cannot rush a Great Wonder, but finishes an improvement.
    let mut world = WorldBuilder::new(game);
    world.production_stock(city, Production::Building(Building::PYRAMIDS), 0);
    let military = world.unit(1, UnitKind::Leader, (10, 24));
    game.run_ai_great_leader(&game.unit_by_id(military).unwrap().clone());
    let refused = game.unit_by_id(military).is_some()
        && game.city_by_id(city).is_some_and(|city| city.shields == 0);
    WorldBuilder::new(game).production_stock(city, Production::Building(Building::TEMPLE), 0);
    game.run_ai_great_leader(&game.unit_by_id(military).unwrap().clone());
    game.process_player_cities(1);
    let improvement = game.unit_by_id(military).is_none()
        && game
            .city_by_id(city)
            .is_some_and(|city| city.has_building(Building::TEMPLE));

    // A Scientific Leader completes the Great Wonder in one turn.
    let mut world = WorldBuilder::new(game);
    world.production_stock(wonder_city, Production::Building(Building::PYRAMIDS), 0);
    let leader = world.scientific_leader(1, (20, 24));
    game.run_ai_great_leader(&game.unit_by_id(leader).unwrap().clone());
    let consumed = game.unit_by_id(leader).is_none();
    game.process_player_cities(1);
    let completes = game
        .city_by_id(wonder_city)
        .is_some_and(|city| city.has_building(Building::PYRAMIDS));
    checks.extend([
        (
            "leader_unbuildable",
            unbuildable,
            "No city can build a Leader even with every technology".into(),
        ),
        (
            "military_leader_cannot_rush_great_wonder",
            refused,
            "A military Leader is refused a Great Wonder and is not consumed".into(),
        ),
        (
            "military_leader_completes_improvement",
            improvement,
            "A military Leader still completes an ordinary improvement".into(),
        ),
        (
            "leader_completes_wonder",
            completes,
            "An AI Scientific Leader completes its wonder city's wonder in one turn".into(),
        ),
        (
            "leader_consumed",
            consumed,
            "Completing production consumes the Leader".into(),
        ),
    ]);
}

pub(super) const ARMY: Scenario = Scenario::new(
    "leader-army",
    1,
    ScenarioKind::Actions {
        control: Control::Human,
        execute: army_formation,
    },
);

fn army_formation(game: &mut Simulation, checks: &mut Checks) {
    use crate::sim::{Command, HUMAN};
    let mut world = WorldBuilder::new(game);
    world.grassland_campaign(false);
    let mut cities = Vec::new();
    for index in 0..12 {
        cities.push(world.founding_city(HUMAN, (8 + (index % 6) * 4, 12 + (index / 6) * 6)));
    }
    world.finish();
    let city_id = cities[0];
    let initially_blocked = !game.can_produce(
        game.city_by_id(city_id).unwrap(),
        Production::Building(Building::PENTAGON),
    );
    let army_unbuildable = !game.apply(Command::Produce(city_id, Production::Unit(UnitKind::Army)));
    let mut formed = true;
    let mut leaders_consumed = true;
    for _ in 0..3 {
        let leader = WorldBuilder::new(game).unit(HUMAN, UnitKind::Leader, (8, 12));
        formed &= game.apply(Command::FormArmy(leader));
        leaders_consumed &= game.unit_by_id(leader).is_none();
    }
    let armies = game
        .units()
        .iter()
        .filter(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Army)
        .count();
    let pentagon_unlocked = game.can_produce(
        game.city_by_id(city_id).unwrap(),
        Production::Building(Building::PENTAGON),
    );
    let extra = WorldBuilder::new(game).unit(HUMAN, UnitKind::Leader, (8, 12));
    let capacity_enforced =
        !game.apply(Command::FormArmy(extra)) && game.unit_by_id(extra).is_some();
    checks.extend([
        (
            "army_not_city_production",
            army_unbuildable,
            "Armies cannot be chosen as ordinary city production".into(),
        ),
        (
            "leaders_form_armies",
            formed && armies == 3,
            "Three Leaders form three Armies through the player command".into(),
        ),
        (
            "leaders_consumed",
            leaders_consumed,
            "Each successful formation consumes exactly its Leader".into(),
        ),
        (
            "army_city_limit",
            capacity_enforced,
            "Twelve cities support three Armies; a fourth Leader is retained on rejection".into(),
        ),
        (
            "pentagon_requires_armies",
            initially_blocked && pentagon_unlocked,
            "The Pentagon unlocks only after three owned Armies exist".into(),
        ),
    ]);
}

pub(super) const ARMY_COMBAT: Scenario = Scenario::new(
    "army-combat",
    1,
    ScenarioKind::Actions {
        control: Control::Human,
        execute: army_combat,
    },
);

fn army_combat(game: &mut Simulation, checks: &mut Checks) {
    use crate::sim::{Command, HUMAN, Pact};
    let origin = (10, 24);
    let target = game
        .map_wrap
        .eight_way_neighbor(origin.0, origin.1, 2)
        .unwrap();
    let mut world = WorldBuilder::new(game);
    world.grassland_campaign(false);
    let home = world.founding_city(HUMAN, origin);
    let enemy_city = world.city(1, target, 3);
    world.founding_city(1, (24, 24));
    let army = world.unit(HUMAN, UnitKind::Army, origin);
    let members: Vec<_> = (0..4)
        .map(|_| world.unit(HUMAN, UnitKind::ModernArmor, origin))
        .collect();
    let defender = world.unit(1, UnitKind::Warrior, target);
    world.finish();
    game.sign_pact(HUMAN, 1, Pact::War);
    let mut loaded = true;
    for &member in &members[..3] {
        loaded &= game.apply(Command::BoardOrDeboard(member));
    }
    let fourth_blocked = !game.apply(Command::BoardOrDeboard(members[3]));
    WorldBuilder::new(game).culture_building(home, Building::PENTAGON);
    let fourth_loaded = game.apply(Command::BoardOrDeboard(members[3]));
    let max_hp: i32 = members
        .iter()
        .map(|&id| game.unit_by_id(id).unwrap().max_hit_points())
        .sum();
    let combined_health = game.unit_by_id(army).unwrap().hp == max_hp
        && game.unit_by_id(army).unwrap().max_hit_points() == max_hp;
    let retained = !game.apply(Command::BoardOrDeboard(members[0]));
    let attacked = game.apply(Command::MoveByDirection(army, 2));
    let won =
        game.unit_by_id(defender).is_none() && game.city_by_id(enemy_city).unwrap().owner == HUMAN;
    let cohesive = game.unit_by_id(army).is_some_and(|army| {
        army.hp
            == members
                .iter()
                .filter_map(|&id| game.unit_by_id(id))
                .map(|u| u.hp)
                .sum::<i32>()
            && members.iter().all(|&id| {
                game.unit_by_id(id)
                    .is_some_and(|u| (u.x, u.y) == target && u.transport == Some(army.id))
            })
    });
    checks.extend([
        (
            "army_loads_three",
            loaded && fourth_blocked,
            "Three troops load; a fourth is blocked without the Pentagon".into(),
        ),
        (
            "pentagon_fourth_member",
            fourth_loaded && game.cargo_count(army) == 4,
            "The owner's Pentagon enables the fourth member".into(),
        ),
        (
            "combined_army_health",
            combined_health,
            "Army HP and maximum HP combine the four members".into(),
        ),
        (
            "army_members_stay_formed",
            retained,
            "Loaded members cannot leave or act independently".into(),
        ),
        (
            "army_wins_and_captures",
            attacked && won,
            "A player movement command resolves Army combat and captures the defended city".into(),
        ),
        (
            "army_damage_and_movement",
            cohesive,
            "Members retain damage and follow the Army into the captured tile".into(),
        ),
    ]);
}
