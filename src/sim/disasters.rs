//! Random and rules-driven disasters affecting cities and terrain.

#[cfg(test)]
use super::terrain_work::neighbors;
use super::*;
use crate::rules::Genus;

#[derive(Clone, Copy)]
enum Disaster {
    Earthquake,
    Flood,
    Fire,
    IndustrialAccident,
    NuclearAccident,
    Robbery,
}

impl Disaster {
    const ALL: [Self; 6] = [
        Self::Earthquake,
        Self::Flood,
        Self::Fire,
        Self::IndustrialAccident,
        Self::NuclearAccident,
        Self::Robbery,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Earthquake => "Earthquake",
            Self::Flood => "Flood",
            Self::Fire => "Fire",
            Self::IndustrialAccident => "Industrial accident",
            Self::NuclearAccident => "Nuclear accident",
            Self::Robbery => "Robbery",
        }
    }
}

impl Simulation {
    pub(super) fn check_disasters(&mut self) {
        let mut cities: Vec<_> = (0..self.cities.len()).collect();
        cities.sort_by_key(|&index| self.cities[index].owner);
        for index in cities {
            for disaster in Disaster::ALL {
                // Each Civ2Civ3 disaster has frequency 10 and the ruleset
                // sets the disaster multiplier to 100.
                if self.rand_below(1_000_000) < 100 * 10 && self.disaster_possible(index, disaster)
                {
                    self.apply_disaster(index, disaster);
                }
            }
        }
    }

    fn disaster_possible(&self, index: usize, disaster: Disaster) -> bool {
        let city = &self.cities[index];
        match disaster {
            Disaster::Earthquake => self.tile(city.x, city.y).terrain == Terrain::Hills,
            Disaster::Flood => {
                self.tile(city.x, city.y).river
                    || self
                        .map_wrap
                        .civ3_neighbors(city.x, city.y)
                        .any(|(x, y)| self.tile(x, y).river)
            }
            Disaster::Fire => city.size >= 9,
            Disaster::IndustrialAccident => city.has_building(Building::MFG_PLANT),
            Disaster::NuclearAccident => city.has_building(Building::NUCLEAR_PLANT),
            Disaster::Robbery => true,
        }
    }

    fn apply_disaster(&mut self, index: usize, disaster: Disaster) {
        let city = self.cities[index].clone();
        let detail = match disaster {
            Disaster::Earthquake => {
                let city = &mut self.cities[index];
                let loss = city.shields;
                city.shields = 0;
                city.protected_shields = 0;
                city.production_before_switch = None;
                format!("{loss} shields lost.")
            }
            Disaster::Flood => {
                self.cities[index].food = 0;
                format!("{} stored food lost.", city.food)
            }
            Disaster::Fire => {
                let mut targets: Vec<_> = city
                    .buildings
                    .iter()
                    .copied()
                    .filter(|building| building.data().genus == Genus::Improvement)
                    .collect();
                targets.sort_by_key(|building| building.0);
                if targets.is_empty() {
                    String::new()
                } else {
                    let building = targets[self.rand_below(targets.len() as i32) as usize];
                    self.cities[index]
                        .buildings
                        .retain(|&built| built != building);
                    format!("{} destroyed.", building.data().name)
                }
            }
            Disaster::IndustrialAccident | Disaster::NuclearAccident => {
                let contamination = if matches!(disaster, Disaster::IndustrialAccident) {
                    Contamination::Pollution
                } else {
                    Contamination::Fallout
                };
                let placed = self.place_contamination(&city, contamination);
                let lost_population = city.size > 1;
                if lost_population {
                    let city = &mut self.cities[index];
                    city.size -= 1;
                    city.remove_specialists(1);
                    city.food = city.food.min(Self::growth_box(city.size));
                    self.remove_citizen_nationalities(index, 1);
                    self.reconcile_city_workers();
                }
                match (placed, lost_population) {
                    (true, true) => format!(
                        "One citizen lost and {} appeared nearby.",
                        contamination.name()
                    ),
                    (true, false) => format!("{} appeared nearby.", contamination.name()),
                    (false, true) => "One citizen lost.".to_owned(),
                    (false, false) => String::new(),
                }
            }
            Disaster::Robbery => {
                let gold = self.players[city.owner].gold;
                let loss = gold.min(self.city_forecast(&city).trade * 5).max(0);
                self.players[city.owner].gold -= loss;
                format!("{loss} gold stolen.")
            }
        };
        if city.owner == HUMAN {
            self.add_event(
                format!("{} struck {}. {detail}", disaster.name(), city.name)
                    .trim_end()
                    .to_owned(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disasters_check_local_conditions_and_protect_wonders() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let city = &mut game.cities[0];
        city.size = 9;
        city.food = 12;
        city.shields = 17;
        city.protected_shields = 10;
        city.buildings
            .extend([Building::PYRAMIDS, Building::GRANARY]);
        game.tile_mut(10, 20).terrain = Terrain::Hills;
        let (nx, ny) = neighbors(10, 20).next().unwrap();
        game.tile_mut(nx, ny).river = true;
        assert!(game.disaster_possible(0, Disaster::Earthquake));
        assert!(game.disaster_possible(0, Disaster::Flood));
        assert!(game.disaster_possible(0, Disaster::Fire));
        assert!(!game.disaster_possible(0, Disaster::IndustrialAccident));
        assert!(!game.disaster_possible(0, Disaster::NuclearAccident));
        game.apply_disaster(0, Disaster::Earthquake);
        game.apply_disaster(0, Disaster::Flood);
        game.apply_disaster(0, Disaster::Fire);
        assert_eq!(game.cities[0].shields, 0);
        assert_eq!(game.cities[0].protected_shields, 0);
        assert_eq!(game.cities[0].food, 0);
        assert!(game.cities[0].has_building(Building::PYRAMIDS));
        assert!(!game.cities[0].has_building(Building::GRANARY));
        game.players[HUMAN].gold = 1000;
        let loss = game.city_forecast(&game.cities[0]).trade * 5;
        game.apply_disaster(0, Disaster::Robbery);
        assert_eq!(game.players[HUMAN].gold, 1000 - loss);
    }
}
