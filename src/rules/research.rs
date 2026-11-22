//! Technology identifiers and the ordered technology catalog.
//!
//! Stable advance IDs retain the Freeciv-derived aliases used by existing
//! effects and requirements. Researchable Civ III advances and all research
//! metadata come from the modular Rust technology catalog.
//! Research eligibility and progress are resolved in `sim::research` and
//! `sim::research_phase`.
//!
//! Catalog identity and order are used by saved player knowledge and UI/AI
//! iteration. Preserve them when editing the ruleset data.

use std::sync::OnceLock;

/// Persisted tech indices are permanent. Slots 0..101 were the original
/// numbering; indices not claimed by a Civ III advance are retired and never
/// reused, so existing saves keep their meaning. 101/102 are Future/Unset.
const STABLE_TECH_COUNT: usize = 101;

fn tech_storage_len_for(catalog: &crate::rules::native::TechnologyCatalog) -> usize {
    catalog
        .technologies
        .iter()
        .map(|tech| tech.runtime_index + 1)
        .max()
        .unwrap_or(0)
        .max(STABLE_TECH_COUNT + 2)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Tech(pub(crate) usize);

pub(crate) struct Advance {
    pub(crate) name: &'static str,
    pub(crate) prerequisites: &'static [Tech],
    pub(crate) requirement_count: i32,
    pub(crate) first_discovery_bonus: bool,
    pub(crate) era: TechEra,
    pub(crate) required_for_era: bool,
    pub(crate) era_gate: Option<TechEra>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TechEra {
    Ancient,
    MiddleAges,
    Industrial,
    Modern,
}

impl TechEra {
    pub(crate) const ALL: [Self; 4] = [
        Self::Ancient,
        Self::MiddleAges,
        Self::Industrial,
        Self::Modern,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Ancient => "Ancient Times",
            Self::MiddleAges => "Middle Ages",
            Self::Industrial => "Industrial Age",
            Self::Modern => "Modern Times",
        }
    }

    pub(crate) fn index(self) -> usize {
        match self {
            Self::Ancient => 0,
            Self::MiddleAges => 1,
            Self::Industrial => 2,
            Self::Modern => 3,
        }
    }
}

/// Built from Civ III technology data.
struct TechCatalog {
    advances: Vec<Option<Advance>>,
}

impl TechCatalog {
    fn get(&'static self, tech: Tech) -> Option<&'static Advance> {
        self.advances.get(tech.0).and_then(Option::as_ref)
    }

    fn is_researchable(&self, tech: Tech) -> bool {
        self.advances.get(tech.0).is_some_and(Option::is_some)
    }
}

#[allow(dead_code)] // Includes aliases and advances reserved for the Civ III ruleset.
impl Tech {
    pub(crate) const POLYTHEISM: Self = Self(61);
    pub(crate) const MONOTHEISM: Self = Self(53);
    pub(crate) const MEDITATION: Self = Self(103);
    pub(crate) const DIVINE_RIGHT: Self = Self(104);
    pub(crate) const FREE_ARTISTRY: Self = Self(87);
    pub(crate) const GENETICS: Self = Self(88);
    pub(crate) const INTEGRATED_DEFENSE: Self = Self(89);
    pub(crate) const LITERATURE: Self = Self(90);
    pub(crate) const MILITARY_TRADITION: Self = Self(91);
    pub(crate) const ROBOTICS: Self = Self(68);
    pub(crate) const SPACE_FLIGHT: Self = Self(72);
    pub(crate) const MUSIC_THEORY: Self = Self(92);
    pub(crate) const PRINTING_PRESS: Self = Self(93);
    pub(crate) const REPLACEABLE_PARTS: Self = Self(94);
    pub(crate) const SATELLITES: Self = Self(95);
    pub(crate) const SCIENTIFIC_METHOD: Self = Self(96);
    pub(crate) const STEAM_POWER: Self = Self(97);
    pub(crate) const SMART_WEAPONS: Self = Self(98);

    // Civ III names for existing CivCraft advances with close equivalents.
    pub(crate) const NATIONALISM: Self = Self(17);
    pub(crate) const ECOLOGY: Self = Self(25);
    pub(crate) const THE_LASER: Self = Self(39);
    pub(crate) const SYNTHETIC_FIBERS: Self = Self(60);

    pub(crate) const STEALTH: Self = Self(73);
    pub(crate) const COMBUSTION: Self = Self(14);
    pub(crate) const MASS_PRODUCTION: Self = Self(46);
    pub(crate) const STEEL: Self = Self(75);
    pub(crate) const FLIGHT: Self = Self(29);
    pub(crate) const ADVANCED_FLIGHT: Self = Self(0);
    pub(crate) const ASTRONOMY: Self = Self(3);
    pub(crate) const CONSTRUCTION: Self = Self(18);
    pub(crate) const ELECTRONICS: Self = Self(23);
    pub(crate) const AMPHIBIOUS_WARFARE: Self = Self(2);
    pub(crate) const ALPHABET: Self = Self(1);
    pub(crate) const BRONZE_WORKING: Self = Self(8);
    pub(crate) const CEREMONIAL_BURIAL: Self = Self(9);
    pub(crate) const WARRIOR_CODE: Self = Self(85);
    pub(crate) const HORSEBACK_RIDING: Self = Self(34);
    pub(crate) const INDUSTRIALIZATION: Self = Self(35);
    pub(crate) const POTTERY: Self = Self(62);
    pub(crate) const IRON_WORKING: Self = Self(37);
    pub(crate) const THE_WHEEL: Self = Self(80);
    pub(crate) const MAP_MAKING: Self = Self(44);
    pub(crate) const MASONRY: Self = Self(45);
    pub(crate) const MAGNETISM: Self = Self(43);
    pub(crate) const NAVIGATION: Self = Self(55);
    pub(crate) const MEDICINE: Self = Self(48);
    pub(crate) const MATHEMATICS: Self = Self(47);
    pub(crate) const EDUCATION: Self = Self(84);
    pub(crate) const ROCKETRY: Self = Self(69);
    pub(crate) const SUPERCONDUCTOR: Self = Self(76);
    pub(crate) const CHIVALRY: Self = Self(11);
    pub(crate) const GUNPOWDER: Self = Self(33);
    pub(crate) const METALLURGY: Self = Self(49);
    pub(crate) const FUTURE: Self = Self(STABLE_TECH_COUNT);
    // Keep all persisted real/Future indices stable. A_UNSET is a goal state,
    // never a researchable/discoverable advance.
    pub(crate) const UNSET: Self = Self(STABLE_TECH_COUNT + 1);

    pub(crate) const COMPUTERS: Self = Self(16);
    pub(crate) const COMMUNISM: Self = Self(15);
    pub(crate) const DEMOCRACY: Self = Self(20);
    pub(crate) const MOTORIZED_TRANSPORTATION: Self = Self(51);
    pub(crate) const MINIATURIZATION: Self = Self(50);
    pub(crate) const REFINING: Self = Self(66);
    pub(crate) const ENGINEERING: Self = Self(24);
    pub(crate) const INVENTION: Self = Self(36);
    pub(crate) const ESPIONAGE: Self = Self(26);
    pub(crate) const ELECTRICITY: Self = Self(22);
    pub(crate) const ECONOMICS: Self = Self(21);
    pub(crate) const MONARCHY: Self = Self(52);
    pub(crate) const MYSTICISM: Self = Self(54);
    pub(crate) const NUCLEAR_POWER: Self = Self(57);
    pub(crate) const SANITATION: Self = Self(70);
    pub(crate) const FISSION: Self = Self(56);
    pub(crate) const THE_CORPORATION: Self = Self(78);
    pub(crate) const THE_REPUBLIC: Self = Self(79);
    pub(crate) const THEOLOGY: Self = Self(81);
    pub(crate) const FEUDALISM: Self = Self(28);
    pub(crate) const FASCISM: Self = Self(99);
    pub(crate) const IRONCLADS: Self = Self(100);

    pub(crate) fn storage_len() -> usize {
        tech_catalog().advances.len()
    }

    /// Every researchable Civ III advance, in stable-index order. Unused slots
    /// (retired Freeciv-only indices) and the Future/Unset sentinels are skipped.
    pub(crate) fn all() -> impl Iterator<Item = Self> {
        (0..Self::storage_len()).filter_map(Self::from_index)
    }

    /// The advance with this stable index, if it is one of `Tech::all()`.
    pub(crate) fn from_index(index: usize) -> Option<Self> {
        tech_catalog()
            .is_researchable(Self(index))
            .then_some(Self(index))
    }

    /// Resolve a native catalog ID or display name (case-insensitive).
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        crate::rules::technologies::runtime_index(name).and_then(Self::from_index)
    }

    pub(crate) fn data(self) -> Option<&'static Advance> {
        tech_catalog().get(self)
    }

    pub(crate) fn name(self) -> &'static str {
        if self == Self::UNSET {
            "No goal"
        } else {
            self.data().map_or("Future Tech", |advance| advance.name)
        }
    }

    pub(crate) fn prerequisites(self) -> &'static [Self] {
        self.data().map_or(&[], |advance| advance.prerequisites)
    }

    /// True when this stable runtime ID names a researchable native campaign
    /// technology. Retired indices and the sentinels are excluded.
    ///
    /// Based on the Civ III technology catalog.
    pub(crate) fn is_researchable_advance(self) -> bool {
        tech_catalog().is_researchable(self)
    }

    /// Direct native prerequisites, preserving the order of the four historical
    /// prerequisite slots. Returns `None` for a retired index or a sentinel.
    ///
    /// Based on the Civ III technology catalog.
    pub(crate) fn catalog_prerequisites(self) -> Option<&'static [Self]> {
        self.is_researchable_advance().then(|| {
            self.data()
                .expect("researchable Civ III advance")
                .prerequisites
        })
    }

    /// Freeciv linear requirement count: this advance plus every unique
    /// transitive prerequisite in the native campaign graph.
    ///
    /// Freeciv's linear requirement count over the Civ III technology graph.
    pub(crate) fn linear_requirement_count(self) -> Option<i32> {
        self.is_researchable_advance().then(|| {
            self.data()
                .expect("researchable Civ III advance")
                .requirement_count
        })
    }

    pub(crate) fn era(self) -> TechEra {
        if self == Self::FUTURE {
            return TechEra::Modern;
        }
        self.data().map_or(TechEra::Ancient, |advance| advance.era)
    }

    /// Civ III marks some advances optional for era advancement.
    pub(crate) fn optional_for_era(self) -> bool {
        self.data().is_some_and(|advance| !advance.required_for_era)
    }

    /// Civ III era advancement requirement.
    pub(crate) fn is_required_for_era(self) -> bool {
        self.is_researchable_advance() && !self.optional_for_era()
    }

    pub(crate) fn era_gate(self) -> Option<TechEra> {
        self.data().and_then(|advance| advance.era_gate)
    }
}

