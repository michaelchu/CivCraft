//! Frozen diplomacy records for the 0x26–0x2A campaign payload.
//! Format 0x2B appends extra technology clauses outside these old records.
use super::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{borrow::Cow, collections::BTreeMap};

#[derive(Serialize, Deserialize)]
#[serde(remote = "Concessions")]
struct LegacyConcessions {
    gold: i32,
    technology: Option<Tech>,
    #[serde(skip)]
    additional_technologies: Vec<Tech>,
    access: u64,
    cities: Vec<u32>,
}

struct TermsRef<'a>(&'a Concessions);
impl Serialize for TermsRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        LegacyConcessions::serialize(self.0, serializer)
    }
}
#[derive(Deserialize)]
struct TermsOwned(#[serde(with = "LegacyConcessions")] Concessions);

mod terms {
    use super::*;
    pub(super) fn serialize<S: Serializer>(
        value: &[Concessions; 2],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        [TermsRef(&value[0]), TermsRef(&value[1])].serialize(serializer)
    }
    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[Concessions; 2], D::Error> {
        Ok(<[TermsOwned; 2]>::deserialize(deserializer)?.map(|value| value.0))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "TreatyOffer")]
struct LegacyOffer {
    #[serde(skip)]
    vassalage: Option<VassalTerms>,
    pact: Option<Pact>,
    #[serde(with = "terms")]
    terms: [Concessions; 2],
}
struct OfferRef<'a>(&'a TreatyOffer);
impl Serialize for OfferRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        LegacyOffer::serialize(self.0, serializer)
    }
}
#[derive(Deserialize)]
struct OfferOwned(#[serde(with = "LegacyOffer")] TreatyOffer);
mod offer {
    use super::*;
    pub(super) fn serialize<S: Serializer>(
        value: &Option<TreatyOffer>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.as_ref().map(OfferRef).serialize(serializer)
    }
    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<TreatyOffer>, D::Error> {
        Ok(Option::<OfferOwned>::deserialize(deserializer)?.map(|value| value.0))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Relation")]
struct LegacyRelation {
    pact: Pact,
    until: i32,
    first_contact: Option<i32>,
    cause_until: [i32; 2],
    trust: [i32; 2],
    #[serde(with = "offer")]
    offer: Option<TreatyOffer>,
    vision: [bool; 2],
    embassy: [bool; 2],
    spy: [Option<espionage::SpyAsset>; 2],
    failed_spy_plant_turn: [Option<i32>; 2],
    military_plans_until: [i32; 2],
    offer_after: i32,
}
struct RelationRef<'a>(&'a Relation);
impl Serialize for RelationRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        LegacyRelation::serialize(self.0, serializer)
    }
}
#[derive(Deserialize)]
struct RelationOwned(#[serde(with = "LegacyRelation")] Relation);
type Relations = BTreeMap<(usize, usize), Relation>;

pub(in crate::sim) fn serialize<S: Serializer>(
    value: &Relations,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    let mut map = serializer.serialize_map(Some(value.len()))?;
    for (pair, relation) in value.iter() {
        map.serialize_entry(pair, &RelationRef(relation))?;
    }
    map.end()
}

pub(in crate::sim) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Cow<'static, Relations>, D::Error> {
    Ok(Cow::Owned(
        BTreeMap::<(usize, usize), RelationOwned>::deserialize(deserializer)?
            .into_iter()
            .map(|(pair, relation)| (pair, relation.0))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_terms_keep_the_original_four_field_binary_layout() {
        let terms = Concessions::default()
            .set_gold(17)
            .add_technology(Tech::ALPHABET)
            .add_technology(Tech::POTTERY)
            .set_map(Some(TreatyMap::World));
        let bytes = bincode::serialize(&TermsRef(&terms)).unwrap();
        let old =
            bincode::serialize(&(17_i32, Some(Tech::ALPHABET), 2_u64, Vec::<u32>::new())).unwrap();
        assert_eq!(bytes, old);
        let loaded: TermsOwned = bincode::deserialize(&bytes).unwrap();
        assert_eq!(
            loaded.0.technologies().collect::<Vec<_>>(),
            [Tech::ALPHABET]
        );
    }
}
