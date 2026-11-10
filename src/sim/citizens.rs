//! Citizen roles, specialists, and population accounting.

use super::*;
use std::sync::OnceLock;

fn government_assimilation_chance(government: Government) -> i32 {
    static CHANCES: OnceLock<Vec<(String, i32)>> = OnceLock::new();
    let chances = CHANCES.get_or_init(|| {
        let mut lines = include_str!(
            "../../data/rulesets/civ3_conquests/government_assimilation.tsv"
        )
        .lines();
        assert_eq!(
            lines.next(),
            Some(
                "source_section\tsource_record_index\tgovernment\tassimilation_chance_percent\tbiq_sha256"
            ),
            "invalid Conquests government-assimilation table header"
        );
        let mut rows = Vec::new();
        let mut hash = None;
        for (index, line) in lines.enumerate() {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 5, "invalid government assimilation row");
            assert_eq!(fields[0], "GOVT");
            assert_eq!(fields[1].parse::<usize>().unwrap(), index);
            let chance = fields[3].parse::<i32>().unwrap();
            assert!((0..=100).contains(&chance));
            assert_eq!(fields[4].len(), 64);
            assert!(hash.replace(fields[4]).is_none_or(|old| old == fields[4]));
            rows.push((fields[2].to_owned(), chance));
        }
        assert_eq!(rows.len(), 8, "unexpected Conquests government count");
        assert_eq!(
            hash.as_deref(),
            Some("1bd610ab1420217f7d64a9214b60c0bc7686e4781a0d6e66590171535495609c")
        );
        rows
    });
    chances
        .iter()
        .find_map(|(name, chance)| (name == government.name()).then_some(*chance))
        .unwrap_or_else(|| {
            panic!(
                "government missing from Conquests assimilation table: {}",
                government.name()
            )
        })
}

impl City {
    pub(crate) fn domestic_citizens(&self) -> i32 {
        self.size
            - self
                .foreign_citizens
                .iter()
                .map(|&(_, count)| count)
                .sum::<i32>()
    }

    pub(crate) fn prune_foreign_nationality_tenure(&mut self) {
        self.foreign_nationality_tenure
            .retain(|&(nation, _, _)| self.foreign_citizens.iter().any(|&(n, _)| n == nation));
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

    pub(super) fn transfer_citizens(&mut self, new_owner: usize) {
        let old_owner = self.owner;
        let domestic = self.domestic_citizens();
        self.owner = new_owner;
        self.ensure_culture_civilization(new_owner);
        self.foreign_citizens
            .retain(|&(nation, _)| nation != new_owner);
        self.foreign_nationality_tenure
            .retain(|&(nation, _, _)| nation != new_owner);
        self.add_citizen_nationality(old_owner, domestic);
    }
}

impl Simulation {
    /// Record whether each extant foreign nationality spent this city turn as
    /// a minority. The Conquests Civilopedia says assimilation follows being
    /// a minority "longer than not"; the exact historic starting point is not
    /// specified, so new/legacy cohorts begin with zero recorded turns.
    pub(super) fn advance_foreign_nationality_tenure(&mut self, index: usize) {
        let city = &mut self.cities[index];
        let owner_population = city.domestic_citizens();
        city.foreign_nationality_tenure
            .retain(|&(nation, _, _)| city.foreign_citizens.iter().any(|&(n, _)| n == nation));
        for &(nationality, population) in &city.foreign_citizens {
            let nationality_is_minority = owner_population > population;
            match city
                .foreign_nationality_tenure
                .binary_search_by_key(&nationality, |&(nation, _, _)| nation)
            {
                Ok(position) => {
                    let (_, minority_turns, majority_turns) =
                        &mut city.foreign_nationality_tenure[position];
                    if nationality_is_minority {
                        *minority_turns = minority_turns.saturating_add(1);
                    } else {
                        *majority_turns = majority_turns.saturating_add(1);
                    }
                }
                Err(position) => {
                    let (minority_turns, majority_turns) = if nationality_is_minority {
                        (1, 0)
                    } else {
                        (0, 1)
                    };
                    city.foreign_nationality_tenure
                        .insert(position, (nationality, minority_turns, majority_turns));
                }
            }
        }
    }

