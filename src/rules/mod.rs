//! Implements mod behavior within the rules subsystem.

mod buildings;
pub(crate) use buildings::{Building, Genus, Requirement};

mod research;
pub(crate) use research::Tech;

pub(crate) mod city_art;

pub(crate) mod civilizations;
