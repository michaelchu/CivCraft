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

    /// The tile model stores one base even though C2C3's Fortress and Airbase
    /// extras sit on top of Fort and Airstrip. Preserve that underlay when the
    /// higher extra is pillaged.
    pub(super) fn underlying_after_removal(self) -> Option<Self> {
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
        if unit.transport.is_some()
            || self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (unit.x, unit.y))
            || !self.work_action_available_for_unit_fail_closed(
                unit,
                WorkKind::BuildBase(base),
                (unit.x, unit.y),
            )
        {
            return None;
        }
        let time = self.work_activity_time_fail_closed(WorkKind::BuildBase(base), tile.terrain);
        (time > 0).then_some(time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bases_require_their_prerequisites_and_fortresses_protect_without_extra_healing() {
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
        assert_eq!(game.units[1].hp, 1);
        assert!(game.apply(Command::Work(
            warrior,
            WorkKind::Pillage(PillageTarget::Base(Base::Fortress))
        )));
        game.update_activities_and_restore();
        assert_eq!(game.tile(x, y).base, Some(Base::Fort));
    }

    #[test]
    fn c2c3_airstrip_requirements_replace_a_conflicting_fort() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Plains;
        game.tile_mut(x, y).base = Some(Base::Fort);
        game.players[HUMAN].known.push(Tech::RADIO);
        let worker = game.spawn_unit(HUMAN, UnitKind::Worker, x, y);

        assert!(
            game.base_work_time(game.unit_by_id(worker).unwrap(), Base::Airstrip)
                .is_some()
        );
        assert!(game.apply(Command::Work(worker, WorkKind::BuildBase(Base::Airstrip))));
        for _ in 0..10 {
            game.update_activities_and_restore();
        }
        assert_eq!(game.tile(x, y).base, Some(Base::Airstrip));
    }
}
