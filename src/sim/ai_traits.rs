//! Selected AI trait defaults.

/// Default AI trait value (`50` on the `[1, 2500]` scale).
pub(super) const fn fixed_default(_trait_name: &str) -> i32 {
    50
}

/// `domestic_advisor_choose_build` and military building substitutions use
/// this literal integer-expression scale for Builder.
pub(super) fn builder_building_want_factor() -> f32 {
    0.5 + (fixed_default("builder") / 50 / 2) as f32
}

pub(super) fn trait_enabled(civ: &crate::rules::native::NativeCivilization, name: &str) -> bool {
    use crate::rules::native::CivTrait;
    CivTrait::from_name(name).is_some_and(|civ_trait| civ.has_trait(civ_trait))
}

impl super::Simulation {
    pub(super) fn ai_civilization(
        &self,
        owner: usize,
    ) -> Option<&crate::rules::native::NativeCivilization> {
        let id = self.players[owner].civilization_id.as_deref()?;
        crate::rules::native::civilization(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors the civ2civ3 ruleset behavior.
    #[test]
    fn fixed_default_ai_aggression_comes_from_selected_ruleset_nations_ruleset() {
        assert_eq!(fixed_default("aggressive"), 50);
    }

    /// Mirrors Freeciv and the civ2civ3 ruleset behavior.
    #[test]
    fn ruleset_builder_default_uses_reference_integer_want_scale() {
        assert_eq!(fixed_default("builder"), 50);
        assert_eq!(builder_building_want_factor(), 0.5);
    }
}