/// An advance as read from Civ III technology data.
struct RawAdvance {
    tech: Tech,
    prerequisites: Vec<Tech>,
    era: TechEra,
    required_for_era: bool,
    era_gate: Option<TechEra>,
}

/// The catalog is built from Civ III technology data.
fn tech_catalog() -> &'static TechCatalog {
    static CATALOG: OnceLock<TechCatalog> = OnceLock::new();
    CATALOG.get_or_init(build_tech_catalog)
}

/// Builds the catalog from Civ III technology data.
fn build_tech_catalog() -> TechCatalog {
    let mut raw_advances = Vec::new();
    let native_catalog = crate::rules::native::technologies();
    let storage_len = tech_storage_len_for(native_catalog);
    let mut seen_runtime_ids = vec![false; storage_len];
    for (line_number, native) in native_catalog.technologies.iter().enumerate() {
        let tech = Tech(native.runtime_index);
        assert!(
            tech.0 < storage_len && tech != Tech::FUTURE && tech != Tech::UNSET,
            "Civ III technology {:?} has invalid stable runtime index {} on row {}",
            native.name,
            tech.0,
            line_number + 1
        );
        assert!(
            !seen_runtime_ids[tech.0],
            "multiple native campaign technologies map to runtime Tech({})",
            tech.0
        );
        seen_runtime_ids[tech.0] = true;

        let prerequisites = native
            .prerequisites
            .iter()
            .map(|name| {
                let prerequisite = native_catalog
                    .technologies
                    .iter()
                    .find(|candidate| {
                        candidate.id == *name || candidate.name.eq_ignore_ascii_case(name)
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "unmapped native campaign prerequisite {name:?} for {:?}",
                            native.name
                        )
                    });
                Tech(prerequisite.runtime_index)
            })
            .collect();
        raw_advances.push(RawAdvance {
            tech,
            prerequisites,
            era: parse_tech_era(native.era),
            required_for_era: native.required_for_era,
            era_gate: native.era_gate.map(parse_tech_era),
        });
    }

    let mut advances: Vec<Option<Advance>> =
        std::iter::repeat_with(|| None).take(storage_len).collect();
    for raw in &raw_advances {
        let requirement_count = linear_requirement_count_for(raw.tech, &raw_advances);
        let prerequisites = Box::leak(raw.prerequisites.clone().into_boxed_slice());
        let native = native_catalog
            .technologies
            .iter()
            .find(|candidate| candidate.runtime_index == raw.tech.0)
            .expect("Civ III runtime technology has a native catalog record");
        let display_name = native
            .display_name
            .as_deref()
            .unwrap_or(&native.name)
            .to_owned();
        advances[raw.tech.0] = Some(Advance {
            name: Box::leak(display_name.into_boxed_str()),
            prerequisites,
            requirement_count,
            first_discovery_bonus: native
                .flags
                .iter()
                .any(|flag| flag.eq_ignore_ascii_case("Bonus_Tech")),
            era: parse_tech_era(native.era),
            required_for_era: native.required_for_era,
            era_gate: native.era_gate.map(parse_tech_era),
        });
    }
    TechCatalog { advances }
}

