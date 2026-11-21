//! Frozen entity layouts for campaign formats 0x26 through 0x29.
//!
//! These remote definitions read and write the original bincode sequence and
//! enum tags. The simulation types can evolve by updating the conversion at
//! this boundary without changing the disk fields. Borrowed wrappers avoid
//! copying the full map before encoding it.

use super::*;
use serde::{Deserializer, Serializer};
use std::fmt;
use std::marker::PhantomData;

pub(super) trait Wire: Sized {
    fn write<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;
    fn read<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>;
}

macro_rules! wire_records {
    ($($model:ty => $layout:ident),+ $(,)?) => {
        $(impl Wire for $model {
            fn write<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                $layout::serialize(self, serializer)
            }

            fn read<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                $layout::deserialize(deserializer)
            }
        })+
    };
}

struct Borrowed<'a, T>(&'a T);
struct Decoded<T>(T);

impl<T: Wire> Serialize for Borrowed<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.write(serializer)
    }
}

impl<'de, T: Wire> Deserialize<'de> for Decoded<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::read(deserializer).map(Self)
    }
}

impl<T: Wire> Wire for Option<T> {
    fn write<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_ref().map(Borrowed).serialize(serializer)
    }

    fn read<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<Decoded<T>>::deserialize(deserializer).map(|value| value.map(|value| value.0))
    }
}

impl<T: Wire> Wire for (u64, T) {
    fn write<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.0, Borrowed(&self.1)).serialize(serializer)
    }

    fn read<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (revision, value) = <(u64, Decoded<T>)>::deserialize(deserializer)?;
        Ok((revision, value.0))
    }
}

impl<T: Serialize + for<'de> Deserialize<'de>> Wire for (Production, T) {
    fn write<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (Borrowed(&self.0), &self.1).serialize(serializer)
    }

    fn read<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (production, value) = <(Decoded<Production>, T)>::deserialize(deserializer)?;
        Ok((production.0, value))
    }
}

mod record {
    use super::*;

    pub(super) fn serialize<T: Wire, S: Serializer>(
        value: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.write(serializer)
    }

    pub(super) fn deserialize<'de, T: Wire, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<T, D::Error> {
        T::read(deserializer)
    }
}

pub(super) mod sequence {
    use super::*;

    pub(in crate::sim::save::schema) fn serialize<T: Wire, S: Serializer>(
        values: &[T],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(values.iter().map(Borrowed))
    }

    pub(in crate::sim::save::schema) fn deserialize<'de, T: Wire, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<T>, D::Error> {
        struct Sequence<T>(PhantomData<T>);
        impl<'de, T: Wire> serde::de::Visitor<'de> for Sequence<T> {
            type Value = Vec<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a sequence of saved records")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut input: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(Decoded(value)) = input.next_element()? {
                    values.push(value);
                }
                Ok(values)
            }
        }
        deserializer.deserialize_seq(Sequence(PhantomData))
    }

    pub(in crate::sim::save::schema) fn deserialize_cow<
        'de,
        T: Wire + Clone,
        D: Deserializer<'de>,
    >(
        deserializer: D,
    ) -> Result<Cow<'static, [T]>, D::Error> {
        deserialize(deserializer).map(Cow::Owned)
    }
}

pub(super) mod map {
    use super::*;

    pub(in crate::sim::save::schema) fn serialize<K: Serialize, T: Wire, S: Serializer>(
        values: &BTreeMap<K, T>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_map(values.iter().map(|(key, value)| (key, Borrowed(value))))
    }

    pub(in crate::sim::save::schema) fn deserialize<
        'de,
        K: Ord + Deserialize<'de>,
        T: Wire,
        D: Deserializer<'de>,
    >(
        deserializer: D,
    ) -> Result<BTreeMap<K, T>, D::Error> {
        struct Map<K, T>(PhantomData<(K, T)>);
        impl<'de, K: Ord + Deserialize<'de>, T: Wire> serde::de::Visitor<'de> for Map<K, T> {
            type Value = BTreeMap<K, T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map of saved records")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut input: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = BTreeMap::new();
                while let Some((key, Decoded(value))) = input.next_entry()? {
                    values.insert(key, value);
                }
                Ok(values)
            }
        }
        deserializer.deserialize_map(Map(PhantomData))
    }
}

pub(super) mod observation {
    use super::*;

    pub(in crate::sim::save::schema) fn serialize<S: Serializer>(
        value: &observed_map::ObservedMap,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.write(serializer)
    }

    pub(in crate::sim::save::schema) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Cow<'static, observed_map::ObservedMap>, D::Error> {
        Wire::read(deserializer).map(Cow::Owned)
    }
}

wire_records!(
    Tile => TileWire,
    Unit => UnitWire,
    City => CityWire,
    Player => PlayerWire,
    UnitState => UnitStateWire,
    UnitOrder => UnitOrderWire,
    WorkKind => WorkKindWire,
    Production => ProductionWire,
    Terrain => TerrainWire,
    TerrainBase => TerrainBaseWire,
    Resource => ResourceWire,
    Government => GovernmentWire,
    Base => BaseWire,
    UnitKind => UnitKindWire,
    Contamination => ContaminationWire,
    PillageTarget => PillageTargetWire,
    movement::RouteTile => RouteTileWire,
    MapObservation => MapObservationWire,
    observed_map::ObservedCity => ObservedCityWire,
    observed_map::ObservedMap => ObservedMapWire,
);

