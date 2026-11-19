//! Native player lists prepend new/received entities; tile lists prepend
//! arrivals independently. Keep these orders separate from stable Vec storage.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct ListOrder {
    pub(super) next: u64,
    units: BTreeMap<u32, UnitOrder>,
    cities: BTreeMap<u32, CityOrder>,
    #[serde(skip)]
    tile_revision: u64,
    #[serde(skip)]
    membership: Option<PlanningMembership>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct UnitOrder {
    owner: usize,
    player: u64,
    tile: (i32, i32),
    arrival: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct CityOrder {
    owner: usize,
    player: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PlanningMembership {
    tiles: BTreeMap<(i32, i32), Vec<u32>>,
    owners: BTreeMap<usize, Vec<u32>>,
    cities: BTreeMap<usize, Vec<u32>>,
    city_tiles: BTreeMap<(i32, i32), u32>,
}

impl ListOrder {
    pub(super) fn begin_planning(&mut self, units: &[Unit], cities: &[City]) {
        let mut membership = PlanningMembership::default();
        for unit in units {
            membership
                .tiles
                .entry((unit.x, unit.y))
                .or_default()
                .push(unit.id);
            membership
                .owners
                .entry(unit.owner)
                .or_default()
                .push(unit.id);
        }
        for (tile, ids) in &mut membership.tiles {
            ids.sort_by_key(|id| {
                std::cmp::Reverse(
                    self.units
                        .get(id)
                        .filter(|order| order.tile == *tile)
                        .map_or((true, u64::from(*id)), |order| (false, order.arrival)),
                )
            });
        }
        for (owner, ids) in &mut membership.owners {
            ids.sort_by_key(|id| {
                std::cmp::Reverse(
                    self.units
                        .get(id)
                        .filter(|order| order.owner == *owner)
                        .map_or((true, u64::from(*id)), |order| (false, order.player)),
                )
            });
        }
        for city in cities {
            membership
                .cities
                .entry(city.owner)
                .or_default()
                .push(city.id);
            membership.city_tiles.insert((city.x, city.y), city.id);
        }
        for ids in membership.cities.values_mut() {
            ids.sort_by_key(|id| std::cmp::Reverse(self.cities[id].player));
        }
        self.membership = Some(membership);
    }
    pub(super) fn unit_ids_at(&self, x: i32, y: i32) -> Option<&[u32]> {
        self.membership
            .as_ref()
            .map(|m| m.tiles.get(&(x, y)).map_or(&[][..], Vec::as_slice))
    }

    pub(super) fn city_id_at(&self, x: i32, y: i32) -> Option<u32> {
        self.membership.as_ref()?.city_tiles.get(&(x, y)).copied()
    }
    pub(super) fn planning_active(&self) -> bool {
        self.membership.is_some()
    }
    pub(super) fn end_planning(&mut self) {
        self.membership = None;
    }

    fn stamp(&mut self) -> u64 {
        self.next = self.next.checked_add(1).expect("entity order exhausted");
        self.next
    }

    pub(super) fn unit(&mut self, unit: &Unit) {
        let previous = self.units.get(&unit.id);
        if let Some(membership) = &mut self.membership {
            let tile = (unit.x, unit.y);
            if previous.is_none_or(|old| old.tile != tile) {
                if let Some(ids) = previous.and_then(|old| membership.tiles.get_mut(&old.tile)) {
                    ids.retain(|&id| id != unit.id);
                }
                membership.tiles.entry(tile).or_default().insert(0, unit.id);
            }
            if previous.is_none_or(|old| old.owner != unit.owner) {
                if let Some(ids) = previous.and_then(|old| membership.owners.get_mut(&old.owner)) {
                    ids.retain(|&id| id != unit.id);
                }
                membership
                    .owners
                    .entry(unit.owner)
                    .or_default()
                    .insert(0, unit.id);
            }
        }
        let Some(old) = previous else {
            let stamp = self.stamp();
            self.tile_revision = self.tile_revision.wrapping_add(1);
            self.units.insert(
                unit.id,
                UnitOrder {
                    owner: unit.owner,
                    player: stamp,
                    tile: (unit.x, unit.y),
                    arrival: stamp,
                },
            );
            return;
        };
        let changed_owner = old.owner != unit.owner;
        let changed_tile = old.tile != (unit.x, unit.y);
        let player = changed_owner.then(|| self.stamp());
        let arrival = changed_tile.then(|| self.stamp());
        if changed_tile {
            self.tile_revision = self.tile_revision.wrapping_add(1);
        }
        let old = self.units.get_mut(&unit.id).unwrap();
        old.owner = unit.owner;
        old.tile = (unit.x, unit.y);
        if let Some(stamp) = player {
            old.player = stamp;
        }
        if let Some(stamp) = arrival {
            old.arrival = stamp;
        }
    }

    pub(super) fn city(&mut self, city: &City) {
        if self
            .cities
            .get(&city.id)
            .is_some_and(|old| old.owner == city.owner)
        {
            return;
        }
        if let Some(membership) = &mut self.membership {
            if let Some(ids) = self
                .cities
                .get(&city.id)
                .and_then(|old| membership.cities.get_mut(&old.owner))
            {
                ids.retain(|&id| id != city.id);
            }
            membership
                .cities
                .entry(city.owner)
                .or_default()
                .insert(0, city.id);
            membership.city_tiles.insert((city.x, city.y), city.id);
        }
        let stamp = self.stamp();
        self.cities.insert(
            city.id,
            CityOrder {
                owner: city.owner,
                player: stamp,
            },
        );
    }

    pub(super) fn forget_unit(&mut self, id: u32) {
        if let Some(old) = self.units.remove(&id) {
            if let Some(membership) = &mut self.membership {
                if let Some(ids) = membership.tiles.get_mut(&old.tile) {
                    ids.retain(|&entry| entry != id);
                }
                if let Some(ids) = membership.owners.get_mut(&old.owner) {
                    ids.retain(|&entry| entry != id);
                }
            }
            self.tile_revision = self.tile_revision.wrapping_add(1);
        }
    }

    pub(super) fn tile_revision(&self) -> u64 {
        self.tile_revision
    }

    pub(super) fn tile_unit_order_key(&self, unit: &Unit, tile: (i32, i32)) -> (bool, u64) {
        self.units
            .get(&unit.id)
            .filter(|order| order.tile == tile)
            .map_or((true, u64::from(unit.id)), |order| (false, order.arrival))
    }

    pub(super) fn synchronize(&mut self, units: &[Unit], cities: &[City]) {
        let live_units: BTreeSet<_> = units.iter().map(|unit| unit.id).collect();
        let live_cities: BTreeSet<_> = cities.iter().map(|city| city.id).collect();
        self.units.retain(|id, _| live_units.contains(id));
        self.cities.retain(|id, _| live_cities.contains(id));
        for unit in units {
            self.unit(unit);
        }
        for city in cities {
            self.city(city);
        }
    }

    pub(super) fn valid(&self, units: &[Unit], cities: &[City]) -> bool {
        if self.units.len() != units.len() || self.cities.len() != cities.len() {
            return false;
        }
        let mut players = BTreeSet::new();
        let mut arrivals = BTreeSet::new();
        units.iter().all(|unit| {
            self.units.get(&unit.id).is_some_and(|order| {
                order.owner == unit.owner
                    && order.tile == (unit.x, unit.y)
                    && order.player > 0
                    && order.player <= self.next
                    && order.arrival > 0
                    && order.arrival <= self.next
                    && players.insert(order.player)
                    && arrivals.insert(order.arrival)
            })
        }) && cities.iter().all(|city| {
            self.cities.get(&city.id).is_some_and(|order| {
                order.owner == city.owner
                    && order.player > 0
                    && order.player <= self.next
                    && players.insert(order.player)
            })
        })
    }
}

impl Simulation {
    pub(super) fn sync_list_order(&mut self) {
        self.list_order.synchronize(&self.units, &self.cities);
        self.sync_cargo_order();
    }

    pub(super) fn cargo_units_ordered(&self, boat: u32) -> impl Iterator<Item = &Unit> {
        let order = self.cargo_order.get(&boat);
        if self.list_order.planning_active() {
            return order
                .into_iter()
                .flatten()
                .filter_map(|id| self.unit_by_id(*id))
                .collect::<Vec<_>>()
                .into_iter();
        }
        let mut cargo: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.transport == Some(boat))
            .collect();
        cargo.sort_by_key(|unit| {
            order
                .and_then(|ids| ids.iter().position(|id| *id == unit.id))
                .map_or((true, 0), |index| (false, index))
        });
        cargo.into_iter()
    }

    pub(super) fn record_transport_change(&mut self, id: u32, boat: Option<u32>) {
        if boat.is_some_and(|boat| {
            self.cargo_order
                .get(&boat)
                .is_some_and(|ids| ids.contains(&id))
        }) {
            return;
        }
        for cargo in self.cargo_order.values_mut() {
            cargo.retain(|unit| *unit != id);
        }
        self.cargo_order.retain(|_, ids| !ids.is_empty());
        if let Some(boat) = boat {
            self.cargo_order.entry(boat).or_default().push(id);
        }
    }

    pub(super) fn sync_cargo_order(&mut self) {
        self.cargo_order = self.cargo_order_snapshot();
    }

    pub(super) fn cargo_order_snapshot(&self) -> BTreeMap<u32, Vec<u32>> {
        let mut cargo_order = self.cargo_order.clone();
        cargo_order.retain(|boat, ids| {
            ids.retain(|id| {
                self.units
                    .iter()
                    .any(|unit| unit.id == *id && unit.transport == Some(*boat))
            });
            !ids.is_empty() && self.units.iter().any(|unit| unit.id == *boat)
        });
        for unit in &self.units {
            if let Some(boat) = unit.transport {
                let ids = cargo_order.entry(boat).or_default();
                if !ids.contains(&unit.id) {
                    ids.push(unit.id);
                }
            }
        }
        cargo_order
    }

    pub(super) fn valid_cargo_order(&self) -> bool {
        let mut recorded = BTreeSet::new();
        self.cargo_order.iter().all(|(boat, ids)| {
            !ids.is_empty()
                && self
                    .units
                    .iter()
                    .any(|unit| unit.id == *boat && unit.kind.transport_capacity() > 0)
                && ids.iter().all(|id| {
                    recorded.insert(*id)
                        && self
                            .units
                            .iter()
                            .any(|unit| unit.id == *id && unit.transport == Some(*boat))
                })
        }) && self
            .units
            .iter()
            .filter(|unit| unit.transport.is_some())
            .all(|unit| recorded.contains(&unit.id))
    }

    pub(super) fn city_transfer_unit_ids(&self, city_id: u32, previous_owner: usize) -> Vec<u32> {
        let Some(city) = self.city_by_id(city_id) else {
            return Vec::new();
        };
        // citytools.c:transfer_city_units handles the central tile first.
        // transfer_city reverses the supported list into old_city_units,
        // so the remaining supported units follow oldest home assignment first.
        let mut ids: Vec<_> = self
            .tile_units_ordered(city.x, city.y)
            .filter(|unit| unit.owner == previous_owner)
            .map(|unit| unit.id)
            .collect();
        let mut supported: Vec<_> = self
            .player_units_ordered(previous_owner)
            .filter(|unit| unit.home_city == Some(city_id) && !ids.contains(&unit.id))
            .collect();
        supported.sort_by_key(|unit| unit.home_order);
        ids.extend(supported.into_iter().map(|unit| unit.id));
        ids
    }

    pub(super) fn player_units_ordered(&self, owner: usize) -> impl Iterator<Item = &Unit> {
        if let Some(membership) = &self.list_order.membership {
            return membership
                .owners
                .get(&owner)
                .into_iter()
                .flatten()
                .filter_map(|id| self.unit_by_id(*id))
                .collect::<Vec<_>>()
                .into_iter();
        }
        let mut units: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.owner == owner)
            .collect();
        units.sort_by_key(|unit| {
            std::cmp::Reverse(
                self.list_order
                    .units
                    .get(&unit.id)
                    .filter(|order| order.owner == owner)
                    .map_or((true, u64::from(unit.id)), |order| (false, order.player)),
            )
        });
        units.into_iter()
    }

    pub(super) fn tile_units_ordered(&self, x: i32, y: i32) -> impl Iterator<Item = &Unit> {
        self.trace.count_operation("tile_units_ordered.calls", 1);
        if let Some(membership) = &self.list_order.membership {
            self.trace
                .count_operation("tile_units_ordered.index_hits", 1);
            return membership
                .tiles
                .get(&(x, y))
                .into_iter()
                .flatten()
                .filter_map(|id| self.unit_by_id(*id))
                .collect::<Vec<_>>()
                .into_iter();
        }
        let indexed_units = if self.advisor_cache.active() {
            let tile_index = self.map_wrap.index(x, y);
            self.advisor_cache.ordered_tile_units(
                tile_index,
                self.list_order.tile_revision(),
                &self.units,
            )
        } else {
            None
        };
        if let Some(units) = indexed_units {
            self.trace
                .count_operation("tile_units_ordered.index_hits", 1);
            self.trace.count_operation(
                "tile_units_ordered.entries_returned",
                units.len().try_into().unwrap_or(u64::MAX),
            );
            return units.into_iter();
        }
        self.trace.count_operation(
            "tile_units_ordered.units_scanned",
            self.units.len().try_into().unwrap_or(u64::MAX),
        );
        let mut units: Vec<_> = self
            .units
            .iter()
            .filter(|unit| (unit.x, unit.y) == (x, y))
            .collect();
        units.sort_by_key(|unit| {
            std::cmp::Reverse(self.list_order.tile_unit_order_key(unit, (x, y)))
        });
        self.trace.count_operation(
            "tile_units_ordered.entries_returned",
            units.len().try_into().unwrap_or(u64::MAX),
        );
        units.into_iter()
    }

    pub(super) fn player_cities_ordered(&self, owner: usize) -> impl Iterator<Item = &City> {
        if let Some(membership) = &self.list_order.membership {
            return membership
                .cities
                .get(&owner)
                .into_iter()
                .flatten()
                .filter_map(|id| self.city_by_id(*id))
                .collect::<Vec<_>>()
                .into_iter();
        }
        let mut cities: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .collect();
        cities.sort_by_key(|city| {
            std::cmp::Reverse(
                self.list_order
                    .cities
                    .get(&city.id)
                    .filter(|order| order.owner == owner)
                    .map_or((true, u64::from(city.id)), |order| (false, order.player)),
            )
        });
        cities.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn city_activity_order_game() -> Simulation {
        let map_wrap = crate::world::topology::MapWrap::with_dimensions(36, 48)
            .expect("valid compact city-order test map");
        let grassland = crate::world::GeneratedTile {
            terrain: Terrain::Grassland,
            base_terrain: crate::world::TerrainBase::Grassland,
            altitude: 0,
            hut: false,
            river: false,
            river_edges: 0,
            resource: None,
        };
        Simulation::new(crate::world::GeneratedMap {
            seed: 42,
            map_seed: 42,
            map_wrap,
            game_random: crate::random::FreecivRng::new(42),
            tiles: vec![grassland; map_wrap.tile_count()],
            starts: Vec::new(),
            civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
        })
    }

    #[test]
    fn player_creation_tile_arrival_and_transfer_orders_are_independent() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.native_list_order_test(), (true, true, true));
    }

    #[test]
    fn cargo_appends_on_boarding_and_reboarding_independently_of_unit_creation() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.native_cargo_order_test());
    }

    #[test]
    fn planning_membership_tracks_arrivals_and_transfers_in_native_order() {
        let mut game = city_activity_order_game();
        game.sync_list_order();
        game.list_order.begin_planning(&game.units, &game.cities);
        let first = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
        let second = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
        assert_eq!(
            game.list_order.unit_ids_at(10, 24),
            Some(&[second, first][..])
        );
        let index = game.unit_index_by_id(first).unwrap();
        game.units[index].owner = 1;
        game.units[index].x = 11;
        game.list_order.unit(&game.units[index]);
        assert_eq!(game.list_order.unit_ids_at(10, 24), Some(&[second][..]));
        assert_eq!(game.player_units_ordered(1).next().unwrap().id, first);
        assert_eq!(game.list_order.unit_ids_at(11, 24), Some(&[first][..]));
        game.list_order.forget_unit(first);
        assert!(game.list_order.unit_ids_at(11, 24).unwrap().is_empty());
    }

    #[test]
    fn city_activity_uses_native_list_then_random_selection_after_conversion() {
        let mut game = city_activity_order_game();
        assert!(game.native_city_activity_order_test());
    }

    #[test]
    fn stale_removed_entities_are_pruned_and_duplicate_order_stamps_are_invalid() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.sync_list_order();
        let first = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
        let second = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
        let stamp = game.list_order.units[&first].player;
        game.list_order.units.get_mut(&second).unwrap().player = stamp;
        assert!(!game.list_order.valid(&game.units, &game.cities));
        game.units.retain(|unit| unit.id != second);
        game.sync_list_order();
        assert!(game.list_order.valid(&game.units, &game.cities));
    }
}
