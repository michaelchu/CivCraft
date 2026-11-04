use super::*;

impl Simulation {
    pub(super) fn movement_cost_for(&self, unit: &Unit, nx: i32, ny: i32) -> Option<i32> {
        let dest = self.tile(nx, ny);
        if dest.terrain.is_water() {
            return None;
        }
        let source = self.tile(unit.x, unit.y);
        if source.road && dest.road {
            Some(2)
        } else if unit.kind.stats().ignores_terrain {
            Some(2)
        } else {
            Some(dest.terrain.movement_cost() * MOVE_FRAGMENTS)
        }
    }

    pub(crate) fn attack_power(unit: &Unit) -> i32 {
        let stats = unit.kind.stats();
        let mut power = stats.attack * POWER_FACTOR;
        if unit.moves_left < MOVE_FRAGMENTS {
            power = power * unit.moves_left.max(0) / MOVE_FRAGMENTS;
        }
        power
    }

    pub(crate) fn defense_power(&self, unit: &Unit, x: i32, y: i32) -> i32 {
        // Freeciv applies these as separate integer-multiplication stages:
        // terrain in get_defense_power, then Defend_Bonus, then
        // Fortify_Defense_Bonus in defense_multiplication.
        let mut power = unit.kind.stats().defense * POWER_FACTOR;
        power = apply_percent_bonus(power, self.tile(x, y).terrain.defense_bonus());

        let city = self.cities.iter().find(|c| c.x == x && c.y == y);
        let mut defend_bonus = 0;
        if let Some(city) = city {
            defend_bonus += CITY_DEFEND_BONUS;
            if city.size >= LARGE_CITY_SIZE {
                defend_bonus += LARGE_CITY_DEFEND_BONUS;
            }
        }
        power = apply_percent_bonus(power, defend_bonus);

        // Civ2Civ3 gives CanFortify units without the Cant_Fortify type flag
        // the same 50% city stage even while they are not explicitly fortified.
        let fortify_bonus =
            if unit.state == UnitState::Fortified || (city.is_some() && unit.kind.can_fortify()) {
                FORTIFY_DEFENSE_BONUS
            } else {
                0
            };
        apply_percent_bonus(power, fortify_bonus)
    }

    pub(super) fn defender_rank(
        &self,
        attacker: &Unit,
        defender: &Unit,
        x: i32,
        y: i32,
    ) -> (i32, i32, i32) {
        let attack_power = Self::attack_power(attacker);
        let defense_power = self.defense_power(defender, x, y);
        let attacker_stats = attacker.kind.stats();
        let defender_stats = defender.kind.stats();
        let attacker_win = combat_win_chance(
            attack_power,
            attacker.hp,
            attacker_stats.firepower,
            defense_power,
            defender.hp,
            defender_stats.firepower,
        );

        // get_defender truncates the defender's win chance to five decimal
        // places, then prefers the cheaper unit and finally the higher
        // defense rating when those values tie.
        let defender_value = (100_000.0 * (1.0 - attacker_win)) as i32;
        let rounds_survived =
            (defender.hp + attacker_stats.firepower - 1) / attacker_stats.firepower;
        let defense_rating = defense_power * rounds_survived * defender_stats.firepower;
        (defender_value, -defender_stats.cost, defense_rating)
    }

    pub(super) fn best_defender_id(&self, attacker: &Unit, x: i32, y: i32) -> Option<u32> {
        let mut best = None;
        let mut best_rank = None;
        for defender in self
            .units
            .iter()
            .filter(|unit| unit.x == x && unit.y == y && unit.owner != attacker.owner)
        {
            let rank = self.defender_rank(attacker, defender, x, y);
            if best_rank.is_none() || best_rank.is_some_and(|current| rank > current) {
                best = Some(defender.id);
                best_rank = Some(rank);
            }
        }
        best
    }

