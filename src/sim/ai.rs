use super::*;

impl Simulation {
    pub(super) fn best_city_site(&self, owner: usize) -> Option<(i32, i32)> {
        let founder = self
            .units
            .iter()
            .find(|u| u.owner == owner && u.kind == UnitKind::Settlers)?;
        let mut best = None;
        let mut best_score = i32::MIN;
        for index in 0..self.tiles.len() {
            let (x, y) = Self::xy(index);
            if !self.can_found_at(x, y) {
                continue;
            }
            let distance = self.map_distance(founder.x, founder.y, x, y);
            let (f, s, t) = self.tile(x, y).yields();
            let score = f * 8 + s * 5 + t * 3 - distance;
            if score > best_score {
                best_score = score;
                best = Some((x, y));
            }
        }
        best
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

        // Sail toward a reachable coast next to a rival city, then unload
        // onto a free landing tile using normal movement.
        let landing = self
            .cities
            .iter()
            .filter(|city| city.owner != boat.owner)
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
                if (x, y) == (boat.x, boat.y) {
                    return Some((0, x, y, direction));
                }
                let path = self.path_to(boat.id, x, y);
                (!path.is_empty()).then_some((path.len(), x, y, direction))
            })
            .min_by_key(|candidate| candidate.0);

        let Some((distance, x, y, (dx, dy))) = landing else {
            self.wander_unit(boat.id);
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
        let mut best = None;
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
                if best.is_none_or(|(current, _)| score < current) {
                    best = Some((score, rates));
                }
            }
        }
        if let Some((_, rates)) = best {
            self.players[owner].rates = rates;
        }
    }

    pub(super) fn run_ai_player(&mut self, owner: usize) {
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
            if unit.transport.is_some() {
                continue;
            }
            if unit.kind.transport_capacity() > 0 {
                self.run_raider_transport(&unit);
                continue;
            }
            match unit.kind {
                UnitKind::Settlers => {
                    if self.can_found_at(unit.x, unit.y)
                        && (self.cities.iter().all(|c| c.owner != owner)
                            || self
                                .cities
                                .iter()
                                .filter(|c| c.owner == owner)
                                .all(|c| self.map_distance(unit.x, unit.y, c.x, c.y) >= 4))
                    {
                        self.found_city(id);
                    } else if let Some((x, y)) = self.best_city_site(owner) {
                        self.goto_unit(id, x, y);
                        if self
                            .units
                            .iter()
                            .find(|u| u.id == id)
                            .is_some_and(|u| self.can_found_at(u.x, u.y))
                        {
                            self.found_city(id);
                        }
                    }
                }
                UnitKind::Worker => self.run_automated_unit(id),
                UnitKind::Explorer | UnitKind::Diplomat => self.wander_unit(id),
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
                        if defenders <= 1 {
                            self.fortify_unit(id);
                            continue;
                        }
                    }
                    if let Some(target) = self
                        .cities
                        .iter()
                        .filter(|c| c.owner != owner)
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
