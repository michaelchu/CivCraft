//! Index positions are hints: validate their persistent IDs on every lookup.
use super::super::{City, Unit};
use std::{cell::RefCell, collections::HashMap};

#[derive(Clone, Default)]
pub(super) struct EntityIndexes {
    units: IdIndex,
    cities: IdIndex,
}

#[derive(Clone, Default)]
struct IdIndex(RefCell<HashMap<u32, usize>>);

impl IdIndex {
    fn lookup<T>(&self, rows: &[T], id: u32, row_id: impl Fn(&T) -> u32) -> Option<usize> {
        if let Some(index) = self.0.borrow().get(&id).copied()
            && rows.get(index).is_some_and(|row| row_id(row) == id)
        {
            return Some(index);
        }
        let index = rows.iter().position(|row| row_id(row) == id);
        let mut cache = self.0.borrow_mut();
        if let Some(index) = index {
            cache.insert(id, index);
        } else {
            cache.remove(&id);
        }
        index
    }
}

impl EntityIndexes {
    pub(super) fn unit_index(&self, units: &[Unit], id: u32) -> Option<usize> {
        self.units.lookup(units, id, |unit| unit.id)
    }
    pub(super) fn city_index(&self, cities: &[City], id: u32) -> Option<usize> {
        self.cities.lookup(cities, id, |city| city.id)
    }
}
