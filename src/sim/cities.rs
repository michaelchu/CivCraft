use super::*;
use crate::rules::Genus;

impl Simulation {
    pub(crate) fn city_radius(city: &City) -> impl Iterator<Item = (i32, i32, i32, i32)> + use<> {
        Self::city_radius_at(city.x, city.y)
    }

    pub(super) fn city_radius_at(x: i32, y: i32) -> impl Iterator<Item = (i32, i32, i32, i32)> {
        let (map_x, map_y) = native_to_map(x, y);
        (-2..=2).flat_map(move |dy| {
            (-2..=2).filter_map(move |dx| {
                let distance = isohex_vector_distance(dx, dy);
                let (x, y) = map_to_native(map_x + dx, map_y + dy);
                (distance * distance <= 5 && (0..MAP_H).contains(&y)).then_some((
                    dx,
                    dy,
                    x.rem_euclid(MAP_W),
                    y,
                ))
            })
        })
    }

    pub(super) fn tile_can_support_worker(&self, city: &City, x: i32, y: i32) -> bool {
        self.tile(x, y)
            .owner
            .is_none_or(|owner| owner == city.owner)
            && !self.cities.iter().any(|other| (other.x, other.y) == (x, y))
            && !self.units.iter().any(|unit| {
                (unit.x, unit.y) == (x, y)
                    && unit.kind.is_military()
                    && unit.transport.is_none()
                    && self.at_war(unit.owner, city.owner)
            })
    }

    pub(crate) fn can_assign_city_tile(&self, city: &City, x: i32, y: i32) -> bool {
        Self::city_radius(city).any(|(_, _, tx, ty)| (x, y) == (tx, ty))
            && self.tile_can_support_worker(city, x, y)
            && !self
                .cities
                .iter()
                .any(|other| other.worked_tiles.contains(&(x, y)))
    }

    pub(super) fn toggle_city_tile(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(index) = self
            .cities
            .iter()
            .position(|city| city.id == id && city.owner == HUMAN)
        else {
            return false;
        };
        let city = &self.cities[index];
        if let Some(tile) = city.worked_tiles.iter().position(|&pos| pos == (x, y)) {
            self.cities[index].worked_tiles.remove(tile);
            self.cities[index].specialists[2] += 1;
        } else {
            let Some(specialist) = [2, 0, 1].into_iter().find(|&i| city.specialists[i] > 0) else {
                return false;
            };
            if !self.can_assign_city_tile(city, x, y) {
                return false;
            }
            self.cities[index].specialists[specialist] -= 1;
            self.cities[index].worked_tiles.push((x, y));
        }
        true
    }

