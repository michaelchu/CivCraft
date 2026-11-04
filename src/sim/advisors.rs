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
        self.cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| self.city_tech_want(city, tech) + self.military_tech_want(city, tech))
            .sum()
    }

    fn advisor_city_output(&self, city: &City, granted: Option<Tech>) -> i32 {
        let (food, shields, trade) = self.city_yields_with_tech(city, granted);
        let [science, gold, luxury] = self.city_trade(city, trade);
        let tax = self.players[city.owner].rates[1];
        let gold = gold - self.building_upkeep(city);
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
        let direct = match tech.name() {
            "Railroad" | "Trade" | "The Corporation" => {
                self.advisor_city_output(city, Some(tech)) - baseline
            }
            "Mysticism"
                if city.has_building("Temple")
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
            // Growth headroom matters when a city is about to reach its current limit.
            let limit = Self::city_size_limit(city);
            let new_limit = Self::city_size_limit(&improved);
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
            let retention = Self::food_retained(&improved) - Self::food_retained(city);
            if retention > 0 {
                benefit += 8
                    + retention / 10 * food_surplus.max(0)
                    + retention / 15 * (-food_surplus).max(1);
            }
            if data.name == "City Walls" {
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

    fn city_threats<'a>(&'a self, city: &'a City) -> impl Iterator<Item = (&'a Unit, i32)> + 'a {
        self.units.iter().filter_map(|unit| {
            let stats = unit.kind.stats();
            if !unit.kind.is_military()
                || unit.transport.is_some()
                || !self.at_war(city.owner, unit.owner)
                || !self.tile(unit.x, unit.y).known_by[city.owner]
            {
                return None;
            }
            let distance = self.map_distance(city.x, city.y, unit.x, unit.y);
            if distance > 3 * stats.moves {
                return None;
            }
            let route = self.path_to(unit.id, city.x, city.y);
            if distance > 0 && route.is_empty() {
                return None;
            }
            let turns = (route.len() as i32 + stats.moves - 1) / stats.moves;
            let rating =
                POWER_FACTOR * stats.attack * unit.hp * stats.firepower / ADVISOR_POWER_DIVIDER;
            Some((unit, rating.max(1) / turns.max(1)))
        })
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
                    self.defense_power(enemy, enemy.x, enemy.y),
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
