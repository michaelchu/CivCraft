//! Persistent Freeciv-style city-to-city trade-route records.
//!
//! Route state lives independently of the Civ III unit catalog. A route keeps
//! the city pair and selected Freeciv good; eligibility, route income, and
//! lifecycle rules are resolved by this subsystem from the active ruleset.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::rules::freeciv::{
    TradeGoodFlag, TradeGoodRulesetRecord, TradeRevenueStyle, TradeRouteCancellation,
    TradeRouteRulesetSettings, TradeRouteType, TradeRouteTypeSettings, requirements_met,
};

use super::action_rules::SimulationRequirementContext;
use super::{City, Pact, Simulation};

const FREECIV_MAX_TRADE_ROUTES: i32 = 20;

fn campaign_settings() -> &'static TradeRouteRulesetSettings {
    static SETTINGS: OnceLock<TradeRouteRulesetSettings> = OnceLock::new();
    SETTINGS.get_or_init(|| {
        crate::rules::freeciv::RulesetData::campaign_default()
            .trade_route_settings()
            .unwrap_or_else(|error| panic!("invalid C2C3 trade-route rules: {error}"))
    })
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TradeRoute {
    pub(crate) from_city_id: u32,
    pub(crate) to_city_id: u32,
    pub(crate) goods_rule_name: String,
}

impl TradeRoute {
    pub(crate) fn unordered_city_pair(&self) -> (u32, u32) {
        if self.from_city_id < self.to_city_id {
            (self.from_city_id, self.to_city_id)
        } else {
            (self.to_city_id, self.from_city_id)
        }
    }
}

#[derive(Debug)]
struct TradeNetworkComponents {
    parents: Vec<usize>,
    ranks: Vec<u8>,
}

impl TradeNetworkComponents {
    fn new(size: usize) -> Self {
        Self {
            parents: (0..size).collect(),
            ranks: vec![0; size],
        }
    }

    fn find(&mut self, index: usize) -> usize {
        let mut root = index;
        while self.parents[root] != root {
            root = self.parents[root];
        }
        let mut current = index;
        while self.parents[current] != current {
            let next = self.parents[current];
            self.parents[current] = root;
            current = next;
        }
        root
    }

    fn connect(&mut self, first: usize, second: usize) {
        let mut first = self.find(first);
        let mut second = self.find(second);
        if first == second {
            return;
        }
        if self.ranks[first] < self.ranks[second] {
            std::mem::swap(&mut first, &mut second);
        }
        self.parents[second] = first;
        if self.ranks[first] == self.ranks[second] {
            self.ranks[first] += 1;
        }
    }
}

impl Simulation {
    /// Establishes routes between cities joined by a road, railroad, or
    /// Harbor-linked water network. This is CivCraft's explicit actor-free
    /// adapter for the C2C3 Caravan/Freight delivery action.
    pub(super) fn establish_connected_trade_routes(&mut self) {
        if self.cities.len() < 2 {
            return;
        }
        let settings = campaign_settings();
        if settings.goods.is_empty() {
            return;
        }
        let components = self.connected_trade_city_components();
        let continents = self.continent_labels();
        let mut cities: Vec<_> = self.cities.iter().map(|city| city.id).collect();
        cities.sort_unstable();
        for (position, &from_city_id) in cities.iter().enumerate() {
            for &to_city_id in cities.iter().skip(position + 1) {
                if components.get(&from_city_id) != components.get(&to_city_id) {
                    continue;
                }
                let Some(from) = self.city_by_id(from_city_id) else {
                    continue;
                };
                let good = settings
                    .goods
                    .iter()
                    .filter(|good| self.city_provides_good(from, good))
                    .max_by(|first, second| {
                        first
                            .select_priority
                            .cmp(&second.select_priority)
                            .then_with(|| second.id.cmp(&first.id))
                    });
                let Some(good) = good else {
                    continue;
                };
                if !self.can_establish_trade_route_with_continents(
                    from_city_id,
                    to_city_id,
                    good,
                    &continents,
                    settings,
                ) {
                    continue;
                }
                self.create_connected_trade_route(
                    from_city_id,
                    to_city_id,
                    good,
                    &continents,
                    settings,
                );
            }
        }
    }

