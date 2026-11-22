//! Headless proof scenario for ICBM, Tactical Nuke, Cruise Missile, SDI
//! Defense interception, and nuclear fallout creation across normal AI turns.

use super::*;
use crate::{
    rules::{Building, Tech},
    sim::{
        Government, Pact, Simulation, UnitKind,
        fixtures::{CivPreset, TilePreset, WorldBuilder, capture_known_tiles},
    },
    world::{GeneratedMap, Terrain, TerrainBase},
};
use serde_json::{Value, json};

const ROME: usize = 0;
const GERMANY: usize = 1;

pub(super) const SCENARIO: Scenario = Scenario::new(
    "nuclear-warfare",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup,
    },
);

fn land() -> TilePreset {
    TilePreset {
        terrain: Terrain::Grassland,
        base_terrain: TerrainBase::Grassland,
        resource: None,
        river: false,
        road: true,
        railroad: false,
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
    world.start_at(260, 1990);

    for y in 16..=28 {
        for x in 8..=36 {
            world.tile((x, y), land());
        }
    }

    let rome_silo = (10, 22);
    let rome_front = (16, 22);
    let rome_sdi_city = (10, 18);
    let germany_front = (20, 22);
    let germany_target_city = (26, 22);
    let germany_sdi_city = (32, 22);

    let c_rome_silo = world.city(ROME, rome_silo, 10);
    let c_rome_front = world.city(ROME, rome_front, 8);
    let c_rome_sdi = world.city(ROME, rome_sdi_city, 16);
    let c_ger_front = world.city(GERMANY, germany_front, 8);
    let c_ger_target = world.city(GERMANY, germany_target_city, 14);
    let c_ger_sdi = world.city(GERMANY, germany_sdi_city, 12);

    world.city_buildings(c_rome_silo, &[Building::PALACE, Building::AIRPORT]);
    world.city_buildings(c_rome_front, &[Building::AIRPORT]);
    world.city_buildings(c_rome_sdi, &[Building::SDI_DEFENSE]);
    world.city_buildings(c_ger_front, &[Building::BARRACKS_III]);
    world.city_buildings(c_ger_target, &[Building::PALACE, Building::BARRACKS_III]);
    world.city_buildings(c_ger_sdi, &[Building::SDI_DEFENSE]);

    world.unit(ROME, UnitKind::MechInfantry, rome_silo);
    world.unit(ROME, UnitKind::MechInfantry, rome_front);
    world.unit(ROME, UnitKind::MechInfantry, rome_sdi_city);
    world.unit(GERMANY, UnitKind::MechInfantry, germany_front);
    world.unit(GERMANY, UnitKind::MechInfantry, germany_target_city);
    world.unit(GERMANY, UnitKind::MechInfantry, germany_sdi_city);
    // Forward enemy target within Cruise Missile bombard range (2 tiles from rome_front).
    world.unit(GERMANY, UnitKind::Tank, (18, 22));

    let rome_icbm = world.unit(ROME, UnitKind::ICBM, rome_silo);
    let rome_tactical = world.unit(ROME, UnitKind::TacticalNuke, rome_front);
    let rome_cruise = world.unit(ROME, UnitKind::CruiseMissile, rome_front);
    // Germany's ICBM picks Rome's largest city (rome_sdi_city, size 16) -> guaranteed SDI intercept!
    let germany_icbm = world.unit(GERMANY, UnitKind::ICBM, germany_sdi_city);

    for civ in [ROME, GERMANY] {
        world.government(civ, Government::COMMUNISM);
        world.treasury(civ, 1000);
        for y in 16..=28 {
            for x in 8..=36 {
                world.mark_known(civ, (x, y));
            }
        }
    }
    world.diplomacy(ROME, GERMANY, Pact::War, 200, [-5000, -5000]);
    world.finish();
    capture_known_tiles(&mut game, ROME);
    capture_known_tiles(&mut game, GERMANY);

    let run = NuclearWarfare {
        rome_icbm,
        rome_tactical,
        rome_cruise,
        germany_icbm,
        icbm_launched: false,
        tactical_nuke_launched: false,
        cruise_missile_fired: false,
        sdi_intercepted: false,
        fallout_tiles_seen: 0,
    };
    (game, Box::new(run))
}

struct NuclearWarfare {
    rome_icbm: u32,
    rome_tactical: u32,
    rome_cruise: u32,
    germany_icbm: u32,
    icbm_launched: bool,
    tactical_nuke_launched: bool,
    cruise_missile_fired: bool,
    sdi_intercepted: bool,
    fallout_tiles_seen: usize,
}

impl ScenarioRun for NuclearWarfare {
    fn observe_turn(&mut self, game: &Simulation, _turn: u32) -> Value {
        if game.unit_by_id(self.rome_icbm).is_none() {
            self.icbm_launched = true;
        }
        if game.unit_by_id(self.rome_tactical).is_none() {
            self.tactical_nuke_launched = true;
        }
        if game.unit_by_id(self.rome_cruise).is_none() {
            self.cruise_missile_fired = true;
        }
        if game.unit_by_id(self.germany_icbm).is_none()
            && game
                .city_at(10, 18)
                .is_some_and(|city| city.owner == ROME && city.size >= 12)
            && !game.tile(10, 18).fallout
        {
            self.sdi_intercepted = true;
        }
        let mut fallout_tiles = 0;
        for y in 16..=28 {
            for x in 8..=36 {
                if game.tile(x, y).fallout {
                    fallout_tiles += 1;
                }
            }
        }
        self.fallout_tiles_seen = self.fallout_tiles_seen.max(fallout_tiles);
        json!({
            "icbm_launched": self.icbm_launched,
            "tactical_nuke_launched": self.tactical_nuke_launched,
            "cruise_missile_fired": self.cruise_missile_fired,
            "sdi_intercepted": self.sdi_intercepted,
            "fallout_tiles": fallout_tiles,
        })
    }

    fn finish(self: Box<Self>, _completed_turns: u32) -> Vec<Check> {
        let mut checks = Checks::default();
        checks.push((
            "ai_icbm_launched",
            self.icbm_launched,
            "AI ICBM launched across the map".into(),
        ));
        checks.push((
            "ai_tactical_nuke_launched",
            self.tactical_nuke_launched,
            "AI Tactical Nuke launched at frontline enemy city".into(),
        ));
        checks.push((
            "ai_cruise_missile_fired",
            self.cruise_missile_fired,
            "AI Cruise Missile fired and expended itself".into(),
        ));
        checks.push((
            "nuclear_fallout_polluted_area",
            self.fallout_tiles_seen > 0,
            format!(
                "observed {} fallout tiles around nuclear blast(s)",
                self.fallout_tiles_seen
            ),
        ));
        checks.push((
            "sdi_defense_intercepted_icbm",
            self.sdi_intercepted,
            "SDI Defense intercepted incoming enemy ICBM".into(),
        ));
        checks.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nuclear_warfare_setup_allows_ai_launches() {
        let (game, _run) = setup(crate::test_support::seeded_map());
        let rome_icbm = game
            .units()
            .iter()
            .find(|u| u.owner == ROME && u.kind == UnitKind::ICBM)
            .unwrap();
        let rome_tact = game
            .units()
            .iter()
            .find(|u| u.owner == ROME && u.kind == UnitKind::TacticalNuke)
            .unwrap();
        let ger_icbm = game
            .units()
            .iter()
            .find(|u| u.owner == GERMANY && u.kind == UnitKind::ICBM)
            .unwrap();
        assert!(
            game.can_launch_nuclear_strike_at(rome_icbm, 26, 22),
            "rome icbm cannot launch at (26,22): moves={} known={} owner={:?} city={:?} avail={:?} at_war={}",
            rome_icbm.moves_left,
            game.tile(26, 22).known_by[ROME],
            game.tile(26, 22).owner,
            game.city_at(26, 22).map(|c| c.owner),
            game.action_available_for_unit(rome_icbm, "Nuke City", (26, 22)),
            game.at_war(ROME, GERMANY),
        );
        assert!(
            game.can_launch_nuclear_strike_at(rome_tact, 20, 22),
            "rome tactical cannot launch at (20,22): moves={} dist={}",
            rome_tact.moves_left,
            game.map_wrap
                .civ3_distance(rome_tact.x, rome_tact.y, 20, 22),
        );
        assert!(
            game.can_launch_nuclear_strike_at(ger_icbm, 10, 18),
            "germany icbm cannot launch at (10,18): moves={} known={}",
            ger_icbm.moves_left,
            game.tile(10, 18).known_by[GERMANY],
        );
    }
}
