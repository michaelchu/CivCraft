use super::*;
use crate::rules::{Genus, Requirement};

impl Simulation {
    pub(crate) fn can_produce(&self, city: &City, target: Production) -> bool {
        match target {
            Production::Unit(kind) => self.unit_available(city.owner, kind),
            Production::Building(building) => self.building_blocker(city, building).is_none(),
        }
    }

    pub(crate) fn building_blocker(&self, city: &City, building: Building) -> Option<String> {
        let data = building.data();
        if city.buildings.contains(&building) {
            return Some("Built".into());
        }
        if data.genus == Genus::GreatWonder && self.completed_wonders.contains(&building) {
            return Some("Wonder already completed".into());
        }
        if let Some((part, limit)) = spaceship_part(building)
            && (city.owner >= PLAYER_COUNT || self.spaceship_parts[city.owner][part] >= limit)
        {
            return Some("Spaceship part limit reached".into());
        }
        let nearby = || {
            std::iter::once((0, 0)).chain(ISOHEX_DIRS).map(|(dx, dy)| {
                let (x, y) = offset_native_by_map(city.x, city.y, dx, dy);
                self.tile(x, y)
            })
        };
        let met = |&(requirement, present): &(Requirement, bool)| {
            let matches = match requirement {
                Requirement::Tech(id) => self.has_tech(city.owner, Tech(id)),
                Requirement::Improvement(id) => city.buildings.contains(&Building(id)),
                Requirement::River => nearby().any(|tile| tile.river),
                Requirement::Lake => nearby().any(|tile| tile.terrain == Terrain::Lake),
                Requirement::Sea => {
                    nearby().any(|tile| matches!(tile.terrain, Terrain::Ocean | Terrain::DeepOcean))
                }
                Requirement::Water => nearby().any(|tile| tile.terrain.is_water()),
                Requirement::MinSize(size) => city.size >= size,
            };
            matches == present
        };
        if let Some(&(requirement, _)) = data.obsolete.iter().find(|&req| met(req)) {
            return Some(format!("Obsolete with {}", requirement_name(requirement)));
        }
        data.requirements
            .iter()
            .find(|&req| !met(req))
            .map(|&(requirement, present)| {
                let name = requirement_name(requirement);
                if present {
                    format!("Needs {name}")
                } else {
                    format!("Blocked by {name}")
                }
            })
    }

    pub(super) fn finish_building(
        &mut self,
        city_id: u32,
        owner: usize,
        building: Building,
        shields: i32,
    ) {
        match building.data().genus {
            Genus::Convert => {
                self.players[owner].gold += shields;
                return;
            }
            Genus::Special => {
                if let Some((part, _)) = spaceship_part(building) {
                    self.spaceship_parts[owner][part] += 1;
                }
                return;
            }
            Genus::GreatWonder => self.completed_wonders.push(building),
            Genus::SmallWonder => {
                for city in self.cities.iter_mut().filter(|city| city.owner == owner) {
                    city.buildings.retain(|&built| built != building);
                }
            }
            Genus::Improvement => {}
        }
        let city = self
            .cities
            .iter_mut()
            .find(|city| city.id == city_id)
            .unwrap();
        city.buildings.push(building);
        city.production = Production::Unit(UnitKind::Warrior);
    }
}

fn requirement_name(requirement: Requirement) -> String {
    match requirement {
        Requirement::Tech(id) => Tech(id).name().into(),
        Requirement::Improvement(id) => Building(id).data().name.into(),
        Requirement::River => "a nearby river".into(),
        Requirement::Lake => "a nearby lake".into(),
        Requirement::Sea => "a sea coast".into(),
        Requirement::Water => "nearby sea or lake".into(),
        Requirement::MinSize(size) => format!("city size {size}"),
    }
}

