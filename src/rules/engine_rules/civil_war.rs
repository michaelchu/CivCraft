//! Selected ruleset settings used by Freeciv's city-conquest civil-war path.

/// Freeciv's default campaign limits relevant to civil-war creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CivilWarRulesetSettings {
    pub(crate) city_threshold: usize,
    pub(crate) max_players: usize,
}
