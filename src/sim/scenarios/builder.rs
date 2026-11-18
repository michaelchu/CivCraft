//! Small typed helpers for constructing deterministic campaign fixtures.

use crate::{
    rules::{Building, Genus, Tech, civilizations},
    sim::{Base, Government, MAX_CIVILIZATIONS, Pact, Simulation, UnitKind},
    world::{GeneratedMap, GeneratedTile, Resource, Terrain, TerrainBase},
};

mod readiness;
pub(crate) use readiness::UnitReadiness;

/// The stable catalog ID and starting knowledge for a civilization slot.
pub(crate) struct CivPreset<'a> {
    pub id: &'a str,
    /// None uses the catalog defaults. Some(&[]) starts with no technologies.
    pub technologies: Option<&'a [Tech]>,
}

/// Terrain and infrastructure for one tile in the starting map.
pub(crate) struct TilePreset {
    pub terrain: Terrain,
    pub base_terrain: TerrainBase,
    pub resource: Option<Resource>,
    pub river: bool,
    pub road: bool,
    pub railroad: bool,
    pub base: Option<Base>,
}

impl TilePreset {
    pub(crate) fn new(terrain: Terrain, base_terrain: TerrainBase) -> Self {
        Self {
            terrain,
            base_terrain,
            resource: None,
            river: false,
            road: false,
            railroad: false,
            base: None,
        }
    }
}

pub(crate) struct WorldBuilder<'a> {
    game: &'a mut Simulation,
    river_edges_dirty: bool,
}

impl<'a> WorldBuilder<'a> {
    pub(crate) fn new(game: &'a mut Simulation) -> Self {
        Self {
            game,
            river_edges_dirty: false,
        }
    }

