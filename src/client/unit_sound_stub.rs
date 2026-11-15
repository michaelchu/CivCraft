//! No-op unit sound backend used by headless and test builds.

use crate::sim::UnitKind;
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct UnitSoundRequest {
    pub(crate) unit_id: u32,
    pub(crate) kind: UnitKind,
    pub(crate) era_index: usize,
    pub(crate) action: String,
    pub(crate) delay_seconds: f64,
}

pub(crate) struct UnitSoundPlayer;

impl UnitSoundPlayer {
    pub(crate) fn new(_enabled: bool) -> Self {
        Self
    }

    pub(crate) async fn update(
        &mut self,
        _paths: impl IntoIterator<Item = (PathBuf, f64)>,
        _muted: bool,
        _volume: u8,
    ) {
    }
}
