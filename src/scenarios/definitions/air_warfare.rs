//! Civ III: Conquests air warfare & counter-air causal validation scenarios.
//!
//! Three runtime scenarios prove both mechanics and AI decision quality:
//! - `air-counter-defense`: defender faces active enemy Bombers/Fighters and
//!   must build `SAM_BATTERY` / counter-air units in response to observed raids.
//! - `air-counter-defense-control`: identical world/techs/war with ground-only
//!   enemy forces; defender must NOT waste shields on `SAM_BATTERY` or AA units.
//! - `air-warfare`: two modern AI powers fight a full air campaign exercising
//!   base-only stationing, bombardment/precision bombing, Air Superiority CAP,
//!   passive SAM/AA defense, forward rebasing, recon, and air production.

use super::*;
use crate::{
    rules::{Building, Tech},
    sim::{
        Government, Pact, Production, Simulation, UnitKind, UnitOrder,
        fixtures::{CivPreset, TilePreset, WorldBuilder},
    },
    world::{GeneratedMap, Resource, Terrain, TerrainBase},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const ROME: usize = 0;
const GERMANY: usize = 1;

pub(super) const COUNTER_DEFENSE: Scenario = Scenario::new(
    "air-counter-defense",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup: setup_counter_defense_treatment,
    },
);

pub(super) const COUNTER_DEFENSE_CONTROL: Scenario = Scenario::new(
    "air-counter-defense-control",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup: setup_counter_defense_control,
    },
);

pub(super) const WARFARE: Scenario = Scenario::new(
    "air-warfare",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup: setup_air_warfare,
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

fn setup_modern_continent(map: GeneratedMap, start_turn: i32) -> Simulation {
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
    world.start_at(start_turn, 1985);
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
    ];
    for (pos, terrain, base, resource) in resources {
        world.tile(pos, land_tile(terrain, base, Some(resource)));
    }
    world.completed_great_wonders();
    game
}

fn setup_counter_defense_treatment(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    setup_counter_defense(map, true)
}

fn setup_counter_defense_control(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    setup_counter_defense(map, false)
}

fn setup_counter_defense(
    map: GeneratedMap,
    enemy_has_air: bool,
) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = setup_modern_continent(map, 250);
    {
        let mut world = WorldBuilder::new(&mut game);
        // Rome (defender): 3 developed cities without SAM Batteries yet.
        let rome_cities = [
            world.city(ROME, (10, 22), 8),
            world.city(ROME, (18, 22), 8),
            world.city(ROME, (18, 28), 8),
        ];
        // Germany (attacker): 2 cities within Bomber range (distance 6 from (18,22)).
        let germany_cities = [
            world.city(GERMANY, (24, 22), 8),
            world.city(GERMANY, (34, 22), 8),
        ];

        for (idx, city_id) in rome_cities.iter().copied().enumerate() {
            world.city_workers(city_id, 6);
            let mut buildings = vec![
                Building::BARRACKS,
                Building::FACTORY,
                Building::AIRPORT,
                Building::MARKETPLACE,
                Building::LIBRARY,
            ];
            if idx == 0 {
                buildings.push(Building::PALACE);
            }
            world.city_buildings(city_id, &buildings);
        }
        for (idx, city_id) in germany_cities.iter().copied().enumerate() {
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
            world.city_buildings(city_id, &buildings);
        }

        for owner in [ROME, GERMANY] {
            world.government(owner, Government::COMMUNISM);
            world.treasury(owner, 2500);
            world.tax_rates(owner, [40, 40, 20]);
        }
        world.diplomacy(ROME, GERMANY, Pact::War, 200, [-5000, -5000]);

        // Strong Roman garrisons so Rome is NOT in a grave ground-conquest panic.
        for pos in [(10, 22), (18, 22), (18, 28)] {
            for _ in 0..3 {
                world.unit(ROME, UnitKind::MechInfantry, pos);
            }
        }
        for pos in [(24, 22), (34, 22)] {
            for _ in 0..3 {
                world.unit(GERMANY, UnitKind::MechInfantry, pos);
            }
        }

        if enemy_has_air {
            // Treatment: Germany fields active Bombers & Fighters in range of Rome.
            for _ in 0..4 {
                world.unit(GERMANY, UnitKind::Bomber, (24, 22));
            }
            for _ in 0..2 {
                world.unit(GERMANY, UnitKind::JetFighter, (24, 22));
            }
        } else {
            // Control: equivalent ground forces, zero enemy aircraft.
            for _ in 0..2 {
                world.unit(GERMANY, UnitKind::Tank, (34, 22));
            }
        }

        for y in 14..=32 {
            for x in 6..=40 {
                world.mark_known(ROME, (x, y));
                world.mark_known(GERMANY, (x, y));
            }
        }
        world.finish();
    }

    let initial_rome_units: BTreeSet<u32> = game.units_owned_by(ROME).map(|u| u.id).collect();
    let rome_city_ids: Vec<u32> = game.cities_owned_by(ROME).map(|c| c.id).collect();
    (
        game,
        Box::new(CounterDefenseRun {
            enemy_has_air,
            rome_city_ids,
            initial_rome_units,
            enemy_air_raids_observed: false,
            sam_queued_or_built: false,
            aa_or_cap_queued_or_built: false,
            off_base_aircraft_seen: false,
            sam_cities: BTreeSet::new(),
            counter_air_productions: BTreeSet::new(),
        }),
    )
}

