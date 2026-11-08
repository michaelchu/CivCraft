//! Pillage target selection and improvement removal.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum PillageTarget {
    Road,
    Railroad,
    Irrigation,
    Farmland,
    Mine,
    OilWell,
    OilPlatform,
    Base(Base),
}

impl PillageTarget {
    pub(crate) const ALL: [Self; 7] = [
        Self::Road,
        Self::Railroad,
        Self::Irrigation,
        Self::Farmland,
        Self::Mine,
        Self::OilWell,
        Self::OilPlatform,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Road => "Road",
            Self::Railroad => "Railroad",
            Self::Irrigation => "Irrigation",
            Self::Farmland => "Farmland",
            Self::Mine => "Mine",
            Self::OilWell => "Oil Well",
            Self::OilPlatform => "Oil Platform",
            Self::Base(base) => base.name(),
        }
    }
}

impl Simulation {
    pub(crate) fn can_pillage(&self, unit: &Unit, target: PillageTarget) -> bool {
        // Civ2Civ3 assigns CanPillage to the Land class, excluding Small and Big Land.
        if unit.transport.is_some()
            || !matches!(
                unit.kind,
                UnitKind::Warrior
                    | UnitKind::Phalanx
                    | UnitKind::Archer
                    | UnitKind::Legion
                    | UnitKind::Horseman
                    | UnitKind::Musketeers
                    | UnitKind::Knights
                    | UnitKind::Partisan
            )
        {
            return false;
        }
        if self.units.iter().any(|other| {
            other.id != unit.id
                && (other.x, other.y) == (unit.x, unit.y)
                && other.work_kind == Some(WorkKind::Pillage(target))
        }) {
            return false;
        }
        self.pillage_target_present(unit.x, unit.y, target)
    }

    fn pillage_target_present(&self, x: i32, y: i32, target: PillageTarget) -> bool {
        let tile = self.tile(x, y);
        let city = self.city_at(x, y);
        match target {
            PillageTarget::Road => tile.road && !tile.railroad && city.is_none(),
            PillageTarget::Railroad => {
                tile.railroad
                    && city.is_none_or(|city| {
                        !self.has_tech(city.owner, Tech::STEAM_POWER)
                            || (tile.river && !self.has_tech(city.owner, Tech::ENGINEERING))
                    })
            }
            PillageTarget::Irrigation => tile.irrigation && !tile.farmland,
            PillageTarget::Farmland => tile.farmland,
            PillageTarget::Mine => tile.mine && !tile.oil_well,
            PillageTarget::OilWell => tile.oil_well,
            PillageTarget::OilPlatform => tile.oil_platform,
            PillageTarget::Base(base) => tile.base == Some(base),
        }
    }

    pub(super) fn finish_pillage(&mut self, owner: usize, x: i32, y: i32, target: PillageTarget) {
        if !self.pillage_target_present(x, y, target) {
            return;
        }
        let tile = self.tile_mut(x, y);
        match target {
            PillageTarget::Road => tile.road = false,
            PillageTarget::Railroad => tile.railroad = false,
            PillageTarget::Irrigation => tile.irrigation = false,
            PillageTarget::Farmland => tile.farmland = false,
            PillageTarget::Mine => tile.mine = false,
            PillageTarget::OilWell => tile.oil_well = false,
            PillageTarget::OilPlatform => tile.oil_platform = false,
            PillageTarget::Base(base) => tile.base = base.prerequisite(),
        }
        if owner == HUMAN {
            self.add_event(format!("Your unit pillaged {}.", target.name()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pillaging_respects_dependencies_city_roads_and_unit_class() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        game.create_city(HUMAN, x, y);
        let warrior = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        assert!(!game.apply(Command::Work(
            warrior,
            WorkKind::Pillage(PillageTarget::Road)
        )));
        let (x, y) = offset_native_by_map(x, y, 1, 0);
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Plains;
        tile.road = true;
        tile.railroad = true;
        tile.irrigation = true;
        tile.farmland = true;

        let warrior = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        let other = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        let cannon = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
        for target in [PillageTarget::Road, PillageTarget::Irrigation] {
            assert!(!game.apply(Command::Work(warrior, WorkKind::Pillage(target))));
        }
        let railroad = WorkKind::Pillage(PillageTarget::Railroad);
        assert!(!game.apply(Command::Work(cannon, railroad)));
        assert!(game.apply(Command::Work(warrior, railroad)));
        assert!(!game.apply(Command::Work(other, railroad)));
        assert!(game.apply(Command::Work(
            other,
            WorkKind::Pillage(PillageTarget::Farmland)
        )));
        assert!(game.tile(x, y).railroad && game.tile(x, y).farmland);
        game.update_activities_and_restore();
        let tile = game.tile(x, y);
        assert!(tile.road && tile.irrigation && !tile.railroad && !tile.farmland);
    }
}