    fn connected_trade_city_components(&self) -> BTreeMap<u32, usize> {
        let tile_count = self.tiles.len();
        let city_offset = tile_count * 2;
        let mut components = TradeNetworkComponents::new(city_offset + self.cities.len());
        let road_tiles: Vec<_> = self
            .tiles
            .iter()
            .map(|tile| !tile.terrain.is_water() && (tile.road || tile.railroad))
            .collect();
        let water_tiles: Vec<_> = self
            .tiles
            .iter()
            .map(|tile| tile.terrain.is_water())
            .collect();

        for index in 0..tile_count {
            let (x, y) = self.map_wrap.position(index);
            for (nx, ny) in self.map_wrap.civ3_neighbors(x, y) {
                let neighbor = self.map_wrap.index(nx, ny);
                if road_tiles[index] && road_tiles[neighbor] {
                    components.connect(index, neighbor);
                }
                if water_tiles[index] && water_tiles[neighbor] {
                    components.connect(tile_count + index, tile_count + neighbor);
                }
            }
        }

        for (city_index, city) in self.cities.iter().enumerate() {
            let hub = city_offset + city_index;
            let center = self.map_wrap.index(city.x, city.y);
            if road_tiles[center] {
                components.connect(hub, center);
            }
            for (nx, ny) in self.map_wrap.civ3_neighbors(city.x, city.y) {
                let neighbor = self.map_wrap.index(nx, ny);
                if road_tiles[neighbor] {
                    components.connect(hub, neighbor);
                }
                if city.has_building(crate::rules::Building::HARBOR) && water_tiles[neighbor] {
                    components.connect(hub, tile_count + neighbor);
                }
            }
        }

        self.cities
            .iter()
            .enumerate()
            .map(|(city_index, city)| (city.id, components.find(city_offset + city_index)))
            .collect()
    }

    fn create_connected_trade_route(
        &mut self,
        from_city_id: u32,
        to_city_id: u32,
        good: &TradeGoodRulesetRecord,
        continents: &[usize],
        settings: &TradeRouteRulesetSettings,
    ) {
        let mut replace = BTreeSet::new();
        for city_id in [from_city_id, to_city_id] {
            let Some(city) = self.city_by_id(city_id) else {
                return;
            };
            let capacity = self.city_route_capacity(city);
            if capacity <= 0 {
                return;
            }
            let current = self
                .trade_routes
                .iter()
                .filter(|route| route.from_city_id == city_id || route.to_city_id == city_id)
                .count() as i32;
            if current >= capacity {
                replace.extend(
                    self.trade_route_replacement_candidates(
                        city_id,
                        good.replace_priority,
                        capacity,
                        continents,
                        settings,
                    )
                    .into_iter()
                    .map(|(_, pair)| pair),
                );
            }
        }
        if !replace.is_empty() {
            self.trade_routes
                .retain(|route| !replace.contains(&route.unordered_city_pair()));
        }
        self.trade_routes.push(TradeRoute {
            from_city_id,
            to_city_id,
            goods_rule_name: good.id.clone(),
        });
    }

    /// Adds route trade to the city's gross trade output before city bonuses,
    /// waste, and the science/luxury/gold split are applied.
    pub(super) fn trade_route_income(
        &self,
        city: &City,
        granted: Option<crate::rules::Tech>,
    ) -> i32 {
        if !self
            .trade_routes
            .iter()
            .any(|route| route.from_city_id == city.id || route.to_city_id == city.id)
        {
            return 0;
        }
        let settings = campaign_settings();
        let continents = self.continent_labels();
        self.trade_routes
            .iter()
            .filter_map(|route| {
                let from = self
                    .cities
                    .iter()
                    .find(|candidate| candidate.id == route.from_city_id)?;
                let to = self
                    .cities
                    .iter()
                    .find(|candidate| candidate.id == route.to_city_id)?;
                let good = settings.good(&route.goods_rule_name)?;
                let direction_pct = if city.id == from.id {
                    good.from_percent
                } else if city.id == to.id {
                    good.to_percent
                } else {
                    return None;
                };
                let route_type = self.trade_route_type(from, to, &continents);
                let route_type_settings = settings.route_type(route_type)?;
                let can_trade = self.cities_can_trade(from, to, route_type_settings, settings);
                if !can_trade && route_type_settings.cancellation != TradeRouteCancellation::Active
                {
                    return Some(0);
                }
                if direction_pct <= 0 {
                    return Some(0);
                }
                let mut value =
                    self.trade_route_base_value(from, to, route_type_settings, settings, granted)
                        * direction_pct
                        / 100;
                value = value.max(settings.minimum_trade_route_value);
                // The pinned Freeciv effect name is `Traderoute_Pct`.
                let route_effect = self.city_rule_effect(
                    city,
                    "Traderoute_Pct",
                    None,
                    None,
                    None,
                    false,
                    self.players[city.owner].government,
                    granted,
                );
                Some(value * (100 + route_effect) / 100)
            })
            .sum()
    }

