//! Engine technology names mapped to native, compiled technology identities.

use crate::rules::Tech;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EngineTechBinding {
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

/// Resolve a Freeciv technology universal to a native campaign identity.
/// Plastics has no counterpart. Radio maps to Flight for the C2C3 airfield rules.
pub(crate) fn bind_engine_technology(
    freeciv_name: &str,
) -> Result<EngineTechBinding, ContentBindingError> {
    let display_name = freeciv_name;
    if display_name.eq_ignore_ascii_case("Plastics") {
        return Ok(EngineTechBinding::MissingFromCiv3Catalog);
    }
    if display_name.eq_ignore_ascii_case("Radio") {
        // C2C3's Radio advance unlocks airstrips and airbases, but Conquests
        // has no researchable Radio node. Its manual makes Flight the
        // prerequisite for the corresponding Airfield worker action. Bind
        // this selected-ruleset requirement to that Civ III advance without
        // adding the legacy Freeciv Radio ID to the Civ III research tree.
        return Ok(EngineTechBinding::Bound(Tech::FLIGHT));
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
        return Ok(EngineTechBinding::Bound(compatibility_tech));
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
    Ok(EngineTechBinding::Bound(tech))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_technology_effects_use_explicit_civ3_bindings() {
        assert_eq!(
            bind_engine_technology("Space Flight"),
            Ok(EngineTechBinding::Bound(Tech::SPACE_FLIGHT))
        );
        assert_eq!(
            bind_engine_technology("Superconductors"),
            Ok(EngineTechBinding::Bound(Tech::SUPERCONDUCTORS))
        );
        assert_eq!(
            bind_engine_technology("Nuclear Fission"),
            Ok(EngineTechBinding::Bound(Tech::FISSION))
        );
        assert_eq!(
            bind_engine_technology("Conscription"),
            Ok(EngineTechBinding::Bound(Tech::NATIONALISM))
        );
        assert_eq!(
            bind_engine_technology("Radio"),
            Ok(EngineTechBinding::Bound(Tech::FLIGHT))
        );
        assert!(!Tech::RADIO.is_civ3_advance());
        assert_eq!(
            bind_engine_technology("Environmentalism"),
            Ok(EngineTechBinding::Bound(Tech::ECOLOGY))
        );
        assert_eq!(
            bind_engine_technology("Laser"),
            Ok(EngineTechBinding::Bound(Tech::THE_LASER))
        );
        assert_eq!(
            bind_engine_technology("Mobile Warfare"),
            Ok(EngineTechBinding::Bound(Tech::MOTORIZED_TRANSPORTATION))
        );
        assert_eq!(
            bind_engine_technology("Plastics"),
            Ok(EngineTechBinding::MissingFromCiv3Catalog)
        );
    }
}
