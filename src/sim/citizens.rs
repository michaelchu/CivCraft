//! Citizen nationality counts and population changes.

use super::*;

impl City {
    pub(crate) fn domestic_citizens(&self) -> i32 {
        self.size
            - self
                .foreign_citizens
                .iter()
                .map(|&(_, count)| count)
                .sum::<i32>()
    }

    pub(super) fn add_citizen_nationality(&mut self, nationality: usize, count: i32) {
        if nationality == self.owner || count == 0 {
            return;
        }
        match self
            .foreign_citizens
            .binary_search_by_key(&nationality, |&(nation, _)| nation)
        {
            Ok(index) => self.foreign_citizens[index].1 += count,
            Err(index) => self.foreign_citizens.insert(index, (nationality, count)),
        }
    }

    /// Preserve each nationality cohort when city ownership changes. The new
    /// owner's existing cohort becomes domestic; the former owner's domestic
    /// residents become a foreign cohort.
    pub(super) fn transfer_citizens(&mut self, new_owner: usize) {
        let old_owner = self.owner;
        let domestic = self.domestic_citizens();
        self.owner = new_owner;
        self.foreign_citizens
            .retain(|&(nation, _)| nation != new_owner);
        self.add_citizen_nationality(old_owner, domestic);
    }

    fn convert_foreign_citizens_to_owner(&mut self, percent: i32) {
        if percent == 0 {
            return;
        }
        let foreign = self.foreign_citizens.clone();
        for (nationality, count) in foreign {
            let convert = (count.saturating_mul(percent) / 100).max(1).min(count);
            if let Ok(index) = self
                .foreign_citizens
                .binary_search_by_key(&nationality, |&(nation, _)| nation)
            {
                self.foreign_citizens[index].1 -= convert;
                if self.foreign_citizens[index].1 == 0 {
                    self.foreign_citizens.remove(index);
                }
            }
        }
    }
}

impl Simulation {
    /// Freeciv's cityturn phase rolls once for every city before city activity,
    /// then converts one randomly selected foreign resident on success.
    pub(super) fn convert_city_citizen_for_turn(&mut self, city_index: usize) {
        if !crate::sim::migration::selected_citizen_nationality() {
            return;
        }
        let speed = crate::sim::migration::selected_integer_parameter(
            "cities.ruleset",
            "citizen",
            "convert_speed",
        );
        if self.rand_below(1000) + 1 > speed {
            return;
        }
        let foreign: Vec<_> = self.cities[city_index]
            .foreign_citizens
            .iter()
            .filter(|&&(_, count)| count > 0)
            .map(|&(nationality, _)| nationality)
            .collect();
        if foreign.is_empty() {
            return;
        }
        let selected = self.rand_below(foreign.len() as i32) as usize;
        let nationality = foreign[selected];
        if let Ok(index) = self.cities[city_index]
            .foreign_citizens
            .binary_search_by_key(&nationality, |&(nation, _)| nation)
        {
            self.cities[city_index].foreign_citizens[index].1 -= 1;
            if self.cities[city_index].foreign_citizens[index].1 == 0 {
                self.cities[city_index].foreign_citizens.remove(index);
            }
        }
    }

    /// Apply C2C3's optional nationality conversion after a city conquest.
    /// `cities.ruleset [citizen].conquest_convert_pct` is zero by default.
    pub(super) fn convert_conquest_citizens(&mut self, city_index: usize) {
        if !crate::sim::migration::selected_citizen_nationality() {
            return;
        }
        let percent = crate::sim::migration::selected_integer_parameter(
            "cities.ruleset",
            "citizen",
            "conquest_convert_pct",
        );
        self.cities[city_index].convert_foreign_citizens_to_owner(percent);
    }

