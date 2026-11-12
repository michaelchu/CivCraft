//! Map, minimap, and world rendering projections.

use super::*;

impl Simulation {
    pub(crate) fn displayed_tile_owner_view(&self, x: i32, y: i32) -> Option<usize> {
        self.displayed_tile_owner(x, y)
    }

    pub(crate) fn minimap_tile_view(&self, x: i32, y: i32) -> MinimapTileView {
        let tile = self.render_tile(x, y);
        MinimapTileView {
            terrain: tile.terrain,
            seen: tile.seen,
            visible: tile.visible,
        }
    }

    pub(crate) fn render_tile_view(&self, x: i32, y: i32) -> RenderTileView {
        let tile = self.render_tile(x, y);
        RenderTileView {
            terrain: tile.terrain,
            base_terrain: tile.base_terrain,
            resource: tile
                .active_resource()
                .filter(|&resource| self.resource_visible_to(HUMAN, resource)),
            hut: tile.hut,
            river_edges: tile.river_edges,
            road: tile.road,
            railroad: tile.railroad,
            irrigation: tile.irrigation,
            farmland: tile.farmland,
            pollution: tile.pollution,
            fallout: tile.fallout,
            craters: tile.craters,
            base: tile.base,
            mine: tile.mine,
            oil_well: tile.oil_well,
            oil_platform: tile.oil_platform,
            seen: tile.seen,
            visible: tile.visible,
        }
    }

    pub(crate) fn minimap_city_views(&self) -> Vec<MinimapCityView> {
        self.render_city_markers()
            .into_iter()
            .map(|city| self.minimap_city_view(city.id, city.x, city.y, city.owner))
            .collect()
    }

    pub(crate) fn minimap_city_override_view(&self, city: &RenderedCityView) -> MinimapCityView {
        MinimapCityView {
            id: city.id,
            x: city.x,
            y: city.y,
            owner: city.owner,
            civilization_id: city.civilization_id.clone(),
        }
    }

    fn minimap_city_view(&self, id: u32, x: i32, y: i32, owner: usize) -> MinimapCityView {
        MinimapCityView {
            id,
            x,
            y,
            owner,
            civilization_id: self.players[owner].civilization_id.clone(),
        }
    }

    pub(crate) fn rendered_city_views(&self) -> Vec<RenderedCityView> {
        self.render_cities()
            .iter()
            .map(|observed| self.rendered_city_view_from_observed(observed))
            .collect()
    }

    pub(crate) fn rendered_city_override_view(&self, city: &City) -> RenderedCityView {
        let observed = self.observed_city(city);
        self.rendered_city_view_from_observed(&observed)
    }

    pub(crate) fn rendered_unit_views(&self) -> Vec<RenderedUnitView> {
        self.units
            .iter()
            .map(|unit| self.rendered_unit_view(unit))
            .filter(|unit| unit.visible)
            .collect()
    }

    pub(crate) fn rendered_unit_override_view(&self, unit: &Unit) -> RenderedUnitView {
        let mut view = self.rendered_unit_view(unit);
        view.context = Some(self.unit_context_view(unit));
        view
    }

    fn rendered_unit_view(&self, unit: &Unit) -> RenderedUnitView {
        let stats = unit.kind.stats();
        RenderedUnitView {
            id: unit.id,
            kind: unit.kind,
            owner: unit.owner,
            civilization_id: self.players[unit.owner].civilization_id.clone(),
            era_index: self.research_era(unit.owner).index(),
            x: unit.x,
            y: unit.y,
            facing: unit.facing,
            transport: unit.transport,
            is_transporter: unit.kind.transport_capacity() > 0,
            work_kind: unit.work_kind,
            state: unit.state,
            moves_left: unit.moves_left,
            movement_rate: self.unit_move_rate(unit),
            hp: unit.hp,
            max_hp: unit.max_hit_points(),
            attack: stats.attack,
            defense: stats.defense,
            visible: self.unit_visible_to_human(unit),
            context: None,
        }
    }

    pub(super) fn rendered_city_view_from_observed(
        &self,
        observed: &ObservedCity,
    ) -> RenderedCityView {
        let city = &observed.city;
        let (growth_label, production_label) = if city.owner == HUMAN {
            let forecast = self.city_forecast(city);
            let capacity = Self::growth_box(city.size);
            let surplus = forecast.food_surplus(city.size, forecast.support.food);
            let can_grow = city.size < self.city_size_limit(city);
            let growth = if city.food + surplus >= capacity && !can_grow {
                "blocked".to_owned()
            } else if city.food + surplus >= capacity {
                "1".to_owned()
            } else if surplus > 0 {
                ((capacity - city.food + surplus - 1) / surplus).to_string()
            } else if surplus < 0 {
                format!("famine {}", 1 - city.food / surplus)
            } else {
                "-".to_owned()
            };
            let production = match city.production {
                Production::Building(building)
                    if building.data().genus == crate::rules::Genus::Convert =>
                {
                    city.production.name().to_owned()
                }
                _ => {
                    let available = match city.production {
                        Production::Building(building)
                            if building.data().genus == crate::rules::Genus::GreatWonder =>
                        {
                            self.great_wonder_available(building)
                        }
                        _ => true,
                    };
                    let turns = if available {
                        let cost = city.production.cost();
                        let surplus = forecast.shield_surplus();
                        if city.shields >= cost {
                            Some(1)
                        } else if surplus > 0 {
                            Some((cost - city.shields - 1) / surplus + 1)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                    .filter(|turns| *turns <= 999);
                    format!(
                        "{} : {}",
                        city.production.name(),
                        turns.map_or_else(|| "-".to_owned(), |turns| turns.to_string())
                    )
                }
            };
            (Some(growth), Some(production))
        } else {
            (None, None)
        };
        RenderedCityView {
            id: city.id,
            name: city.name.clone(),
            owner: city.owner,
            x: city.x,
            y: city.y,
            size: city.size,
            civilization_id: self.players[city.owner].civilization_id.clone(),
            civ3_style: observed.civ3_style,
            civ3_era: observed.civ3_era,
            occupied: observed.occupied,
            disorder: observed.disorder,
            capital: city.buildings.contains(&Building::PALACE)
                || city.buildings.contains(&Building::ECCLESIASTICAL_PALACE),
            growth_label,
            production_label,
        }
    }
}