fn parse_tech_era(value: i32) -> TechEra {
    match value {
        0 => TechEra::Ancient,
        1 => TechEra::MiddleAges,
        2 => TechEra::Industrial,
        3 => TechEra::Modern,
        other => panic!("invalid Civ III technology era {other}"),
    }
}

/// Freeciv's linear requirement count over the Civ III technology graph.
fn linear_requirement_count_for(tech: Tech, graph: &[RawAdvance]) -> i32 {
    let storage_len = graph
        .iter()
        .map(|advance| advance.tech.0 + 1)
        .max()
        .unwrap_or(0);
    let mut seen = vec![false; storage_len];
    let mut pending = vec![tech];
    while let Some(current) = pending.pop() {
        if seen[current.0] {
            continue;
        }
        seen[current.0] = true;
        let advance = graph
            .iter()
            .find(|advance| advance.tech == current)
            .expect("Civ III prerequisite belongs to the native graph");
        pending.extend(advance.prerequisites.iter().copied());
        if let Some(gate_era) = advance.era_gate {
            pending.extend(
                graph
                    .iter()
                    .filter(|candidate| candidate.era == gate_era && candidate.required_for_era)
                    .map(|candidate| candidate.tech),
            );
        }
    }
    seen.into_iter().filter(|included| *included).count() as i32
}

