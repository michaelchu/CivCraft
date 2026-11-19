//! Registration order and feature availability for every named scenario.

use super::{Scenario, ScenarioKind};

const CATALOG: &[&[Scenario]] = &[
    &[Scenario::new("generated-ai", 1, ScenarioKind::Generated)],
    #[cfg(any(test, feature = "scenarios"))]
    super::definitions::SCENARIOS,
];

pub(crate) fn all() -> impl Iterator<Item = &'static Scenario> {
    CATALOG.iter().flat_map(|group| group.iter())
}

pub(crate) fn find(id: &str) -> Option<&'static Scenario> {
    all().find(|scenario| scenario.id == id)
}
