//! City production selection, progress, and completion rules.

use super::*;

impl Simulation {
    pub(in crate::sim) fn change_production(&mut self, city_id: u32, target: Production) -> bool {
        let Some(index) = self.city_index_by_id(city_id) else {
            return false;
        };
        if !self.can_change_production(&self.cities[index], target) {
            return false;
        }
        let city = &mut self.cities[index];
        let (original, shields) = *city
            .production_before_switch
            .get_or_insert((city.production, city.shields));
        let regular = shields;
        let protected = city.protected_shields.min(regular);
        let mut unpenalized = protected;
        let mut penalized = 0;
        if original.class() == target.class() {
            unpenalized += regular - protected;
        } else {
            penalized += regular - protected;
        }
        city.shields = unpenalized + penalized / 2;
        city.production = target;
        true
    }

    pub(in crate::sim) fn choose_ai_production(&self, owner: usize, city_id: u32) -> Production {
        let Some(city) = self.city_by_id(city_id) else {
            return Production::Unit(UnitKind::Warrior);
        };
        if self.players[owner].ai {
            if let Some(choice) = self.ai_build_choice(city) {
                return choice.production;
            }
        } else {
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
        Production::Unit(UnitKind::Warrior)
    }

    pub(in crate::sim) fn process_cities(&mut self) {
        for owner in 0..self.players.len() {
            let income = self.process_player_cities(owner);
            self.finish_player_national_activities(owner, income);
        }
    }

    pub(in crate::sim) fn process_player_cities(&mut self, owner: usize) -> (i32, Vec<(u32, i32)>) {
        self.resolve_resistance_for_owner(owner);
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
            if let Some(index) = self.city_index_by_id(id) {
                self.assimilate_citizen(index);
            }
        }
        while !city_ids.is_empty() {
            let selected = self.rand_below(city_ids.len() as i32) as usize;
            let city_id = city_ids.swap_remove(selected);
            let Some(index) = self.city_index_by_id(city_id) else {
                continue;
            };
            let city = &self.cities[index];
            if !city.bought_production {
                self.cities[index].production = self.worklist_replacement(city, city.production);
            }
            let snapshot = self.cities[index].clone();
            let forecast = self.city_forecast(&snapshot);
            let shield_surplus = forecast.shield_surplus();
            let initial_gold = self.city_gold_upkeep(&snapshot);
            let can_produce = match snapshot.production {
                Production::Unit(kind) => self.unit_buildable(&snapshot, kind),
                target => self.can_produce(&snapshot, target),
            };
            let city = &mut self.cities[index];
            city.production_before_switch = None;
            city.protected_shields = 0;
            let surplus = if forecast.disorder { 0 } else { shield_surplus };
            city.shields += surplus;
            let finished = if can_produce {
                city.resolve_production(self.map_wrap)
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
                        unit.hp = unit.max_hit_points();
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
            self.record_city_culture(index);
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
                || forecast.food_surplus(populated.size, 0),
                |updated| updated.food_surplus(populated.size, 0),
            );
            let city = &mut self.cities[index];
            let growth = city.resolve_growth(
                self.map_wrap,
                food_surplus,
                Self::city_size_limit(&populated),
                Self::food_retained(&populated),
                false,
            );
            if growth == Growth::Grew
                && populated.has_building(Building::LONGEVITY)
                && self.cities[index].size < Self::city_size_limit(&populated)
            {
                let city = &mut self.cities[index];
                city.size += 1;
                city.specialists[2] += 1;
            }
            if growth == Growth::Starved {
                self.remove_resisters_first_nationalities(index, 1);
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
                let income_refreshed = refreshed || growth != Growth::Unchanged;
                let income = if plague || growth != Growth::Unchanged {
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

    pub(in crate::sim) fn finish_player_national_activities(
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
pub(super) enum Growth {
    Unchanged,
    Grew,
    Starved,
    Destroyed,
}

impl City {
    pub(super) fn resolve_growth(
        &mut self,
        wrap: MapWrap,
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
        self.remove_citizen_assignments(1, wrap);
        self.food = percent_of(Simulation::growth_box(self.size), retained);
        Growth::Starved
    }

    pub(super) fn resolve_production(&mut self, wrap: MapWrap) -> Option<(Production, i32)> {
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
            self.remove_citizen_assignments(population, wrap);
        }
        Some((self.production, cost))
    }
}
