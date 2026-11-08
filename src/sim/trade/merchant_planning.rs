//! Merchant destination and delivery valuation.

use super::normal_paths::GotoTask;
use super::*;

pub(super) fn worthwhile_marketplace(windfall: f64, cost: i32, surplus: i32, turns: i32) -> bool {
    windfall * 0.95_f64.powi(turns) >= f64::from(cost) * 0.95_f64.powi(-cost / surplus.max(2))
}

#[derive(Clone, Copy)]
pub(super) enum MerchantPolicy {
    National,
    Allied,
    NonWar,
}

pub(super) struct MerchantPlan {
    pub(super) value: f64,
    pub(super) city: u32,
    pub(super) wonder: bool,
    pub(super) approach: (i32, i32),
    pub(super) turns: i32,
}

impl Simulation {
    fn merchant_can_act(&self, unit: &Unit, city: &City) -> bool {
        matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight)
            && unit.moves_left > 0
            && self.map_wrap.civ3_distance(unit.x, unit.y, city.x, city.y) <= 1
    }

    pub(super) fn can_trade(&self, from: &City, to: &City) -> bool {
        from.owner != to.owner && !self.at_war(from.owner, to.owner)
    }

    pub(super) fn home_for_merchant(&self, unit: &Unit) -> Option<&City> {
        self.cities
            .iter()
            .find(|city| Some(city.id) == unit.home_city && city.owner == unit.owner)
    }

    pub(crate) fn can_deliver_goods(&self, unit: &Unit, city: &City) -> bool {
        self.merchant_can_act(unit, city)
            && self
                .home_for_merchant(unit)
                .is_some_and(|home| self.can_trade(home, city))
    }

    pub(crate) fn can_help_wonder(&self, unit: &Unit, city: &City) -> bool {
        self.merchant_can_act(unit, city)
            && self.allied(unit.owner, city.owner)
            && matches!(city.production, Production::Building(building)
                if building.data().genus == Genus::GreatWonder && city.shields < building.data().cost)
    }

    pub(in crate::sim) fn help_wonder(&mut self, unit: u32, city: u32) -> bool {
        let Some(merchant) = self.unit_by_id(unit) else {
            return false;
        };
        let Some(index) = self
            .cities
            .iter()
            .position(|item| item.id == city && self.can_help_wonder(merchant, item))
        else {
            return false;
        };
        let contribution = merchant.kind.stats().cost;
        let city = &mut self.cities[index];
        city.shields += contribution;
        city.caravan_shields += contribution;
        if let Some((_, original_shields)) = &mut city.production_before_switch {
            *original_shields += contribution;
        }
        let name = city.name.clone();
        self.remove_units_with_cargo(vec![unit]);
        self.refresh_visibility();
        self.add_event(format!(
            "A merchant contributed {contribution} shields to the wonder in {name}."
        ));
        true
    }

    pub(super) fn delivery_gold(&self, from: &City, to: &City) -> i32 {
        let base = (self.weighted_trade_distance(from, to) + 10)
            * (self.city_yields(from).2 + self.city_yields(to).2)
            / 24;
        let bonus = if self.has_tech(from.owner, Tech::STEAM_POWER) {
            -415.0
        } else {
            585.0
        };
        (f64::from(base) * 2.0_f64.powf(bonus / 1000.0)).ceil() as i32
    }

    pub(in crate::sim) fn merchant_trade_value(
        &self,
        merchant: &Unit,
        home: &City,
        destination: &City,
        turns: i32,
    ) -> f64 {
        let replacements = self.route_replacements(home, destination);
        let recurring = replacements.map_or(0, |(old_home, old_destination)| {
            let loss = |city: &City, old: Option<u32>| {
                old.and_then(|id| self.city_by_id(id))
                    .map_or(0, |other| self.route_value(city, other))
            };
            let mut gain = self.route_value(home, destination) - loss(home, old_home);
            // caravan.c:one_city_trade_benefit also accounts for the back
            // route lost by one of our other cities at the foreign endpoint.
            if let Some(old) = old_destination.and_then(|id| self.city_by_id(id))
                && old.owner == merchant.owner
            {
                gain -= self.route_value(destination, old);
            }
            gain
        });
        let windfall = f64::from(self.delivery_gold(home, destination));
        if replacements.is_none() {
            let cost = merchant.kind.stats().cost;
            let surplus = self.city_forecast(home).shield_surplus().max(2);
            // A marketplace-only delivery must repay the alternative of
            // producing gold (caravan.c:get_discounted_reward).
            if !worthwhile_marketplace(windfall, cost, surplus, turns) {
                return 0.0;
            }
        }
        windfall + f64::from(recurring) / (1.0 - 0.95)
    }

    pub(in crate::sim) fn merchant_wonder_value(
        &self,
        merchant: &Unit,
        city: &City,
        turns: i32,
    ) -> f64 {
        // caravan.c:wonder_benefit compares rush costs at the arrival stock,
        // including the double price when construction has not started.
        let stock = city.shields + turns * self.city_forecast(city).shield_surplus();
        let rush = |stock: i32| {
            4 * (city.production.cost() - stock).max(0) * if stock == 0 { 2 } else { 1 }
        };
        f64::from(2 * (rush(stock) - rush(stock + merchant.kind.stats().cost)))
    }

    pub(super) fn merchant_land_approach(
        &self,
        merchant: &Unit,
        city: &City,
    ) -> Option<(i32, i32, i32)> {
        let current = (merchant.x, merchant.y);
        if self
            .map_wrap
            .civ3_distance(current.0, current.1, city.x, city.y)
            <= 1
        {
            Some((0, current.0, current.1))
        } else {
            self.map_wrap
                .civ3_neighbors(city.x, city.y)
                .chain([(city.x, city.y)])
                .filter(|&(x, y)| !self.tile(x, y).terrain.is_water())
                .filter_map(|(x, y)| {
                    if self.best_defender_id(merchant, x, y).is_some() {
                        return None;
                    }
                    self.route_cost_within(merchant, x, y, i32::MAX)
                        .map(|cost| (cost, x, y))
                })
                .min()
        }
    }

    pub(super) fn merchant_needs_boat(&self, merchant: &Unit, city: &City) -> bool {
        self.merchant_land_approach(merchant, city).is_none()
    }

    pub(super) fn merchant_tired_of_waiting(&self, merchant: &Unit, city: &City) -> bool {
        let Some(home) = self.home_for_merchant(merchant) else {
            return false;
        };
        if merchant.transport.is_some()
            || !self.same_continent(
                home,
                &City {
                    x: merchant.x,
                    y: merchant.y,
                    ..home.clone()
                },
            )
            || !self.merchant_needs_boat(merchant, city)
        {
            return false;
        }
        let born = self
            .merchant_birth_turns
            .get(&merchant.id)
            .copied()
            .unwrap_or(self.turn);
        let age = self.turn.saturating_sub(born);
        let available = self.ferry_available_count(merchant.owner) > 0;
        age > 20 || (age > 15 && !available)
    }

    pub(in crate::sim) fn prepare_merchant_mission(&mut self, merchant: &Unit) {
        // dai_manage_unit returns before task management for exhausted units.
        if merchant.moves_left <= 0 {
            return;
        }
        if !self.merchant_mission_attempts.insert(merchant.id) {
            return;
        }
        self.merchant_birth_turns
            .entry(merchant.id)
            .or_insert(self.turn);
        let snapshot;
        let merchant = if self.home_for_merchant(merchant).is_none() {
            let labels = self.continent_labels();
            let continent = labels[self.map_wrap.index(merchant.x, merchant.y)];
            let Some(city) = self
                .cities
                .iter()
                .filter(|city| {
                    city.owner == merchant.owner
                        && labels[self.map_wrap.index(city.x, city.y)] == continent
                })
                .min_by_key(|city| {
                    self.map_wrap
                        .civ3_distance(merchant.x, merchant.y, city.x, city.y)
                })
            else {
                return;
            };
            let position = (city.x, city.y);
            self.ai_goto_unit(merchant.id, position, GotoTask::None);
            let Some(index) = self
                .units
                .iter()
                .position(|unit| unit.id == merchant.id && (unit.x, unit.y) == position)
            else {
                return;
            };
            if !self.change_home_city(index) {
                return;
            }
            snapshot = self.units[index].clone();
            &snapshot
        } else {
            merchant
        };
        let mut national_only = false;
        if let Some(mission) = self.ai_merchant_missions.get(&merchant.id).copied() {
            if let Some(city) = self.city_by_id(mission.city) {
                let valid = mission.owner == merchant.owner
                    && self.allied(merchant.owner, city.owner)
                    && if mission.wonder {
                        matches!(city.production, Production::Building(building) if building.data().genus == Genus::GreatWonder)
                            && city.shields < city.production.cost()
                    } else {
                        self.home_for_merchant(merchant).is_some_and(|home| {
                            self.can_trade(home, city)
                                && self.route_replacements(home, city).is_some()
                        })
                    };
                if valid {
                    if !self.merchant_tired_of_waiting(merchant, city) {
                        if merchant.transport.is_some() || self.merchant_needs_boat(merchant, city)
                        {
                            self.request_ai_ferry(merchant.id);
                        } else {
                            self.release_ai_ferry(merchant.id);
                        }
                        return;
                    }
                    national_only = true;
                }
            }
            self.ai_merchant_missions.remove(&merchant.id);
            self.unit_goto_tiles.remove(&merchant.id);
            self.release_ai_ferry(merchant.id);
        }
        // daiunit.c draws this even before a tired passenger forces national-only.
        let random_policy = if self.rand_below(2) != 0 {
            MerchantPolicy::NonWar
        } else {
            MerchantPolicy::Allied
        };
        let policy = if national_only {
            MerchantPolicy::National
        } else {
            random_policy
        };
        let ignore_transit = !national_only
            && self.home_for_merchant(merchant).is_some_and(|home| {
                self.cities.iter().any(|city| {
                    self.allied(merchant.owner, city.owner)
                        && self.can_trade(home, city)
                        && !self.same_continent(home, city)
                })
            });
        if let Some(plan) = self.merchant_plan_for(merchant, 0, policy, ignore_transit, None) {
            if let Some(city) = self.city_by_id(plan.city)
                && self.merchant_needs_boat(merchant, city)
            {
                self.request_ai_ferry(merchant.id);
            }

            if let Some(city) = self.city_by_id(plan.city) {
                self.unit_goto_tiles.insert(merchant.id, (city.x, city.y));
            }
            self.ai_merchant_missions.insert(
                merchant.id,
                MerchantMission {
                    city: plan.city,
                    wonder: plan.wonder,
                    owner: merchant.owner,
                },
            );
        }
    }

    pub(super) fn merchant_plan(&self, merchant: &Unit, ready_in: i32) -> Option<MerchantPlan> {
        let mission = self.ai_merchant_missions.get(&merchant.id).copied()?;
        self.merchant_plan_for(
            merchant,
            ready_in,
            MerchantPolicy::NonWar,
            false,
            Some(mission),
        )
    }

    fn merchant_plan_for(
        &self,
        merchant: &Unit,
        ready_in: i32,
        policy: MerchantPolicy,
        ignore_transit: bool,
        mission: Option<MerchantMission>,
    ) -> Option<MerchantPlan> {
        let home = self.home_for_merchant(merchant);
        let mut best: Option<MerchantPlan> = None;
        let mut cities: Vec<_> = self.cities.iter().collect();
        cities.sort_by_key(|city| (city.owner, city.id));
        for city in cities {
            if mission.is_some_and(|mission| mission.city != city.id)
                || !match policy {
                    MerchantPolicy::National => city.owner == merchant.owner,
                    MerchantPolicy::Allied => self.allied(merchant.owner, city.owner),
                    MerchantPolicy::NonWar => !self.at_war(merchant.owner, city.owner),
                }
            {
                continue;
            }
            let trade = home.filter(|home| {
                self.can_trade(home, city) && !mission.is_some_and(|mission| mission.wonder)
            });
            let wonder = (city.owner == merchant.owner
                || mission.is_some_and(|mission| mission.wonder)
                    && self.allied(merchant.owner, city.owner))
                && (ready_in == 0 || Some(city.id) != merchant.home_city)
                && matches!(city.production,
                Production::Building(building) if building.data().genus == Genus::GreatWonder
                    && city.shields < building.data().cost);
            if trade.is_none() && !wonder {
                continue;
            }
            let approach = if ignore_transit {
                Some((0, city.x, city.y))
            } else {
                self.merchant_land_approach(merchant, city)
            };
            let Some((cost, x, y)) = approach else {
                continue;
            };
            let remaining = merchant.moves_left.max(0);
            let turns = if ignore_transit {
                0
            } else {
                ready_in
                    + if cost < remaining {
                        0
                    } else {
                        1 + (cost - remaining) / self.unit_move_rate(merchant).max(1)
                    }
            };
            let value = if let Some(home) = trade {
                self.merchant_trade_value(merchant, home, city, turns)
            } else {
                self.merchant_wonder_value(merchant, city, turns)
            } * 0.95_f64.powi(turns);
            if (mission.is_some() || value > 0.0)
                && best.as_ref().is_none_or(|plan| {
                    value > plan.value || (value == plan.value && turns < plan.turns)
                })
            {
                best = Some(MerchantPlan {
                    value,
                    city: city.id,
                    wonder,
                    approach: (x, y),
                    turns,
                });
            }
        }
        best
    }
}