    pub(super) fn reconcile_city_workers(&mut self) {
        let mut occupied = std::collections::HashSet::new();
        // Preserve existing assignments before placing any newly available workers.
        for index in 0..self.cities.len() {
            let city = &self.cities[index];
            let tiles = city
                .worked_tiles
                .iter()
                .copied()
                .filter(|&(x, y)| self.tile_can_support_worker(city, x, y))
                .filter(|pos| !occupied.contains(pos))
                .take(city.tile_workers() as usize)
                .collect::<Vec<_>>();
            occupied.extend(tiles.iter().copied());
            self.cities[index].worked_tiles = tiles;
        }
        for index in 0..self.cities.len() {
            let city = &self.cities[index];
            let needed = city.tile_workers() as usize - city.worked_tiles.len();
            if needed == 0 {
                continue;
            }
            let mut candidates: Vec<_> = Self::city_radius(city)
                .filter(|&(_, _, x, y)| !occupied.contains(&(x, y)))
                .filter(|&(_, _, x, y)| self.tile_can_support_worker(city, x, y))
                .map(|(_, _, x, y)| {
                    let tile = self.tile(x, y);
                    let (food, shields, trade) = tile.yields();
                    (
                        tile.contamination_output(food) * 4
                            + tile.contamination_output(shields) * 3
                            + tile.contamination_output(trade) * 2,
                        (x, y),
                    )
                })
                .collect();
            candidates.sort_by_key(|&(score, _)| std::cmp::Reverse(score));
            let city = &mut self.cities[index];
            for (_, pos) in candidates.into_iter().take(needed) {
                city.worked_tiles.push(pos);
                occupied.insert(pos);
            }
            city.specialists[2] += city.tile_workers() - city.worked_tiles.len() as i32;
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

    pub(crate) fn can_irrigate_at(&self, owner: usize, x: i32, y: i32) -> bool {
        let tile = self.tile(x, y);
        !tile.irrigation
            && tile.terrain.irrigation_time() > 0
            && self.has_irrigation_source(owner, x, y)
    }

    pub(crate) fn can_farm_at(&self, owner: usize, x: i32, y: i32) -> bool {
        let tile = self.tile(x, y);
        tile.irrigation
            && !tile.farmland
            && !tile.mine
            && tile.terrain.irrigation_time() > 0
            && self.has_tech(owner, Tech::REFRIGERATION)
            && (tile.terrain != Terrain::Desert
                || tile.river
                || tile.resource == Some(Resource::Oasis))
            && self.has_irrigation_source(owner, x, y)
    }

    fn has_irrigation_source(&self, owner: usize, x: i32, y: i32) -> bool {
        let (mx, my) = native_to_map(x, y);
        self.has_tech(owner, Tech::ELECTRICITY)
            || self.tile(x, y).river
            || ISOHEX_DIRS.iter().any(|&(dx, dy)| {
                let (nx, ny) = map_to_native(mx + dx, my + dy);
                if !(0..MAP_H).contains(&ny) {
                    return false;
                }
                let source = self.tile(nx, ny);
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
        if !unit.kind.stats().can_found
            || unit.transport.is_some()
            || !self.can_found_at(unit.x, unit.y)
        {
            if unit.owner == HUMAN {
                self.add_event("A city must be on land and at least 3 tiles from another city.");
            }
            return false;
        }
        self.units.remove(index);
        let name = self.create_city(unit.owner, unit.x, unit.y);
        self.cities
            .last_mut()
            .unwrap()
            .add_citizen_nationality(unit.nationality, 1);
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
        self.city_founders.insert(id, owner);
        let first_city = !self.cities.iter().any(|city| city.owner == owner);
        self.cities.push(City {
            id,
            owner,
            x,
            y,
            name: name.clone(),
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
            production_before_switch: None,
            protected_shields: 0,
            caravan_shields: 0,
            technology_thefts: 0,
            worklist: Vec::new(),
            buildings: if first_city && owner != BARBARIANS {
                vec![Building::PALACE]
            } else {
                Vec::new()
            },
        });
        let railroad = self.has_tech(owner, Tech::RAILROAD);
        let tile = self.tile_mut(x, y);
        tile.road = true;
        tile.base = None;
        tile.railroad |= railroad;
        self.refresh_visibility();
        self.rebuild_city_ownership();
        name
    }

    pub(crate) fn can_join_city(&self, unit: &Unit, city: &City) -> bool {
        matches!(unit.kind, UnitKind::Migrants | UnitKind::Settlers)
            && unit.owner == city.owner
            && unit.moves_left > 0
            && (unit.y - city.y).abs() <= 2
            && self.map_distance(unit.x, unit.y, city.x, city.y) <= 1
            && city.size + unit.kind.stats().pop_cost <= 8.min(Self::city_size_limit(city))
    }

    pub(super) fn join_city(&mut self, unit_id: u32, city_id: u32) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == unit_id) else {
            return false;
        };
        let Some(index) = self
            .cities
            .iter()
            .position(|city| city.id == city_id && self.can_join_city(unit, city))
        else {
            return false;
        };
        let population = unit.kind.stats().pop_cost;
        let retained = Self::food_retained(&self.cities[index]);
        let city = &mut self.cities[index];
        city.size += population;
        city.add_citizen_nationality(unit.nationality, population);
        city.specialists[2] += population;
        city.food = city
            .food
            .max(percent_of(Self::growth_box(city.size), retained));
        let name = city.name.clone();
        let kind = unit.kind;
        let owner = unit.owner;
        self.remove_units_with_cargo(vec![unit_id]);
        self.refresh_visibility();
        if owner == HUMAN {
            self.add_event(format!(
                "{} joined {name}, adding {population} population.",
                kind.name()
            ));
        }
        true
    }

    pub(super) fn apply_combat_population_loss(&mut self, attacker: &Unit, x: i32, y: i32) {
        // Civ2Civ3 gives KillCitizen to Big Land and protects size-one cities.
        if !matches!(
            attacker.kind,
            UnitKind::Chariot | UnitKind::Catapult | UnitKind::Cannon
        ) {
            return;
        }
        let Some(index) = self.cities.iter().position(|city| {
            (city.x, city.y) == (x, y)
                && city.size > 1
                && !city.has_building(Building::CITY_WALLS)
                && !self.has_active_wonder(city.owner, Building::GREAT_WALL)
        }) else {
            return;
        };
        let city = &self.cities[index];
        let event = (attacker.owner == HUMAN || city.owner == HUMAN)
            .then(|| format!("{} lost one citizen in the attack.", city.name));
        self.reduce_city_population(index);
        if let Some(event) = event {
            self.add_event(event);
        }
    }

    pub(super) fn reduce_city_population(&mut self, index: usize) {
        let city = &mut self.cities[index];
        debug_assert!(city.size > 1);
        city.size -= 1;
        city.food = city.food.min(Self::growth_box(city.size));
        city.remove_specialists(1);
        self.remove_citizen_nationalities(index, 1);
        self.rebuild_city_ownership();
    }

    pub(super) fn transfer_traded_city(&mut self, index: usize, buyer: usize) {
        let original = self.cities[index].owner;
        let id = self.cities[index].id;
        let center = (self.cities[index].x, self.cities[index].y);
        let first_city = !self.cities.iter().any(|city| city.owner == buyer);
        self.cities[index].transfer_citizens(buyer);
        self.cities[index]
            .buildings
            .retain(|building| building.data().genus != Genus::SmallWonder);
        if first_city {
            self.cities[index].buildings.push(Building::PALACE);
        }

        let mut affected = std::collections::BTreeSet::from([center]);
        for unit in self.units.iter_mut().filter(|unit| unit.owner == original) {
            let stationed = (unit.x, unit.y) == center;
            if !stationed && unit.home_city != Some(id) {
                continue;
            }
            if !stationed
                && let Some(home) = self
                    .cities
                    .iter()
                    .find(|city| city.owner == original && (city.x, city.y) == (unit.x, unit.y))
            {
                unit.home_city = Some(home.id);
                continue;
            }
            affected.insert((unit.x, unit.y));
            unit.owner = buyer;
            unit.home_city = unit.home_city.map(|_| id);
            unit.automated = false;
            unit.clear_work();
            self.agent_city_targets.remove(&unit.id);
        }

        let displaced: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                affected.contains(&(unit.x, unit.y))
                    && (!self.allied(buyer, unit.owner)
                        || unit.transport.is_some_and(|id| {
                            self.units.iter().any(|carrier| {
                                carrier.id == id && !self.allied(unit.owner, carrier.owner)
                            })
                        }))
            })
            .map(|unit| unit.id)
            .collect();
        for unit in displaced {
            self.bounce_traded_city_unit(unit);
        }

        if !self.can_produce(&self.cities[index], self.cities[index].production) {
            self.cities[index].production = self.choose_ai_production(buyer, id);
        }
        self.prune_trade_routes();
        self.rebuild_city_ownership();
        self.refresh_visibility();
    }

