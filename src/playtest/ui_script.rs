use super::*;
mod combat;
mod economy;
mod espionage;
mod naval_ai;
mod worker_terrain;

impl Playtest {
    pub(super) fn advance_ui_script(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
    ) {
        self.advance_worker_terrain_ui(frame, game, buttons);
        self.advance_espionage_ui(frame, game, buttons);
        self.advance_combat_ui(frame, game, buttons);
        self.advance_economy_ui(frame, game, buttons);
        self.advance_naval_ai_ui(frame, game, buttons);
    }
}