    /// Keep the seed and topology. Active slots match the requested order.
    pub(crate) fn ocean_campaign(mut map: GeneratedMap, civs: &[CivPreset<'_>]) -> Simulation {
        assert!(
            (1..=MAX_CIVILIZATIONS).contains(&civs.len()),
            "invalid scenario roster size"
        );
        let mut roster: Vec<(&civilizations::Civilization, &[Tech])> = Vec::new();
        for preset in civs {
            let civ = civilizations::by_id(preset.id)
                .unwrap_or_else(|| panic!("unknown scenario civilization {}", preset.id));
            assert!(
                !roster.iter().any(|(entry, _)| entry.id == civ.id),
                "duplicate scenario civilization {}",
                civ.id
            );
            let techs = preset.technologies.unwrap_or(&civ.starting_techs);
            for (index, tech) in techs.iter().enumerate() {
                assert!(
                    Tech::all().any(|candidate| candidate == *tech),
                    "invalid scenario technology {tech:?}"
                );
                assert!(
                    !techs[..index].contains(tech),
                    "duplicate scenario technology {tech:?}"
                );
            }
            roster.push((civ, techs));
        }
        // Keep inactive slots for the fixed barbarian slot and save layout.
        // Their empty knowledge keeps them out of global technology history.
        for civ in civilizations::all() {
            if roster.len() == MAX_CIVILIZATIONS {
                break;
            }
            if !roster.iter().any(|(entry, _)| entry.id == civ.id) {
                roster.push((civ, &[]));
            }
        }
        map.civilization_count = civs.len();
        map.starts.clear();
        map.tiles.fill(GeneratedTile {
            terrain: Terrain::Sea,
            base_terrain: TerrainBase::Sea,
            altitude: 0,
            hut: false,
            river: false,
            river_edges: 0,
            resource: None,
        });
        Simulation::new_with_roster(map, roster)
    }

    pub(crate) fn start_at(&mut self, turn: i32, year: i32) {
        assert!(
            (1..=super::super::score::TURN_LIMIT).contains(&turn),
            "invalid scenario start turn {turn}"
        );
        self.game.turn = turn;
        self.game.year = year;
    }

    pub(crate) fn grassland(&mut self, position: (i32, i32)) {
        let tile = self.game.tile_mut(position.0, position.1);
        self.river_edges_dirty |= tile.terrain.is_water();
        tile.terrain = Terrain::Grassland;
        tile.base_terrain = TerrainBase::Grassland;
    }

    /// Apply a starting tile preset before placing cities and units.
    pub(crate) fn tile(&mut self, position: (i32, i32), preset: TilePreset) {
        assert!(
            !preset.terrain.is_water()
                || !(preset.river || preset.road || preset.railroad || preset.base.is_some()),
            "land infrastructure requires land at {position:?}"
        );
        let tile = self.game.tile_mut(position.0, position.1);
        self.river_edges_dirty |=
            tile.river != preset.river || tile.terrain.is_water() != preset.terrain.is_water();
        tile.terrain = preset.terrain;
        tile.base_terrain = preset.base_terrain;
        tile.resource = preset.resource;
        tile.river = preset.river;
        tile.road = preset.road || preset.railroad;
        tile.railroad = preset.railroad;
        tile.base = preset.base;
    }

    pub(crate) fn mark_known(&mut self, owner: usize, position: (i32, i32)) {
        self.game
            .tile_mut(position.0, position.1)
            .known_by
            .set(owner, true);
    }

    pub(crate) fn city(&mut self, owner: usize, position: (i32, i32), size: i32) -> u32 {
        assert!(
            owner < self.game.civilization_count,
            "inactive scenario city owner {owner}"
        );
        assert!(size > 0, "scenario city size must be positive");
        assert!(
            !self.game.tile(position.0, position.1).terrain.is_water(),
            "scenario city must start on land"
        );
        assert!(
            self.game.city_at(position.0, position.1).is_none(),
            "scenario city tile already occupied"
        );
        self.game.create_city(owner, position.0, position.1);
        let city = self.game.cities.last_mut().expect("city was just created");
        city.size = size;
        city.specialists[2] = size;
        city.id
    }

    /// Install a regular city improvement already in place at the start.
    pub(crate) fn city_improvement(&mut self, city_id: u32, building: Building) {
        assert_eq!(
            building.data().genus,
            Genus::Improvement,
            "expected a regular city improvement"
        );
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city must exist");
        if let Some(reason) = self
            .game
            .building_blocker(&self.game.cities[index], building)
        {
            panic!(
                "cannot preset {} in city {city_id}: {reason}",
                building.data().name
            );
        }
        self.game.cities[index].buildings.push(building);
    }

    pub(crate) fn unit(&mut self, owner: usize, kind: UnitKind, position: (i32, i32)) -> u32 {
        assert!(
            owner < self.game.civilization_count || owner == crate::sim::BARBARIANS,
            "inactive scenario unit owner {owner}"
        );
        self.game.spawn_unit(owner, kind, position.0, position.1)
    }

    pub(crate) fn unit_experience_rank(&mut self, id: u32, rank: usize) {
        let unit = self.unit_mut(id);
        assert!(rank < crate::rules::native::experience().levels.len());
        unit.veteran = rank;
        unit.hp = unit.max_hit_points();
    }

    pub(crate) fn unit_hit_points(&mut self, id: u32, hit_points: i32) {
        let unit = self.unit_mut(id);
        assert!((1..=unit.max_hit_points()).contains(&hit_points));
        unit.hp = hit_points;
    }

    fn unit_mut(&mut self, id: u32) -> &mut crate::sim::Unit {
        let index = self
            .game
            .unit_index_by_id(id)
            .expect("fixture unit must exist");
        &mut self.game.units[index]
    }

    /// Seed the previous turn's position for AI policies that track movement.
    pub(crate) fn previous_unit_position(&mut self, id: u32, position: (i32, i32)) {
        assert!(
            self.game.unit_by_id(id).is_some(),
            "fixture unit must exist"
        );
        self.game.previous_naval_positions.insert(id, position);
    }

    /// Trust values are ordered by the two slots passed by the caller.
    pub(crate) fn diplomacy(
        &mut self,
        first: usize,
        second: usize,
        pact: Pact,
        first_contact: i32,
        trust: [i32; 2],
    ) {
        assert!(
            first != second
                && first < self.game.civilization_count
                && second < self.game.civilization_count,
            "diplomacy requires two active scenario civilizations"
        );
        assert!(
            (0..=self.game.turn).contains(&first_contact),
            "first contact cannot occur after the scenario starts"
        );
        self.game
            .set_scenario_diplomacy(first, second, pact, first_contact, trust);
    }

    /// Start under an unlocked government without scheduling a revolution.
    pub(crate) fn government(&mut self, owner: usize, government: Government) {
        assert!(
            owner < self.game.civilization_count,
            "inactive scenario government owner {owner}"
        );
        self.game.set_scenario_government(owner, government);
    }

    /// The holder has an embassy with the host, independent of the reverse direction.
    pub(crate) fn embassy(&mut self, holder: usize, host: usize) {
        self.game.grant_scenario_embassy(holder, host);
    }

    /// The receiver sees the tiles currently visible to the giver.
    pub(crate) fn shared_vision(&mut self, giver: usize, receiver: usize) {
        self.game.grant_scenario_vision(giver, receiver);
    }

    /// Establish a route between two existing city IDs without a delivery bonus.
    pub(crate) fn trade_route(&mut self, from: u32, to: u32, goods: &str) {
        assert!(
            self.game.establish_scenario_trade_route(from, to, goods),
            "cannot preset trade route {from} -> {to} for goods {goods:?}"
        );
    }

    /// Make the assembled world ready for the runner's initial snapshot.
    pub(crate) fn finish(self) {
        if self.river_edges_dirty {
            crate::world::topology::rebuild_civ3_river_edges(
                self.game.map_wrap,
                &mut self.game.tiles,
            );
        }
        self.game.refresh_visibility();
        self.game.refresh_score_snapshot();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_initialize_roster_knowledge_calendar_entities_and_diplomacy() {
        let mut game = WorldBuilder::ocean_campaign(
            crate::test_support::seeded_map(),
            &[
                CivPreset {
                    id: "england",
                    technologies: Some(&[Tech::COMBUSTION]),
                },
                CivPreset {
                    id: "germany",
                    technologies: Some(&[]),
                },
            ],
        );
        let mut world = WorldBuilder::new(&mut game);
        world.start_at(120, 1900);
        world.grassland((10, 24));
        let city = world.city(0, (10, 24), 3);
        let ship = world.unit(1, UnitKind::Battleship, (10, 25));
        world.diplomacy(1, 0, Pact::Peace, 42, [3000, 5000]);
        world.finish();

        assert_eq!(game.civilization_count, 2);
        assert_eq!(game.players[0].civilization_id.as_deref(), Some("england"));
        assert_eq!(game.players[1].civilization_id.as_deref(), Some("germany"));
        assert_eq!(game.players[0].known, [Tech::COMBUSTION]);
        assert!(game.players[1].known.is_empty());
        assert_eq!(game.global_advances, [Tech::COMBUSTION]);
        assert_ne!(game.players[0].researching, Tech::COMBUSTION);
        assert_eq!((game.turn, game.year), (120, 1900));
        assert_eq!(game.city_by_id(city).unwrap().founded_turn, 120);
        assert_eq!(game.nation_scores[0].score_population, 3);
        assert_eq!(game.unit_by_id(ship).unwrap().owner, 1);
        assert_eq!(game.pact(0, 1), Pact::Peace);
        assert_eq!(game.diplomacy[&(0, 1)].trust, [5000, 3000]);
    }
}