    fn bounce_traded_city_unit(&mut self, id: u32) {
        let Some(unit) = self.units.iter().find(|unit| unit.id == id).cloned() else {
            return;
        };
        let (mx, my) = native_to_map(unit.x, unit.y);
        let mut destinations = Vec::new();
        for dx in -2..=2 {
            for dy in -2..=2 {
                if (dx, dy) == (0, 0) {
                    continue;
                }
                let (x, y) = map_to_native(mx + dx, my + dy);
                if !(0..MAP_H).contains(&y) {
                    continue;
                }
                let x = x.rem_euclid(MAP_W);
                if self.movement_cost_for(&unit, x, y).is_some()
                    && (!unit.kind.is_aircraft() || self.refuel_site(unit.owner, x, y))
                    && self.cities.iter().all(|city| {
                        (city.x, city.y) != (x, y) || self.allied(unit.owner, city.owner)
                    })
                    && self.units.iter().all(|other| {
                        (other.x, other.y) != (x, y) || self.allied(unit.owner, other.owner)
                    })
                {
                    destinations.push((x, y));
                }
            }
        }
        if destinations.is_empty() {
            let cargo: Vec<_> = self
                .units
                .iter()
                .filter(|unit| unit.transport == Some(id))
                .map(|unit| unit.id)
                .collect();
            for passenger in cargo {
                self.bounce_traded_city_unit(passenger);
            }
            self.remove_units_with_cargo(vec![id]);
            return;
        }
        let (x, y) = destinations[self.rand_below(destinations.len() as i32) as usize];
        let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
        (unit.x, unit.y) = (x, y);
        unit.transport = None;
        unit.clear_work();
        self.move_cargo(id, x, y);
    }

