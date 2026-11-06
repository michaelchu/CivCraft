use super::normal_paths::GotoTask;
use super::*;

impl Simulation {
    /// Our ferry itinerary stores a sea waypoint separately from the final
    /// passenger destination. Native amphibious PF fills the ferry parameter
    /// with that final land tile, permanently retaining it as goto_tile.
    pub(super) fn ai_ferry_goto_waypoint(
        &mut self,
        id: u32,
        waypoint: (i32, i32),
        destination: (i32, i32),
    ) {
        self.unit_goto_tiles.insert(id, destination);
        let Some(actor) = self.units.iter().find(|u| u.id == id).cloned() else {
            return;
        };
        if let Some((path, _)) = self.ai_goto_path(&actor, waypoint, GotoTask::None) {
            self.follow_source_ai_path(id, path);
        }
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
        if unit.kind.stats().can_work {
            self.run_auto_worker(unit_id);
            return;
        }
        if matches!(
            self.run_auto_explorer(unit_id),
            exploration::ExploreResult::Done | exploration::ExploreResult::Paused
        ) && let Some(unit) = self.units.iter_mut().find(|u| u.id == unit_id)
        {
            unit.automated = false;
        }
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
    ) -> Option<(i32, i32, i32, (i32, i32))> {
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
                    .filter_map(move |(dx, dy)| self.map_wrap.offset(city.x, city.y, dx, dy))
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
                ISOHEX_DIRS.into_iter().filter_map(move |(dx, dy)| {
                    self.map_wrap
                        .offset(lx, ly, -dx, -dy)
                        .map(|p| (p, (dx, dy)))
                })
            })
            .filter(|&((x, y), _)| self.tile(x, y).terrain.is_water())
            .filter_map(|((x, y), direction)| {
                let mut distance = 0;
                for ship in std::iter::once(boat).chain(companions.iter().copied()) {
                    if (ship.x, ship.y) != (x, y) {
                        let cost = self.route_cost_within(ship, x, y, i32::MAX)?;
                        let allowance = self.unit_move_rate(&ship).max(1);
                        let turns = ((cost - ship.moves_left).max(0) + allowance - 1) / allowance;
                        distance = distance.max(turns);
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

    fn military_ferry_ready(&self, boat: u32) -> bool {
        if self
            .units
            .iter()
            .find(|unit| unit.id == boat)
            .is_some_and(|unit| {
                unit.hp < unit.kind.stats().hp
                    && self
                        .cities
                        .iter()
                        .any(|city| (city.x, city.y) == (unit.x, unit.y))
            })
        {
            return false;
        }
        self.ai_ferries
            .boats
            .get(&boat)
            .is_none_or(|record| match record.state {
                ferries::FerryState::Available => true,
                ferries::FerryState::Assigned(id) => self.units.iter().any(|unit| {
                    unit.id == id
                        && unit.kind.is_military()
                        && !unit.kind.is_aircraft()
                        && !unit.kind.is_naval()
                        && unit.transport == Some(boat)
                }),
            })
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
                        && self.military_ferry_ready(boat.id)
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
                        let cost = self.route_cost_within(boat, city.x, city.y, i32::MAX)?;
                        let allowance = self.unit_move_rate(&boat).max(1);
                        Some((
                            boat,
                            ((cost - boat.moves_left).max(0) + allowance - 1) / allowance,
                        ))
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
                    self.unit_goto_tiles.insert(id, assembly);
                    self.ai_goto_unit(id, assembly, GotoTask::None);
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
                self.refresh_ferry_boss(ship);
                if let Some(target) = self.map_wrap.offset(x, y, dx, dy) {
                    self.ai_ferry_goto_waypoint(ship, (x, y), target);
                }
            }
            if ship_ids.iter().all(|id| {
                self.units
                    .iter()
                    .any(|ship| ship.id == *id && (ship.x, ship.y) == (x, y))
            }) {
                for id in troop_ids {
                    self.try_move_unit(id, dx, dy);
                    if self
                        .units
                        .iter()
                        .any(|unit| unit.id == id && unit.transport.is_none())
                    {
                        self.release_ai_ferry(id);
                    }
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
                    && self.military_ferry_ready(boat.id)
            })
            .cloned()
            .collect();
        struct Pickup {
            cost: i32,
            port: u32,
            position: (i32, i32),
            guard: u32,
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
                let Some((_, pickup_cost)) =
                    self.search_route(&boat, port.x, port.y, false, i32::MAX)
                else {
                    continue;
                };
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
                let allowance = self.unit_move_rate(&boat).max(1);
                let pickup_turns =
                    ((pickup_cost - boat.moves_left).max(0) + allowance - 1) / allowance;
                staged.moves_left =
                    (boat.moves_left + pickup_turns * allowance - pickup_cost).max(0);
                let origin = labels[Self::idx(port.x, port.y)];
                let Some((distance, _, _, _)) =
                    self.raider_landing(&staged, Some((&labels, origin)), Some(&troops), &[])
                else {
                    continue;
                };
                let cost = pickup_turns + distance;
                if best.as_ref().is_none_or(|previous| cost < previous.cost) {
                    best = Some(Pickup {
                        cost,
                        port: port.id,
                        position,
                        guard,
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
            self.unit_goto_tiles.insert(boat.id, plan.position);
            self.ai_goto_unit(boat.id, plan.position, GotoTask::None);
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

    pub(super) fn run_ai_ferry_after_handoff(&mut self, boat: u32) {
        if let Some(ship) = self.units.iter().find(|u| u.id == boat).cloned() {
            self.run_raider_transport(&ship);
        }
    }

    fn run_raider_transport(&mut self, boat: &Unit) {
        self.refresh_ferry_boss(boat.id);
        if !self.military_ferry_ready(boat.id) {
            return;
        }
        let cargo: Vec<_> = self
            .cargo_units_ordered(boat.id)
            .map(|unit| unit.id)
            .collect();
        if cargo.is_empty() {
            if boat.owner == BARBARIANS {
                self.wander_unit(boat.id);
            } else {
                self.run_idle_ai_ferry(boat.id);
            }
            return;
        }

        if let Some((dx, dy)) = ISOHEX_DIRS.into_iter().find(|&(dx, dy)| {
            let location = self.map_wrap.offset(boat.x, boat.y, dx, dy);
            self.cities.iter().any(|city| {
                Some((city.x, city.y)) == location
                    && self.at_war(city.owner, boat.owner)
                    && (boat.owner == BARBARIANS
                        || cargo.iter().any(|id| {
                            self.units
                                .iter()
                                .find(|unit| unit.id == *id && unit.kind.has_marine_attack())
                                .is_some_and(|marine| {
                                    self.assault_capture_chance(city, &[marine]) >= 0.5
                                })
                        }))
            })
        }) {
            for &id in &cargo {
                if self.units.iter().any(|unit| {
                    unit.id == id
                        && unit.kind.has_marine_attack()
                        && (boat.owner == BARBARIANS
                            || self.cities.iter().any(|city| {
                                let target = self.map_wrap.offset(boat.x, boat.y, dx, dy);
                                Some((city.x, city.y)) == target
                                    && self.assault_capture_chance(city, &[unit]) >= 0.5
                            }))
                }) && self.try_move_unit(id, dx, dy) != MoveResult::Blocked
                {
                    if self
                        .units
                        .iter()
                        .any(|unit| unit.id == id && unit.transport.is_none())
                    {
                        self.release_ai_ferry(id);
                    }
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
        let Some((_, x, y, (dx, dy))) = landing else {
            if boat.owner == BARBARIANS {
                self.wander_unit(boat.id);
            }
            return;
        };
        if (boat.x, boat.y) != (x, y) {
            if let Some(target) = self.map_wrap.offset(x, y, dx, dy) {
                self.ai_ferry_goto_waypoint(boat.id, (x, y), target);
            }
        }
        if self
            .units
            .iter()
            .any(|unit| unit.id == boat.id && (unit.x, unit.y) == (x, y))
        {
            for id in cargo {
                self.try_move_unit(id, dx, dy);
                if self
                    .units
                    .iter()
                    .any(|unit| unit.id == id && unit.transport.is_none())
                {
                    self.release_ai_ferry(id);
                }
            }
        }
    }

    pub(super) fn choose_ai_rates(&mut self, owner: usize) {
        self.manage_ai_taxes(owner);
    }

    // Civ2Civ3's hostile-action enablers: ordinary combat (including
    // attack-zero military types), diplomatic attacks, and Capture Units.
    fn hunter_type_acts_hostile(kind: UnitKind) -> bool {
        use super::unit_catalog::UnitFlag;
        !kind.has_flag(UnitFlag::NonMil)
            || kind.has_flag(UnitFlag::Diplomat)
            || kind.has_flag(UnitFlag::Spy)
            || kind.has_flag(UnitFlag::Capturer)
    }

    fn hunter_juiciness(&self, target: &Unit) -> (i32, i32) {
        use super::unit_catalog::UnitFlag;
        let mut cost = 0;
        let mut threat = 0;
        for unit in self.tile_units_ordered(target.x, target.y) {
            let stats = unit.kind.stats();
            threat += stats.attack * stats.hp * stats.firepower;
            if unit.kind.has_flag(UnitFlag::GameLoss) {
                cost += 1000;
                threat += 5000;
            }
            if Self::hunter_type_acts_hostile(unit.kind) {
                threat += 500;
            }
            cost += stats.cost;
        }
        (threat * 9 + cost, cost)
    }

    fn hunter_candidate_value(&self, hunter: &Unit, target: &Unit, distance: i32) -> Option<i32> {
        // Chase comparisons use the unit TYPE's unmodified speed. Damage,
        // veterancy and positional effects affect PF, but not this decision.
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
        let (threat, cost) = self.hunter_juiciness(target);
        let cost = if let Some(defender) = self
            .best_defender_id(hunter, target.x, target.y)
            .and_then(|id| self.units.iter().find(|unit| unit.id == id))
        {
            let (attack, defense) = self.combat_firepower(hunter, defender);
            let win = combat_win_chance(
                Self::attack_power(hunter),
                hunter.hp,
                attack,
                self.defense_power_against(defender, target.x, target.y, hunter.kind),
                defender.hp,
                defense,
            );
            (f64::from(cost) * win) as i32
        } else {
            cost
        };
        let value = threat / (distance + 1);
        (cost >= hunter.kind.stats().cost && value >= hunter.kind.stats().cost).then_some(value)
    }

    fn hunt_value(&self, hunter: &Unit, target: &Unit) -> Option<(i32, i32)> {
        let limit = 6 * self.unit_move_rate(hunter);
        let distance = super::normal_paths::NormalPaths::enemy_map(self, hunter)
            .movement_cost(self, target.x, target.y)?;
        if distance == 0 || distance > limit {
            return None;
        }
        self.hunter_candidate_value(hunter, target, distance)
            .map(|value| (value, distance))
    }

    pub(super) fn hunter_production_choice(&self, _city: &City) -> Option<(UnitKind, f64)> {
        // difficulty.c's Normal profile has H_TARGETS. daihunter.c's
        // dai_hunter_choice returns before both missile and hunter advice when
        // that handicap is set. Existing Hunter-role units are still managed.
        // Hard-profile guess_best/hunter production is not implemented.
        None
    }

    pub(crate) fn normal_hunter_production_playtest(&self) -> (bool, bool, bool) {
        let mut game = self.clone();
        let (port, target) = game.hunter_production_playtest_setup();
        let city = game.cities.iter().find(|c| c.id == port).unwrap().clone();
        let mut hunter = game.make_unit(0, city.owner, UnitKind::Submarine, city.x, city.y);
        hunter.home_city = Some(port);
        let target = game.units.iter().find(|u| u.id == target).unwrap().clone();
        let worthwhile = game.hunt_value(&hunter, &target).is_some();
        let hull_suppressed = game.hunter_production_choice(&city).is_none();
        game.spawn_unit(city.owner, UnitKind::Submarine, city.x, city.y);
        let ammo_available = game.can_produce(&city, Production::Unit(UnitKind::CruiseMissile));
        (
            worthwhile,
            hull_suppressed,
            ammo_available && game.hunter_production_choice(&city).is_none(),
        )
    }

    /// Source tactical hunter policies, isolated from the dedicated production
    /// gate (which Normal AI's handicap disables).
    pub(crate) fn native_hunter_tactics_playtest(&self) -> (bool, bool, bool, bool) {
        use super::unit_catalog::UnitFlag;
        let mut sim = self.clone();
        sim.units.clear();
        sim.cities.clear();
        sim.hunter_targets.clear();
        sim.previous_naval_positions.clear();
        sim.ai_ground_targets.clear();
        sim.sign_pact(1, HUMAN, Pact::War);
        for tile in &mut sim.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.hut = false;
            tile.base = None;
            tile.known_by.fill(false);
        }
        let start = (20, 30);
        let near = offset_native_by_map(start.0, start.1, 2, 0);
        let far = offset_native_by_map(start.0, start.1, 5, 0);
        let hunter_id = sim.spawn_unit(1, UnitKind::Horseman, start.0, start.1);
        let near_id = sim.spawn_unit(HUMAN, UnitKind::Diplomat, near.0, near.1);
        let far_id = sim.spawn_unit(HUMAN, UnitKind::Leader, far.0, far.1);
        let hunter = sim
            .units
            .iter()
            .find(|u| u.id == hunter_id)
            .unwrap()
            .clone();
        // A much richer GameLoss target must not replace an already worthwhile
        // earlier PF tile. Both enemy squares are unknown to this Normal AI.
        let first = sim.hunter_target(&hunter, &Default::default()).map(|v| v.0);
        let far_unit = sim.units.iter().find(|u| u.id == far_id).unwrap();
        let richer = sim.hunt_value(&hunter, far_unit).is_some_and(|(v, _)| {
            sim.hunt_value(&hunter, sim.units.iter().find(|u| u.id == near_id).unwrap())
                .is_some_and(|(near_value, _)| v > near_value)
        });
        let normal_omni = first == Some(near_id)
            && !sim.tile(near.0, near.1).known_by[1]
            && !sim.tile(far.0, far.1).known_by[1];
        sim.run_ai_player(1);
        let land = first == Some(near_id)
            && richer
            && sim
                .units
                .iter()
                .any(|u| u.id == hunter_id && (u.x, u.y) != start);

        let mut stack = self.clone();
        stack.units.clear();
        stack.cities.clear();
        stack.sign_pact(1, HUMAN, Pact::War);
        let leader = stack.spawn_unit(HUMAN, UnitKind::Leader, near.0, near.1);
        let target = stack.units.iter().find(|u| u.id == leader).unwrap();
        let stats = target.kind.stats();
        let (threat, cost) = stack.hunter_juiciness(target);
        let gameloss = target.kind.has_flag(UnitFlag::GameLoss)
            && cost == stats.cost + 1000
            && threat
                == (stats.attack * stats.hp * stats.firepower
                    + 5000
                    + 500 * i32::from(Self::hunter_type_acts_hostile(target.kind)))
                    * 9
                    + cost;

        let mut speed = sim.clone();
        let target = speed.spawn_unit(HUMAN, UnitKind::Cavalry, far.0, far.1);
        let mut fast = speed.units.iter().find(|u| u.id == target).unwrap().clone();
        fast.hp = 1;
        speed.previous_naval_positions.insert(target, far);
        let base_speed = speed.hunter_candidate_value(&hunter, &fast, 6).is_none()
            && speed.unit_move_rate(&fast) <= speed.unit_move_rate(&hunter);
        (land, normal_omni, gameloss, base_speed)
    }

    fn hunter_target(
        &self,
        hunter: &Unit,
        claimed: &std::collections::HashSet<u32>,
    ) -> Option<(u32, i32, i32)> {
        use super::unit_catalog::UnitFlag;
        let original = self
            .hunter_targets
            .get(&hunter.id)
            .and_then(|id| self.units.iter().find(|unit| unit.id == *id));
        let original_threat = original.map_or(0, |unit| self.hunter_juiciness(unit).0);
        let limit = 6 * self.unit_move_rate(hunter);
        let mut map = super::normal_paths::NormalPaths::enemy_map(self, hunter);
        // Native PF order, then native tile-list order; this is deliberately
        // the FIRST worthwhile stack, rather than the globally richest stack.
        for (x, y) in map.reachable_within(self, limit) {
            if self.cities.iter().any(|city| (city.x, city.y) == (x, y))
                || self
                    .tile_units_ordered(x, y)
                    .any(|unit| !self.at_war(hunter.owner, unit.owner))
            {
                continue;
            }
            let distance = map.movement_cost(self, x, y)?;
            for target in self.tile_units_ordered(x, y) {
                if !self.advisor_player_dangerous(hunter.owner, target.owner)
                    || claimed.contains(&target.id)
                    || (!Self::hunter_type_acts_hostile(target.kind)
                        && target.kind.transport_capacity() == 0
                        && !target.kind.has_flag(UnitFlag::GameLoss))
                    || !self.pf_physical_attack(hunter, target.kind, x, y)
                {
                    continue;
                }
                let Some(value) = self.hunter_candidate_value(hunter, target, distance) else {
                    continue;
                };
                if original.is_some_and(|unit| unit.id != target.id) && original_threat > value {
                    continue;
                }
                return Some((target.id, x, y));
            }
        }
        None
    }

    // Return -1 for a completed kill, 0 for no task/death, positive for an
    // unfinished hunt. dai_manage_military retries a successful hunt once.
    fn manage_hunter(
        &mut self,
        hunter: &Unit,
        claimed: &mut std::collections::HashSet<u32>,
    ) -> i32 {
        let Some((target, x, y)) = self.hunter_target(hunter, claimed) else {
            if self.hunter_targets.remove(&hunter.id).is_some() {
                self.unit_goto_tiles.remove(&hunter.id);
            }
            return 0;
        };
        self.hunter_targets.insert(hunter.id, target);
        claimed.insert(target);
        self.clear_ai_charge(hunter.id);
        self.unit_goto_tiles.insert(hunter.id, (x, y));
        // dai_unit_new_task(AIUNIT_HUNTER) grabs eligible missiles in native
        // tile-list order wherever the hunt begins, not only in a home port.
        let missiles: Vec<_> = self
            .tile_units_ordered(hunter.x, hunter.y)
            .filter(|missile| {
                missile.owner == hunter.owner
                    && missile.kind.is_missile()
                    && missile.transport.is_none()
                    && !self.ai_guard_targets.contains_key(&missile.id)
            })
            .map(|missile| missile.id)
            .collect();
        for missile in missiles {
            self.board_specific_ship(missile, hunter.id);
        }
        self.launch_hunter_missiles(hunter.id, target);
        if self.units.iter().any(|unit| unit.id == target) {
            let path = super::normal_paths::NormalPaths::enemy_map(self, hunter).path(self, x, y);
            if let Some((path, _)) = path {
                if !self.follow_source_ai_path(hunter.id, path).alive {
                    return 0;
                }
            }
            if self.units.iter().any(|unit| unit.id == target) {
                self.launch_hunter_missiles(hunter.id, target);
            }
        }
        if !self.units.iter().any(|unit| unit.id == target) {
            self.hunter_targets.remove(&hunter.id);
            self.unit_goto_tiles.remove(&hunter.id);
            claimed.remove(&target);
            return -1;
        }
        1
    }

    fn run_existing_hunter(
        &mut self,
        hunter: &Unit,
        claimed: &mut std::collections::HashSet<u32>,
    ) -> bool {
        let result = self.manage_hunter(hunter, claimed);
        if result == -1 {
            if let Some(actor) = self.units.iter().find(|unit| unit.id == hunter.id).cloned() {
                self.manage_hunter(&actor, claimed);
            }
        }
        result != 0 || !self.units.iter().any(|unit| unit.id == hunter.id)
    }

    fn launch_hunter_missiles(&mut self, hunter_id: u32, primary: u32) {
        self.source_hunter_launch(hunter_id, primary);
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
                        || self.ai_guard_targets.contains_key(&attacker.id)
                    {
                        continue;
                    }
                    for (tx, ty) in self.map_wrap.neighbors(x, y) {
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
                let (dx, dy) = self.map_wrap.vector(x, y, tx, ty);
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
                    && !self.ai_guard_targets.contains_key(&unit.id)
            }) {
                if let Some(home) = self
                    .cities
                    .iter()
                    .find(|home| home.owner == owner && (home.x, home.y) == (unit.x, unit.y))
                    && ((cities.iter().any(|&(id, _)| id == home.id)
                        && (self.city_grave_danger(home) > 0 || self.city_grave_danger(city) == 0))
                        || !self.units.iter().any(|guard| {
                            is_defender(guard)
                                && guard.id != unit.id
                                && (guard.x, guard.y) == (home.x, home.y)
                        }))
                {
                    continue;
                }
                let budget = unit.moves_left + self.unit_move_rate(&unit);
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
            if let Some((id, _)) = best {
                self.unit_goto_tiles.insert(id, target);
                self.ai_guard_for_destination(id, target.0, target.1);
                if self.ai_goto_unit(id, target, GotoTask::Defend).moved {
                    dispatched.insert(id);
                }
            }
        }
        dispatched
    }

    pub(crate) fn ai_defender_forecast_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_ground_targets.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.resource = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.known_by[1] = true;
        }
        self.sign_pact(1, HUMAN, Pact::War);
        let origin = (10, 24);
        let target = offset_native_by_map(origin.0, origin.1, 5, 0);
        self.create_city(HUMAN, target.0, target.1);
        self.cities[0].buildings.push(Building::BARRACKS_II);
        self.players[HUMAN].known = vec![Tech::GUNPOWDER];
        self.players[HUMAN].gold = 0;
        let guard = self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
        self.units.last_mut().unwrap().hp = 1;
        let id = self.spawn_unit(1, UnitKind::Marines, origin.0, origin.1);
        let attacker = self.units.iter().find(|u| u.id == id).unwrap().clone();
        let city = &self.cities[0];
        let near = self.anticipated_city_defender(city, &attacker, 1, false);
        let far = self.anticipated_city_defender(city, &attacker, 2, false);
        let immediate = near.is_some_and(|u| u.id == guard && u.hp == 1);
        let prepared = far.is_some_and(|u| {
            u.kind == UnitKind::Musketeers && u.veteran > 0 && u.hp == u.kind.stats().hp
        });
        let avoided = self.ground_city_attack_choice(&attacker).is_none();
        self.players[HUMAN].known.clear();
        self.cities[0].buildings.clear();
        immediate && prepared && avoided && self.ground_city_attack_choice(&attacker).is_some()
    }

    pub(crate) fn ai_ground_invasion_playtest_setup(&mut self) -> (u32, u32, u32) {
        self.units.clear();
        self.cities.clear();
        self.ai_ground_targets.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
            tile.base = None;
            tile.hut = false;
            tile.known_by[1] = true;
        }
        self.sign_pact(1, HUMAN, Pact::War);
        let home = (10, 24);
        let front = offset_native_by_map(home.0, home.1, 6, 0);
        self.create_city(1, home.0, home.1);
        self.create_city(HUMAN, front.0, front.1);
        let city = self.cities.last().unwrap().id;
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        // daiair's strict MC < currentMP target bound excludes OneAttack
        // bombers, whose native PF attack endpoint consumes a whole turn.
        // A Fighter exercises the actual supported combined-air mission.
        let bomber = self.spawn_unit(1, UnitKind::Fighter, home.0, home.1);
        let departure = offset_native_by_map(home.0, home.1, 1, 0);
        let infantry = self.spawn_unit(1, UnitKind::Marines, departure.0, departure.1);
        self.spawn_unit(HUMAN, UnitKind::Warrior, front.0, front.1);
        self.units.last_mut().unwrap().hp = 2;
        self.set_random_seed(1);
        self.refresh_ground_invasions(1, &Default::default());
        (infantry, bomber, city)
    }

    // daiunit.c:single_invader/invasion_funct. A real GOTO marks its
    // destination even when distant; mere proximity marks undefended cities.
    pub(super) fn invasion_support(&self, owner: usize, city: &City, except: u32) -> (i32, i32) {
        if !self.at_war(owner, city.owner) {
            return (0, 0);
        }
        let undefended = !self.units.iter().any(|u| {
            (u.x, u.y) == (city.x, city.y)
                && u.kind.stats().defense > 0
                && !self.allied(owner, u.owner)
        });
        let mut attacks = 0;
        let mut occupiers = 0;
        for actor in self.units.iter().filter(|u| {
            u.owner == owner && u.id != except && u.kind.is_military() && u.transport.is_none()
        }) {
            let assigned = self.ai_ground_targets.get(&actor.id) == Some(&city.id);
            let radius = self.unit_move_rate(&actor) / MOVE_FRAGMENTS;
            let local = undefended && self.map_distance(actor.x, actor.y, city.x, city.y) <= radius;
            let marks = i32::from(assigned) + i32::from(local);
            if marks == 0 {
                continue;
            }
            attacks += marks
                * if actor.kind.bombard_rate() > 0 {
                    1
                } else {
                    actor.kind.stats().moves * MOVE_FRAGMENTS
                };
            occupiers += marks * i32::from(actor.kind.can_occupy_city());
            for cargo in self
                .units
                .iter()
                .filter(|u| u.transport == Some(actor.id) && u.kind.is_military())
            {
                attacks += marks * cargo.kind.stats().moves * MOVE_FRAGMENTS;
                occupiers += marks * i32::from(cargo.kind.can_occupy_city());
            }
        }
        // Nonattacking ferries still carry occupation-capable reinforcements.
        for boat in self.units.iter().filter(|u| {
            u.owner == owner && u.id != except && !u.kind.is_military() && u.kind.is_naval()
        }) {
            let assigned = self.ai_ground_targets.get(&boat.id) == Some(&city.id);
            let local = undefended && self.map_distance(boat.x, boat.y, city.x, city.y) <= 2;
            if assigned || local {
                for cargo in self
                    .units
                    .iter()
                    .filter(|u| u.transport == Some(boat.id) && u.kind.is_military())
                {
                    let marks = i32::from(assigned) + i32::from(local);
                    attacks += marks * cargo.kind.stats().moves * MOVE_FRAGMENTS;
                    occupiers += marks * i32::from(cargo.kind.can_occupy_city());
                }
            }
        }
        (attacks, occupiers)
    }

    fn refresh_ground_invasions(
        &mut self,
        owner: usize,
        reserved: &std::collections::HashSet<u32>,
    ) {
        let valid: std::collections::HashSet<_> = self
            .ai_ground_targets
            .iter()
            .filter_map(|(&id, &target)| {
                let actor = self.units.iter().find(|u| u.id == id)?;
                let city = self.cities.iter().find(|c| c.id == target)?;
                let invader =
                    actor.kind.is_military() && !actor.kind.is_aircraft() && !actor.kind.is_naval();
                let ferry = actor.kind.is_naval()
                    && self
                        .units
                        .iter()
                        .any(|cargo| cargo.transport == Some(id) && cargo.kind.can_occupy_city());
                (self.at_war(actor.owner, city.owner) && (invader || ferry)).then_some(id)
            })
            .collect();
        self.ai_ground_targets.retain(|id, _| valid.contains(id));
        let boats: Vec<_> =
            self.units
                .iter()
                .filter(|u| {
                    u.owner == owner
                        && u.kind.is_naval()
                        && self.units.iter().any(|cargo| {
                            cargo.transport == Some(u.id) && cargo.kind.can_occupy_city()
                        })
                })
                .cloned()
                .collect();
        for boat in boats {
            let troops: Vec<_> = self
                .units
                .iter()
                .filter(|u| u.transport == Some(boat.id) && u.kind.is_military())
                .collect();
            let target = self
                .raider_landing(&boat, None, Some(&troops), &[])
                .and_then(|(_, x, y, (dx, dy))| {
                    let shore = self.map_wrap.offset(x, y, dx, dy)?;
                    self.cities
                        .iter()
                        .filter(|c| {
                            self.at_war(owner, c.owner)
                                && self.map_distance(shore.0, shore.1, c.x, c.y) <= 1
                                && self.assault_capture_chance(c, &troops) >= 0.5
                        })
                        .min_by_key(|c| c.id)
                        .map(|c| c.id)
                });
            if let Some(target) = target {
                self.ai_ground_targets.insert(boat.id, target);
            } else {
                self.ai_ground_targets.remove(&boat.id);
            }
        }
        let actors: Vec<_> = self
            .units
            .iter()
            .filter(|u| {
                u.owner == owner
                    && u.kind.is_military()
                    && !u.kind.is_aircraft()
                    && !u.kind.is_naval()
                    && u.transport.is_none()
            })
            .cloned()
            .collect();
        for unit in actors {
            let guarding = self.cities.iter().any(|c| {
                c.owner == owner
                    && (c.x, c.y) == (unit.x, unit.y)
                    && (self
                        .units
                        .iter()
                        .filter(|u| {
                            u.owner == owner && u.kind.is_military() && (u.x, u.y) == (c.x, c.y)
                        })
                        .count()
                        <= 1
                        || self.city_threats(c).next().is_some())
            });
            if reserved.contains(&unit.id) || guarding {
                self.ai_ground_targets.remove(&unit.id);
                continue;
            }
            if let Some((target, _)) = self.ground_city_attack_choice(&unit) {
                self.ai_ground_targets.insert(unit.id, target);
            } else {
                self.ai_ground_targets.remove(&unit.id);
            }
        }
    }

    pub(super) fn ground_attack_is_my_turn(&self, actor: &Unit, defender: &Unit) -> bool {
        // daiunit.c:is_my_turn: strongest ready attacker against this defender
        // strikes first, so a weak unit cannot sacrifice the entire stack.
        let rating = |unit: &Unit| {
            Self::attack_power(unit) * unit.hp * unit.kind.stats().firepower / (POWER_FACTOR * 3)
        };
        let own = rating(actor);
        let own_defense = self.defense_power_against(defender, defender.x, defender.y, actor.kind);
        for other in self.units.iter().filter(|u| {
            u.id != actor.id
                && u.owner == actor.owner
                && u.moves_left > 0
                && u.kind.is_military()
                && !u.kind.is_aircraft()
                && !u.kind.is_naval()
                && (u.transport.is_none() || u.kind.has_marine_attack())
                && self.map_distance(u.x, u.y, defender.x, defender.y) <= 1
                && !self.treaty_blocks_move(u, defender.x, defender.y)
                && self.best_defender_id(u, defender.x, defender.y).is_some()
        }) {
            let defense = self.defense_power_against(defender, defender.x, defender.y, other.kind);
            if defense == 0 {
                return true;
            }
            if rating(other) * own_defense / defense > own {
                return false;
            }
        }
        true
    }

    fn follow_ground_invasion_path(&mut self, id: u32, path: Vec<(i32, i32)>) {
        let Some(actor) = self.units.iter().find(|u| u.id == id).cloned() else {
            return;
        };
        let goal = path.iter().fold((actor.x, actor.y), |(x, y), &(dx, dy)| {
            self.map_wrap
                .offset(x, y, dx, dy)
                .expect("planned step is real")
        });
        if goal == (actor.x, actor.y) {
            return;
        }
        self.ai_guard_for_destination(id, goal.0, goal.1);
        self.unit_goto_tiles.insert(id, goal);
        let Some((path, _)) = self.ai_goto_path(&actor, goal, GotoTask::Attack) else {
            return;
        };
        let length = path.len();
        for (position, (dx, dy)) in path.into_iter().enumerate() {
            let Some(actor) = self.units.iter().find(|u| u.id == id).cloned() else {
                return;
            };
            let Some(target) = self.map_wrap.offset(actor.x, actor.y, dx, dy) else {
                break;
            };
            let enemy =
                self.units
                    .iter()
                    .any(|u| (u.x, u.y) == target && self.at_war(actor.owner, u.owner))
                    || self.cities.iter().any(|city| {
                        (city.x, city.y) == target && self.at_war(actor.owner, city.owner)
                    });
            if actor.moves_left <= 0 || enemy && position + 1 != length {
                return;
            }
            // adv_follow_path postpones a peaceful end-of-turn step into
            // danger. Keep this beside the specialized attacker ordering
            // below; routing through the generic executor would lose it.
            let cost = self.raw_move_cost_for(&actor, target.0, target.1);
            if !enemy
                && actor.moves_left <= cost
                && self.unit_move_rate(&actor) > cost
                && self.explorer_danger_at(&actor, target.0, target.1)
                && !self.explorer_danger_at(&actor, actor.x, actor.y)
            {
                return;
            }
            if self
                .best_defender_id(&actor, target.0, target.1)
                .and_then(|id| self.units.iter().find(|u| u.id == id))
                .is_some_and(|defender| !self.ground_attack_is_my_turn(&actor, defender))
            {
                return;
            }
            let result = self.try_move_unit(id, dx, dy);
            if result == MoveResult::Blocked
                || result == MoveResult::Fought
                || self
                    .units
                    .iter()
                    .find(|u| u.id == id)
                    .is_none_or(|u| u.moves_left <= 0)
            {
                return;
            }
        }
    }

    fn ground_city_attack_choice(&self, unit: &Unit) -> Option<(u32, Vec<(i32, i32)>)> {
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
            let (invasion_attacks, invasion_occupiers) =
                self.invasion_support(unit.owner, city, unit.id);
            let allowance = self.unit_move_rate(&unit).max(1);
            let turns = (cost + allowance - unit.moves_left.max(0)) / allowance;
            if turns > 10 {
                continue;
            }
            let rating = |actor: &Unit| {
                Self::attack_power(actor) * actor.hp * actor.kind.stats().firepower
                    / (POWER_FACTOR * 3)
            };
            let helpers: Vec<_> = self
                .units
                .iter()
                .filter_map(|u| {
                    if u.id == unit.id || !self.allied(unit.owner, u.owner) || rating(u) == 0 {
                        return None;
                    }
                    let nearby = i32::from(self.map_distance(u.x, u.y, city.x, city.y) <= 1);
                    let assigned = i32::from(self.ai_ground_targets.get(&u.id) == Some(&city.id));
                    (nearby + assigned > 0).then_some((u, nearby + assigned))
                })
                .collect();
            let attack = rating(unit)
                + helpers
                    .iter()
                    .map(|(u, count)| rating(u) * count)
                    .sum::<i32>();
            let attack = f64::from(attack).powi(2);
            let loss = unit.kind.stats().cost
                + helpers
                    .iter()
                    .map(|(u, count)| u.kind.stats().cost * count)
                    .sum::<i32>();
            let anticipated = self.anticipated_city_defender(city, unit, turns, false);
            let defender = anticipated.as_ref();
            let mut benefit = defender.map_or(0.0, |d| f64::from(d.kind.stats().cost));
            let reserves = invasion_attacks - defenders as i32;
            if reserves > 0 && (unit.kind.can_occupy_city() || invasion_occupiers > 0) {
                benefit += f64::from(self.advisor_city_output(city, None).max(0) * reserves) / 5.0;
            }
            let vulnerability = defender.map_or(0.0, |d| {
                let rating = self.defense_power_against(d, city.x, city.y, unit.kind)
                    * d.hp
                    * d.kind.stats().firepower
                    / (POWER_FACTOR * 3);
                f64::from(rating).powi(2)
            });
            let victims = defenders as f64 + 1.0;
            let gain = if unit.kind.can_occupy_city()
                && invasion_occupiers == 0
                && (invasion_attacks > 0 || defenders == 0)
            {
                f64::from(unit.kind.stats().cost * 17)
            } else if !unit.kind.can_occupy_city() && defender.is_none() {
                0.0
            } else if attack + vulnerability * victims > 0.0 {
                (benefit * attack - f64::from(loss) * vulnerability) * victims * 17.0
                    / (attack + vulnerability * victims)
            } else {
                0.0
            };
            let gain = gain - f64::from(turns * 17);
            let production: Vec<_> = self
                .cities
                .iter()
                .filter(|c| c.owner == unit.owner)
                .map(|c| self.city_forecast(c).shield_surplus())
                .collect();
            let average = production.iter().sum::<i32>() / production.len().max(1) as i32;
            let output = self
                .cities
                .iter()
                .find(|c| Some(c.id) == unit.home_city)
                .map_or(1, |c| self.city_forecast(c).shield_surplus())
                .max(average)
                .max(1);
            let stats = unit.kind.stats();
            let balanced = 2 * stats.cost * stats.attack / (stats.attack + stats.defense).max(1);
            let value = gain * (23.0_f64 / 24.0).powi(turns.max(1) + balanced / output);
            if value > best_value {
                best_value = value;
                best = Some((city.id, path));
            }
        }
        best
    }

    pub(super) fn run_ai_player(&mut self, owner: usize) {
        // dai_do_first_activities assesses danger (and its research feedback)
        // before moving units; do_last_activities clears these wants later.
        self.record_ai_first_activity_tech_wants(owner);
        self.refresh_ai_ferries();
        self.refresh_ai_guards();
        let boats: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.owner == owner && Self::is_ai_ferry(unit.kind))
            .map(|unit| unit.id)
            .collect();
        for boat in boats {
            self.refresh_ferry_boss(boat);
        }
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
        let ferry_dispatch = self.load_ai_merchant_ferries(owner);
        self.prune_settlement_reservations(owner);
        let settlement_ferries = self.dispatch_settlement_ferries(owner, &ferry_dispatch);
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
        let reserved = counterattacks
            .iter()
            .chain(&reinforcements)
            .chain(&drops)
            .chain(&fleet)
            .chain(&assault)
            .copied()
            .collect();
        self.refresh_ground_invasions(owner, &reserved);
        let mut claimed_targets = std::collections::HashSet::new();
        let mut ferry_managed = std::collections::HashSet::new();
        let unit_ids: Vec<u32> = self.player_units_ordered(owner).map(|u| u.id).collect();
        for id in unit_ids {
            let Some(unit) = self.units.iter().find(|u| u.id == id).cloned() else {
                continue;
            };
            if unit.kind == UnitKind::BarbarianLeader {
                self.run_barbarian_leader(id);
                continue;
            }
            if self.ai_guard_targets.contains_key(&id) && self.run_ai_guard(id) {
                continue;
            }
            if ferry_managed.contains(&id)
                || unit
                    .transport
                    .and_then(|boat| self.units.iter().find(|u| u.id == boat))
                    .is_some_and(|boat| boat.owner == owner)
            {
                // Native owner iteration delegates own passengers through
                // their transport. Foreign allied transports are different.
                continue;
            }
            if unit.kind.fuel_capacity() > 0 {
                self.run_ai_strike_aircraft(&unit);
                continue;
            }
            if assault.contains(&id)
                || fleet.contains(&id)
                || drops.contains(&id)
                || counterattacks.contains(&id)
                || reinforcements.contains(&id)
                || ferry_dispatch.values().any(|&passenger| passenger == id)
                || settlement_ferries.values().any(|plan| plan.founder == id)
            {
                continue;
            }
            // Native worker/founder dispatch precedes standalone ferry AI.
            if unit.kind.is_worker() || unit.kind.stats().can_found {
                continue;
            }
            if unit.kind.is_naval()
                && unit.kind != UnitKind::Submarine
                && unit.kind.transport_capacity() > 0
                && !self.hunter_targets.contains_key(&id)
            {
                let (handled, done) = self.run_source_air_ferry(&unit);
                ferry_managed.extend(done);
                if handled {
                    continue;
                }
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
            if owner != BARBARIANS && unit.kind.has_role(super::unit_catalog::UnitRole::Hunter) {
                if self.run_existing_hunter(&unit, &mut claimed_targets) {
                    continue;
                }
            }
            if unit.kind.is_military()
                && !unit.kind.is_aircraft()
                && !Self::is_ai_ferry(unit.kind)
                && self.run_ai_guard(id)
            {
                continue;
            }
            match unit.kind {
                // Founders and workers share the late-phase workermap.
                UnitKind::Settlers | UnitKind::Worker | UnitKind::Engineer | UnitKind::Migrants => {
                }
                UnitKind::Diplomat | UnitKind::Spy => {
                    self.ai_bribe_nearby(&unit);
                    let Some(agent) = self.units.iter().find(|agent| agent.id == id).cloned()
                    else {
                        continue;
                    };
                    if agent.moves_left <= 0 {
                        continue;
                    }
                    // Freeciv keeps a sole agent in an exposed city. Otherwise
                    // it resumes/acquires an offensive mission before defending
                    // another city (ai/default/daidiplomat.c:dai_manage_diplomat).
                    if self.holds_counterintelligence(&agent) {
                        self.agent_city_targets.remove(&id);
                    } else if !self.ai_city_espionage(&agent)
                        && !self.assign_counterintelligence(&agent)
                    {
                        self.wander_unit(id);
                    }
                }
                UnitKind::Explorer => {
                    self.run_auto_explorer(id);
                }
                UnitKind::BarbarianLeader => unreachable!("leader handled before cargo dispatch"),
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
                            self.unit_goto_tiles.insert(id, (city.x, city.y));
                            self.fortify_unit(id);
                            continue;
                        }
                    }
                    if unit.kind.is_military()
                        && !unit.kind.is_aircraft()
                        && !unit.kind.is_naval()
                        && owner != BARBARIANS
                    {
                        let mission = self
                            .ai_ground_targets
                            .get(&id)
                            .and_then(|target| self.cities.iter().find(|city| city.id == *target))
                            .filter(|city| self.at_war(owner, city.owner))
                            .and_then(|city| {
                                self.ai_goto_path(&unit, (city.x, city.y), GotoTask::Attack)
                            })
                            .map(|(path, _)| path);
                        if let Some(path) = mission {
                            self.follow_ground_invasion_path(id, path);
                        } else {
                            self.unit_goto_tiles.remove(&id);
                            self.run_auto_explorer(id);
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
                        self.unit_goto_tiles.insert(id, (target.x, target.y));
                        self.ai_goto_unit(id, (target.x, target.y), GotoTask::Attack);
                    } else if owner == BARBARIANS {
                        self.wander_unit(id);
                    } else {
                        self.unit_goto_tiles.remove(&id);
                        self.run_auto_explorer(id);
                    }
                }
            }
        }
        self.run_ai_worker_phase(owner);
        let alive: std::collections::HashSet<_> = self.units.iter().map(|u| u.id).collect();
        self.ai_ground_targets.retain(|id, _| alive.contains(id));
    }
}

#[cfg(test)]
mod hunter_tests {
    use super::*;

    #[test]
    fn native_hunter_tactical_fixture_uses_land_pf_order_gameloss_and_base_type_speed() {
        let sim = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            sim.native_hunter_tactics_playtest(),
            (true, true, true, true)
        );
    }

    #[test]
    fn hunter_assigns_same_tile_missiles_only_when_a_real_hunt_begins() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (hunter, missile, target) = sim.ai_submarine_playtest_setup();
        sim.cities.clear();
        sim.units
            .iter_mut()
            .find(|u| u.id == missile)
            .unwrap()
            .transport = None;
        sim.record_transport_change(missile, None);
        sim.units
            .iter_mut()
            .find(|u| u.id == missile)
            .unwrap()
            .moves_left = 0;
        let actor = sim.units.iter().find(|u| u.id == hunter).unwrap().clone();
        let mut no_target = sim.clone();
        no_target.remove_units_with_cargo(vec![target]);
        assert_eq!(no_target.manage_hunter(&actor, &mut Default::default()), 0);
        assert_eq!(
            no_target
                .units
                .iter()
                .find(|u| u.id == missile)
                .unwrap()
                .transport,
            None
        );
        assert_ne!(sim.manage_hunter(&actor, &mut Default::default()), 0);
        assert_eq!(
            sim.units
                .iter()
                .find(|u| u.id == missile)
                .unwrap()
                .transport,
            Some(hunter)
        );
    }

    #[test]
    fn land_hunter_target_and_interception_history_survive_actual_end_turn() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.units.clear();
        sim.cities.clear();
        sim.hunter_targets.clear();
        sim.previous_naval_positions.clear();
        for player in &mut sim.players {
            player.ai = false;
        }
        sim.players[1].ai = true;
        sim.sign_pact(1, HUMAN, Pact::War);
        for tile in &mut sim.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.hut = false;
            tile.base = None;
        }
        let start = (20, 30);
        let far = offset_native_by_map(start.0, start.1, 10, 0);
        let hunter = sim.spawn_unit(1, UnitKind::Horseman, start.0, start.1);
        let target = sim.spawn_unit(HUMAN, UnitKind::Leader, far.0, far.1);
        sim.hunter_targets.insert(hunter, target);
        assert!(sim.apply(Command::EndTurn));
        assert_eq!(sim.hunter_targets.get(&hunter), Some(&target));
        let actor = sim.units.iter().find(|u| u.id == hunter).unwrap();
        assert_ne!(
            (actor.x, actor.y),
            start,
            "actual land hunter phase advances toward its distant victim"
        );
        assert_eq!(
            sim.previous_naval_positions.get(&hunter),
            Some(&(actor.x, actor.y))
        );
        assert_eq!(sim.previous_naval_positions.get(&target), Some(&far));
    }

    #[test]
    fn hunter_stack_uses_native_prepend_order_and_keeps_raw_original_threat() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (hunter_id, _, carrier) = sim.ai_submarine_playtest_setup();
        let hunter = sim
            .units
            .iter()
            .find(|u| u.id == hunter_id)
            .unwrap()
            .clone();
        let target = sim.units.iter().find(|u| u.id == carrier).unwrap().clone();
        let second = sim.spawn_unit(HUMAN, UnitKind::Carrier, target.x, target.y);
        sim.units.iter_mut().find(|u| u.id == second).unwrap().hp = target.hp;
        assert_eq!(
            sim.hunter_target(&hunter, &Default::default()).map(|v| v.0),
            Some(second)
        );
        // Original is compared without distance discount. The first unit in
        // the same stack therefore cannot displace the previous target.
        sim.hunter_targets.insert(hunter_id, carrier);
        assert_eq!(
            sim.hunter_target(&hunter, &Default::default()).map(|v| v.0),
            Some(carrier)
        );
        sim.sign_pact(1, HUMAN, Pact::Peace);
        assert_eq!(sim.hunter_target(&hunter, &Default::default()), None);
    }

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
        let faster = game.units.last().unwrap().clone();
        game.previous_naval_positions
            .insert(destroyer, (faster.x, faster.y));
        assert_eq!(game.hunt_value(&sub, &faster), None);
        let mut damaged = faster.clone();
        damaged.hp = 1;
        assert!(
            game.hunt_value(&sub, &damaged).is_none(),
            "source chase eligibility compares type speed, even when damage slows actual movement"
        );
        let farther = offset_native_by_map(sub.x, sub.y, 2, 0);
        game.previous_naval_positions.insert(destroyer, farther);
        assert!(game.hunt_value(&sub, &faster).is_some());
        assert_eq!(
            game.hunter_target(&sub, &[carrier, destroyer].into_iter().collect()),
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
            game.hunter_target(&sub, &Default::default())
                .map(|target| target.0),
            Some(carrier)
        );
        game.remove_units_with_cargo(vec![carrier]);
        assert!(game.units.iter().any(|unit| unit.id == transport));
        assert_eq!(game.hunter_target(&sub, &Default::default()), None);
    }

    #[test]
    fn normal_targets_handicap_suppresses_hunter_and_ammunition_production() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.normal_hunter_production_playtest(), (true, true, true));
    }

    #[test]
    fn distant_ground_missions_coordinate_air_raids_before_infantry_moves() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (infantry, bomber, city) = game.ai_ground_invasion_playtest_setup();
        assert_eq!(game.ai_ground_targets.get(&infantry), Some(&city));
        let target = game.cities.iter().find(|c| c.id == city).unwrap();
        let unit = game.units.iter().find(|u| u.id == infantry).unwrap();
        assert!(game.map_distance(unit.x, unit.y, target.x, target.y) > unit.kind.stats().moves);
        assert!(game.invasion_support(1, target, bomber).1 > 0);
        game.run_ai_player(1);
        assert!(game.units.iter().find(|u| u.id == bomber).unwrap().moved);
        assert_eq!(game.ai_ground_targets.get(&infantry), Some(&city));
    }

    #[test]
    fn local_occupation_marks_only_undefended_cities_and_obeys_actual_movement_radius() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (infantry, _, city) = game.ai_ground_invasion_playtest_setup();
        game.ai_ground_targets.clear();
        let target = game.cities.iter().find(|c| c.id == city).unwrap().clone();
        let adjacent = offset_native_by_map(target.x, target.y, -1, 0);
        let unit = game.units.iter_mut().find(|u| u.id == infantry).unwrap();
        (unit.x, unit.y) = adjacent;
        assert_eq!(game.invasion_support(1, &target, 0).1, 0);
        game.units.retain(|u| u.owner != HUMAN);
        assert!(game.invasion_support(1, &target, 0).1 > 0);
        let outside = offset_native_by_map(target.x, target.y, -2, 0);
        let unit = game.units.iter_mut().find(|u| u.id == infantry).unwrap();
        (unit.x, unit.y) = outside;
        assert_eq!(game.invasion_support(1, &target, 0).1, 0);
    }

    #[test]
    fn peace_and_defensive_reservations_cancel_ground_missions() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (infantry, _, city) = game.ai_ground_invasion_playtest_setup();
        game.refresh_ground_invasions(1, &[infantry].into_iter().collect());
        assert!(!game.ai_ground_targets.contains_key(&infantry));
        game.ai_ground_targets.insert(infantry, city);
        game.sign_pact(1, HUMAN, Pact::Peace);
        game.refresh_ground_invasions(1, &Default::default());
        assert!(!game.ai_ground_targets.contains_key(&infantry));
        let target = game.cities.iter().find(|c| c.id == city).unwrap();
        assert_eq!(game.invasion_support(1, target, 0), (0, 0));
    }
    #[test]
    fn defended_overseas_force_keeps_its_invasion_until_capture() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let result = game.overseas_assault_playtest(true);
        assert_eq!(
            result,
            (true, true, true),
            "plans{:?} units{:?} cities{:?}",
            game.ai_ground_targets,
            game.units
                .iter()
                .map(|u| (u.id, u.owner, u.kind, u.x, u.y, u.hp, u.transport))
                .collect::<Vec<_>>(),
            game.cities
                .iter()
                .map(|c| (c.id, c.owner, c.x, c.y))
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn distant_attacks_forecast_buildable_upgraded_defenders_and_training() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_defender_forecast_playtest());
    }
    #[test]
    fn military_pickup_preserves_a_civilian_boss_until_released() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.hut = false;
            tile.resource = None;
        }
        let home = (10, 24);
        let target = offset_native_by_map(home.0, home.1, 9, 0);
        let shore = offset_native_by_map(home.0, home.1, 8, 0);
        for (x, y) in [home, shore, target] {
            game.tile_mut(x, y).terrain = Terrain::Grassland;
            game.tile_mut(x, y).known_by[1] = true;
        }
        game.create_city(1, home.0, home.1);
        game.create_city(HUMAN, target.0, target.1);
        game.spawn_unit(1, UnitKind::Marines, home.0, home.1);
        let soldier = game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let merchant = game.spawn_unit(1, UnitKind::Caravan, home.0, home.1);
        let sea = offset_native_by_map(home.0, home.1, -1, 0);
        let boat = game.spawn_unit(1, UnitKind::Transport, sea.0, sea.1);
        game.refresh_ai_ferries();
        assert!(game.assign_ai_ferry(boat, merchant));
        assert!(
            game.dispatch_assault_transports(1, |_| false, |_| false)
                .is_empty()
        );
        assert!(
            game.units
                .iter()
                .find(|u| u.id == soldier)
                .unwrap()
                .transport
                .is_none()
        );
        assert_eq!(
            game.ai_ferries.boats[&boat].state,
            ferries::FerryState::Assigned(merchant)
        );
        game.release_ai_ferry(merchant);
        assert!(
            game.dispatch_assault_transports(1, |_| false, |_| false)
                .contains(&boat)
        );
        assert_eq!(
            game.ai_ferries.boats[&boat].state,
            ferries::FerryState::Assigned(soldier)
        );
    }

    #[test]
    fn worker_transport_dispatch_keeps_its_passenger_and_booking_without_ferry_management() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.pollution = false;
            tile.fallout = false;
            tile.hut = false;
        }
        let position = (10, 24);
        let boat = game.spawn_unit(1, UnitKind::Transport, position.0, position.1);
        let passenger = game.spawn_unit(1, UnitKind::Marines, position.0, position.1);
        game.units
            .iter_mut()
            .find(|u| u.id == passenger)
            .unwrap()
            .transport = Some(boat);
        game.refresh_ai_ferries();
        assert!(game.assign_ai_ferry(boat, passenger));
        game.run_ai_player(1);
        let ship = game.units.iter().find(|u| u.id == boat).unwrap();
        assert_eq!((ship.x, ship.y), position);
        assert!(
            game.units
                .iter()
                .any(|u| u.id == passenger && u.transport == Some(boat))
        );
        assert_eq!(
            game.ai_ferries.boats[&boat].state,
            ferries::FerryState::Assigned(passenger)
        );
    }

    #[test]
    fn military_delivery_releases_the_persisted_ferry_boss() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.overseas_assault_playtest(false), (true, true, true));
        assert!(!game.ai_ferries.boats.is_empty());
        assert!(
            game.ai_ferries
                .boats
                .values()
                .all(|boat| boat.state == ferries::FerryState::Available)
        );
    }
    #[test]
    fn road_reinforcement_prioritizes_grave_front_and_keeps_a_home_guard() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_road_reinforcement_playtest());
    }

    #[test]
    fn grave_home_danger_keeps_its_strong_defender_even_with_a_threatened_neighbor() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = true;
            tile.railroad = false;
            tile.river = false;
        }
        let home = (10, 24);
        let front = offset_native_by_map(home.0, home.1, 3, 0);
        game.create_city(1, home.0, home.1);
        game.create_city(1, front.0, front.1);
        let marine = game.spawn_unit(1, UnitKind::Marines, home.0, home.1);
        game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        game.spawn_unit(1, UnitKind::Warrior, front.0, front.1);
        let near_home = offset_native_by_map(home.0, home.1, -1, 0);
        let near_front = offset_native_by_map(front.0, front.1, 1, 0);
        game.spawn_unit(HUMAN, UnitKind::Cannon, near_home.0, near_home.1);
        game.spawn_unit(HUMAN, UnitKind::Cannon, near_front.0, near_front.1);
        assert!(game.city_grave_danger(&game.cities[0]) > 0);
        assert!(!game.reinforce_ai_cities(1).contains(&marine));
        assert!(
            game.units
                .iter()
                .any(|u| u.id == marine && (u.x, u.y) == home)
        );
    }

    #[test]
    fn amphibious_waypoint_keeps_the_passengers_final_land_goal() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.owner = None;
            tile.hut = false;
            tile.known_by[1] = true;
        }
        let start = (10, 24);
        let waypoint = offset_native_by_map(start.0, start.1, 5, 0);
        let land = offset_native_by_map(waypoint.0, waypoint.1, 1, 0);
        game.tile_mut(land.0, land.1).terrain = Terrain::Grassland;
        let boat = game.spawn_unit(1, UnitKind::Transport, start.0, start.1);
        game.units.last_mut().unwrap().moves_left = MOVE_FRAGMENTS;
        game.ai_ferry_goto_waypoint(boat, waypoint, land);
        let live = game.units.iter().find(|u| u.id == boat).unwrap();
        assert_ne!((live.x, live.y), start);
        assert_ne!((live.x, live.y), waypoint);
        assert_eq!(game.unit_goto_tiles.get(&boat), Some(&land));
        // Ordinary ferry pickup fills its parameter with the pickup itself.
        game.ai_goto_unit(boat, start, GotoTask::None);
        assert_eq!(game.unit_goto_tiles.get(&boat), Some(&start));
    }

    #[test]
    fn damaged_military_ferry_waits_in_city_until_recovered() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
        }
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        game.create_city(1, 10, 24);
        let boat = game.spawn_unit(1, UnitKind::Transport, 10, 24);
        game.units.last_mut().unwrap().hp = 1;
        assert!(!game.military_ferry_ready(boat));
        let unit = game.units.last_mut().unwrap();
        unit.hp = unit.kind.stats().hp;
        assert!(game.military_ferry_ready(boat));
    }
}
