//! Stand-in for the self-test harness in builds without the `scenarios`
//! feature. The desktop entry point rejects `--self-test` before one is
//! created, so it keeps that code path free of conditional compilation.

use crate::client::{Game, UiAction};
use macroquad::prelude::Rect;

pub(crate) enum Playtest {}

impl Playtest {
    pub(crate) fn new(_path: String, _seed: u64) -> Self {
        unreachable!("--self-test is rejected in builds without the scenarios feature")
    }

    pub(crate) fn advance(&mut self, _frame: u32, _game: &mut Game, _buttons: &[(Rect, UiAction)]) {
        match *self {}
    }

    pub(crate) fn finish(&mut self, _hud_count: usize) -> bool {
        match *self {}
    }
}
