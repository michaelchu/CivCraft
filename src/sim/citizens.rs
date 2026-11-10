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
        self.add_citizen_nationality(old_owner, domestic);
    }
}

impl Simulation {
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
        if loss > dominant.1 {
            self.cities[index].owner
        } else {
            dominant.0
        }
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
        let candidates = self.cities[index].foreign_citizens.clone();
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
    fn assimilation_uses_conquests_government_chance_and_requires_owner_majority() {
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
        game.set_random_seed(double_success_seed);
        game.assimilate_citizen(0);
        assert!(game.cities[0].foreign_citizens.is_empty());

        // A tie between the city nationality and a foreign nationality also
        // prevents assimilation.
        game.cities[0].size = 4;
        game.cities[0].foreign_citizens = vec![(2, 2)];
        game.set_random_seed(seed);
        game.assimilate_citizen(0);
        assert_eq!(game.cities[0].foreign_citizens, [(2, 2)]);
    }
}
