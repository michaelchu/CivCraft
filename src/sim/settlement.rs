use super::*;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct SettlementReservation {
    pub(super) owner: usize,
    pub(super) destination: (i32, i32),
    pub(super) worker: Option<(i32, i32)>,
}

pub(super) struct CitySite {
    pub(super) value: f64,
    pub(super) worker: Option<(i32, i32)>,
}

impl Simulation {
    pub(super) fn settlement_citymap(&self, owner: usize, except: Option<u32>) -> Vec<i32> {
        let mut map = vec![0; self.tiles.len()];
        for city in &self.cities {
            for (_, _, x, y) in Self::city_radius_at(city.x, city.y) {
                let index = Self::idx(x, y);
                if map[index] >= 0 {
                    map[index] += 1;
                }
            }
            for &(x, y) in &city.worked_tiles {
                map[Self::idx(x, y)] = -1;
            }
            map[Self::idx(city.x, city.y)] = -1;
        }
        for (&id, reservation) in &self.ai_settlement_targets {
            if reservation.owner != owner
                || Some(id) == except
                || !self
                    .units
                    .iter()
                    .any(|unit| unit.id == id && unit.owner == owner && unit.kind.stats().can_found)
            {
                continue;
            }
            for (_, _, x, y) in
                Self::city_radius_at(reservation.destination.0, reservation.destination.1)
            {
                let index = Self::idx(x, y);
                if map[index] >= 0 {
                    map[index] += 1;
                }
            }
            map[Self::idx(reservation.destination.0, reservation.destination.1)] = -1;
            if let Some((x, y)) = reservation.worker {
                map[Self::idx(x, y)] = -1;
            }
        }
        map
    }

    pub(super) fn prune_settlement_reservations(&mut self, owner: usize) {
        let invalid: Vec<_> = self
            .ai_settlement_targets
            .iter()
            .filter_map(|(&id, reservation)| {
                let valid_unit = self.units.iter().any(|unit| {
                    unit.id == id && unit.owner == reservation.owner && unit.kind.stats().can_found
                });
                (!valid_unit).then_some(id)
            })
            .collect();
        for id in invalid {
            self.ai_settlement_targets.remove(&id);
            self.unit_goto_tiles.remove(&id);
            self.ai_worker_tasks.remove(&id);
        }
        // citymap_turn_init rebuilds future centers and crowding, but the
        // extra first-worker reservation exists only in the assignment turn.
        for reservation in self
            .ai_settlement_targets
            .values_mut()
            .filter(|entry| entry.owner == owner)
        {
            reservation.worker = None;
        }
    }

    pub(super) fn reserve_settlement_site(&mut self, founder: &Unit, destination: (i32, i32)) {
        let government = self.ai_goal_government(founder.owner);
        let map = self.settlement_citymap(founder.owner, Some(founder.id));
        if let Some(site) = self.city_site_result(
            founder.owner,
            destination.0,
            destination.1,
            government,
            &map,
        ) {
            self.ai_worker_tasks.remove(&founder.id);
            self.unit_goto_tiles.insert(founder.id, destination);
            self.ai_settlement_targets.insert(
                founder.id,
                SettlementReservation {
                    owner: founder.owner,
                    destination,
                    worker: site.worker,
                },
            );
        }
    }

    pub(super) fn run_ai_founder(&mut self, founder: &Unit) {
        let improvement = if founder.kind.stats().can_work {
            self.worker_improvement_want(founder)
        } else {
            0.0
        };
        if !self.run_ai_founder_with_improvement(founder, improvement) {
            self.run_auto_worker(founder.id);
        }
    }

