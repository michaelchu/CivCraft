//! Neutral world data shared by map generation and campaign simulation.
//!
//! These values describe generated geography without ownership, visibility,
//! improvements, or other mutable campaign state. Keeping the output contract
//! here lets map generation and simulation depend on shared data without
//! depending on each other's modules.

use crate::random::FreecivRng;

mod terrain;
pub(crate) mod topology;

pub(crate) use terrain::{Resource, Terrain, TerrainBase};

/// Default number of active civilizations in a standard generated world.
pub(crate) const DEFAULT_CIVILIZATION_COUNT: usize = 6;
pub(crate) const TILES_PER_PLAYER: i32 = 100;
pub(crate) const PERCENT_SCALE: i32 = 100;

/// Compact map-generation output, independent of campaign ownership,
/// visibility, and improvement state.
pub(crate) struct GeneratedMap {
    pub(crate) seed: u64,
    pub(crate) map_wrap: topology::MapWrap,
    pub(crate) game_random: FreecivRng,
    pub(crate) tiles: Vec<GeneratedTile>,
    pub(crate) starts: Vec<(i32, i32)>,
    pub(crate) civilization_count: usize,
}

/// One generated tile before campaign-only state is initialized.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct GeneratedTile {
    pub(crate) terrain: Terrain,
    pub(crate) base_terrain: TerrainBase,
    pub(crate) altitude: i32,
    pub(crate) hut: bool,
    pub(crate) river: bool,
    pub(crate) river_edges: u8,
    pub(crate) resource: Option<Resource>,
}
