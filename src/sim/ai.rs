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