    pub(super) fn try_move_unit(&mut self, unit_id: u32, dx: i32, dy: i32) -> MoveResult {
        let Some(index) = self.units.iter().position(|u| u.id == unit_id) else {
            return MoveResult::Blocked;
        };
        if !is_isohex_step(dx, dy) {
            return MoveResult::Blocked;
        }
        let unit = self.units[index].clone();
        if unit.moves_left <= 0 || unit.work_remaining > 0 || unit.state == UnitState::Sentry {
            return MoveResult::Blocked;
        }
        let (nx, ny) = offset_native_by_map(unit.x, unit.y, dx, dy);
        let Some(cost) = self.movement_cost_for(&unit, nx, ny) else {
            return MoveResult::Blocked;
        };
        let defender = self.best_defender_id(&unit, nx, ny);

        if let Some(defender_id) = defender {
            if unit.kind.stats().attack <= 0 {
                return MoveResult::Blocked;
            }
            self.resolve_combat(unit_id, defender_id, nx, ny);
            return MoveResult::Fought;
        }

        if let Some(city) = self.cities.iter().find(|c| c.x == nx && c.y == ny) {
            if city.owner != unit.owner && !unit.kind.is_military() {
                return MoveResult::Blocked;
            }
        }

        if let Some(mover) = self.units.iter_mut().find(|u| u.id == unit_id) {
            mover.x = nx;
            mover.y = ny;
            mover.moves_left = (mover.moves_left - cost).max(0);
            mover.moved = true;
            mover.state = UnitState::Idle;
        }
        self.capture_city_if_needed(unit_id, nx, ny);
        if unit.owner == HUMAN {
            self.refresh_visibility();
        }
        MoveResult::Moved
    }

    pub(super) fn resolve_combat(&mut self, attacker_id: u32, defender_id: u32, nx: i32, ny: i32) {
        let Some(attacker_index) = self.units.iter().position(|u| u.id == attacker_id) else {
            return;
        };
        let Some(defender_index) = self.units.iter().position(|u| u.id == defender_id) else {
            return;
        };
        let attacker = self.units[attacker_index].clone();
        let defender = self.units[defender_index].clone();
        let astats = attacker.kind.stats();
        let dstats = defender.kind.stats();

        let attack_power = Self::attack_power(&attacker);
        let defense_power = self.defense_power(&defender, nx, ny);
        let mut att_hp = attacker.hp;
        let mut def_hp = defender.hp;
        if attack_power <= 0 {
            att_hp = 0;
        } else if defense_power <= 0 {
            def_hp = 0;
        }
        let mut rounds = 0;
        while att_hp > 0 && def_hp > 0 {
            if self.rand_below(attack_power + defense_power) >= defense_power {
                def_hp -= astats.firepower;
            } else {
                att_hp -= dstats.firepower;
            }
            rounds += 1;
        }
        att_hp = att_hp.max(0);
        def_hp = def_hp.max(0);

        if let Some(a) = self.units.iter_mut().find(|u| u.id == attacker_id) {
            a.hp = att_hp;
            a.moves_left = (a.moves_left - MOVE_FRAGMENTS).max(0);
            a.moved = true;
        }
        if let Some(d) = self.units.iter_mut().find(|u| u.id == defender_id) {
            d.hp = def_hp;
        }

        let attacker_name = attacker.kind.name();
        let defender_name = defender.kind.name();
        let attacker_owner = attacker.owner;
        let defender_owner = defender.owner;
        if def_hp <= 0 {
            self.units.retain(|u| u.id != defender_id);
            if att_hp > 0 {
                if let Some(a) = self.units.iter_mut().find(|u| u.id == attacker_id) {
                    a.x = nx;
                    a.y = ny;
                }
                self.capture_city_if_needed(attacker_id, nx, ny);
            }
            if attacker_owner == HUMAN || defender_owner == HUMAN {
                self.add_event(format!(
                    "{attacker_name} defeated {defender_name} after {rounds} combat rounds."
                ));
            }
        } else if att_hp <= 0 {
            self.units.retain(|u| u.id != attacker_id);
            if attacker_owner == HUMAN || defender_owner == HUMAN {
                self.add_event(format!(
                    "{defender_name} held the tile against {attacker_name}."
                ));
            }
        }
        if attacker_owner == HUMAN || defender_owner == HUMAN {
            self.refresh_visibility();
        }
        self.check_game_over();
    }

    pub(super) fn path_to(&self, unit_id: u32, tx: i32, ty: i32) -> Vec<(i32, i32)> {
        let Some(unit) = self.units.iter().find(|u| u.id == unit_id) else {
            return Vec::new();
        };
        let start = Self::idx(unit.x, unit.y);
        let goal = Self::idx(tx, ty);
        if start == goal {
            return Vec::new();
        }
        let mut queue = VecDeque::new();
        let mut previous = vec![None::<(usize, (i32, i32))>; (MAP_W * MAP_H) as usize];
        previous[start] = Some((start, (0, 0)));
        queue.push_back(start);
        while let Some(current) = queue.pop_front() {
            if current == goal {
                break;
            }
            let (cx, cy) = Self::xy(current);
            for (dx, dy) in ISOHEX_DIRS {
                let (nx, ny) = offset_native_by_map(cx, cy, dx, dy);
                let ni = Self::idx(nx, ny);
                if previous[ni].is_some() || self.tile(nx, ny).terrain.is_water() {
                    continue;
                }
                let enemy_block = self
                    .units
                    .iter()
                    .any(|u| u.x == nx && u.y == ny && u.owner != unit.owner);
                if enemy_block && ni != goal {
                    continue;
                }
                previous[ni] = Some((current, (dx, dy)));
                queue.push_back(ni);
            }
        }
        if previous[goal].is_none() {
            return Vec::new();
        }
        let mut path = Vec::new();
        let mut cursor = goal;
        while cursor != start {
            let Some((prior, direction)) = previous[cursor] else {
                return Vec::new();
            };
            path.push(direction);
            cursor = prior;
        }
        path.reverse();
        path
    }