struct CounterDefenseRun {
    enemy_has_air: bool,
    rome_city_ids: Vec<u32>,
    initial_rome_units: BTreeSet<u32>,
    enemy_air_raids_observed: bool,
    sam_queued_or_built: bool,
    aa_or_cap_queued_or_built: bool,
    off_base_aircraft_seen: bool,
    sam_cities: BTreeSet<u32>,
    counter_air_productions: BTreeSet<String>,
}

impl ScenarioRun for CounterDefenseRun {
    fn observe_turn(&mut self, game: &Simulation, _turn: u32) -> Value {
        if game.observed_air_raid_turn(ROME, GERMANY).is_some()
            || self
                .rome_city_ids
                .iter()
                .any(|&cid| game.city_bombed_turn(cid).is_some())
        {
            self.enemy_air_raids_observed = true;
        }
        for unit in game.units() {
            if unit.kind.is_fixed_wing_aircraft() && !game.aircraft_on_base(unit) {
                self.off_base_aircraft_seen = true;
            }
        }
        for city in game.cities_owned_by(ROME) {
            if city.has_building(Building::SAM_BATTERY)
                || city.production == Production::Building(Building::SAM_BATTERY)
            {
                self.sam_queued_or_built = true;
                self.sam_cities.insert(city.id);
                self.counter_air_productions
                    .insert("SAM Battery".to_owned());
            }
            if matches!(
                city.production,
                Production::Unit(
                    UnitKind::Flak
                        | UnitKind::MobileSAM
                        | UnitKind::Fighter
                        | UnitKind::JetFighter
                        | UnitKind::F15
                        | UnitKind::StealthFighter
                )
            ) {
                self.aa_or_cap_queued_or_built = true;
                self.counter_air_productions
                    .insert(city.production.name().to_owned());
            }
        }
        for unit in game.units_owned_by(ROME) {
            if !self.initial_rome_units.contains(&unit.id)
                && matches!(
                    unit.kind,
                    UnitKind::Flak
                        | UnitKind::MobileSAM
                        | UnitKind::Fighter
                        | UnitKind::JetFighter
                        | UnitKind::F15
                        | UnitKind::StealthFighter
                )
            {
                self.aa_or_cap_queued_or_built = true;
                self.counter_air_productions
                    .insert(unit.kind.name().to_owned());
            }
        }
        json!({
            "enemy_air_raids_observed": self.enemy_air_raids_observed,
            "sam_queued_or_built": self.sam_queued_or_built,
            "aa_or_cap_queued_or_built": self.aa_or_cap_queued_or_built,
            "counter_air_productions": self.counter_air_productions,
            "off_base_aircraft_seen": self.off_base_aircraft_seen,
        })
    }

