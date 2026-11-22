//! The campaign's random disasters.
//!
//! Each disaster strikes a city that meets its requirements with probability
//! `DISASTER_FREQUENCY * frequency` per million per turn. The order is the
//! order Freeciv evaluates them in.

use super::action_enablers::{Req, is, requirements};
use super::loader::resolve_requirements;
use super::{
    Requirement,
    RequirementKind::{Building, Extra, MinSize, Terrain},
    RequirementRange::{Adjacent, City, Tile},
};
use std::sync::OnceLock;

/// What a disaster does to the city it strikes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DisasterEffect {
    Robbery,
    Pollution,
    Fallout,
    ReducePopulation,
    DestroyBuilding,
    EmptyFoodStock,
    EmptyProdStock,
}

pub(crate) struct Disaster {
    /// Short stable identifier.
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    /// Relative chance, multiplied by `DISASTER_FREQUENCY`.
    pub(crate) frequency: i32,
    pub(crate) requirements: Vec<Requirement>,
    /// Applied in this order of precedence by the simulation, not list order.
    pub(crate) effects: &'static [DisasterEffect],
}

struct Spec {
    id: &'static str,
    name: &'static str,
    requirements: &'static [Req],
    effects: &'static [DisasterEffect],
}

const SPECS: &[Spec] = &[
    Spec {
        id: "earthquake",
        name: "Earthquake",
        requirements: &[is(Terrain, "Hills", Tile)],
        effects: &[DisasterEffect::EmptyProdStock],
    },
    Spec {
        id: "flood",
        name: "Flood",
        requirements: &[is(Extra, "River", Adjacent)],
        effects: &[DisasterEffect::EmptyFoodStock],
    },
    Spec {
        id: "fire",
        name: "Fire",
        requirements: &[is(MinSize, "9", City)],
        effects: &[DisasterEffect::DestroyBuilding],
    },
    Spec {
        id: "industrial_accident",
        name: "Industrial Accident",
        requirements: &[is(Building, "Mfg. Plant", City)],
        effects: &[DisasterEffect::ReducePopulation, DisasterEffect::Pollution],
    },
    Spec {
        id: "nuclear_accident",
        name: "Nuclear Accident",
        requirements: &[is(Building, "Nuclear Plant", City)],
        effects: &[DisasterEffect::ReducePopulation, DisasterEffect::Fallout],
    },
    Spec {
        id: "robbery",
        name: "Robbery",
        requirements: &[],
        effects: &[DisasterEffect::Robbery],
    },
];

/// Every disaster has the same relative frequency.
const FREQUENCY: i32 = 10;

pub(crate) fn campaign_disasters() -> &'static [Disaster] {
    static DISASTERS: OnceLock<Vec<Disaster>> = OnceLock::new();
    DISASTERS.get_or_init(|| {
        SPECS
            .iter()
            .map(|spec| {
                let mut reqs =
                    requirements(&format!("disaster_{}", spec.id), "reqs", spec.requirements);
                resolve_requirements(&mut reqs);
                Disaster {
                    id: spec.id,
                    name: spec.name,
                    frequency: FREQUENCY,
                    requirements: reqs,
                    effects: spec.effects,
                }
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_disasters_share_one_frequency() {
        let disasters = campaign_disasters();
        assert_eq!(disasters.len(), 6);
        assert!(disasters.iter().all(|d| d.frequency == 10));
        assert!(disasters.iter().all(|d| !d.effects.is_empty()));
    }
}
