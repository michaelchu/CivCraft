//! Builds the campaign's rule tables into one validated, indexed ruleset.

use crate::rules::content_adapter::{EngineTechBinding, bind_engine_technology};
use crate::rules::engine_rules::RequirementKind;
use crate::rules::{Building, Tech};
use crate::world::Terrain;

use super::{
    EffectSet, LoadError, Requirement, RequirementValue, ResolvedRequirementValue, RulesetData,
};

impl RulesetData {
    /// Build the campaign ruleset from its Rust tables. The tables are fixed
    /// at compile time, so this fails only when a table names a flag, class,
    /// technology or building that does not exist.
    pub(super) fn build_campaign() -> Result<Self, LoadError> {
        let mut rules = Self {
            effects: EffectSet::from(super::effect_table::campaign_effects()),
            action_enablers: super::action_enablers::campaign_action_enablers(),
            action_index: Default::default(),
        };
        validate_requirement_values(&rules)?;
        resolve_requirement_values(&mut rules);
        rules.build_lookup_indexes();
        Ok(rules)
    }
}

/// Resolve runtime-backed requirement names once, at ruleset load time. Names
/// without a stable CivCraft identity intentionally remain strings: they are
/// still valid Freeciv content, but cannot be evaluated against campaign
/// state by the runtime adapters.
pub(super) fn resolve_requirement_values(rules: &mut RulesetData) {
    let mut effects: Vec<_> = rules.effects.iter().cloned().collect();
    for effect in &mut effects {
        resolve_requirements(&mut effect.requirements);
    }
    rules.effects = effects.into();
    for enabler in &mut rules.action_enablers {
        resolve_requirements(&mut enabler.actor_requirements);
        resolve_requirements(&mut enabler.target_requirements);
    }
}

pub(super) fn resolve_requirements(requirements: &mut [Requirement]) {
    for requirement in requirements {
        let RequirementValue::String(name) = &requirement.name else {
            continue;
        };
        let resolved = match requirement.kind {
            RequirementKind::Tech => resolve_tech(name).map(ResolvedRequirementValue::Tech),
            RequirementKind::Building => {
                resolve_building(name).map(ResolvedRequirementValue::Building)
            }
            RequirementKind::Terrain => {
                resolve_terrain(name).map(ResolvedRequirementValue::Terrain)
            }
            _ => None,
        };
        if let Some(resolved) = resolved {
            requirement.resolved = Some(resolved);
        }
    }
}

fn resolve_tech(name: &str) -> Option<Tech> {
    match bind_engine_technology(name).ok()? {
        EngineTechBinding::Bound(tech) => Some(tech),
        EngineTechBinding::MissingFromCatalog => None,
    }
}

/// Spelling variants Freeciv uses for buildings the Civ III catalog spells
/// differently, as (variant, catalog name).
const BUILDING_ALIASES: [(&str, &str); 2] = [
    ("Harbour", "Harbor"),
    ("Shakespeare's Theatre", "Shakespeare's Theater"),
];

fn resolve_building(name: &str) -> Option<Building> {
    let canonical = BUILDING_ALIASES
        .iter()
        .find(|(variant, _)| variant.eq_ignore_ascii_case(name))
        .map_or(name, |(_, catalog_name)| catalog_name);
    Building::all().find(|building| {
        building.data().name.eq_ignore_ascii_case(name)
            || building.data().name.eq_ignore_ascii_case(canonical)
    })
}

fn resolve_terrain(name: &str) -> Option<Terrain> {
    match name.to_ascii_lowercase().as_str() {
        "ocean" => Some(Terrain::Sea),
        "deep ocean" => Some(Terrain::Ocean),
        "swamp" => Some(Terrain::Marsh),
        _ => [
            Terrain::Sea,
            Terrain::Ocean,
            Terrain::Lake,
            Terrain::Glacier,
            Terrain::Jungle,
            Terrain::Desert,
            Terrain::Forest,
            Terrain::Grassland,
            Terrain::Hills,
            Terrain::Mountains,
            Terrain::Plains,
            Terrain::Marsh,
            Terrain::Tundra,
            Terrain::Coast,
            Terrain::FloodPlains,
            Terrain::Volcano,
            Terrain::GrasslandShield,
        ]
        .into_iter()
        .find(|terrain| terrain.name().eq_ignore_ascii_case(name)),
    }
}

fn validate_requirement_values(rules: &RulesetData) -> Result<(), LoadError> {
    let mut requirements = Vec::new();
    requirements.extend(rules.effects.iter().flat_map(|record| &record.requirements));
    requirements.extend(rules.action_enablers.iter().flat_map(|record| {
        record
            .actor_requirements
            .iter()
            .chain(&record.target_requirements)
    }));

    for requirement in requirements {
        if requirement_value_is_valid(requirement) {
            continue;
        }
        let source = &requirement.source;
        let name =
            requirement_name_text(&requirement.name).unwrap_or_else(|| "<non-scalar>".into());
        return Err(LoadError::at_line(
            &source.file,
            &source.section,
            &source.key,
            source.line,
            format!(
                "unknown or unsupported value {name:?} for Freeciv requirement type {:?}",
                requirement.kind
            ),
        ));
    }
    Ok(())
}

