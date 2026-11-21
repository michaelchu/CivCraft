//! Derived data with an AI player-planning lifetime, separate from saved AI memory.

use crate::sim::{Base, City, Simulation, Tile, UnitKind, advisors::GuardDangerCache};
use crate::world::topology::MapWrap;
use std::{
    borrow::Cow,
    cell::{OnceCell, RefCell},
    sync::Arc,
};

#[derive(Clone, Default)]
pub(in crate::sim) struct PlanningCaches {
    spatial: Option<SpatialCaches>,
    guard_danger: GuardDangerCache,
}

#[derive(Clone, Default)]
struct SpatialCaches {
    airfields: OnceCell<Vec<(i32, i32)>>,
    continents: OnceCell<Arc<[usize]>>,
    sea_regions: RefCell<Option<(usize, Arc<[usize]>)>>,
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

    pub(in crate::sim) fn sea_regions(
        &self,
        tiles: &[Tile],
        cities: &[City],
        map_wrap: MapWrap,
    ) -> Arc<[usize]> {
        let build = || build_sea_region_labels(tiles, cities, map_wrap);
        match &self.spatial {
            Some(spatial) => {
                if let Some((cached_cities, labels)) = spatial.sea_regions.borrow().as_ref()
                    && *cached_cities == cities.len()
                {
                    return labels.clone();
                }
                let labels = build();
                *spatial.sea_regions.borrow_mut() = Some((cities.len(), labels.clone()));
                labels
            }
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

fn build_sea_region_labels(tiles: &[Tile], cities: &[City], map_wrap: MapWrap) -> Arc<[usize]> {
    let mut passable = vec![false; tiles.len()];
    for (index, tile) in tiles.iter().enumerate() {
        passable[index] = tile.terrain.is_water();
    }
    for city in cities {
        passable[map_wrap.index(city.x, city.y)] = true;
    }
    let mut labels = vec![0; tiles.len()];
    let mut region = 0;
    for index in 0..tiles.len() {
        if labels[index] != 0 || !passable[index] {
            continue;
        }
        region += 1;
        labels[index] = region;
        let mut pending = vec![map_wrap.position(index)];
        while let Some((x, y)) = pending.pop() {
            for (nx, ny) in map_wrap.civ3_neighbors(x, y) {
                let next = map_wrap.index(nx, ny);
                if labels[next] == 0 && passable[next] {
                    labels[next] = region;
                    pending.push((nx, ny));
                }
            }
        }
    }
    labels.into()
}

/// O(1) landmass and sea-region reachability oracle.
///
/// Never returns `false` for a tile that `NormalPaths` can directly reach:
/// land components merge any continents bridged by a cluster of allied
/// transports on water, and sea components treat every city as a canal tile.
pub(in crate::sim) struct MovementConnectivity {
    continents: Arc<[usize]>,
    sea_regions: Arc<[usize]>,
    land_parent: Vec<usize>,
}

impl MovementConnectivity {
    fn land_root(&self, mut label: usize) -> usize {
        while label < self.land_parent.len() && self.land_parent[label] != label {
            label = self.land_parent[label];
        }
        label
    }

    pub(in crate::sim) fn can_directly_reach_tile(
        &self,
        sim: &Simulation,
        kind: UnitKind,
        start: (i32, i32),
        target: (i32, i32),
        allow_adjacent_attack: bool,
    ) -> bool {
        if kind.is_aircraft() || kind.fuel_capacity() > 0 {
            return true;
        }
        let si = sim.map_wrap.index(start.0, start.1);
        let ti = sim.map_wrap.index(target.0, target.1);
        if kind.is_naval() {
            let sc = self.sea_regions[si];
            if sc == 0 {
                return true;
            }
            if self.sea_regions[ti] == sc {
                return true;
            }
            if allow_adjacent_attack {
                return sim
                    .map_wrap
                    .civ3_neighbors(target.0, target.1)
                    .any(|(nx, ny)| self.sea_regions[sim.map_wrap.index(nx, ny)] == sc);
            }
            return false;
        }
        let sc = self.continents[si];
        let tc = self.continents[ti];
        if sc == 0 || tc == 0 {
            return true;
        }
        self.land_root(sc) == self.land_root(tc)
    }
}

impl Simulation {
    pub(in crate::sim) fn sea_region_labels(&self) -> Arc<[usize]> {
        self.planning
            .sea_regions(&self.tiles, &self.cities, self.map_wrap)
    }

    pub(in crate::sim) fn movement_connectivity(&self, owner: usize) -> MovementConnectivity {
        let continents = self.advisor_continent_labels();
        let sea_regions = self.advisor_sea_regions();
        let max_continent = continents.iter().copied().max().unwrap_or(0);
        let mut land_parent: Vec<usize> = (0..=max_continent).collect();

        // Any connected cluster of allied water-tile transports can act as a
        // land bridge in NormalPaths (via TRANSPORT scope), so merge any
        // continents adjacent to the same transport cluster.
        let water_transports: Vec<usize> = self
            .units
            .iter()
            .filter(|unit| {
                unit.transport.is_none()
                    && unit.kind.transport_capacity() > 0
                    && self.allied(owner, unit.owner)
                    && self.tile(unit.x, unit.y).terrain.is_water()
            })
            .map(|unit| self.map_wrap.index(unit.x, unit.y))
            .collect();
        if !water_transports.is_empty() {
            let mut visited = vec![false; water_transports.len()];
            let find_root = |parent: &mut [usize], mut x: usize| {
                while parent[x] != x {
                    parent[x] = parent[parent[x]];
                    x = parent[x];
                }
                x
            };
            for start_i in 0..water_transports.len() {
                if visited[start_i] {
                    continue;
                }
                visited[start_i] = true;
                let mut stack = vec![water_transports[start_i]];
                let mut cluster_root: Option<usize> = None;
                while let Some(tile_idx) = stack.pop() {
                    let (tx, ty) = self.map_wrap.position(tile_idx);
                    for (nx, ny) in self.map_wrap.civ3_neighbors(tx, ty) {
                        let ni = self.map_wrap.index(nx, ny);
                        let c = continents[ni];
                        if c != 0 {
                            let rc = find_root(&mut land_parent, c);
                            if let Some(root) = cluster_root {
                                let rr = find_root(&mut land_parent, root);
                                if rc != rr {
                                    land_parent[rc] = rr;
                                }
                            } else {
                                cluster_root = Some(rc);
                            }
                        } else if let Some(pos) = water_transports.iter().position(|&idx| idx == ni)
                            && !visited[pos]
                        {
                            visited[pos] = true;
                            stack.push(ni);
                        }
                    }
                }
            }
            for c in 0..land_parent.len() {
                let root = find_root(&mut land_parent, c);
                land_parent[c] = root;
            }
        }

        MovementConnectivity {
            continents,
            sea_regions,
            land_parent,
        }
    }

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
