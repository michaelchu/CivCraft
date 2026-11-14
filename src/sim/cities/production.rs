//! City production selection, progress, and completion rules.

use super::*;
use crate::rules::freeciv::UnitUpkeepOutput;
use crate::sim::economy::{
    CityForecast,
    ruleset::{CityEconomySettings, GoldUpkeepStyle},
};
use crate::sim::upkeep::UnitUpkeepMapping;

impl Simulation {
    fn resolve_city_shield_upkeep(&mut self, city_id: u32, forecast: &CityForecast) -> Option<i32> {
        let _city_index = self.city_index_by_id(city_id)?;
        let shield_output = if forecast.disorder {
            0
        } else {
            forecast.shields
        };
        let mut surplus = shield_output - forecast.support.shields;
        if surplus >= 0 {
            return Some(surplus);
        }

        // city_distribute_surplus_shields() first offers each unit with a
        // positive per-unit cost after free upkeep the selected ruleset's
        // ordered auto actions. Removing a unit refreshes every remaining
        // unit's free-upkeep allocation before the deficit check continues.
        for row in &forecast.support.per_unit {
            if surplus >= 0 {
                break;
            }
            let city_index = self.city_index_by_id(city_id)?;
            let current_upkeep = self.city_unit_upkeep(&self.cities[city_index]);
            let Some(current_row) = current_upkeep
                .per_unit
                .iter()
                .find(|candidate| candidate.unit_id == row.unit_id)
            else {
                continue;
            };
            let UnitUpkeepMapping::Mapped { net, .. } = current_row.economics else {
                continue;
            };
            if net.shields <= 0 || self.unit_by_id(row.unit_id).is_none() {
                continue;
            }
            if self.resolve_missing_unit_upkeep(row.unit_id, UnitUpkeepOutput::Shield) {
                let index = self.city_index_by_id(city_id)?;
                surplus = shield_output - self.city_unit_upkeep(&self.cities[index]).shields;
            }
        }

        let index = self.city_index_by_id(city_id)?;
        let remaining = self.city_unit_upkeep(&self.cities[index]);
        let mut protected_population_losses = 0;
        for row in remaining.per_unit {
            if surplus >= 0 {
                break;
            }
            let UnitUpkeepMapping::Mapped { net, .. } = row.economics else {
                continue;
            };
            if net.shields > 0 && self.unit_by_id(row.unit_id).is_some() {
                // C2C3 has shield_wipe=FALSE. Units that survive their listed
                // actions are waived for this turn and each costs one citizen.
                surplus += net.shields;
                protected_population_losses += 1;
            }
        }

        if protected_population_losses > 0 {
            let index = self.city_index_by_id(city_id)?;
            let city = &self.cities[index];
            if city.size <= protected_population_losses {
                let owner = city.owner;
                let name = city.name.clone();
                self.remove_city_after_upkeep_population_loss(city_id);
                if owner == HUMAN {
                    self.add_event(format!("Failure to support units destroyed {name}."));
                }
                return None;
            }
            for _ in 0..protected_population_losses {
                let index = self.city_index_by_id(city_id)?;
                self.reduce_city_population(index);
            }
            if let Some(index) = self.city_index_by_id(city_id)
                && self.cities[index].owner == HUMAN
            {
                self.add_event(format!(
                    "Citizens in {} perished because the city could not support its units.",
                    self.cities[index].name
                ));
            }
        }
        Some(surplus)
    }

    fn first_food_upkeep_unit(&self, city_id: u32) -> Option<u32> {
        let city = self.city_by_id(city_id)?;
        self.city_unit_upkeep(city)
            .per_unit
            .into_iter()
            .find_map(|row| match row.economics {
                UnitUpkeepMapping::Mapped { net, .. } if net.food > 0 => Some(row.unit_id),
                UnitUpkeepMapping::Mapped { .. } | UnitUpkeepMapping::Unsupported(_) => None,
            })
    }