    pub(super) fn capture_city_if_needed(&mut self, unit_id: u32, x: i32, y: i32) {
        let Some(unit) = self.units.iter().find(|u| u.id == unit_id).cloned() else {
            return;
        };
        if !unit.kind.can_occupy_city() {
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
            self.lose_spaceship(old_owner);
        }
        self.cities[city_index].transfer_citizens(unit.owner);
        self.prune_trade_routes();
        if self.cities[city_index].size > 1 {
            self.cities[city_index].size -= 1;
            self.remove_citizen_nationalities(city_index, 1);
            self.cities[city_index].remove_specialists(1);
        }
        self.release_city_support(self.cities[city_index].id);
        self.cities[city_index].food = 0;
        self.cities[city_index].shields = 0;
        self.cities[city_index].production_before_switch = None;
        self.cities[city_index].protected_shields = 0;
        self.cities[city_index].caravan_shields = 0;
        self.cities[city_index].worklist.clear();
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
        self.rebuild_city_ownership();
        if civil_war {
            self.split_civil_war(old_owner);
        }
        self.inspire_partisans(city_index, old_owner);
        if unit.owner == HUMAN || old_owner == HUMAN {
            self.add_event(format!(
                "{} captured {city_name} from {}.",
                self.players[unit.owner].name, self.players[old_owner].name
            ));
        }
    }

    pub(crate) fn city_yields(&self, city: &City) -> (i32, i32, i32) {
        self.city_yields_with_tech(city, None)
    }