    pub(super) fn run_ai_founder_with_improvement(
        &mut self,
        founder: &Unit,
        improvement: f32,
    ) -> bool {
        if let Some(reservation) = self.ai_settlement_targets.get(&founder.id).copied() {
            if reservation.owner != founder.owner
                || !self.can_found_at(reservation.destination.0, reservation.destination.1)
            {
                self.ai_settlement_targets.remove(&founder.id);
                self.unit_goto_tiles.remove(&founder.id);
                self.ai_worker_tasks.remove(&founder.id);
                self.release_ai_ferry(founder.id);
                return true; // daisettler.c BUILD_CITY cancels without same-turn recursion.
            }
            let path = self.path_for_unit(
                founder,
                reservation.destination.0,
                reservation.destination.1,
            );
            if founder.transport.is_none()
                && (!path.is_empty() || (founder.x, founder.y) == reservation.destination)
            {
                self.release_ai_ferry(founder.id);
            }
            // Temporary route obstruction does not abandon an assigned city
            // mission; Freeciv waits for its ferry/route to become available.
            self.ai_guard_for_destination(
                founder.id,
                reservation.destination.0,
                reservation.destination.1,
            );
            self.follow_path(founder.id, path);
            if self.units.iter().any(|unit| {
                unit.id == founder.id
                    && unit.moves_left > 0
                    && (unit.x, unit.y) == reservation.destination
            }) {
                self.found_city(founder.id);
                self.ai_settlement_targets.remove(&founder.id);
            }
            return true;
        }
        if founder.work_left > 0 {
            return false;
        }
        let city = self.best_city_site(founder);
        // daisettler.c:dai_auto_settler_run chooses a city only when its
        // adv_want is STRICTLY greater; equal wants keep terrain work.
        if let Some((destination, path, _)) = city.filter(|(_, _, want)| *want as f32 > improvement)
        {
            if founder.transport.is_none() {
                self.release_ai_ferry(founder.id);
            }
            self.reserve_settlement_site(founder, destination);
            self.ai_guard_for_destination(founder.id, destination.0, destination.1);
            self.follow_path(founder.id, path);
            if self.units.iter().any(|unit| {
                unit.id == founder.id && unit.moves_left > 0 && (unit.x, unit.y) == destination
            }) {
                self.found_city(founder.id);
                self.ai_settlement_targets.remove(&founder.id);
            }
            true
        } else {
            if improvement > 0.0 {
                self.release_ai_ferry(founder.id);
            }
            false
        }
    }

    pub(super) fn best_city_site(
        &self,
        founder: &Unit,
    ) -> Option<((i32, i32), Vec<(i32, i32)>, f64)> {
        use std::{cmp::Reverse, collections::BinaryHeap};
        let government = self.ai_goal_government(founder.owner);
        let reservations = self.settlement_citymap(founder.owner, Some(founder.id));
        let start = Self::idx(founder.x, founder.y);
        let mut costs = vec![i32::MAX; self.tiles.len()];
        let mut previous = vec![None; self.tiles.len()];
        costs[start] = 0;
        let mut queue = BinaryHeap::from([Reverse((0, start))]);
        let mut best = None;
        let mut best_score = 0.0;
        let mut best_turn = 0;
        let mut traveler = founder.clone();
        let allowance = self.unit_move_rate(founder).max(1);
        while let Some(Reverse((cost, index))) = queue.pop() {
            if cost != costs[index] {
                continue;
            }
            let (x, y) = Self::xy(index);
            let turns = cost / allowance;
            if let Some(site) =
                self.city_site_result(founder.owner, x, y, government, &reservations)
            {
                let score = site.value * (23.0_f64 / 24.0).powi(3 * turns)
                    - f64::from(founder.kind.stats().cost);
                if score > best_score {
                    best = Some(index);
                    best_score = score;
                    best_turn = turns;
                }
                if best_score > 250.0 && turns > allowance && best_turn < turns {
                    break;
                }
            }
            (traveler.x, traveler.y) = (x, y);
            let remaining = if cost < founder.moves_left {
                founder.moves_left - cost
            } else {
                allowance - (cost - founder.moves_left.max(0)) % allowance
            };
            traveler.moves_left = remaining;
            for (nx, ny, step) in self.settler_land_steps(&traveler) {
                let next = Self::idx(nx, ny);
                let next_cost = cost + step.min(remaining);
                if next_cost < costs[next] {
                    costs[next] = next_cost;
                    previous[next] = Some((index, wrapped_map_vector(x, y, nx, ny)));
                    queue.push(Reverse((next_cost, next)));
                }
            }
        }

        let destination = best?;
        let mut cursor = destination;
        let mut path = Vec::new();
        while let Some((prior, step)) = previous[cursor] {
            path.push(step);
            cursor = prior;
        }
        path.reverse();
        Some((Self::xy(destination), path, best_score))
    }

