use super::*;

impl Simulation {
    pub(super) fn best_city_site(
        &self,
        founder: &Unit,
    ) -> Option<((i32, i32), Vec<(i32, i32)>, f64)> {
        use std::{cmp::Reverse, collections::BinaryHeap};
        let start = Self::idx(founder.x, founder.y);
        let mut costs = vec![i32::MAX; self.tiles.len()];
        let mut previous = vec![None; self.tiles.len()];
        costs[start] = 0;
        let mut queue = BinaryHeap::from([Reverse((0, start))]);
        let mut best = None;
        let mut best_score = 0.0;
        let mut traveler = founder.clone();
        while let Some(Reverse((cost, index))) = queue.pop() {
            if cost != costs[index] {
                continue;
            }
            let (x, y) = Self::xy(index);
            let turns = cost / self.unit_move_allowance(founder.owner, founder.kind);
            if let Some(value) = self.city_site_value(founder.owner, x, y) {
                let score = f64::from(value) * (23.0_f64 / 24.0).powi(3 * turns)
                    - f64::from(founder.kind.stats().cost);
                if score > best_score {
                    best = Some(index);
                    best_score = score;
                }
                if best_score >= 250.0 && turns >= 3 {
                    break;
                }
            }
            (traveler.x, traveler.y) = (x, y);
            for (nx, ny, step) in self.settler_land_steps(&traveler) {
                let next = Self::idx(nx, ny);
                if cost + step < costs[next] {
                    costs[next] = cost + step;
                    previous[next] = Some((index, wrapped_map_vector(x, y, nx, ny)));
                    queue.push(Reverse((cost + step, next)));
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

    pub(super) fn city_site_value(&self, owner: usize, x: i32, y: i32) -> Option<i32> {
        let tile = self.tile(x, y);
        if !self.can_found_at(x, y)
            || tile.owner.is_some_and(|other| other != owner)
            || self.units.iter().any(|unit| {
                unit.kind.is_military()
                    && self.at_war(unit.owner, owner)
                    && self.map_distance(x, y, unit.x, unit.y) <= 1
            })
        {
            return None;
        }
        let weighted = |(food, shields, trade): (i32, i32, i32)| {
            (food * 30 + shields * 17 + trade * 21) * if food >= 2 { 2 } else { 1 }
        };
        let mut center = tile.clone();
        center.road = true;
        let (mut food, shields, trade) = center.yields();
        if !center.irrigation && !center.mine {
            food += center.irrigation_food();
        }
        let center = (food.max(1), shields.max(1), trade.max(1));
        let mut best = (0, 0);
        let mut remaining = 0;
        for (_, _, tx, ty) in Self::city_radius_at(x, y) {
            if (tx, ty) == (x, y)
                || self.tile(tx, ty).owner.is_some_and(|other| other != owner)
                || self.cities.iter().any(|city| {
                    (city.x, city.y) == (tx, ty) || city.worked_tiles.contains(&(tx, ty))
                })
            {
                continue;
            }
            let yields = self.tile(tx, ty).yields();
            let value = weighted(yields);
            remaining += value;
            if value > best.0 {
                best = (value, yields.0);
            }
        }
        if center.0 + best.1 <= CITIZEN_FOOD_COST {
            return None;
        }
        let value = weighted(center) + best.0;
        let coastal = super::terrain_work::neighbors(x, y).any(|(nx, ny)| {
            matches!(
                self.tile(nx, ny).terrain,
                Terrain::Ocean | Terrain::DeepOcean
            )
        });
        Some(value + i32::from(coastal) * value / 5 + (remaining - best.0) / 8)
    }

    pub(super) fn wander_unit(&mut self, unit_id: u32) {
        let mut safety = 0;
        while self
            .units
            .iter()
            .find(|u| u.id == unit_id)
            .is_some_and(|u| u.moves_left > 0)
            && safety < 6
        {
            let start = self.rand_below(6) as usize;
            let mut moved = false;
            for offset in 0..6 {
                let (dx, dy) = ISOHEX_DIRS[(start + offset) % 6];
                if self.try_move_unit(unit_id, dx, dy) != MoveResult::Blocked {
                    moved = true;
                    break;
                }
            }
            if !moved {
                break;
            }
            safety += 1;
        }
    }

    pub(super) fn run_automated_unit(&mut self, unit_id: u32) {
        let Some(unit) = self.units.iter().find(|u| u.id == unit_id).cloned() else {
            return;
        };
        if unit.transport.is_some() {
            return;
        }
        if self.clean_current_tile(&unit) {
            return;
        }
        if unit.kind.stats().can_work {
            let tile = self.tile(unit.x, unit.y).clone();
            if !tile.road && self.begin_work(unit_id, WorkKind::Road) {
                return;
            }
            if self.can_railroad_at(unit.owner, unit.x, unit.y)
                && self.begin_work(unit_id, WorkKind::Railroad)
            {
                return;
            }
            if tile.terrain.prefers_mine() {
                if !tile.mine && self.begin_work(unit_id, WorkKind::Mine) {
                    return;
                }
            } else if self.can_farm_at(unit.owner, unit.x, unit.y) {
                if self.begin_work(unit_id, WorkKind::Farmland) {
                    return;
                }
            } else if !tile.irrigation {
                if self.begin_work(unit_id, WorkKind::Irrigation) {
                    return;
                }
                if !tile.mine && self.begin_work(unit_id, WorkKind::Mine) {
                    return;
                }
            }
        }
        self.wander_unit(unit_id);
    }

    fn clean_current_tile(&mut self, unit: &Unit) -> bool {
        unit.kind.can_clean()
            && [Contamination::Fallout, Contamination::Pollution]
                .into_iter()
                .any(|target| {
                    self.tile(unit.x, unit.y).has_contamination(target)
                        && self.begin_work(unit.id, WorkKind::Clean(target))
                })
    }

    fn assault_capture_chance(&self, city: &City, troops: &[&Unit]) -> f64 {
        let Some(first) = troops.first() else {
            return 0.0;
        };
        let defenders: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                (unit.x, unit.y) == (city.x, city.y)
                    && unit.transport.is_none()
                    && self.at_war(first.owner, unit.owner)
            })
            .collect();
        if !troops.iter().any(|unit| unit.kind.can_occupy_city()) || defenders.len() > troops.len()
        {
            return 0.0;
        }
        if defenders.is_empty() {
            return 1.0;
        }
        // Estimate successive victories conservatively: each attacker faces
        // the hardest remaining type at its current health.
        let needed = defenders.len();
        let mut victories = vec![0.0; needed + 1];
        victories[0] = 1.0;
        for attacker in troops {
            let chance = defenders
                .iter()
                .map(|defender| {
                    let (attack_fp, defense_fp) = self.combat_firepower(attacker, defender);
                    let attack = percent_of(
                        attacker.kind.stats().attack * POWER_FACTOR,
                        attacker.veteran_power(),
                    );
                    combat_win_chance(
                        attack,
                        attacker.hp,
                        attack_fp,
                        self.defense_power_against(defender, city.x, city.y, attacker.kind),
                        defender.hp,
                        defense_fp,
                    )
                })
                .fold(1.0_f64, f64::min);
            victories[needed] += victories[needed - 1] * chance;
            for wins in (1..needed).rev() {
                victories[wins] = victories[wins] * (1.0 - chance) + victories[wins - 1] * chance;
            }
            victories[0] *= 1.0 - chance;
        }
        victories[needed]
    }

    fn raider_landing(
        &self,
        boat: &Unit,
        overseas: Option<(&[usize], usize)>,
        troops: Option<&[&Unit]>,
        companions: &[&Unit],
    ) -> Option<(usize, i32, i32, (i32, i32))> {
        self.cities
            .iter()
            .filter(|city| {
                self.at_war(city.owner, boat.owner)
                    && troops.is_none_or(|troops| self.assault_capture_chance(city, troops) >= 0.5)
                    && overseas.is_none_or(|(labels, origin)| {
                        labels[Self::idx(city.x, city.y)] != origin
                            && self.tile(city.x, city.y).known_by[boat.owner]
                    })
            })
            .flat_map(|city| {
                ISOHEX_DIRS
                    .into_iter()
                    .map(move |(dx, dy)| offset_native_by_map(city.x, city.y, dx, dy))
            })
            .filter(|&(x, y)| !self.tile(x, y).terrain.is_water())
            .filter(|&(x, y)| {
                self.units
                    .iter()
                    .all(|unit| (unit.x, unit.y) != (x, y) || unit.owner == boat.owner)
                    && self
                        .cities
                        .iter()
                        .all(|city| (city.x, city.y) != (x, y) || city.owner == boat.owner)
            })
            .flat_map(|(lx, ly)| {
                ISOHEX_DIRS
                    .into_iter()
                    .map(move |(dx, dy)| (offset_native_by_map(lx, ly, -dx, -dy), (dx, dy)))
            })
            .filter(|&((x, y), _)| self.tile(x, y).terrain.is_water())
            .filter_map(|((x, y), direction)| {
                let mut distance = 0;
                for ship in std::iter::once(boat).chain(companions.iter().copied()) {
                    if (ship.x, ship.y) != (x, y) {
                        let path = self.path_for_unit(ship, x, y);
                        if path.is_empty() {
                            return None;
                        }
                        distance = distance.max(path.len());
                    }
                }
                Some((distance, x, y, direction))
            })
            .min_by_key(|candidate| candidate.0)
    }

    pub(crate) fn assault_transport_playtest_has_orders(&mut self, owner: usize) -> bool {
        !self
            .dispatch_assault_transports(owner, |_| false, |_| false)
            .is_empty()
    }

    fn coordinate_assault_fleets(
        &mut self,
        owner: usize,
        unavailable: impl Fn(u32) -> bool,
        reserved: impl Fn(u32) -> bool,
    ) -> std::collections::HashSet<u32> {
        let mut assigned = std::collections::HashSet::new();
        if owner == BARBARIANS {
            return assigned;
        }
        let ports: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner && self.is_port(city))
            .map(|city| city.id)
            .collect();
        let labels = self.continent_labels();
        for port in ports {
            let city = self.cities.iter().find(|city| city.id == port).unwrap();
            let origin = labels[Self::idx(city.x, city.y)];
            let overseas = Some((labels.as_slice(), origin));
            let boats: Vec<_> = self
                .units
                .iter()
                .filter(|boat| {
                    boat.owner == owner
                        && matches!(
                            boat.kind,
                            UnitKind::Trireme
                                | UnitKind::Caravel
                                | UnitKind::Galleon
                                | UnitKind::Transport
                        )
                        && !assigned.contains(&boat.id)
                        && !unavailable(boat.id)
                })
                .collect();
            let mut loaded = Vec::new();
            let mut troops = Vec::new();
            for boat in &boats {
                let cargo: Vec<_> = self
                    .units
                    .iter()
                    .filter(|unit| unit.transport == Some(boat.id))
                    .collect();
                if !cargo.is_empty()
                    && cargo.iter().all(|unit| {
                        unit.home_city == Some(port)
                            && unit.kind.is_military()
                            && !unit.kind.is_aircraft()
                            && !unit.kind.is_naval()
                    })
                {
                    loaded.push(*boat);
                    troops.extend(cargo);
                }
            }

            let mut boarding = Vec::new();
            let mut guard = None;
            if loaded.len() < 2 {
                if !loaded.is_empty() || self.city_threats(city).next().is_some() {
                    continue;
                }
                let mut spare: Vec<_> = self
                    .units
                    .iter()
                    .filter(|unit| {
                        unit.owner == owner
                            && (unit.x, unit.y) == (city.x, city.y)
                            && unit.kind.is_military()
                            && !unit.kind.is_aircraft()
                            && !unit.kind.is_naval()
                            && unit.transport.is_none()
                            && !assigned.contains(&unit.id)
                            && !reserved(unit.id)
                    })
                    .collect();
                spare.sort_by_key(|unit| {
                    (
                        std::cmp::Reverse(unit.kind.stats().defense * unit.hp),
                        unit.id,
                    )
                });
                if spare.len() < 3 {
                    continue;
                }
                guard = Some(spare.remove(0).id);
                spare.retain(|unit| unit.home_city.is_none_or(|home| home == port));
                let mut arrivals: Vec<_> = boats
                    .into_iter()
                    .filter_map(|boat| {
                        if self.cargo_count(boat.id) != 0
                            || !self.naval_city_access(city, boat.kind)
                        {
                            return None;
                        }
                        let path = self.path_for_unit(boat, city.x, city.y);
                        ((boat.x, boat.y) == (city.x, city.y) || !path.is_empty())
                            .then_some((boat, path.len()))
                    })
                    .collect();
                arrivals.sort_by_key(|&(boat, steps)| (steps, boat.id));
                for (boat, _) in arrivals {
                    let mut passengers = Vec::new();
                    spare.retain(|unit| {
                        if passengers.len() < boat.kind.transport_capacity()
                            && boat.kind.can_carry(unit.kind)
                        {
                            passengers.push(*unit);
                            false
                        } else {
                            true
                        }
                    });
                    if passengers.is_empty() {
                        continue;
                    }
                    let mut staged = boat.clone();
                    (staged.x, staged.y) = (city.x, city.y);
                    if self
                        .raider_landing(&staged, overseas, Some(&passengers), &[])
                        .is_some()
                    {
                        // The normal dispatch can handle an expedition on one ship.
                        loaded.clear();
                        break;
                    }
                    boarding.extend(passengers.iter().map(|unit| (unit.id, boat.id)));
                    troops.extend(passengers);
                    loaded.push(boat);
                    if loaded.len() >= 2 {
                        let staged: Vec<_> = loaded
                            .iter()
                            .map(|ship| {
                                let mut ship = (*ship).clone();
                                (ship.x, ship.y) = (city.x, city.y);
                                ship
                            })
                            .collect();
                        let companions: Vec<_> = staged[1..].iter().collect();
                        if self
                            .raider_landing(&staged[0], overseas, Some(&troops), &companions)
                            .is_some()
                        {
                            break;
                        }
                    }
                }
            }
            if loaded.len() < 2 {
                continue;
            }
            let assembly = (city.x, city.y);
            let mut staged = Vec::new();
            let sailing_ships = if boarding.is_empty() {
                loaded.clone()
            } else {
                staged.extend(loaded.iter().map(|ship| {
                    let mut ship = (*ship).clone();
                    (ship.x, ship.y) = assembly;
                    ship
                }));
                staged.iter().collect()
            };
            let Some((_, x, y, (dx, dy))) = self.raider_landing(
                sailing_ships[0],
                overseas,
                Some(&troops),
                &sailing_ships[1..],
            ) else {
                continue;
            };
            let ship_ids: Vec<_> = loaded.iter().map(|ship| ship.id).collect();
            let troop_ids: Vec<_> = troops.iter().map(|unit| unit.id).collect();
            let assembling =
                !boarding.is_empty() && loaded.iter().any(|ship| (ship.x, ship.y) != assembly);
            if assembling {
                assigned.extend(guard);
                assigned.extend(ship_ids.iter().copied());
                assigned.extend(troop_ids.iter().copied());
                for id in ship_ids {
                    self.goto_unit(id, assembly.0, assembly.1);
                }
                continue;
            }
            for (unit, ship) in boarding {
                if self.board_specific_ship(unit, ship) {
                    self.units
                        .iter_mut()
                        .find(|passenger| passenger.id == unit)
                        .unwrap()
                        .home_city = Some(port);
                }
            }
            assigned.extend(guard);
            assigned.extend(ship_ids.iter().copied());
            assigned.extend(troop_ids.iter().copied());
            for &ship in &ship_ids {
                self.goto_unit(ship, x, y);
            }
            if ship_ids.iter().all(|id| {
                self.units
                    .iter()
                    .any(|ship| ship.id == *id && (ship.x, ship.y) == (x, y))
            }) {
                for id in troop_ids {
                    self.try_move_unit(id, dx, dy);
                }
            }
        }
        assigned
    }

    fn dispatch_assault_transports(
        &mut self,
        owner: usize,
        unavailable: impl Fn(u32) -> bool,
        reserved: impl Fn(u32) -> bool,
    ) -> std::collections::HashSet<u32> {
        let mut assigned = std::collections::HashSet::new();
        if owner == BARBARIANS {
            return assigned;
        }
        let labels = self.continent_labels();
        let boats: Vec<_> = self
            .units
            .iter()
            .filter(|boat| {
                boat.owner == owner
                    && matches!(
                        boat.kind,
                        UnitKind::Trireme
                            | UnitKind::Caravel
                            | UnitKind::Galleon
                            | UnitKind::Transport
                    )
                    && self.cargo_count(boat.id) == 0
                    && !unavailable(boat.id)
            })
            .cloned()
            .collect();
        struct Pickup {
            cost: usize,
            port: u32,
            position: (i32, i32),
            guard: u32,
            path: Vec<(i32, i32)>,
            troops: Vec<u32>,
        }
        for boat in boats {
            let mut best: Option<Pickup> = None;
            for port in self.cities.iter().filter(|port| {
                port.owner == owner
                    && self.naval_city_access(port, boat.kind)
                    && self.city_threats(port).next().is_none()
            }) {
                let position = (port.x, port.y);
                let pickup = self.path_for_unit(&boat, port.x, port.y);
                if (boat.x, boat.y) != position && pickup.is_empty() {
                    continue;
                }
                let mut troops: Vec<_> = self
                    .units
                    .iter()
                    .filter(|unit| {
                        unit.owner == owner
                            && (unit.x, unit.y) == position
                            && unit.kind.is_military()
                            && !unit.kind.is_aircraft()
                            && !unit.kind.is_naval()
                            && unit.transport.is_none()
                            && !assigned.contains(&unit.id)
                            && !reserved(unit.id)
                    })
                    .collect();
                if troops.len() <= 1 {
                    continue;
                }
                troops.sort_by_key(|unit| {
                    (
                        std::cmp::Reverse(unit.kind.stats().defense * unit.hp),
                        unit.id,
                    )
                });
                let guard = troops.remove(0).id;
                troops.retain(|unit| boat.kind.can_carry(unit.kind));
                troops.truncate(boat.kind.transport_capacity());
                if troops.is_empty() {
                    continue;
                }
                let mut staged = boat.clone();
                (staged.x, staged.y) = position;
                let origin = labels[Self::idx(port.x, port.y)];
                let Some((distance, _, _, _)) =
                    self.raider_landing(&staged, Some((&labels, origin)), Some(&troops), &[])
                else {
                    continue;
                };
                let cost = pickup.len() + distance;
                if best.as_ref().is_none_or(|previous| cost < previous.cost) {
                    best = Some(Pickup {
                        cost,
                        port: port.id,
                        position,
                        guard,
                        path: pickup,
                        troops: troops
                            .into_iter()
                            .take(boat.kind.transport_capacity())
                            .map(|unit| unit.id)
                            .collect(),
                    });
                }
            }
            let Some(plan) = best else {
                continue;
            };
            assigned.insert(boat.id);
            assigned.insert(plan.guard);
            assigned.extend(plan.troops.iter().copied());
            self.follow_path(boat.id, plan.path);
            if let Some(staged) = self
                .units
                .iter()
                .find(|unit| unit.id == boat.id && (unit.x, unit.y) == plan.position)
                .cloned()
            {
                for id in plan.troops {
                    if self.board_specific_ship(id, boat.id) {
                        let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
                        if unit.home_city.is_none() {
                            unit.home_city = Some(plan.port);
                        }
                    }
                }
                self.run_raider_transport(&staged);
            }
        }
        assigned
    }

    fn run_raider_transport(&mut self, boat: &Unit) {
        let cargo: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.transport == Some(boat.id))
            .map(|unit| unit.id)
            .collect();
        if cargo.is_empty() {
            self.wander_unit(boat.id);
            return;
        }

        if let Some((dx, dy)) = ISOHEX_DIRS.into_iter().find(|&(dx, dy)| {
            let location = offset_native_by_map(boat.x, boat.y, dx, dy);
            self.cities.iter().any(|city| {
                (city.x, city.y) == location
                    && self.at_war(city.owner, boat.owner)
                    && (boat.owner == BARBARIANS
                        || cargo.iter().any(|id| {
                            self.units
                                .iter()
                                .find(|unit| unit.id == *id && unit.kind == UnitKind::Marines)
                                .is_some_and(|marine| {
                                    self.assault_capture_chance(city, &[marine]) >= 0.5
                                })
                        }))
            })
        }) {
            for &id in &cargo {
                if self.units.iter().any(|unit| {
                    unit.id == id
                        && unit.kind == UnitKind::Marines
                        && (boat.owner == BARBARIANS
                            || self.cities.iter().any(|city| {
                                let target = offset_native_by_map(boat.x, boat.y, dx, dy);
                                (city.x, city.y) == target
                                    && self.assault_capture_chance(city, &[unit]) >= 0.5
                            }))
                }) && self.try_move_unit(id, dx, dy) != MoveResult::Blocked
                {
                    return;
                }
            }
        }

        let labels = self.continent_labels();
        let origin = cargo.iter().find_map(|id| {
            let home = self.units.iter().find(|unit| unit.id == *id)?.home_city?;
            let city = self.cities.iter().find(|city| city.id == home)?;
            Some(labels[Self::idx(city.x, city.y)])
        });
        let troops: Vec<_> = self
            .units
            .iter()
            .filter(|unit| cargo.contains(&unit.id))
            .collect();
        let landing = self.raider_landing(
            boat,
            origin.map(|region| (labels.as_slice(), region)),
            (boat.owner != BARBARIANS).then_some(troops.as_slice()),
            &[],
        );
        let Some((distance, x, y, (dx, dy))) = landing else {
            if boat.owner == BARBARIANS {
                self.wander_unit(boat.id);
            }
            return;
        };
        if distance > 0 {
            self.goto_unit(boat.id, x, y);
        }
        if self
            .units
            .iter()
            .any(|unit| unit.id == boat.id && (unit.x, unit.y) == (x, y))
        {
            for id in cargo {
                self.try_move_unit(id, dx, dy);
            }
        }
    }

