//! Engine technology names mapped to native, compiled technology identities.

use crate::rules::Tech;
use crate::rules::engine_rules::RulesetData;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FreecivTechBinding {
    Bound(Tech),
    MissingFromCiv3Catalog,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContentBindingError(String);

impl std::fmt::Display for ContentBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ContentBindingError {}

/// Resolve a Freeciv technology universal through the selected ruleset and
/// the explicit Civ III source catalog. Missing source counterparts are only
/// accepted for the documented Freeciv-only `Plastics` advance and for the
/// derived C2C3 Radio-to-Flight airfield mapping.
pub(crate) fn bind_freeciv_technology(
    _ruleset: &RulesetData,
    freeciv_name: &str,
) -> Result<FreecivTechBinding, ContentBindingError> {
    // Requirements are resolved against the Civ III catalog directly.  The
    // selected Freeciv tech record used to be the lookup table here, which
    // made a ruleset content record authoritative even after the native
    // catalog had become the campaign source of truth.
    let display_name = freeciv_name;
    if display_name.eq_ignore_ascii_case("Plastics") {
        return Ok(FreecivTechBinding::MissingFromCiv3Catalog);
    }
    if display_name.eq_ignore_ascii_case("Radio") {
        // C2C3's Radio advance unlocks airstrips and airbases, but Conquests
        // has no researchable Radio node. Its manual makes Flight the
        // prerequisite for the corresponding Airfield worker action. Bind
        // this selected-ruleset requirement to that Civ III advance without
        // adding the legacy Freeciv Radio ID to the Civ III research tree.
        return Ok(FreecivTechBinding::Bound(Tech::FLIGHT));
    }

    // Some C2C3 requirements intentionally name a Freeciv compatibility
    // advance that is not a researchable Conquests node.  Resolve its stable
    // CivCraft equivalent before consulting the 83-node native catalog.
    if let Some(compatibility_tech) = Tech::all()
        .find(|tech| tech.name().eq_ignore_ascii_case(display_name))
        .and_then(|tech| {
            tech.is_civ3_advance()
                .then_some(tech)
                .or_else(|| tech.civ3_equivalent())
        })
    {
        return Ok(FreecivTechBinding::Bound(compatibility_tech));
    }

    let source = crate::rules::native::technologies()
        .technologies
        .iter()
        .find(|source| {
            source.name.eq_ignore_ascii_case(display_name)
                    || (display_name.eq_ignore_ascii_case("Superconductors")
                        && source.name.eq_ignore_ascii_case("Superconductor"))
                    || (display_name.eq_ignore_ascii_case("Amphibious Warfare")
                        && source.name.eq_ignore_ascii_case("Amphibious War"))
                    // Freeciv calls the draft-related advance Conscription;
                    // the Civ III catalog names that advance Nationalism.
                    || (display_name.eq_ignore_ascii_case("Conscription")
                        && source.name.eq_ignore_ascii_case("Nationalism"))
                    || (display_name.eq_ignore_ascii_case("Environmentalism")
                        && source.name.eq_ignore_ascii_case("Ecology"))
                    || (display_name.eq_ignore_ascii_case("Laser")
                        && source.name.eq_ignore_ascii_case("The Laser"))
                    || (display_name.eq_ignore_ascii_case("Mobile Warfare")
                        && source.name.eq_ignore_ascii_case("Motorized Transportation"))
                    || (display_name.eq_ignore_ascii_case("Nuclear Fission")
                        && source.name.eq_ignore_ascii_case("Fission"))
        })
        .ok_or_else(|| {
            ContentBindingError(format!(
                "Freeciv technology {display_name:?} has no Civ III source mapping"
            ))
        })?;
    let tech = Tech::all()
        .find(|tech| {
            tech.name().eq_ignore_ascii_case(&source.name)
                || (source.name.eq_ignore_ascii_case("Amphibious War")
                    && *tech == Tech::AMPHIBIOUS_WARFARE)
        })
        .ok_or_else(|| {
            ContentBindingError(format!(
                "native technology {:?} has no stable Tech mapping",
                source.name
            ))
        })?;
    Ok(FreecivTechBinding::Bound(tech))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn calendar_technology_effects_use_explicit_civ3_bindings() {
        let data_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/rulesets");
        let rules = RulesetData::load_civcraft(&data_root).unwrap();

        assert_eq!(
            bind_freeciv_technology(&rules, "Space Flight"),
            Ok(FreecivTechBinding::Bound(Tech::SPACE_FLIGHT))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Superconductors"),
            Ok(FreecivTechBinding::Bound(Tech::SUPERCONDUCTORS))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Nuclear Fission"),
            Ok(FreecivTechBinding::Bound(Tech::FISSION))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Conscription"),
            Ok(FreecivTechBinding::Bound(Tech::NATIONALISM))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Radio"),
            Ok(FreecivTechBinding::Bound(Tech::FLIGHT))
        );
        assert!(!Tech::RADIO.is_civ3_advance());
        assert_eq!(
            bind_freeciv_technology(&rules, "Environmentalism"),
            Ok(FreecivTechBinding::Bound(Tech::ECOLOGY))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Laser"),
            Ok(FreecivTechBinding::Bound(Tech::THE_LASER))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Mobile Warfare"),
            Ok(FreecivTechBinding::Bound(Tech::MOTORIZED_TRANSPORTATION))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Plastics"),
            Ok(FreecivTechBinding::MissingFromCiv3Catalog)
        );
    }
}
