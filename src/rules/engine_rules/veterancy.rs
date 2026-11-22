//! Veteran level data as the simulation consumes it.

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

impl VeteranSystem {
    pub(crate) fn level(&self, index: usize) -> Option<&VeteranLevel> {
        self.levels.get(index)
    }
}
