use super::*;
use crate::rules::{Genus, Requirement};

const FOOD_WEIGHT: i32 = 30;
const SHIELD_WEIGHT: i32 = 17;
const TRADE_WEIGHT: i32 = 18;
const ADVISOR_POWER_DIVIDER: i32 = POWER_FACTOR * 3;

impl Simulation {
    pub(super) fn advisor_tech_want(&self, owner: usize, tech: Tech) -> i32 {
        if !self.players[owner].ai || self.has_tech(owner, tech) {
            return 0;
        }
        (if tech == Tech::ALPHABET {
            self.counterintelligence_tech_want(owner)
        } else {
            0
        }) + self.ferry_tech_want(owner, tech)
            + self
                .cities
                .iter()
                .filter(|city| city.owner == owner)
                .map(|city| self.city_tech_want(city, tech) + self.military_tech_want(city, tech))
                .sum::<i32>()
    }

    fn advisor_city_output(&self, city: &City, granted: Option<Tech>) -> i32 {
        let forecast = self.city_forecast_with_tech(city, granted);
        let food = forecast.food;
        let shields = forecast.shields;
        let science = forecast.gross_science;
        let luxury = forecast.luxury;
        let tax = self.players[city.owner].rates[1];
        let gold = forecast.gross_gold - forecast.building_upkeep;
        let wants_science = self.players[city.owner].researching != Tech::FUTURE;
        let (science_weight, luxury_weight) = if wants_science {
            (21.6, 1)
        } else {
            (1.0, TRADE_WEIGHT)
        };
        let gold_weight = if tax > 50 {
            f64::from(TRADE_WEIGHT * (tax - 40)) / 14.0
        } else {
            f64::from(TRADE_WEIGHT)
        };
        FOOD_WEIGHT * food
            + SHIELD_WEIGHT * shields
            + luxury_weight * luxury
            + (f64::from(science) * science_weight + f64::from(gold) * gold_weight) as i32
    }

    fn city_tech_want(&self, city: &City, tech: Tech) -> i32 {
        let baseline = self.advisor_city_output(city, None);
        let owner = city.owner;
        let direct = match tech {
            Tech::RAILROAD | Tech::TRADE | Tech::THE_CORPORATION => {
                self.advisor_city_output(city, Some(tech)) - baseline
            }
            Tech::MYSTICISM
                if city.has_building(Building::TEMPLE)
                    && self.players[owner].government != Government::Communism =>
            {
                TRADE_WEIGHT
            }
            _ => 0,
        };
        let (food, gross_shields, _) = self.city_yields(city);
        let upkeep = self.city_unit_upkeep(city);
        let food_surplus = food - city.size * CITIZEN_FOOD_COST - upkeep.food;
        let shields = (gross_shields - upkeep.shields).max(0);
        let mut want = direct * 14 / 8;
        for &building in &city.buildings {
            if building
                .data()
                .obsolete
                .contains(&(Requirement::Tech(tech.0), true))
            {
                let mut obsolete = city.clone();
                obsolete.buildings.retain(|&built| built != building);
                want += (self.advisor_city_output(&obsolete, Some(tech))
                    - self.advisor_city_output(city, Some(tech)))
                    * 14
                    / 8;
            }
        }
        for building in
            Building::all().filter(|building| building.data().genus == Genus::Improvement)
        {
            let data = building.data();
            if !data
                .requirements
                .contains(&(Requirement::Tech(tech.0), true))
                || self
                    .building_blocker_with_tech(city, building, Some(tech))
                    .is_some()
            {
                continue;
            }
            let mut improved = city.clone();
            improved.buildings.push(building);
            let mut benefit = self.advisor_city_output(&improved, None) - baseline;
            benefit += self.building_growth_value(city, &improved, food_surplus);
            if building == Building::CITY_WALLS {
                benefit += self
                    .city_threats(city)
                    .map(|(_, danger)| danger)
                    .sum::<i32>();
            }
            let cost = data.cost / (shields * 10 + 1);
            want += (benefit - cost).max(0) * 14 / 8;
        }
        want
    }

