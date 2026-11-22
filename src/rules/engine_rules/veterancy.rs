//! Typed veterancy data from the selected Freeciv `units.ruleset`.

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VeteranLevel {
    pub(crate) name: String,
    pub(crate) base_raise_chance: i32,
    pub(crate) work_raise_chance: i32,
    pub(crate) power_factor: i32,
    pub(crate) move_bonus: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VeteranSystem {
    pub(crate) levels: Vec<VeteranLevel>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VeterancySettings {
    pub(crate) upgrade_veteran_loss: usize,
    pub(crate) autoupgrade_veteran_loss: usize,
}

impl VeteranSystem {
    pub(crate) fn level(&self, index: usize) -> Option<&VeteranLevel> {
        self.levels.get(index)
    }
}
