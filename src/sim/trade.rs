use super::{terrain_work::neighbors, *};
use crate::rules::Genus;
use std::collections::BTreeMap;

struct MerchantPlan {
    value: f64,
    city: u32,
    wonder: bool,
    approach: (i32, i32),
}

struct FerryDelivery {
    value: f64,
    ship: u32,
    city: u32,
    approach: (i32, i32),
    landing: Option<(i32, i32)>,
}

impl Simulation {
    fn merchant_can_act(&self, unit: &Unit, city: &City) -> bool {
        matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight)
            && unit.moves_left > 0
            && (unit.y - city.y).abs() <= 2
            && self.map_distance(unit.x, unit.y, city.x, city.y) <= 1
    }

    fn can_trade(&self, from: &City, to: &City) -> bool {
        from.owner != to.owner && !self.at_war(from.owner, to.owner)
    }

    fn home_for_merchant(&self, unit: &Unit) -> Option<&City> {
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

    pub(super) fn help_wonder(&mut self, unit: u32, city: u32) -> bool {
        let Some(merchant) = self.units.iter().find(|item| item.id == unit) else {
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

    fn weighted_trade_distance(&self, a: &City, b: &City) -> i32 {
        self.map_distance(a.x, a.y, b.x, b.y) * 40 / MAP_W.max(MAP_H)
    }

    fn same_continent(&self, a: &City, b: &City) -> bool {
        let mut seen = vec![false; self.tiles.len()];
        let mut queue = vec![(a.x, a.y)];
        seen[Self::idx(a.x, a.y)] = true;
        while let Some((x, y)) = queue.pop() {
            if (x, y) == (b.x, b.y) {
                return true;
            }
            for (nx, ny) in neighbors(x, y) {
                let index = Self::idx(nx, ny);
                if !seen[index] && !self.tiles[index].terrain.is_water() {
                    seen[index] = true;
                    queue.push((nx, ny));
                }
            }
        }
        false
    }

    fn route_value(&self, a: &City, b: &City) -> i32 {
        let value = self.weighted_trade_distance(a, b) + a.size + b.size;
        value * if self.same_continent(a, b) { 1 } else { 2 } / 12
    }

    fn route_partners(&self, city: u32) -> impl Iterator<Item = &City> {
        self.trade_routes.iter().filter_map(move |&(a, b)| {
            let partner = if a == city {
                b
            } else if b == city {
                a
            } else {
                return None;
            };
            self.cities.iter().find(|other| other.id == partner)
        })
    }

    pub(super) fn route_trade(&self, city: &City) -> i32 {
        self.route_partners(city.id)
            .filter(|other| self.can_trade(city, other))
            .map(|other| self.route_value(city, other))
            .sum()
    }

    pub(super) fn trade_illness(&self, city: &City) -> i32 {
        self.route_partners(city.id)
            .filter(|other| self.can_trade(city, other))
            .filter(|other| other.last_plague.is_some_and(|turn| self.turn - turn < 5))
            .map(|other| 0.5 * f64::from(city.size * other.size).sqrt())
            .sum::<f64>() as i32
    }

    fn route_to_replace(&self, city: &City, value: i32) -> Result<Option<u32>, ()> {
        let partners: Vec<_> = self.route_partners(city.id).collect();
        if partners.len() < 2 {
            return Ok(None);
        }
        let weakest = partners
            .iter()
            .map(|other| (self.route_value(city, other), other.id))
            .min();
        match weakest {
            Some((old, id)) if old < value => Ok(Some(id)),
            _ => Err(()),
        }
    }

    fn remove_route(&mut self, a: u32, b: u32) {
        self.trade_routes
            .retain(|&pair| pair != (a, b) && pair != (b, a));
    }

    pub(super) fn prune_trade_routes(&mut self) {
        let mut routes = std::mem::take(&mut self.trade_routes);
        routes.retain(|&(a, b)| {
            let from = self.cities.iter().find(|city| city.id == a);
            let to = self.cities.iter().find(|city| city.id == b);
            from.zip(to)
                .is_some_and(|(from, to)| self.can_trade(from, to))
        });
        self.trade_routes = routes;
    }

    fn route_replacements(&self, from: &City, to: &City) -> Option<(Option<u32>, Option<u32>)> {
        if self
            .route_partners(from.id)
            .any(|partner| partner.id == to.id)
        {
            return None;
        }
        let value = self.route_value(from, to);
        self.route_to_replace(from, value)
            .ok()
            .zip(self.route_to_replace(to, value).ok())
    }

    fn delivery_gold(&self, from: &City, to: &City) -> i32 {
        let base = (self.weighted_trade_distance(from, to) + 10)
            * (self.city_yields(from).2 + self.city_yields(to).2)
            / 24;
        let bonus = if self.has_tech(from.owner, Tech::RAILROAD) {
            -415.0
        } else {
            585.0
        };
        (f64::from(base) * 2.0_f64.powf(bonus / 1000.0)).ceil() as i32
    }

    fn merchant_trade_value(&self, home: &City, destination: &City) -> f64 {
        let recurring = self
            .route_replacements(home, destination)
            .map_or(0, |(removed, _)| {
                let lost = removed
                    .and_then(|id| self.cities.iter().find(|city| city.id == id))
                    .map_or(0, |old| self.route_value(home, old));
                self.route_value(home, destination) - lost
            });
        f64::from(self.delivery_gold(home, destination)) + f64::from(recurring) / 0.05
    }

    fn inland_ferry_approach(
        &self,
        merchant: &Unit,
        ship: &Unit,
        city: &City,
        allowance: i32,
        remaining: i32,
        labels: &[usize],
    ) -> Option<(i32, i32, i32, (i32, i32))> {
        let destination = labels[Self::idx(city.x, city.y)];
        let mut best = None;
        for (index, &label) in labels.iter().enumerate() {
            if label != destination || label == 0 {
                continue;
            }
            let (lx, ly) = Self::xy(index);
            if self.treaty_blocks_move(merchant, lx, ly)
                || self.units.iter().any(|unit| {
                    (unit.x, unit.y) == (lx, ly) && !self.allied(merchant.owner, unit.owner)
                })
            {
                continue;
            }
            let sea_approach = neighbors(lx, ly)
                .filter(|&(x, y)| {
                    self.tile(x, y).terrain.is_water()
                        && !self.units.iter().any(|unit| {
                            (unit.x, unit.y) == (x, y) && !self.allied(ship.owner, unit.owner)
                        })
                })
                .filter_map(|(x, y)| {
                    let cost = self.route_cost_within(ship, x, y, i32::MAX)?;
                    let sea_turns = ((cost - remaining).max(0) + allowance - 1) / allowance;
                    Some((sea_turns, x, y))
                })
                .min();
            let Some((sea_turns, x, y)) = sea_approach else {
                continue;
            };

            let mut walker = merchant.clone();
            (walker.x, walker.y) = (x, y);
            walker.transport = None;
            let foot_allowance = self.unit_move_allowance(walker.owner, walker.kind);
            let mut landing_turn = sea_turns;
            if landing_turn > 0 {
                walker.moves_left = foot_allowance;
            } else if walker.moves_left <= 0 {
                landing_turn += 1;
                walker.moves_left = foot_allowance;
            }
            let Some(landing_cost) = self.movement_cost_for(&walker, lx, ly) else {
                continue;
            };
            walker.moves_left = (walker.moves_left - landing_cost).max(0);
            (walker.x, walker.y) = (lx, ly);
            let Some((foot_cost, _, _)) = self.merchant_land_approach(&walker, city) else {
                continue;
            };
            // Delivery requires a movement fragment after the last walking step.
            let walking_turns = if foot_cost < walker.moves_left {
                0
            } else {
                1 + (foot_cost - walker.moves_left) / foot_allowance
            };
            let candidate = (landing_turn + walking_turns, x, y, (lx, ly));
            if best.is_none_or(|previous| candidate < previous) {
                best = Some(candidate);
            }
        }
        best
    }

    fn ferry_delivery(
        &self,
        merchant: &Unit,
        ready_in: i32,
        reserved: &BTreeMap<u32, u32>,
    ) -> Option<FerryDelivery> {
        let home = self.home_for_merchant(merchant)?;
        let mut best: Option<FerryDelivery> = None;
        let mut labels = None;
        for ship in self.units.iter().filter(|ship| {
            ship.owner == merchant.owner
                && ship.kind.is_naval()
                && ship.kind.can_carry(merchant.kind)
                && !reserved.contains_key(&ship.id)
                && if let Some(id) = merchant.transport {
                    id == ship.id
                } else {
                    self.cargo_count(ship.id) < ship.kind.transport_capacity()
                        && self
                            .units
                            .iter()
                            .all(|cargo| cargo.transport != Some(ship.id))
                }
        }) {
            let mut staged = ship.clone();
            let allowance = self
                .unit_move_allowance(ship.owner, ship.kind)
                .max(MOVE_FRAGMENTS);
            let mut remaining = if ready_in > 0 {
                allowance
            } else {
                ship.moves_left
            };
            let mut pickup_turns = 0;
            staged.moves_left = remaining;
            if (ship.x, ship.y) != (merchant.x, merchant.y) {
                let Some(cost) = self.route_cost_within(&staged, merchant.x, merchant.y, i32::MAX)
                else {
                    continue;
                };
                pickup_turns = ((cost - remaining).max(0) + allowance - 1) / allowance;
                remaining = (remaining + pickup_turns * allowance - cost).max(0);
                (staged.x, staged.y) = (merchant.x, merchant.y);
                staged.moves_left = remaining;
            }
            let mut passenger = merchant.clone();
            if ready_in > 0 || pickup_turns > 0 {
                passenger.moves_left = self.unit_move_allowance(passenger.owner, passenger.kind);
            }
            for city in self.cities.iter().filter(|city| self.can_trade(home, city)) {
                let approach = neighbors(city.x, city.y)
                    .filter(|&(x, y)| {
                        self.tile(x, y).terrain.is_water()
                            && !self.units.iter().any(|unit| {
                                (unit.x, unit.y) == (x, y) && !self.allied(ship.owner, unit.owner)
                            })
                    })
                    .filter_map(|(x, y)| {
                        self.route_cost_within(&staged, x, y, i32::MAX)
                            .map(|cost| (cost, x, y))
                    })
                    .min();
                let (journey, x, y, landing) = if let Some((cost, x, y)) = approach {
                    let sailing_turns = ((cost - remaining).max(0) + allowance - 1) / allowance;
                    let delivery_turn = sailing_turns.max(i32::from(passenger.moves_left <= 0));
                    (delivery_turn, x, y, None)
                } else {
                    let labels = labels.get_or_insert_with(|| self.continent_labels());
                    let Some((turns, x, y, landing)) = self.inland_ferry_approach(
                        &passenger, &staged, city, allowance, remaining, labels,
                    ) else {
                        continue;
                    };
                    (turns, x, y, Some(landing))
                };
                let turns = ready_in + pickup_turns + journey;
                let value = self.merchant_trade_value(home, city) * 0.95_f64.powi(turns);
                if value > 0.0 && best.as_ref().is_none_or(|plan| value > plan.value) {
                    best = Some(FerryDelivery {
                        value,
                        ship: ship.id,
                        city: city.id,
                        landing,
                        approach: (x, y),
                    });
                }
            }
        }
        best
    }

    pub(super) fn load_ai_merchant_ferries(&mut self, owner: usize) -> BTreeMap<u32, u32> {
        let waiting: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == owner
                    && unit.transport.is_none()
                    && matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight)
                    && self
                        .cities
                        .iter()
                        .any(|city| city.owner == owner && (city.x, city.y) == (unit.x, unit.y))
            })
            .cloned()
            .collect();
        let mut dispatch = BTreeMap::new();
        for merchant in waiting {
            if let Some(ferry) = self.ferry_delivery(&merchant, 0, &dispatch)
                && self
                    .merchant_plan(&merchant, 0)
                    .is_none_or(|land| ferry.value > land.value)
            {
                dispatch.insert(ferry.ship, merchant.id);
            }
        }
        dispatch
    }

    pub(super) fn run_merchant_ferry(&mut self, ship: u32, pickup: Option<u32>) -> bool {
        if let Some(passenger) = pickup {
            let Some((x, y)) = self
                .units
                .iter()
                .find(|unit| unit.id == passenger)
                .map(|unit| (unit.x, unit.y))
            else {
                return false;
            };
            if self
                .units
                .iter()
                .any(|unit| unit.id == ship && (unit.x, unit.y) != (x, y))
            {
                self.goto_unit(ship, x, y);
            }
            if !self.board_specific_ship(passenger, ship) {
                return true;
            }
        }
        let Some(merchant) = self
            .units
            .iter()
            .find(|unit| {
                unit.transport == Some(ship)
                    && matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight)
            })
            .cloned()
        else {
            return false;
        };
        if let Some(ferry) = self.ferry_delivery(&merchant, 0, &BTreeMap::new()) {
            let (x, y) = ferry.approach;
            if (merchant.x, merchant.y) != (x, y) {
                self.goto_unit(ship, x, y);
            }
            if let Some((lx, ly)) = ferry.landing {
                let arrived = self
                    .units
                    .iter()
                    .any(|unit| unit.id == ship && (unit.x, unit.y) == (x, y));
                if arrived {
                    let (dx, dy) = wrapped_map_vector(x, y, lx, ly);
                    if self.try_move_unit(merchant.id, dx, dy) == MoveResult::Moved
                        && let Some(walker) = self
                            .units
                            .iter()
                            .find(|unit| unit.id == merchant.id)
                            .cloned()
                        && let Some(city) = self.cities.iter().find(|city| city.id == ferry.city)
                        && let Some((cost, ax, ay)) = self.merchant_land_approach(&walker, city)
                    {
                        if cost > 0 {
                            self.goto_unit(walker.id, ax, ay);
                        }
                        self.deliver_goods(walker.id, ferry.city);
                    }
                }
            } else {
                self.deliver_goods(merchant.id, ferry.city);
            }
        } else if !self.tile(merchant.x, merchant.y).terrain.is_water() {
            self.board_or_deboard(merchant.id);
        }
        true
    }

    fn merchant_land_approach(&self, merchant: &Unit, city: &City) -> Option<(i32, i32, i32)> {
        let current = (merchant.x, merchant.y);
        if self.map_distance(current.0, current.1, city.x, city.y) <= 1
            && (current.1 - city.y).abs() <= 2
        {
            Some((0, current.0, current.1))
        } else {
            neighbors(city.x, city.y)
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

    fn merchant_plan(&self, merchant: &Unit, ready_in: i32) -> Option<MerchantPlan> {
        let home = self.home_for_merchant(merchant);
        let mut best: Option<MerchantPlan> = None;
        for city in &self.cities {
            let trade = home.filter(|home| self.can_trade(home, city));
            let wonder = city.owner == merchant.owner
                && (ready_in == 0 || Some(city.id) != merchant.home_city)
                && matches!(city.production,
                Production::Building(building) if building.data().genus == Genus::GreatWonder
                    && city.shields < building.data().cost);
            if trade.is_none() && !wonder {
                continue;
            }
            let approach = self.merchant_land_approach(merchant, city);
            let Some((cost, x, y)) = approach else {
                continue;
            };
            let remaining = merchant.moves_left.max(0);
            let turns = ready_in
                + if cost < remaining {
                    0
                } else {
                    1 + (cost - remaining) / (merchant.kind.stats().moves * MOVE_FRAGMENTS)
                };
            let value = if let Some(home) = trade {
                self.merchant_trade_value(home, city)
            } else {
                let remaining = city.production.cost()
                    - city.shields
                    - turns * self.city_forecast(city).shield_surplus();
                f64::from(8 * remaining.clamp(0, merchant.kind.stats().cost))
            } * 0.95_f64.powi(turns);
            if value > 0.0 && best.as_ref().is_none_or(|plan| value > plan.value) {
                best = Some(MerchantPlan {
                    value,
                    city: city.id,
                    wonder,
                    approach: (x, y),
                });
            }
        }
        best
    }

    pub(super) fn merchant_production(&self, city: &City) -> Option<UnitKind> {
        if self.units.iter().any(|unit| {
            unit.owner == city.owner
                && unit.home_city == Some(city.id)
                && matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight)
        }) {
            return None;
        }
        let kind = [UnitKind::Freight, UnitKind::Caravan]
            .into_iter()
            .find(|&kind| self.can_produce(city, Production::Unit(kind)))?;
        let surplus = self.city_forecast(city).shield_surplus();
        if surplus <= 0 {
            return None;
        }
        let cost = kind.stats().cost;
        let missing = (cost - city.shields).max(1);
        let turns = (missing + surplus - 1) / surplus;
        let mut merchant = self.make_unit(0, city.owner, kind, city.x, city.y);
        merchant.home_city = Some(city.id);
        let land = self
            .merchant_plan(&merchant, turns)
            .map_or(0.0, |plan| plan.value);
        let sea = self
            .ferry_delivery(&merchant, turns, &BTreeMap::new())
            .map_or(0.0, |plan| plan.value);
        (land.max(sea) >= f64::from(cost)).then_some(kind)
    }

    pub(super) fn run_ai_merchant(&mut self, merchant: &Unit) {
        let Some(MerchantPlan {
            city,
            wonder,
            approach: (x, y),
            ..
        }) = self.merchant_plan(merchant, 0)
        else {
            return;
        };
        if (merchant.x, merchant.y) != (x, y) {
            self.goto_unit(merchant.id, x, y);
        }
        if wonder {
            self.help_wonder(merchant.id, city);
        } else {
            self.deliver_goods(merchant.id, city);
        }
    }

    pub(super) fn deliver_goods(&mut self, unit: u32, city: u32) -> bool {
        self.prune_trade_routes();
        let Some(merchant) = self.units.iter().find(|item| item.id == unit) else {
            return false;
        };
        let Some(to) = self
            .cities
            .iter()
            .find(|item| item.id == city && self.can_deliver_goods(merchant, item))
        else {
            return false;
        };
        let from = self.home_for_merchant(merchant).unwrap();
        let owner = merchant.owner;
        let (a, b) = (from.id, to.id);
        let replacements = self.route_replacements(from, to);
        let gold = self.delivery_gold(from, to);
        let destination = to.name.clone();
        if let Some((old_home, old_dest)) = replacements {
            if let Some(old) = old_home {
                self.remove_route(a, old);
            }
            if let Some(old) = old_dest {
                self.remove_route(b, old);
            }
            self.trade_routes.push((a, b));
        }
        self.players[owner].gold += gold;
        self.remove_units_with_cargo(vec![unit]);
        self.refresh_visibility();
        let outcome = if replacements.is_some() {
            "Trade route established"
        } else {
            "Goods delivered"
        };
        self.add_event(format!("{outcome} in {destination}. Earned {gold} gold."));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ferry_landing_counts_only_the_movement_available_to_the_caravan() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.road = false;
            tile.railroad = false;
        }
        let sea = (10, 24);
        let landing = offset_native_by_map(sea.0, sea.1, 1, 0);
        let destination = offset_native_by_map(sea.0, sea.1, 2, 0);
        game.tile_mut(landing.0, landing.1).terrain = Terrain::Mountains;
        game.tile_mut(destination.0, destination.1).terrain = Terrain::Grassland;
        game.create_city(HUMAN, destination.0, destination.1);
        game.sign_pact(HUMAN, 1, Pact::Ceasefire);
        let mut ship = game.make_unit(0, 1, UnitKind::Transport, sea.0, sea.1);
        ship.moves_left = 0;
        let allowance = game.unit_move_allowance(ship.owner, ship.kind);
        let mut merchant = game.make_unit(0, 1, UnitKind::Caravan, sea.0, sea.1);
        let labels = game.continent_labels();
        let city = &game.cities[0];

        assert_eq!(
            game.inland_ferry_approach(&merchant, &ship, city, allowance, 0, &labels),
            Some((1, sea.0, sea.1, landing))
        );
        merchant.moves_left = 0;
        assert_eq!(
            game.inland_ferry_approach(&merchant, &ship, city, allowance, 0, &labels)
                .map(|(turns, _, _, _)| turns),
            Some(2)
        );
    }

    #[test]
    fn ai_freight_can_help_a_wonder_under_construction() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(1, 10, 20);
        game.cities[0].production = Production::Building(Building::PYRAMIDS);
        let freight = game.spawn_unit(1, UnitKind::Freight, 10, 20);
        let unit = game.units.last().unwrap().clone();
        game.run_ai_merchant(&unit);
        assert_eq!(game.cities[0].shields, 50);
        assert!(game.units.iter().all(|unit| unit.id != freight));
    }

    #[test]
    fn repeat_deliveries_pay_gold_and_plague_spreads_only_over_active_routes() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let (caravan, _, home, foreign) = game.merchant_playtest_setup();
        let owner = game.players[HUMAN].gold;
        let repeat = game
            .units
            .iter()
            .find(|unit| unit.id == caravan)
            .unwrap()
            .clone();
        assert!(game.apply(Command::DeliverGoods(caravan, foreign)));
        assert_eq!(game.trade_routes, [(home, foreign)]);
        assert!(game.players[HUMAN].gold > owner);
        let after_first = game.players[HUMAN].gold;
        let duplicate = game.spawn_unit(HUMAN, UnitKind::Freight, repeat.x, repeat.y);
        game.units.last_mut().unwrap().home_city = Some(home);
        assert!(game.apply(Command::DeliverGoods(duplicate, foreign)));
        assert_eq!(game.trade_routes, [(home, foreign)]);
        assert!(game.players[HUMAN].gold > after_first);

        let home_index = game.cities.iter().position(|city| city.id == home).unwrap();
        let partner = game
            .cities
            .iter_mut()
            .find(|city| city.id == foreign)
            .unwrap();
        partner.last_plague = Some(game.turn);
        game.cities[home_index].size = 9;
        assert_eq!(game.trade_illness(&game.cities[home_index]), 4);
        game.turn += 5;
        assert_eq!(game.trade_illness(&game.cities[home_index]), 0);
        assert!(game.cancel_pact(HUMAN, 1));
        assert!(game.trade_routes.is_empty());
    }
}