    /// Apply a population loss to foreign nationality cohorts. Any remaining
    /// loss comes from the implicit city-owner cohort.
    pub(super) fn remove_citizen_nationalities(&mut self, index: usize, mut loss: i32) -> usize {
        let mut foreign = self.cities[index].foreign_citizens.clone();
        let mut removed = Vec::<(usize, i32)>::new();
        let mut dominant = (self.cities[index].owner, 0);
        while loss > 0 && !foreign.is_empty() {
            let selected = self.rand_below(foreign.len() as i32) as usize;
            let (nation, count) = foreign[selected];
            let reduction = loss.min((count / 2).max(1));
            foreign[selected].1 -= reduction;
            if foreign[selected].1 == 0 {
                foreign.swap_remove(selected);
            }
            loss -= reduction;
            if !removed.iter().any(|&(owner, _)| owner == nation) {
                removed.push((nation, 0));
            }
            let entry = removed
                .iter_mut()
                .find(|entry| entry.0 == nation)
                .expect("removed nationality was inserted");
            entry.1 += reduction;
            if entry.1 > dominant.1 {
                dominant = *entry;
            }
        }
        foreign.sort_unstable_by_key(|&(nation, _)| nation);
        self.cities[index].foreign_citizens = foreign;
        if loss > dominant.1 {
            self.cities[index].owner
        } else {
            dominant.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_preserves_foreign_nationality_cohorts() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(1, 10, 20);
        let city = &mut game.cities[0];
        city.size = 4;
        city.add_citizen_nationality(HUMAN, 1);

        city.transfer_citizens(HUMAN);

        assert_eq!(city.owner, HUMAN);
        assert_eq!(city.foreign_citizens, [(1, 3)]);
        assert_eq!(city.domestic_citizens(), 1);
        assert_eq!(
            city.domestic_citizens() + city.foreign_citizens.iter().map(|&(_, n)| n).sum::<i32>(),
            city.size
        );
    }

    #[test]
    fn conquest_conversion_uses_selected_c2c3_percent_and_is_zero_by_default() {
        assert_eq!(
            crate::sim::migration::selected_integer_parameter(
                "cities.ruleset",
                "citizen",
                "conquest_convert_pct"
            ),
            0
        );

        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(1, 10, 20);
        game.cities[0].size = 10;
        game.cities[0].foreign_citizens = vec![(2, 4), (3, 3)];
        game.cities[0].transfer_citizens(HUMAN);
        let before = game.cities[0].foreign_citizens.clone();
        game.convert_conquest_citizens(0);
        assert_eq!(game.cities[0].foreign_citizens, before);

        game.cities[0].convert_foreign_citizens_to_owner(25);
        assert_eq!(game.cities[0].foreign_citizens, [(1, 2), (2, 3), (3, 2)]);
        assert_eq!(game.cities[0].domestic_citizens(), 3);
    }

    #[test]
    fn city_turn_conversion_uses_c2c3_probability_and_foreign_nationality_order() {
        assert_eq!(
            crate::sim::migration::selected_integer_parameter(
                "cities.ruleset",
                "citizen",
                "convert_speed"
            ),
            50
        );
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.create_city(HUMAN, 10, 20);
        game.cities[0].size = 7;
        game.cities[0].foreign_citizens = vec![(1, 4), (2, 3)];

        let mut expected = game.clone();
        let seed = (1..=1000)
            .find(|&seed| {
                expected.set_random_seed(seed);
                expected.rand_below(1000) < 50
            })
            .expect("one in twenty city conversion roll");
        game.set_random_seed(seed);
        expected.set_random_seed(seed);
        expected.rand_below(1000);
        let nationality = [1, 2][expected.rand_below(2) as usize];
        let index = expected.cities[0]
            .foreign_citizens
            .binary_search_by_key(&nationality, |&(nation, _)| nation)
            .unwrap();
        expected.cities[0].foreign_citizens[index].1 -= 1;

        game.convert_city_citizen_for_turn(0);

        assert_eq!(
            game.cities[0].foreign_citizens,
            expected.cities[0].foreign_citizens
        );
        assert_eq!(game.random_snapshot(), expected.random_snapshot());
    }

    #[test]
    fn city_turn_conversion_rolls_even_when_no_foreign_citizens_exist() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.create_city(HUMAN, 10, 20);
        game.set_random_seed(42);
        let mut expected = game.clone();
        expected.rand_below(1000);

        game.convert_city_citizen_for_turn(0);

        assert_eq!(game.random_snapshot(), expected.random_snapshot());
        assert!(game.cities[0].foreign_citizens.is_empty());
    }

    #[test]
    fn population_loss_updates_foreign_cohort_counts() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        game.cities[0].size = 4;
        game.cities[0].foreign_citizens = vec![(1, 2), (2, 1)];
        game.set_random_seed(42);

        game.cities[0].size -= 1;
        game.remove_citizen_nationalities(0, 1);

        assert_eq!(
            game.cities[0]
                .foreign_citizens
                .iter()
                .map(|&(_, n)| n)
                .sum::<i32>(),
            2
        );
        assert_eq!(
            game.cities[0].domestic_citizens()
                + game.cities[0]
                    .foreign_citizens
                    .iter()
                    .map(|&(_, n)| n)
                    .sum::<i32>(),
            game.cities[0].size
        );
    }
}
