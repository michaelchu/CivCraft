//! Selected Freeciv AI trait defaults.

use crate::rules::freeciv::RulesetData;

/// Freeciv's default `traitdistribution` is fixed, so omitted nation-specific
/// values use the selected ruleset's `*_default` (or its default midpoint).
/// CivCraft currently has no Freeciv nation-specific trait overrides.
pub(super) fn fixed_default(trait_name: &str) -> i32 {
    let rules = RulesetData::campaign_default();
    let read = |suffix: &str| -> Option<i32> {
        let key = format!("{trait_name}_{suffix}");
        let mut matches = rules.parameters.iter().filter(|parameter| {
            parameter.file.eq_ignore_ascii_case("nations.ruleset")
                && parameter.section.eq_ignore_ascii_case("default_traits")
                && parameter.key.eq_ignore_ascii_case(&key)
        });
        let parameter = matches.next()?;
        assert!(matches.next().is_none(), "duplicate default AI trait {key}");
        Some(
            i32::try_from(
                parameter
                    .integer_value()
                    .unwrap_or_else(|error| panic!("invalid C2C3 AI trait {key}: {error}"))
                    .unwrap_or_else(|| panic!("C2C3 AI trait {key} has no value")),
            )
            .unwrap_or_else(|_| panic!("C2C3 AI trait {key} exceeds i32")),
        )
    };

    let value = read("default").unwrap_or_else(|| {
        let minimum = read("min").unwrap_or(50);
        let maximum = read("max").unwrap_or(50);
        minimum + (maximum - minimum) / 2
    });
    value.clamp(1, 2500)
}

/// Freeciv's `domestic_advisor_choose_build` and military building
/// substitutions use this literal integer-expression scale for Builder.
pub(super) fn builder_building_want_factor() -> f32 {
    0.5 + (fixed_default("builder") / 50 / 2) as f32
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
