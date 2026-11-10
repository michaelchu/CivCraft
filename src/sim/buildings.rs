//! Building completion, availability, and effects.

use super::*;
use crate::rules::{Genus, Requirement};

impl Simulation {
    pub(crate) fn great_wonder_available(&self, building: Building) -> bool {
        building.data().genus == Genus::GreatWonder && !self.completed_wonders.contains(&building)
    }

    pub(crate) fn can_change_production(&self, city: &City, target: Production) -> bool {
        (!city.bought_production || city.shields <= 0) && self.can_produce(city, target)
    }

    pub(crate) fn rush_cost(&self, city: &City) -> Option<i32> {
        if city.founded_turn == self.turn
            || city.bought_production
            || !self.can_produce(city, city.production)
        {
            return None;
        }
        let missing = city.production.cost() - city.shields;
        if missing <= 0 {
            return None;
        }
        let cost = match city.production {
            Production::Unit(_) => {
                if self.city_forecast(city).disorder {
                    return None;
                }
                2 * missing + missing * missing / 20
            }
            Production::Building(building) => match building.data().genus {
                Genus::Convert => return None,
                Genus::GreatWonder => 4 * missing,
                _ => 2 * missing,
            },
        };
        Some(cost * if city.shields == 0 { 2 } else { 1 })
    }

    pub(super) fn rush_production(&mut self, owner: usize, id: u32) -> bool {
        let Some(index) = self
            .cities
            .iter()
            .position(|city| city.id == id && city.owner == owner)
        else {
            return false;
        };
        let Some(cost) = self.rush_cost(&self.cities[index]) else {
            return false;
        };
        if cost > self.players[owner].gold {
            return false;
        }
        self.players[owner].gold -= cost;
        let city = &mut self.cities[index];
        city.shields = city.production.cost();
        city.bought_production = true;
        let message = format!(
            "{} rushed {} for {cost} gold.",
            city.name,
            city.production.name()
        );
        if owner == HUMAN {
            self.add_event(message);
        }
        true
    }

    pub(crate) fn building_sale_price(city: &City, building: Building) -> Option<i32> {
        (!city.sold_building
            && building.data().genus == Genus::Improvement
            && city.buildings.contains(&building))
        .then_some(building.data().cost.max(1))
    }

    pub(super) fn sell_building(&mut self, owner: usize, id: u32, building: Building) -> bool {
        let Some(city) = self
            .cities
            .iter_mut()
            .find(|city| city.id == id && city.owner == owner)
        else {
            return false;
        };
        let Some(price) = Self::building_sale_price(city, building) else {
            return false;
        };
        city.buildings.retain(|&built| built != building);
        city.sold_building = true;
        self.players[owner].gold += price;
        let message = format!(
            "{} sold {} for {price} gold.",
            city.name,
            building.data().name
        );
        if owner == HUMAN {
            self.add_event(message);
        }
        true
    }

    pub(crate) fn can_produce(&self, city: &City, target: Production) -> bool {
        match target {
            Production::Unit(kind) => {
                self.unit_buildable(city, kind) && self.production_successor(city, kind).is_none()
            }
            Production::Building(building) => self.building_blocker(city, building).is_none(),
        }
    }

    pub(crate) fn production_successor(&self, city: &City, kind: UnitKind) -> Option<UnitKind> {
        self.available_unit_successor(city.owner, kind)
    }

    // Native obsolescence walks the entire chain using player requirements,
    // independently of whether this particular city can launch the successor.
    pub(super) fn available_unit_successor(
        &self,
        owner: usize,
        kind: UnitKind,
    ) -> Option<UnitKind> {
        let mut next = kind.obsolete_by();
        let mut best = None;
        while let Some(candidate) = next {
            if self.unit_available(owner, candidate) {
                best = Some(candidate);
            }
            next = candidate.obsolete_by();
        }
        best
    }

    pub(super) fn unit_buildable(&self, city: &City, kind: UnitKind) -> bool {
        self.unit_available(city.owner, kind)
            && self.unit_resources_available(city, kind)
            && self.available_unit_successor(city.owner, kind).is_none()
            && (kind.unit_class().build_anywhere()
                || kind.is_aircraft()
                || self.naval_city_access(city, kind))
    }

    fn unit_resources_available(&self, city: &City, kind: UnitKind) -> bool {
        kind.required_resources()
            .iter()
            .all(|&resource| self.resource_connected_to_city(city, resource))
    }