    pub(super) fn city_yields_with_tech(
        &self,
        city: &City,
        granted: Option<Tech>,
    ) -> (i32, i32, i32) {
        let mut center = self.tile(city.x, city.y).clone();
        center.railroad |= granted == Some(Tech::RAILROAD);
        let (mut cf, cs, ct) = center.yields();
        if !center.irrigation && !center.mine {
            // City centers receive the terrain's irrigation bonus
            // automatically, but an explicit irrigation does not stack.
            cf += center.irrigation_food();
        }
        let government = self.players[city.owner].government;
        let tile_penalty = matches!(
            government,
            Government::Tribal | Government::Despotism | Government::Anarchy
        ) && granted != Some(Tech::RAILROAD)
            && !self.has_tech(city.owner, Tech::RAILROAD)
            && !self.has_active_wonder(city.owner, Building::PYRAMIDS);
        let trade_bonus = matches!(government, Government::Republic | Government::Democracy);
        let supermarket = city.has_building(Building::SUPERMARKET);
        let highways = city.has_building(Building::SUPER_HIGHWAYS);
        let colossus = city.has_building(Building::COLOSSUS)
            && !self.building_is_obsolete(city, Building::COLOSSUS, granted);
        let adjust = |tile: &Tile, (food, shields, trade): (i32, i32, i32), is_center: bool| {
            let farm_output =
                tile.farmland || (is_center && !tile.mine && tile.terrain.irrigation_time() > 0);
            let food = food
                + if supermarket && farm_output {
                    food / 2
                } else {
                    0
                };
            let shields = shields
                + i32::from(
                    city.has_building(Building::OFFSHORE_PLATFORM)
                        && tile.terrain.is_water()
                        && (tile.terrain != Terrain::DeepOcean || tile.oil_platform),
                );
            let highway_bonus = highways && tile.road && trade > 0 && (is_center || !tile.farmland);
            let trade = trade
                + i32::from(trade_bonus && trade > 0 && !tile.terrain.is_water())
                + i32::from(highway_bonus)
                + i32::from(colossus && trade > 0);
            let cap =
                |value| tile.contamination_output(value - i32::from(tile_penalty && value > 2));
            (cap(food), cap(shields), cap(trade))
        };
        let (mut food, mut shields, mut trade) = adjust(&center, (cf, cs, ct), true);
        food = food.max(1);
        shields = shields.max(1);
        trade = trade.max(1);
        for &(x, y) in city.worked_tiles.iter().take(city.tile_workers() as usize) {
            let tile = self.tile(x, y);
            let (f, s, t) = adjust(tile, tile.yields(), false);
            food += f;
            shields += s;
            trade += t;
        }
        trade += self.route_trade(city);
        let (shields, trade) = self.government_output_waste(city, shields, trade, granted);
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

    pub(super) fn hitpoint_gain(unit: &Unit, in_city_center: bool, fortress: bool) -> i32 {
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
        let hp_regen = if fortress && !unit.kind.is_naval() && !unit.kind.is_aircraft() {
            percent_of(base, 25)
        } else {
            0
        };
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
        if !self.can_change_production(&self.cities[index], target) {
            return false;
        }
        let city = &mut self.cities[index];
        let (original, shields) = *city
            .production_before_switch
            .get_or_insert((city.production, city.shields));
        let caravan = city.caravan_shields.min(shields);
        let regular = shields - caravan;
        let protected = city.protected_shields.min(regular);
        let mut unpenalized = protected;
        let mut penalized = 0;
        if original.class() == target.class() {
            unpenalized += regular - protected;
        } else {
            penalized += regular - protected;
        }
        if matches!(target, Production::Building(building) if building.data().genus == Genus::GreatWonder)
        {
            unpenalized += caravan;
        } else {
            penalized += caravan;
        }
        city.shields = unpenalized + penalized / 2;
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
        if self.players[owner].ai {
            if let Some(target) = self
                .counterintelligence_production(city)
                .or_else(|| self.city_defense_production(city))
            {
                return target;
            }
            if defenders == 0 {
                return Production::Unit(self.preferred_city_defender(city));
            }
        }
        if defenders == 0 {
            return Production::Unit(if self.unit_available(owner, UnitKind::Phalanx) {
                UnitKind::Phalanx
            } else {
                UnitKind::Warrior
            });
        }
        if self.players[owner].ai
            && let Some(kind) = self.settlement_production(city)
        {
            return Production::Unit(kind);
        }
        if self.players[owner].ai
            && let Some(kind) = self.hunter_production(city)
        {
            return Production::Unit(kind);
        }
        if self.players[owner].ai {
            if let Some(building) = self.domestic_production(city) {
                return Production::Building(building);
            }
        } else {
            let priorities = [
                (city.size >= 4, Building::TEMPLE),
                (city.size >= 4, Building::GRANARY),
                (city.size >= 2, Building::LIBRARY),
                (city.size >= 3, Building::MARKETPLACE),
                (city.size >= 7, Building::AQUEDUCT),
                (city.size >= 7, Building::AQUEDUCT_RIVER),
                (city.size >= 7, Building::AQUEDUCT_LAKE),
            ];
            for (wanted, building) in priorities {
                if wanted && self.building_blocker(city, building).is_none() {
                    return Production::Building(building);
                }
            }
        }
        if self.players[owner].ai {
            for building in [
                Building::APOLLO_PROGRAM,
                Building::SPACE_MODULE,
                Building::SPACE_COMPONENT,
                Building::SPACE_STRUCTURAL,
            ] {
                if self.building_blocker(city, building).is_none() {
                    return Production::Building(building);
                }
            }
        }
        if self.players[owner].ai
            && let Some(kind) = self.merchant_production(city)
        {
            return Production::Unit(kind);
        }
        if self.players[owner].ai
            && let Some(kind) = self.offensive_agent_production(city)
        {
            return Production::Unit(kind);
        }
        if self.players[owner].ai
            && let Some(kind) = self.offensive_production(city)
        {
            return Production::Unit(kind);
        }
        for kind in UnitKind::AI_MILITARY {
            if self.unit_available(owner, kind) {
                return Production::Unit(kind);
            }
        }
        Production::Unit(UnitKind::Warrior)
    }

    pub(super) fn process_cities(&mut self) {
        self.reconcile_city_workers();
        let mut science_income = vec![0; self.players.len()];
        let mut gold_paid = Vec::new();
        let city_ids: Vec<u32> = self.cities.iter().map(|c| c.id).collect();
        for city_id in city_ids {
            let Some(index) = self.cities.iter().position(|city| city.id == city_id) else {
                continue;
            };
            self.assimilate_citizen(index);
            let city = &self.cities[index];
            if !city.bought_production {
                self.cities[index].production = self.worklist_replacement(city, city.production);
            }
            let snapshot = self.cities[index].clone();
            let forecast = self.city_forecast(&snapshot);
            let (shield_surplus, recovered) =
                self.pay_shield_support(&snapshot, forecast.shields, forecast.support.shields);
            let food_support = self.city_unit_upkeep(&snapshot);
            let initial_gold = self.city_gold_upkeep(&snapshot);
            let can_produce = match snapshot.production {
                Production::Unit(kind) => self.unit_buildable(&snapshot, kind),
                target => self.can_produce(&snapshot, target),
            };
            let city = &mut self.cities[index];
            city.production_before_switch = None;
            city.protected_shields = 0;
            city.caravan_shields = 0;
            let surplus = if forecast.disorder { 0 } else { shield_surplus };
            city.shields += recovered + surplus;
            let finished = if can_produce {
                city.resolve_production()
            } else {
                None
            };
            if finished.is_some() {
                city.protected_shields = surplus.min(city.shields);
                self.reconcile_city_workers();
            }
            if let Some((target, shields)) = finished {
                match target {
                    Production::Unit(kind) => {
                        self.nation_scores[snapshot.owner].units_built += 1;
                        let nationality =
                            self.remove_citizen_nationalities(index, kind.stats().pop_cost);
                        let rank = self.trained_rank(&snapshot, kind);
                        let id = self.spawn_unit(snapshot.owner, kind, snapshot.x, snapshot.y);
                        let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
                        unit.nationality = nationality;
                        unit.home_city = Some(city_id);
                        unit.home_order = id;
                        unit.veteran = rank;
                    }
                    Production::Building(building) => {
                        self.finish_building(city_id, snapshot.owner, building, shields)
                    }
                }
                if snapshot.owner == HUMAN {
                    self.add_event(format!("{} completed {}.", snapshot.name, target.name()));
                }
                if !self.advance_city_worklist(index)
                    && (self.players[snapshot.owner].ai
                        || !self.can_produce(&self.cities[index], target))
                {
                    self.cities[index].production =
                        self.choose_ai_production(snapshot.owner, city_id);
                }
            }
            self.record_history(index);
            // Buildings and population-cost units refresh city output before growth.
            let refreshed = self.cities[index].size != snapshot.size
                || matches!(finished, Some((Production::Building(building), _))
                    if matches!(building.data().genus, Genus::Improvement | Genus::SmallWonder | Genus::GreatWonder));
            let illness_shields = if refreshed {
                self.city_forecast(&self.cities[index]).shields
            } else {
                forecast.shields
            };
            let plague = self.check_city_plague(index, illness_shields);
            let populated = self.cities[index].clone();
            let refreshed = refreshed || plague;
            let growth_forecast = refreshed.then(|| self.city_forecast(&populated));
            let food_surplus = growth_forecast.as_ref().map_or_else(
                || forecast.food_surplus(populated.size, food_support.food),
                |updated| updated.food_surplus(populated.size, updated.support.food),
            );
            let starving_unit = if populated.food + food_surplus < 0 {
                growth_forecast
                    .as_ref()
                    .map_or(food_support.food_unit, |updated| updated.support.food_unit)
            } else {
                None
            };
            let city = &mut self.cities[index];
            city.was_happy = forecast.happy
                || growth_forecast
                    .as_ref()
                    .is_some_and(|updated| updated.happy);
            let growth = city.resolve_growth(
                food_surplus,
                Self::city_size_limit(&populated),
                Self::food_retained(&populated),
                starving_unit.is_some(),
            );
            if growth == Growth::Starved {
                self.remove_citizen_nationalities(index, 1);
            }
            if let Some(unit) = starving_unit {
                self.remove_unsupported_unit(unit, &populated, "food");
            }
            if growth == Growth::Destroyed {
                self.release_city_support(city_id);
                if self.cities[index].has_building(Building::PALACE) {
                    self.lose_spaceship(snapshot.owner);
                }
                let removed = self.cities.remove(index);
                self.city_founders.remove(&removed.id);
                self.rebuild_city_ownership();
                if snapshot.owner == HUMAN {
                    self.add_event(format!("Famine destroyed {} entirely.", snapshot.name));
                }
                continue;
            }
            if growth == Growth::Grew && snapshot.owner == HUMAN {
                self.add_event(format!(
                    "{} grew to size {}.",
                    snapshot.name,
                    populated.size + 1
                ));
            }
            if growth == Growth::Starved && snapshot.owner == HUMAN {
                self.add_event(format!("{} lost population to famine.", snapshot.name));
            }
            self.cities[index].bought_production = false;
            self.cities[index].sold_building = false;
            if growth != Growth::Unchanged {
                self.reconcile_city_workers();
            }
            if snapshot.owner != BARBARIANS {
                let city = &self.cities[index];
                let income_refreshed =
                    refreshed || growth != Growth::Unchanged || starving_unit.is_some();
                let income = if income_refreshed {
                    self.city_forecast(city)
                } else {
                    forecast
                };
                science_income[snapshot.owner] += income.science;
                let unit_gold = if income_refreshed {
                    self.city_gold_upkeep(city)
                } else {
                    initial_gold
                };
                self.players[snapshot.owner].gold +=
                    income.gold_surplus(unit_gold.iter().map(|(_, cost)| cost).sum());
                gold_paid.extend(unit_gold);
                self.balance_building_upkeep(snapshot.owner, city_id);
                let city = self.cities[index].clone();
                self.check_city_pollution(&city, income.shields);
            }
        }

        self.reconcile_city_workers();
        for (owner, science) in science_income.into_iter().enumerate() {
            if owner == BARBARIANS {
                continue;
            }
            self.balance_unit_gold_upkeep(owner, &gold_paid);
            self.resolve_research_turn(owner, science);
        }
    }
}

#[derive(PartialEq, Eq)]
enum Growth {
    Unchanged,
    Grew,
    Starved,
    Destroyed,
}

impl City {
    fn resolve_growth(
        &mut self,
        surplus: i32,
        size_limit: i32,
        retained: i32,
        starving_unit: bool,
    ) -> Growth {
        self.food += surplus;
        let capacity = Simulation::growth_box(self.size);
        if self.food >= capacity {
            if self.size >= size_limit {
                self.food = capacity;
                return Growth::Unchanged;
            }
            let overflow = self.food - capacity;
            self.size += 1;
            self.food = self
                .food
                .min(overflow + percent_of(Simulation::growth_box(self.size), retained));
            return Growth::Grew;
        }
        if self.food >= 0 {
            return Growth::Unchanged;
        }
        if starving_unit {
            self.food = percent_of(capacity, retained);
            return Growth::Unchanged;
        }
        if self.size == 1 {
            return Growth::Destroyed;
        }
        self.size -= 1;
        self.remove_specialists(1);
        self.food = percent_of(Simulation::growth_box(self.size), retained);
        Growth::Starved
    }