    pub(super) fn settler_land_steps<'a>(
        &'a self,
        traveler: &'a Unit,
    ) -> impl Iterator<Item = (i32, i32, i32)> + 'a {
        super::terrain_work::neighbors(traveler.x, traveler.y).filter_map(move |(nx, ny)| {
            if self.treaty_blocks_move(traveler, nx, ny)
                || self.zone_blocks_step(traveler, (traveler.x, traveler.y), (nx, ny))
                || self.units.iter().any(|other| {
                    (other.x, other.y) == (nx, ny) && !self.allied(other.owner, traveler.owner)
                })
                || self.cities.iter().any(|city| {
                    (city.x, city.y) == (nx, ny) && !self.allied(city.owner, traveler.owner)
                })
            {
                return None;
            }
            self.movement_cost_for(traveler, nx, ny)
                .map(|step| (nx, ny, step))
        })
    }

    pub(super) fn city_site_value(&self, owner: usize, x: i32, y: i32) -> Option<f64> {
        let government = self.ai_goal_government(owner);
        let reservations = self.settlement_citymap(owner, None);
        self.city_site_result(owner, x, y, government, &reservations)
            .map(|site| site.value)
    }

    pub(super) fn city_site_result(
        &self,
        owner: usize,
        x: i32,
        y: i32,
        government: Government,
        reservations: &[i32],
    ) -> Option<CitySite> {
        let tile = self.tile(x, y);
        if !self.can_found_at(x, y)
            || Self::city_radius_at(x, y).any(|(_, _, tx, ty)| reservations[Self::idx(tx, ty)] < 0)
            || tile.owner.is_some_and(|other| {
                other != owner
                    && matches!(
                        self.pact(owner, other),
                        Pact::Armistice | Pact::Peace | Pact::Alliance
                    )
            })
            || self.units.iter().any(|unit| {
                unit.kind.is_military()
                    && self.at_war(unit.owner, owner)
                    && self.map_distance(x, y, unit.x, unit.y) <= 1
            })
        {
            return None;
        }
        let city = City {
            id: u32::MAX,
            owner,
            x,
            y,
            name: String::new(),
            size: 1,
            specialists: [0; 3],
            worked_tiles: Vec::new(),
            was_happy: false,
            last_plague: None,
            history: 0,
            foreign_citizens: Vec::new(),
            airlift_used: false,
            food: 0,
            shields: 0,
            production: Production::Unit(UnitKind::Warrior),
            founded_turn: self.turn,
            bought_production: false,
            sold_building: false,
            worklist: Vec::new(),
            technology_thefts: 0,
            protected_shields: 0,
            caravan_shields: 0,
            production_before_switch: None,
            buildings: Vec::new(),
        };
        let science = if self.players[owner].researching == Tech::FUTURE {
            1.0
        } else {
            21.0
        };
        let weighted = |(food, shields, trade): (i32, i32, i32), reservation: i32| {
            let mut value =
                (f64::from(food * 30 + shields * 17) + f64::from(trade) * science) as i32;
            if food >= 2 {
                value *= 2;
            }
            if value > 0 {
                value -= (reservation * 15).min(value - 1);
            }
            value
        };
        let reserved = |tx, ty| reservations[Self::idx(tx, ty)];
        let center = self.city_tile_yields_under_government(&city, x, y, true, None, government);
        let mut best = None;
        let mut remaining = 0;
        let mut city_tiles: Vec<_> = Self::city_radius_at(x, y).collect();
        // daisettler.c:cityresult_fill uses native city indices and only
        // replaces best_other on a strictly greater want. Equal output
        // workers therefore use city.c:compare_iter_index, not row order.
        city_tiles.sort_by_key(|&(dx, dy, _, _)| (isohex_vector_distance(dx, dy).pow(2), dx, dy));
        for (_, _, tx, ty) in city_tiles {
            if (tx, ty) == (x, y) || reserved(tx, ty) < 0 {
                continue;
            }
            let output =
                self.city_tile_yields_under_government(&city, tx, ty, false, None, government);
            let value = weighted(output, reserved(tx, ty));
            if best.is_none_or(|(score, _, _)| value > score) {
                if let Some((score, _, _)) = best {
                    remaining += score.max(0) / 8;
                }
                best = Some((value, output, (tx, ty)));
            } else {
                remaining += value.max(0) / 8;
            }
        }
        let worker = best.map(|(_, _, tile)| tile);
        let (best_value, best_output, _) = best.unwrap_or((0, (0, 0, 0), (x, y)));
        if center.0 + best_output.0 <= CITIZEN_FOOD_COST || center.1 + best_output.1 == 0 {
            return None;
        }
        let gross_shields = center.1 + best_output.1;
        let gross_trade = center.2 + best_output.2;
        let (net_shields, net_trade) =
            self.government_output_waste_under(&city, gross_shields, gross_trade, None, government);
        let total = (weighted(center, reserved(x, y)) + best_value
            - 17 * (gross_shields - net_shields)
            - (science * f64::from(gross_trade - net_trade)) as i32)
            .max(0);
        let mut total = total as f32;
        // daisettler.c preserves integer division order, including its float
        // defense tie-breaker and integer per-tile growth rounding.
        let mut defense = 10 + tile.terrain.defense_bonus() / 10;
        defense += defense * tile.base.map_or(0, |base| base.defense_bonus()) / 100;
        total += 100.0 / (total + 1.0) * (100 / defense * 20) as f32;
        if super::terrain_work::neighbors(x, y).any(|(nx, ny)| self.tile(nx, ny).terrain.is_water())
        {
            total += total * 20.0 / 100.0;
        }
        Some(CitySite {
            value: f64::from(total + remaining as f32),
            worker,
        })
    }
}

