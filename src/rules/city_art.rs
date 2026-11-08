//! City presentation requirements imported from civ2civ3 and Amplio2.
//! The six civilizations have explicit initial styles; technology-based
//! replacements, image effects and atlas coordinates come from reference data.
use super::{Building, Tech};
use std::{collections::BTreeMap, sync::OnceLock};

struct Style {
    graphic: String,
    base: Option<String>,
    tech: Option<Tech>,
}
struct CityArtRules {
    styles: Vec<Style>,
    size_effects: Vec<(i32, i32)>,
    walls: Building,
    cells: BTreeMap<String, (i32, i32)>,
}

fn sections(source: &str) -> Vec<Vec<&str>> {
    let mut result = Vec::<Vec<&str>>::new();
    for line in source
        .lines()
        .map(|line| line.split(';').next().unwrap().trim())
    {
        if line.starts_with('[') {
            result.push(vec![line]);
        } else if !line.is_empty()
            && let Some(section) = result.last_mut()
        {
            section.push(line);
        }
    }
    result
}
fn quoted(line: &str) -> Vec<&str> {
    line.split('"')
        .enumerate()
        .filter_map(|(i, s)| (i % 2 == 1).then_some(s))
        .collect()
}
fn rules() -> &'static CityArtRules {
    static RULES: OnceLock<CityArtRules> = OnceLock::new();
    RULES.get_or_init(|| {
        let mut styles = Vec::new();
        for section in sections(include_str!("../../assets/civ2civ3_styles.ruleset")) {
            if !section[0].starts_with("[citystyle_") {
                continue;
            }
            let graphic = section.iter().find(|s| s.starts_with("graphic ")).unwrap();
            let req = section
                .iter()
                .find_map(|line| {
                    let q = quoted(line);
                    (q.len() == 3 && q[2] == "Player").then_some(q)
                })
                .unwrap();
            let (base, tech) = if req[0].eq_ignore_ascii_case("style") {
                (Some(req[1].to_owned()), None)
            } else {
                assert!(req[0].eq_ignore_ascii_case("tech"));
                (
                    None,
                    Some(
                        Tech::all()
                            .find(|t| t.name() == req[1])
                            .expect("known city style tech"),
                    ),
                )
            };
            styles.push(Style {
                graphic: quoted(graphic)[0].to_owned(),
                base,
                tech,
            });
        }
        let mut size_effects = Vec::new();
        let mut walls = None;
        for section in sections(include_str!("../../assets/civ2civ3_city_art.ruleset")) {
            let effect = section.iter().find(|s| s.starts_with("type")).unwrap();
            let value: i32 = section
                .iter()
                .find(|s| s.starts_with("value"))
                .unwrap()
                .split('=')
                .nth(1)
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            let req = section
                .iter()
                .find_map(|line| {
                    let q = quoted(line);
                    (q.len() == 3 && q[2] == "City").then_some(q)
                })
                .unwrap();
            match quoted(effect)[0] {
                "City_Image" => {
                    assert_eq!(req[0], "MinSize");
                    size_effects.push((req[1].parse().unwrap(), value));
                }
                "Visible_Walls" => {
                    assert_eq!(req[0], "Building");
                    walls = Some(
                        Building::all()
                            .find(|b| b.data().name == req[1])
                            .expect("wall building"),
                    );
                }
                _ => unreachable!(),
            }
        }
        let mut cells = BTreeMap::new();
        for line in include_str!("../../assets/amplio2_cities.spec").lines() {
            let fields: Vec<_> = line.trim().split(',').map(str::trim).collect();
            if fields.len() >= 3
                && let (Ok(row), Ok(col)) = (fields[0].parse::<i32>(), fields[1].parse::<i32>())
            {
                let tag = quoted(line);
                if let Some(tag) = tag.first() {
                    cells.insert(tag.to_string(), (row, col));
                }
            }
        }
        CityArtRules {
            styles,
            size_effects,
            walls: walls.unwrap(),
            cells,
        }
    })
}

// Closest available Amplio2 historical themes; technological replacements
// continue to follow the imported Civ2Civ3 requirements.
pub(crate) fn initial_style(owner: usize) -> &'static str {
    super::civilizations::CIVILIZATIONS
        .get(owner)
        .map(|civ| civ.city_style)
        .unwrap_or("European")
}

pub(crate) fn city_art_cell(
    style: &str,
    size: i32,
    has_tech: impl Fn(Tech) -> bool,
    has_building: impl Fn(Building) -> bool,
) -> (i32, i32) {
    let rules = rules();
    let style = rules
        .styles
        .iter()
        .rev()
        .find(|candidate| {
            candidate.base.as_deref() == Some(style) || candidate.tech.is_some_and(&has_tech)
        })
        .unwrap_or(&rules.styles[0]);
    let image: i32 = rules
        .size_effects
        .iter()
        .filter(|(min, _)| size >= *min)
        .map(|(_, value)| value)
        .sum();
    let variant = if has_building(rules.walls) {
        "wall"
    } else {
        "city"
    };
    rules.cells[&format!("{}_{variant}_{image}", style.graphic)]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_city_effects_and_style_requirements_choose_all_100_sprites() {
        for (base, row) in [
            ("Celtic", 0),
            ("European", 1),
            ("Classical", 2),
            ("Babylonian", 3),
            ("Asian", 4),
            ("Tropical", 5),
        ] {
            for (size, image) in [
                (1, 0),
                (4, 0),
                (5, 1),
                (8, 1),
                (9, 2),
                (12, 2),
                (13, 3),
                (16, 3),
                (17, 4),
            ] {
                assert_eq!(
                    city_art_cell(base, size, |_| false, |_| false),
                    (row, image)
                );
                assert_eq!(
                    city_art_cell(base, size, |_| false, |_| true),
                    (row, image + 5)
                );
            }
        }
        for (tech, row) in [
            (Tech::STEAM_POWER, 6),
            (Tech::MOTORIZED_TRANSPORTATION, 7),
            (Tech::ROCKETRY, 8),
            (Tech::SUPERCONDUCTORS, 9),
        ] {
            for (size, image) in [(1, 0), (5, 1), (9, 2), (13, 3), (17, 4)] {
                for walls in [false, true] {
                    assert_eq!(
                        city_art_cell("Classical", size, |t| t == tech, |_| walls),
                        (row, image + if walls { 5 } else { 0 })
                    );
                }
            }
        }
        assert_eq!(city_art_cell("Classical", 12, |_| true, |_| false), (9, 2));
    }
}