    /// Freeciv checks this before establishing a route and again while
    /// cancelling routes in the city activity phase. Missing diplomacy state
    /// is Freeciv's no-contact relation, whose route category is International.
    fn cities_can_trade(
        &self,
        first: &City,
        second: &City,
        route_type: &TradeRouteTypeSettings,
        settings: &TradeRouteRulesetSettings,
    ) -> bool {
        first.id != second.id
            && (first.owner != second.owner
                || self
                    .map_wrap
                    .civ3_distance(first.x, first.y, second.x, second.y)
                    >= settings.minimum_city_distance)
            && route_type.trade_percent > 0
    }

    fn trade_route_type(
        &self,
        first: &City,
        second: &City,
        continents: &[usize],
    ) -> TradeRouteType {
        let different_continents = {
            let first_region = continents[self.map_wrap.index(first.x, first.y)];
            let second_region = continents[self.map_wrap.index(second.x, second.y)];
            first_region != second_region
        };
        if first.owner == second.owner {
            return if different_continents {
                TradeRouteType::NationalIntercontinental
            } else {
                TradeRouteType::National
            };
        }
        let pact = self.contacted_pact(first.owner, second.owner);
        let base_type = match pact {
            Some(Pact::Alliance) => TradeRouteType::Ally,
            Some(Pact::War) => TradeRouteType::Enemy,
            Some(Pact::Ceasefire | Pact::Armistice | Pact::Peace) | None => {
                TradeRouteType::International
            }
        };
        if !different_continents {
            return base_type;
        }
        match base_type {
            TradeRouteType::Ally => TradeRouteType::AllyIntercontinental,
            TradeRouteType::Enemy => TradeRouteType::EnemyIntercontinental,
            _ => TradeRouteType::InternationalIntercontinental,
        }
    }

    fn trade_route_base_value(
        &self,
        first: &City,
        second: &City,
        route_type: &TradeRouteTypeSettings,
        settings: &TradeRouteRulesetSettings,
        granted: Option<crate::rules::Tech>,
    ) -> i32 {
        let distance = self
            .map_wrap
            .civ3_distance(first.x, first.y, second.x, second.y);
        let base = match settings.revenue_style {
            TradeRevenueStyle::Classic => {
                let map_scale = self.map_wrap.width().max(self.map_wrap.height()).max(1);
                let weighted_distance = ((100 - settings.world_relative_percent) * distance
                    + settings.world_relative_percent * (distance * 40 / map_scale))
                    / 100;
                weighted_distance + first.size + second.size
            }
            TradeRevenueStyle::Simple => {
                let first_trade = self.city_gross_yields_with_tech(first, granted).2;
                let second_trade = self.city_gross_yields_with_tech(second, granted).2;
                (first_trade + second_trade + 4) * 3
            }
        };
        base * route_type.trade_percent / 100 / 12
    }

    fn city_route_capacity(&self, city: &City) -> i32 {
        self.city_rule_effect(
            city,
            "Max_Trade_Routes",
            None,
            None,
            None,
            false,
            self.players[city.owner].government,
            None,
        )
        .clamp(0, FREECIV_MAX_TRADE_ROUTES)
    }

    fn city_provides_good(&self, city: &City, good: &TradeGoodRulesetRecord) -> bool {
        let context = SimulationRequirementContext::for_city_at(self, city, Some((city.x, city.y)));
        requirements_met(&good.requirements, &context)
            .unwrap_or_else(|error| panic!("C2C3 trade-good requirement failed: {error:?}"))
    }

