//! City lifecycle and turn processing, including production and population changes.

use super::*;

mod production;
use crate::rules::Genus;

impl Simulation {
    // Toroidal fixture convenience; gameplay uses the campaign-aware methods below.
    pub(crate) fn city_radius(city: &City) -> impl Iterator<Item = (i32, i32, i32, i32)> + use<> {
        Self::city_radius_at(city.x, city.y)
    }

    pub(super) fn city_radius_at(x: i32, y: i32) -> impl Iterator<Item = (i32, i32, i32, i32)> {
        MapWrap::X.city_radius(x, y)
    }

    pub(crate) fn map_city_radius(
        &self,
        city: &City,
    ) -> impl Iterator<Item = (i32, i32, i32, i32)> + use<> {
        self.map_city_radius_at(city.x, city.y)
    }
    pub(super) fn map_city_radius_at(
        &self,
        x: i32,
        y: i32,
    ) -> impl Iterator<Item = (i32, i32, i32, i32)> + use<> {
        self.map_wrap.city_radius(x, y)
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
            && self.has_tech(owner, Tech::SANITATION)
            && (tile.terrain != Terrain::Desert
                || tile.river
                || tile.resource == Some(Resource::Oasis))
            && self.has_irrigation_source(owner, x, y)
    }

    fn has_irrigation_source(&self, owner: usize, x: i32, y: i32) -> bool {
        self.has_tech(owner, Tech::ELECTRICITY)
            || self.tile(x, y).river
            || self.map_wrap.civ3_neighbors(x, y).any(|(nx, ny)| {
                let source = self.tile(nx, ny);
                source.terrain.is_water() || source.river || source.irrigation
            })
    }

    pub(super) fn city_name(&self, owner: usize) -> String {
        if let Some(civilization_id) = self.players[owner].civilization_id.as_deref() {
            if let Some(name) =
                crate::rules::civilizations::next_city_name(civilization_id, |name| {
                    self.cities
                        .iter()
                        .any(|city| city.name.eq_ignore_ascii_case(name))
                })
            {
                return name;
            }
        }
        // Rebels/barbarians retain their own identity, without borrowing a nation.
        for number in 1.. {
            let name = format!("{} {number}", self.players[owner].name);
            if !self
                .cities
                .iter()
                .any(|city| city.name.eq_ignore_ascii_case(&name))
            {
                return name;
            }
        }
        unreachable!()
    }

    pub(super) fn found_city(&mut self, unit_id: u32) -> bool {
        let Some(index) = self.unit_index_by_id(unit_id) else {
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
        self.publish_visual_event(VisualEvent::FoundCity { unit: unit.clone() });
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
        let mut city = City::new(id, owner, x, y, name.clone(), self.turn);
        city.specialists = [0, 0, 1];
        if first_city && owner != BARBARIANS {
            city.buildings.push(Building::PALACE);
            city.record_culture_building_completion(Building::PALACE, self.year);
        }
        self.cities.push(city);
        self.list_order.city(self.cities.last().unwrap());
        let railroad = self.has_tech(owner, Tech::STEAM_POWER);
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
        unit.kind == UnitKind::Settler
            && unit.owner == city.owner
            && unit.moves_left > 0
            && self.map_wrap.civ3_distance(unit.x, unit.y, city.x, city.y) <= 1
            && city.size + unit.kind.stats().pop_cost <= Self::city_size_limit(city)
    }

    pub(super) fn join_city(&mut self, unit_id: u32, city_id: u32) -> bool {
        let Some(unit) = self.unit_by_id(unit_id).cloned() else {
            return false;
        };
        let Some(index) = self
            .cities
            .iter()
            .position(|city| city.id == city_id && self.can_join_city(&unit, city))
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
        city.remove_citizen_assignments(1, self.map_wrap);
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
            self.cities[index].record_culture_building_completion(Building::PALACE, self.year);
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
        self.rebuild_city_ownership_for_city_event(id);
        self.refresh_diplomatic_contact_at(buyer, center.0, center.1);
        self.refresh_visibility();
    }

    fn bounce_traded_city_unit(&mut self, id: u32) {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return;
        };
        let mut destinations = Vec::new();
        for (x, y, distance) in self.map_wrap.civ3_radius(unit.x, unit.y, 2) {
            if distance == 0 {
                continue;
            }
            if self.movement_cost_for(&unit, x, y).is_some()
                && (!unit.kind.is_aircraft() || self.refuel_site(unit.owner, x, y))
                && self
                    .cities
                    .iter()
                    .all(|city| (city.x, city.y) != (x, y) || self.allied(unit.owner, city.owner))
                && self.units.iter().all(|other| {
                    (other.x, other.y) != (x, y) || self.allied(unit.owner, other.owner)
                })
            {
                destinations.push((x, y));
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
        let Some(unit) = self.unit_by_id(unit_id).cloned() else {
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
        if self.cities[city_index].size > 1 {
            self.cities[city_index].size -= 1;
            self.remove_citizen_nationalities(city_index, 1);
            self.cities[city_index].remove_citizen_assignments(1, self.map_wrap);
        }
        self.release_city_support(self.cities[city_index].id);
        self.cities[city_index].food = 0;
        self.cities[city_index].shields = 0;
        self.cities[city_index].production_before_switch = None;
        self.cities[city_index].protected_shields = 0;
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
            capital.record_culture_building_completion(Building::PALACE, self.year);
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
        let tile_penalty = matches!(government, Government::Despotism | Government::Anarchy)
            && granted != Some(Tech::STEAM_POWER)
            && !self.has_tech(city.owner, Tech::STEAM_POWER)
            && !self.has_active_wonder(city.owner, Building::PYRAMIDS)
            && self.city_rapture.get(&city.id).copied().unwrap_or(0) <= 0;
        let republic_trade = matches!(government, Government::Republic | Government::Democracy);
        let celebrate_trade = celebrating
            && matches!(
                government,
                Government::Monarchy
                    | Government::Feudalism
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
            center.railroad |= granted == Some(Tech::STEAM_POWER);
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
                && matches!(tile.terrain, Terrain::Coast | Terrain::Sea | Terrain::Ocean),
        );
        shields += i32::from(is_center)
            + i32::from(
                city.has_building(Building::OFFSHORE_PLATFORM)
                    && tile.terrain.is_water()
                    && (tile.terrain != Terrain::Ocean || tile.oil_platform),
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

    pub(super) fn hitpoint_gain(unit: &Unit, in_city_center: bool) -> i32 {
        if unit.moved {
            return 0;
        }
        let base = unit.max_hit_points();
        let minimum_percent = if in_city_center {
            CITY_MIN_HP_PERCENT
        } else {
            0
        };
        // hp_gain_coord first takes the greater of HP_Regen and Min_HP_Pct,
        // then adds HP_Regen_2. Conquests fortresses affect defense and ZOC;
        // they do not grant a separate healing bonus.
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
}

#[cfg(test)]
#[path = "cities/tests.rs"]
mod tests;
