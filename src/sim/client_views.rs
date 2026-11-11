//! Read-only projections for client screens.

use super::*;

/// The values needed to draw the selected unit's context card, without
/// exposing player, tile, transport, or movement-rule lookups to that view.
pub(crate) struct UnitContextView {
    pub(crate) kind: UnitKind,
    pub(crate) owner: usize,
    pub(crate) civilization_id: Option<String>,
    pub(crate) era_index: usize,
    pub(crate) owner_name: String,
    pub(crate) terrain_name: &'static str,
    pub(crate) status: String,
    pub(crate) hp: i32,
    pub(crate) max_hp: i32,
    pub(crate) moves_left: i32,
    pub(crate) moves_allowance: i32,
    pub(crate) attack: i32,
    pub(crate) defense: i32,
}

/// The computed values needed by the selected city's compact context card.
pub(crate) struct CityContextView {
    pub(crate) name: String,
    pub(crate) size: i32,
    pub(crate) foreign_citizens: i32,
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) trade: i32,
    pub(crate) illness: i32,
    pub(crate) food_store: i32,
    pub(crate) growth_box: i32,
    pub(crate) production_name: &'static str,
    pub(crate) production_stock: i32,
    pub(crate) production_cost: i32,
}

impl Simulation {
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

    /// Projects a unit and its adjacent campaign state for the context card.
    /// `unit` may be the presentation playback snapshot for an active animation.
    pub(crate) fn unit_context_view(&self, unit: &Unit) -> UnitContextView {
        let status = if unit.work_left > 0 {
            format!(
                "Building {} - {} turns",
                unit.work_kind
                    .map(|work| work.name())
                    .unwrap_or("improvement"),
                self.project_turns_left(unit)
            )
        } else if let Some(carrier) = unit.transport {
            self.units
                .iter()
                .find(|ship| ship.id == carrier)
                .map_or_else(
                    || "Aboard transport".to_owned(),
                    |ship| format!("Aboard {}", ship.kind.name()),
                )
        } else if unit.kind.transport_capacity() > 0 {
            format!(
                "Cargo {}/{}",
                self.cargo_count(unit.id),
                unit.kind.transport_capacity()
            )
        } else if unit.automated {
            "Automated".to_owned()
        } else {
            match unit.state {
                UnitState::Idle => "Ready".to_owned(),
                UnitState::Fortifying => "Fortifying".to_owned(),
                UnitState::Fortified => "Fortified (+50% defense)".to_owned(),
                UnitState::Sentry => "Sentry".to_owned(),
                UnitState::AirSuperiority => "Air Superiority".to_owned(),
            }
        };
        let status = if unit.kind.is_aircraft() {
            format!(
                "{status} · Fuel {}/{}",
                unit.fuel,
                unit.kind.fuel_capacity()
            )
        } else {
            status
        };
        let status = if unit.veteran > 0 {
            format!("{status} · {}", unit.rank_name())
        } else {
            status
        };
        let player = &self.players[unit.owner];
        let stats = unit.kind.stats();
        UnitContextView {
            kind: unit.kind,
            owner: unit.owner,
            civilization_id: player.civilization_id.clone(),
            era_index: self.research_era(unit.owner).index(),
            owner_name: player.name.clone(),
            terrain_name: self.tile(unit.x, unit.y).terrain.name(),
            status,
            hp: unit.hp,
            max_hp: unit.max_hit_points(),
            moves_left: unit.moves_left,
            moves_allowance: self.unit_move_allowance(unit.owner, unit.kind),
            attack: stats.attack,
            defense: stats.defense,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_context_projection_includes_rule_and_map_values() {
        let sim = Simulation::new(crate::mapgen::generate(42));
        let unit = sim.units_owned_by(HUMAN).next().unwrap();
        let view = sim.unit_context_view(unit);
        assert_eq!(view.kind, unit.kind);
        assert_eq!(view.owner, unit.owner);
        assert_eq!(view.hp, unit.hp);
        assert_eq!(view.max_hp, unit.max_hit_points());
        assert_eq!(view.moves_left, unit.moves_left);
        assert_eq!(view.attack, unit.kind.stats().attack);
        assert_eq!(view.defense, unit.kind.stats().defense);
        assert_eq!(view.terrain_name, sim.tile(unit.x, unit.y).terrain.name());
    }

    #[test]
    fn city_context_projection_uses_the_current_city_forecast() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.create_city(HUMAN, 20, 24);
        let id = sim.city_at(20, 24).unwrap().id;
        let city = sim.city_by_id(id).unwrap();
        let forecast = sim.city_forecast(city);
        let view = sim.city_context_view(id).unwrap();
        assert_eq!(
            (view.food, view.shields, view.trade),
            (forecast.food, forecast.shields, forecast.trade)
        );
        assert_eq!(view.food_store, city.food);
        assert_eq!(view.production_name, city.production.name());
        assert_eq!(view.production_cost, city.production.cost());
        assert!(sim.city_context_view(u32::MAX).is_none());
    }
}
