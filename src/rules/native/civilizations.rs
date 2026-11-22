//! Civilization catalog schema, validation and accessor.

use super::*;

#[allow(
    dead_code,
    reason = "schema_version and source are required for strict TOML decoding but not read at runtime"
)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CivilizationCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) civilizations: Vec<NativeCivilization>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeCivilization {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) noun: String,
    pub(crate) adjective: String,
    pub(crate) leader: String,
    #[serde(skip)]
    pub(crate) primary_color_index: u8,
    #[serde(skip)]
    pub(crate) secondary_color_index: u8,
    #[serde(skip)]
    pub(crate) city_style: u8,
    pub(crate) starting_techs: Vec<String>,
    pub(crate) cities: Vec<String>,
    #[serde(skip)]
    pub(crate) culture_group_index: i32,
    pub(crate) agricultural: bool,
    pub(crate) commercial: bool,
    pub(crate) expansionist: bool,
    pub(crate) industrious: bool,
    pub(crate) militaristic: bool,
    pub(crate) religious: bool,
    pub(crate) scientific: bool,
    pub(crate) seafaring: bool,
    pub(crate) starting_units: Vec<NativeUnitStack>,
    #[serde(default)]
    pub(crate) bonus_units: Vec<NativeUnitStack>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeUnitStack {
    pub(crate) id: String,
    pub(crate) count: i32,
}

pub(crate) fn parse_civilizations(source: &str) -> Result<CivilizationCatalog, String> {
    let mut catalog: CivilizationCatalog =
        toml::from_str(source).map_err(|error| error.to_string())?;
    for civilization in &mut catalog.civilizations {
        let art = super::bindings::civilization_art(&civilization.id)
            .ok_or_else(|| format!("civilization {} has no art binding", civilization.id))?;
        civilization.primary_color_index = art.primary_color_index;
        civilization.secondary_color_index = art.secondary_color_index;
        civilization.city_style = art.city_style;
        civilization.culture_group_index = art.culture_group_index;
    }
    validate_ids(
        catalog.civilizations.iter().map(|c| c.id.as_str()),
        "civilization",
    )?;
    Ok(catalog)
}

/// The civilization with this exact `id`, using a map built once.
pub(crate) fn civilization(id: &str) -> Option<&'static NativeCivilization> {
    static BY_ID: OnceLock<std::collections::HashMap<&'static str, usize>> = OnceLock::new();
    let by_id = BY_ID.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        for (index, civ) in civilizations().civilizations.iter().enumerate() {
            map.entry(civ.id.as_str()).or_insert(index);
        }
        map
    });
    by_id
        .get(id)
        .map(|&index| &civilizations().civilizations[index])
}

pub(crate) fn civilizations() -> &'static CivilizationCatalog {
    static CATALOG: OnceLock<CivilizationCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        parse_civilizations(include_str!(
            "../../../data/rulesets/civcraft/content/civilizations.toml"
        ))
        .unwrap_or_else(|error| panic!("invalid native civilizations TOML: {error}"))
    })
}

/// The eight Conquests civilization traits, named as in `civilizations.toml`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CivTrait {
    Agricultural,
    Commercial,
    Expansionist,
    Industrious,
    Militaristic,
    Religious,
    Scientific,
    Seafaring,
}

impl CivTrait {
    pub(crate) const ALL: [Self; 8] = [
        Self::Agricultural,
        Self::Commercial,
        Self::Expansionist,
        Self::Industrious,
        Self::Militaristic,
        Self::Religious,
        Self::Scientific,
        Self::Seafaring,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Agricultural => "agricultural",
            Self::Commercial => "commercial",
            Self::Expansionist => "expansionist",
            Self::Industrious => "industrious",
            Self::Militaristic => "militaristic",
            Self::Religious => "religious",
            Self::Scientific => "scientific",
            Self::Seafaring => "seafaring",
        }
    }

    /// Exact lowercase trait name; unknown names are not traits.
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.name() == name)
    }
}

impl NativeCivilization {
    pub(crate) fn has_trait(&self, civ_trait: CivTrait) -> bool {
        match civ_trait {
            CivTrait::Agricultural => self.agricultural,
            CivTrait::Commercial => self.commercial,
            CivTrait::Expansionist => self.expansionist,
            CivTrait::Industrious => self.industrious,
            CivTrait::Militaristic => self.militaristic,
            CivTrait::Religious => self.religious,
            CivTrait::Scientific => self.scientific,
            CivTrait::Seafaring => self.seafaring,
        }
    }
}
