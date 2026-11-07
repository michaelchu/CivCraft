use super::*;

mod city_actions;
mod counterintelligence;
mod planning;
mod technology_theft;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SabotageTarget {
    Production,
    Building(Building),
}

#[cfg(test)]
#[path = "espionage/tests.rs"]
mod tests;
