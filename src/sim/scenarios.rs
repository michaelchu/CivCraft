//! Opt-in fixtures used by screenshot review and presentation regressions.
//! Keep scenario setup separate from normal campaign systems.

use super::*;

#[cfg(any(test, feature = "scenarios"))]
mod builder;
pub(crate) mod diplomacy;
#[cfg(any(test, feature = "scenarios"))]
mod naval;
pub(crate) mod regression;
#[cfg(any(test, feature = "scenarios"))]
mod state_presets;
pub(crate) mod ui_review;