    fn resource_connected_to_city(&self, city: &City, resource: Resource) -> bool {
        let center = self.tile(city.x, city.y);
        if center.active_resource() == Some(resource) {
            return true;
        }
        let mut reached = vec![false; self.tiles.len()];
        let mut queue = std::collections::VecDeque::new();
        for (x, y) in self.map_wrap.civ3_neighbors(city.x, city.y) {
            let index = self.map_wrap.index(x, y);
            let tile = self.tile(x, y);
            if tile.owner == Some(city.owner) && tile.road && !reached[index] {
                reached[index] = true;
                queue.push_back((x, y));
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            let tile = self.tile(x, y);
            if tile.owner == Some(city.owner)
                && tile.road
                && tile.active_resource() == Some(resource)
            {
                return true;
            }
            for (nx, ny) in self.map_wrap.civ3_neighbors(x, y) {
                let index = self.map_wrap.index(nx, ny);
                let next = self.tile(nx, ny);
                if next.owner == Some(city.owner) && next.road && !reached[index] {
                    reached[index] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
        false
    }

    // common/city.c:can_city_build_unit_later deliberately ignores the
    // candidate's current technology/government requirements, but not an
    // already available successor or the city's native terrain access.
    pub(super) fn unit_buildable_later(&self, city: &City, kind: UnitKind) -> bool {
        !kind.has_no_build()
            && self.available_unit_successor(city.owner, kind).is_none()
            && (kind.unit_class().build_anywhere()
                || kind.is_aircraft()
                || self.naval_city_access(city, kind))
    }

    fn building_requirement_met(
        &self,
        city: &City,
        requirement: Requirement,
        present: bool,
        granted: Option<Tech>,
    ) -> bool {
        let nearby = || {
            std::iter::once(self.tile(city.x, city.y)).chain(
                self.map_wrap
                    .civ3_neighbors(city.x, city.y)
                    .map(|(x, y)| self.tile(x, y)),
            )
        };
        let matches = match requirement {
            Requirement::Tech(id) => {
                granted == Some(Tech(id)) || self.has_tech(city.owner, Tech(id))
            }
            Requirement::Improvement(id) => city.buildings.contains(&Building(id)),
            Requirement::River => nearby().any(|tile| tile.river),
            Requirement::Lake => nearby().any(|tile| tile.terrain == Terrain::Lake),
            Requirement::Sea => nearby()
                .any(|tile| matches!(tile.terrain, Terrain::Coast | Terrain::Sea | Terrain::Ocean)),
            Requirement::Water => nearby().any(|tile| tile.terrain.is_water()),
            Requirement::MinSize(size) => city.size >= size,
            Requirement::OwnerCityCount(count) => {
                self.cities
                    .iter()
                    .filter(|other| other.owner == city.owner)
                    .count()
                    >= count
            }
            Requirement::OwnerBuildingCount(id, count) => {
                self.cities
                    .iter()
                    .filter(|other| other.owner == city.owner && other.has_building(Building(id)))
                    .count()
                    >= count
            }
            Requirement::Resource(id) => {
                let resource = match id {
                    0 => Resource::Aluminum,
                    1 => Resource::Coal,
                    2 => Resource::Horses,
                    3 => Resource::Iron,
                    4 => Resource::Oil,
                    5 => Resource::Rubber,
                    6 => Resource::Saltpeter,
                    7 => Resource::Uranium,
                    _ => return false,
                };
                nearby().any(|tile| tile.active_resource() == Some(resource))
            }
        };
        matches == present
    }

    pub(super) fn building_is_obsolete(
        &self,
        city: &City,
        building: Building,
        granted: Option<Tech>,
    ) -> bool {
        building
            .data()
            .obsolete
            .iter()
            .any(|&(requirement, present)| {
                self.building_requirement_met(city, requirement, present, granted)
            })
    }

    pub(crate) fn building_blocker(&self, city: &City, building: Building) -> Option<String> {
        self.building_blocker_with_tech(city, building, None)
    }

    pub(super) fn building_blocker_with_tech(
        &self,
        city: &City,
        building: Building,
        granted: Option<Tech>,
    ) -> Option<String> {
        let data = building.data();
        if city.buildings.contains(&building) {
            return Some("Built".into());
        }
        if matches!(building, Building::BARRACKS_II | Building::BARRACKS_III) {
            return Some("Barracks training upgrades automatically with technology".into());
        }
        if data.genus == Genus::GreatWonder && self.completed_wonders.contains(&building) {
            return Some("Wonder already completed".into());
        }
        if data.genus == Genus::SmallWonder
            && self
                .cities
                .iter()
                .any(|other| other.owner == city.owner && other.has_building(building))
        {
            return Some("Small Wonder already completed by this civilization".into());
        }
        if let Some((part, limit)) = spaceship_part(building) {
            if !self.apollo_completed {
                return Some("Needs the Apollo Program".into());
            }
            if city.owner == BARBARIANS
                || self.spaceships[city.owner].built[part] >= limit
                || self.spaceships[city.owner].launched()
            {
                return Some("Spaceship part unavailable".into());
            }
        }
        let met = |&(requirement, present)| {
            self.building_requirement_met(city, requirement, present, granted)
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
                    self.spaceships[owner].add_part(part);
                }
                return;
            }
            Genus::GreatWonder => self.completed_wonders.push(building),
            Genus::SmallWonder => {
                for city in self.cities.iter_mut().filter(|city| city.owner == owner) {
                    city.buildings.retain(|&built| built != building);
                }
                if building == Building::APOLLO_PROGRAM {
                    self.apollo_completed = true;
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
        city.record_culture_building_completion(building, self.year);
        if matches!(
            building,
            Building::GREAT_LIBRARY | Building::ISAAC_NEWTONS_COLLEGE | Building::INTERNET
        ) {
            self.grant_cheapest_advance(owner);
        }
        if building == Building::THEORY_OF_EVOLUTION {
            self.grant_cheapest_advance(owner);
            self.grant_cheapest_advance(owner);
        }
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
        Requirement::OwnerCityCount(count) => format!("{count} cities"),
        Requirement::OwnerBuildingCount(id, count) => {
            format!("{count} {} buildings", Building(id).data().name)
        }
        Requirement::Resource(id) => format!(
            "{} in the city radius",
            match id {
                0 => "Aluminum",
                1 => "Coal",
                2 => "Horses",
                3 => "Iron",
                4 => "Oil",
                5 => "Rubber",
                6 => "Saltpeter",
                7 => "Uranium",
                _ => "resource",
            }
        ),
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
        assert_eq!(Building::all().count(), 90);
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let settler = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
            .unwrap()
            .clone();
        assert!(game.apply(Command::FoundCity(settler.id)));
        let id = game.cities[0].id;
        let library = building("Library");
        let university = Production::Building(building("University"));
        let target = Production::Building(library);
        assert!(!game.apply(Command::Produce(id, target)));
        game.discover(HUMAN, Tech::LITERATURE);
        game.discover(HUMAN, Tech::EDUCATION);
        assert!(!game.can_produce(&game.cities[0], university));
        assert!(game.apply(Command::Produce(id, target)));
        game.cities[0].shields = target.cost();
        let units = game.units.len();
        game.process_cities();
        assert_eq!(game.units.len(), units);
        assert!(game.cities[0].buildings.contains(&library));
        // Library adds 50% science; the starting Despotism capital adds
        // 75% gold through its Palace (6*1.5=9 and 4*1.75=7).
        assert_eq!(game.city_trade(&game.cities[0], 10), [9, 7, 0]);
        assert_eq!(game.building_upkeep(&game.cities[0]), library.data().upkeep);
        assert!(!game.can_produce(&game.cities[0], target));
        assert!(game.apply(Command::Produce(id, university)));
    }

    #[test]
    fn coastal_and_aqueduct_requirements_distinguish_lakes_sea_and_rivers() {
        let map = crate::mapgen::generate(42);
        let (x, y) = map.starts[0];
        let mut game = Simulation::new(map);
        game.create_city(HUMAN, x, y);
        let adjacent: Vec<_> = game.map_wrap.civ3_neighbors(x, y).collect();
        for (nx, ny) in std::iter::once((x, y)).chain(adjacent.iter().copied()) {
            let tile = game.tile_mut(nx, ny);
            tile.terrain = Terrain::Plains;
            tile.river = false;
        }
        for name in ["Map Making", "Construction"] {
            game.discover(HUMAN, Tech::all().find(|tech| tech.name() == name).unwrap());
        }
        let harbor = building("Harbor");
        let aqueduct = building("Aqueduct");
        let lake_aqueduct = building("Aqueduct, Lake");
        let river_aqueduct = building("Aqueduct, River");
        // A diagonal Civ III neighbor also satisfies city-local terrain rules.
        let (nx, ny) = adjacent[1];
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
        game.tile_mut(nx, ny).terrain = Terrain::Sea;
        assert!(game.building_blocker(&game.cities[0], harbor).is_none());
        assert!(
            game.building_blocker(&game.cities[0], river_aqueduct)
                .is_none()
        );
    }

    #[test]
    fn added_unit_resource_requirements_need_owned_road_connected_resources() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.create_city(HUMAN, 10, 20);
        let city = game.cities[0].clone();
        assert!(!game.unit_resources_available(&city, UnitKind::Privateer));
        let (iron_x, iron_y) = game.map_wrap.offset(city.x, city.y, 1, 0).unwrap();
        let iron = game.tile_mut(iron_x, iron_y);
        iron.terrain = Terrain::Hills;
        iron.resource = Some(Resource::Iron);
        iron.owner = Some(HUMAN);
        iron.road = true;
        let (salt_x, salt_y) = game.map_wrap.offset(city.x, city.y, -1, 0).unwrap();
        let salt = game.tile_mut(salt_x, salt_y);
        salt.terrain = Terrain::Desert;
        salt.resource = Some(Resource::Saltpeter);
        salt.owner = Some(HUMAN);
        salt.road = true;
        assert!(game.unit_resources_available(&city, UnitKind::Privateer));
    }

    #[test]
    fn civ3_wonder_catalog_uses_small_wonder_uniqueness_for_apollo_and_sdi() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.create_city(HUMAN, 10, 20);
        game.create_city(HUMAN, 20, 20);
        assert_eq!(Building::all().count(), 90);
        assert_eq!(Building::APOLLO_PROGRAM.data().genus, Genus::SmallWonder);
        assert_eq!(Building::SDI_DEFENSE.data().genus, Genus::SmallWonder);
        game.cities[0].buildings.push(Building::APOLLO_PROGRAM);
        assert_eq!(
            game.building_blocker(&game.cities[1], Building::APOLLO_PROGRAM)
                .as_deref(),
            Some("Small Wonder already completed by this civilization")
        );
    }

    #[test]
    fn missing_conquests_culture_wonders_use_appended_catalog_ids_and_source_requirements() {
        assert_eq!(Building::SISTINE_CHAPEL.0, 88);
        assert_eq!(Building::KNIGHTS_TEMPLAR.0, 89);
        assert_eq!(Building::SISTINE_CHAPEL.data().name, "Sistine Chapel");
        assert_eq!(Building::SISTINE_CHAPEL.data().cost, 600);
        assert_eq!(Building::SISTINE_CHAPEL.data().genus, Genus::GreatWonder);
        assert!(
            Building::SISTINE_CHAPEL
                .data()
                .requirements
                .contains(&(Requirement::Tech(Tech::THEOLOGY.0), true))
        );

        assert_eq!(Building::KNIGHTS_TEMPLAR.data().name, "Knights Templar");
        assert_eq!(Building::KNIGHTS_TEMPLAR.data().cost, 300);
        assert_eq!(Building::KNIGHTS_TEMPLAR.data().genus, Genus::GreatWonder);
        assert!(
            Building::KNIGHTS_TEMPLAR
                .data()
                .requirements
                .contains(&(Requirement::Tech(Tech::CHIVALRY.0), true))
        );
        assert!(
            Building::KNIGHTS_TEMPLAR
                .data()
                .obsolete
                .contains(&(Requirement::Tech(Tech::STEAM_POWER.0), true))
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

#[cfg(test)]
mod transaction_tests {
    use super::*;

    #[test]
    fn rush_fills_shield_stock_at_reference_prices_and_locks_the_target() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for (_, _, x, y) in Simulation::city_radius_at(24, 15) {
            let tile = game.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.road = true;
            tile.river = true;
            tile.irrigation = true;
        }
        game.tile_mut(24, 15).terrain = Terrain::Grassland;
        game.create_city(HUMAN, 24, 15);
        let id = game.cities[0].id;
        assert!(!game.apply(Command::RushProduction(id)));
        game.turn += 1;
        assert_eq!(game.rush_cost(&game.cities[0]), Some(50));
        game.players[HUMAN].gold = 49;
        assert!(!game.apply(Command::RushProduction(id)));
        assert_eq!(game.cities[0].shields, 0);

        game.players[HUMAN].known.push(Tech::POTTERY);
        assert!(game.apply(Command::Produce(
            id,
            Production::Building(Building::GRANARY)
        )));
        let price = Building::GRANARY.data().cost * 4;
        assert_eq!(game.rush_cost(&game.cities[0]), Some(price));
        game.players[HUMAN].gold = price;
        assert!(game.apply(Command::RushProduction(id)));
        assert_eq!(game.players[HUMAN].gold, 0);
        assert!(!game.cities[0].has_building(Building::GRANARY));
        assert!(!game.apply(Command::Produce(id, Production::Unit(UnitKind::Warrior))));
        assert!(!game.apply(Command::RushProduction(id)));
        game.process_cities();
        assert!(game.cities[0].has_building(Building::GRANARY));
        assert!(game.apply(Command::Produce(id, Production::Unit(UnitKind::Warrior))));
    }

    #[test]
    fn only_regular_buildings_can_be_sold_and_only_one_per_turn() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let id = game.cities[0].id;
        game.cities[0]
            .buildings
            .extend([Building::GRANARY, Building::TEMPLE]);
        let gold = game.players[HUMAN].gold;
        assert!(!game.apply(Command::SellBuilding(id, Building::PALACE)));
        assert!(game.apply(Command::SellBuilding(id, Building::TEMPLE)));
        assert!(!game.cities[0].has_building(Building::TEMPLE));
        assert_eq!(
            game.players[HUMAN].gold,
            gold + Building::TEMPLE.data().cost
        );
        assert!(!game.apply(Command::SellBuilding(id, Building::GRANARY)));
        game.process_cities();
        assert!(game.apply(Command::SellBuilding(id, Building::GRANARY)));
    }
}
