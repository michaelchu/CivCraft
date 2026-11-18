//! Player controlled citizen assignments, based on Freeciv's CMA.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GovernorSettings {
    // Freeciv output order: food, shields, trade, gold, luxury, science.
    pub(crate) minimum: [i32; 6],
    pub(crate) weight: [i32; 6],
    pub(crate) celebration_weight: i32,
    pub(crate) require_celebration: bool,
    pub(crate) fastest_growth: bool,
}

impl GovernorSettings {
    pub(in crate::sim) const FOUNDING: Self = Self::preset(0, 0, [10, 5, 0, 4, 0, 4], 25);

    pub(crate) const PRESETS: [(&'static str, Self); 5] = [
        ("Very happy", Self::FOUNDING),
        ("Food", Self::preset(-20, 0, [25, 5, 0, 4, 0, 4], 0)),
        ("Production", Self::preset(0, -20, [10, 25, 0, 4, 0, 4], 0)),
        ("Gold", Self::preset(0, 0, [10, 5, 0, 25, 0, 4], 0)),
        ("Science", Self::preset(0, 0, [10, 5, 0, 4, 0, 25], 0)),
    ];

    const fn preset(food: i32, shields: i32, weight: [i32; 6], celebration_weight: i32) -> Self {
        Self {
            minimum: [food, shields, 0, -20, 0, 0],
            weight,
            celebration_weight,
            require_celebration: false,
            fastest_growth: false,
        }
    }

    pub(in crate::sim) fn valid(self) -> bool {
        self.minimum.iter().all(|value| (-20..=20).contains(value))
            && self.weight.iter().all(|value| (0..=25).contains(value))
            && (0..=50).contains(&self.celebration_weight)
    }
}

impl Default for GovernorSettings {
    fn default() -> Self {
        Self {
            minimum: [0; 6],
            weight: [1; 6],
            celebration_weight: 1,
            require_celebration: false,
            fastest_growth: false,
        }
    }
}

#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct CityGovernor {
    pub(crate) active: bool,
    pub(crate) settings: GovernorSettings,
}

impl Simulation {
    pub(in crate::sim) fn governor_controls(&self, id: u32) -> bool {
        self.city_governors
            .get(&id)
            .is_some_and(|state| state.active)
    }

    pub(super) fn set_city_governor(&mut self, id: u32, state: CityGovernor) -> bool {
        if !state.settings.valid() || !self.city_by_id(id).is_some_and(|city| city.owner == HUMAN) {
            return false;
        }
        self.city_governors.insert(id, state);
        true
    }

    pub(super) fn city_governor_preview(&self, id: u32) -> Option<[i32; 6]> {
        let city = self.city_by_id(id).filter(|city| city.owner == HUMAN)?;
        let state = self.city_governors.get(&id).copied().unwrap_or_default();
        if state.active {
            return Some(self.city_manager_surplus(city, &self.city_forecast(city)));
        }
        let occupied = self
            .cities
            .iter()
            .filter(|other| other.id != id)
            .flat_map(|other| other.worked_tiles.iter().copied())
            .collect();
        let result = self.governor_assignment(city, &occupied, state.settings)?;
        Some(self.city_manager_surplus(&result, &self.city_forecast(&result)))
    }

    pub(in crate::sim) fn refresh_city_governors(&mut self) {
        if self.game_over.is_some()
            || self.players[HUMAN].ai
            || !self.city_governors.values().any(|state| state.active)
        {
            return;
        }
        let ids: Vec<_> = self
            .player_cities_ordered(HUMAN)
            .filter(|city| self.governor_controls(city.id))
            .map(|city| city.id)
            .collect();
        let mut occupied: std::collections::HashSet<_> = self
            .cities
            .iter()
            .flat_map(|city| city.worked_tiles.iter().copied())
            .collect();
        for id in ids {
            let Some(index) = self.city_index_by_id(id) else {
                continue;
            };
            let city = &self.cities[index];
            for tile in &city.worked_tiles {
                occupied.remove(tile);
            }
            let settings = self.city_governors[&id].settings;
            if let Some(result) = self.governor_assignment(city, &occupied, settings) {
                self.cities[index].worked_tiles = result.worked_tiles;
                self.cities[index].specialists = result.specialists;
            } else {
                self.city_governors.get_mut(&id).unwrap().active = false;
                let name = city.name.clone();
                self.add_event(format!("The governor could not meet the requirements for {name}. City returned to manual control."));
            }
            occupied.extend(self.cities[index].worked_tiles.iter().copied());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_founding_enables_governor_and_allows_releasing_control() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
        }
        let settler = game.spawn_unit(HUMAN, UnitKind::Settler, 12, 24);
        assert!(game.apply(Command::FoundCity(settler)));
        let city_id = game.cities[0].id;
        let governor = game.city_manager_view(city_id).unwrap().governor;
        assert!(governor.active);
        assert_eq!(governor.settings, GovernorSettings::FOUNDING);
        assert!(game.city_governor_preview(city_id).unwrap()[0] >= 0);

        let (x, y) = game.cities[0].worked_tiles[0];
        assert!(!game.apply(Command::WorkCityTile { city_id, x, y }));
        assert!(game.apply(Command::SetCityGovernor {
            city_id,
            state: CityGovernor {
                active: false,
                ..governor
            },
        }));
        assert!(game.apply(Command::WorkCityTile { city_id, x, y }));
    }

