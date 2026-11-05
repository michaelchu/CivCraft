use super::*;

impl Terrain {
    pub(super) fn alteration(self, kind: WorkKind) -> Option<(Self, i32)> {
        use Terrain::*;
        use WorkKind::*;
        Some(match (kind, self) {
            (Cultivate, Forest) => (Plains, 10),
            (Cultivate, Jungle) => (Swamp, 10),
            (Plant, Forest) => (Grassland, 15),
            (Plant, Grassland) => (Forest, 15),
            (Plant, Plains) => (Forest, 10),
            (Transform, Lake | Ocean) => (Grassland, 36),
            (Transform, Glacier) => (Lake, 24),
            (Transform, Grassland | Plains) => (Lake, 36),
            (Transform, Desert | Hills | Tundra) => (Plains, 24),
            (Transform, Jungle) => (Forest, 24),
            (Transform, Forest) => (Hills, 24),
            (Transform, Mountains) => (Hills, 36),
            (Transform, Swamp) => (Grassland, 24),
            _ => return None,
        })
    }
}

pub(super) fn neighbors(x: i32, y: i32) -> impl Iterator<Item = (i32, i32)> {
    let (mx, my) = native_to_map(x, y);
    ISOHEX_DIRS.into_iter().filter_map(move |(dx, dy)| {
        let (nx, ny) = map_to_native(mx + dx, my + dy);
        (0..MAP_H)
            .contains(&ny)
            .then_some((nx.rem_euclid(MAP_W), ny))
    })
}

impl Simulation {
    pub(crate) fn oil_work(&self, unit: &Unit) -> Option<WorkKind> {
        let tile = self.tile(unit.x, unit.y);
        if tile.terrain == Terrain::DeepOcean
            && !tile.oil_platform
            && self.has_tech(unit.owner, Tech::MINIATURIZATION)
            && (unit.kind == UnitKind::Transport
                || (unit.transport.is_some() && unit.kind.stats().can_work))
        {
            return Some(WorkKind::OilPlatform);
        }
        if matches!(tile.terrain, Terrain::Desert | Terrain::Glacier)
            && tile.mine
            && !tile.oil_well
            && unit.kind.stats().can_work
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
        if !unit.kind.stats().can_work
            || (kind == WorkKind::Transform
                && (unit.kind != UnitKind::Engineer
                    || !self.has_tech(unit.owner, Tech::FUSION_POWER)))
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
        let adjacent: Vec<_> = neighbors(x, y).collect();
        let water = adjacent
            .iter()
            .filter(|&&(nx, ny)| self.tile(nx, ny).terrain.is_water())
            .count();
        if target.is_water() {
            water * 100 >= adjacent.len() * 10
        } else {
            (ISOHEX_DIRS.len() - water) * 100 >= ISOHEX_DIRS.len() * 30
        }
    }

    pub(super) fn alter_terrain(&mut self, x: i32, y: i32, target: Terrain) {
        let tile = self.tile_mut(x, y);
        let changed_class = tile.terrain.is_water() != target.is_water();
        tile.terrain = target;
        if matches!(target, Terrain::Lake | Terrain::DeepOcean) {
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
        if target != Terrain::DeepOcean {
            tile.oil_platform = false;
        }
        if target.mine_time() == 0 {
            tile.mine = false;
        }
        if target.is_water() {
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
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
            let Some(unit) = self.units.iter().find(|unit| unit.id == id) else {
                continue;
            };
            let destination = neighbors(x, y).find(|&(nx, ny)| {
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
            }
        }
        self.refresh_visibility();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_ocean_oil_platform_requires_technology_and_enables_offshore_shields() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        let (sx, sy) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(sx, sy).terrain = Terrain::DeepOcean;
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

        game.tile_mut(x, y).terrain = Terrain::Ocean;
        let nearby: Vec<_> = neighbors(x, y).collect();
        for &(nx, ny) in &nearby {
            game.tile_mut(nx, ny).terrain = Terrain::Ocean;
        }
        game.units.clear();
        let ship = game.spawn_unit(HUMAN, UnitKind::Caravel, x, y);
        let engineer = game.spawn_unit(HUMAN, UnitKind::Engineer, x, y);
        assert!(game.apply(Command::BoardOrDeboard(engineer)));
        assert!(!game.apply(Command::Work(engineer, WorkKind::Transform)));
        game.players[HUMAN].known.push(Tech::FUSION_POWER);
        assert!(!game.apply(Command::Work(engineer, WorkKind::Transform)));
        for &(nx, ny) in nearby.iter().take(2) {
            game.tile_mut(nx, ny).terrain = Terrain::Plains;
        }
        assert!(game.apply(Command::Work(engineer, WorkKind::Transform)));
        for _ in 0..17 {
            game.update_activities_and_restore();
        }
        assert_eq!(game.tile(x, y).terrain, Terrain::Ocean);
        game.update_activities_and_restore();
        assert_eq!(game.tile(x, y).terrain, Terrain::Grassland);
        let engineer = game.units.iter().find(|unit| unit.id == engineer).unwrap();
        assert_eq!((engineer.x, engineer.y, engineer.transport), (x, y, None));
        let ship = game.units.iter().find(|unit| unit.id == ship).unwrap();
        assert!(game.tile(ship.x, ship.y).terrain.is_water());
    }
}
