//! Civ III Great Leader creation odds, loaded from the native settlement rules.
//!
//! Each value is the denominator of a "1 in N" chance per eligible kill. They
//! come from the community guide for Civ III patch 1.29f and live in data so
//! they can be tuned without code changes.

use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GreatLeaderSettings {
    pub(crate) attack_denominator: i32,
    pub(crate) attack_heroic_epic_denominator: i32,
    pub(crate) defense_denominator: i32,
    pub(crate) defense_heroic_epic_denominator: i32,
}

impl GreatLeaderSettings {
    pub(crate) fn campaign() -> &'static Self {
        static SETTINGS: OnceLock<GreatLeaderSettings> = OnceLock::new();
        SETTINGS.get_or_init(|| {
            let native = &crate::rules::native::settlements().great_leader;
            assert!(native.attack_denominator > 0);
            assert!(native.attack_heroic_epic_denominator > 0);
            assert!(native.defense_denominator > 0);
            assert!(native.defense_heroic_epic_denominator > 0);
            Self {
                attack_denominator: native.attack_denominator,
                attack_heroic_epic_denominator: native.attack_heroic_epic_denominator,
                defense_denominator: native.defense_denominator,
                defense_heroic_epic_denominator: native.defense_heroic_epic_denominator,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn great_leader_odds_match_the_design() {
        let settings = GreatLeaderSettings::campaign();
        assert_eq!(settings.attack_denominator, 16);
        assert_eq!(settings.attack_heroic_epic_denominator, 12);
        assert_eq!(settings.defense_denominator, 32);
        assert_eq!(settings.defense_heroic_epic_denominator, 24);
    }
}