impl Simulation {
    pub(crate) fn ai_settlement_goal_government_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_settlement_targets.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.road = true;
            tile.river = true;
        }
        self.create_city(1, 5, 8);
        self.cities[0].size = 4;
        self.reconcile_city_workers();
        self.players[1].government = Government::Anarchy;
        self.players[1].known = self.research_path(1, Tech::MONARCHY);
        self.players[1].known.retain(|&tech| tech != Tech::MONARCHY);
        self.players[1].rates = [60, 40, 0];
        let goal = self.ai_goal_government(1);
        let map = self.settlement_citymap(1, None);
        let current = self.city_site_result(1, 15, 40, Government::Anarchy, &map);
        let projected = self.city_site_result(1, 15, 40, goal, &map);
        goal.requirement()
            .is_some_and(|tech| !self.has_tech(1, tech))
            && current.zip(projected).is_some_and(|(current, projected)| {
                current.value != projected.value
                    && self.city_site_value(1, 15, 40) == Some(projected.value)
            })
            && self.players[1].government == Government::Anarchy
            && self.players[1].rates == [60, 40, 0]
    }

    pub(crate) fn ai_settlement_reservations_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_settlement_targets.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.owner = None;
            tile.road = true;
            tile.irrigation = true;
            tile.farmland = false;
            tile.mine = false;
            tile.river = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.hut = false;
        }
        self.players[1].government = Government::Communism;
        let first_id = self.spawn_unit(1, UnitKind::Settlers, 2, 8);
        let second_id = self.spawn_unit(1, UnitKind::Settlers, 20, 24);
        let first = self
            .units
            .iter()
            .find(|unit| unit.id == first_id)
            .unwrap()
            .clone();
        let second = self
            .units
            .iter()
            .find(|unit| unit.id == second_id)
            .unwrap()
            .clone();
        let destination = (10, 24);
        let baseline: Vec<_> = (0..self.tiles.len())
            .map(Self::xy)
            .map(|(x, y)| self.city_site_value(1, x, y))
            .collect();
        self.reserve_settlement_site(&first, destination);
        let Some(reservation) = self.ai_settlement_targets.get(&first_id).copied() else {
            return false;
        };
        let protected = self.city_site_value(1, 10, 24).is_none()
            && reservation
                .worker
                .is_some_and(|(x, y)| self.settlement_citymap(1, None)[Self::idx(x, y)] < 0);
        let crowding = (0..self.tiles.len()).any(|index| {
            let (x, y) = Self::xy(index);
            baseline[index]
                .zip(self.city_site_value(1, x, y))
                .is_some_and(|(old, new)| new < old)
        });
        let distinct = self.best_city_site(&second).is_some_and(|(target, _, _)| {
            self.map_distance(target.0, target.1, destination.0, destination.1) >= 3
        });
        self.units
            .iter_mut()
            .find(|unit| unit.id == first_id)
            .unwrap()
            .moves_left = 0;
        let waiting = self
            .units
            .iter()
            .find(|unit| unit.id == first_id)
            .unwrap()
            .clone();
        self.run_ai_founder(&waiting);
        let retained = self
            .ai_settlement_targets
            .get(&first_id)
            .is_some_and(|entry| entry.destination == destination);
        self.units
            .iter_mut()
            .find(|unit| unit.id == first_id)
            .unwrap()
            .owner = HUMAN;
        self.prune_settlement_reservations(1);
        let released = !self.ai_settlement_targets.contains_key(&first_id)
            && self.city_site_value(1, 10, 24).is_some();
        protected && crowding && distinct && retained && released
    }

    pub(crate) fn ai_settlement_value_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = None;
            tile.owner = None;
            tile.road = true;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.river = false;
            tile.base = None;
            tile.pollution = false;
            tile.fallout = false;
        }
        self.players[1].government = Government::Communism;
        self.players[1].researching = Tech::BRONZE_WORKING;
        let home = (10, 24);
        let baseline = self.city_site_value(1, home.0, home.1);
        // Source: center115*2 (2food/2shield/1trade) + best68
        // +17 individually truncated68/8; centerShield is additive, not min1.
        let expected = f64::from(298.0_f32 + 100.0_f32 / 299.0 * 200.0 + 136.0);
        let rounded = baseline.is_some_and(|value| (value - expected).abs() < 0.0001);
        let neighbor = offset_native_by_map(home.0, home.1, 3, 0);
        self.create_city(1, neighbor.0, neighbor.1);
        let crowded = self
            .city_site_value(1, home.0, home.1)
            .is_some_and(|value| value < baseline.unwrap());
        self.tile_mut(home.0, home.1).pollution = true;
        rounded && crowded && self.city_site_value(1, home.0, home.1).is_none()
    }
}