    pub(super) fn goto_unit(&mut self, unit_id: u32, tx: i32, ty: i32) -> bool {
        let path = self.path_to(unit_id, tx, ty);
        if path.is_empty() {
            return false;
        }
        let mut moved = false;
        for (dx, dy) in path {
            let result = self.try_move_unit(unit_id, dx, dy);
            if result == MoveResult::Blocked {
                break;
            }
            moved = true;
            let Some(unit) = self.units.iter().find(|u| u.id == unit_id) else {
                break;
            };
            if unit.moves_left <= 0 || result == MoveResult::Fought {
                break;
            }
        }
        moved
    }

    pub(super) fn begin_work(&mut self, unit_id: u32, kind: WorkKind) -> bool {
        let Some(index) = self.units.iter().position(|u| u.id == unit_id) else {
            return false;
        };
        let unit = self.units[index].clone();
        if !unit.kind.stats().can_work || unit.work_remaining > 0 {
            return false;
        }
        let tile = self.tile(unit.x, unit.y);
        let turns = match kind {
            WorkKind::Road if !tile.road => tile.terrain.road_time(),
            WorkKind::Irrigation if self.can_irrigate_at(unit.x, unit.y) => {
                tile.terrain.irrigation_time()
            }
            WorkKind::Mine if !tile.mine => tile.terrain.mine_time(),
            _ => 0,
        };
        if turns <= 0 {
            if unit.owner == HUMAN {
                self.add_event(format!(
                    "{} cannot build {} on {}.",
                    unit.kind.name(),
                    kind.name(),
                    tile.terrain.name()
                ));
            }
            return false;
        }
        let worker = &mut self.units[index];
        worker.work_kind = Some(kind);
        worker.work_remaining = turns;
        worker.moves_left = 0;
        worker.state = UnitState::Idle;
        if unit.owner == HUMAN {
            self.add_event(format!(
                "{} began {} ({turns} turns).",
                unit.kind.name(),
                kind.name()
            ));
        }
        true
    }

    pub(super) fn fortify_unit(&mut self, unit_id: u32) -> bool {
        let (owner, name) = {
            let Some(unit) = self.units.iter_mut().find(|u| u.id == unit_id) else {
                return false;
            };
            if !unit.kind.can_fortify() {
                return false;
            }
            unit.state = UnitState::Fortifying;
            unit.moves_left = 0;
            (unit.owner, unit.kind.name())
        };
        if owner == HUMAN {
            self.add_event(format!("{name} will be fortified next turn."));
        }
        true
    }

    pub(super) fn update_activities_and_restore(&mut self) {
        let mut completed = Vec::new();
        let city_centers: Vec<(i32, i32)> =
            self.cities.iter().map(|city| (city.x, city.y)).collect();
        for unit in &mut self.units {
            if unit.work_remaining > 0 {
                unit.work_remaining -= 1;
                if unit.work_remaining == 0 {
                    if let Some(kind) = unit.work_kind.take() {
                        completed.push((unit.owner, unit.x, unit.y, kind));
                    }
                }
            }
            if unit.state == UnitState::Fortifying {
                unit.state = UnitState::Fortified;
            }
            let in_city_center = city_centers.contains(&(unit.x, unit.y));
            unit.hp += Self::hitpoint_gain(unit, in_city_center);
            unit.moves_left = unit.kind.stats().moves * MOVE_FRAGMENTS;
            unit.moved = false;
        }
        for (owner, x, y, kind) in completed {
            let tile = self.tile_mut(x, y);
            match kind {
                WorkKind::Road => tile.road = true,
                WorkKind::Irrigation => {
                    tile.irrigation = true;
                    tile.mine = false;
                }
                WorkKind::Mine => {
                    tile.mine = true;
                    tile.irrigation = false;
                }
            }
            if owner == HUMAN {
                self.add_event(format!("Your workers completed {}.", kind.name()));
            }
        }
    }
}
