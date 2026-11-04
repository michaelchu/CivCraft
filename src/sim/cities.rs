use super::*;

impl Simulation {
    pub(super) fn claim_city_tiles(&mut self, city_x: i32, city_y: i32, owner: usize) {
        for dy in -2..=2 {
            for dx in -2..=2 {
                let distance = isohex_vector_distance(dx, dy);
                if distance * distance <= 5 {
                    let (claim_x, claim_y) = offset_native_by_map(city_x, city_y, dx, dy);
                    self.tile_mut(claim_x, claim_y).owner = Some(owner);
                }
            }
        }
    }

    pub(super) fn rebuild_city_ownership(&mut self) {
        for tile in &mut self.tiles {
            tile.owner = None;
        }
        let cities: Vec<(i32, i32, usize)> = self
            .cities
            .iter()
            .map(|city| (city.x, city.y, city.owner))
            .collect();
        for (x, y, owner) in cities {
            self.claim_city_tiles(x, y, owner);
        }
    }

    pub(crate) fn can_found_at(&self, x: i32, y: i32) -> bool {
        if !self.tile(x, y).terrain.allows_city() {
            return false;
        }
        self.cities
            .iter()
            .all(|c| self.map_distance(x, y, c.x, c.y) >= CITY_MIN_DISTANCE)
    }

    pub(crate) fn can_irrigate_at(&self, x: i32, y: i32) -> bool {
        let tile = self.tile(x, y);
        if tile.irrigation || tile.terrain.irrigation_time() == 0 {
            return false;
        }
        ISOHEX_DIRS.iter().any(|&(dx, dy)| {
            let (source_x, source_y) = offset_native_by_map(x, y, dx, dy);
            let source = self.tile(source_x, source_y);
            source.terrain.is_water() || source.irrigation
        })
    }

    pub(super) fn city_name(&self, owner: usize) -> String {
        const ROOTS: [[&str; 4]; PLAYER_COUNT] = [
            ["Aster", "Briar", "Cinder", "Dawn"],
            ["Gilden", "Hearth", "Ilex", "Jasper"],
            ["Lumen", "Mere", "Northwind", "Opal"],
            ["Pine", "Quartz", "Rill", "Solace"],
            ["Tern", "Umber", "Vale", "Willow"],
            ["Yarrow", "Zephyr", "Alder", "Brook"],
        ];
        let count = self.cities.iter().filter(|c| c.owner == owner).count();
        if count < 4 {
            ROOTS[owner][count].to_owned()
        } else {
            format!("{} {}", ROOTS[owner][count % 4], count + 1)
        }
    }

    pub(super) fn found_city(&mut self, unit_id: u32) -> bool {
        let Some(index) = self.units.iter().position(|u| u.id == unit_id) else {
            return false;
        };
        let unit = self.units[index].clone();
        if !unit.kind.stats().can_found || !self.can_found_at(unit.x, unit.y) {
            if unit.owner == HUMAN {
                self.add_event("A city must be on land and at least 3 tiles from another city.");
            }
            return false;
        }
        let name = self.city_name(unit.owner);
        let city_id = self.next_city_id;
        self.next_city_id += 1;
        self.units.remove(index);
        self.cities.push(City {
            id: city_id,
            owner: unit.owner,
            x: unit.x,
            y: unit.y,
            name: name.clone(),
            size: 1,
            food: 0,
            shields: 0,
            production: UnitKind::Warrior,
        });
        let tile = self.tile_mut(unit.x, unit.y);
        tile.road = true;
        tile.owner = Some(unit.owner);
        self.claim_city_tiles(unit.x, unit.y, unit.owner);
        if unit.owner == HUMAN {
            self.add_event(format!(
                "{name} founded. Its people begin training Warriors."
            ));
        }
        self.refresh_visibility();
        true
    }

