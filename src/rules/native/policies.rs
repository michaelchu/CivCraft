//! Accessors for AI strategy, diplomacy, vassalage and religion catalogs.

use super::*;

pub(crate) use crate::rules::policies::{
    DiplomacyCatalog, FaithDefinition, NegotiationPolicy, RelationshipPolicy, ReligionCatalog,
    StrategyCatalog, VassalageCatalog,
};

pub(crate) fn strategy() -> &'static StrategyCatalog {
    static CATALOG: OnceLock<StrategyCatalog> = OnceLock::new();
    CATALOG.get_or_init(crate::rules::policies::default_strategy)
}

pub(crate) fn diplomacy() -> &'static DiplomacyCatalog {
    static CATALOG: OnceLock<DiplomacyCatalog> = OnceLock::new();
    CATALOG.get_or_init(crate::rules::policies::default_diplomacy)
}

pub(crate) fn vassalage() -> &'static VassalageCatalog {
    static CATALOG: OnceLock<VassalageCatalog> = OnceLock::new();
    CATALOG.get_or_init(crate::rules::policies::default_vassalage)
}

pub(crate) fn religion() -> &'static ReligionCatalog {
    static CATALOG: OnceLock<ReligionCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        crate::rules::policies::parse_religion(
            include_str!("../../../data/rulesets/civcraft/content/religion.toml"),
            technologies(),
        )
        .unwrap_or_else(|error| panic!("invalid native religion rules: {error}"))
    })
}