    pub(super) fn choose_ai_rates(&mut self, owner: usize) {
        let government = self.players[owner].government;
        if government == Government::Anarchy {
            return;
        }
        let cap = government.max_rate();
        let cities: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| (city, self.city_yields(city).2))
            .collect();
        let upkeep: i32 = cities
            .iter()
            .map(|(city, _)| self.building_upkeep(city) + self.city_unit_upkeep(city).gold)
            .sum();
        let mut choices = Vec::new();
        for science_rate in (0..=cap).step_by(10) {
            for tax_rate in (0..=cap).step_by(10) {
                let luxury_rate = 100 - science_rate - tax_rate;
                if !(0..=cap).contains(&luxury_rate) {
                    continue;
                }
                let rates = [science_rate, tax_rate, luxury_rate];
                let (mut disorder, mut science, mut gold) = (0, 0, 0);
                for &(city, trade) in &cities {
                    let [bulbs, coins, luxury] = self.split_city_trade(city, trade, rates);
                    if self.city_in_disorder(city, luxury) {
                        disorder += 1;
                    } else {
                        science += bulbs;
                        gold += coins;
                    }
                }
                let score = (
                    disorder,
                    (upkeep - gold).max(0),
                    -science,
                    -gold,
                    luxury_rate,
                );
                choices.push((score, rates));
            }
        }
        if let Some(&(score, mut rates)) = choices.iter().min_by_key(|(score, _)| *score) {
            if self.players[owner].gold < self.ai_gold_reserve(owner)
                && let Some((_, refill)) = choices
                    .into_iter()
                    .filter(|(candidate, refill)| {
                        candidate.0 == score.0
                            && candidate.1 <= score.1
                            && refill[1] == rates[1] + 10
                    })
                    .min_by_key(|(candidate, _)| *candidate)
            {
                rates = refill;
            }
            self.players[owner].rates = rates;
        }
    }

    fn hunter_build_value(kind: UnitKind, shields: i32) -> f64 {
        let stats = kind.stats();
        // Both available hunter classes cost one food, shield and unhappy citizen.
        let mut desire =
            (stats.hp * stats.attack * stats.firepower * stats.moves + stats.defense) / 3;
        if kind == UnitKind::Submarine {
            desire += desire / 6; // Missile platform.
            desire -= desire / 4; // Cannot attack non-native tiles.
        }
        f64::from(desire) * (23.0_f64 / 24.0).powi(stats.cost / shields.max(1))
    }

    fn hunt_value(&self, hunter: &Unit, target: &Unit) -> Option<(i32, i32)> {
        if hunter.kind.stats().moves < target.kind.stats().moves
            && !self
                .previous_naval_positions
                .get(&target.id)
                .is_some_and(|&(x, y)| {
                    self.map_distance(hunter.x, hunter.y, target.x, target.y)
                        < self.map_distance(hunter.x, hunter.y, x, y)
                })
        {
            return None;
        }
        let path = self.path_for_unit(hunter, target.x, target.y);
        let distance = path.len() as i32;
        if distance == 0 || distance > hunter.kind.stats().moves * 6 {
            return None;
        }
        let Some(defender) = self
            .best_defender_id(hunter, target.x, target.y)
            .and_then(|id| self.units.iter().find(|unit| unit.id == id))
        else {
            return None;
        };
        let (attack, defense) = self.combat_firepower(hunter, defender);
        let win = combat_win_chance(
            Self::attack_power(hunter),
            hunter.hp,
            attack,
            self.defense_power_against(defender, target.x, target.y, hunter.kind),
            defender.hp,
            defense,
        );
        let mut cost = 0;
        let mut threat = 0;
        for unit in self
            .units
            .iter()
            .filter(|unit| (unit.x, unit.y) == (target.x, target.y))
        {
            cost += unit.kind.stats().cost;
            threat += unit.kind.stats().attack + 500 * i32::from(unit.kind.is_military());
        }
        let value = (threat * 9 + cost) / (distance + 1);
        ((f64::from(cost) * win) as i32 >= hunter.kind.stats().cost
            && value >= hunter.kind.stats().cost)
            .then_some((value, distance))
    }

    pub(super) fn hunter_production(&self, city: &City) -> Option<UnitKind> {
        let owner = city.owner;
        let targets: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.kind.is_naval()
                    && unit.transport.is_none()
                    && self.at_war(owner, unit.owner)
                    && self.tile(unit.x, unit.y).terrain.is_water()
                    && self.tile(unit.x, unit.y).known_by[owner]
            })
            .collect();
        if targets.is_empty() || !self.is_port(city) {
            return None;
        }
        let at_port = |unit: &Unit| unit.owner == owner && (unit.x, unit.y) == (city.x, city.y);
        let hunters = self.units.iter().filter(|unit| {
            unit.owner == owner && matches!(unit.kind, UnitKind::Submarine | UnitKind::Destroyer)
        });
        let needs_missile = hunters.clone().any(|unit| {
            at_port(unit) && unit.kind == UnitKind::Submarine && self.cargo_count(unit.id) == 0
        }) && !self.units.iter().any(|unit| {
            at_port(unit) && unit.kind == UnitKind::CruiseMissile && unit.transport.is_none()
        });
        if needs_missile && self.can_produce(city, Production::Unit(UnitKind::CruiseMissile)) {
            return Some(UnitKind::CruiseMissile);
        }
        let home_hunter = hunters.clone().any(|unit| unit.home_city == Some(city.id));
        let other_port_building_hunter = self.cities.iter().any(|other| {
            other.id != city.id
                && other.owner == owner
                && other.production == Production::Unit(UnitKind::Submarine)
        });
        if home_hunter || other_port_building_hunter || hunters.clone().any(at_port) {
            return None;
        }
        let shields = self.city_forecast(city).shield_surplus();
        let kind = [UnitKind::Destroyer, UnitKind::Submarine]
            .into_iter()
            .filter(|&kind| self.can_produce(city, Production::Unit(kind)))
            .max_by(|&a, &b| {
                Self::hunter_build_value(a, shields)
                    .total_cmp(&Self::hunter_build_value(b, shields))
            })?;
        let mut virtual_hunter = self.make_unit(0, owner, kind, city.x, city.y);
        virtual_hunter.home_city = Some(city.id);
        virtual_hunter.veteran = self.trained_rank(city, kind);
        targets
            .into_iter()
            .any(|target| self.hunt_value(&virtual_hunter, target).is_some())
            .then_some(kind)
    }

    fn load_hunter_missiles(&mut self, owner: usize) {
        let hunters: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == owner
                    && unit.kind == UnitKind::Submarine
                    && self
                        .cities
                        .iter()
                        .any(|city| city.owner == owner && (city.x, city.y) == (unit.x, unit.y))
            })
            .map(|unit| (unit.id, unit.x, unit.y))
            .collect();
        for (hunter, x, y) in hunters {
            let missiles: Vec<_> = self
                .units
                .iter()
                .filter(|unit| {
                    unit.owner == owner
                        && unit.kind == UnitKind::CruiseMissile
                        && unit.transport.is_none()
                        && (unit.x, unit.y) == (x, y)
                })
                .map(|unit| unit.id)
                .collect();
            for missile in missiles {
                if !self.board_specific_ship(missile, hunter) {
                    break;
                }
            }
        }
    }

    fn naval_hunt_target(
        &self,
        hunter: &Unit,
        claimed: &std::collections::HashSet<u32>,
    ) -> Option<(u32, i32, i32)> {
        let candidates: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.kind.is_naval()
                    && unit.transport.is_none()
                    && !claimed.contains(&unit.id)
                    && self.tile(unit.x, unit.y).terrain.is_water()
                    && self.at_war(hunter.owner, unit.owner)
                    && !self
                        .cities
                        .iter()
                        .any(|city| (city.x, city.y) == (unit.x, unit.y))
            })
            .filter_map(|unit| {
                self.hunt_value(hunter, unit)
                    .map(|(value, distance)| (value, distance, unit))
            })
            .collect();
        let &(best_value, _, best) = candidates.iter().max_by_key(|(value, distance, unit)| {
            (
                *value,
                std::cmp::Reverse(*distance),
                std::cmp::Reverse(unit.id),
            )
        })?;
        let target = self
            .hunter_targets
            .get(&hunter.id)
            .and_then(|old| candidates.iter().find(|(_, _, unit)| unit.id == *old))
            // Freeciv compares a replacement's distance-adjusted threat with
            // the original target's undiscounted threat.
            .filter(|(value, distance, _)| value * (distance + 1) >= best_value)
            .map_or(best, |(_, _, old)| *old);
        Some((target.id, target.x, target.y))
    }

    fn hunt_at_sea(&mut self, hunter: &Unit, claimed: &mut std::collections::HashSet<u32>) -> bool {
        let Some((target, x, y)) = self.naval_hunt_target(hunter, claimed) else {
            self.hunter_targets.remove(&hunter.id);
            return false;
        };
        self.hunter_targets.insert(hunter.id, target);
        claimed.insert(target);
        self.launch_hunter_missiles(hunter.id, target);
        if self.units.iter().any(|unit| unit.id == target) {
            self.goto_unit(hunter.id, x, y);
            self.launch_hunter_missiles(hunter.id, target);
        }
        true
    }

    fn launch_hunter_missiles(&mut self, hunter_id: u32, primary: u32) {
        if !self.units.iter().any(|unit| unit.id == primary) {
            return;
        }
        let Some(hunter) = self.units.iter().find(|unit| unit.id == hunter_id).cloned() else {
            return;
        };
        let missiles: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.transport == Some(hunter_id)
                    && unit.owner == hunter.owner
                    && unit.kind == UnitKind::CruiseMissile
                    && unit.moves_left > 0
            })
            .map(|unit| unit.id)
            .collect();
        for id in missiles {
            let Some(missile) = self.units.iter().find(|unit| unit.id == id) else {
                continue;
            };
            let target = self
                .units
                .iter()
                .filter(|unit| {
                    self.at_war(hunter.owner, unit.owner)
                        && unit.transport.is_none()
                        && self.tile(unit.x, unit.y).terrain.is_water()
                        && !self
                            .cities
                            .iter()
                            .any(|city| (city.x, city.y) == (unit.x, unit.y))
                        && (unit.id == primary
                            || (unit.kind.is_naval()
                                && unit.kind.stats().attack > hunter.kind.stats().defense))
                })
                .filter_map(|unit| {
                    let approach = self.path_to(hunter_id, unit.x, unit.y).len();
                    let flight = self.path_to(id, unit.x, unit.y).len();
                    (approach > 0
                        && flight > 0
                        && (approach.max(flight) as i32 - 1) * MOVE_FRAGMENTS < missile.moves_left
                        && self.best_defender_id(missile, unit.x, unit.y).is_some())
                    .then_some((approach, unit.id != primary, unit.id, unit.x, unit.y))
                })
                .min();
            if let Some((_, _, _, x, y)) = target {
                self.goto_unit(id, x, y);
                if !self.units.iter().any(|unit| unit.id == primary) {
                    break;
                }
            }
        }
    }

    fn counterattack_from_ai_cities(&mut self, owner: usize) -> std::collections::HashSet<u32> {
        let mut committed = std::collections::HashSet::new();
        let cities: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| (city.x, city.y))
            .collect();
        for (x, y) in cities {
            loop {
                let garrison: Vec<_> = self
                    .units
                    .iter()
                    .filter(|unit| {
                        unit.owner == owner
                            && (unit.x, unit.y) == (x, y)
                            && unit.kind.is_military()
                            && !unit.kind.is_naval()
                            && !unit.kind.is_aircraft()
                            && unit.transport.is_none()
                    })
                    .collect();
                if garrison.len() <= 1 {
                    break;
                }
                let mut best = None;
                let mut best_gain = 0.0;
                for attacker in garrison {
                    if attacker.moves_left <= 0
                        || attacker.work_left > 0
                        || attacker.state == UnitState::Sentry
                        || committed.contains(&attacker.id)
                    {
                        continue;
                    }
                    for (tx, ty) in super::terrain_work::neighbors(x, y) {
                        if !self.tile(tx, ty).known_by[owner]
                            || self.treaty_blocks_move(attacker, tx, ty)
                            || self.movement_cost_for(attacker, tx, ty).is_none()
                            || self.cities.iter().any(|city| (city.x, city.y) == (tx, ty))
                        {
                            continue;
                        }
                        let Some(defender) = self
                            .best_defender_id(attacker, tx, ty)
                            .and_then(|id| self.units.iter().find(|unit| unit.id == id))
                            .filter(|unit| unit.kind.is_military())
                        else {
                            continue;
                        };
                        let (attack_fp, defense_fp) = self.combat_firepower(attacker, defender);
                        let chance = combat_win_chance(
                            Self::attack_power(attacker),
                            attacker.hp,
                            attack_fp,
                            self.defense_power_against(defender, tx, ty, attacker.kind),
                            defender.hp,
                            defense_fp,
                        );
                        let gain = chance * f64::from(defender.kind.stats().cost)
                            - (1.0 - chance) * f64::from(attacker.kind.stats().cost);
                        if chance >= 0.65 && gain > best_gain {
                            best = Some((attacker.id, tx, ty));
                            best_gain = gain;
                        }
                    }
                }
                let Some((id, tx, ty)) = best else {
                    break;
                };
                let (dx, dy) = wrapped_map_vector(x, y, tx, ty);
                committed.insert(id);
                self.try_move_unit(id, dx, dy);
            }
        }
        committed
    }

    pub(super) fn reinforce_ai_cities(&mut self, owner: usize) -> std::collections::HashSet<u32> {
        let mut dispatched = std::collections::HashSet::new();
        if owner == BARBARIANS {
            return dispatched;
        }
        let cities: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| {
                (
                    city.id,
                    self.city_threats(city)
                        .map(|(_, danger)| danger)
                        .sum::<i32>(),
                )
            })
            .filter(|&(_, danger)| danger > 0)
            .collect();
        let mut cities = cities;
        cities.sort_by_key(|&(id, danger)| (std::cmp::Reverse(danger), id));
        let is_defender = |unit: &Unit| {
            unit.owner == owner
                && unit.kind.is_military()
                && !unit.kind.is_naval()
                && !unit.kind.is_aircraft()
                && unit.transport.is_none()
        };
        for &(city_id, _) in &cities {
            let city = self.cities.iter().find(|city| city.id == city_id).unwrap();
            let target = (city.x, city.y);
            let garrison: Vec<_> = self
                .units
                .iter()
                .filter(|unit| is_defender(unit) && (unit.x, unit.y) == target)
                .collect();
            let threatened: Vec<_> = self
                .city_threats(city)
                .filter_map(|(enemy, danger)| {
                    let odds = garrison
                        .iter()
                        .map(|unit| self.city_attacker_odds(city, enemy, unit, None))
                        .fold(1.0_f64, f64::min);
                    (odds > 0.5).then_some((enemy, danger, odds))
                })
                .collect();
            if threatened.is_empty() {
                continue;
            }
            let mut best = None;
            for unit in self.units.iter().filter(|unit| {
                is_defender(unit)
                    && unit.moves_left > 0
                    && unit.state != UnitState::Sentry
                    && unit.work_left == 0
                    && (unit.x, unit.y) != target
                    && !dispatched.contains(&unit.id)
            }) {
                if let Some(home) = self
                    .cities
                    .iter()
                    .find(|home| home.owner == owner && (home.x, home.y) == (unit.x, unit.y))
                    && (cities.iter().any(|&(id, _)| id == home.id)
                        || !self.units.iter().any(|guard| {
                            is_defender(guard)
                                && guard.id != unit.id
                                && (guard.x, guard.y) == (home.x, home.y)
                        }))
                {
                    continue;
                }
                let budget = unit.moves_left + self.unit_move_allowance(owner, unit.kind);
                if self.map_distance(unit.x, unit.y, city.x, city.y) > budget {
                    continue;
                }
                let Some(cost) = self.route_cost_within(unit, city.x, city.y, budget) else {
                    continue;
                };
                let mut stationed = unit.clone();
                (stationed.x, stationed.y) = target;
                stationed.state = UnitState::Idle;
                let gain = threatened
                    .iter()
                    .map(|&(enemy, danger, odds)| {
                        (odds - self.city_attacker_odds(city, enemy, &stationed, None)).max(0.0)
                            * f64::from(danger)
                    })
                    .sum::<f64>()
                    / f64::from(cost.max(1));
                if gain > 0.0 && best.as_ref().is_none_or(|&(_, previous)| gain > previous) {
                    best = Some((unit.id, gain));
                }
            }
            if let Some((id, _)) = best
                && self.goto_unit(id, target.0, target.1)
            {
                dispatched.insert(id);
            }
        }
        dispatched
    }

    fn ground_city_attack_path(&self, unit: &Unit) -> Option<Vec<(i32, i32)>> {
        let mut best = None;
        let mut best_value = 0.0;
        for city in self.cities.iter().filter(|city| {
            self.at_war(unit.owner, city.owner) && self.tile(city.x, city.y).known_by[unit.owner]
        }) {
            let Some((path, cost)) = self.search_route(unit, city.x, city.y, false, i32::MAX)
            else {
                continue;
            };
            if path.is_empty() {
                continue;
            }
            let defenders = self
                .units
                .iter()
                .filter(|defender| {
                    (defender.x, defender.y) == (city.x, city.y)
                        && !self.allied(unit.owner, defender.owner)
                })
                .count();
            let conquest = if unit.kind.can_occupy_city() && defenders <= 1 {
                f64::from(20 + 5 * city.size)
            } else {
                0.0
            };
            let gain = if let Some(defender) = self
                .best_defender_id(unit, city.x, city.y)
                .and_then(|id| self.units.iter().find(|defender| defender.id == id))
            {
                let (attack_fp, defense_fp) = self.combat_firepower(unit, defender);
                let attack = percent_of(
                    unit.kind.stats().attack * POWER_FACTOR,
                    unit.veteran_power(),
                );
                let chance = combat_win_chance(
                    attack,
                    unit.hp,
                    attack_fp,
                    self.defense_power_against(defender, city.x, city.y, unit.kind),
                    defender.hp,
                    defense_fp,
                );
                chance * (conquest + f64::from(defender.kind.stats().cost))
                    - (1.0 - chance) * f64::from(unit.kind.stats().cost)
            } else if defenders == 0 {
                conquest
            } else {
                continue;
            };
            let allowance = self.unit_move_allowance(unit.owner, unit.kind).max(1);
            let turns = 1 + ((cost - unit.moves_left).max(0) + allowance - 1) / allowance;
            let value = gain * (23.0_f64 / 24.0).powi(turns);
            if value > best_value {
                best_value = value;
                best = Some(path);
            }
        }
        best
    }

    fn escort_assault_transports(&mut self, owner: usize) -> std::collections::HashSet<u32> {
        use std::collections::HashSet;
        let mut assigned = HashSet::new();
        let mut screened = HashSet::new();
        if owner == BARBARIANS {
            return assigned;
        }
        let transports: Vec<_> = self
            .units
            .iter()
            .filter(|ship| {
                ship.owner == owner
                    && matches!(
                        ship.kind,
                        UnitKind::Trireme
                            | UnitKind::Caravel
                            | UnitKind::Galleon
                            | UnitKind::Transport
                    )
                    && self.units.iter().any(|cargo| {
                        cargo.transport == Some(ship.id)
                            && cargo.kind.is_military()
                            && !cargo.kind.is_aircraft()
                    })
            })
            .cloned()
            .collect();
        for transport in transports {
            let position = (transport.x, transport.y);
            if !screened.insert(position) {
                continue;
            }
            let threats: Vec<_> = self
                .units
                .iter()
                .filter(|enemy| {
                    enemy.kind.is_naval()
                        && enemy.kind.is_military()
                        && enemy.transport.is_none()
                        && self.at_war(owner, enemy.owner)
                        && self.tile(enemy.x, enemy.y).known_by[owner]
                        && self.map_distance(transport.x, transport.y, enemy.x, enemy.y)
                            <= enemy.kind.stats().moves + 1
                })
                .collect();
            if threats.is_empty() {
                continue;
            }

            let mut best = None;
            let mut best_score = 0.0;
            for escort in self.units.iter().filter(|unit| {
                unit.owner == owner
                    && unit.kind.is_naval()
                    && unit.kind.is_military()
                    && unit.kind != UnitKind::Submarine
                    && unit.transport.is_none()
                    && unit.moves_left > 0
                    && !assigned.contains(&unit.id)
                    && self.map_distance(unit.x, unit.y, transport.x, transport.y)
                        <= 2 * unit.kind.stats().moves
            }) {
                let path = self.path_for_unit(escort, transport.x, transport.y);
                if (escort.x, escort.y) != position && path.is_empty() {
                    continue;
                }
                let defense = escort.kind.stats().defense * escort.hp;
                let score = f64::from(defense) / (1.0 + path.len() as f64);
                if score > best_score {
                    best_score = score;
                    best = Some((escort, path));
                }
            }
            let Some((escort, screen_path)) = best else {
                continue;
            };
            let mut interception = None;
            let mut best_gain = 0.0;
            for enemy in threats {
                let path = self.path_for_unit(escort, enemy.x, enemy.y);
                if path.is_empty()
                    || path.len() as i32 * MOVE_FRAGMENTS > escort.moves_left
                    || self.best_defender_id(escort, enemy.x, enemy.y) != Some(enemy.id)
                    || self.treaty_blocks_move(escort, enemy.x, enemy.y)
                {
                    continue;
                }
                let (attack_fp, defense_fp) = self.combat_firepower(escort, enemy);
                let chance = combat_win_chance(
                    Self::attack_power(escort),
                    escort.hp,
                    attack_fp,
                    self.defense_power_against(enemy, enemy.x, enemy.y, escort.kind),
                    enemy.hp,
                    defense_fp,
                );
                let gain = chance * f64::from(enemy.kind.stats().cost)
                    - (1.0 - chance) * f64::from(escort.kind.stats().cost);
                if chance >= 0.65 && gain > best_gain {
                    best_gain = gain;
                    interception = Some(path);
                }
            }
            let id = escort.id;
            assigned.insert(id);
            self.follow_path(id, interception.unwrap_or(screen_path));
        }
        assigned
    }

    pub(super) fn run_ai_player(&mut self, owner: usize) {
        self.agent_city_targets.retain(|id, city| {
            self.units
                .iter()
                .any(|unit| unit.id == *id && unit.kind.is_diplomatic())
                && self.cities.iter().any(|target| target.id == *city)
        });
        let counterattacks = self.counterattack_from_ai_cities(owner);
        let reinforcements = self.reinforce_ai_cities(owner);
        let paratroopers: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == owner
                    && unit.kind == UnitKind::Paratroopers
                    && !counterattacks.contains(&unit.id)
                    && !reinforcements.contains(&unit.id)
            })
            .cloned()
            .collect();
        let drops: std::collections::HashSet<_> = paratroopers
            .iter()
            .filter_map(|unit| self.run_ai_paratrooper(unit).then_some(unit.id))
            .collect();
        self.load_hunter_missiles(owner);
        let ferry_dispatch = self.load_ai_merchant_ferries(owner);
        let settlement_ferries = self.dispatch_settlement_ferries(owner, &ferry_dispatch);
        let carriers: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == owner && unit.kind == UnitKind::Carrier && unit.transport.is_none()
            })
            .cloned()
            .collect();
        let air_carriers: std::collections::HashSet<_> = carriers
            .iter()
            .filter_map(|carrier| {
                (!ferry_dispatch.contains_key(&carrier.id)
                    && !settlement_ferries.contains_key(&carrier.id)
                    && self.position_ai_carrier(carrier))
                .then_some(carrier.id)
            })
            .collect();
        let fleet = self.coordinate_assault_fleets(
            owner,
            |id| ferry_dispatch.contains_key(&id) || settlement_ferries.contains_key(&id),
            |id| {
                counterattacks.contains(&id) || reinforcements.contains(&id) || drops.contains(&id)
            },
        );
        let assault = self.dispatch_assault_transports(
            owner,
            |id| {
                ferry_dispatch.contains_key(&id)
                    || settlement_ferries.contains_key(&id)
                    || fleet.contains(&id)
            },
            |id| {
                counterattacks.contains(&id)
                    || reinforcements.contains(&id)
                    || drops.contains(&id)
                    || fleet.contains(&id)
            },
        );
        let escorts = self.escort_assault_transports(owner);
        let mut claimed_targets = std::collections::HashSet::new();
        let unit_ids: Vec<u32> = self
            .units
            .iter()
            .filter(|u| u.owner == owner)
            .map(|u| u.id)
            .collect();
        for id in unit_ids {
            let Some(unit) = self.units.iter().find(|u| u.id == id).cloned() else {
                continue;
            };
            if matches!(
                unit.kind,
                UnitKind::Bomber
                    | UnitKind::StealthBomber
                    | UnitKind::Fighter
                    | UnitKind::StealthFighter
            ) {
                self.run_ai_strike_aircraft(&unit);
                continue;
            }
            if unit.transport.is_some()
                || air_carriers.contains(&id)
                || assault.contains(&id)
                || fleet.contains(&id)
                || escorts.contains(&id)
                || drops.contains(&id)
                || counterattacks.contains(&id)
                || reinforcements.contains(&id)
                || ferry_dispatch.values().any(|&passenger| passenger == id)
                || settlement_ferries.values().any(|plan| plan.founder == id)
            {
                continue;
            }
            if unit.kind.is_naval()
                && unit.kind != UnitKind::Submarine
                && unit.kind.transport_capacity() > 0
            {
                if let Some(plan) = settlement_ferries.get(&id) {
                    self.run_settlement_ferry(id, plan);
                    continue;
                }
                if !self.run_merchant_ferry(id, ferry_dispatch.get(&id).copied())
                    && !self.clean_current_tile(&unit)
                {
                    self.run_raider_transport(&unit);
                }
                continue;
            }
            if matches!(unit.kind, UnitKind::Submarine | UnitKind::Destroyer) {
                let waiting_for_missile = self.cargo_count(id) == 0
                    && self.cities.iter().any(|city| {
                        city.owner == owner
                            && (city.x, city.y) == (unit.x, unit.y)
                            && city.production == Production::Unit(UnitKind::CruiseMissile)
                    });
                if waiting_for_missile || self.hunt_at_sea(&unit, &mut claimed_targets) {
                    continue;
                }
            }
            match unit.kind {
                UnitKind::Settlers => {
                    if let Some((destination, path, _)) = self.best_city_site(&unit) {
                        self.follow_path(id, path);
                        if self
                            .units
                            .iter()
                            .any(|u| u.id == id && (u.x, u.y) == destination)
                        {
                            self.found_city(id);
                        }
                    }
                }
                UnitKind::Worker | UnitKind::Engineer | UnitKind::Migrants => {
                    self.run_automated_unit(id)
                }
                UnitKind::Diplomat | UnitKind::Spy => {
                    if self.ai_bribe_nearby(&unit) {
                        continue;
                    }
                    if self.assign_counterintelligence(&unit) {
                        self.agent_city_targets.remove(&unit.id);
                    } else if !self.ai_city_espionage(&unit) {
                        self.wander_unit(id);
                    }
                }
                UnitKind::Explorer | UnitKind::BarbarianLeader => self.wander_unit(id),
                UnitKind::Caravan | UnitKind::Freight => self.run_ai_merchant(&unit),
                _ => {
                    let on_city = self
                        .cities
                        .iter()
                        .find(|c| c.owner == owner && c.x == unit.x && c.y == unit.y)
                        .cloned();
                    if let Some(city) = on_city {
                        let defenders = self
                            .units
                            .iter()
                            .filter(|u| {
                                u.owner == owner
                                    && u.x == city.x
                                    && u.y == city.y
                                    && u.kind.is_military()
                            })
                            .count();
                        if defenders <= 1 || self.city_threats(&city).next().is_some() {
                            self.fortify_unit(id);
                            continue;
                        }
                    }
                    if unit.kind.is_military()
                        && !unit.kind.is_aircraft()
                        && !unit.kind.is_naval()
                        && owner != BARBARIANS
                    {
                        if let Some(path) = self.ground_city_attack_path(&unit) {
                            self.follow_path(id, path);
                        } else {
                            self.wander_unit(id);
                        }
                        continue;
                    }
                    if let Some(target) = self
                        .cities
                        .iter()
                        .filter(|c| self.at_war(c.owner, owner))
                        .min_by_key(|c| self.map_distance(unit.x, unit.y, c.x, c.y))
                        .cloned()
                    {
                        self.goto_unit(id, target.x, target.y);
                    } else {
                        self.wander_unit(id);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod hunter_tests {
    use super::*;

    #[test]
    fn faster_ship_is_huntable_only_on_an_intercept_vector() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (hunter, _, carrier) = game.ai_submarine_playtest_setup();
        let sub = game
            .units
            .iter()
            .find(|unit| unit.id == hunter)
            .unwrap()
            .clone();
        let target = game
            .units
            .iter()
            .find(|unit| unit.id == carrier)
            .unwrap()
            .clone();
        let destroyer = game.spawn_unit(HUMAN, UnitKind::Destroyer, target.x, target.y);
        game.units.last_mut().unwrap().hp = 1;
        let faster = game.units.last().unwrap().clone();
        game.previous_naval_positions
            .insert(destroyer, (faster.x, faster.y));
        assert_eq!(game.hunt_value(&sub, &faster), None);
        let farther = offset_native_by_map(sub.x, sub.y, 2, 0);
        game.previous_naval_positions.insert(destroyer, farther);
        assert!(game.hunt_value(&sub, &faster).is_some());
        assert_eq!(
            game.naval_hunt_target(&sub, &[carrier, destroyer].into_iter().collect()),
            None
        );
    }

    #[test]
    fn active_hunter_passes_over_a_cheap_ship_for_a_carrier() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (hunter, _, carrier) = game.ai_submarine_playtest_setup();
        let sub = game
            .units
            .iter()
            .find(|unit| unit.id == hunter)
            .unwrap()
            .clone();
        let (x, y) = offset_native_by_map(sub.x, sub.y, 0, 1);
        game.tile_mut(x, y).terrain = Terrain::Ocean;
        let transport = game.spawn_unit(HUMAN, UnitKind::Transport, x, y);
        game.units.last_mut().unwrap().hp = 1;
        assert_eq!(
            game.naval_hunt_target(&sub, &Default::default())
                .map(|target| target.0),
            Some(carrier)
        );
        game.remove_units_with_cargo(vec![carrier]);
        assert!(game.units.iter().any(|unit| unit.id == transport));
        assert_eq!(game.naval_hunt_target(&sub, &Default::default()), None);
    }

    #[test]
    fn hunter_production_compares_available_ships_and_rejects_cheap_targets() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (port, target) = game.hunter_production_playtest_setup();
        let city = game
            .cities
            .iter()
            .find(|city| city.id == port)
            .unwrap()
            .clone();
        assert_eq!(game.hunter_production(&city), Some(UnitKind::Submarine));
        game.players[city.owner]
            .known
            .retain(|&tech| tech != Tech::COMBUSTION);
        game.players[city.owner].known.push(Tech::ENGINEERING);
        assert_eq!(game.hunter_production(&city), Some(UnitKind::Destroyer));
        game.units
            .iter_mut()
            .find(|unit| unit.id == target)
            .unwrap()
            .kind = UnitKind::Transport;
        assert_eq!(game.hunter_production(&city), None);
    }
}