    fn resolve_production(&mut self) -> Option<(Production, i32)> {
        let population = match self.production {
            Production::Unit(kind) => kind.stats().pop_cost,
            Production::Building(_) => 0,
        };
        let cost = if matches!(self.production, Production::Building(building) if building.data().genus == Genus::Convert)
        {
            self.shields
        } else {
            self.production.cost()
        };
        if self.shields < cost || (population > 0 && self.size <= population) {
            return None;
        }
        self.shields -= cost;
        self.size -= population;
        if population > 0 {
            self.food = self.food.min(Simulation::growth_box(self.size));
            self.remove_specialists(population);
        }
        Some((self.production, cost))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrants_cost_population_and_obey_join_size_limits() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        assert!(!game.unit_available(HUMAN, UnitKind::Migrants));
        game.players[HUMAN].known.push(Tech::POTTERY);
        assert!(game.unit_available(HUMAN, UnitKind::Migrants));
        let city = &mut game.cities[0];
        city.production = Production::Unit(UnitKind::Migrants);
        city.shields = 10;
        assert!(city.resolve_production().is_none());
        city.size = 2;
        assert_eq!(
            city.resolve_production(),
            Some((Production::Unit(UnitKind::Migrants), 10))
        );
        assert_eq!(city.size, 1);
        let id = city.id;
        city.buildings.push(Building::SEWER_SYSTEM);
        city.size = 8;
        let migrants = game.spawn_unit(HUMAN, UnitKind::Migrants, 10, 20);
        assert!(!game.apply(Command::JoinCity(migrants, id)));
        game.cities[0].size = 7;
        assert!(game.apply(Command::JoinCity(migrants, id)));
        assert_eq!(game.cities[0].size, 8);
    }

