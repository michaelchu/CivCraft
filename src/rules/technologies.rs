//! Campaign technology validation and index helpers backed by `content/technologies.toml`.
//!
//! Gameplay edits belong in `data/rulesets/civcraft/content/technologies.toml`.

use super::native::{NativeTechnology, TechnologyCatalog};

/// Resolve author-facing IDs without initializing the runtime research graph.
pub(crate) fn runtime_index(id: &str) -> Option<usize> {
    runtime_index_in(catalog(), id)
}

pub(crate) fn runtime_index_in(catalog: &TechnologyCatalog, id: &str) -> Option<usize> {
    catalog
        .technologies
        .iter()
        .find(|d| d.id == id || d.name.eq_ignore_ascii_case(id))
        .map(|d| d.runtime_index)
        .or_else(|| {
            let suffix = id.strip_prefix("TECH_COMPAT_")?;
            compat_runtime_index(suffix)
                .or_else(|| suffix.parse::<usize>().ok().filter(|i| *i < 101))
        })
}

fn compat_runtime_index(suffix: &str) -> Option<usize> {
    match suffix.to_ascii_lowercase().replace('_', " ").as_str() {
        "automobile" => Some(5),
        "bridge building" => Some(7),
        "combined arms" => Some(13),
        "explosives" => Some(27),
        "fusion power" => Some(30),
        "genetic engineering" => Some(31),
        "guerilla warfare" => Some(32),
        "labor union" => Some(38),
        "leadership" => Some(40),
        "literacy" => Some(41),
        "machine tools" => Some(42),
        "radio" => Some(63),
        "railroad" => Some(64),
        "refrigeration" => Some(67),
        "seafaring" => Some(71),
        "steam engine" => Some(74),
        "tactics" => Some(77),
        "trade" => Some(83),
        _ => None,
    }
}

pub(crate) fn validate(technologies: &[NativeTechnology]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    let mut indices = std::collections::HashSet::new();
    for definition in technologies {
        if definition.id.is_empty()
            || !ids.insert(definition.id.as_str())
            || !indices.insert(definition.runtime_index)
        {
            return Err(format!(
                "duplicate or empty technology identity {}",
                definition.id
            ));
        }
        if [101, 102].contains(&definition.runtime_index)
            || definition.cost < 0
            || !(0..=3).contains(&definition.era)
            || definition
                .era_gate
                .is_some_and(|gate| !(0..=3).contains(&gate))
        {
            return Err(format!("invalid technology values {}", definition.id));
        }
    }
    fn visit(
        index: usize,
        technologies: &[NativeTechnology],
        visiting: &mut [u8],
    ) -> Result<(), String> {
        if visiting[index] == 1 {
            return Err(format!(
                "technology prerequisite cycle at {}",
                technologies[index].id
            ));
        }
        if visiting[index] == 2 {
            return Ok(());
        }
        visiting[index] = 1;
        for prerequisite in &technologies[index].prerequisites {
            let next = technologies
                .iter()
                .position(|d| d.id == *prerequisite || d.name.eq_ignore_ascii_case(prerequisite))
                .ok_or_else(|| format!("unknown technology prerequisite {prerequisite}"))?;
            visit(next, technologies, visiting)?;
        }
        visiting[index] = 2;
        Ok(())
    }
    let mut visiting = vec![0; technologies.len()];
    for index in 0..technologies.len() {
        visit(index, technologies, &mut visiting)?;
    }
    Ok(())
}

