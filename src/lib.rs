//! Shared application code. Both executables use this single module tree so
//! shared unit tests are compiled and run once.
mod analytics;
mod client;
mod desktop;
mod headless;
mod mapgen;
mod playtest;
mod random;
mod rules;
mod sim;
mod visual_test;
mod world;

pub use desktop::run_desktop;
pub use headless::run_cli as run_headless_cli;

/// Window configuration for the desktop executable's Macroquad entry point.
pub fn desktop_window_conf() -> macroquad::prelude::Conf {
    client::window_conf()
}

pub(crate) const QUICKSAVE_PATH: &str = "civcraft.save";
pub(crate) const AUTOSAVE_PATH: &str = "civcraft-autosave.save";

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