#[derive(Serialize, Deserialize)]
#[serde(remote = "Tile")]
struct TileWire {
    #[serde(with = "record")]
    terrain: Terrain,

    #[serde(with = "record")]
    base_terrain: TerrainBase,
    altitude: i32,
    hut: bool,
    river: bool,

    river_edges: u8,
    #[serde(with = "record")]
    resource: Option<Resource>,
    road: bool,
    railroad: bool,
    irrigation: bool,
    farmland: bool,
    pollution: bool,
    fallout: bool,

    craters: bool,
    #[serde(with = "record")]
    base: Option<crate::sim::Base>,
    mine: bool,
    oil_well: bool,
    oil_platform: bool,
    owner: Option<usize>,
    border_city: Option<u32>,
    last_seen_owner: Option<usize>,
    seen: bool,
    visible: bool,
    stealth_visible: bool,
    #[serde(with = "crate::sim::save::known_by")]
    known_by: crate::sim::KnownBy,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Unit")]
struct UnitWire {
    id: u32,
    nationality: usize,
    owner: usize,
    #[serde(with = "record")]
    kind: UnitKind,
    transport: Option<u32>,
    home_city: Option<u32>,
    home_order: u32,

    veteran: usize,

    c2c3_veteran: usize,
    // Not part of any existing wire layout; saved in a trailing section.
    #[serde(skip)]
    created_leader: bool,
    fuel: u8,

    facing: u8,
    x: i32,
    y: i32,
    #[serde(skip)]
    army_max_hp: i32,
    hp: i32,
    moves_left: i32,
    #[serde(with = "record")]
    state: UnitState,
    moved: bool,

    defensive_bombarded: bool,

    air_superiority_shot: bool,
    automated: bool,
    #[serde(with = "record")]
    work_kind: Option<WorkKind>,
    work_left: i32,
    work_done: i32,

    paradropped: bool,

    #[serde(with = "record")]
    order: Option<UnitOrder>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "City")]
struct CityWire {
    id: u32,
    owner: usize,
    x: i32,
    y: i32,
    name: String,
    size: i32,
    specialists: [i32; 3],
    worked_tiles: Vec<(i32, i32)>,
    was_happy: bool,

    anarchy: i32,

    confused_citizens: i32,
    last_plague: Option<i32>,

    history: i32,
    // Culture-building ages are appended by the newest outer save wrapper.
    // Keep this skipped in all frozen entity layouts.
    #[serde(skip)]
    culture_buildings: Vec<CultureBuildingAge>,
    foreign_citizens: Vec<(usize, i32)>,
    airlift_used: bool,
    food: i32,
    shields: i32,
    #[serde(with = "record")]
    production: Production,
    founded_turn: i32,
    bought_production: bool,
    sold_building: bool,
    #[serde(with = "sequence")]
    worklist: Vec<Production>,
    protected_shields: i32,
    #[serde(with = "record")]
    production_before_switch: Option<(Production, i32)>,
    buildings: Vec<Building>,

    #[serde(with = "record")]
    ai_building_choice: Option<(Production, f64)>,

    ai_building_next: i32,

    #[serde(with = "record")]
    ai_settlement_choice: Option<(Production, f64)>,

    ai_settlement_next: i32,

    ai_domestic_tech_wants: Option<Vec<(Tech, i32)>>,

    ai_domestic_tech_rates: [i32; 3],

    ai_domestic_tech_next: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Player")]
struct PlayerWire {
    name: String,

    civilization_id: Option<String>,
    gold: i32,
    rates: [i32; 3],
    #[serde(with = "record")]
    government: crate::sim::Government,

    history: i32,
    #[serde(with = "record")]
    revolution_target: Option<crate::sim::Government>,
    revolution_turn: Option<i32>,

    mobilized: bool,
    known: Vec<Tech>,
    researching: Tech,

    researching_saved: Option<Tech>,
    research_saved: i32,
    research_goal: Tech,
    future_techs: i32,
    research: i32,
    last_science: i32,
    ai: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "UnitState")]
enum UnitStateWire {
    Idle,
    Fortifying,
    Fortified,
    Sentry,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "UnitOrder")]
enum UnitOrderWire {
    Connect {
        x: i32,
        y: i32,
    },
    Patrol {
        origin: (i32, i32),
        destination: (i32, i32),
        returning: bool,
    },
    Bombard {
        x: i32,
        y: i32,
    },
    AirSuperiority {
        x: i32,
        y: i32,
    },
    PrecisionBombard {
        x: i32,
        y: i32,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "WorkKind")]
