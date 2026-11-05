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

    pub(super) fn transfer_citizens(&mut self, new_owner: usize) {
        let old_owner = self.owner;
        let domestic = self.domestic_citizens();
        self.owner = new_owner;
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
        let chance = self.rand_below(1000);
        let count = self.cities[index].foreign_citizens.len();
        if chance >= 50 || count == 0 {
            return;
        }
        let selected = self.rand_below(count as i32) as usize;
        let foreign = &mut self.cities[index].foreign_citizens;
        foreign[selected].1 -= 1;
        if foreign[selected].1 == 0 {
            foreign.remove(selected);
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
        city.transfer_citizens(HUMAN);
        assert_eq!(city.foreign_citizens, [(1, 3)]);
        assert_eq!(city.domestic_citizens(), 1);
        city.size -= 1;
        assert_eq!(game.remove_citizen_nationalities(0, 1), 1);
        assert_eq!(game.cities[0].foreign_citizens, [(1, 2)]);
        game.rng = 109;
        game.assimilate_citizen(0);
        assert_eq!(game.cities[0].foreign_citizens, [(1, 1)]);
        assert_eq!(game.cities[0].domestic_citizens(), 2);
    }
}
