//! No-op audio backend used by headless and test builds.

use crate::rules::TechEra;

pub(crate) struct MusicPlayer;

impl MusicPlayer {
    pub(crate) fn new(_enabled: bool) -> Self {
        Self
    }

    pub(crate) async fn update(
        &mut self,
        _era: TechEra,
        _culture_group: i32,
        _muted: bool,
        _volume: u8,
    ) {
    }
}