enum WorkKindWire {
    Road,
    Railroad,
    Irrigation,
    Farmland,
    Cultivate,
    Plant,
    Pillage(#[serde(with = "record")] crate::sim::PillageTarget),
    BuildBase(#[serde(with = "record")] crate::sim::Base),
    Clean(#[serde(with = "record")] crate::sim::Contamination),
    OilWell,
    OilPlatform,
    Mine,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Production")]
enum ProductionWire {
    Unit(#[serde(with = "record")] UnitKind),
    Building(Building),
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Terrain")]
enum TerrainWire {
    Sea,
    Ocean,
    Lake,
    Glacier,
    Jungle,
    Desert,
    Forest,
    Grassland,
    Hills,
    Mountains,
    Plains,
    Marsh,
    Tundra,

    Coast,
    FloodPlains,
    Volcano,
    GrasslandShield,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "TerrainBase")]
enum TerrainBaseWire {
    Grassland,
    Plains,
    Desert,
    Tundra,
    Coast,
    Sea,
    Ocean,
    Ice,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Resource")]
enum ResourceWire {
    Aluminum,
    Coal,
    Horses,
    Iron,
    Oil,
    Rubber,
    Saltpeter,
    Uranium,
    Dyes,
    Furs,
    Gems,
    Incense,
    Ivory,
    Silk,
    Spice,
    Wine,
    Cattle,
    Fish,
    Gold,
    Game,
    Oasis,
    Sugar,
    Tobacco,
    TropicalFruit,
    Whales,
    Wheat,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Government")]
enum GovernmentWire {
    Anarchy,
    Despotism,
    Monarchy,
    Communism,
    Fascism,
    Feudalism,
    Republic,
    Democracy,
    Tribal,
    Fundamentalism,
    Federation,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Base")]
enum BaseWire {
    Fort,
    Fortress,
    Airstrip,
    Airbase,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "UnitKind")]
enum UnitKindWire {
    Settler,
    Worker,
    Scout,
    Explorer,
    Marine,
    ModernParatrooper,
    Warrior,
    Archer,
    Spearman,
    Swordsman,
    Chariot,
    Horseman,
    Pikeman,
    Longbowman,
    Musketman,
    Knight,
    Rifleman,
    Cavalry,
    Infantry,
    Tank,
    MechInfantry,
    ModernArmor,
    Catapult,
    Cannon,
    Artillery,
    RadarArtillery,
    CruiseMissile,
    TacticalNuke,
    #[expect(
        clippy::upper_case_acronyms,
        reason = "Preserve the existing unit identity in saved games and diagnostic output."
    )]
    ICBM,
    Galley,
    Caravel,
    Frigate,
    Galleon,
    Ironclad,
    Transport,
    Carrier,
    Submarine,
    Destroyer,
    Battleship,
    AegisCruiser,
    NuclearSubmarine,
    Fighter,
    Bomber,
    Helicopter,
    JetFighter,
    StealthFighter,
    StealthBomber,
    Leader,
    Army,
    JaguarWarrior,
    Bowman,
    Hoplite,
    Impi,
    Legionary,
    Immortals,
    WarChariot,
    Rider,
    MountedWarrior,
    Musketeer,
    Samurai,
    WarElephant,
    Cossack,
    Panzer,
    ManOWar,
    F15,
    Privateer,
    Keshik,
    Conquistador,
    Berserk,
    Sipahi,
    GallicSwordsman,
    AnsarWarrior,
    NumidianMercenary,
    Hwacha,
    MedievalInfantry,
    Guerilla,
    EnkiduWarrior,
    ThreeManChariot,
    Carrack,
    SwissMercenary,
    Trebuchet,
    ChasquiScout,
    JavelinThrower,
    Dromon,
    Cruiser,
    Crusader,
    AncientCavalry,
    Curragh,
    Paratrooper,
    TOWInfantry,
    Flak,
    MobileSAM,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Contamination")]
enum ContaminationWire {
    Pollution,
    Fallout,
    Craters,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "PillageTarget")]
enum PillageTargetWire {
    Road,
    Railroad,
    Irrigation,
    Farmland,
    Mine,
    OilWell,
    OilPlatform,
    Base(#[serde(with = "record")] Base),
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "movement::RouteTile")]
struct RouteTileWire {
    #[serde(with = "record")]
    terrain: Terrain,
    road: bool,
    railroad: bool,
    river: bool,
    city_owner: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "MapObservation")]
struct MapObservationWire {
    #[serde(with = "record")]
    route: movement::RouteTile,
    owner: Option<usize>,
    ocean: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "observed_map::ObservedCity")]
struct ObservedCityWire {
    #[serde(with = "record")]
    city: City,
    civ3_style: u8,
    civ3_era: u8,
    occupied: bool,
    disorder: bool,
    remembered: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "observed_map::ObservedMap")]
struct ObservedMapWire {
    revision: u64,
    #[serde(with = "map")]
    tiles: BTreeMap<(usize, usize), (u64, Tile)>,
    #[serde(with = "map")]
    cities: BTreeMap<(usize, u32), (u64, ObservedCity)>,
    #[serde(skip)]
    city_owners: BTreeMap<(usize, usize), usize>,
    #[serde(skip)]
    partial_sites: observed_map::PartialCitySites,
}