    /// Remove routes that Freeciv's city activity phase would cancel. C2C3
    /// uses the `Cancel` policy for all ten route categories.
    pub(super) fn reconcile_trade_routes(&mut self) {
        if self.trade_routes.is_empty() {
            return;
        }
        let settings = campaign_settings();
        let continents = self.continent_labels();
        let retained: Vec<_> = self
            .trade_routes
            .iter()
            .filter(|route| {
                let Some(from) = self
                    .cities
                    .iter()
                    .find(|city| city.id == route.from_city_id)
                else {
                    return false;
                };
                let Some(to) = self.cities.iter().find(|city| city.id == route.to_city_id) else {
                    return false;
                };
                let Some(good) = settings.good(&route.goods_rule_name) else {
                    return false;
                };
                if good.flags.contains(&TradeGoodFlag::Depletes)
                    && !self.city_provides_good(from, good)
                {
                    return false;
                }
                let route_type = self.trade_route_type(from, to, &continents);
                let Some(route_type_settings) = settings.route_type(route_type) else {
                    return false;
                };
                self.cities_can_trade(from, to, route_type_settings, settings)
                    || route_type_settings.cancellation != TradeRouteCancellation::Cancel
            })
            .cloned()
            .collect();
        self.trade_routes = retained;
    }

    /// Source-compatible eligibility for a selected Good. The Civ III
    /// road/Harbor adapter calls this instead of duplicating C2C3 capacity,
    /// route-type, and replacement rules; no Caravan or Freight identity is
    /// introduced.
    pub(crate) fn can_establish_trade_route(
        &self,
        from_city_id: u32,
        to_city_id: u32,
        good_rule_name: &str,
    ) -> bool {
        let settings = campaign_settings();
        let Some(good) = settings.good(good_rule_name) else {
            return false;
        };
        let continents = self.continent_labels();
        self.can_establish_trade_route_with_continents(
            from_city_id,
            to_city_id,
            good,
            &continents,
            settings,
        )
    }

    fn can_establish_trade_route_with_continents(
        &self,
        from_city_id: u32,
        to_city_id: u32,
        good: &TradeGoodRulesetRecord,
        continents: &[usize],
        settings: &TradeRouteRulesetSettings,
    ) -> bool {
        let Some(from) = self.cities.iter().find(|city| city.id == from_city_id) else {
            return false;
        };
        let Some(to) = self.cities.iter().find(|city| city.id == to_city_id) else {
            return false;
        };
        let pair = TradeRoute {
            from_city_id,
            to_city_id,
            goods_rule_name: good.id.clone(),
        }
        .unordered_city_pair();
        if pair.0 == pair.1
            || self
                .trade_routes
                .iter()
                .any(|route| route.unordered_city_pair() == pair)
            || !self.city_provides_good(from, good)
        {
            return false;
        }
        let route_type = self.trade_route_type(from, to, continents);
        let Some(route_type_settings) = settings.route_type(route_type) else {
            return false;
        };
        if !self.cities_can_trade(from, to, route_type_settings, settings) {
            return false;
        }
        let from_capacity = self.city_route_capacity(from);
        let to_capacity = self.city_route_capacity(to);
        if from_capacity <= 0 || to_capacity <= 0 {
            return false;
        }
        let base_value = self.trade_route_base_value(from, to, route_type_settings, settings, None);
        let from_removable = self.trade_route_removable_value(
            from_city_id,
            good.replace_priority,
            from_capacity,
            continents,
            settings,
        );
        let to_removable = self.trade_route_removable_value(
            to_city_id,
            good.replace_priority,
            to_capacity,
            continents,
            settings,
        );
        from_removable < base_value && to_removable < base_value
    }

    fn trade_route_removable_value(
        &self,
        city_id: u32,
        priority: i32,
        capacity: i32,
        continents: &[usize],
        settings: &TradeRouteRulesetSettings,
    ) -> i32 {
        self.trade_route_replacement_candidates(city_id, priority, capacity, continents, settings)
            .iter()
            .map(|(value, _)| *value)
            .sum()
    }