    fn finish(self: Box<Self>, _completed_turns: u32) -> Vec<Check> {
        let mut checks = Checks::default();
        checks.push((
            "fixed_wing_stayed_on_bases",
            !self.off_base_aircraft_seen,
            "all fixed-wing aircraft ended every turn stationed on a valid base".into(),
        ));
        if self.enemy_has_air {
            checks.push((
                "enemy_air_raids_occurred",
                self.enemy_air_raids_observed,
                "German fixed-wing aircraft launched air raids against Roman targets".into(),
            ));
            checks.push((
                "defender_built_sam_battery",
                self.sam_queued_or_built,
                format!(
                    "Rome queued/built SAM Battery in {} city/cities ({:?})",
                    self.sam_cities.len(),
                    self.counter_air_productions
                ),
            ));
            checks.push((
                "defender_built_or_queued_counter_air",
                self.sam_queued_or_built || self.aa_or_cap_queued_or_built,
                format!("counter-air responses: {:?}", self.counter_air_productions),
            ));
        } else {
            checks.push((
                "no_enemy_air_raids",
                !self.enemy_air_raids_observed,
                "control scenario had zero enemy air raids".into(),
            ));
            checks.push((
                "no_wasted_sam_or_aa_in_ground_war",
                !self.sam_queued_or_built
                    && !self.counter_air_productions.contains("Flak")
                    && !self.counter_air_productions.contains("Mobile SAM"),
                format!(
                    "control counter-air productions (expected no SAM/AA): {:?}",
                    self.counter_air_productions
                ),
            ));
        }
        checks.0
    }
}

fn setup_air_warfare(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = setup_modern_continent(map, 250);
    let mut initial_aircraft_pos = BTreeMap::new();
    let mut initial_improvements = 0usize;
    let mut initial_aircraft_ids = BTreeSet::new();
    let mut initial_city_ids = Vec::new();
    {
        let mut world = WorldBuilder::new(&mut game);
        // Rome: rear airbase at (10,22) (distance 14 > Bomber range 10 from (24,22),
        // requiring forward Rebase to (18,22)), plus frontline (18,22) and coastal (16,32).
        let rome_rear = world.city(ROME, (10, 22), 8);
        let rome_front = world.city(ROME, (18, 22), 8);
        let rome_port = world.city(ROME, (16, 32), 8);
        // Germany: frontline (24,22) with SAM Battery, coastal (28,32), rear (34,22).
        let germ_front = world.city(GERMANY, (24, 22), 8);
        let germ_port = world.city(GERMANY, (28, 32), 8);
        let germ_rear = world.city(GERMANY, (34, 22), 8);

        for (city_id, palace, coastal, has_sam) in [
            (rome_rear, true, false, false),
            (rome_front, false, false, true),
            (rome_port, false, true, false),
            (germ_front, false, false, true),
            (germ_port, false, true, false),
            (germ_rear, true, false, false),
        ] {
            world.city_workers(city_id, 6);
            let mut buildings = vec![
                Building::BARRACKS,
                Building::FACTORY,
                Building::AIRPORT,
                Building::MARKETPLACE,
                Building::LIBRARY,
                Building::UNIVERSITY,
            ];
            if palace {
                buildings.push(Building::PALACE);
            }
            if coastal {
                buildings.push(Building::HARBOR);
            }
            if has_sam {
                buildings.push(Building::SAM_BATTERY);
            }
            world.city_buildings(city_id, &buildings);
        }

        for owner in [ROME, GERMANY] {
            world.government(owner, Government::COMMUNISM);
            world.treasury(owner, 3000);
            world.tax_rates(owner, [40, 40, 20]);
        }
        world.diplomacy(ROME, GERMANY, Pact::War, 200, [-5000, -5000]);

        // Garrisons + AA units on both frontlines.
        for pos in [(10, 22), (18, 22), (16, 32)] {
            for _ in 0..2 {
                world.unit(ROME, UnitKind::MechInfantry, pos);
            }
        }
        world.unit(ROME, UnitKind::MobileSAM, (18, 22));
        world.unit(ROME, UnitKind::ModernArmor, (20, 22));
        world.unit(ROME, UnitKind::ModernArmor, (20, 24));

        for pos in [(24, 22), (28, 32), (34, 22)] {
            for _ in 0..2 {
                world.unit(GERMANY, UnitKind::MechInfantry, pos);
            }
        }
        world.unit(GERMANY, UnitKind::MobileSAM, (24, 22));
        world.unit(GERMANY, UnitKind::Flak, (24, 22));
        world.unit(GERMANY, UnitKind::ModernArmor, (22, 22));
        world.unit(GERMANY, UnitKind::ModernArmor, (22, 24));

        // Rear bombers that must REBASE forward (distance 14 from (10,22) to (24,22) > range 10).
        for _ in 0..2 {
            world.unit(ROME, UnitKind::Bomber, (10, 22));
            world.unit(GERMANY, UnitKind::Bomber, (34, 22));
        }
        // Frontline fighters (for CAP + Recon) and Stealth Bombers (Precision Bombing).
        for _ in 0..3 {
            world.unit(ROME, UnitKind::JetFighter, (18, 22));
            world.unit(GERMANY, UnitKind::JetFighter, (24, 22));
        }
        for _ in 0..2 {
            world.unit(ROME, UnitKind::StealthBomber, (18, 22));
            world.unit(GERMANY, UnitKind::StealthBomber, (24, 22));
        }
        // Carrier with embarked JetFighter on the southern sea lane.
        let carrier = world.unit(ROME, UnitKind::Carrier, (20, 35));
        let deck_fighter = world.unit(ROME, UnitKind::JetFighter, (20, 35));
        world.load_cargo(deck_fighter, carrier);

        // Reveal the frontline cities, while leaving outer enemy tiles fogged
        // or unexplored so Recon missions have real scouting targets.
        for y in 18..=36 {
            for x in 6..=26 {
                world.mark_known(ROME, (x, y));
            }
            for x in 16..=40 {
                world.mark_known(GERMANY, (x, y));
            }
        }
        world.finish();
    }

    for unit in game.units() {
        if unit.kind.is_fixed_wing_aircraft() {
            initial_aircraft_ids.insert(unit.id);
            initial_aircraft_pos.insert(unit.id, (unit.x, unit.y));
        }
    }
    for city in game.cities() {
        initial_city_ids.push(city.id);
        initial_improvements += city
            .buildings
            .iter()
            .filter(|b| b.data().genus == crate::rules::Genus::Improvement)
            .count();
    }

    (
        game,
        Box::new(AirWarfareRun {
            initial_aircraft_ids,
            initial_aircraft_pos,
            initial_improvements,
            initial_city_ids,
            off_base_aircraft_seen: false,
            cap_observed: false,
            rebase_observed: false,
            air_raid_observed: false,
            aircraft_loss_observed: false,
            improvements_destroyed_observed: false,
            air_or_counter_air_produced: false,
            combat_kills_peak: 0,
        }),
    )
}

