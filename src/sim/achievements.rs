use super::*;

#[derive(Clone, Copy)]
pub(super) enum Achievement {
    Spaceship,
    EntireMap,
    LandAhoy,
    Literate,
    Metropolis,
    CulturedCity,
    CulturedNation,
    Multicultural,
}

impl Achievement {
    fn details(self) -> (&'static str, i32) {
        match self {
            Self::Spaceship => ("Spaceship Launch", 5000),
            Self::EntireMap => ("Entire Map Known", 1000),
            Self::LandAhoy => ("Land Ahoy", 100),
            Self::Literate => ("Literate", 1000),
            Self::Metropolis => ("Metropolis", 1000),
            Self::CulturedCity => ("Cultured City", 1000),
            Self::Multicultural => ("Multicultural", 1000),
            Self::CulturedNation => ("Cultured Nation", 0),
        }
    }

    fn bit(self) -> u8 {
        1 << self as u8
    }
}

impl Simulation {
    pub(super) fn award_achievement(&mut self, owner: usize, achievement: Achievement) {
        let score = &mut self.nation_scores[owner];
        if score.achievements & achievement.bit() != 0 {
            return;
        }
        score.achievements |= achievement.bit();
        let (name, culture) = achievement.details();
        score.achievement_culture += culture;
        if owner == HUMAN {
            self.add_event(format!("Achievement earned: {name}."));
        }
    }

    pub(super) fn continent_labels(&self) -> Vec<usize> {
        let mut labels = vec![0; self.tiles.len()];
        let mut continent = 0;
        for index in 0..self.tiles.len() {
            if labels[index] != 0 || self.tiles[index].terrain.is_water() {
                continue;
            }
            continent += 1;
            labels[index] = continent;
            let mut pending = vec![self.map_wrap.position(index)];
            while let Some((x, y)) = pending.pop() {
                for (nx, ny) in self.map_wrap.neighbors(x, y) {
                    let index = self.map_wrap.index(nx, ny);
                    if labels[index] == 0 && !self.tiles[index].terrain.is_water() {
                        labels[index] = continent;
                        pending.push((nx, ny));
                    }
                }
            }
        }
        labels
    }

    fn literacy(&self, owner: usize) -> i32 {
        let mut population = 0;
        let mut literate = 0;
        for city in self.cities.iter().filter(|city| city.owner == owner) {
            let pop = city.size * (city.size + 1) * 5;
            population += pop;
            let bonus = self.science_building_bonus(city)
                - 50 * i32::from(self.players[owner].government == Government::Fundamentalism);
            literate += percent_of(pop, bonus.clamp(0, 100));
        }
        match population {
            0 => 0,
            1..=9999 => literate * 100 / population,
            _ => literate / (population / 100),
        }
    }

    pub(super) fn check_achievements(&mut self) {
        use Achievement::*;
        let labels = self.continent_labels();
        for owner in 0..self.players.len() {
            if owner == BARBARIANS || !self.player_alive(owner) {
                continue;
            }
            let map_known = self.tiles.iter().all(|tile| tile.known_by[owner]);
            let mut seen_land = self
                .tiles
                .iter()
                .zip(&labels)
                .filter(|(tile, label)| **label > 0 && tile.known_by[owner])
                .map(|(_, &label)| label);
            let land_ahoy = seen_land
                .next()
                .is_some_and(|first| seen_land.any(|label| label != first));
            let cities: Vec<_> = self
                .cities
                .iter()
                .filter(|city| city.owner == owner)
                .collect();
            let mut nationalities = cities.iter().flat_map(|city| {
                city.foreign_citizens
                    .iter()
                    .map(|&(nation, _)| nation)
                    .chain((city.domestic_citizens() > 0).then_some(owner))
            });
            let multicultural = nationalities
                .next()
                .is_some_and(|first| nationalities.any(|nation| nation != first));
            let metropolis = cities.iter().any(|city| city.size >= 20);
            let cultured_city = cities.iter().any(|city| city.history >= 1000);
            let history: i32 = cities.iter().map(|city| city.history).sum();
            for (earned, achievement) in [
                (map_known, EntireMap),
                (land_ahoy, LandAhoy),
                (self.literacy(owner) >= 100, Literate),
                (metropolis, Metropolis),
                (multicultural, Multicultural),
                (cultured_city, CulturedCity),
            ] {
                if earned {
                    self.award_achievement(owner, achievement);
                }
            }
            if history + self.nation_scores[owner].achievement_culture >= 10_000 {
                self.award_achievement(owner, CulturedNation);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn culture_and_literacy_achievements_are_awarded_once() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let city = &mut game.cities[0];
        city.history = 9000;
        city.size = 20;
        city.buildings
            .extend([Building::LIBRARY, Building::UNIVERSITY]);
        assert_eq!(game.literacy(HUMAN), 100);
        game.check_achievements();
        let score = &game.nation_scores[HUMAN];
        let required = Achievement::Literate.bit()
            | Achievement::Metropolis.bit()
            | Achievement::CulturedCity.bit()
            | Achievement::CulturedNation.bit();
        assert_eq!(score.achievements & required, required);
        assert!(score.achievement_culture >= 3000);
        let culture = score.achievement_culture;
        game.check_achievements();
        assert_eq!(game.nation_scores[HUMAN].achievement_culture, culture);
    }
}
