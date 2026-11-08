//! City illness and population health effects.

use super::*;

impl Simulation {
    /// Return the current chance of a plague in tenths of one percent.
    pub(crate) fn city_illness(&self, city: &City, shields: i32) -> i32 {
        if city.size <= 5 {
            return 0;
        }
        let size_illness = ((1.0 - (-f64::from(city.size - 5) / 10.0).exp()) * 100.0) as i32;
        let pollution_illness = self.pollution_risk(city, shields) / 2;
        let aqueduct = [
            Building::AQUEDUCT,
            Building::AQUEDUCT_RIVER,
            Building::AQUEDUCT_LAKE,
        ]
        .into_iter()
        .any(|building| city.has_building(building));
        let health = 30
            * (i32::from(aqueduct)
                + i32::from(city.has_building(Building::SEWER_SYSTEM))
                + i32::from(self.has_tech(city.owner, Tech::MEDICINE)))
            + 10 * i32::from(self.has_active_wonder(city.owner, Building::CURE_FOR_CANCER));
        percent_of(
            size_illness + pollution_illness + self.trade_illness(city),
            100 - health,
        )
        .clamp(0, 999)
    }

    pub(super) fn check_city_plague(&mut self, index: usize, shields: i32) -> bool {
        let risk = self.city_illness(&self.cities[index], shields);
        if self.rand_below(1000) >= risk {
            return false;
        }
        let city = &mut self.cities[index];
        city.size -= 1;
        city.last_plague = Some(self.turn);
        city.remove_citizen_assignments(1, self.map_wrap);
        city.food = city.food.min(Self::growth_box(city.size));
        if city.owner == HUMAN {
            let name = city.name.clone();
            self.add_event(format!("Plague struck {name}. One citizen died."));
        }
        self.remove_citizen_nationalities(index, 1);
        self.rebuild_city_ownership_for_city_event(self.cities[index].id);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn illness_combines_population_pollution_and_nonstacking_aqueducts() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let mut city = game.cities[0].clone();
        city.size = 5;
        assert_eq!(game.city_illness(&city, 100), 0);
        city.size = 9;
        assert_eq!(game.city_illness(&city, 0), 32);
        assert_eq!(game.city_illness(&city, 100), 72);
        city.buildings.extend([
            Building::AQUEDUCT,
            Building::AQUEDUCT_RIVER,
            Building::AQUEDUCT_LAKE,
        ]);
        assert_eq!(game.city_illness(&city, 100), 50);
        city.buildings.push(Building::SEWER_SYSTEM);
        game.players[HUMAN].known.push(Tech::MEDICINE);
        assert_eq!(game.city_illness(&city, 100), 7);
        game.cities[0].buildings.push(Building::CURE_FOR_CANCER);
        assert_eq!(game.city_illness(&city, 100), 0);
    }
}