    fn city_rapture_growth_due(&self, city: &City, food_surplus: i32) -> bool {
        // `rapturedelay` is a Freeciv server setting; use its pinned engine
        // default. The selected C2C3 ruleset has no Rapture_Grow effect, but
        // resolving the effect here keeps the population/upkeep ordering
        // correct if that ruleset is intentionally adjusted later.
        const RAPTURE_DELAY: i32 = 1;
        let rapture = self.city_rapture.get(&city.id).copied().unwrap_or(0);
        if rapture <= 0 || food_surplus <= 0 || rapture % RAPTURE_DELAY != 0 {
            return false;
        }
        let government = self.players[city.owner].government;
        let celebrating = self.base_city_celebrating(city) && self.city_forecast(city).happy;
        self.city_rule_effect(
            city,
            "Rapture_Grow",
            None,
            None,
            None,
            celebrating,
            government,
            None,
        ) > 0
    }

    fn remove_city_after_upkeep_population_loss(&mut self, city_id: u32) {
        let Some(index) = self.city_index_by_id(city_id) else {
            return;
        };
        self.release_city_support(city_id);
        if self.cities[index].has_building(Building::PALACE) {
            self.lose_spaceship(self.cities[index].owner);
        }
        let removed = self.cities.remove(index);
        self.city_founders.remove(&removed.id);
        self.city_rapture.remove(&removed.id);
        self.rebuild_city_ownership_for_city_event(removed.id);
    }

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
        self.reconcile_trade_routes();
        self.establish_connected_trade_routes();
        for owner in 0..self.players.len() {
            // Match end_turn: unused civilization slots have no active
            // research target and must not receive national activities.
            if !self.player_alive(owner) || (owner < BARBARIANS && owner >= self.civilization_count)
            {
                continue;
            }
            let income = self.process_player_cities(owner);
            self.finish_player_national_activities(owner, income);
        }
    }

    pub(in crate::sim) fn process_player_cities(&mut self, owner: usize) -> (i32, Vec<(u32, i32)>) {
        let mut city_ids: Vec<u32> = self
            .player_cities_ordered(owner)
            .map(|city| city.id)
            .collect();
        // cityturn.c:update_city_activities converts citizens in stable city
        // list order before canceling routes and randomizing city activities.
        for &city_id in &city_ids {
            if let Some(index) = self.city_index_by_id(city_id) {
                self.convert_city_citizen_for_turn(index);
            }
        }
        self.reconcile_trade_routes();
        self.reconcile_city_workers();
        let mut science_income = 0;
        let mut gold_paid = Vec::new();
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
            let before_shield_upkeep = self.cities[index].clone();
            let before_shield_forecast = self.city_forecast(&before_shield_upkeep);
            let Some(shield_surplus) =
                self.resolve_city_shield_upkeep(city_id, &before_shield_forecast)
            else {
                continue;
            };
            let Some(index) = self.city_index_by_id(city_id) else {
                continue;
            };
            let snapshot = self.cities[index].clone();
            let forecast = self.city_forecast(&snapshot);
            let shield_refreshed = snapshot.size != before_shield_upkeep.size
                || self.city_unit_upkeep(&snapshot).per_unit.len()
                    != before_shield_forecast.support.per_unit.len();
            let initial_gold = self.city_gold_upkeep(&snapshot);
            let can_produce = match snapshot.production {
                Production::Unit(kind) => self.unit_buildable(&snapshot, kind),
                target => self.can_produce(&snapshot, target),
            };
            let city = &mut self.cities[index];
            city.production_before_switch = None;
            city.protected_shields = 0;
            city.shields += shield_surplus;
            let surplus = shield_surplus.max(0);
            let finished = if can_produce {
                city.resolve_production(self.map_wrap)
            } else {
                None
            };
            if finished.is_some() {
                city.protected_shields = surplus.min(city.shields);
            }
            if let Some((target, shields)) = finished {
                self.trace.emit(
                    self.turn,
                    || {
                        vec![
                            trace::Entity::City(city_id),
                            trace::Entity::Civilization(snapshot.owner),
                        ]
                    },
                    || crate::sim::cities::observations::ProductionCompleted {
                        item: target.name().into(),
                        shields_before: snapshot.shields,
                        shields_after: self.cities[index].shields,
                    },
                );
                match target {
                    Production::Unit(kind) => {
                        self.nation_scores[snapshot.owner].units_built += 1;
                        let nationality =
                            self.remove_citizen_nationalities(index, kind.stats().pop_cost);
                        let rank = self.trained_rank(&snapshot, kind);
                        let c2c3_veteran = self.trained_c2c3_veteran(&snapshot, kind);
                        let id = self.spawn_unit(snapshot.owner, kind, snapshot.x, snapshot.y);
                        let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
                        unit.nationality = nationality;
                        unit.home_city = Some(city_id);
                        unit.home_order = id;
                        unit.veteran = rank;
                        unit.c2c3_veteran = c2c3_veteran;
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
            self.record_city_history(index);
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
            let refreshed = shield_refreshed || refreshed || plague;
            let growth_forecast = if plague {
                Some(self.city_forecast(&populated))
            } else {
                production_forecast
            };
            let food_surplus = growth_forecast.as_ref().map_or_else(
                || forecast.food_surplus(populated.size, forecast.support.food),
                |updated| updated.food_surplus(populated.size, updated.support.food),
            );
            let rapture_growth = self.city_rapture_growth_due(&populated, food_surplus);
            let size_limit = self.city_size_limit(&populated);
            let growth_retained = self.growth_food_retained(&populated);
            let shrink_retained = self.shrink_food_retained(&populated);
            let food_upkeep_unit = (!rapture_growth && self.cities[index].food + food_surplus < 0)
                .then(|| self.first_food_upkeep_unit(city_id))
                .flatten();
            let food_unit_removed = food_upkeep_unit.is_some_and(|unit_id| {
                self.resolve_missing_unit_upkeep(unit_id, UnitUpkeepOutput::Food)
            });
            let city = &mut self.cities[index];
            let food_before = city.food;
            let population_before = city.size;
            let growth = city.resolve_growth(
                self.map_wrap,
                food_surplus,
                size_limit,
                growth_retained,
                shrink_retained,
                food_upkeep_unit.is_some(),
                rapture_growth,
            );
            self.trace.emit(
                self.turn,
                || {
                    vec![
                        trace::Entity::City(city_id),
                        trace::Entity::Civilization(populated.owner),
                    ]
                },
                || crate::sim::cities::observations::GrowthApplied {
                    food_before,
                    food_after: self.cities[index].food,
                    population_before,
                    population_after: self.cities[index].size,
                    applied_surplus: food_surplus,
                    size_limit,
                    outcome: match growth {
                        Growth::Starved => "starved",
                        Growth::Destroyed => "destroyed",
                        Growth::Grew => "grew",
                        Growth::Unchanged => "unchanged",
                    },
                },
            );
            if growth == Growth::Starved {
                self.remove_citizen_nationalities(index, 1);
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
                let income_refreshed = refreshed
                    || growth != Growth::Unchanged
                    || matches!(finished, Some((Production::Unit(_), _)));
                let income = if plague || growth != Growth::Unchanged {
                    self.city_forecast(city)
                } else {
                    growth_forecast.unwrap_or(forecast)
                };
                science_income += income.science;
                let unit_gold = if income_refreshed || food_unit_removed {
                    self.city_gold_upkeep(city)
                } else {
                    initial_gold
                };
                let unit_gold_cost = unit_gold.paid.iter().map(|(_, cost)| cost).sum::<i32>();
                let local_unit_upkeep = match CityEconomySettings::campaign().gold_upkeep_style {
                    GoldUpkeepStyle::City => unit_gold_cost,
                    GoldUpkeepStyle::Mixed | GoldUpkeepStyle::Nation => 0,
                };
                self.players[snapshot.owner].gold += income.gold_surplus(local_unit_upkeep);
                if CityEconomySettings::campaign().gold_upkeep_style == GoldUpkeepStyle::Mixed {
                    // cityturn.c subtracts each city's supported-unit costs
                    // before checking whether that city must sell buildings.
                    // National unit disbanding still waits until every city
                    // has been processed.
                    self.players[snapshot.owner].gold -= unit_gold_cost;
                }
                gold_paid.extend(unit_gold.paid);
                if CityEconomySettings::campaign().gold_upkeep_style != GoldUpkeepStyle::Nation {
                    self.balance_building_upkeep(snapshot.owner, city_id);
                }
                let city = self.cities[index].clone();
                self.check_city_pollution(&city, income.shields);
            }
            self.update_city_disorder_revolution(city_id);
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
            let style = CityEconomySettings::campaign().gold_upkeep_style;
            let mut paid = income.1.clone();
            if style != GoldUpkeepStyle::City {
                let homeless = self.homeless_unit_gold_upkeep(owner);
                self.players[owner].gold -= homeless.paid.iter().map(|(_, cost)| cost).sum::<i32>();
                paid.extend(homeless.paid);
            }
            if style == GoldUpkeepStyle::Nation {
                self.balance_national_building_upkeep(owner);
            }
            self.record_player_history(owner);
            if style == GoldUpkeepStyle::Nation {
                self.players[owner].gold -= income.1.iter().map(|(_, cost)| cost).sum::<i32>();
            }
            self.balance_unit_gold_upkeep(owner, &paid);
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
    #[expect(
        clippy::too_many_arguments,
        reason = "Growth resolution takes the independently computed ruleset effects for this city."
    )]
    pub(super) fn resolve_growth(
        &mut self,
        wrap: MapWrap,
        surplus: i32,
        size_limit: i32,
        growth_retained: i32,
        shrink_retained: i32,
        starving_unit: bool,
        rapture_growth: bool,
    ) -> Growth {
        self.food += surplus;
        let capacity = Simulation::growth_box(self.size);
        if self.food >= capacity || rapture_growth {
            if self.size >= size_limit {
                self.food = capacity;
                return Growth::Unchanged;
            }
            let previous_food = self.food;
            let overflow = (self.food - capacity).max(0);
            self.size += 1;
            self.specialists[2] += 1; // Native default specialist before growth CM.
            let new_capacity = Simulation::growth_box(self.size);
            self.food = if rapture_growth {
                // city_increase_size() retains the old food stock on rapture,
                // capped only by the new granary capacity.
                previous_food.min(new_capacity)
            } else {
                previous_food.min(overflow + percent_of(new_capacity, growth_retained))
            };
            return Growth::Grew;
        }
        if self.food >= 0 {
            return Growth::Unchanged;
        }
        if starving_unit {
            self.food = percent_of(capacity, shrink_retained);
            return Growth::Unchanged;
        }
        if self.size == 1 {
            return Growth::Destroyed;
        }
        self.size -= 1;
        self.remove_citizen_assignments(1, wrap);
        self.food = percent_of(Simulation::growth_box(self.size), shrink_retained);
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

#[cfg(test)]
mod upkeep_tests {
    use super::*;
    use crate::sim::action_rules::{UnitEconomicsRecordBinding, freeciv_unit_economics_record};

    fn flat_game() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.owner = None;
        }
        game.create_city(HUMAN, 12, 24);
        game
    }

    fn add_supported_unit(game: &mut Simulation, city_id: u32, kind: UnitKind) -> u32 {
        let city = game.city_by_id(city_id).unwrap();
        let id = game.spawn_unit(HUMAN, kind, city.x, city.y);
        let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
        unit.home_city = Some(city_id);
        unit.home_order = id;
        id
    }

    #[test]
    fn city_processing_skips_unused_player_slots_with_unset_research() {
        let mut game = flat_game();
        let inactive = game.civilization_count;
        assert_eq!(game.players[inactive].researching, Tech::UNSET);
        let before = bincode::serialize(&game.players[inactive]).unwrap();
        game.cities[0].production = Production::Unit(UnitKind::Warrior);
        game.cities[0].shields = game.cities[0].production.cost();
        let city_id = game.cities[0].id;

        game.process_cities();

        assert_eq!(bincode::serialize(&game.players[inactive]).unwrap(), before);
        assert!(game.units.iter().any(|unit| {
            unit.owner == HUMAN && unit.kind == UnitKind::Warrior && unit.home_city == Some(city_id)
        }));
    }

    #[test]
    fn shield_deficit_uses_net_cost_and_ordered_recover_action() {
        let mut game = flat_game();
        game.players[HUMAN].government = Government::Republic;
        let city_id = game.cities[0].id;
        let older = add_supported_unit(&mut game, city_id, UnitKind::Warrior);
        let newer = add_supported_unit(&mut game, city_id, UnitKind::Warrior);
        let mut forecast = game.city_forecast(&game.cities[0]);
        assert_eq!(forecast.support.shields, 1);
        forecast.shields = 0;
        let unit = game.unit_by_id(older).unwrap().clone();
        let UnitEconomicsRecordBinding::Mapped { record, .. } =
            freeciv_unit_economics_record(UnitKind::Warrior)
        else {
            panic!("Warrior has a selected C2C3 economics record");
        };
        let recover_value =
            game.unit_shield_value_for_action(&unit, record, "Disband Unit Recover");

        assert_eq!(game.resolve_city_shield_upkeep(city_id, &forecast), Some(0));
        assert!(game.unit_by_id(older).is_none());
        assert!(game.unit_by_id(newer).is_some());
        assert_eq!(game.cities[0].shields, recover_value);
        assert!(recover_value > 0);
    }

    #[test]
    fn newly_built_unit_uses_free_gold_support_before_same_turn_charging_excess_units() {
        let mut game = flat_game();
        let city_id = game.cities[0].id;
        let already_supported = [
            add_supported_unit(&mut game, city_id, UnitKind::Warrior),
            add_supported_unit(&mut game, city_id, UnitKind::Warrior),
        ];
        game.cities[0].production = Production::Unit(UnitKind::Warrior);
        game.cities[0].shields = game.cities[0].production.cost();

        let (_, paid) = game.process_player_cities(HUMAN);
        let built = game
            .units
            .iter()
            .find(|unit| {
                unit.home_city == Some(city_id)
                    && unit.kind == UnitKind::Warrior
                    && !already_supported.contains(&unit.id)
            })
            .expect("city completed its Warrior");
        let built_row = game
            .city_unit_upkeep(&game.cities[0])
            .per_unit
            .into_iter()
            .find(|row| row.unit_id == built.id)
            .expect("new unit is included in the same-turn upkeep refresh");
        assert!(matches!(
            built_row.economics,
            UnitUpkeepMapping::Mapped {
                raw: crate::sim::upkeep::UnitOutputCosts { gold: 1, .. },
                net: crate::sim::upkeep::UnitOutputCosts { gold: 0, .. },
            }
        ));
        assert!(
            paid.iter()
                .any(|&(unit_id, cost)| { already_supported.contains(&unit_id) && cost > 0 })
        );
        assert!(paid.iter().all(|&(unit_id, _)| unit_id != built.id));
    }

    #[test]
    fn food_deficit_applies_ruleset_wipe_to_first_supported_food_unit() {
        let mut game = flat_game();
        let city_id = game.cities[0].id;
        let settlers = (0..5)
            .map(|_| add_supported_unit(&mut game, city_id, UnitKind::Settler))
            .collect::<Vec<_>>();
        let settler = settlers[0];
        assert_eq!(game.first_food_upkeep_unit(city_id), Some(settler));

        assert!(game.resolve_missing_unit_upkeep(settler, UnitUpkeepOutput::Food));

        assert!(game.unit_by_id(settler).is_none());
        assert_eq!(game.cities[0].size, 1);
    }

    #[test]
    fn food_surplus_includes_net_supported_unit_cost() {
        let mut game = flat_game();
        let city_id = game.cities[0].id;
        for _ in 0..6 {
            add_supported_unit(&mut game, city_id, UnitKind::Settler);
        }
        let forecast = game.city_forecast(&game.cities[0]);
        assert_eq!(forecast.support.food, 2);
        let expected = forecast.food
            - game.cities[0].size * CityEconomySettings::campaign().food_cost
            - forecast.support.food;

        assert_eq!(
            forecast.food_surplus(game.cities[0].size, forecast.support.food),
            expected
        );
    }

    #[test]
    fn city_turn_disbands_food_upkeep_before_famine_reduces_population() {
        let mut game = flat_game();
        let city_id = game.cities[0].id;
        for _ in 0..5 {
            add_supported_unit(&mut game, city_id, UnitKind::Settler);
        }
        game.cities[0].food = -20;
        let supported_before = game
            .units
            .iter()
            .filter(|unit| unit.home_city == Some(city_id))
            .count();
        assert!(game.first_food_upkeep_unit(city_id).is_some());

        game.process_player_cities(HUMAN);

        assert_eq!(game.cities[0].size, 1);
        assert_eq!(
            game.units
                .iter()
                .filter(|unit| unit.home_city == Some(city_id))
                .count(),
            supported_before - 1
        );
        assert!(game.cities[0].food >= 0);
    }
}
