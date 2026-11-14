//! City management and city context projections.

use super::*;

impl Simulation {
    pub(crate) fn city_manager_view(&self, city_id: u32) -> Option<CityManagerView> {
        let city = self.city_by_id(city_id)?;
        let observed = self.observed_city(city);
        let city_sprite = self.rendered_city_view_from_observed(&observed);
        let forecast = self.city_forecast(city);
        let support = &forecast.support;
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
                let tile = self.tile(x, y);
                let is_city = (dx, dy) == (0, 0);
                let worked = city.worked_tiles.contains(&(x, y));
                CityMapTileView {
                    dx,
                    dy,
                    x,
                    y,
                    is_city,
                    seen: tile.seen,
                    worked,
                    can_assign: !is_city
                        && (worked
                            || (city.specialists.iter().any(|&n| n > 0)
                                && self.can_assign_city_tile(city, x, y))),
                    yields: tile.yields(),
                }
            })
            .collect();
        let garrison = self
            .units
            .iter()
            .filter(|unit| unit.owner == HUMAN && unit.x == city.x && unit.y == city.y)
            .map(|unit| GarrisonUnitView {
                id: unit.id,
                kind: unit.kind,
            })
            .collect();
        Some(CityManagerView {
            id: city.id,
            name: city.name.clone(),
            owner: city.owner,
            x: city.x,
            y: city.y,
            civilization_id: self.players[city.owner].civilization_id.clone(),
            era_index: self.research_era(city.owner).index(),
            city_sprite,
            size: city.size,
            food: city.food,
            shields: city.shields,
            production_name: city.production.name().to_owned(),
            production_cost: city.production.cost(),
            bought_production: city.bought_production,
            specialists: city.specialists,
            tile_workers: city.tile_workers(),
            worklist,
            food_net: forecast.food_surplus(city.size, support.food),
            shield_net: forecast.shield_surplus(),
            science: forecast.science,
            gold_net: forecast.gold_surplus(support.gold),
            luxury: forecast.luxury,
            disorder: forecast.disorder,
            growth_box: Self::growth_box(city.size),
            growth_limited: city.size >= self.city_size_limit(city),
            support_food: support.food,
            support_shields: support.shields,
            support_gold: support.gold,
            rush_cost: self.rush_cost(city),
            treasury: self.players[city.owner].gold,
            buildings,
            units,
            garrison,
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
        })
    }
}