struct AirWarfareRun {
    initial_aircraft_ids: BTreeSet<u32>,
    initial_aircraft_pos: BTreeMap<u32, (i32, i32)>,
    initial_improvements: usize,
    initial_city_ids: Vec<u32>,
    off_base_aircraft_seen: bool,
    cap_observed: bool,
    rebase_observed: bool,
    air_raid_observed: bool,
    aircraft_loss_observed: bool,
    improvements_destroyed_observed: bool,
    air_or_counter_air_produced: bool,
    combat_kills_peak: i32,
}

impl ScenarioRun for AirWarfareRun {
    fn observe_turn(&mut self, game: &Simulation, _turn: u32) -> Value {
        if game.observed_air_raid_turn(ROME, GERMANY).is_some()
            || game.observed_air_raid_turn(GERMANY, ROME).is_some()
        {
            self.air_raid_observed = true;
        }
        let mut live_initial_air = 0usize;
        for unit in game.units() {
            if !unit.kind.is_fixed_wing_aircraft() {
                continue;
            }
            if !game.aircraft_on_base(unit) {
                self.off_base_aircraft_seen = true;
            }
            if matches!(unit.order, Some(UnitOrder::AirSuperiority { .. })) {
                self.cap_observed = true;
            }
            if let Some(&orig) = self.initial_aircraft_pos.get(&unit.id) {
                live_initial_air += 1;
                if (unit.x, unit.y) != orig && unit.transport.is_none() {
                    self.rebase_observed = true;
                }
            } else {
                self.air_or_counter_air_produced = true;
            }
        }
        if live_initial_air < self.initial_aircraft_ids.len() {
            self.aircraft_loss_observed = true;
        }
        let mut current_improvements = 0usize;
        for &cid in &self.initial_city_ids {
            if let Some(city) = game.city_by_id(cid) {
                current_improvements += city
                    .buildings
                    .iter()
                    .filter(|b| b.data().genus == crate::rules::Genus::Improvement)
                    .count();
            }
        }
        if current_improvements < self.initial_improvements {
            self.improvements_destroyed_observed = true;
        }
        for city in game.cities() {
            if matches!(
                city.production,
                Production::Building(Building::SAM_BATTERY)
                    | Production::Unit(
                        UnitKind::Fighter
                            | UnitKind::JetFighter
                            | UnitKind::F15
                            | UnitKind::Bomber
                            | UnitKind::StealthFighter
                            | UnitKind::StealthBomber
                            | UnitKind::Flak
                            | UnitKind::MobileSAM
                    )
            ) {
                self.air_or_counter_air_produced = true;
            }
        }
        let kills = game.combat_kills(ROME) + game.combat_kills(GERMANY);
        self.combat_kills_peak = self.combat_kills_peak.max(kills);

        json!({
            "air_raid_observed": self.air_raid_observed,
            "cap_observed": self.cap_observed,
            "rebase_observed": self.rebase_observed,
            "aircraft_loss_observed": self.aircraft_loss_observed,
            "improvements_destroyed": self.improvements_destroyed_observed,
            "air_or_counter_air_produced": self.air_or_counter_air_produced,
            "combat_kills": kills,
        })
    }