    pub(super) fn capture_city_if_needed(&mut self, unit_id: u32, x: i32, y: i32) {
        let Some(unit) = self.units.iter().find(|u| u.id == unit_id).cloned() else {
            return;
        };
        if !unit.kind.is_military() {
            return;
        }
        let Some(city_index) = self
            .cities
            .iter()
            .position(|c| c.x == x && c.y == y && c.owner != unit.owner)
        else {
            return;
        };
        let old_owner = self.cities[city_index].owner;
        let city_name = self.cities[city_index].name.clone();
        self.cities[city_index].owner = unit.owner;
        self.cities[city_index].size = (self.cities[city_index].size - 1).max(1);
        self.cities[city_index].food = 0;
        self.cities[city_index].shields = 0;
        self.cities[city_index].production = UnitKind::Warrior;
        self.claim_city_tiles(x, y, unit.owner);
        if unit.owner == HUMAN || old_owner == HUMAN {
            self.add_event(format!(
                "{} captured {city_name} from {}.",
                self.players[unit.owner].name, self.players[old_owner].name
            ));
        }
    }

    pub(crate) fn city_yields(&self, city: &City) -> (i32, i32, i32) {
        let center = self.tile(city.x, city.y);
        let (mut cf, cs, ct) = center.yields();
        if !center.irrigation && !center.mine {
            // City centers receive the terrain's irrigation bonus
            // automatically, but an explicit irrigation does not stack.
            cf += center.terrain.irrigation_bonus();
        }
        let mut food = cf.max(1);
        let mut shields = cs.max(1);
        let mut trade = ct.max(1);
        let mut candidates = Vec::new();
        for dy in -2..=2 {
            for dx in -2..=2 {
                let distance = isohex_vector_distance(dx, dy);
                if (dx == 0 && dy == 0) || distance * distance > 5 {
                    continue;
                }
                let (tile_x, tile_y) = offset_native_by_map(city.x, city.y, dx, dy);
                let tile = self.tile(tile_x, tile_y);
                let yields = tile.yields();
                let score = yields.0 * 4 + yields.1 * 3 + yields.2 * 2;
                candidates.push((score, yields));
            }
        }
        candidates.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, (f, s, t)) in candidates.into_iter().take(city.size as usize) {
            food += f;
            shields += s;
            trade += t;
        }
        (food, shields, trade)
    }

    pub(crate) fn growth_box(size: i32) -> i32 {
        if size <= 0 {
            return 0;
        }
        let initial_count = GRANARY_FOOD_INITIAL.len() as i32;
        let base = if size <= initial_count {
            GRANARY_FOOD_INITIAL[(size - 1) as usize]
        } else {
            GRANARY_FOOD_INITIAL[GRANARY_FOOD_INITIAL.len() - 1]
                + GRANARY_FOOD_INCREMENT * (size - initial_count)
        };
        percent_of(base, FOODBOX_PERCENT).max(1)
    }

    pub(super) fn hitpoint_gain(unit: &Unit, in_city_center: bool) -> i32 {
        if unit.moved {
            return 0;
        }
        let base = unit.kind.stats().hp;
        let minimum_percent = if in_city_center {
            CITY_MIN_HP_PERCENT
        } else {
            0
        };
        // hp_gain_coord first takes the greater of HP_Regen and Min_HP_Pct,
        // then adds HP_Regen_2. Opening ground units have no HP_Regen
        // effect, a universal 10 HP_Regen_2, and another 10 when fortified.
        let hp_regen = 0;
        let minimum = percent_of(base, minimum_percent);
        let mut regeneration_2 = BASE_HP_REGEN_2;
        if unit.state == UnitState::Fortified {
            regeneration_2 += FORTIFIED_HP_REGEN_2;
        }
        // hp_gain_coord at 0x1003fee50 divides the integer HP value by 10
        // before applying HP_Regen_2, then truncates again after the bonus.
        let regeneration_after_minimum = (base / 10) * regeneration_2 / 10;
        (hp_regen.max(minimum) + regeneration_after_minimum).clamp(0, base - unit.hp)
    }

    pub(super) fn change_unit_production(&mut self, city_id: u32, kind: UnitKind) -> bool {
        let Some(city) = self.cities.iter_mut().find(|city| city.id == city_id) else {
            return false;
        };
        // Native city_change_production_penalty keeps every shield when the
        // old and new targets are both units. This focused build has no
        // building production, so every exposed switch is in that class.
        city.production = kind;
        true
    }

    pub(super) fn choose_ai_production(&self, owner: usize, city_id: u32) -> UnitKind {
        let Some(city) = self.cities.iter().find(|c| c.id == city_id) else {
            return UnitKind::Warrior;
        };
        let defenders = self
            .units
            .iter()
            .filter(|u| u.owner == owner && u.x == city.x && u.y == city.y && u.kind.is_military())
            .count();
        if defenders == 0 {
            return if self.unit_available(owner, UnitKind::Phalanx) {
                UnitKind::Phalanx
            } else {
                UnitKind::Warrior
            };
        }
        let city_count = self.cities.iter().filter(|c| c.owner == owner).count();
        let has_founder = self
            .units
            .iter()
            .any(|u| u.owner == owner && u.kind == UnitKind::Settlers);
        if city.size >= 3 && city_count < 3 && !has_founder {
            return UnitKind::Settlers;
        }
        for kind in [
            UnitKind::Catapult,
            UnitKind::Chariot,
            UnitKind::Legion,
            UnitKind::Archer,
            UnitKind::Horseman,
            UnitKind::Phalanx,
            UnitKind::Warrior,
        ] {
            if self.unit_available(owner, kind) {
                return kind;
            }
        }
        UnitKind::Warrior
    }

    pub(super) fn process_cities(&mut self) {
        let city_ids: Vec<u32> = self.cities.iter().map(|c| c.id).collect();
        for city_id in city_ids {
            let Some(snapshot) = self.cities.iter().find(|c| c.id == city_id).cloned() else {
                continue;
            };
            let (food_income, shield_income, _) = self.city_yields(&snapshot);
            let mut grew = false;
            let mut starved = false;
            let mut destroyed = false;
            let mut finished = None;
            if let Some(city) = self.cities.iter_mut().find(|c| c.id == city_id) {
                city.food += food_income - city.size * CITIZEN_FOOD_COST;
                if city.food >= Self::growth_box(city.size) {
                    city.food -= Self::growth_box(city.size);
                    city.size += 1;
                    grew = true;
                } else if city.food < 0 {
                    starved = true;
                    if city.size > 1 {
                        city.size -= 1;
                        city.food = 0;
                    } else {
                        destroyed = true;
                    }
                } else {
                    city.food = city.food.max(0);
                }
                if !destroyed {
                    city.shields += shield_income;
                    let stats = city.production.stats();
                    if city.shields >= stats.cost
                        && (stats.pop_cost == 0 || city.size > stats.pop_cost)
                    {
                        city.shields -= stats.cost;
                        city.size -= stats.pop_cost;
                        finished = Some(city.production);
                    }
                }
            }
            if destroyed {
                self.cities.retain(|city| city.id != city_id);
                self.rebuild_city_ownership();
                if snapshot.owner == HUMAN {
                    self.add_event(format!("Famine destroyed {} entirely.", snapshot.name));
                }
                continue;
            }
            if grew && snapshot.owner == HUMAN {
                self.add_event(format!(
                    "{} grew to size {}.",
                    snapshot.name,
                    snapshot.size + 1
                ));
            }
            if starved && snapshot.owner == HUMAN {
                self.add_event(format!("{} lost population to famine.", snapshot.name));
            }
            if let Some(kind) = finished {
                self.spawn_unit(snapshot.owner, kind, snapshot.x, snapshot.y);
                if snapshot.owner == HUMAN {
                    self.add_event(format!("{} completed {}.", snapshot.name, kind.name()));
                }
                if self.players[snapshot.owner].ai {
                    let next = self.choose_ai_production(snapshot.owner, city_id);
                    if let Some(city) = self.cities.iter_mut().find(|c| c.id == city_id) {
                        city.production = next;
                    }
                }
            }
        }

        for owner in 0..PLAYER_COUNT {
            let science: i32 = self
                .cities
                .iter()
                .filter(|c| c.owner == owner)
                .map(|c| self.city_yields(c).2.max(1))
                .sum();
            if science <= 0 {
                continue;
            }
            self.players[owner].research += science;
            let current = self.players[owner].researching;
            let cost = self.target_cost(owner, current);
            if self.players[owner].research >= cost {
                self.players[owner].research -= cost;
                let first_bonus = current
                    .data()
                    .is_some_and(|advance| advance.first_discovery_bonus)
                    && self
                        .players
                        .iter()
                        .all(|player| !player.known.contains(&current));
                self.discover(owner, current);
                if first_bonus {
                    let bonus = self.research_target(owner);
                    if bonus != Tech::FUTURE {
                        self.discover(owner, bonus);
                    }
                }
                self.players[owner].researching = self.research_target(owner);
            }
        }
    }
}