    fn building_growth_value(&self, city: &City, improved: &City, food_surplus: i32) -> i32 {
        let mut benefit = 0;
        // Growth headroom matters when a city is about to reach its current limit.
        let limit = Self::city_size_limit(city);
        let new_limit = Self::city_size_limit(improved);
        if new_limit > limit && limit != i32::MAX {
            let increase = (new_limit - limit).min(8);
            benefit += increase * 4 / limit;
            if city.size >= limit {
                let stored = if city.food == Self::growth_box(city.size) {
                    city.food - Self::growth_box(city.size - 1)
                } else {
                    0
                };
                benefit += (food_surplus + stored) * (FOOD_WEIGHT * increase + 30);
            }
        }
        let retention = Self::food_retained(improved) - Self::food_retained(city);
        if retention > 0 {
            benefit +=
                8 + retention / 10 * food_surplus.max(0) + retention / 15 * (-food_surplus).max(1);
        }
        benefit
    }

    pub(super) fn domestic_production(&self, city: &City) -> Option<Building> {
        let forecast = self.city_forecast(city);
        let shields = (forecast.shields - forecast.support.shields).max(1);
        let food = forecast.food_surplus(city.size, forecast.support.food);
        let baseline = self.government_city_value(city);
        Building::all()
            .filter(|building| {
                building.data().genus == Genus::Improvement
                    && self.building_blocker(city, *building).is_none()
            })
            .filter_map(|building| {
                let mut improved = city.clone();
                improved.buildings.push(building);
                let gain = self.government_city_value(&improved) - baseline
                    + self.building_growth_value(city, &improved, food);
                let target = Production::Building(building);
                let stock = if city.production.class() == target.class() {
                    city.shields
                } else {
                    city.protected_shields + (city.shields - city.protected_shields).max(0) / 2
                };
                let delay = ((target.cost() - stock).max(0) + shields - 1) / shields;
                let value = f64::from(gain) * (23.0_f64 / 24.0).powi(delay);
                (value > 0.0).then_some((building, value))
            })
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(building, _)| building)
    }

    pub(super) fn city_threats<'a>(
        &'a self,
        city: &'a City,
    ) -> impl Iterator<Item = (&'a Unit, i32)> + 'a {
        self.city_threat_arrivals(city)
            .map(|(unit, rating, _)| (unit, rating))
    }

    pub(super) fn city_threat_arrivals<'a>(
        &'a self,
        city: &'a City,
    ) -> impl Iterator<Item = (&'a Unit, i32, i32)> + 'a {
        self.units.iter().filter_map(|unit| {
            let stats = unit.kind.stats();
            if !unit.kind.is_military()
                || unit.transport.is_some()
                || !self.at_war(city.owner, unit.owner)
                || !self.tile(unit.x, unit.y).known_by[city.owner]
            {
                return None;
            }
            let allowance = self.unit_move_allowance(unit.owner, unit.kind).max(1);
            let limit = 3 * allowance;
            if self.map_distance(city.x, city.y, unit.x, unit.y) > limit {
                return None;
            }
            let mut ready = unit.clone();
            ready.moves_left = allowance;
            let spent = if unit.kind.is_naval() {
                super::terrain_work::neighbors(city.x, city.y)
                    .filter(|&(x, y)| self.tile(x, y).terrain.is_water())
                    .filter_map(|(x, y)| {
                        let approach = self.route_cost_within(&ready, x, y, limit - 1)?;
                        Some(approach + MOVE_FRAGMENTS.min(allowance - approach % allowance))
                    })
                    .min()?
            } else {
                self.route_cost_within(&ready, city.x, city.y, limit)?
            };
            let turns = (spent.max(1) + allowance - 1) / allowance;
            let rating =
                POWER_FACTOR * stats.attack * unit.hp * stats.firepower / ADVISOR_POWER_DIVIDER;
            Some((unit, rating.max(1) / turns, turns))
        })
    }

    pub(super) fn preferred_city_defender(&self, city: &City) -> UnitKind {
        let roster = [
            (UnitKind::Warrior, false),
            (UnitKind::Phalanx, true),
            (UnitKind::Archer, false),
            (UnitKind::Legion, false),
            (UnitKind::Knights, false),
            (UnitKind::Musketeers, true),
            (UnitKind::Partisan, true),
            (UnitKind::Marines, true),
            (UnitKind::Paratroopers, false),
        ];
        roster
            .into_iter()
            .filter(|&(kind, _)| self.unit_buildable(city, kind))
            .max_by_key(|&(kind, good)| {
                let stats = kind.stats();
                let affordable = stats.cost <= city.shields + 40;
                let desire =
                    stats.hp * stats.firepower * stats.defense + stats.moves + stats.attack;
                (
                    affordable,
                    if affordable {
                        desire / if good { 1 } else { 2 }
                    } else {
                        -stats.cost
                    },
                    -stats.cost,
                )
            })
            .map_or(UnitKind::Warrior, |(kind, _)| kind)
    }

    pub(super) fn city_attacker_odds(
        &self,
        city: &City,
        enemy: &Unit,
        defender: &Unit,
        building: Option<Building>,
    ) -> f64 {
        let full_strength = percent_of(
            enemy.kind.stats().attack * POWER_FACTOR,
            enemy.veteran_power(),
        );
        let (attack_fp, defense_fp) = self.combat_firepower(enemy, defender);
        combat_win_chance(
            full_strength,
            enemy.hp,
            attack_fp,
            self.defense_power_with_building(defender, city.x, city.y, enemy.kind, building),
            defender.hp,
            defense_fp,
        )
    }

    pub(super) fn city_defense_production(&self, city: &City) -> Option<Production> {
        let defenders: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                (unit.x, unit.y) == (city.x, city.y)
                    && unit.owner == city.owner
                    && unit.kind.is_military()
                    && unit.transport.is_none()
            })
            .collect();
        let threats: Vec<_> = self
            .city_threat_arrivals(city)
            .filter(|&(_, _, turns)| turns <= 2)
            .map(|(unit, rating, _)| (unit, rating))
            .collect();
        if threats.is_empty() {
            return None;
        }

        let risk = |building, extra: Option<&Unit>| {
            threats
                .iter()
                .map(|(enemy, _)| {
                    defenders
                        .iter()
                        .copied()
                        .chain(extra)
                        .map(|defender| self.city_attacker_odds(city, enemy, defender, building))
                        .fold(1.0_f64, f64::min)
                })
                .fold(0.0_f64, f64::max)
        };
        let current = risk(None, None);
        if current <= 0.5 {
            return None;
        }
        let kind = self.preferred_city_defender(city);
        let recruit = self.make_unit(0, city.owner, kind, city.x, city.y);
        let recruit_risk = risk(None, Some(&recruit));
        let unit = Production::Unit(kind);
        let shields_remaining = |target: Production| {
            let stock = if city.production.class() == target.class() {
                city.shields
            } else {
                city.protected_shields + (city.shields - city.protected_shields).max(0) / 2
            };
            (target.cost() - stock).max(0)
        };
        let building = [
            Building::CITY_WALLS,
            Building::COASTAL_DEFENSE,
            Building::SAM_BATTERY,
            Building::SDI_DEFENSE,
        ]
        .into_iter()
        .filter(|&building| self.building_blocker(city, building).is_none())
        .filter_map(|building| {
            let target = Production::Building(building);
            let protected_risk = risk(Some(building), None);
            (protected_risk < current
                && protected_risk <= recruit_risk
                && shields_remaining(target) <= shields_remaining(unit))
            .then_some((target, protected_risk))
        })
        .min_by(|(a, a_risk), (b, b_risk)| {
            a_risk
                .total_cmp(b_risk)
                .then_with(|| shields_remaining(*a).cmp(&shields_remaining(*b)))
        });
        Some(building.map_or(unit, |(target, _)| target))
    }

    pub(super) fn prepare_ai_city_defenses(&mut self, owner: usize) {
        let orders: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .filter_map(|city| {
                let target = self
                    .counterintelligence_production(city)
                    .or_else(|| self.city_defense_production(city))?;
                (city.production != target).then_some((city.id, target))
            })
            .collect();
        for (city, target) in orders {
            self.change_production(city, target);
        }
    }

    pub(super) fn ai_gold_reserve(&self, owner: usize) -> i32 {
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

    pub(super) fn spend_ai_defense_gold(&mut self, owner: usize) {
        let owned = self.cities.iter().filter(|city| city.owner == owner);
        let expenses: i32 = owned
            .clone()
            .map(|city| self.building_upkeep(city) + self.city_unit_upkeep(city).gold)
            .sum();
        let reserve = self.ai_gold_reserve(owner);
        let mut priorities: Vec<_> = owned
            .filter_map(|city| {
                self.city_defense_production(city)?;
                let immediate = self
                    .city_threat_arrivals(city)
                    .filter(|&(_, _, turns)| turns == 1)
                    .map(|(_, rating, _)| rating)
                    .sum::<i32>();
                (immediate > 0).then_some((std::cmp::Reverse(immediate), city.id))
            })
            .collect();
        priorities.sort_unstable();
        self.ai_purchase_reserve.remove(&owner);
        for (_, id) in priorities {
            let city = self.cities.iter().find(|city| city.id == id).unwrap();
            let position = (city.x, city.y);
            self.spend_ai_upgrade_gold(owner, Some(position), expenses);
            let city = self.cities.iter().find(|city| city.id == id).unwrap();
            let Some(target) = self.city_defense_production(city) else {
                continue;
            };
            if city.production != target && !self.change_production(id, target) {
                continue;
            }
            let city = self.cities.iter().find(|city| city.id == id).unwrap();
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
        // Without a separate war-preparation countdown, active war with a
        // contacted rival is the available signal for holding back upgrades.
        let at_war = self.diplomacy.keys().any(|&(a, b)| {
            let rival = if a == owner {
                b
            } else if b == owner {
                a
            } else {
                return false;
            };
            rival != BARBARIANS && self.player_alive(rival) && self.at_war(owner, rival)
        });
        if !at_war {
            self.spend_ai_upgrade_gold(owner, None, reserve.max(expenses));
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

    pub(super) fn offensive_production(&self, city: &City) -> Option<UnitKind> {
        let forecast = self.city_forecast(city);
        let surplus = (forecast.shields - forecast.support.shields).max(1);
        let owner = city.owner;
        let mut targets: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                self.at_war(owner, unit.owner)
                    && unit.transport.is_none()
                    && !self.tile(unit.x, unit.y).terrain.is_water()
                    && self.tile(unit.x, unit.y).known_by[owner]
            })
            .map(|unit| (unit.x, unit.y))
            .collect();
        targets.extend(
            self.cities
                .iter()
                .filter(|target| {
                    self.at_war(owner, target.owner)
                        && self.tile(target.x, target.y).known_by[owner]
                })
                .map(|target| (target.x, target.y)),
        );
        targets.sort_unstable();
        targets.dedup();

        let mut best = None;
        let mut best_value = 0.0;
        for kind in UnitKind::AI_MILITARY
            .into_iter()
            .chain([
                UnitKind::Knights,
                UnitKind::Cannon,
                UnitKind::Musketeers,
                UnitKind::Marines,
            ])
            .filter(|&kind| self.unit_buildable(city, kind))
        {
            let candidate = self.make_unit(0, owner, kind, city.x, city.y);
            let stock = if matches!(city.production, Production::Unit(_)) {
                city.shields
            } else {
                city.protected_shields + (city.shields - city.protected_shields).max(0) / 2
            };
            let stats = kind.stats();
            let build_turns = ((stats.cost - stock).max(0) + surplus - 1) / surplus;
            for &(x, y) in &targets {
                if self.map_distance(city.x, city.y, x, y) > 4 * stats.moves
                    || self.treaty_blocks_move(&candidate, x, y)
                {
                    continue;
                }
                let path = self.path_for_unit(&candidate, x, y);
                if path.is_empty() || path.len() as i32 > 4 * stats.moves {
                    continue;
                }
                let city_value = self
                    .cities
                    .iter()
                    .find(|target| {
                        (target.x, target.y) == (x, y) && self.at_war(owner, target.owner)
                    })
                    .map_or(0, |target| 20 + 5 * target.size);
                let gain = if let Some(defender) = self
                    .best_defender_id(&candidate, x, y)
                    .and_then(|id| self.units.iter().find(|unit| unit.id == id))
                {
                    let (attack_fp, defense_fp) = self.combat_firepower(&candidate, defender);
                    let chance = combat_win_chance(
                        Self::attack_power(&candidate),
                        candidate.hp,
                        attack_fp,
                        self.defense_power_against(defender, x, y, kind),
                        defender.hp,
                        defense_fp,
                    );
                    let exposed: i32 = self
                        .combat_stack_losses(defender, &candidate)
                        .map(|unit| unit.kind.stats().cost)
                        .sum();
                    chance * f64::from(exposed) - (1.0 - chance) * f64::from(stats.cost)
                } else if city_value > 0
                    && kind.can_occupy_city()
                    && self
                        .units
                        .iter()
                        .all(|unit| (unit.x, unit.y) != (x, y) || self.allied(owner, unit.owner))
                {
                    f64::from(city_value)
                } else {
                    continue;
                };
                let travel = (path.len() as i32 + stats.moves - 1) / stats.moves;
                let value =
                    (gain - f64::from(travel)) * (23.0_f64 / 24.0).powi(build_turns + travel);
                if value > best_value {
                    best_value = value;
                    best = Some(kind);
                }
            }
        }
        best
    }

    fn military_tech_want(&self, city: &City, tech: Tech) -> i32 {
        let roster = UnitKind::AI_MILITARY;
        if !roster.iter().any(|kind| kind.required_tech() == Some(tech)) {
            return 0;
        }
        let available = |kind| self.unit_available(city.owner, kind);
        let mut want = 0;
        let best_defense = roster
            .into_iter()
            .filter(|&kind| available(kind))
            .map(|kind| kind.stats().defense)
            .max()
            .unwrap_or(0);
        // Freeciv requests the cheapest research that unlocks a stronger defender.
        let defender = roster
            .into_iter()
            .rev()
            .filter(|&kind| !available(kind) && kind.stats().defense > best_defense)
            .filter_map(|kind| kind.required_tech().map(|tech| (kind, tech)))
            .min_by_key(|&(_, tech)| {
                self.research_path(city.owner, tech)
                    .into_iter()
                    .map(|required| self.target_cost(city.owner, required))
                    .sum::<i32>()
            });
        for (enemy, danger) in self.city_threats(city) {
            if let Some((kind, required)) = defender
                && required == tech
            {
                want += if 2 * kind.stats().defense <= 3 * best_defense {
                    danger / 2
                } else {
                    danger
                };
            }
            // Compare the prospective attacker with units already available,
            // charging for travel time and replacement shields.
            let target = enemy.kind.stats();
            let distance = self.map_distance(city.x, city.y, enemy.x, enemy.y);
            let attack_want = |kind: UnitKind| {
                let stats = kind.stats();
                let turns = ((distance + stats.moves - 1) / stats.moves).max(1);
                let win = combat_win_chance(
                    stats.attack * POWER_FACTOR,
                    stats.hp,
                    stats.firepower,
                    self.defense_power_against(enemy, enemy.x, enemy.y, kind),
                    enemy.hp,
                    target.firepower,
                );
                let gain =
                    (win * f64::from(target.cost) - (1.0 - win) * f64::from(stats.cost)) as i32;
                (gain * SHIELD_WEIGHT / turns - SHIELD_WEIGHT * turns).max(0)
            };
            let current = roster
                .into_iter()
                .filter(|&kind| available(kind))
                .map(attack_want)
                .max()
                .unwrap_or(0);
            let prospective = roster
                .into_iter()
                .filter(|kind| kind.required_tech() == Some(tech))
                .map(attack_want)
                .max()
                .unwrap_or(0);
            want += (prospective - current).max(0);
        }
        want
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn city_output_priorities_raise_the_bid_for_a_library_technology() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
        game.players[HUMAN].known = vec![Tech::ALPHABET, writing];
        game.players[1].known = vec![Tech::ALPHABET];
        let no_cities = game.tech_sell_price(HUMAN, 1, writing).unwrap();
        game.create_city(1, 20, 20);
        game.cities.last_mut().unwrap().size = 3;
        game.cities.last_mut().unwrap().specialists = [2, 0, 0];
        assert!(game.tech_sell_price(HUMAN, 1, writing).unwrap() > no_cities);
        assert!(!game.has_tech(1, writing));
    }

    #[test]
    fn a_reachable_enemy_raises_demand_for_a_defender_upgrade() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[HUMAN].known = vec![Tech::BRONZE_WORKING];
        game.players[1].known.clear();
        let (x, y) = (20, 20);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.create_city(1, x, y);
        let peaceful_price = game
            .tech_sell_price(HUMAN, 1, Tech::BRONZE_WORKING)
            .unwrap();
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(nx, ny).terrain = Terrain::Grassland;
        game.tile_mut(nx, ny).known_by[1] = true;
        game.spawn_unit(HUMAN, UnitKind::Legion, nx, ny);
        assert!(
            game.tech_sell_price(HUMAN, 1, Tech::BRONZE_WORKING)
                .unwrap()
                > peaceful_price
        );
    }
}
