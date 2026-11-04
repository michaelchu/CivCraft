use super::*;
use crate::rules::Genus;

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
        tile.river
            || ISOHEX_DIRS.iter().any(|&(dx, dy)| {
                let (source_x, source_y) = offset_native_by_map(x, y, dx, dy);
                let source = self.tile(source_x, source_y);
                source.terrain.is_water() || source.river || source.irrigation
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
        if owner >= PLAYER_COUNT {
            return format!("{} {}", self.players[owner].name, count + 1);
        }
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
        if self
            .tile(unit.x, unit.y)
            .owner
            .is_some_and(|owner| owner != unit.owner && !self.at_war(owner, unit.owner))
        {
            if unit.owner == HUMAN {
                self.add_event("A treaty prevents settling on that tile.");
            }
            return false;
        }
        if !unit.kind.stats().can_found || !self.can_found_at(unit.x, unit.y) {
            if unit.owner == HUMAN {
                self.add_event("A city must be on land and at least 3 tiles from another city.");
            }
            return false;
        }
        self.units.remove(index);
        let name = self.create_city(unit.owner, unit.x, unit.y);
        if unit.owner == HUMAN {
            self.add_event(format!(
                "{name} founded. Its people begin training Warriors."
            ));
        }
        self.refresh_visibility();
        true
    }

    pub(super) fn create_city(&mut self, owner: usize, x: i32, y: i32) -> String {
        let name = self.city_name(owner);
        let id = self.next_city_id;
        self.next_city_id += 1;
        let first_city = !self.cities.iter().any(|city| city.owner == owner);
        self.cities.push(City {
            id,
            owner,
            x,
            y,
            name: name.clone(),
            size: 1,
            specialists: [0; 3],
            was_happy: false,
            food: 0,
            shields: 0,
            production: Production::Unit(UnitKind::Warrior),
            production_before_switch: None,
            buildings: if first_city && owner != BARBARIANS {
                vec![Building::PALACE]
            } else {
                Vec::new()
            },
        });
        let railroad = self.has_tech(owner, Tech::RAILROAD);
        let tile = self.tile_mut(x, y);
        tile.road = true;
        tile.railroad |= railroad;
        self.claim_city_tiles(x, y, owner);
        name
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
            .position(|c| c.x == x && c.y == y && self.at_war(c.owner, unit.owner))
        else {
            return;
        };
        let old_owner = self.cities[city_index].owner;
        self.record_provocation(old_owner, unit.owner);
        let city_name = self.cities[city_index].name.clone();
        let capital = self.cities[city_index]
            .buildings
            .contains(&Building::PALACE);
        let civil_war = capital && self.capital_loss_triggers_civil_war(old_owner);
        if capital {
            self.spaceship_parts[old_owner] = [0; 3];
        }
        self.cities[city_index].owner = unit.owner;
        if self.cities[city_index].size > 1 {
            self.cities[city_index].size -= 1;
            self.cities[city_index].remove_specialists(1);
        }
        self.release_city_support(self.cities[city_index].id);
        self.cities[city_index].food = 0;
        self.cities[city_index].shields = 0;
        self.cities[city_index].production_before_switch = None;
        self.cities[city_index].production = Production::Unit(UnitKind::Warrior);
        self.cities[city_index]
            .buildings
            .retain(|building| building.data().genus != Genus::SmallWonder);
        if old_owner != BARBARIANS
            && !self
                .cities
                .iter()
                .any(|city| city.owner == old_owner && city.buildings.contains(&Building::PALACE))
            && let Some(capital) = self.cities.iter_mut().find(|city| city.owner == old_owner)
        {
            capital.buildings.push(Building::PALACE);
        }
        self.claim_city_tiles(x, y, unit.owner);
        if civil_war {
            self.split_civil_war(old_owner);
        }
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
        let government = self.players[city.owner].government;
        let tile_penalty = matches!(
            government,
            Government::Tribal | Government::Despotism | Government::Anarchy
        ) && !self.has_tech(city.owner, Tech::RAILROAD)
            && !self.owns_wonder(city.owner, "Pyramids");
        let trade_bonus = matches!(government, Government::Republic | Government::Democracy);
        let adjust = |tile: &Tile, (food, shields, trade): (i32, i32, i32)| {
            let trade = trade + i32::from(trade_bonus && trade > 0 && !tile.terrain.is_water());
            let cap = |value| value - i32::from(tile_penalty && value > 2);
            (cap(food), cap(shields), cap(trade))
        };
        let (mut food, mut shields, mut trade) = adjust(center, (cf.max(1), cs.max(1), ct.max(1)));
        let mut candidates = Vec::new();
        for dy in -2..=2 {
            for dx in -2..=2 {
                let distance = isohex_vector_distance(dx, dy);
                if (dx == 0 && dy == 0) || distance * distance > 5 {
                    continue;
                }
                let (tile_x, tile_y) = offset_native_by_map(city.x, city.y, dx, dy);
                let tile = self.tile(tile_x, tile_y);
                let yields = adjust(tile, tile.yields());
                let score = yields.0 * 4 + yields.1 * 3 + yields.2 * 2;
                candidates.push((score, yields));
            }
        }
        candidates.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, (f, s, t)) in candidates.into_iter().take(city.tile_workers() as usize) {
            food += f;
            shields += s;
            trade += t;
        }
        let (shields, trade) = self.government_output_waste(city, shields, trade);
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

    pub(super) fn change_production(&mut self, city_id: u32, target: Production) -> bool {
        let Some(index) = self.cities.iter().position(|city| city.id == city_id) else {
            return false;
        };
        if !self.can_produce(&self.cities[index], target) {
            return false;
        }
        let city = &mut self.cities[index];
        let (original, shields) = *city
            .production_before_switch
            .get_or_insert((city.production, city.shields));
        city.shields = if original.class() == target.class() {
            shields
        } else {
            shields / 2
        };
        city.production = target;
        true
    }

    pub(super) fn choose_ai_production(&self, owner: usize, city_id: u32) -> Production {
        let Some(city) = self.cities.iter().find(|c| c.id == city_id) else {
            return Production::Unit(UnitKind::Warrior);
        };
        let defenders = self
            .units
            .iter()
            .filter(|u| u.owner == owner && u.x == city.x && u.y == city.y && u.kind.is_military())
            .count();
        if defenders == 0 {
            return Production::Unit(if self.unit_available(owner, UnitKind::Phalanx) {
                UnitKind::Phalanx
            } else {
                UnitKind::Warrior
            });
        }
        let city_count = self.cities.iter().filter(|c| c.owner == owner).count();
        let has_founder = self
            .units
            .iter()
            .any(|u| u.owner == owner && u.kind == UnitKind::Settlers);
        if city.size >= 3 && city_count < 3 && !has_founder {
            return Production::Unit(UnitKind::Settlers);
        }
        let priorities = [
            (city.size >= 4, "Temple"),
            (city.size >= 4, "Granary"),
            (city.size >= 2, "Library"),
            (city.size >= 3, "Marketplace"),
            (city.size >= 7, "Aqueduct"),
            (city.size >= 7, "Aqueduct, River"),
            (city.size >= 7, "Aqueduct, Lake"),
        ];
        for (wanted, name) in priorities {
            if wanted
                && let Some(building) = Building::all().find(|building| {
                    building.data().name == name && self.building_blocker(city, *building).is_none()
                })
            {
                return Production::Building(building);
            }
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
                return Production::Unit(kind);
            }
        }
        Production::Unit(UnitKind::Warrior)
    }

    pub(super) fn process_cities(&mut self) {
        let mut science_income = vec![0; self.players.len()];
        let mut gold_paid = Vec::new();
        let city_ids: Vec<u32> = self.cities.iter().map(|c| c.id).collect();
        for city_id in city_ids {
            let Some(snapshot) = self.cities.iter().find(|c| c.id == city_id).cloned() else {
                continue;
            };
            let (food_income, shield_income, trade) = self.city_yields(&snapshot);
            let [science, gold, luxury] = self.city_trade(&snapshot, trade);
            let (disorder, happy) = self.city_mood(&snapshot, luxury);
            let shields_due = self.city_unit_upkeep(&snapshot).shields;
            let (shield_surplus, recovered) =
                self.pay_shield_support(&snapshot, shield_income, shields_due);
            let support = self.city_unit_upkeep(&snapshot);
            let food_surplus = food_income - snapshot.size * CITIZEN_FOOD_COST - support.food;
            let starving_unit = (snapshot.food
                + if disorder {
                    food_surplus.min(0)
                } else {
                    food_surplus
                }
                < 0)
            .then_some(support.food_unit)
            .flatten();
            let size_limit = Self::city_size_limit(&snapshot);
            let food_retained = Self::food_retained(&snapshot);
            let can_produce = self.can_produce(&snapshot, snapshot.production);
            let mut grew = false;
            let mut starved = false;
            let mut destroyed = false;
            let mut finished = None;
            if let Some(city) = self.cities.iter_mut().find(|c| c.id == city_id) {
                city.production_before_switch = None;
                city.was_happy = happy;
                city.food += if disorder {
                    food_surplus.min(0)
                } else {
                    food_surplus
                };
                if city.food >= Self::growth_box(city.size) {
                    if city.size < size_limit {
                        let overflow = city.food - Self::growth_box(city.size);
                        city.size += 1;
                        city.food = city
                            .food
                            .min(overflow + percent_of(Self::growth_box(city.size), food_retained));
                        grew = true;
                    } else {
                        city.food = Self::growth_box(city.size);
                    }
                } else if city.food < 0 && starving_unit.is_some() {
                    city.food = percent_of(Self::growth_box(city.size), food_retained);
                } else if city.food < 0 {
                    starved = true;
                    if city.size > 1 {
                        city.size -= 1;
                        city.remove_specialists(1);
                        city.food = percent_of(Self::growth_box(city.size), food_retained);
                    } else {
                        destroyed = true;
                    }
                } else {
                    city.food = city.food.max(0);
                }
                if !destroyed {
                    city.shields += recovered + if disorder { 0 } else { shield_surplus };
                    if can_produce && !disorder {
                        let pop_cost = match city.production {
                            Production::Unit(kind) => kind.stats().pop_cost,
                            Production::Building(_) => 0,
                        };
                        let converts = matches!(city.production, Production::Building(building) if building.data().genus == Genus::Convert);
                        let cost = if converts {
                            city.shields
                        } else {
                            city.production.cost()
                        };
                        if city.shields >= cost && (pop_cost == 0 || city.size > pop_cost) {
                            city.shields -= cost;
                            city.size -= pop_cost;
                            city.remove_specialists(pop_cost);
                            finished = Some((city.production, cost));
                        }
                    }
                }
            }
            if let Some(unit) = starving_unit {
                self.remove_unsupported_unit(unit, &snapshot, "food");
            }
            if destroyed {
                self.release_city_support(city_id);
                self.cities.retain(|city| city.id != city_id);
                self.rebuild_city_ownership();
                if snapshot.owner == HUMAN {
                    self.add_event(format!("Famine destroyed {} entirely.", snapshot.name));
                }
                continue;
            }
            if snapshot.owner != BARBARIANS {
                science_income[snapshot.owner] += if disorder { 0 } else { science };
                let unit_gold = self.city_gold_upkeep(&snapshot);
                self.players[snapshot.owner].gold += (if disorder { 0 } else { gold })
                    - self.building_upkeep(&snapshot)
                    - unit_gold.iter().map(|(_, cost)| cost).sum::<i32>();
                gold_paid.extend(unit_gold);
                self.balance_building_upkeep(snapshot.owner, city_id);
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
            if let Some((target, shields)) = finished {
                match target {
                    Production::Unit(kind) => {
                        let id = self.spawn_unit(snapshot.owner, kind, snapshot.x, snapshot.y);
                        let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
                        unit.home_city = Some(city_id);
                        unit.home_order = id;
                    }
                    Production::Building(building) => {
                        self.finish_building(city_id, snapshot.owner, building, shields)
                    }
                }
                if snapshot.owner == HUMAN {
                    self.add_event(format!("{} completed {}.", snapshot.name, target.name()));
                }
                if self.players[snapshot.owner].ai {
                    let next = self.choose_ai_production(snapshot.owner, city_id);
                    if let Some(city) = self.cities.iter_mut().find(|c| c.id == city_id) {
                        city.production = next;
                    }
                }
            }
        }

        for (owner, science) in science_income.into_iter().enumerate() {
            if owner == BARBARIANS {
                continue;
            }
            self.balance_unit_gold_upkeep(owner, &gold_paid);
            self.apply_research_upkeep(owner, science);
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
