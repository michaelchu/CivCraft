//! Implements terrain work behavior within the sim subsystem.

use super::*;

pub(super) fn neighbors(x: i32, y: i32) -> impl Iterator<Item = (i32, i32)> {
    ISOHEX_DIRS
        .into_iter()
        .map(move |(dx, dy)| offset_native_by_map(x, y, dx, dy))
}

impl Terrain {
    pub(super) fn alteration(self, kind: WorkKind) -> Option<(Self, i32)> {
        use Terrain::*;
        use WorkKind::*;
        Some(match (kind, self) {
            (Cultivate, Forest) => (Plains, 10),
            (Cultivate, Jungle) => (Marsh, 10),
            (Plant, Forest) => (Grassland, 15),
            (Plant, Grassland | GrasslandShield) => (Forest, 15),
            (Plant, Plains) => (Forest, 10),
            (Transform, Coast | Sea | Ocean | Lake) => (Grassland, 36),
            (Transform, Glacier) => (Lake, 24),
            (Transform, Grassland | GrasslandShield | Plains) => (Lake, 36),
            (Transform, Desert | Hills | Tundra) => (Plains, 24),
            (Transform, Jungle) => (Forest, 24),
            (Transform, Forest) => (Hills, 24),
            (Transform, Mountains) => (Hills, 36),
            (Transform, Marsh) => (Grassland, 24),
            _ => return None,
        })
    }
}

impl Simulation {
    pub(crate) fn oil_work(&self, unit: &Unit) -> Option<WorkKind> {
        let tile = self.tile(unit.x, unit.y);
        if tile.terrain == Terrain::Ocean
            && !tile.oil_platform
            && self.has_tech(unit.owner, Tech::MINIATURIZATION)
            && (unit.kind.unit_class() == super::unit_catalog::UnitClass::Sea
                && unit.kind.is_worker()
                || (unit.transport.is_some() && unit.kind.is_worker()))
        {
            return Some(WorkKind::OilPlatform);
        }
        if matches!(tile.terrain, Terrain::Desert | Terrain::Glacier)
            && tile.mine
            && !tile.oil_well
            && unit.kind.is_worker()
            && unit.kind.unit_class() != super::unit_catalog::UnitClass::Sea
            && unit.transport.is_none()
            && self.has_tech(unit.owner, Tech::REFINING)
        {
            return Some(WorkKind::OilWell);
        }
        None
    }

    pub(crate) fn terrain_work(&self, unit: &Unit, kind: WorkKind) -> Option<(Terrain, i32)> {
        let tile = self.tile(unit.x, unit.y);
        let (target, time) = tile.terrain.alteration(kind)?;
        if !unit.kind.is_worker()
            || (matches!(kind, WorkKind::Cultivate | WorkKind::Plant)
                && unit.kind.unit_class() == super::unit_catalog::UnitClass::Sea)
            || (kind == WorkKind::Plant && !self.has_tech(unit.owner, Tech::ENGINEERING))
            || (kind == WorkKind::Transform
                && (!unit.kind.has_flag(super::unit_catalog::UnitFlag::Transform)
                    || !self.has_tech(unit.owner, Tech::NUCLEAR_POWER)))
            || (unit.transport.is_some() && kind != WorkKind::Transform)
        {
            return None;
        }
        self.terrain_change_allowed(unit.x, unit.y, target)
            .then_some((target, time))
    }

    pub(super) fn terrain_change_allowed(&self, x: i32, y: i32, target: Terrain) -> bool {
        if (target.is_water() || target == Terrain::Glacier)
            && self.cities.iter().any(|city| (city.x, city.y) == (x, y))
        {
            return false;
        }
        let old = self.tile(x, y).terrain;
        if target.is_water() == old.is_water() {
            return true;
        }
        let adjacent: Vec<_> = self.map_wrap.civ3_neighbors(x, y).collect();
        let water = adjacent
            .iter()
            .filter(|&&(nx, ny)| self.tile(nx, ny).terrain.is_water())
            .count();
        if target.is_water() {
            water * 100 >= adjacent.len() * 10
        } else {
            (adjacent.len() - water) * 100 >= adjacent.len() * 30
        }
    }

