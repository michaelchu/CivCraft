//! City management and city context projections.

use super::*;

impl Simulation {
    pub(crate) fn city_manager_view(&self, city_id: u32) -> Option<CityManagerView> {
        let city = self.city_by_id(city_id)?;
        let forecast = self.city_forecast(city);
        let support = &forecast.support;
        let governor = self
            .city_governors
            .get(&city.id)
            .copied()
            .unwrap_or_default();
        let units = UnitKind::ALL_NATIVE
            .into_iter()
            .map(|kind| {
                let production = Production::Unit(kind);
                UnitProductionView {
                    kind,
                    cost: kind.stats().cost,
                    available: self.can_produce(city, production),
                    current: city.production == production,
                    can_change: self.can_change_production(city, production),
                    can_queue: city.worklist.len() < WORKLIST_LIMIT
                        && self.can_produce(city, production),
                }
            })
            .collect();
        let buildings = Building::all()
            .map(|building| {
                let data = building.data();
                let production = Production::Building(building);
                let blocker = self.building_blocker(city, building);
                let cost_label = if data.genus == crate::rules::Genus::Convert {
                    "Converts shields to gold".to_owned()
                } else {
                    format!("{} shields  /  ref. upkeep {}", data.cost, data.upkeep)
                };
                BuildingProductionView {
                    building,
                    name: data.name,
                    cost_label,
                    built: city.buildings.contains(&building),
                    current: city.production == production,
                    can_change: blocker.is_none() && self.can_change_production(city, production),
                    can_queue: city.worklist.len() < WORKLIST_LIMIT
                        && self.can_produce(city, production),
                    sale_price: Self::building_sale_price(city, building),
                    blocker,
                }
            })
            .collect();
        let worklist = city
            .worklist
            .iter()
            .copied()
            .map(|production| {
                let target = self.worklist_replacement(city, production);
                CityWorklistItemView {
                    production,
                    name: production.name().to_owned(),
                    target,
                    target_name: target.name().to_owned(),
                    available: self.can_produce(city, target),
                }
            })
            .collect();
        let city_map_tiles = self
            .map_city_radius(city)
            .map(|(dx, dy, x, y)| {
                let tile = self.human_known_tile(x, y);
                let is_city = (dx, dy) == (0, 0);
                let worked = city.worked_tiles.contains(&(x, y));
                CityMapTileView {
                    dx,
                    dy,
                    x,
                    y,
                    is_city,
                    seen: tile.is_some(),
                    worked,
                    can_assign: !governor.active
                        && !is_city
                        && (worked
                            || (city.specialists.iter().any(|&n| n > 0)
                                && self.can_assign_city_tile(city, x, y))),
                    yields: tile.map_or((0, 0, 0), |tile| {
                        self.c2c3_city_tile_yields(
                            city,
                            (x, y),
                            tile,
                            is_city,
                            self.base_city_celebrating(city),
                            None,
                            self.players[city.owner].government,
                        )
                    }),
                }
            })
            .collect();
        // Civ III luxury identities, using the existing city resource-supply network.
        let luxuries = [
            (Resource::Dyes, "Dyes"),
            (Resource::Furs, "Furs"),
            (Resource::Gems, "Gems"),
            (Resource::Incense, "Incense"),
            (Resource::Ivory, "Ivory"),
            (Resource::Silk, "Silk"),
            (Resource::Spice, "Spices"),
            (Resource::Wine, "Wine"),
        ]
        .into_iter()
        .filter(|(resource, _)| self.resource_connected_to_city(city, *resource))
        .collect();
        Some(CityManagerView {
            id: city.id,
            name: city.name.clone(),
            owner: city.owner,
            x: city.x,
            y: city.y,
            era_index: self.research_era(city.owner).index(),
            size: city.size,
            culture: self.city_culture_score(city),
            culture_per_turn: self.city_culture_per_turn(city),
            next_border_threshold: crate::sim::culture::next_city_culture_border_threshold(
                self.city_culture_score(city),
            ),
            food: city.food,
            shields: city.shields,
            production_name: city.production.name().to_owned(),
            production_cost: city.production.cost(),
            bought_production: city.bought_production,
            specialists: city.specialists,
            tile_workers: city.tile_workers(),
            governor,
            worklist,
            food_net: forecast.food_surplus(city.size, support.food),
            shield_net: forecast.shield_surplus(),
            science: forecast.science,
            gold_net: forecast.gold_surplus(support.gold),
            luxury: forecast.luxury,
            disorder: forecast.disorder,
            growth_box: Self::growth_box(city.size),
            growth_limited: city.size >= self.city_size_limit(city),
            rush_cost: self.rush_cost(city),
            treasury: self.players[city.owner].gold,
            luxuries,
            buildings,
            units,
            map_tiles: city_map_tiles,
        })
    }

    pub(crate) fn city_context_view(&self, city_id: u32) -> Option<CityContextView> {
        let city = self.city_by_id(city_id)?;
        let forecast = self.city_forecast(city);
        Some(CityContextView {
            name: city.name.clone(),
            size: city.size,
            foreign_citizens: city.size - city.domestic_citizens(),
            food: forecast.food,
            shields: forecast.shields,
            trade: forecast.trade,
            illness: self.city_illness(city, forecast.shields),
            food_store: city.food,
            growth_box: Self::growth_box(city.size),
            production_name: city.production.name(),
            production_stock: city.shields,
            production_cost: city.production.cost(),
            culture: self.city_culture_score(city),
            culture_per_turn: self.city_culture_per_turn(city),
            next_border_threshold: crate::sim::culture::next_city_culture_border_threshold(
                self.city_culture_score(city),
            ),
        })
    }
}

#[cfg(test)]
mod city_luxury_tests {
    use super::*;

    #[test]
    fn city_luxuries_require_an_owned_road_connection_and_exclude_other_resources() {
        let mut sim = Simulation::new(crate::test_support::seeded_map());
        sim.cities.clear();
        sim.units.clear();
        for tile in &mut sim.tiles {
            tile.resource = None;
            tile.road = false;
        }
        sim.create_city(HUMAN, 10, 20);
        let city_id = sim.cities[0].id;
        let (x, y) = sim.map_wrap.civ3_neighbor(10, 20, 2).unwrap();
        let tile = sim.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.owner = Some(HUMAN);
        tile.resource = Some(Resource::Wine);
        assert!(sim.city_manager_view(city_id).unwrap().luxuries.is_empty());
        sim.tile_mut(x, y).road = true;
        assert_eq!(
            sim.city_manager_view(city_id).unwrap().luxuries,
            vec![(Resource::Wine, "Wine")]
        );
        sim.tile_mut(x, y).owner = Some(1);
        assert!(sim.city_manager_view(city_id).unwrap().luxuries.is_empty());
        let tile = sim.tile_mut(x, y);
        tile.owner = Some(HUMAN);
        tile.resource = Some(Resource::Horses);
        assert!(sim.city_manager_view(city_id).unwrap().luxuries.is_empty());
    }
}