    fn trade_route_replacement_candidates(
        &self,
        city_id: u32,
        priority: i32,
        capacity: i32,
        continents: &[usize],
        settings: &TradeRouteRulesetSettings,
    ) -> Vec<(i32, (u32, u32))> {
        let route_count = self
            .trade_routes
            .iter()
            .filter(|route| route.from_city_id == city_id || route.to_city_id == city_id)
            .count() as i32;
        if route_count < capacity {
            return Vec::new();
        }
        if self.cities.iter().all(|city| city.id != city_id) {
            return vec![(i32::MAX, (u32::MAX, u32::MAX))];
        }
        let mut candidates: Vec<_> = self
            .trade_routes
            .iter()
            .filter(|route| route.from_city_id == city_id || route.to_city_id == city_id)
            .filter_map(|route| {
                let good = settings.good(&route.goods_rule_name)?;
                if good.replace_priority > priority {
                    return None;
                }
                let from = self
                    .cities
                    .iter()
                    .find(|candidate| candidate.id == route.from_city_id)?;
                let to = self
                    .cities
                    .iter()
                    .find(|candidate| candidate.id == route.to_city_id)?;
                let route_type = self.trade_route_type(from, to, continents);
                let route_type_settings = settings.route_type(route_type)?;
                let base =
                    self.trade_route_base_value(from, to, route_type_settings, settings, None);
                let direction_pct = if city_id == route.from_city_id {
                    good.from_percent
                } else {
                    good.to_percent
                };
                let mut value = base * direction_pct / 100;
                if direction_pct > 0 {
                    value = value.max(settings.minimum_trade_route_value);
                }
                Some((good.replace_priority, value, route.unordered_city_pair()))
            })
            .collect();
        candidates
            .sort_by_key(|(replace_priority, value, pair)| (-replace_priority, *value, *pair));
        let to_remove = candidates.len() as i32 + 1 - capacity;
        let to_remove = if to_remove < 0 { 1 } else { to_remove as usize };
        candidates
            .iter()
            .take(to_remove)
            .map(|(_, value, pair)| (*value, *pair))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Building, Tech};
    use crate::world::Terrain;