    #[test]
    fn desert_farmland_needs_a_food_bonus_and_supermarket_applies_to_worked_tiles() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.create_city(HUMAN, x, y);
        let (fx, fy) = offset_native_by_map(x, y, 1, 0);
        let tile = game.tile_mut(fx, fy);
        tile.terrain = Terrain::Desert;
        tile.irrigation = true;
        tile.river = false;
        tile.resource = None;
        game.players[HUMAN]
            .known
            .extend([Tech::REFRIGERATION, Tech::ELECTRICITY, Tech::RAILROAD]);
        let worker = game.spawn_unit(HUMAN, UnitKind::Worker, fx, fy);
        assert!(!game.apply(Command::Work(worker, WorkKind::Farmland)));
        game.tile_mut(fx, fy).river = true;
        assert!(game.apply(Command::Work(worker, WorkKind::Farmland)));
        for _ in 0..5 {
            game.update_activities_and_restore();
        }
        assert!(game.tile(fx, fy).farmland);
        game.cities[0].worked_tiles = vec![(fx, fy)];
        let center_food = game.tile(x, y).yields().0 + game.tile(x, y).irrigation_food();
        let farm_food = game.tile(fx, fy).yields().0;
        assert_eq!(game.city_yields(&game.cities[0]).0, center_food + farm_food);
        game.cities[0].buildings.push(Building::SUPERMARKET);
        assert_eq!(
            game.city_yields(&game.cities[0]).0,
            center_food + center_food / 2 + farm_food + farm_food / 2
        );

