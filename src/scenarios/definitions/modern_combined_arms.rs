//! Modern combined-arms AI war scenario for military analytics inspection.
//!
//! Two full-tech AI superpowers fight a land, air, sea, missile, nuclear, and
//! Army campaign so the `military` analyzer can audit production choices,
//! bombardment, air missions, missiles, Great Leader rolls, and Army usage.

use super::*;
use crate::{
    rules::{Building, Tech},
    sim::{
        Government, Pact, Simulation, UnitKind,
        fixtures::{CivPreset, TilePreset, WorldBuilder},
    },
    world::{GeneratedMap, Resource, Terrain, TerrainBase},
};
use serde_json::{Value, json};

const ROME: usize = 0;
const GERMANY: usize = 1;
const ELITE_RANK: usize = 3;

pub(super) const SCENARIO: Scenario = Scenario::new(
    "modern-combined-arms",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup,
    },
);

fn land_tile(
    terrain: Terrain,
    base_terrain: TerrainBase,
    resource: Option<Resource>,
) -> TilePreset {
    TilePreset {
        terrain,
        base_terrain,
        resource,
        river: false,
        road: true,
        railroad: true,
        base: None,
    }
}

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let all_techs: Vec<Tech> = Tech::all().collect();
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "rome",
                technologies: Some(&all_techs),
            },
            CivPreset {
                id: "germany",
                technologies: Some(&all_techs),
            },
        ],
    );

    let mut world = WorldBuilder::new(&mut game);
    world.start_at(250, 1980);

    // Connected continent (x in 6..=40, y in 14..=32) above a southern sea lane.
    for y in 14..=32 {
        for x in 6..=40 {
            world.tile(
                (x, y),
                land_tile(Terrain::GrasslandShield, TerrainBase::Grassland, None),
            );
        }
    }
    for y in 33..=40 {
        for x in 6..=40 {
            world.tile((x, y), TilePreset::new(Terrain::Coast, TerrainBase::Coast));
        }
    }

    // Strategic resources on valid native tiles directly adjacent to each capital/core city.
    let resources = [
        ((9, 22), Terrain::Desert, TerrainBase::Desert, Resource::Oil),
        (
            (11, 22),
            Terrain::Forest,
            TerrainBase::Grassland,
            Resource::Rubber,
        ),
        (
            (10, 20),
            Terrain::Hills,
            TerrainBase::Grassland,
            Resource::Aluminum,
        ),
        (
            (10, 24),
            Terrain::Mountains,
            TerrainBase::Grassland,
            Resource::Uranium,
        ),
        (
            (9, 28),
            Terrain::Hills,
            TerrainBase::Grassland,
            Resource::Iron,
        ),
        (
            (11, 28),
            Terrain::Hills,
            TerrainBase::Grassland,
            Resource::Coal,
        ),
        (
            (10, 30),
            Terrain::Desert,
            TerrainBase::Desert,
            Resource::Saltpeter,
        ),
        (
            (35, 22),
            Terrain::Desert,
            TerrainBase::Desert,
            Resource::Oil,
        ),
        (
            (33, 22),
            Terrain::Forest,
            TerrainBase::Grassland,
            Resource::Rubber,
        ),
        (
            (34, 20),
            Terrain::Hills,
            TerrainBase::Grassland,
            Resource::Aluminum,
        ),
        (
            (34, 24),
            Terrain::Mountains,
            TerrainBase::Grassland,
            Resource::Uranium,
        ),
        (
            (35, 28),
            Terrain::Hills,
            TerrainBase::Grassland,
            Resource::Iron,
        ),
        (
            (33, 28),
            Terrain::Hills,
            TerrainBase::Grassland,
            Resource::Coal,
        ),
        (
            (34, 30),
            Terrain::Desert,
            TerrainBase::Desert,
            Resource::Saltpeter,
        ),
    ];
    for (pos, terrain, base, resource) in resources {
        world.tile(pos, land_tile(terrain, base, Some(resource)));
    }

    world.completed_great_wonders();

    let rome_cities = [
        (world.city(ROME, (10, 22), 8), false),
        (world.city(ROME, (10, 28), 8), false),
        (world.city(ROME, (18, 22), 8), false),
        (world.city(ROME, (16, 32), 8), true),
    ];
    let germany_cities = [
        (world.city(GERMANY, (26, 22), 8), false),
        (world.city(GERMANY, (28, 32), 8), true),
        (world.city(GERMANY, (34, 22), 8), false),
        (world.city(GERMANY, (34, 28), 8), false),
    ];

    for (idx, (city_id, coastal)) in rome_cities
        .into_iter()
        .enumerate()
        .chain(germany_cities.into_iter().enumerate())
    {
        world.city_workers(city_id, 6);
        let mut buildings = vec![
            Building::BARRACKS,
            Building::FACTORY,
            Building::AIRPORT,
            Building::MARKETPLACE,
        ];
        if idx == 0 {
            buildings.push(Building::PALACE);
        }
        if coastal {
            buildings.push(Building::HARBOR);
        }
        if city_id == germany_cities[3].0 {
            buildings.push(Building::SDI_DEFENSE);
        }
        world.city_buildings(city_id, &buildings);
    }

    for owner in [ROME, GERMANY] {
        world.government(owner, Government::COMMUNISM);
        world.treasury(owner, 2500);
        world.tax_rates(owner, [20, 60, 20]);
    }
    world.diplomacy(ROME, GERMANY, Pact::War, 200, [-5000, -5000]);

    for y in 12..=40 {
        for x in 4..=42 {
            world.mark_known(ROME, (x, y));
            world.mark_known(GERMANY, (x, y));
        }
    }

    // Garrisons for every city.
    for &(pos, count) in &[((10, 22), 1), ((10, 28), 1), ((18, 22), 2), ((16, 32), 2)] {
        for _ in 0..count {
            world.unit(ROME, UnitKind::MechInfantry, pos);
        }
    }
    for &(pos, count) in &[((26, 22), 2), ((28, 32), 2), ((34, 22), 1), ((34, 28), 1)] {
        for _ in 0..count {
            world.unit(GERMANY, UnitKind::MechInfantry, pos);
        }
    }

    // Frontline armor (with elite units for Great Leader eligibility), artillery, air, and paratroopers.
    for (owner, front, rear) in [(ROME, (18, 22), (10, 22)), (GERMANY, (26, 22), (34, 22))] {
        for i in 0..6 {
            let armor = world.unit(owner, UnitKind::ModernArmor, front);
            if i < 4 {
                world.legacy_combat_rank(armor, ELITE_RANK);
            }
        }
        for _ in 0..3 {
            world.unit(owner, UnitKind::RadarArtillery, front);
        }
        for _ in 0..2 {
            world.unit(owner, UnitKind::StealthBomber, front);
            world.unit(owner, UnitKind::JetFighter, front);
            world.unit(owner, UnitKind::CruiseMissile, front);
        }
        world.unit(owner, UnitKind::Paratrooper, front);
        world.unit(owner, UnitKind::TacticalNuke, front);
        world.unit(owner, UnitKind::ICBM, rear);
    }

    // One pre-loaded Army (Rome) and one empty Army (Germany) to observe both behaviors.
    let rome_army = world.unit(ROME, UnitKind::Army, (18, 22));
    for _ in 0..3 {
        let member = world.unit(ROME, UnitKind::ModernArmor, (18, 22));
        world.load_cargo(member, rome_army);
    }
    let germany_army = world.unit(GERMANY, UnitKind::Army, (26, 22));

    // Naval + Cruise Missile + Carrier task forces in the southern sea lane.
    for (owner, sub_pos, carrier_pos, bb_pos) in [
        (ROME, (16, 34), (14, 36), (18, 34)),
        (GERMANY, (26, 34), (28, 36), (24, 34)),
    ] {
        let sub = world.unit(owner, UnitKind::NuclearSubmarine, sub_pos);
        let sub_nuke = world.unit(owner, UnitKind::TacticalNuke, sub_pos);
        world.load_cargo(sub_nuke, sub);
        let carrier = world.unit(owner, UnitKind::Carrier, carrier_pos);
        let plane = world.unit(owner, UnitKind::JetFighter, carrier_pos);
        world.load_cargo(plane, carrier);
        world.unit(owner, UnitKind::Battleship, bb_pos);
        world.unit(owner, UnitKind::AegisCruiser, bb_pos);
    }

    world.finish();

    (
        game,
        Box::new(ModernCombinedArms {
            rome_army,
            germany_army,
            rome_army_origin: (18, 22),
            rome_army_engaged: false,
            germany_army_loaded: false,
            total_kills_seen: 0,
        }),
    )
}

