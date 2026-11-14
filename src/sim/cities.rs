//! City lifecycle and turn processing, including production and population changes.

pub(super) mod observations;

use super::*;

mod production;
use crate::rules::Genus;
use crate::rules::freeciv::{RulesetData, effect_value};

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

    pub(super) fn city_name(&self, owner: usize) -> String {
        if let Some(civilization_id) = self.players[owner].civilization_id.as_deref()
            && let Some(name) =
                crate::rules::civilizations::next_city_name(civilization_id, |name| {
                    self.cities
                        .iter()
                        .any(|city| city.name.eq_ignore_ascii_case(name))
                })
        {
            return name;
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
        self.trace.emit(
            self.turn,
            || {
                vec![
                    trace::Entity::Unit(unit_id),
                    trace::Entity::City(self.next_city_id - 1),
                    trace::Entity::Civilization(unit.owner),
                ]
            },
            || crate::sim::settlement::observations::CityFounded {
                x: unit.x,
                y: unit.y,
                name: name.clone(),
            },
        );
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
        }
        self.cities.push(city);
        self.list_order.city(self.cities.last().unwrap());
        let tile = self.tile_mut(x, y);
        tile.base = None;
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
            && city.size + unit.kind.stats().pop_cost <= self.city_size_limit(city)
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
        let retained = self.growth_food_retained(&self.cities[index]);
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
        self.apply_civilian_casualties(attacker, x, y, "Attack");
    }

    pub(super) fn apply_bombard_population_loss(&mut self, attacker: &Unit, x: i32, y: i32) {
        self.apply_civilian_casualties(attacker, x, y, "Bombard");
    }

    fn apply_civilian_casualties(&mut self, attacker: &Unit, x: i32, y: i32, action: &str) {
        let class = crate::sim::movement_rules::MovementRules::campaign()
            .unit_class_movement_data_for_kind(attacker.kind)
            .expect("campaign C2C3 unit class mapping is complete");
        if !crate::sim::combat_rules::selected().killcitizen || !class.kills_citizens {
            return;
        }
        let Some(index) = self
            .cities
            .iter()
            .position(|city| (city.x, city.y) == (x, y))
        else {
            return;
        };
        let city = self.cities[index].clone();
        let context = super::action_rules::SimulationRequirementContext::for_city_at(
            self,
            &city,
            Some((x, y)),
        )
        .with_unit_preserving_owner(attacker)
        .with_action(action);
        if effect_value(
            &RulesetData::campaign_default().effects,
            "Unit_No_Lose_Pop",
            &context,
        )
        .expect("campaign C2C3 Unit_No_Lose_Pop requirements are supported")
            > 0
        {
            return;
        }

        let event = (attacker.owner == HUMAN || city.owner == HUMAN)
            .then(|| format!("{} lost one citizen during {action}.", city.name));
        if city.size <= 1 {
            self.remove_city_for_population_loss(city.id);
        } else {
            self.reduce_city_population(index);
        }
        if let Some(event) = event {
            self.add_event(event);
        }
    }

    pub(super) fn remove_city_for_population_loss(&mut self, city_id: u32) {
        let Some(index) = self.city_index_by_id(city_id) else {
            return;
        };
        self.release_city_support(city_id);
        let removed = self.cities.remove(index);
        for unit in self
            .units
            .iter_mut()
            .filter(|unit| unit.home_city == Some(city_id))
        {
            unit.home_city = None;
        }
        if removed.has_building(Building::PALACE) {
            self.lose_spaceship(removed.owner);
        }
        self.city_founders.remove(&removed.id);
        self.city_rapture.remove(&removed.id);
        self.rebuild_city_ownership_for_city_event(removed.id);
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

    pub(super) fn city_conquest_action_for_unit(
        &self,
        unit: &Unit,
        city: &City,
    ) -> Option<&'static str> {
        ["Conquer City Shrink", "Conquer City Shrink 2"]
            .into_iter()
            .find(
                |action| match self.action_available_for_unit_against_city(unit, action, city) {
                    Ok(available) => available,
                    Err(error) => {
                        debug_assert!(false, "could not evaluate C2C3 action {action}: {error:?}");
                        false
                    }
                },
            )
    }

    pub(super) fn capture_city_if_needed(&mut self, unit_id: u32, x: i32, y: i32) {
        let Some(unit) = self.unit_by_id(unit_id).cloned() else {
            return;
        };
        // Movement calls this only after a selected Conquer City action was
        // enabled against the destination while the actor was still on its
        // source tile. The forced action's Tile requirement cannot be
        // re-evaluated after the actor has entered the city center.
        if unit.transport.is_some() {
            return;
        }
        let Some(city_index) = self.cities.iter().position(|city| {
            city.x == x
                && city.y == y
                && city.owner != unit.owner
                && self.at_war(city.owner, unit.owner)
        }) else {
            return;
        };
        let old_owner = self.cities[city_index].owner;
        let city_id = self.cities[city_index].id;
        let city_name = self.cities[city_index].name.clone();
        let city_size = self.cities[city_index].size;
        let had_palace = self.cities[city_index].has_building(Building::PALACE);
        if had_palace {
            self.lose_spaceship(old_owner);
        }
        // citytools.c checks and rolls for a capital-triggered civil war
        // before size-one destruction, loot, and conquest-technology RNG.
        let civil_war = had_palace && self.capital_loss_triggers_civil_war(old_owner);

        // citytools.c destroys a size-one city before transferring it or
        // awarding its gold. The same path also clears its supported units.
        if city_size <= 1 {
            self.remove_city_for_population_loss(city_id);
            if civil_war {
                self.split_civil_war(old_owner);
            }
            self.refresh_visibility();
            if unit.owner == HUMAN || old_owner == HUMAN {
                self.add_event(format!(
                    "{} destroyed {city_name}.",
                    self.players[unit.owner].name
                ));
            }
            return;
        }

        let defender_gold = self.players[old_owner].gold.max(0);
        let loot = (self.rand_below(defender_gold / 20 + 1)
            + defender_gold.saturating_mul(city_size) / 200)
            .min(defender_gold);
        self.players[unit.owner].gold = self.players[unit.owner].gold.saturating_add(loot);
        self.players[old_owner].gold -= loot;

        // citytools.c tries the conquest technology effect after loot and
        // before transferring the city. The selected C2C3 effect is 100%.
        let city_for_effect = self.cities[city_index].clone();
        let conquest_tech_pct = effect_value(
            &RulesetData::campaign_default().effects,
            "Conquest_Tech_Pct",
            &super::action_rules::SimulationRequirementContext::for_city_at(
                self,
                &city_for_effect,
                Some((x, y)),
            )
            .with_unit_preserving_owner(&unit),
        )
        .expect("campaign C2C3 Conquest_Tech_Pct requirements are supported");
        if i64::from(self.rand_below(100)) < conquest_tech_pct {
            self.steal_conquest_technology(unit.owner, old_owner);
        }

        let first_city_for_taker = !self.cities.iter().any(|city| city.owner == unit.owner);
        let center = (x, y);

        // City history remains with the city and is scored for its new owner.
        // Freeciv applies the selected conquest_convert_pct after transfer;
        // C2C3 sets that value to zero.
        self.cities[city_index].transfer_citizens(unit.owner);
        self.convert_conquest_citizens(city_index);
        self.cities[city_index]
            .buildings
            .retain(|building| building.data().genus != Genus::SmallWonder);
        self.list_order.city(&self.cities[city_index]);

        self.transfer_conquered_city_units(city_id, old_owner, unit.owner, center);
        self.resolve_city_transfer_stacks(old_owner, unit.owner);

        // transfer_city() razes ordinary improvements after removing small
        // wonders. Great wonders survive; C2C3 supplies razechance = 10.
        let raze_chance = (crate::sim::combat_rules::selected().razechance
            + i32::from(unit.owner == BARBARIANS) * 30)
            .clamp(0, 100);
        let mut buildings = std::mem::take(&mut self.cities[city_index].buildings);
        buildings.sort_by_key(|building| building.0);
        let mut kept_buildings = Vec::with_capacity(buildings.len());
        for building in buildings {
            if building.data().genus == Genus::GreatWonder
                || (building.data().genus != Genus::SmallWonder
                    && self.rand_below(100) >= raze_chance)
            {
                kept_buildings.push(building);
            }
        }
        self.cities[city_index].buildings = kept_buildings;
        // Palace has the selected SaveSmallWonder flag in C2C3. Other
        // small-wonder save flags are not present in this catalog.
        if first_city_for_taker && unit.owner != BARBARIANS {
            self.cities[city_index].buildings.push(Building::PALACE);
        }
        if had_palace && crate::sim::combat_rules::selected().savepalace {
            let old_city_ids = self
                .player_cities_ordered(old_owner)
                .map(|city| city.id)
                .collect::<Vec<_>>();
            if !old_city_ids.is_empty() {
                let chosen_city = old_city_ids[self.rand_below(old_city_ids.len() as i32) as usize];
                if let Some(city) = self.cities.iter_mut().find(|city| city.id == chosen_city)
                    && !city.has_building(Building::PALACE)
                {
                    city.buildings.push(Building::PALACE);
                }
            }
        }

        self.cities[city_index].shields = 0;
        self.cities[city_index].production_before_switch = None;
        self.cities[city_index].protected_shields = 0;
        let city_id = self.cities[city_index].id;
        self.reduce_city_population(city_index);
        let Some(city_index) = self.city_index_by_id(city_id) else {
            return;
        };
        if !self.can_produce(&self.cities[city_index], self.cities[city_index].production) {
            self.cities[city_index].production = self.choose_ai_production(unit.owner, city_id);
        }
        self.rebuild_city_ownership_for_city_event(city_id);
        if civil_war {
            self.split_civil_war(old_owner);
        }
        // The inherited default.lua city_transferred callback runs after the
        // city population loss and any resulting civil war. It is only emitted
        // for a city that survived the transfer.
        if let Some(index) = self.city_index_by_id(city_id) {
            self.inspire_partisans(index, old_owner);
        }
        self.refresh_diplomatic_contact_at(unit.owner, x, y);
        self.refresh_visibility();
        if unit.owner == HUMAN || old_owner == HUMAN {
            self.add_event(format!(
                "{} captured {city_name} from {} and looted {loot} gold.",
                self.players[unit.owner].name, self.players[old_owner].name,
            ));
        }
    }

    fn transfer_conquered_city_units(
        &mut self,
        city_id: u32,
        old_owner: usize,
        new_owner: usize,
        center: (i32, i32),
    ) {
        let ids = self.city_transfer_unit_ids(city_id, old_owner);
        let supported_ids = self
            .units
            .iter()
            .filter(|unit| unit.owner == old_owner && unit.home_city == Some(city_id))
            .map(|unit| unit.id)
            .collect::<std::collections::HashSet<_>>();
        let center_ids = self
            .tile_units_ordered(center.0, center.1)
            .map(|unit| unit.id)
            .collect::<Vec<_>>();
        let rehome_center_ids = center_ids
            .iter()
            .filter_map(|id| {
                self.unit_by_id(*id).and_then(|unit| {
                    (unit.owner == old_owner
                        && (unit.home_city.is_some() || supported_ids.contains(id)))
                    .then_some(*id)
                })
            })
            .collect::<std::collections::HashSet<_>>();

        // transfer_city() first detaches the captured city's support list.
        // This leaves a Unique GameLoss unit that bounces from the center
        // homeless, instead of letting the later release pass destroy it.
        for unit in self
            .units
            .iter_mut()
            .filter(|unit| supported_ids.contains(&unit.id))
        {
            unit.home_city = None;
        }

        // citytools.c::transfer_city_units first handles every unit standing
        // on the transferred city's center, including units whose home city
        // is elsewhere and units with no home city.
        for id in &center_ids {
            let Some(unit) = self.unit_by_id(*id).cloned() else {
                continue;
            };
            if unit.owner == old_owner {
                let rehome = rehome_center_ids.contains(id);
                self.transfer_city_center_unit(&unit, new_owner, city_id, rehome);
            } else if unit.owner != new_owner && !self.allied(new_owner, unit.owner) {
                // transfer_city_units resolves third-party units that are not
                // allied to the new city owner through Freeciv's bounce path.
                self.bounce_unit_freeciv(unit.id);
            }
        }

        let mut lost = Vec::new();
        for id in ids {
            // The center stack was resolved above. A GameLoss unique unit may
            // have bounced and retained its old ID, so skip all original
            // center IDs even when they still exist.
            if center_ids.contains(&id) {
                continue;
            }
            let Some(unit) = self.unit_by_id(id).cloned() else {
                continue;
            };
            if let Some(home) = self.cities.iter().find(|city| {
                city.id != city_id
                    && city.owner == old_owner
                    && (city.x, city.y) == (unit.x, unit.y)
            }) {
                if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) {
                    unit.home_city = Some(home.id);
                    unit.home_order = self.next_unit_id;
                    self.next_unit_id += 1;
                }
            } else {
                lost.push(id);
            }
        }
        if !lost.is_empty() {
            self.remove_units_with_cargo(lost);
        }
        self.release_city_support_except(city_id, Some(new_owner));
    }

    fn transfer_city_center_unit(
        &mut self,
        unit: &Unit,
        new_owner: usize,
        city_id: u32,
        rehome: bool,
    ) {
        let unique = crate::sim::action_rules::freeciv_unit_type_flag(unit.kind, "Unique")
            .expect("campaign C2C3 city-transfer Unique flag is explicitly mapped");
        let unique_conflict = unique
            && self.units.iter().any(|other| {
                other.id != unit.id && other.owner == new_owner && other.kind == unit.kind
            });
        if unique_conflict {
            let game_loss = crate::sim::action_rules::freeciv_unit_type_flag(unit.kind, "GameLoss")
                .expect("campaign C2C3 city-transfer GameLoss flag is explicitly mapped");
            if game_loss {
                self.bounce_unit_freeciv(unit.id);
            } else {
                self.remove_units_with_cargo(vec![unit.id]);
            }
            return;
        }

        if let Some(transferred) = self
            .units
            .iter_mut()
            .find(|candidate| candidate.id == unit.id)
        {
            // citytools.c::transfer_unit routes city ownership changes through
            // unit_change_homecity_handling(), which preserves unit identity.
            transferred.owner = new_owner;
            transferred.home_city = rehome.then_some(city_id);
            if rehome {
                transferred.home_order = self.next_unit_id;
                self.next_unit_id += 1;
            }
            self.list_order.unit(transferred);
        }
    }

    fn resolve_city_transfer_stacks(&mut self, giver: usize, taker: usize) {
        // Freeciv citytools.c::transfer_city calls resolve_unit_stacks after
        // transfer_city_units: illegal city occupants are bounced first,
        // then mixed stacks are resolved once for each participant.
        self.throw_units_from_nonallied_cities(giver);
        self.throw_units_from_nonallied_cities(taker);
        self.resolve_city_transfer_stack_conflicts(giver, taker);
        self.resolve_city_transfer_stack_conflicts(taker, giver);
    }

    fn throw_units_from_nonallied_cities(&mut self, owner: usize) {
        // Match unittools.c::throw_units_from_illegal_cities' unload pass:
        // cargo that is not allied to the carrier owner is unloaded when it
        // can physically exist on the city tile.
        let carrier_ids = self
            .player_units_ordered(owner)
            .filter(|carrier| {
                self.cargo_count(carrier.id) > 0
                    && self
                        .city_at(carrier.x, carrier.y)
                        .is_some_and(|city| !self.allied(owner, city.owner))
            })
            .map(|carrier| carrier.id)
            .collect::<Vec<_>>();
        for carrier_id in carrier_ids {
            let Some(carrier) = self.unit_by_id(carrier_id).cloned() else {
                continue;
            };
            let cargo_ids = self
                .cargo_units_ordered(carrier_id)
                .map(|cargo| cargo.id)
                .collect::<Vec<_>>();
            for cargo_id in cargo_ids {
                let Some(cargo) = self.unit_by_id(cargo_id).cloned() else {
                    continue;
                };
                if !self.allied(owner, cargo.owner)
                    && self.unit_can_survive_at_tile(&cargo, carrier.x, carrier.y)
                    && let Some(index) = self.unit_index_by_id(cargo_id)
                {
                    self.set_transport(index, None);
                }
            }
        }

        // Then bounce the owner's units from cities they no longer have
        // access to. Cargo on an owner-controlled carrier moves with it.
        let unit_ids = self
            .player_units_ordered(owner)
            .map(|unit| unit.id)
            .collect::<Vec<_>>();
        for id in unit_ids {
            let Some(unit) = self.unit_by_id(id).cloned() else {
                continue;
            };
            let Some(city) = self.city_at(unit.x, unit.y) else {
                continue;
            };
            if self.allied(owner, city.owner) {
                continue;
            }
            let carried_by_own_unit = unit.transport.is_some_and(|carrier_id| {
                self.unit_by_id(carrier_id)
                    .is_some_and(|carrier| carrier.owner == owner)
            });
            if !carried_by_own_unit {
                self.bounce_unit_freeciv(id);
            }
        }
    }

    fn resolve_city_transfer_stack_conflicts(&mut self, owner: usize, other: usize) {
        let unit_ids = self
            .player_units_ordered(owner)
            .map(|unit| unit.id)
            .collect::<Vec<_>>();
        for id in unit_ids {
            let Some(unit) = self.unit_by_id(id).cloned() else {
                continue;
            };
            let (x, y) = (unit.x, unit.y);
            let has_nonallied_unit = self
                .tile_units_ordered(x, y)
                .any(|occupant| self.unit_is_nonallied_to(occupant, owner));
            if !has_nonallied_unit {
                continue;
            }

            // Freeciv bounces both transfer participants from mixed stacks,
            // plus any third-party unit that cannot survive at the tile.
            let occupants = self
                .tile_units_ordered(x, y)
                .map(|occupant| occupant.id)
                .collect::<Vec<_>>();
            for occupant_id in occupants {
                let Some(occupant) = self.unit_by_id(occupant_id).cloned() else {
                    continue;
                };
                if occupant.owner == owner
                    || occupant.owner == other
                    || !self.unit_can_survive_at_tile(&occupant, x, y)
                {
                    self.bounce_unit_freeciv(occupant_id);
                }
            }
        }
    }

    fn steal_conquest_technology(&mut self, taker: usize, victim: usize) {
        let settings = *super::research::campaign_research_settings();
        let eligible = Tech::all()
            .filter(|&tech| {
                tech.is_civ3_advance()
                    && self.has_tech(victim, tech)
                    && !self.has_tech(taker, tech)
                    && (settings.tech_steal_allow_holes || self.tech_available(taker, tech))
            })
            .collect::<Vec<_>>();
        let stolen = if eligible.is_empty() {
            (self.players[victim].future_techs > self.players[taker].future_techs)
                .then_some(Tech::FUTURE)
        } else {
            Some(eligible[self.rand_below(eligible.len() as i32) as usize])
        };
        let Some(tech) = stolen else {
            return;
        };

        if settings.tech_lost_donor_percent > 0
            && (settings.tech_trade_loss_allow_holes
                || self.players[victim]
                    .known
                    .iter()
                    .all(|&known| !known.civ3_prerequisites().unwrap_or(&[]).contains(&tech)))
            && self.rand_below(100) < settings.tech_lost_donor_percent
        {
            if tech == Tech::FUTURE {
                self.players[victim].future_techs -= 1;
            } else {
                self.players[victim].known.retain(|&known| known != tech);
                self.handle_government_tech_loss(victim, tech);
            }
        }

        if !self.technology_transfer_succeeds(settings.tech_lost_recv_percent) {
            return;
        }

        let was_researching = self.players[taker].researching == tech;
        let free_bulbs = self.players[taker].research;
        // Freeciv's engine default GAME_DEFAULT_CONQUERCOST is 0 for this
        // pinned ruleset; it is not a C2C3 ruleset field.
        self.apply_research_penalty(taker, tech, 0);
        self.discover(taker, tech);
        if was_researching {
            let next = self.choose_research_target(taker);
            self.choose_research_tech(taker, next, free_bulbs);
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
        self.c2c3_city_tile_yields(city, x, y, is_center, celebrating, granted, government)
    }

    pub(super) fn city_yields_with_tech(
        &self,
        city: &City,
        granted: Option<Tech>,
    ) -> (i32, i32, i32) {
        let (food, shields, mut trade) = self.city_gross_yields_with_tech(city, granted);
        trade += self.trade_route_income(city, granted);
        self.c2c3_finalize_city_outputs(
            city,
            (food, shields, trade),
            self.players[city.owner].government,
            granted,
        )
    }

    /// Worked-tile output before city-wide output effects and trade routes.
    /// Freeciv's Simple route-revenue setting uses this citizen-base trade.
    pub(super) fn city_gross_yields_with_tech(
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
        (food, shields, trade)
    }

    pub(crate) fn growth_box(size: i32) -> i32 {
        if size <= 0 {
            return 0;
        }
        let settings = crate::sim::economy::ruleset::CityEconomySettings::campaign();
        let initial_count = settings.granary_food_initial.len() as i32;
        let base = if size <= initial_count {
            settings.granary_food_initial[(size - 1) as usize]
        } else {
            settings.granary_food_initial[settings.granary_food_initial.len() - 1]
                + settings.granary_food_increment * (size - initial_count)
        };
        percent_of(base, settings.foodbox_percent).max(1)
    }
}

#[cfg(test)]
#[path = "cities/tests.rs"]
mod tests;