    pub(super) fn alter_terrain(&mut self, x: i32, y: i32, target: Terrain) {
        let tile = self.tile_mut(x, y);
        let changed_class = tile.terrain.is_water() != target.is_water();
        tile.terrain = target;
        if matches!(target, Terrain::Lake | Terrain::Ocean) {
            tile.pollution = false;
            tile.fallout = false;
        }
        if target.irrigation_time() == 0 {
            tile.irrigation = false;
            tile.farmland = false;
        }
        if !matches!(target, Terrain::Desert | Terrain::Glacier) {
            tile.oil_well = false;
        }
        if target != Terrain::Ocean {
            tile.oil_platform = false;
        }
        if target.mine_time() == 0 {
            tile.mine = false;
        }
        if target.is_water() {
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.river_edges = 0;
            tile.hut = false;
            tile.base = None;
        }
        for unit in self
            .units
            .iter_mut()
            .filter(|unit| (unit.x, unit.y) == (x, y))
        {
            if unit.work_kind.is_some_and(|kind| {
                matches!(
                    kind,
                    WorkKind::Cultivate | WorkKind::Plant | WorkKind::Transform
                )
            }) {
                unit.clear_work();
            }
        }
        if !changed_class {
            return;
        }
        let stranded: Vec<_> = self
            .units
            .iter()
            .filter(|unit| (unit.x, unit.y) == (x, y) && unit.transport.is_none())
            .filter(|unit| !unit.kind.is_aircraft() && unit.kind.is_naval() != target.is_water())
            .map(|unit| unit.id)
            .collect();
        for id in stranded {
            let Some(unit) = self.unit_by_id(id) else {
                continue;
            };
            let destination = self.map_wrap.civ3_neighbors(x, y).find(|&(nx, ny)| {
                self.movement_cost_for(unit, nx, ny).is_some()
                    && !self.treaty_blocks_move(unit, nx, ny)
                    && self.best_defender_id(unit, nx, ny).is_none()
                    && !self.cities.iter().any(|city| {
                        (city.x, city.y) == (nx, ny) && self.at_war(city.owner, unit.owner)
                    })
            });
            // Land cargo can stand on the newly reclaimed tile while the ship moves.
            for passenger in self
                .units
                .iter_mut()
                .filter(|unit| unit.transport == Some(id))
            {
                passenger.transport = None;
                passenger.clear_work();
            }
            if let Some((nx, ny)) = destination {
                let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
                (unit.x, unit.y) = (nx, ny);
                unit.clear_work();
                unit.moves_left = 0;
            } else {
                self.units.retain(|unit| unit.id != id);
                self.discard_unit_ai(id);
            }
        }
        self.refresh_visibility();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_workers_build_deep_ocean_platforms_and_clean_but_cannot_do_sea_excluded_work() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.transport_worker_actions_playtest(), (true, true));
    }

    #[test]
    fn deep_ocean_oil_platform_requires_technology_and_enables_offshore_shields() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        let (sx, sy) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(sx, sy).terrain = Terrain::Ocean;
        game.create_city(HUMAN, x, y);
        let ship = game.spawn_unit(HUMAN, UnitKind::Transport, sx, sy);
        let engineer = game.spawn_unit(HUMAN, UnitKind::Engineer, sx, sy);
        assert!(game.apply(Command::BoardOrDeboard(engineer)));
        assert!(!game.apply(Command::Work(ship, WorkKind::OilPlatform)));
        game.players[HUMAN].known.push(Tech::MINIATURIZATION);
        game.cities[0].worked_tiles = vec![(sx, sy)];
        game.cities[0].buildings.push(Building::OFFSHORE_PLATFORM);
        let before = game.city_yields(&game.cities[0]).1;
        assert!(game.apply(Command::Work(engineer, WorkKind::OilPlatform)));
        for _ in 0..5 {
            game.update_activities_and_restore();
        }
        assert!(game.tile(sx, sy).oil_platform);
        assert_eq!(game.city_yields(&game.cities[0]).1, before + 1);
    }

    #[test]
    fn oil_well_requires_refining_and_mined_oil_terrain_then_adds_a_shield() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Desert;
        game.tile_mut(x, y).mine = true;
        game.tile_mut(x, y).resource = Some(Resource::Oil);
        let before = game.tile(x, y).yields().1;
        let worker = game.spawn_unit(HUMAN, UnitKind::Worker, x, y);

        assert!(!game.apply(Command::Work(worker, WorkKind::OilWell)));
        game.players[HUMAN].known.push(Tech::REFINING);
        assert!(game.apply(Command::Work(worker, WorkKind::OilWell)));
        for _ in 0..5 {
            game.update_activities_and_restore();
        }

        let tile = game.tile(x, y);
        assert!(tile.oil_well);
        assert!(tile.mine);
        assert_eq!(tile.yields().1, before + 1);

        let glacier = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(glacier.0, glacier.1).terrain = Terrain::Glacier;
        game.tile_mut(glacier.0, glacier.1).mine = true;
        let second_worker = game.spawn_unit(HUMAN, UnitKind::Worker, glacier.0, glacier.1);
        assert!(game.apply(Command::Work(second_worker, WorkKind::OilWell)));
    }

    #[test]
    fn workers_cultivate_and_engineers_reclaim_ocean_with_fusion_power() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Forest;
        let worker = game.spawn_unit(HUMAN, UnitKind::Worker, x, y);
        assert!(game.apply(Command::Work(worker, WorkKind::Cultivate)));
        for _ in 0..10 {
            game.update_activities_and_restore();
        }
        assert_eq!(game.tile(x, y).terrain, Terrain::Plains);

        game.tile_mut(x, y).terrain = Terrain::Sea;
        game.tile_mut(x, y).river = false;
        game.tile_mut(x, y).river_edges = 0;
        let nearby: Vec<_> = game.map_wrap.civ3_neighbors(x, y).collect();
        for &(nx, ny) in &nearby {
            game.tile_mut(nx, ny).terrain = Terrain::Sea;
        }
        game.units.clear();
        let ship = game.spawn_unit(HUMAN, UnitKind::Caravel, x, y);
        let engineer = game.spawn_unit(HUMAN, UnitKind::Engineer, x, y);
        assert!(game.apply(Command::BoardOrDeboard(engineer)));
        assert!(!game.apply(Command::Work(engineer, WorkKind::Transform)));
        game.players[HUMAN].known.push(Tech::NUCLEAR_POWER);
        assert!(!game.apply(Command::Work(engineer, WorkKind::Transform)));
        // With eight Civ III neighbors, three land tiles meet the 30% rule.
        for &(nx, ny) in nearby.iter().take(3) {
            game.tile_mut(nx, ny).terrain = Terrain::Plains;
        }
        assert!(game.apply(Command::Work(engineer, WorkKind::Transform)));
        for _ in 0..20 {
            if game.tile(x, y).terrain != Terrain::Sea {
                break;
            }
            game.update_activities_and_restore();
        }
        assert_eq!(game.tile(x, y).terrain, Terrain::Grassland);
        let engineer = game.unit_by_id(engineer).unwrap();
        assert_eq!((engineer.x, engineer.y, engineer.transport), (x, y, None));
        let ship = game.unit_by_id(ship).unwrap();
        assert!(game.tile(ship.x, ship.y).terrain.is_water());
    }
}