#[cfg(test)]
mod sentinel_tests {
    use super::*;

    #[test]
    fn adding_unset_preserves_every_existing_persisted_advance_index() {
        // Only the 85 Civ III advances are enumerated; retired Freeciv-only
        // indices stay reserved so persisted indices never change meaning.
        assert_eq!(Tech::all().count(), 85);
        assert!(Tech::all().all(|tech| tech.is_researchable_advance()));
        assert!(Tech::from_index(5).is_none());
        assert_eq!(Tech::IRONCLADS.0, 100);
        assert_eq!(Tech::IRONCLADS.name(), "Ironclads");
        assert_eq!(Tech::FUTURE.0, 101);
        assert_eq!(Tech::UNSET.0, 102);
        assert_eq!(Tech::POTTERY.0, 62);
        assert_eq!(
            bincode::serialize(&Tech::FUTURE).unwrap(),
            101_u64.to_le_bytes()
        );
        assert_eq!(
            bincode::deserialize::<Tech>(&102_u64.to_le_bytes()).unwrap(),
            Tech::UNSET
        );
        assert_eq!(Tech::UNSET.name(), "No goal");
        assert!(Tech::UNSET.data().is_none());
        assert!(Tech::UNSET.prerequisites().is_empty());
        assert_eq!(Tech::AMPHIBIOUS_WARFARE.name(), "Amphibious Warfare");
        assert_eq!(Tech::AMPHIBIOUS_WARFARE.era(), TechEra::Industrial);
        let philosophy = Tech::all()
            .find(|tech| tech.name() == "Philosophy")
            .unwrap();
        assert!(
            philosophy
                .data()
                .is_some_and(|advance| advance.first_discovery_bonus)
        );
        assert!(Tech::LITERATURE.optional_for_era());
        assert_eq!(Tech::ENGINEERING.era_gate(), Some(TechEra::Ancient));
    }