    fn fixture() -> (Simulation, u32, (i32, i32)) {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = None;
            tile.river = false;
        }
        game.create_city(HUMAN, 12, 24);
        let city = &game.cities[0];
        let id = city.id;
        let tile = game
            .map_city_radius(city)
            .map(|(_, _, x, y)| (x, y))
            .find(|&(x, y)| game.tile_can_support_worker(city, x, y))
            .unwrap();
        game.cities[0].worked_tiles = vec![tile];
        game.cities[0].specialists = [0; 3];
        (game, id, tile)
    }

    #[test]
    fn governor_prioritizes_science_and_releases_when_requirements_fail() {
        let (mut game, id, tile) = fixture();
        let mut settings = GovernorSettings {
            minimum: [-20; 6],
            weight: [0, 0, 0, 0, 0, 25],
            celebration_weight: 0,
            ..Default::default()
        };
        let initial = game.city_forecast(&game.cities[0]).science;
        let state = CityGovernor {
            active: true,
            settings,
        };
        assert!(game.apply(Command::SetCityGovernor { city_id: id, state }));
        assert!(game.governor_controls(id));
        assert!(game.city_forecast(&game.cities[0]).science > initial);
        assert_eq!(game.cities[0].specialists, [1, 0, 0]);
        assert!(!game.apply(Command::WorkCityTile {
            city_id: id,
            x: tile.0,
            y: tile.1
        }));
        assert!(!game.apply(Command::Specialist {
            city_id: id,
            index: 0,
            delta: -1
        }));

        settings.minimum[5] = 20;
        let state = CityGovernor {
            active: true,
            settings,
        };
        assert!(game.apply(Command::SetCityGovernor { city_id: id, state }));
        assert!(!game.governor_controls(id));
        assert_eq!(game.city_governors[&id].settings, settings);
        assert_eq!(game.cities[0].specialists, [1, 0, 0]);
        assert!(game.apply(Command::WorkCityTile {
            city_id: id,
            x: tile.0,
            y: tile.1
        }));
    }

    #[test]
    fn governor_respects_neighbor_tiles_and_reacts_to_tile_changes() {
        let (mut game, id, _) = fixture();
        game.create_city(HUMAN, 14, 24);
        let shared = (13, 24);
        game.tile_mut(shared.0, shared.1).owner = Some(HUMAN);
        game.tile_mut(shared.0, shared.1).resource = Some(Resource::Wheat);
        game.cities[0].worked_tiles.clear();
        game.cities[0].specialists = [0, 0, 1];
        game.cities[1].worked_tiles = vec![shared];
        game.cities[1].specialists = [0; 3];
        let settings = GovernorSettings::PRESETS[1].1;
        let state = CityGovernor {
            active: true,
            settings,
        };
        assert!(game.apply(Command::SetCityGovernor { city_id: id, state }));
        assert!(game.governor_controls(id));
        assert!(!game.cities[0].worked_tiles.contains(&shared));
        assert_eq!(game.cities[1].worked_tiles, [shared]);

        assert!(game.apply(Command::WorkCityTile {
            city_id: game.cities[1].id,
            x: shared.0,
            y: shared.1,
        }));
        assert!(game.cities[0].worked_tiles.contains(&shared));
    }
}
