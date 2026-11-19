//! Enemy danger is fixed during unit planning; defense is recomputed after moves.

use std::{cell::RefCell, collections::HashMap};

#[derive(Clone, Copy)]
pub(super) struct GuardDanger {
    pub(super) danger: i32,
    pub(super) urgency: i32,
    pub(super) grave: i32,
}

#[derive(Clone, Default)]
pub(in crate::sim) struct GuardDangerCache(RefCell<Option<HashMap<u32, GuardDanger>>>);

impl GuardDangerCache {
    pub(in crate::sim) fn begin(&mut self) {
        *self.0.get_mut() = Some(HashMap::new());
    }

    pub(in crate::sim) fn clear(&mut self) {
        *self.0.get_mut() = None;
    }

    // Only advisors may populate this cache, and it cannot store garrison defense.
    pub(super) fn get_or_init(
        &self,
        city: u32,
        build: impl FnOnce() -> GuardDanger,
    ) -> GuardDanger {
        if let Some(danger) = self
            .0
            .borrow()
            .as_ref()
            .and_then(|cities| cities.get(&city))
            .copied()
        {
            return danger;
        }
        let danger = build();
        if let Some(cities) = self.0.borrow_mut().as_mut() {
            cities.insert(city, danger);
        }
        danger
    }
}