    // City size has already changed. Any loss not covered by foreign
    // citizens comes out of the implicit domestic population.
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
            let entry = removed.iter_mut().find(|entry| entry.0 == nation).unwrap();
            entry.1 += reduction;
            if entry.1 > dominant.1 {
                dominant = *entry;
            }
        }
        foreign.sort_unstable_by_key(|&(nation, _)| nation);
        self.cities[index].foreign_citizens = foreign;
        self.cities[index].prune_foreign_nationality_tenure();
        if loss > dominant.1 {
            self.cities[index].owner
        } else {
            dominant.0
        }
    }

    pub(super) fn remove_resisters_first_nationalities(
        &mut self,
        index: usize,
        mut loss: i32,
    ) -> usize {
        let mut dominant = (self.cities[index].owner, 0);
        while loss > 0 {
            let resisting_total = self.cities[index]
                .resisting_citizens
                .iter()
                .map(|&(_, count)| count)
                .sum::<i32>();
            if resisting_total == 0 {
                break;
            }

            let mut selected = self.rand_below(resisting_total);
            let position = self.cities[index]
                .resisting_citizens
                .iter()
                .position(|&(_, count)| {
                    if selected < count {
                        true
                    } else {
                        selected -= count;
                        false
                    }
                })
                .expect("selected resister must have a nationality group");
            let nationality = self.cities[index].resisting_citizens[position].0;
            self.cities[index].resisting_citizens[position].1 -= 1;
            if self.cities[index].resisting_citizens[position].1 == 0 {
                self.cities[index].resisting_citizens.remove(position);
            }
            let foreign_position = self.cities[index]
                .foreign_citizens
                .binary_search_by_key(&nationality, |&(nation, _)| nation)
                .expect("every resisting citizen must have a foreign nationality");
            self.cities[index].foreign_citizens[foreign_position].1 -= 1;
            if self.cities[index].foreign_citizens[foreign_position].1 == 0 {
                self.cities[index].foreign_citizens.remove(foreign_position);
            }

            if dominant.1 == 0 {
                dominant = (nationality, 1);
            }
            loss -= 1;
        }

        if loss > 0 {
            let remaining_majority = self.remove_citizen_nationalities(index, loss);
            if dominant.1 == 0 {
                dominant.0 = remaining_majority;
            }
        }
        self.cities[index].prune_foreign_nationality_tenure();
        dominant.0
    }

    pub(super) fn assimilate_citizen(&mut self, index: usize) {
        let city = &self.cities[index];
        if city.foreign_citizens.is_empty() {
            return;
        }

        // The Conquests Civilopedia identifies the majority nationality as the
        // assimilation target. A Firaxis executive producer's explanation of
        // the base-game rule says a captured nationality cannot assimilate
        // until the city's current nationality is the strict majority. Keep
        // that target rather than allowing a foreign nationality to absorb
        // another foreign group.
        let owner = city.owner;
        if city.domestic_citizens() * 2 <= city.size {
            return;
        }

        let chance = government_assimilation_chance(self.players[owner].government);
        let candidates = self.cities[index]
            .foreign_citizens
            .iter()
            .copied()
            .filter(|(nationality, _)| {
                self.cities[index]
                    .foreign_nationality_tenure
                    .binary_search_by_key(nationality, |&(nation, _, _)| nation)
                    .ok()
                    .is_some_and(|position| {
                        let (_, minority_turns, majority_turns) =
                            self.cities[index].foreign_nationality_tenure[position];
                        minority_turns > majority_turns
                    })
            })
            .collect::<Vec<_>>();
        let mut conversions = Vec::new();
        for (nationality, population) in candidates {
            let converted = (0..population)
                .filter(|_| self.rand_below(100) < chance)
                .count() as i32;
            if converted > 0 {
                conversions.push((nationality, converted));
            }
        }

        let city = &mut self.cities[index];
        for (nationality, converted) in conversions {
            if let Ok(position) = city
                .foreign_citizens
                .binary_search_by_key(&nationality, |&(nation, _)| nation)
            {
                city.foreign_citizens[position].1 -= converted;
                if city.foreign_citizens[position].1 == 0 {
                    city.foreign_citizens.remove(position);
                    if let Ok(tenure_position) = city
                        .foreign_nationality_tenure
                        .binary_search_by_key(&nationality, |&(nation, _, _)| nation)
                    {
                        city.foreign_nationality_tenure.remove(tenure_position);
                    }
                }
            }
            // Successful residents assimilate into the city's current owner
            // nationality, which is the city nationality in this model.
            debug_assert_ne!(nationality, owner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_population_loss_and_assimilation_preserve_counts() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(1, 10, 20);
        let city = &mut game.cities[0];
        city.size = 4;
        city.add_citizen_nationality(HUMAN, 1);
        city.add_culture(1, 125);
        city.transfer_citizens(HUMAN);
        assert_eq!(city.foreign_citizens, [(1, 3)]);
        assert_eq!(city.domestic_citizens(), 1);
        assert_eq!(city.culture_for_civilization(1), 125);
        assert_eq!(city.culture_for_civilization(HUMAN), 0);
        city.add_culture(HUMAN, 40);
        city.size -= 1;
        assert_eq!(game.remove_citizen_nationalities(0, 1), 1);
        assert_eq!(game.cities[0].foreign_citizens, [(1, 2)]);
        // Assimilation is exercised separately with the BIQ government chance
        // and majority-nationality target below.
        assert_eq!(game.cities[0].domestic_citizens(), 1);
        assert_eq!(game.cities[0].culture_for_civilization(1), 125);
        assert_eq!(game.cities[0].culture_for_civilization(HUMAN), 40);
        assert_eq!(game.cities[0].total_culture(), 165);
    }

    #[test]
    fn starvation_removes_resisting_foreign_citizens_first() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let city = &mut game.cities[0];
        city.size = 4;
        city.foreign_citizens = vec![(1, 1), (2, 2)];
        city.resisting_citizens = vec![(2, 1)];
        city.size -= 1;

        game.remove_resisters_first_nationalities(0, 1);

        assert_eq!(game.cities[0].foreign_citizens, [(1, 1), (2, 1)]);
        assert!(game.cities[0].resisting_citizens.is_empty());
    }

    #[test]
    fn assimilation_uses_biq_chances_and_requires_owner_majority_and_tenure() {
        assert_eq!(government_assimilation_chance(Government::Anarchy), 1);
        assert_eq!(government_assimilation_chance(Government::Despotism), 1);
        assert_eq!(government_assimilation_chance(Government::Monarchy), 2);
        assert_eq!(government_assimilation_chance(Government::Communism), 4);
        assert_eq!(government_assimilation_chance(Government::Republic), 2);
        assert_eq!(government_assimilation_chance(Government::Democracy), 4);
        assert_eq!(government_assimilation_chance(Government::Fascism), 2);
        assert_eq!(government_assimilation_chance(Government::Feudalism), 3);

        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.players[HUMAN].government = Government::Democracy;
        game.create_city(HUMAN, 10, 20);
        game.cities[0].size = 5;
        game.cities[0].foreign_citizens = vec![(1, 3), (2, 1)];
        game.cities[0].foreign_nationality_tenure = vec![(1, 100, 0), (2, 100, 0)];

        // Choose a seed whose first per-citizen roll falls within the BIQ's
        // 4-percent Democracy chance.
        let seed = (0..100_000)
            .find(|&seed| {
                game.set_random_seed(seed);
                game.rand_below(100) < government_assimilation_chance(Government::Democracy)
            })
            .expect("find successful deterministic assimilation roll");
        game.set_random_seed(seed);
        game.assimilate_citizen(0);
        // A foreign nationality can outnumber another foreign nationality,
        // but assimilation waits until the city's own nationality is a strict
        // majority, as in Firaxis's documented 4-versus-3 example.
        assert_eq!(game.cities[0].foreign_citizens, [(1, 3), (2, 1)]);

        game.cities[0].foreign_citizens = vec![(2, 1)];
        game.cities[0].foreign_nationality_tenure = vec![(2, 100, 0)];
        game.set_random_seed(seed);
        game.assimilate_citizen(0);
        assert!(game.cities[0].foreign_citizens.is_empty());

        // A citizen gets an independent roll: select a seed with two
        // consecutive successful rolls and confirm both foreign citizens can
        // assimilate in the same city turn.
        let double_success_seed = (0..100_000)
            .find(|&seed| {
                game.set_random_seed(seed);
                game.rand_below(100) < 4 && game.rand_below(100) < 4
            })
            .expect("find deterministic pair of assimilation successes");
        game.cities[0].foreign_citizens = vec![(1, 1), (2, 1)];
        game.cities[0].foreign_nationality_tenure = vec![(1, 100, 0), (2, 100, 0)];
        game.set_random_seed(double_success_seed);
        game.assimilate_citizen(0);
        assert!(game.cities[0].foreign_citizens.is_empty());

        // A tie between the city nationality and a foreign nationality also
        // prevents assimilation.
        game.cities[0].size = 4;
        game.cities[0].foreign_citizens = vec![(2, 2)];
        game.cities[0].foreign_nationality_tenure = vec![(2, 100, 0)];
        game.set_random_seed(seed);
        game.assimilate_citizen(0);
        assert_eq!(game.cities[0].foreign_citizens, [(2, 2)]);
    }

    #[test]
    fn assimilation_waits_until_foreign_nationality_spent_more_turns_as_minority() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.players[HUMAN].government = Government::Democracy;
        game.create_city(HUMAN, 10, 20);
        game.cities[0].size = 5;
        game.cities[0].foreign_citizens = vec![(1, 1)];
        game.cities[0].foreign_nationality_tenure = vec![(1, 1, 2)];

        // At an equal history, even a successful random roll cannot assimilate.
        game.advance_foreign_nationality_tenure(0);
        assert_eq!(game.cities[0].foreign_nationality_tenure, [(1, 2, 2)]);
        let success_seed = (0..100_000)
            .find(|&seed| {
                game.set_random_seed(seed);
                game.rand_below(100) < 4
            })
            .unwrap();
        game.set_random_seed(success_seed);
        game.assimilate_citizen(0);
        assert_eq!(game.cities[0].foreign_citizens, [(1, 1)]);

        game.advance_foreign_nationality_tenure(0);
        assert_eq!(game.cities[0].foreign_nationality_tenure, [(1, 3, 2)]);
        game.assimilate_citizen(0);
        assert!(game.cities[0].foreign_citizens.is_empty());
    }

    #[test]
    fn city_turn_advances_persisted_nationality_tenure_once() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Mountains;
            tile.resource = None;
            tile.river = false;
        }
        let (x, y) = (10, 20);
        let work_tiles =
            [(1, 0), (0, 1), (-1, 0)].map(|(dx, dy)| offset_native_by_map(x, y, dx, dy));
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        for (wx, wy) in work_tiles {
            game.tile_mut(wx, wy).terrain = Terrain::Grassland;
        }
        game.create_city(HUMAN, x, y);
        game.cities[0].size = 4;
        game.cities[0].worked_tiles = work_tiles.to_vec();
        game.cities[0].specialists = [0; 3];
        game.cities[0].food = Simulation::growth_box(4) / 2;
        game.cities[0].foreign_citizens = vec![(1, 1)];
        game.cities[0].foreign_nationality_tenure = vec![(1, 0, 20)];

        game.process_player_cities(HUMAN);

        assert_eq!(
            game.cities[0].foreign_nationality_tenure,
            [(1, 1, 20)],
            "size={}, foreign={:?}",
            game.cities[0].size,
            game.cities[0].foreign_citizens
        );
        assert_eq!(game.cities[0].foreign_citizens, [(1, 1)]);
    }
}
