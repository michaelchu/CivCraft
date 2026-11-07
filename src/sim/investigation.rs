use super::*;

pub(crate) struct CityInvestigation {
    pub(crate) city: City,
    pub(crate) forecast: super::economy::CityForecast,
    pub(crate) stationed: Vec<Unit>,
    pub(crate) supported: Vec<Unit>,
}

impl Simulation {
    pub(crate) fn can_investigate_city(&self, unit: &Unit, city: &City) -> bool {
        (unit.kind.is_diplomatic() || unit.kind == UnitKind::Explorer)
            && unit.moves_left >= 1
            && city.owner != unit.owner
            && self.map_distance(unit.x, unit.y, city.x, city.y) <= 1
    }

    pub(crate) fn investigate_city(
        &mut self,
        unit_id: u32,
        city_id: u32,
    ) -> Option<CityInvestigation> {
        if self.game_over.is_some() {
            return None;
        }
        let index = self
            .units
            .iter()
            .position(|unit| unit.id == unit_id && unit.owner == HUMAN)?;
        let city = self.city_by_id(city_id)?;
        if !self.can_investigate_city(&self.units[index], city) {
            return None;
        }
        let report = CityInvestigation {
            city: city.clone(),
            forecast: self.city_forecast(city),
            stationed: self
                .units
                .iter()
                .filter(|unit| (unit.x, unit.y) == (city.x, city.y))
                .cloned()
                .collect(),
            supported: self
                .units
                .iter()
                .filter(|unit| unit.home_city == Some(city_id))
                .cloned()
                .collect(),
        };
        self.units[index].moves_left -= 1;
        self.add_event(format!("Your unit investigated {}.", report.city.name));
        Some(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn investigation_reveals_supported_and_stationed_units_for_one_movement_fragment() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let diplomat = game.embassy_playtest_setup();
        let city = game.cities[0].clone();
        let (x, y) = offset_native_by_map(city.x, city.y, 2, 0);
        let foreign = game.spawn_unit(1, UnitKind::Warrior, x, y);
        let guard = game.spawn_unit(1, UnitKind::Phalanx, city.x, city.y);
        game.units
            .iter_mut()
            .find(|unit| unit.id == foreign)
            .unwrap()
            .home_city = Some(city.id);
        assert!(game.investigate_city(foreign, city.id).is_none());
        let investigator = game
            .units
            .iter_mut()
            .find(|unit| unit.id == diplomat)
            .unwrap();
        investigator.moves_left = 1;
        let report = game.investigate_city(diplomat, city.id).unwrap();
        assert_eq!(report.city.buildings, city.buildings);
        assert_eq!(
            report
                .stationed
                .iter()
                .map(|unit| unit.id)
                .collect::<Vec<_>>(),
            [guard]
        );
        assert_eq!(
            report
                .supported
                .iter()
                .map(|unit| unit.id)
                .collect::<Vec<_>>(),
            [foreign]
        );
        assert_eq!(
            game.units
                .iter()
                .find(|unit| unit.id == diplomat)
                .unwrap()
                .moves_left,
            0
        );
        assert!(game.investigate_city(diplomat, city.id).is_none());
    }
}
