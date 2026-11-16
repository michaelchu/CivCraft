//! Deterministic automated gameplay scenarios.
//!
//! The desktop runner and its scripted scenarios live in `playtest::harness`;
//! simulation fixture access is provided through the dedicated playtest API.
//! Both are compiled only with the `scenarios` feature, so a build without it
//! carries no scenario code and rejects `--self-test`.

#[cfg(feature = "scenarios")]
mod harness;
#[cfg(feature = "scenarios")]
mod report;
#[cfg(feature = "scenarios")]
pub(crate) use harness::Playtest;

#[cfg(not(feature = "scenarios"))]
mod unavailable;
#[cfg(not(feature = "scenarios"))]
pub(crate) use unavailable::Playtest;

/// Whether this build can run the desktop self-test.
pub(crate) const AVAILABLE: bool = cfg!(feature = "scenarios");
