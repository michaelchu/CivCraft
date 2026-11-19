//! Knowledge is the invalidation key for both era and prerequisite-path queries.
use crate::rules::{Tech, TechEra};
use std::{
    cell::{RefCell, RefMut},
    collections::HashMap,
    sync::Arc,
};

#[derive(Clone, Default)]
pub(in crate::sim) struct ResearchCache(RefCell<Vec<Knowledge>>);

#[derive(Clone, Default)]
struct Knowledge {
    known: Vec<Tech>,
    era: Option<TechEra>,
    paths: HashMap<usize, Arc<[Tech]>>,
}

impl ResearchCache {
    fn current(&self, owner: usize, known: &[Tech]) -> RefMut<'_, Knowledge> {
        let mut table = self.0.borrow_mut();
        if table.len() <= owner {
            table.resize_with(owner + 1, Knowledge::default);
        }
        let entry = &mut table[owner];
        if entry.known != known {
            known.clone_into(&mut entry.known);
            entry.era = None;
            entry.paths.clear();
        }
        RefMut::map(table, |owners| &mut owners[owner])
    }
    pub(super) fn era(&self, owner: usize, known: &[Tech]) -> Option<TechEra> {
        self.current(owner, known).era
    }
    pub(super) fn remember_era(&self, owner: usize, known: &[Tech], era: TechEra) {
        self.current(owner, known).era = Some(era);
    }
    pub(super) fn path(&self, owner: usize, known: &[Tech], goal: Tech) -> Option<Arc<[Tech]>> {
        self.current(owner, known).paths.get(&goal.0).cloned()
    }
    pub(super) fn remember_path(
        &self,
        owner: usize,
        known: &[Tech],
        goal: Tech,
        path: Arc<[Tech]>,
    ) {
        self.current(owner, known).paths.insert(goal.0, path);
    }
}
