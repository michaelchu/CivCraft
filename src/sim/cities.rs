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
                let (x, y) = map_pos_to_native(map_x + dx, map_y + dy);
                (distance * distance <= 5).then_some((dx, dy, x, y))
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
        self.has_tech(owner, Tech::ELECTRICITY)
            || self.tile(x, y).river
            || super::terrain_work::neighbors(x, y).any(|(nx, ny)| {
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
        if unit.moves_left <= 0 {
            return false; // Civ2Civ3 Found City requires MinMoveFrags 1.
        }
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
        self.discard_unit_ai(unit_id);
        self.ai_settlement_targets.remove(&unit_id);
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
            specialists: [0, 0, 1],
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
        self.list_order.city(self.cities.last().unwrap());
        let railroad = self.has_tech(owner, Tech::RAILROAD);
        let tile = self.tile_mut(x, y);
        tile.road = true;
        tile.base = None;
        tile.railroad |= railroad;
        self.refresh_visibility();
        self.rebuild_city_ownership_for_city_event(id);
        self.refresh_diplomatic_contact_at(owner, x, y);
        name
    }

    pub(crate) fn can_join_city(&self, unit: &Unit, city: &City) -> bool {
        matches!(
            unit.kind,
            UnitKind::Migrants | UnitKind::Settlers | UnitKind::Fanatics
        ) && unit.owner == city.owner
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
        self.rebuild_city_ownership_for_city_event(city_id);
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
        if !attacker.kind.is_big_land() {
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
        city.remove_citizen_assignments(1);
        self.remove_citizen_nationalities(index, 1);
        self.rebuild_city_ownership_for_city_event(self.cities[index].id);
    }

    pub(super) fn transfer_traded_city(&mut self, index: usize, buyer: usize) {
        let original = self.cities[index].owner;
        let id = self.cities[index].id;
        let center = (self.cities[index].x, self.cities[index].y);
        let first_city = !self.cities.iter().any(|city| city.owner == buyer);
        self.cities[index].transfer_citizens(buyer);
        self.list_order.city(&self.cities[index]);
        self.cities[index]
            .buildings
            .retain(|building| building.data().genus != Genus::SmallWonder);
        if first_city {
            self.cities[index].buildings.push(Building::PALACE);
        }

        let mut affected = std::collections::BTreeSet::from([center]);
        let transfer_units = self.city_transfer_unit_ids(id, original);
        for unit_id in transfer_units {
            let unit = self
                .units
                .iter_mut()
                .find(|unit| unit.id == unit_id)
                .unwrap();
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
            self.list_order.unit(unit);
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
        self.rebuild_city_ownership_for_city_event(id);
        self.refresh_diplomatic_contact_at(buyer, center.0, center.1);
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
                let (x, y) = map_pos_to_native(mx + dx, my + dy);
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
        self.list_order.unit(unit);
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
        let city_name = self.cities[city_index].name.clone();
        let capital = self.cities[city_index]
            .buildings
            .contains(&Building::PALACE);
        let civil_war = capital && self.capital_loss_triggers_civil_war(old_owner);
        if capital {
            self.lose_spaceship(old_owner);
        }
        self.cities[city_index].transfer_citizens(unit.owner);
        self.list_order.city(&self.cities[city_index]);
        self.prune_trade_routes();
        if self.cities[city_index].size > 1 {
            self.cities[city_index].size -= 1;
            self.remove_citizen_nationalities(city_index, 1);
            self.cities[city_index].remove_citizen_assignments(1);
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
        self.rebuild_city_ownership_for_city_event(self.cities[city_index].id);
        self.refresh_diplomatic_contact_at(unit.owner, x, y);
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

    pub(super) fn city_tile_yields(
        &self,
        city: &City,
        x: i32,
        y: i32,
        is_center: bool,
        granted: Option<Tech>,
    ) -> (i32, i32, i32) {
        self.city_tile_yields_under_government(
            city,
            x,
            y,
            is_center,
            granted,
            self.players[city.owner].government,
        )
    }

    pub(super) fn city_tile_yields_under_government(
        &self,
        city: &City,
        x: i32,
        y: i32,
        is_center: bool,
        granted: Option<Tech>,
        government: Government,
    ) -> (i32, i32, i32) {
        let celebrating = self.base_city_celebrating(city);
        let tile_penalty = matches!(
            government,
            Government::Tribal | Government::Despotism | Government::Anarchy
        ) && granted != Some(Tech::RAILROAD)
            && !self.has_tech(city.owner, Tech::RAILROAD)
            && !self.has_active_wonder(city.owner, Building::PYRAMIDS)
            && self.city_rapture.get(&city.id).copied().unwrap_or(0) <= 0;
        let republic_trade = matches!(government, Government::Republic | Government::Democracy);
        let celebrate_trade = celebrating
            && matches!(
                government,
                Government::Monarchy
                    | Government::Federation
                    | Government::Republic
                    | Government::Democracy
            );
        let pyramids = city.has_building(Building::PYRAMIDS)
            && !self.building_is_obsolete(city, Building::PYRAMIDS, granted);
        let colossus = city.has_building(Building::COLOSSUS)
            && !self.building_is_obsolete(city, Building::COLOSSUS, granted);
        let mut center;
        let tile = if is_center {
            center = self.tile(x, y).clone();
            center.railroad |= granted == Some(Tech::RAILROAD);
            &center
        } else {
            self.tile(x, y)
        };
        let (mut food, mut shields, mut trade) = tile.yields();
        if is_center && !tile.irrigation && !tile.mine && tile.terrain.irrigation_time() > 0 {
            food += tile.irrigation_food();
        }
        // common/city.c:city_tile_output: terrain/extra/road outputs, then
        // Add_Tile, Inc_Tile(+celebration), Per_Tile, penalty, punishment,
        // and finally the Civ2Civ3 center minima (all zero in game.ruleset).
        food += i32::from(
            city.has_building(Building::HARBOR)
                && matches!(tile.terrain, Terrain::Ocean | Terrain::DeepOcean),
        );
        shields += i32::from(is_center)
            + i32::from(
                city.has_building(Building::OFFSHORE_PLATFORM)
                    && tile.terrain.is_water()
                    && (tile.terrain != Terrain::DeepOcean || tile.oil_platform),
            );
        if shields > 0 {
            shields += i32::from(pyramids);
        }
        if trade > 0 {
            trade += i32::from(republic_trade && !tile.terrain.is_water())
                + i32::from(celebrate_trade)
                + i32::from(colossus)
                + i32::from(
                    city.has_building(Building::SUPER_HIGHWAYS)
                        && tile.road
                        && (is_center || !tile.farmland),
                );
        }
        let farm = tile.farmland
            || (is_center && !tile.mine && !tile.oil_well && tile.terrain.irrigation_time() > 0);
        if food > 0 && city.has_building(Building::SUPERMARKET) && farm {
            food += food * 50 / 100;
        }
        let finish =
            |value| tile.contamination_output(value - i32::from(tile_penalty && value > 2));
        (
            finish(food).max(0),
            finish(shields).max(0),
            finish(trade).max(0),
        )
    }

    pub(super) fn city_yields_with_tech(
        &self,
        city: &City,
        granted: Option<Tech>,
    ) -> (i32, i32, i32) {
        let (mut food, mut shields, mut trade) =
            self.city_tile_yields(city, city.x, city.y, true, granted);
        for &(x, y) in city.worked_tiles.iter().take(city.tile_workers() as usize) {
            let (f, s, t) = self.city_tile_yields(city, x, y, false, granted);
            food += f;
            shields += s;
            trade += t;
        }
        trade += self.route_trade(city);
        // common/city.c:set_city_production applies output bonuses before
        // calculating waste; waste therefore includes industrial output.
        shields = percent_of(shields, self.shield_output_bonus(city));
        let (shields, trade) = self.government_output_waste(city, shields, trade, granted);
        food = self.government_food_output(city, food, self.players[city.owner].government);
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
        let Some(city) = self.cities.iter().find(|city| city.id == city_id) else {
            return Production::Unit(UnitKind::Warrior);
        };
        if self.players[owner].ai {
            if let Some(choice) = self.ai_build_choice(city) {
                return choice.production;
            }
        } else {
            let defenders = self
                .units
                .iter()
                .filter(|unit| {
                    unit.owner == owner
                        && unit.kind.is_military()
                        && (unit.x, unit.y) == (city.x, city.y)
                })
                .count();
            if defenders == 0 {
                if let Some(kind) = UnitKind::ALL_NATIVE.into_iter().rev().find(|kind| {
                    kind.has_role(super::unit_catalog::UnitRole::FirstBuild)
                        && self.unit_buildable(city, *kind)
                }) {
                    return Production::Unit(kind);
                }
            }
            for (wanted, building) in [
                (city.size >= 4, Building::TEMPLE),
                (city.size >= 4, Building::GRANARY),
                (city.size >= 2, Building::LIBRARY),
                (city.size >= 3, Building::MARKETPLACE),
                (city.size >= 7, Building::AQUEDUCT),
                (city.size >= 7, Building::AQUEDUCT_RIVER),
                (city.size >= 7, Building::AQUEDUCT_LAKE),
            ] {
                if wanted && self.building_blocker(city, building).is_none() {
                    return Production::Building(building);
                }
            }
        }
        for kind in UnitKind::ALL_NATIVE.into_iter().rev() {
            if kind.has_role(super::unit_catalog::UnitRole::FirstBuild)
                && self.unit_buildable(city, kind)
            {
                return Production::Unit(kind);
            }
        }
        Production::Unit(UnitKind::Warrior)
    }

    pub(super) fn process_cities(&mut self) {
        for owner in 0..self.players.len() {
            let income = self.process_player_cities(owner);
            self.finish_player_national_activities(owner, income);
        }
    }

    pub(super) fn process_player_cities(&mut self, owner: usize) -> (i32, Vec<(u32, i32)>) {
        self.reconcile_city_workers();
        let mut science_income = 0;
        let mut gold_paid = Vec::new();
        let mut city_ids: Vec<u32> = self
            .player_cities_ordered(owner)
            .map(|city| city.id)
            .collect();
        // cityturn.c:update_city_activities converts citizens in player
        // list order before randomizing the city activity order.
        for &id in &city_ids {
            if let Some(index) = self.cities.iter().position(|city| city.id == id) {
                self.assimilate_citizen(index);
            }
        }
        while !city_ids.is_empty() {
            let selected = self.rand_below(city_ids.len() as i32) as usize;
            let city_id = city_ids.swap_remove(selected);
            let Some(index) = self.cities.iter().position(|city| city.id == city_id) else {
                continue;
            };
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
                        if kind.stats().pop_cost > 0 {
                            self.rebuild_city_ownership_for_city_event(city_id);
                        }
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
            let production_forecast = refreshed.then(|| self.city_forecast(&self.cities[index]));
            let post_build_happy = production_forecast
                .as_ref()
                .map_or(forecast.happy, |updated| updated.happy);
            let initial_celebrating = self.base_city_celebrating(&snapshot) && forecast.happy;
            let current_celebrating =
                self.base_city_celebrating(&self.cities[index]) && post_build_happy;
            if initial_celebrating || current_celebrating {
                let rapture = self.city_rapture.entry(city_id).or_default();
                *rapture = rapture.wrapping_add(1);
            } else {
                self.city_rapture.remove(&city_id);
            }
            self.cities[index].was_happy = forecast.happy || post_build_happy;
            let illness_shields = production_forecast
                .as_ref()
                .map_or(forecast.shields, |updated| updated.shields);
            let plague = self.check_city_plague(index, illness_shields);
            let populated = self.cities[index].clone();
            let refreshed = refreshed || plague;
            let growth_forecast = if plague {
                Some(self.city_forecast(&populated))
            } else {
                production_forecast
            };
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
                self.city_rapture.remove(&removed.id);
                self.rebuild_city_ownership_for_city_event(removed.id);
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
                self.rebuild_city_ownership_for_city_event(city_id);
            }
            if snapshot.owner != BARBARIANS {
                let city = &self.cities[index];
                let income_refreshed =
                    refreshed || growth != Growth::Unchanged || starving_unit.is_some();
                let income = if plague || growth != Growth::Unchanged || starving_unit.is_some() {
                    self.city_forecast(city)
                } else {
                    growth_forecast.unwrap_or(forecast)
                };
                science_income += income.science;
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
        (science_income, gold_paid)
    }

    pub(super) fn finish_player_national_activities(
        &mut self,
        owner: usize,
        income: (i32, Vec<(u32, i32)>),
    ) {
        if owner != BARBARIANS {
            self.balance_unit_gold_upkeep(owner, &income.1);
            self.resolve_research_turn(owner, income.0);
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
            self.specialists[2] += 1; // Native default specialist before growth CM.
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
        self.remove_citizen_assignments(1);
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
            self.remove_citizen_assignments(population);
        }
        Some((self.production, cost))
    }
}

impl Simulation {
    pub(crate) fn city_worker_lifecycle_playtest(&self) -> (bool, bool, bool) {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Mountains;
            tile.resource = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.irrigation = false;
            tile.mine = false;
            tile.owner = None;
        }
        game.players[HUMAN].government = Government::Communism;
        game.players[HUMAN].known = vec![Tech::COMMUNISM];
        let center = (12, 24);
        let grass = offset_native_by_map(center.0, center.1, -1, 0);
        let coal = [
            offset_native_by_map(center.0, center.1, 1, 0),
            offset_native_by_map(center.0, center.1, 0, 1),
        ];
        game.tile_mut(center.0, center.1).terrain = Terrain::Grassland;
        game.tile_mut(grass.0, grass.1).terrain = Terrain::Grassland;
        for (x, y) in coal {
            game.tile_mut(x, y).terrain = Terrain::Hills;
            game.tile_mut(x, y).resource = Some(Resource::Coal);
        }
        game.create_city(HUMAN, center.0, center.1);
        let founded =
            game.cities[0].worked_tiles == vec![grass] && game.cities[0].specialists == [0; 3];
        game.cities[0].food = Self::growth_box(1) - 1;
        game.process_cities();
        let grew = game.cities[0].size == 2
            && coal
                .into_iter()
                .all(|tile| game.cities[0].worked_tiles.contains(&tile))
            && !game.cities[0].worked_tiles.contains(&grass)
            && game.cities[0].specialists == [0; 3];
        let id = game.cities[0].id;
        game.reduce_city_population(0);
        let reduced = game.cities[0].id == id
            && game.cities[0].size == 1
            && game.cities[0].worked_tiles == vec![grass]
            && game.cities[0].specialists == [0; 3];
        (founded, grew, reduced)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_cm_replaces_existing_workers_when_population_weights_change() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.city_worker_lifecycle_playtest(), (true, true, true));
    }

    #[test]
    fn city_events_preserve_other_manual_workers_and_normal_build_completion_does_not_arrange() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = false;
        }
        game.create_city(HUMAN, 12, 24);
        let manual = offset_native_by_map(12, 24, 1, 0);
        game.tile_mut(manual.0, manual.1).terrain = Terrain::Mountains;
        game.cities[0].worked_tiles = vec![manual];
        game.cities[0].specialists = [0; 3];
        game.create_city(HUMAN, 30, 60);
        assert_eq!(game.cities[0].worked_tiles, vec![manual]);
        game.cities[1].size = 2;
        game.cities[1].specialists = [0, 0, 1];
        game.reduce_city_population(1);
        assert_eq!(game.cities[0].worked_tiles, vec![manual]);
        game.players[HUMAN].known.push(Tech::POTTERY);
        game.cities[0].production = Production::Building(Building::GRANARY);
        game.cities[0].shields = 100;
        game.cities[0].food = 10;
        game.process_cities();
        assert!(game.cities[0].has_building(Building::GRANARY));
        assert_eq!(game.cities[0].size, 1);
        assert_eq!(game.cities[0].worked_tiles, vec![manual]);
    }

    #[test]
    fn population_loss_removes_specialists_before_native_city_index_workers() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 12, 24);
        let city = &mut game.cities[0];
        let mut tiles: Vec<_> = Simulation::city_radius(city)
            .filter(|&(dx, dy, _, _)| (dx, dy) != (0, 0))
            .collect();
        tiles.sort_by_key(|&(dx, dy, _, _)| (isohex_vector_distance(dx, dy).pow(2), dx, dy));
        let first = (tiles[0].2, tiles[0].3);
        let second = (tiles[1].2, tiles[1].3);
        city.size = 5;
        city.specialists = [1, 1, 1];
        city.worked_tiles = vec![second, first];
        city.size -= 4;
        city.remove_citizen_assignments(4);
        assert_eq!(city.specialists, [0; 3]);
        assert_eq!(city.worked_tiles, vec![second]);
    }

    #[test]
    fn founding_consumes_the_unit_and_releases_its_guard_and_task_state() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        let founder = game.spawn_unit(HUMAN, UnitKind::Settlers, 10, 24);
        let guard = game.spawn_unit(HUMAN, UnitKind::Phalanx, 10, 24);
        assert!(game.assign_ai_guard(guard, guards::GuardTarget::Unit(founder)));
        game.unit_goto_tiles.insert(founder, (10, 24));
        game.ai_worker_tasks.insert(founder);
        assert!(game.apply(Command::FoundCity(founder)));
        assert!(!game.unit_goto_tiles.contains_key(&founder));
        assert!(!game.ai_worker_tasks.contains(&founder));
        assert!(!game.ai_unit_guards.contains_key(&founder));
        assert!(!game.ai_guard_targets.contains_key(&guard));
        assert!(game.validate_ai_guards());
    }

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