pub(crate) fn catalog() -> &'static TechnologyCatalog {
    super::native::technologies()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_prerequisites_cycles_and_duplicate_save_ids() {
        let base = &catalog().technologies[0];
        let make = |id: &str, runtime_index: usize, prerequisites: Vec<String>| NativeTechnology {
            id: id.into(),
            runtime_index,
            name: id.into(),
            display_name: None,
            civilopedia_entry: id.into(),
            cost: base.cost,
            era: base.era,
            advance_icon: base.advance_icon,
            x: base.x,
            y: base.y,
            prerequisites,
            flags: Vec::new(),
            required_for_era: true,
            era_gate: None,
        };
        let mut definitions = vec![make("a", 0, vec!["b".into()]), make("b", 1, Vec::new())];
        assert!(validate(&definitions).is_ok());
        definitions[1].prerequisites = vec!["a".into()];
        assert!(validate(&definitions).unwrap_err().contains("cycle"));
        definitions[1].prerequisites = vec!["missing".into()];
        assert!(validate(&definitions).unwrap_err().contains("unknown"));
        definitions[1].prerequisites = Vec::new();
        definitions[1].runtime_index = 0;
        assert!(validate(&definitions).unwrap_err().contains("duplicate"));
    }

    #[test]
    fn allows_extended_runtime_indices_above_bundled_count() {
        let base = &catalog().technologies[0];
        let mut custom = base.clone();
        custom.id = "TECH_Custom_Orbital".into();
        custom.name = "Orbital Elevators".into();
        custom.runtime_index = 105;
        custom.prerequisites = vec![base.id.clone()];
        let extended = vec![base.clone(), custom];
        assert!(validate(&extended).is_ok());

        for sentinel in [101, 102] {
            let mut bad = base.clone();
            bad.id = format!("TECH_Sentinel_{sentinel}");
            bad.runtime_index = sentinel;
            bad.prerequisites = Vec::new();
            assert!(validate(&[bad]).is_err());
        }
    }

    #[test]
    fn resolves_ids_names_and_readable_compat_aliases() {
        assert_eq!(runtime_index("TECH_Bronze_Working"), Some(8));
        assert_eq!(runtime_index("Bronze Working"), Some(8));
        assert_eq!(runtime_index("TECH_COMPAT_Steam_Engine"), Some(74));
        assert_eq!(runtime_index("TECH_COMPAT_74"), Some(74));
        assert_eq!(runtime_index("TECH_COMPAT_Railroad"), Some(64));
        assert_eq!(runtime_index("TECH_COMPAT_Unknown"), None);
    }

    #[test]
    fn compiled_tree_preserves_pre_migration_gameplay_and_save_ids() {
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/technology_catalog_v1.json"
        ))
        .unwrap();
        let all = &catalog().technologies;
        let actual: Vec<_> = all
            .iter()
            .filter(|t| t.runtime_index < 101)
            .map(|t| {
                let prerequisites: Vec<_> = t
                    .prerequisites
                    .iter()
                    .map(|prereq| {
                        all.iter()
                            .find(|candidate| candidate.id == *prereq)
                            .map_or(prereq.as_str(), |candidate| candidate.name.as_str())
                    })
                    .collect();
                serde_json::json!({
                    "id": t.id, "runtime_index": t.runtime_index, "name": t.name,
                    "cost": t.cost, "era": t.era, "prerequisites": prerequisites,
                    "flags": t.flags, "required_for_era": t.required_for_era, "era_gate": t.era_gate,
                    "display_name": t.display_name, "advance_icon": t.advance_icon,
                    "x": t.x, "y": t.y, "civilopedia_entry": t.civilopedia_entry
                })
            })
            .collect();
        assert_eq!(serde_json::json!(actual), expected);
    }
}

#[cfg(test)]
mod religion_tech_tests {
    use super::*;
    #[test]
    fn religious_extensions_preserve_sentinels_and_are_optional_branches() {
        use crate::rules::Tech;
        assert_eq!(Tech::MEDITATION.prerequisites(), &[Tech::MYSTICISM]);
        assert_eq!(
            Tech::DIVINE_RIGHT.prerequisites(),
            &[Tech::THEOLOGY, Tech::MONARCHY]
        );
        for t in [Tech::MEDITATION, Tech::DIVINE_RIGHT] {
            assert!(t.is_civ3_advance());
            assert!(t.optional_in_civ3());
            assert!(![Tech::FUTURE.0, Tech::UNSET.0].contains(&t.0));
            assert_eq!(
                runtime_index(if t == Tech::MEDITATION {
                    "TECH_Meditation"
                } else {
                    "TECH_Divine_Right"
                }),
                Some(t.0)
            );
        }
    }
}
