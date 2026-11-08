//! Military base construction and effects.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum Base {
    Fort,
    Fortress,
    Airstrip,
    Airbase,
}

impl Base {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Fort => "Fort",
            Self::Fortress => "Fortress",
            Self::Airstrip => "Airstrip",
            Self::Airbase => "Airbase",
        }
    }

    pub(super) fn prerequisite(self) -> Option<Self> {
        match self {
            Self::Fortress => Some(Self::Fort),
            Self::Airbase => Some(Self::Airstrip),
            _ => None,
        }
    }

    pub(super) fn defense_bonus(self) -> i32 {
        match self {
            Self::Fort => 50,
            Self::Fortress => 100,
            Self::Airstrip => 0,
            Self::Airbase => 50,
        }
    }
}

impl Simulation {
    pub(crate) fn base_work_time(&self, unit: &Unit, base: Base) -> Option<i32> {
        let tile = self.tile(unit.x, unit.y);
        let (tech, capable) = match base {
            Base::Fort | Base::Fortress => (Tech::CONSTRUCTION, unit.kind.can_build_fortress()),
            Base::Airstrip | Base::Airbase => (Tech::FLIGHT, unit.kind.is_worker()),
        };
        if !capable
            || unit.transport.is_some()
            || tile.terrain.is_water()
            || !self.has_tech(unit.owner, tech)
            || tile.base != base.prerequisite()
            || (base.prerequisite().is_some() && tile.river)
            || self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (unit.x, unit.y))
        {
            return None;
        }
        Some(
            2 * match tile.terrain {
                Terrain::Mountains => 3,
                Terrain::Glacier
                | Terrain::Forest
                | Terrain::Hills
                | Terrain::Jungle
                | Terrain::Swamp => 2,
                _ => 1,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bases_require_their_prerequisites_and_fortresses_protect_and_heal() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Plains;
        game.tile_mut(x, y).river = false;
        let worker = game.spawn_unit(HUMAN, UnitKind::Worker, x, y);
        let warrior = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        let build = |base| Command::Work(worker, WorkKind::BuildBase(base));
        assert!(!game.apply(build(Base::Fort)));
        game.players[HUMAN]
            .known
            .extend([Tech::CONSTRUCTION, Tech::FLIGHT]);
        assert!(!game.apply(build(Base::Fortress)));
        assert!(game.apply(build(Base::Fort)));
        let initial = game.defense_power(&game.units[1], x, y);
        game.update_activities_and_restore();
        assert_eq!(game.tile(x, y).base, None);
        game.update_activities_and_restore();
        assert_eq!(game.tile(x, y).base, Some(Base::Fort));
        assert_eq!(game.defense_power(&game.units[1], x, y), initial * 3 / 2);
        assert!(!game.apply(build(Base::Airstrip)));
        game.tile_mut(x, y).river = true;
        assert!(!game.apply(build(Base::Fortress)));
        game.tile_mut(x, y).river = false;
        assert!(game.apply(build(Base::Fortress)));
        for _ in 0..2 {
            game.update_activities_and_restore();
        }
        assert_eq!(game.defense_power(&game.units[1], x, y), initial * 2);
        game.units[1].hp = 1;
        game.update_activities_and_restore();
        assert_eq!(game.units[1].hp, 4);
        assert!(game.apply(Command::Work(
            warrior,
            WorkKind::Pillage(PillageTarget::Base(Base::Fortress))
        )));
        game.update_activities_and_restore();
        assert_eq!(game.tile(x, y).base, Some(Base::Fort));
    }
}
