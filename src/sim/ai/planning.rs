//! Derived data with an AI player-planning lifetime, separate from saved AI memory.

use crate::sim::{Base, Simulation, Tile, advisors::GuardDangerCache};
use crate::world::topology::MapWrap;
use std::{borrow::Cow, cell::OnceCell, sync::Arc};

#[derive(Clone, Default)]
pub(in crate::sim) struct PlanningCaches {
    spatial: Option<SpatialCaches>,
    guard_danger: GuardDangerCache,
}

#[derive(Clone, Default)]
struct SpatialCaches {
    airfields: OnceCell<Vec<(i32, i32)>>,
    continents: OnceCell<Arc<[usize]>>,
}

impl PlanningCaches {
    fn begin(&mut self) {
        self.spatial = Some(SpatialCaches::default());
        self.guard_danger.clear();
    }

    /// Freeze enemy danger only for unit movement; garrison defense stays live.
    pub(super) fn begin_units(&mut self) {
        self.guard_danger.begin();
    }

    fn finish(&mut self) {
        *self = Self::default();
    }

    pub(in crate::sim) fn airfields(
        &self,
        tiles: &[Tile],
        map_wrap: MapWrap,
    ) -> Cow<'_, [(i32, i32)]> {
        let build = || {
            tiles
                .iter()
                .enumerate()
                .filter(|(_, tile)| matches!(tile.base, Some(Base::Airstrip | Base::Airbase)))
                .map(|(i, _)| map_wrap.position(i))
                .collect()
        };
        match &self.spatial {
            Some(spatial) => Cow::Borrowed(spatial.airfields.get_or_init(build)),
            None => Cow::Owned(build()),
        }
    }

    pub(in crate::sim) fn continents(&self, tiles: &[Tile], map_wrap: MapWrap) -> Arc<[usize]> {
        let build = || build_continent_labels(tiles, map_wrap);
        match &self.spatial {
            Some(spatial) => spatial.continents.get_or_init(build).clone(),
            None => build(),
        }
    }

    pub(in crate::sim) fn guard_danger(&self) -> &GuardDangerCache {
        &self.guard_danger
    }
}

fn build_continent_labels(tiles: &[Tile], map_wrap: MapWrap) -> Arc<[usize]> {
    let mut labels = vec![0; tiles.len()];
    let mut continent = 0;
    for index in 0..tiles.len() {
        if labels[index] != 0 || tiles[index].terrain.is_water() {
            continue;
        }
        continent += 1;
        labels[index] = continent;
        let mut pending = vec![map_wrap.position(index)];
        while let Some((x, y)) = pending.pop() {
            for (nx, ny) in map_wrap.civ3_neighbors(x, y) {
                let index = map_wrap.index(nx, ny);
                if labels[index] == 0 && !tiles[index].terrain.is_water() {
                    labels[index] = continent;
                    pending.push((nx, ny));
                }
            }
        }
    }
    labels.into()
}

impl Simulation {
    // Keep list membership and derived spatial data on the same planning boundary.
    pub(super) fn begin_ai_planning(&mut self) {
        self.sync_list_order();
        self.list_order.begin_planning(&self.units, &self.cities);
        self.planning.begin();
    }

    pub(super) fn end_ai_planning(&mut self) {
        self.planning.finish();
        self.list_order.end_planning();
    }
}

#[cfg(test)]
mod tests;
