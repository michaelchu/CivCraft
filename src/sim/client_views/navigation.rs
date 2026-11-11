//! Route and tile ownership projections.

use super::*;

impl Simulation {
    pub(crate) fn goto_preview_view(&self, unit_id: u32, x: i32, y: i32) -> Option<GotoPreview> {
        self.preview_goto(unit_id, x, y)
    }
}
