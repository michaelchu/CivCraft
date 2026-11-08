//! Advisor evaluation of spending, reserves, and gold priorities.

use super::*;

impl Simulation {
    pub(crate) fn ai_gold_reserve(&self, owner: usize) -> i32 {
        let citizens = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| city.size)
            .sum::<i32>();
        (2 * citizens).max(self.ai_purchase_reserve.get(&owner).copied().unwrap_or(0))
    }

    fn record_ai_purchase_cost(&mut self, owner: usize, price: i32) {
        let highest = self.ai_purchase_reserve.entry(owner).or_default();
        *highest = (*highest).max(price);
    }

    pub(crate) fn spend_ai_defense_gold(&mut self, owner: usize) {
        let owned: Vec<_> = self.player_cities_ordered(owner).collect();
        let expenses: i32 = owned
            .iter()
            .map(|city| self.building_upkeep(city) + self.city_unit_upkeep(city).gold)
            .sum();
        let reserve = self.ai_gold_reserve(owner);
        let mut priorities: Vec<_> = owned
            .into_iter()
            .filter_map(|city| {
                let chosen = self.ai_build_choice(city)?;
                // daicity.c:dai_spend_gold consumes the FINAL city choice.
                // A lower defense-only bid must never replace a selected
                // hunter, ammunition, wonder or domestic improvement here.
                if self.city_defense_production(city) != Some(chosen.production)
                    || city.production != chosen.production
                {
                    return None;
                }
                let immediate = self
                    .city_threat_arrivals(city)
                    .filter(|&(_, _, turns)| turns <= 1)
                    .map(|(_, rating, _)| rating)
                    .sum::<i32>();
                (immediate > 0).then_some((chosen.want, city.id, chosen.production))
            })
            .collect();
        // Stable ties retain the native player-city list order.
        priorities.sort_by(|a, b| b.0.total_cmp(&a.0));
        self.ai_purchase_reserve.remove(&owner);
        let emergency_cities: Vec<_> = priorities.iter().map(|&(_, city, _)| city).collect();
        for (_, id, target) in priorities {
            let city = self.city_by_id(id).unwrap();
            let position = (city.x, city.y);
            self.spend_ai_upgrade_gold(owner, Some(position), expenses);
            let city = self.city_by_id(id).unwrap();
            // Upgrading a garrison may change speculative new advice. Native
            // spending still uses its cached choice from city management.
            if city.production != target {
                continue;
            }
            let city = self.city_by_id(id).unwrap();
            if city.shields + self.city_forecast(city).shield_surplus() >= city.production.cost() {
                continue;
            }
            let Some(price) = self.rush_cost(city) else {
                continue;
            };
            let undefended = !self.units.iter().any(|unit| {
                unit.owner == owner
                    && unit.transport.is_none()
                    && unit.kind.is_military()
                    && (unit.x, unit.y) == position
            });
            let invested = city.shields > 0;
            if undefended
                && matches!(target, Production::Unit(_))
                && self.players[owner].gold - price < expenses
            {
                let sale = Building::all().find(|&building| {
                    !matches!(
                        building,
                        Building::CITY_WALLS
                            | Building::COASTAL_DEFENSE
                            | Building::SAM_BATTERY
                            | Building::SDI_DEFENSE
                    ) && Self::building_sale_price(city, building).is_some()
                });
                if let Some(building) = sale {
                    self.sell_building(owner, id, building);
                }
            }
            let remaining = self.players[owner].gold - price;
            if remaining >= expenses && (undefended || (invested && remaining >= 2 * reserve)) {
                self.rush_production(owner, id);
            } else if undefended && matches!(target, Production::Unit(_)) {
                self.record_ai_purchase_cost(owner, price);
            }
        }
        let at_war = self.ai_on_war_footing(owner);
        self.spend_ai_routine_gold(owner, reserve, expenses, at_war, &emergency_cities);
        if !at_war {
            self.spend_ai_upgrade_gold(owner, None, reserve.max(expenses));
        }
    }

    fn spend_ai_routine_gold(
        &mut self,
        owner: usize,
        reserve: i32,
        expenses: i32,
        at_war: bool,
        emergency_cities: &[u32],
    ) {
        let city_count = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .count();
        let gold = self.players[owner].gold;
        let mut purchases = Vec::new();
        for city in self.cities.iter().filter(|city| {
            city.owner == owner && city.shields > 0 && !emergency_cities.contains(&city.id)
        }) {
            let Some(price) = self.rush_cost(city) else {
                continue;
            };
            let mut cash_floor = 2 * reserve;
            if let Production::Unit(kind) = city.production {
                let stats = kind.stats();
                if city.size <= stats.pop_cost {
                    continue;
                }
                if stats.can_found {
                    if at_war || city_count > 6 {
                        continue;
                    }
                    cash_floor = reserve;
                } else if !at_war
                    && price > 2 * stats.cost
                    && kind.is_military()
                    && (stats.attack > stats.defense
                        || self.offensive_production(city) == Some(kind))
                {
                    continue;
                }
            }
            let cash_floor = cash_floor.max(expenses);
            if gold - price < cash_floor {
                continue;
            }

            let missing = city.production.cost() - city.shields;
            let surplus = self.city_forecast(city).shield_surplus();
            if missing <= surplus {
                continue;
            }
            let want = self
                .ai_build_choice(city)
                .filter(|choice| choice.production == city.production)
                .map_or(0.0, |choice| choice.want);
            purchases.push((want, price, city.id, cash_floor));
        }
        purchases.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        for (_, price, city, cash_floor) in purchases {
            if self.players[owner].gold - price >= cash_floor {
                self.rush_production(owner, city);
            }
        }
    }

    fn spend_ai_upgrade_gold(
        &mut self,
        owner: usize,
        defense_city: Option<(i32, i32)>,
        limit: i32,
    ) {
        let upgrades: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == owner
                    && match defense_city {
                        Some(position) => unit.kind.is_military() && (unit.x, unit.y) == position,
                        None => !unit.kind.is_military(),
                    }
            })
            .filter_map(|unit| {
                self.paid_upgrade(unit)
                    .map(|(target, price)| (unit.id, target, price))
            })
            .collect();
        for (unit, target, price) in upgrades {
            if self.players[owner].gold - price > limit {
                self.upgrade_unit(unit, target, price);
            } else {
                self.record_ai_purchase_cost(owner, price);
            }
        }
    }
}