    #[test]
    fn compiled_catalog_is_the_campaign_research_graph() {
        assert_eq!(
            tech_catalog()
                .advances
                .iter()
                .filter(|advance| advance.is_some())
                .count(),
            85
        );
        assert_eq!(Tech::from_name("Radio"), None);
        assert!(Tech::IRONCLADS.is_researchable_advance());
        assert!(Tech::IRONCLADS.linear_requirement_count().is_some());
        assert_eq!(
            Tech::ALPHABET.linear_requirement_count(),
            Some(1),
            "a root advance counts itself"
        );
        assert_eq!(
            Tech::ADVANCED_FLIGHT.linear_requirement_count(),
            Some(47),
            "includes transitive era-gate prerequisites and counts shared prerequisites once"
        );
        assert!(
            Tech::ENGINEERING.linear_requirement_count().unwrap() > 10,
            "Middle Ages era-root advance includes Ancient era-gate prerequisites"
        );
        assert!(
            Tech::ROCKETRY.linear_requirement_count().unwrap() > 45,
            "Modern era-root advance includes all prior era-gate prerequisites"
        );
        assert_eq!(
            Tech::ADVANCED_FLIGHT.catalog_prerequisites(),
            Some(
                &[
                    Tech::FLIGHT,
                    Tech::ELECTRONICS,
                    Tech::MOTORIZED_TRANSPORTATION
                ][..]
            ),
            "Native prerequisite order is retained"
        );
        assert_eq!(
            Tech::COMMUNISM.catalog_prerequisites(),
            Some(&[Tech::NATIONALISM][..])
        );
        assert_eq!(
            Tech::MOTORIZED_TRANSPORTATION.catalog_prerequisites(),
            Some(&[Tech::MASS_PRODUCTION][..])
        );
        assert_eq!(
            Tech::SYNTHETIC_FIBERS.catalog_prerequisites(),
            Some(&[Tech::ECOLOGY][..])
        );
        assert_eq!(
            Tech::AMPHIBIOUS_WARFARE.catalog_prerequisites(),
            Some(&[Tech::MASS_PRODUCTION][..]),
            "the shorter campaign name binds to the stable Amphibious Warfare ID"
        );
        assert_eq!(
            Tech::IRONCLADS.catalog_prerequisites(),
            Some(&[Tech::STEAM_POWER][..])
        );
        assert_eq!(
            Tech::IRONCLADS.data().map(|advance| advance.name),
            Some("Ironclads")
        );
    }
}

#[cfg(test)]
mod constant_tests {
    use super::*;

    /// Every `const NAME: Self = Self(N);` must still name the advance at
    /// stable index N, so catalog edits cannot silently retarget a constant.
    #[test]
    fn index_constants_name_the_advance_at_their_index() {
        crate::test_support::assert_index_constants_match_catalog(
            include_str!("research.rs"),
            |index| vec![Tech(index).name().to_owned()],
            70,
        );
    }
}
