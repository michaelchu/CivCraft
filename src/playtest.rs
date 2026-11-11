//! Deterministic automated gameplay scenarios.
//!
//! The desktop runner and its scripted scenarios live in `playtest::harness`;
//! simulation fixture access is provided through the dedicated playtest API.

mod harness;
mod report;
pub(crate) use harness::Playtest;