        assert!(game.apply(Command::Work(worker, WorkKind::Mine)));
        for _ in 0..5 {
            game.update_activities_and_restore();
        }
        assert!(game.tile(fx, fy).mine);
        assert!(!game.tile(fx, fy).farmland);
        assert!(!game.tile(fx, fy).irrigation);
    }

    #[test]
    fn production_uses_population_before_growth() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let id = game.cities[0].id;
        let target = Production::Unit(UnitKind::Settlers);
        game.cities[0].size = 2;
        game.cities[0].food = Simulation::growth_box(2) + 20;
        game.cities[0].shields = target.cost();
        assert!(game.apply(Command::Produce(id, target)));
        game.process_cities();
        assert_eq!(game.cities[0].size, 3);
        assert!(
            !game
                .units
                .iter()
                .any(|unit| unit.kind == UnitKind::Settlers && unit.home_city == Some(id))
        );

        game.process_cities();
        assert!(
            game.units
                .iter()
                .any(|unit| unit.kind == UnitKind::Settlers && unit.home_city == Some(id))
        );
    }

    #[test]
    fn completed_aqueduct_allows_growth_in_the_same_turn() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let (x, y) = offset_native_by_map(10, 20, 1, 0);
        game.tile_mut(x, y).terrain = Terrain::Lake;
        let id = game.cities[0].id;
        let target = Production::Building(Building::AQUEDUCT_LAKE);
        game.cities[0].size = 8;
        game.cities[0].food = Simulation::growth_box(8) + 20;
        assert!(game.apply(Command::Produce(id, target)));
        game.cities[0].shields = target.cost();
        game.process_cities();
        assert!(game.cities[0].has_building(Building::AQUEDUCT_LAKE));
        assert_eq!(game.cities[0].size, 9);
    }

    #[test]
    fn overlapping_cities_reserve_worked_tiles_and_release_them_on_reassignment() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        let second = offset_native_by_map(10, 20, 3, 0);
        game.create_city(HUMAN, 10, 20);
        game.create_city(HUMAN, second.0, second.1);
        let shared = offset_native_by_map(10, 20, 1, 0);
        let first = game.cities[0].id;
        let second = game.cities[1].id;
        for id in [first, second] {
            assert!(game.apply(Command::Specialist(id, 2, 1)));
        }
        assert!(game.apply(Command::WorkCityTile(first, shared.0, shared.1)));
        assert!(!game.apply(Command::WorkCityTile(second, shared.0, shared.1)));
        assert!(game.apply(Command::WorkCityTile(first, shared.0, shared.1)));
        assert!(game.apply(Command::WorkCityTile(second, shared.0, shared.1)));

        game.spawn_unit(1, UnitKind::Warrior, shared.0, shared.1);
        game.reconcile_city_workers();
        assert!(!game.cities[1].worked_tiles.contains(&shared));
        assert_eq!(game.cities[1].worked_tiles.len(), 1);
        assert!(!game.cities[0].worked_tiles.contains(&shared));
    }
}