struct ModernCombinedArms {
    rome_army: u32,
    germany_army: u32,
    rome_army_origin: (i32, i32),
    rome_army_engaged: bool,
    germany_army_loaded: bool,
    total_kills_seen: i32,
}

impl ScenarioRun for ModernCombinedArms {
    fn observe_turn(&mut self, game: &Simulation, _turn: u32) -> Value {
        let kills = game.combat_kills(ROME) + game.combat_kills(GERMANY);
        self.total_kills_seen = self.total_kills_seen.max(kills);

        let rome_army_pos = game.unit_by_id(self.rome_army).map(|u| (u.x, u.y));
        if rome_army_pos != Some(self.rome_army_origin) {
            self.rome_army_engaged = true;
        }
        let germany_army_members = game
            .units()
            .iter()
            .filter(|u| u.transport == Some(self.germany_army))
            .count();
        if germany_army_members > 0 {
            self.germany_army_loaded = true;
        }
        let rome_cities = game.cities().iter().filter(|c| c.owner == ROME).count();
        let germany_cities = game.cities().iter().filter(|c| c.owner == GERMANY).count();
        json!({
            "rome_cities": rome_cities,
            "germany_cities": germany_cities,
            "combat_kills": kills,
            "rome_army_pos": rome_army_pos,
            "germany_army_members": germany_army_members,
        })
    }

    fn finish(self: Box<Self>, _completed_turns: u32) -> Vec<Check> {
        let mut checks = Checks::default();
        checks.push((
            "combined_arms_combat_occurred",
            self.total_kills_seen > 0,
            format!("observed {} total combat kills", self.total_kills_seen),
        ));
        checks.push((
            "loaded_army_engaged",
            self.rome_army_engaged,
            "pre-loaded Army moved or entered combat during the war".into(),
        ));
        checks.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_combined_arms_setup_succeeds() {
        let (game, _run) = setup(crate::test_support::seeded_map());
        assert_eq!(game.cities().len(), 8);
        assert!(game.units().iter().any(|u| u.kind == UnitKind::Army));
        assert!(game.units().iter().any(|u| u.kind == UnitKind::ICBM));
        for owner in [ROME, GERMANY] {
            let roles = game.unlocked_military_roles(owner);
            for expected in [
                "attacker",
                "defender",
                "bombard",
                "fighter",
                "bomber",
                "missile",
                "nuclear",
                "naval_combat",
            ] {
                assert!(
                    roles.contains(&expected),
                    "owner {owner} missing buildable role {expected}; got {roles:?}"
                );
            }
        }
    }
}
