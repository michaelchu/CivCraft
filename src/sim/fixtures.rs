//! Small typed helpers for constructing deterministic campaign fixtures.

use crate::{rules::Tech, sim::Simulation};

mod cities;
mod diplomacy;
mod economy;
mod readiness;
mod research;
mod units;
mod world;
pub(crate) use readiness::UnitReadiness;
pub(crate) use units::give_unit_full_movement;
pub(crate) use world::capture_known_tiles;

/// The stable catalog ID and starting knowledge for a civilization slot.
pub(crate) struct CivPreset<'a> {
    pub id: &'a str,
    /// None uses the catalog defaults. Some(&[]) starts with no technologies.
    pub technologies: Option<&'a [Tech]>,
}

pub(crate) use world::{TilePreset, TileSurface};

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
mod tests;
