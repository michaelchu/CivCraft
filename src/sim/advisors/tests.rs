//! Regression tests for the behavior of the parent module.

use super::*;

#[path = "tests/city_economy.rs"]
mod city_economy;
#[path = "tests/defense.rs"]
mod defense;
#[path = "tests/effects.rs"]
mod effects;
#[path = "tests/military_advice.rs"]
mod military_advice;
#[path = "tests/threats.rs"]
mod threats;
#[path = "tests/wonders.rs"]
mod wonders;
