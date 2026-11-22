//! Typed world preparation for deterministic fixtures.

use super::{CivPreset, WorldBuilder};
use crate::{
    rules::{Tech, civilizations},
    sim::{Base, MAX_CIVILIZATIONS, Simulation},
    world::{GeneratedMap, GeneratedTile, Resource, Terrain, TerrainBase},
};

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

/// Override selected surface fields while preserving all other generated state.
/// `Some(None)` removes a resource; `None` leaves it unchanged.
#[derive(Clone, Copy, Default)]
pub(crate) struct TileSurface {
    pub terrain: Option<Terrain>,
    pub resource: Option<Option<Resource>>,
    pub river: Option<bool>,
    pub road: Option<bool>,
    pub irrigation: Option<bool>,
    pub hut: Option<bool>,
}

impl TileSurface {
    fn apply(self, tile: &mut crate::sim::Tile) {
        if let Some(terrain) = self.terrain {
            tile.terrain = terrain;
        }
        if let Some(resource) = self.resource {
            tile.resource = resource;
        }
        if let Some(river) = self.river {
            tile.river = river;
        }
        if let Some(road) = self.road {
            tile.road = road;
        }
        if let Some(irrigation) = self.irrigation {
            tile.irrigation = irrigation;
        }
        if let Some(hut) = self.hut {
            tile.hut = hut;
        }
    }
}

impl WorldBuilder<'_> {
    /// Apply a partial surface preset to one tile.
    pub(crate) fn tile_surface(&mut self, position: (i32, i32), surface: TileSurface) {
        let tile = self.game.tile_mut(position.0, position.1);
        self.river_edges_dirty |= surface.river.is_some_and(|river| tile.river != river)
            || surface
                .terrain
                .is_some_and(|terrain| tile.terrain.is_water() != terrain.is_water());
        surface.apply(tile);
    }

    /// Apply a partial surface preset across the map without changing its topology.
    pub(crate) fn map_surface(&mut self, surface: TileSurface) {
        for tile in &mut self.game.tiles {
            self.river_edges_dirty |= surface.river.is_some_and(|river| tile.river != river)
                || surface
                    .terrain
                    .is_some_and(|terrain| tile.terrain.is_water() != terrain.is_water());
            surface.apply(tile);
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
        self.start_turn(turn);
        self.game.year = year;
    }

    /// Preset the turn counter while preserving the generated calendar year.
    pub(crate) fn start_turn(&mut self, turn: i32) {
        assert!(
            (1..=crate::sim::score::TURN_LIMIT).contains(&turn),
            "invalid scenario start turn {turn}"
        );
        self.game.turn = turn;
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
        let tile = self.game.tile_mut(position.0, position.1);
        tile.known_by.set(owner, true);
        if owner == crate::sim::HUMAN {
            tile.seen = true;
        }
    }

    /// Clear starting entities and relationships for an isolated action case.
    pub(crate) fn clear_campaign_entities(&mut self) {
        self.game.units.clear();
        self.game.cities.clear();
        self.game.diplomacy.clear();
        self.game.vassalage = Default::default();
        self.game.ai_war_plans.clear();
        for player in &mut self.game.players {
            player.ai = false;
        }
    }

    /// Empty grassland for isolated unit and city actions; retain the RNG and roster.
    pub(crate) fn grassland_campaign(&mut self, clear_contamination: bool) {
        self.game.units.clear();
        self.game.cities.clear();
        for tile in &mut self.game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = false;
            tile.hut = false;
            tile.base = None;
            if clear_contamination {
                tile.pollution = false;
                tile.fallout = false;
            }
        }
    }

    pub(crate) fn tile_owner(&mut self, position: (i32, i32), owner: Option<usize>) {
        self.game.tile_mut(position.0, position.1).owner = owner;
    }

    /// Fertile surface for deterministic production and expansion fixtures.
    pub(crate) fn fertile_tiles(&mut self, owner: Option<usize>, known_owner: Option<usize>) {
        for tile in &mut self.game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = Some(Resource::Wheat);
            tile.hut = false;
            tile.owner = owner;
            if let Some(owner) = known_owner {
                tile.known_by.set(owner, true);
            }
        }
    }

    /// Rebuild territory after cities and their culture are preset, so the
    /// initial snapshot and the first turn's decisions see the borders that
    /// culture implies rather than those from founding.
    pub(crate) fn settle_borders(&mut self) {
        self.game.rebuild_city_ownership();
    }

    pub(crate) fn terrain(&mut self, position: (i32, i32), terrain: Terrain) {
        self.game.tile_mut(position.0, position.1).terrain = terrain;
    }

    /// Clear all terrain modifiers for isolated combat comparisons.
    pub(crate) fn combat_world(&mut self) {
        self.clear_units_and_cities();
        for tile in &mut self.game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
            tile.base = None;
            tile.resource = None;
            tile.hut = false;
        }
    }

    pub(crate) fn legacy_random_seed(&mut self, seed: u64) {
        self.game.native_random = None;
        self.game.rng = seed;
    }

    pub(crate) fn clear_outcome(&mut self) {
        self.game.game_over = None;
    }

    pub(crate) fn refresh_visibility(&mut self) {
        self.game.refresh_visibility();
    }
}

pub(crate) fn capture_known_tiles(sim: &mut Simulation, owner: usize) {
    let mut observed = vec![vec![false; sim.tiles.len()]; owner + 1];
    for (index, tile) in sim.tiles.iter().enumerate() {
        observed[owner][index] = tile.known_by[owner];
    }
    sim.remember_render_map(&observed);
}
