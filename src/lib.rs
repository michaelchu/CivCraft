//! Shared application code. Both executables use this single module tree so
//! shared unit tests are compiled and run once.
// The headless runner exposes the same engine without the desktop command
// surface. Keep the dead-code lint active in the default, full build.
#![cfg_attr(not(feature = "desktop"), allow(dead_code))]

mod analytics;
#[cfg(feature = "desktop")]
mod client;
#[cfg(feature = "desktop")]
mod desktop;
mod headless;
mod mapgen;
mod random;
mod rules;
mod sim;
#[cfg(feature = "desktop")]
mod visual_test;
mod world;
mod world_setup;

#[cfg(feature = "desktop")]
pub use desktop::run_desktop;
pub use headless::run_cli as run_headless_cli;

/// Window configuration for the desktop executable's Macroquad entry point.
#[cfg(feature = "desktop")]
pub fn desktop_window_conf() -> macroquad::prelude::Conf {
    client::window_conf()
}

#[cfg(feature = "desktop")]
pub(crate) const QUICKSAVE_PATH: &str = "civcraft.save";
#[cfg(feature = "desktop")]
pub(crate) const AUTOSAVE_PATH: &str = "civcraft-autosave.save";

#[cfg(feature = "desktop")]
/// Creates the parent directory for an output path when one is specified.
pub(crate) fn ensure_parent(path: &str) {
    if let Some(parent) = std::path::Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(parent);
    }
}

#[cfg(test)]
mod test_support;