fn requirement_value_is_valid(requirement: &Requirement) -> bool {
    let Some(name) = requirement_name_text(&requirement.name) else {
        return false;
    };
    match requirement.kind {
        RequirementKind::None => true,
        // The campaign defines no named achievements, counters, goods or tile
        // definitions for requirements to refer to.
        RequirementKind::Achievement
        | RequirementKind::Counter
        | RequirementKind::Good
        | RequirementKind::Tiledef
        | RequirementKind::TiledefConnected
        | RequirementKind::Style => false,
        RequirementKind::Action => {
            super::identifiers::contains_case_insensitive(super::identifiers::ACTION_NAMES, &name)
        }
        RequirementKind::Activity => {
            super::identifiers::contains_case_insensitive(super::identifiers::ACTIVITY_NAMES, &name)
        }
        RequirementKind::Tech => bind_engine_technology(&name).is_ok(),
        RequirementKind::Age => integer_value(&name).is_some_and(|value| value > 0),
        RequirementKind::Ai => {
            super::identifiers::contains_case_insensitive(super::identifiers::AI_LEVEL_NAMES, &name)
        }
        RequirementKind::CityStatus => super::identifiers::contains_case_insensitive(
            super::identifiers::CITY_STATUS_NAMES,
            &name,
        ),
        RequirementKind::CityTile => super::identifiers::contains_case_insensitive(
            super::identifiers::CITY_TILE_NAMES,
            &name,
        ),
        RequirementKind::DiplRel
        | RequirementKind::DiplRelTile
        | RequirementKind::DiplRelTileOther
        | RequirementKind::DiplRelUnitAny
        | RequirementKind::DiplRelUnitAnyOther => super::identifiers::contains_case_insensitive(
            super::identifiers::DIPLO_REL_NAMES,
            &name,
        ),
        RequirementKind::Extra => super::extra_by_name(&name).is_some(),
        RequirementKind::ExtraFlag => super::identifiers::contains_case_insensitive(
            super::identifiers::EXTRA_FLAG_NAMES,
            &name,
        ),
        RequirementKind::FormAge => integer_value(&name).is_some_and(|value| value > 0),
        RequirementKind::FutureTechs => integer_value(&name).is_some_and(|value| value > 0),
        RequirementKind::Gov => {
            crate::rules::native::governments().get(&name).is_some()
                || ["Tribal", "Fundamentalism", "Federation"]
                    .iter()
                    .any(|gov| gov.eq_ignore_ascii_case(&name))
        }
        RequirementKind::GovFlag => {
            super::identifiers::contains_case_insensitive(super::identifiers::GOV_FLAG_NAMES, &name)
        }
        RequirementKind::Building => building_name_exists(&name),
        RequirementKind::BuildingFlag => super::identifiers::contains_case_insensitive(
            super::identifiers::BUILDING_FLAG_NAMES,
            &name,
        ),
        RequirementKind::BuildingGenus => super::identifiers::contains_case_insensitive(
            super::identifiers::IMPR_GENUS_NAMES,
            &name,
        ),
        RequirementKind::MaxLatitude | RequirementKind::MinLatitude => {
            integer_value(&name).is_some_and(|value| (-1000..=1000).contains(&value))
        }
        RequirementKind::MaxTopUnitsOnTile | RequirementKind::MaxUnitsOnTile => {
            integer_value(&name).is_some_and(|value| value >= 0)
        }
        RequirementKind::MinCalFrag => integer_value(&name).is_some_and(|value| value >= 0),
        RequirementKind::MinCities
        | RequirementKind::MinCulture
        | RequirementKind::MinForeignPct
        | RequirementKind::MinHitPoints
        | RequirementKind::MinMoveFrags
        | RequirementKind::MinSize
        | RequirementKind::MinTechs
        | RequirementKind::MinVeteran
        | RequirementKind::MinYear => integer_value(&name).is_some_and(|value| {
            if requirement.kind == RequirementKind::MinYear {
                true
            } else {
                value > 0
            }
        }),
        RequirementKind::Nation | RequirementKind::Nationality | RequirementKind::OriginalOwner => {
            name.eq_ignore_ascii_case("Animal Kingdom")
        }
        RequirementKind::NationGroup => name.eq_ignore_ascii_case("Barbarian"),
        RequirementKind::OutputType => super::identifiers::contains_case_insensitive(
            super::identifiers::OUTPUT_TYPE_NAMES,
            &name,
        ),
        RequirementKind::PlayerFlag => super::identifiers::contains_case_insensitive(
            super::identifiers::PLAYER_FLAG_NAMES,
            &name,
        ),
        RequirementKind::PlayerState => super::identifiers::contains_case_insensitive(
            super::identifiers::PLAYER_STATE_NAMES,
            &name,
        ),
        RequirementKind::RoadFlag => super::identifiers::contains_case_insensitive(
            super::identifiers::ROAD_FLAG_NAMES,
            &name,
        ),
        // The campaign's server settings are code constants, not ruleset names.
        RequirementKind::ServerSetting => false,
        RequirementKind::Site => building_name_exists(&name),
        RequirementKind::Specialist => ["Elvis", "scientist", "taxman"]
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&name)),
        RequirementKind::TechFlag => super::identifiers::contains_case_insensitive(
            super::identifiers::TECH_FLAG_NAMES,
            &name,
        ),
        RequirementKind::Terrain => crate::rules::terrain_catalog::terrain_spec(&name).is_some(),
        RequirementKind::TerrainAlter => super::identifiers::contains_case_insensitive(
            super::identifiers::TERRAIN_ALTER_NAMES,
            &name,
        ),
        RequirementKind::TerrainClass => super::identifiers::contains_case_insensitive(
            super::identifiers::TERRAIN_CLASS_NAMES,
            &name,
        ),
        RequirementKind::TerrainFlag => super::identifiers::contains_case_insensitive(
            super::identifiers::TERRAIN_FLAG_NAMES,
            &name,
        ),
        RequirementKind::Topology => {
            super::identifiers::contains_case_insensitive(super::identifiers::TOPOLOGY_NAMES, &name)
        }
        RequirementKind::UnitClassFlag => super::identifiers::contains_case_insensitive(
            super::identifiers::UNIT_CLASS_FLAG_NAMES,
            &name,
        ),
        RequirementKind::UnitClass => super::identifiers::contains_case_insensitive(
            super::identifiers::UNIT_CLASS_NAMES,
            &name,
        ),
        RequirementKind::UnitState => super::identifiers::contains_case_insensitive(
            super::identifiers::UNIT_STATE_NAMES,
            &name,
        ),
        RequirementKind::UnitTypeFlag => super::identifiers::contains_case_insensitive(
            super::identifiers::UNIT_TYPE_FLAG_NAMES,
            &name,
        ),
        RequirementKind::UnitType => unit_name_exists(&name),
        RequirementKind::Wrap => {
            super::identifiers::contains_case_insensitive(super::identifiers::WRAP_NAMES, &name)
        } // A known type without a source-backed resolver is rejected explicitly.
    }
}