    fn finish(self: Box<Self>, _completed_turns: u32) -> Vec<Check> {
        let mut checks = Checks::default();
        checks.push((
            "fixed_wing_stayed_on_bases",
            !self.off_base_aircraft_seen,
            "every fixed-wing aircraft stayed on a valid city, airfield, or carrier base".into(),
        ));
        checks.push((
            "air_raids_and_bombardment_occurred",
            self.air_raid_observed
                && (self.improvements_destroyed_observed || self.combat_kills_peak > 0),
            format!(
                "air_raid={}, improvements_destroyed={}, combat_kills={}",
                self.air_raid_observed,
                self.improvements_destroyed_observed,
                self.combat_kills_peak
            ),
        ));
        checks.push((
            "air_superiority_cap_deployed",
            self.cap_observed,
            "AI fighters entered Air Superiority CAP to defend threatened bases".into(),
        ));
        checks.push((
            "forward_rebase_executed",
            self.rebase_observed,
            "AI rear bombers rebased forward to put enemy targets in operational range".into(),
        ));
        checks.push((
            "air_defense_inflicted_aircraft_losses",
            self.aircraft_loss_observed,
            "CAP interceptions or passive SAM/AA defenses shot down attacking aircraft".into(),
        ));
        checks.push((
            "ai_produced_air_or_counter_air",
            self.air_or_counter_air_produced,
            "AI cities queued or built aircraft or counter-air defenses during the campaign".into(),
        ));
        checks.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn air_scenarios_setup_with_valid_modern_forces() {
        let (treatment, _) = setup_counter_defense_treatment(crate::test_support::seeded_map());
        let (control, _) = setup_counter_defense_control(crate::test_support::seeded_map());
        assert!(
            treatment
                .units_owned_by(GERMANY)
                .any(|u| u.kind.is_fixed_wing_aircraft())
        );
        assert!(
            !control
                .units_owned_by(GERMANY)
                .any(|u| u.kind.is_fixed_wing_aircraft())
        );
        let (warfare, _) = setup_air_warfare(crate::test_support::seeded_map());
        assert!(
            warfare
                .units()
                .iter()
                .any(|u| u.kind == UnitKind::StealthBomber)
        );
        assert!(
            warfare
                .units()
                .iter()
                .any(|u| u.kind == UnitKind::JetFighter)
        );
        assert!(
            warfare
                .units()
                .iter()
                .any(|u| u.kind == UnitKind::MobileSAM)
        );
    }
}