impl Simulation {
    pub(crate) fn founder_work_arbitration_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_settlement_targets.clear();
        self.ai_government_cache.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = Some(Resource::Wheat);
            tile.owner = None;
            tile.hut = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.road = true;
            tile.railroad = true;
            tile.irrigation = true;
            tile.farmland = true;
            tile.river = false;
            tile.mine = false;
        }
        self.players[1].government = Government::Communism;
        self.create_city(1, 12, 24);
        self.tile_mut(12, 24).pollution = true;
        self.climate_current[0] = 1000;
        let id = self.spawn_unit(1, UnitKind::Settlers, 12, 24);
        let founder = self
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        let Some((destination, _, city_want)) = self.best_city_site(&founder) else {
            return false;
        };
        let improvement = self.worker_improvement_want(&founder);
        if city_want <= 0.0 || improvement <= city_want as f32 {
            return false;
        }
        let mut committed = self.clone();
        committed.reserve_settlement_site(&founder, destination);
        let retained = committed.run_ai_founder_with_improvement(&founder, improvement)
            && (committed
                .ai_settlement_targets
                .get(&id)
                .is_some_and(|reservation| reservation.destination == destination)
                || committed
                    .cities
                    .iter()
                    .any(|city| (city.x, city.y) == destination));
        self.run_ai_founder(&founder);
        retained
            && !self.ai_settlement_targets.contains_key(&id)
            && self.cities.len() == 1
            && self.units.iter().any(|unit| {
                unit.id == id && unit.work_kind == Some(WorkKind::Clean(Contamination::Pollution))
            })
    }
}