fn spaceship_part(building: Building) -> Option<(usize, usize)> {
    match building {
        Building::SPACE_STRUCTURAL => Some((0, 32)),
        Building::SPACE_COMPONENT => Some((1, 16)),
        Building::SPACE_MODULE => Some((2, 12)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn building(name: &str) -> Building {
        Building::all()
            .find(|building| building.data().name == name)
            .unwrap()
    }

    #[test]
    fn researched_building_completes_and_unlocks_its_successor() {
        assert_eq!(Building::all().count(), 73);
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let settler = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settlers)
            .unwrap()
            .clone();
        assert!(game.apply(Command::FoundCity(settler.id)));
        let id = game.cities[0].id;
        let library = building("Library");
        let university = Production::Building(building("University"));
        let target = Production::Building(library);
        assert!(!game.apply(Command::Produce(id, target)));
        let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
        game.discover(HUMAN, writing);
        game.discover(HUMAN, Tech::UNIVERSITY);
        assert!(!game.can_produce(&game.cities[0], university));
        assert!(game.apply(Command::Produce(id, target)));
        game.cities[0].shields = target.cost();
        let units = game.units.len();
        game.process_cities();
        assert_eq!(game.units.len(), units);
        assert!(game.cities[0].buildings.contains(&library));
        assert_eq!(game.city_trade(&game.cities[0], 10), [9, 4, 0]);
        assert_eq!(
            Simulation::building_upkeep(&game.cities[0]),
            library.data().upkeep
        );
        assert!(!game.can_produce(&game.cities[0], target));
        assert!(game.apply(Command::Produce(id, university)));
    }

    #[test]
    fn coastal_and_aqueduct_requirements_distinguish_lakes_sea_and_rivers() {
        let map = crate::mapgen::generate(42);
        let (x, y) = map.starts[0];
        let mut game = Simulation::new(map);
        game.create_city(HUMAN, x, y);
        for (dx, dy) in std::iter::once((0, 0)).chain(ISOHEX_DIRS) {
            let (nx, ny) = offset_native_by_map(x, y, dx, dy);
            let tile = game.tile_mut(nx, ny);
            tile.terrain = Terrain::Plains;
            tile.river = false;
        }
        for name in ["Seafaring", "Construction"] {
            game.discover(HUMAN, Tech::all().find(|tech| tech.name() == name).unwrap());
        }
        let harbor = building("Harbor");
        let aqueduct = building("Aqueduct");
        let lake_aqueduct = building("Aqueduct, Lake");
        let river_aqueduct = building("Aqueduct, River");
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        assert!(game.building_blocker(&game.cities[0], aqueduct).is_none());
        game.tile_mut(nx, ny).terrain = Terrain::Lake;
        game.tile_mut(x, y).river = true;
        assert_eq!(
            game.building_blocker(&game.cities[0], harbor).as_deref(),
            Some("Needs a sea coast")
        );
        assert!(
            game.building_blocker(&game.cities[0], lake_aqueduct)
                .is_none()
        );
        assert!(
            game.building_blocker(&game.cities[0], river_aqueduct)
                .is_some()
        );
        assert!(game.building_blocker(&game.cities[0], aqueduct).is_some());
        game.tile_mut(nx, ny).terrain = Terrain::Ocean;
        assert!(game.building_blocker(&game.cities[0], harbor).is_none());
        assert!(
            game.building_blocker(&game.cities[0], river_aqueduct)
                .is_none()
        );
    }

    #[test]
    fn great_wonder_cannot_finish_in_two_cities() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let starts = crate::mapgen::generate(42).starts;
        let wonder = building("Pyramids");
        let target = Production::Building(wonder);
        for (owner, (x, y)) in starts.into_iter().take(2).enumerate() {
            game.create_city(owner, x, y);
            for &(requirement, present) in wonder.data().requirements {
                if let Requirement::Tech(id) = requirement
                    && present
                {
                    game.discover(owner, Tech(id));
                }
            }
            let id = game.cities[owner].id;
            assert!(game.change_production(id, target));
            game.cities[owner].shields = target.cost();
        }
        game.process_cities();
        assert_eq!(
            game.cities
                .iter()
                .filter(|city| city.buildings.contains(&wonder))
                .count(),
            1
        );
        assert!(!game.can_produce(&game.cities[1], target));
    }
}
