//! Reconstructible query caches, excluded from campaign persistence.
//!
//! Storage and invalidation belong to the cache owners. Callers supply current
//! identities, knowledge or complete movement descriptors instead of mutating maps.
//! Durable fog-of-war memory and scheduled AI decisions are not query caches.

use super::{City, Unit, pf_actions::MovementCache, research::ResearchCache};
mod entities;

#[derive(Clone, Default)]
pub(super) struct QueryCaches {
    entities: entities::EntityIndexes,
    research: ResearchCache,
    movement: MovementCache,
}

impl QueryCaches {
    pub(super) fn unit_index(&self, units: &[Unit], id: u32) -> Option<usize> {
        self.entities.unit_index(units, id)
    }
    pub(super) fn city_index(&self, cities: &[City], id: u32) -> Option<usize> {
        self.entities.city_index(cities, id)
    }
    pub(super) fn research(&self) -> &ResearchCache {
        &self.research
    }
    pub(super) fn movement(&self) -> &MovementCache {
        &self.movement
    }

    /// Discard derived data after restoration; authoritative state is untouched.
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests;