impl Simulation {
    pub(crate) fn settlement_reservation_order_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_settlement_targets.clear();
        self.ai_government_cache.clear();
        self.city_rapture.clear();
        self.players[1].government = Government::Communism;
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = None;
            tile.owner = None;
            tile.road = true;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.oil_well = false;
            tile.oil_platform = false;
            tile.river = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.base = None;
            tile.hut = false;
        }
        let home = (12, 24);
        let id = self.spawn_unit(1, UnitKind::Settlers, home.0, home.1);
        let founder = self
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        let first = offset_native_by_map(home.0, home.1, -1, -1);
        let second = offset_native_by_map(home.0, home.1, -1, 0);
        let mut map = vec![0; self.tiles.len()];
        let initial = self.city_site_result(1, home.0, home.1, Government::Communism, &map);
        let source_first = initial
            .as_ref()
            .is_some_and(|site| site.worker == Some(first));
        map[Self::idx(first.0, first.1)] = 1;
        let next = self.city_site_result(1, home.0, home.1, Government::Communism, &map);
        let source_next = next
            .as_ref()
            .is_some_and(|site| site.worker == Some(second))
            && initial.zip(next).is_some_and(|(a, b)| a.value > b.value);
        map[Self::idx(first.0, first.1)] = -1;
        let overlap_rejected = self
            .city_site_result(1, home.0, home.1, Government::Communism, &map)
            .is_none();
        self.reserve_settlement_site(&founder, home);
        let stored = self
            .ai_settlement_targets
            .get(&id)
            .is_some_and(|reservation| {
                reservation.worker == Some(first) && reservation.destination == home
            });
        let reserved = self.settlement_citymap(1, None)[Self::idx(first.0, first.1)] == -1;
        source_first && source_next && overlap_rejected && stored && reserved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_yield_future_city_workers_and_reservations_follow_native_city_indices() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.settlement_reservation_order_playtest());
    }

    #[test]
    fn founder_can_choose_useful_work_over_a_positive_city_site_and_keeps_committed_missions() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.founder_work_arbitration_playtest());
    }

    #[test]
    fn founder_city_want_must_be_strictly_greater_than_actual_worker_want() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = Some(Resource::Wheat);
        }
        let id = game.spawn_unit(1, UnitKind::Settlers, 12, 24);
        let founder = game.units.last().unwrap().clone();
        let (_, _, want) = game.best_city_site(&founder).unwrap();
        assert!(want > 0.0);
        // The worker evaluator and city result both store adv_want floats.
        assert!(!game.run_ai_founder_with_improvement(&founder, want as f32));
        assert!(!game.ai_settlement_targets.contains_key(&id));
        assert_eq!(
            game.units.last().map(|unit| (unit.x, unit.y)),
            Some((founder.x, founder.y))
        );
        assert!(game.run_ai_founder_with_improvement(&founder, want as f32 - 1.0));
        assert!(game.ai_settlement_targets.contains_key(&id) || !game.cities.is_empty());
    }

    #[test]
    fn settlement_goal_government_desktop_fixture() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_settlement_goal_government_playtest());
    }

    #[test]
    fn exhausted_arriving_founder_keeps_its_mission_until_movement_refreshes() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.road = false;
        }
        let id = game.spawn_unit(1, UnitKind::Settlers, 10, 24);
        let founder = game
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        let destination = offset_native_by_map(10, 24, 1, 0);
        game.reserve_settlement_site(&founder, destination);
        game.run_ai_founder(&founder);
        assert!(game.cities.is_empty());
        let arrived = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert_eq!((arrived.x, arrived.y), destination);
        assert_eq!(arrived.moves_left, 0);
        assert!(game.ai_settlement_targets.contains_key(&id));
        let allowance = game.unit_move_allowance(1, UnitKind::Settlers);
        game.units
            .iter_mut()
            .find(|unit| unit.id == id)
            .unwrap()
            .moves_left = allowance;
        let refreshed = game
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        game.run_ai_founder(&refreshed);
        assert_eq!(game.cities.len(), 1);
        assert!(!game.ai_settlement_targets.contains_key(&id));
    }

    #[test]
    fn reservation_lifecycle_keeps_blocked_mission_but_releases_illegal_target() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
        }
        let id = game.spawn_unit(1, UnitKind::Settlers, 2, 8);
        let founder = game
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        game.reserve_settlement_site(&founder, (10, 24));
        assert!(game.ai_settlement_targets[&id].worker.is_some());
        game.prune_settlement_reservations(1);
        assert_eq!(game.ai_settlement_targets[&id].worker, None);
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
        }
        game.tile_mut(2, 8).terrain = Terrain::Plains;
        game.tile_mut(10, 24).terrain = Terrain::Plains;
        assert!(game.path_for_unit(&founder, 10, 24).is_empty());
        game.run_ai_founder(&founder);
        assert_eq!(game.ai_settlement_targets[&id].destination, (10, 24));
        game.tile_mut(10, 24).terrain = Terrain::Ocean;
        game.run_ai_founder(&founder);
        assert!(!game.ai_settlement_targets.contains_key(&id));
        assert!(game.cities.is_empty());
    }

    #[test]
    fn founders_reserve_destinations_workers_and_growth_then_release_on_capture() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_settlement_reservations_playtest());
    }

    #[test]
    fn settlement_uses_goal_government_without_changing_live_government() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.road = true;
            tile.river = true;
        }
        game.create_city(1, 5, 8);
        game.cities[0].size = 4;
        game.reconcile_city_workers();
        game.players[1].government = Government::Anarchy;
        game.players[1].known = game.research_path(1, Tech::MONARCHY);
        game.players[1].known.retain(|&tech| tech != Tech::MONARCHY);
        game.players[1].rates = [60, 40, 0];
        let goal = game.ai_goal_government(1);
        assert_ne!(goal, Government::Anarchy);
        assert!(
            goal.requirement()
                .is_some_and(|tech| !game.has_tech(1, tech)),
            "goal {goal:?} should be a future government"
        );
        let point = (15, 40);
        let reservations = game.settlement_citymap(1, None);
        let current = game
            .city_site_result(1, point.0, point.1, Government::Anarchy, &reservations)
            .unwrap()
            .value;
        let projected = game
            .city_site_result(1, point.0, point.1, goal, &reservations)
            .unwrap()
            .value;
        assert_ne!(current, projected);
        assert_eq!(game.city_site_value(1, point.0, point.1), Some(projected));
        assert_eq!(game.players[1].government, Government::Anarchy);
        assert_eq!(game.players[1].rates, [60, 40, 0]);
    }

    #[test]
    fn rough_edges_use_remaining_movement_when_amortizing_a_city_site() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Mountains;
            tile.resource = None;
            tile.road = false;
            tile.railroad = false;
            tile.owner = None;
            tile.base = None;
            tile.hut = false;
            tile.river = false;
        }
        game.players[1].government = Government::Communism;
        let home = (10, 24);
        let target = offset_native_by_map(home.0, home.1, 2, 0);
        game.tile_mut(target.0, target.1).terrain = Terrain::Grassland;
        let founder = game.make_unit(0, 1, UnitKind::Settlers, home.0, home.1);
        let (site, path, score) = game.best_city_site(&founder).unwrap();
        assert_eq!(site, target);
        assert_eq!(path.len(), 2);
        let expected = game.city_site_value(1, target.0, target.1).unwrap()
            * (23.0_f64 / 24.0).powi(6)
            - f64::from(founder.kind.stats().cost);
        assert!((score - expected).abs() < 0.0001);
    }

    #[test]
    fn a_virtual_city_does_not_invent_road_trade_on_a_bare_river() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(1, 4, 12);
        let mut city = game.cities[0].clone();
        city.x = 10;
        city.y = 24;
        city.buildings.clear();
        game.players[1].government = Government::Republic;
        let tile = game.tile_mut(city.x, city.y);
        tile.terrain = Terrain::Plains;
        tile.resource = None;
        tile.road = false;
        tile.railroad = false;
        tile.river = true;
        assert_eq!(
            game.city_tile_yields(&city, city.x, city.y, true, None).2,
            2
        );
    }

    #[test]
    fn settlement_values_use_per_tile_rounding_reservations_and_actual_output() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_settlement_value_playtest());
    }
}
