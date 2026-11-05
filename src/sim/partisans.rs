use super::*;

impl Simulation {
    pub(super) fn inspire_partisans(&mut self, index: usize, defender: usize) {
        let city = &self.cities[index];
        let loyal = city
            .foreign_citizens
            .iter()
            .find(|&&(nation, _)| nation == defender)
            .map_or(0, |&(_, count)| count);
        if defender == BARBARIANS
            || 100 * loyal < 75 * city.size
            || !matches!(
                self.players[defender].government,
                Government::Communism | Government::Democracy
            )
            || !self.has_tech(defender, Tech::COMMUNISM)
            || !self.has_tech(defender, Tech::GUNPOWDER)
            || !(0..self.players.len()).any(|owner| self.has_tech(owner, Tech::GUERILLA_WARFARE))
        {
            return;
        }
        let name = city.name.clone();
        let conqueror = city.owner;
        let center = Self::idx(city.x, city.y);
        let sites: Vec<_> = Self::city_radius(city)
            .map(|(_, _, x, y)| (x, y))
            .filter(|&(x, y)| {
                !self.tile(x, y).terrain.is_water()
                    && !self.cities.iter().any(|city| (city.x, city.y) == (x, y))
                    && !self.units.iter().any(|unit| (unit.x, unit.y) == (x, y))
            })
            .collect();
        let count = (self.rand_below(2 + (city.size + 1) / 2) + 1).min(8);
        let labels = self.continent_labels();
        let mut sites = sites;
        let mut placed = 0;
        for _ in 0..count {
            let mut best = None;
            for (index, &(x, y)) in sites.iter().enumerate() {
                let terrain = self.tile(x, y).terrain;
                let mut value = 10
                    * UnitKind::Partisan.stats().defense
                    * (POWER_FACTOR + terrain.defense_bonus() / (100 / POWER_FACTOR));
                if labels[Self::idx(x, y)] != labels[center] {
                    value /= 2;
                }
                value -= self.rand_below(value / 3);
                if best.is_none_or(|(_, previous)| value > previous) {
                    best = Some((index, value));
                }
            }
            let Some((index, _)) = best else { break };
            let (x, y) = sites.remove(index);
            self.spawn_unit(defender, UnitKind::Partisan, x, y);
            self.units.last_mut().unwrap().state = UnitState::Fortified;
            placed += 1;
        }
        if placed > 0 && (defender == HUMAN || conqueror == HUMAN) {
            self.add_event(format!("The loss of {name} inspired {placed} Partisans."));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resistance_requires_loyal_population_and_world_technology() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let (_, target) = game.partisan_playtest_setup();
        let index = game
            .cities
            .iter()
            .position(|city| city.id == target)
            .unwrap();
        game.cities[index].transfer_citizens(HUMAN);
        game.cities[index].size = 4;
        game.cities[index].foreign_citizens = vec![(1, 2)];
        let before = game.units.len();
        game.inspire_partisans(index, 1);
        assert_eq!(game.units.len(), before);
        game.cities[index].foreign_citizens = vec![(1, 3)];
        game.players[HUMAN]
            .known
            .retain(|&tech| tech != Tech::GUERILLA_WARFARE);
        game.inspire_partisans(index, 1);
        assert_eq!(game.units.len(), before);
        game.players[HUMAN].known.push(Tech::GUERILLA_WARFARE);
        assert!(!game.unit_available(1, UnitKind::Partisan));
        game.inspire_partisans(index, 1);
        assert!(game.units.len() > before);
    }
}
