//! Advisor-pass state and its deliberately longer-lived reinforcement routes.

use super::advisor_threats::ThreatCaches;
use crate::sim::{City, Simulation, Unit, list_order::ListOrder};
use crate::world::topology::MapWrap;
use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};

mod memos;
mod spatial;

#[derive(Clone, Default)]
pub(in crate::sim) struct AdvisorCaches {
    active: Cell<bool>,
    spatial: RefCell<spatial::SpatialSnapshot>,
    memos: memos::AdvisorMemos,
    threats: ThreatCaches,
}

impl AdvisorCaches {
    pub(in crate::sim) fn active(&self) -> bool {
        self.active.get()
    }

    pub(super) fn begin(
        &self,
        units: &[Unit],
        cities: &[City],
        topology: MapWrap,
        order: &ListOrder,
        continents: Arc<[usize]>,
        sea_regions: Arc<[usize]>,
    ) {
        self.threats.begin();
        // Founder-phase memoizations keep their existing selective lifetime.
        self.memos.clear_invasion_support();
        *self.spatial.borrow_mut() =
            spatial::SpatialSnapshot::new(units, cities, topology, order, continents, sea_regions);
        self.active.set(true);
    }

    fn discard_advisor_data(&self) {
        self.active.set(false);
        *self.spatial.borrow_mut() = Default::default();
        self.memos.clear();
    }

    pub(super) fn finish(&self) {
        self.discard_advisor_data();
        self.threats.clear();
    }

    pub(super) fn finish_preserving_threats(&self) {
        self.discard_advisor_data();
        self.threats.preserve();
    }

    pub(super) fn clear_threats(&self) {
        self.threats.clear();
    }

    pub(in crate::sim) fn clear_founder_maps(&mut self) {
        self.memos.clear_founder_maps();
    }

    pub(in crate::sim) fn memos(&self) -> &memos::AdvisorMemos {
        &self.memos
    }

    pub(super) fn threats(&self) -> &ThreatCaches {
        &self.threats
    }

    pub(in crate::sim) fn any_unit_at(
        &self,
        tile: usize,
        units: &[Unit],
        predicate: impl Fn(&Unit) -> bool,
    ) -> bool {
        self.spatial.borrow().any_unit_at(tile, units, predicate)
    }

    pub(in crate::sim) fn city_index_at(&self, tile: usize) -> Option<usize> {
        self.spatial.borrow().city_index_at(tile)
    }

    pub(in crate::sim) fn continent_labels(&self) -> Arc<[usize]> {
        self.spatial.borrow().continent_labels()
    }

    pub(in crate::sim) fn sea_region_labels(&self, city_count: usize) -> Option<Arc<[usize]>> {
        self.spatial.borrow().sea_region_labels(city_count)
    }

    pub(in crate::sim) fn ordered_tile_units<'a>(
        &self,
        tile: usize,
        revision: u64,
        units: &'a [Unit],
    ) -> Option<Vec<&'a Unit>> {
        self.spatial
            .borrow()
            .ordered_tile_units(tile, revision, units)
    }

    pub(in crate::sim) fn ordered_owner_units<'a>(
        &self,
        owner: usize,
        revision: u64,
        units: &'a [Unit],
    ) -> Option<Vec<&'a Unit>> {
        self.spatial
            .borrow()
            .ordered_owner_units(owner, revision, units)
    }
}

impl Simulation {
    pub(in crate::sim) fn begin_ai_advisor_cache(&self) {
        self.advisor_cache.begin(
            &self.units,
            &self.cities,
            self.map_wrap,
            &self.list_order,
            self.continent_labels(),
            self.sea_region_labels(),
        );
    }

    pub(in crate::sim) fn end_ai_advisor_cache(&self) {
        self.advisor_cache.finish();
    }

    pub(in crate::sim) fn end_ai_advisor_cache_preserving_threat_routes(&self) {
        self.advisor_cache.finish_preserving_threats();
    }

    pub(in crate::sim) fn clear_ai_threat_route_cache(&self) {
        self.advisor_cache.clear_threats();
    }
}

#[cfg(test)]
mod tests;