    fn route_test_game() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
        }
        game.units.clear();
        game.cities.clear();
        game.trade_routes.clear();
        game.diplomacy.clear();
        game.diplomatic_contact_until.clear();
        game.create_city(0, 5, 20);
        game.create_city(1, 20, 20);
        for city in &mut game.cities {
            city.size = 10;
        }
        game
    }

    #[test]
    fn c2c3_classic_route_income_uses_wrapped_map_scale_and_city_sizes() {
        let mut game = route_test_game();
        let from = game.cities[0].id;
        let to = game.cities[1].id;
        game.trade_routes.push(TradeRoute {
            from_city_id: from,
            to_city_id: to,
            goods_rule_name: "Goods".to_owned(),
        });

        // C2C3 uses world-relative 100: distance * 40 / max(36, 48), then
        // adds both sizes, applies the 100% IN route rate, then divides by 12.
        let distance = game.map_wrap.civ3_distance(5, 20, 20, 20);
        let expected = (distance * 40 / 48 + 10 + 10) / 12;
        assert_eq!(
            campaign_settings().revenue_style,
            TradeRevenueStyle::Classic
        );
        assert_eq!(game.trade_route_income(&game.cities[0], None), expected);
        assert_eq!(game.trade_route_income(&game.cities[1], None), expected);
        assert!(expected > 0);
        let baseline = {
            let mut baseline = game.clone();
            baseline.trade_routes.clear();
            baseline.city_yields(&baseline.cities[0]).2
        };
        assert!(game.city_yields(&game.cities[0]).2 > baseline);
    }

    #[test]
    fn war_cancels_international_route_before_city_output() {
        let mut game = route_test_game();
        let from = game.cities[0].id;
        let to = game.cities[1].id;
        game.trade_routes.push(TradeRoute {
            from_city_id: from,
            to_city_id: to,
            goods_rule_name: "Goods".to_owned(),
        });
        assert!(game.trade_route_income(&game.cities[0], None) > 0);

        game.make_diplomatic_contact(0, 1);
        game.sign_pact(0, 1, Pact::War);
        game.reconcile_trade_routes();
        assert!(game.trade_routes.is_empty());
        assert!(!game.can_establish_trade_route(from, to, "Goods"));
    }

    #[test]
    fn national_route_categories_are_disabled_by_c2c3_and_capacity_is_effect_driven() {
        let mut game = route_test_game();
        game.cities[1].owner = 0;
        let continents = game.continent_labels();
        let route_type = game.trade_route_type(&game.cities[0], &game.cities[1], &continents);
        assert_eq!(route_type, TradeRouteType::National);
        assert!(!game.can_establish_trade_route(game.cities[0].id, game.cities[1].id, "Goods"));
        assert_eq!(game.city_route_capacity(&game.cities[0]), 2);
    }

    #[test]
    fn no_contact_remains_international_even_with_default_relation_storage() {
        let mut game = route_test_game();
        game.diplomacy
            .entry(Simulation::relation_key(0, 1))
            .or_default();
        let continents = game.continent_labels();
        assert_eq!(
            game.trade_route_type(&game.cities[0], &game.cities[1], &continents),
            TradeRouteType::International
        );
    }

    #[test]
    fn established_route_pairs_cannot_be_added_twice_in_either_direction() {
        let mut game = route_test_game();
        let from = game.cities[0].id;
        let to = game.cities[1].id;
        assert!(game.can_establish_trade_route(from, to, "Goods"));
        game.trade_routes.push(TradeRoute {
            from_city_id: from,
            to_city_id: to,
            goods_rule_name: "Goods".to_owned(),
        });
        assert!(!game.can_establish_trade_route(to, from, "Goods"));
    }

    #[test]
    fn full_city_replaces_only_lower_or_equal_priority_routes_with_lower_value() {
        let mut game = route_test_game();
        game.create_city(2, 10, 20);
        game.create_city(3, 16, 20);
        game.create_city(4, 2, 20);
        for city in &mut game.cities {
            city.size = 10;
        }
        game.cities[2].size = 1;
        let origin = game.cities[0].id;
        let existing_far = game.cities[1].id;
        let existing_near = game.cities[2].id;
        let candidate_far = game.cities[3].id;
        let candidate_near = game.cities[4].id;
        game.trade_routes.extend([
            TradeRoute {
                from_city_id: origin,
                to_city_id: existing_far,
                goods_rule_name: "Goods".to_owned(),
            },
            TradeRoute {
                from_city_id: origin,
                to_city_id: existing_near,
                goods_rule_name: "Goods".to_owned(),
            },
        ]);
        assert_eq!(
            game.trade_routes
                .iter()
                .filter(|route| route.from_city_id == origin || route.to_city_id == origin)
                .count(),
            2
        );
        assert_eq!(game.city_route_capacity(&game.cities[0]), 2);

        assert!(game.can_establish_trade_route(origin, candidate_far, "Goods"));
        let settings = campaign_settings();
        let continents = game.continent_labels();
        let from = game.city_by_id(origin).unwrap();
        let near = game.city_by_id(candidate_near).unwrap();
        let route_type = game.trade_route_type(from, near, &continents);
        let route_type_settings = settings.route_type(route_type).unwrap();
        let candidate_value =
            game.trade_route_base_value(from, near, route_type_settings, settings, None);
        let existing_values: Vec<_> = game
            .trade_routes
            .iter()
            .map(|route| {
                let source = game.city_by_id(route.from_city_id).unwrap();
                let destination = game.city_by_id(route.to_city_id).unwrap();
                let kind = game.trade_route_type(source, destination, &continents);
                let value = game.trade_route_base_value(
                    source,
                    destination,
                    settings.route_type(kind).unwrap(),
                    settings,
                    None,
                );
                (kind, value)
            })
            .collect();
        let removable_value = game.trade_route_removable_value(
            origin,
            settings.good("Goods").unwrap().replace_priority,
            game.city_route_capacity(from),
            &continents,
            settings,
        );
        assert!(
            !game.can_establish_trade_route(origin, candidate_near, "Goods"),
            "candidate={candidate_value}, removable={removable_value}, existing={existing_values:?}"
        );
    }

    #[test]
    fn source_tech_requirements_for_goods_use_shared_requirement_evaluator() {
        let mut game = route_test_game();
        game.players[0]
            .known
            .retain(|&tech| tech != Tech::ECONOMICS);
        let mut good = campaign_settings().goods.first().unwrap().clone();
        good.requirements.push(crate::rules::freeciv::Requirement {
            kind: "Tech".to_owned(),
            name: crate::rules::freeciv::RequirementValue::String("Trade".to_owned()),
            range: crate::rules::freeciv::RequirementRange::Player,
            present: true,
            survives: false,
            quiet: false,
            source: crate::rules::freeciv::RuleSourceLocation {
                file: "test.ruleset".into(),
                section: "goods_good".to_owned(),
                key: "reqs".to_owned(),
                line: 1,
            },
        });
        assert!(!game.city_provides_good(&game.cities[0], &good));
        game.players[0].known.push(Tech::ECONOMICS);
        assert!(game.city_provides_good(&game.cities[0], &good));
    }

    #[test]
    fn road_connectivity_establishes_one_persistent_route_without_rng_or_delivery_bonus() {
        let mut game = route_test_game();
        let from = game.cities[0].id;
        let to = game.cities[1].id;
        game.establish_connected_trade_routes();
        assert!(game.trade_routes.is_empty());

        for tile in &mut game.tiles {
            tile.road = true;
        }
        for (x, y) in [(5, 20), (20, 20)] {
            game.tile_mut(x, y).road = false;
        }
        let random = game.random_snapshot();
        let gold = [game.players[0].gold, game.players[1].gold];
        game.establish_connected_trade_routes();
        assert_eq!(game.random_snapshot(), random);
        assert_eq!([game.players[0].gold, game.players[1].gold], gold);
        assert_eq!(game.trade_routes.len(), 1);
        assert_eq!(game.trade_routes[0].unordered_city_pair(), (from, to));

        // Roads trigger creation; once established, Freeciv route lifecycle
        // rules keep the route even if the connecting road is later removed.
        for tile in &mut game.tiles {
            tile.road = false;
        }
        game.establish_connected_trade_routes();
        game.reconcile_trade_routes();
        assert_eq!(game.trade_routes.len(), 1);
    }

    #[test]
    fn end_turn_establishes_connected_routes_before_city_output() {
        let mut game = route_test_game();
        let expected_pair = (game.cities[0].id, game.cities[1].id);
        for tile in &mut game.tiles {
            tile.road = true;
        }

        game.end_turn();

        assert!(
            game.trade_routes
                .iter()
                .any(|route| { route.unordered_city_pair() == expected_pair })
        );
    }

    #[test]
    fn harbor_connectivity_joins_coastal_ports_without_joining_unported_cities() {
        let mut game = route_test_game();
        let first_id = game.cities[0].id;
        let second_id = game.cities[1].id;
        for x in 0..game.map_wrap.width() {
            game.tile_mut(x, 19).terrain = Terrain::Sea;
        }
        game.establish_connected_trade_routes();
        assert!(game.trade_routes.is_empty());

        game.cities
            .iter_mut()
            .find(|city| city.id == first_id)
            .unwrap()
            .buildings
            .push(Building::HARBOR);
        game.cities
            .iter_mut()
            .find(|city| city.id == second_id)
            .unwrap()
            .buildings
            .push(Building::HARBOR);
        game.establish_connected_trade_routes();
        assert_eq!(game.trade_routes.len(), 1);
        assert_eq!(
            game.trade_routes[0].unordered_city_pair(),
            (first_id, second_id)
        );
    }

    #[test]
    fn connected_network_route_creation_obeys_city_capacity_deterministically() {
        let mut game = route_test_game();
        for (owner, x) in [(2, 10), (3, 16), (4, 2)] {
            game.create_city(owner, x, 20);
        }
        for city in &mut game.cities {
            city.size = 10;
        }
        for tile in &mut game.tiles {
            tile.road = true;
        }
        let random = game.random_snapshot();
        game.establish_connected_trade_routes();
        let first = game.trade_routes.clone();
        assert!(!first.is_empty());
        for city in &game.cities {
            let routes = game
                .trade_routes
                .iter()
                .filter(|route| route.from_city_id == city.id || route.to_city_id == city.id)
                .count();
            assert!(routes <= game.city_route_capacity(city) as usize);
        }
        assert_eq!(game.random_snapshot(), random);
        game.establish_connected_trade_routes();
        assert_eq!(game.trade_routes, first);
    }
}