fn requirement_name_text(value: &RequirementValue) -> Option<String> {
    match value {
        RequirementValue::String(value) => Some(value.clone()),
        RequirementValue::Integer(value) => Some(value.to_string()),
        RequirementValue::Bool(value) => Some(if *value { "TRUE" } else { "FALSE" }.to_owned()),
    }
}

fn integer_value(value: &str) -> Option<i64> {
    value
        .parse::<i64>()
        .ok()
        .filter(|number| i32::try_from(*number).is_ok())
}

fn building_name_exists(name: &str) -> bool {
    resolve_building(name).is_some()
}

fn unit_name_exists(name: &str) -> bool {
    crate::rules::native::units()
        .units
        .iter()
        .any(|unit| unit.name.eq_ignore_ascii_case(name))
        || ["Barbarian Leader", "Nuclear"]
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod bundle_tests {
    use super::*;

    #[test]
    fn compiled_requirements_and_runtime_use_the_same_technology_aliases() {
        let rules = RulesetData::campaign_default();
        let tech_requirements = rules
            .action_enablers
            .iter()
            .flat_map(|enabler| {
                enabler
                    .actor_requirements
                    .iter()
                    .chain(&enabler.target_requirements)
            })
            .chain(rules.effects.iter().flat_map(|effect| &effect.requirements))
            .chain(
                crate::rules::engine_rules::campaign_extras()
                    .iter()
                    .flat_map(|extra| &extra.requirements),
            )
            .filter(|requirement| requirement.kind == RequirementKind::Tech)
            .collect::<Vec<_>>();
        for (alias, technology) in [("Radio", Tech::FLIGHT), ("Nuclear Fission", Tech::FISSION)] {
            let requirement = tech_requirements
                .iter()
                .find(|req| matches!(&req.name, RequirementValue::String(name) if name.eq_ignore_ascii_case(alias)))
                .unwrap();
            assert_eq!(
                requirement.resolved,
                Some(ResolvedRequirementValue::Tech(technology))
            );
            assert_eq!(
                bind_engine_technology(alias),
                Ok(EngineTechBinding::Bound(technology))
            );
        }
        assert_eq!(
            bind_engine_technology("Conscription"),
            Ok(EngineTechBinding::Bound(Tech::NATIONALISM))
        );
        assert_eq!(
            bind_engine_technology("Plastics"),
            Ok(EngineTechBinding::MissingFromCatalog)
        );
        assert!(bind_engine_technology("Unknown modification").is_err());
    }
}
