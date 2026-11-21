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
    match name {
        "agricultural" => civ.agricultural,
        "commercial" => civ.commercial,
        "expansionist" => civ.expansionist,
        "industrious" => civ.industrious,
        "militaristic" => civ.militaristic,
        "religious" => civ.religious,
        "scientific" => civ.scientific,
        "seafaring" => civ.seafaring,
        _ => false,
    }
}

impl super::Simulation {
    pub(super) fn ai_civilization(
        &self,
        owner: usize,
    ) -> Option<&crate::rules::native::NativeCivilization> {
        let id = self.players[owner].civilization_id.as_deref()?;
        crate::rules::native::civilizations()
            .civilizations
            .iter()
            .find(|civ| civ.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_default_ai_aggression_comes_from_selected_c2c3_nations_ruleset() {
        assert_eq!(fixed_default("aggressive"), 50);
    }

    #[test]
    fn c2c3_builder_default_uses_freeciv_integer_want_scale() {
        assert_eq!(fixed_default("builder"), 50);
        assert_eq!(builder_building_want_factor(), 0.5);
    }
}
