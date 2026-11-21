//! Immutable locations for one advisor pass; list revisions guard ordered lookups.

use crate::sim::{City, Unit, list_order::ListOrder};
use crate::world::topology::MapWrap;
use std::sync::Arc;

#[derive(Clone, Default)]
pub(super) struct SpatialSnapshot {
    unit_head: Vec<Option<usize>>,
    unit_next: Vec<Option<usize>>,
    tile_units: Option<Vec<Vec<usize>>>,
    owner_units: Option<Vec<Vec<usize>>>,
    tile_revision: u64,
    city_at: Vec<Option<usize>>,
    continents: Arc<[usize]>,
    sea_regions: (usize, Arc<[usize]>),
}

impl SpatialSnapshot {
    pub(super) fn new(
        units: &[Unit],
        cities: &[City],
        topology: MapWrap,
        order: &ListOrder,
        continents: Arc<[usize]>,
        sea_regions: Arc<[usize]>,
    ) -> Self {
        let tile_count = topology.tile_count();
        let mut tile_units = vec![Vec::new(); tile_count];
        let mut owner_units: Vec<Vec<usize>> = Vec::new();
        let mut unit_head = vec![None; tile_count];
        let mut unit_next = vec![None; units.len()];
        for (index, unit) in units.iter().enumerate() {
            let tile = topology.index(unit.x, unit.y);
            tile_units[tile].push(index);
            if unit.owner >= owner_units.len() {
                owner_units.resize_with(unit.owner + 1, Vec::new);
            }
            owner_units[unit.owner].push(index);
            unit_next[index] = unit_head[tile];
            unit_head[tile] = Some(index);
        }
        for (tile, indices) in tile_units.iter_mut().enumerate() {
            if indices.len() <= 1 {
                continue;
            }
            let position = topology.position(tile);
            indices.sort_by_key(|&index| {
                std::cmp::Reverse(order.tile_unit_order_key(&units[index], position))
            });
        }
        for (owner, indices) in owner_units.iter_mut().enumerate() {
            if indices.len() <= 1 {
                continue;
            }
            indices.sort_by_key(|&index| {
                std::cmp::Reverse(order.owner_unit_order_key(&units[index], owner))
            });
        }
        let mut city_at = vec![None; tile_count];
        for (index, city) in cities.iter().enumerate() {
            city_at[topology.index(city.x, city.y)] = Some(index);
        }
        Self {
            unit_head,
            unit_next,
            tile_units: Some(tile_units),
            owner_units: Some(owner_units),
            tile_revision: order.tile_revision(),
            city_at,
            continents,
            sea_regions: (cities.len(), sea_regions),
        }
    }

    pub(super) fn any_unit_at(
        &self,
        tile: usize,
        units: &[Unit],
        predicate: impl Fn(&Unit) -> bool,
    ) -> bool {
        let mut next = self.unit_head[tile];
        while let Some(index) = next {
            if predicate(&units[index]) {
                return true;
            }
            next = self.unit_next[index];
        }
        false
    }

    pub(super) fn city_index_at(&self, tile: usize) -> Option<usize> {
        self.city_at[tile]
    }

    pub(super) fn continent_labels(&self) -> Arc<[usize]> {
        self.continents.clone()
    }

    pub(super) fn sea_region_labels(&self, city_count: usize) -> Option<Arc<[usize]>> {
        (self.sea_regions.0 == city_count).then(|| self.sea_regions.1.clone())
    }

    pub(super) fn ordered_tile_units<'a>(
        &self,
        tile: usize,
        revision: u64,
        units: &'a [Unit],
    ) -> Option<Vec<&'a Unit>> {
        if revision != self.tile_revision {
            return None;
        }
        Some(
            self.tile_units.as_ref()?[tile]
                .iter()
                .map(|&index| &units[index])
                .collect(),
        )
    }

    pub(super) fn ordered_owner_units<'a>(
        &self,
        owner: usize,
        revision: u64,
        units: &'a [Unit],
    ) -> Option<Vec<&'a Unit>> {
        if revision != self.tile_revision {
            return None;
        }
        let owners = self.owner_units.as_ref()?;
        Some(
            owners
                .get(owner)
                .map(|indices| indices.iter().map(|&index| &units[index]).collect())
                .unwrap_or_default(),
        )
    }
}
