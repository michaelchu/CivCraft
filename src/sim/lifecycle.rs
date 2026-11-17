//! Run-local, passive lifecycle accounting. Never serialized into campaigns.
use super::*;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct UnitRecord {
    pub id: u32,
    pub owner: usize,
    pub kind: String,
    pub x: i32,
    pub y: i32,
    pub home_city: Option<u32>,
}
impl From<&Unit> for UnitRecord {
    fn from(unit: &Unit) -> Self {
        Self {
            id: unit.id,
            owner: unit.owner,
            kind: unit.kind.name().into(),
            x: unit.x,
            y: unit.y,
            home_city: unit.home_city,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct CityRecord {
    pub id: u32,
    pub owner: usize,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub population: i32,
}
impl From<&City> for CityRecord {
    fn from(city: &City) -> Self {
        Self {
            id: city.id,
            owner: city.owner,
            name: city.name.clone(),
            x: city.x,
            y: city.y,
            population: city.size,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub(crate) enum UnitOrigin {
    Spawned,
    Built {
        city_id: u32,
    },
    Transferred {
        previous_owner: usize,
        previous_unit_id: u32,
    },
    /// Created by an elite unit's victory; see `great_leader`.
    GreatLeader {
        winner_id: u32,
    },
    Unobserved,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UnitRemoval {
    Destroyed,
    Combat,
    CargoLost,
    Nuclear,
    Attrition,
    FuelExhausted,
    CityLost,
    Expelled,
    TerrainLoss,
    HutBarbarians,
    CivilWar,
    Disbanded,
    UpkeepDisbanded,
    FoundedCity,
    JoinedCity,
    Expended,
    Transferred,
    Unobserved,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TransferReason {
    Conquest,
    Trade,
    Propaganda,
    CivilWar,
    Unobserved,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CityRemoval {
    Conquest,
    PopulationLoss,
    Famine,
    Upkeep,
    Disaster,
    Nuclear,
    Migration,
    Unobserved,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(crate) enum Change {
    UnitCreated {
        unit: UnitRecord,
        origin: UnitOrigin,
    },
    UnitRemoved {
        unit: UnitRecord,
        reason: UnitRemoval,
    },
    UnitTransferred {
        unit: UnitRecord,
        new_owner: usize,
        reason: TransferReason,
    },
    UnitUpgraded {
        unit: UnitRecord,
        new_kind: String,
    },
    CityCreated {
        city: CityRecord,
    },
    CityTransferred {
        city: CityRecord,
        new_owner: usize,
        reason: TransferReason,
    },
    CityDestroyed {
        city: CityRecord,
        reason: CityRemoval,
        by_owner: Option<usize>,
    },
    CityUnobserved {
        city: CityRecord,
    },
}
impl trace::Payload for Change {
    const KIND: &'static str = "lifecycle.changed";
}
impl Change {
    pub(super) fn entities(&self) -> Vec<trace::Entity> {
        use trace::Entity::{City, Civilization, Unit};
        match self {
            Self::UnitCreated { unit, origin } => {
                let mut entities = vec![Unit(unit.id), Civilization(unit.owner)];
                match origin {
                    UnitOrigin::Built { city_id } => entities.push(City(*city_id)),
                    UnitOrigin::GreatLeader { winner_id } => entities.push(Unit(*winner_id)),
                    UnitOrigin::Transferred {
                        previous_owner,
                        previous_unit_id,
                    } => {
                        entities.extend([Civilization(*previous_owner), Unit(*previous_unit_id)]);
                    }
                    _ => {}
                }
                entities
            }
            Self::UnitRemoved { unit, .. } | Self::UnitUpgraded { unit, .. } => {
                vec![Unit(unit.id), Civilization(unit.owner)]
            }
            Self::UnitTransferred {
                unit, new_owner, ..
            } => vec![
                Unit(unit.id),
                Civilization(unit.owner),
                Civilization(*new_owner),
            ],
            Self::CityCreated { city } | Self::CityUnobserved { city } => {
                vec![City(city.id), Civilization(city.owner)]
            }
            Self::CityTransferred {
                city, new_owner, ..
            } => vec![
                City(city.id),
                Civilization(city.owner),
                Civilization(*new_owner),
            ],
            Self::CityDestroyed { city, by_owner, .. } => {
                let mut entities = vec![City(city.id), Civilization(city.owner)];
                if let Some(owner) = by_owner {
                    entities.push(Civilization(*owner));
                }
                entities
            }
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct TimedChange {
    pub turn: i32,
    pub phase: trace::Phase,
    #[serde(flatten)]
    pub change: Change,
}
#[derive(Clone, Default, Debug, Serialize)]
pub(crate) struct UnitTotals {
    pub owner: usize,
    pub kind: String,
    pub initial: u64,
    /// Production and other new spawns; excludes ownership transfers/upgrades.
    pub created: u64,
    /// Subset of created that completed city production.
    pub built: u64,
    pub existing: u64,
    pub destroyed: u64,
    pub consumed: u64,
    pub disbanded: u64,
    pub transferred_in: u64,
    pub transferred_out: u64,
    pub upgraded_in: u64,
    pub upgraded_out: u64,
    pub unobserved_added: u64,
    pub unobserved_removed: u64,
}
#[derive(Clone, Default, Debug, Serialize)]
pub(crate) struct CityTotals {
    pub owner: usize,
    pub initial: u64,
    pub created: u64,
    pub existing: u64,
    pub conquered: u64,
    pub lost_to_conquest: u64,
    pub received: u64,
    pub transferred_out: u64,
    pub destroyed: u64,
    pub unobserved_added: u64,
    pub unobserved_removed: u64,
}
#[derive(Clone, Default, Debug, Serialize)]
pub(crate) struct LifecycleSnapshot {
    pub history_available: bool,
    pub since_turn: Option<i32>,
    pub complete_since_start: bool,
    pub unobserved_changes: u64,
    pub civilizations: BTreeMap<usize, String>,
    pub unit_totals: Vec<UnitTotals>,
    pub city_totals: Vec<CityTotals>,
    /// Only the changes since the previous observation boundary, not full history.
    pub changes: Vec<TimedChange>,
}

pub(super) struct Ledger {
    since_turn: i32,
    unobserved: u64,
    units: BTreeMap<u32, UnitRecord>,
    cities: BTreeMap<u32, CityRecord>,
    unit_totals: BTreeMap<(usize, String), UnitTotals>,
    city_totals: BTreeMap<usize, CityTotals>,
    changes: Vec<TimedChange>,
}
impl Ledger {
    pub(super) fn new(turn: i32, units: Vec<UnitRecord>, cities: Vec<CityRecord>) -> Self {
        let mut ledger = Self {
            since_turn: turn,
            unobserved: 0,
            units: Default::default(),
            cities: Default::default(),
            unit_totals: Default::default(),
            city_totals: Default::default(),
            changes: Vec::new(),
        };
        for unit in units {
            ledger.unit_total(&unit).initial += 1;
            ledger.units.insert(unit.id, unit);
        }
        for city in cities {
            ledger.city_total(city.owner).initial += 1;
            ledger.cities.insert(city.id, city);
        }
        ledger
    }
    fn unit_total(&mut self, unit: &UnitRecord) -> &mut UnitTotals {
        self.unit_totals
            .entry((unit.owner, unit.kind.clone()))
            .or_insert_with(|| UnitTotals {
                owner: unit.owner,
                kind: unit.kind.clone(),
                ..Default::default()
            })
    }
    fn city_total(&mut self, owner: usize) -> &mut CityTotals {
        self.city_totals.entry(owner).or_insert_with(|| CityTotals {
            owner,
            ..Default::default()
        })
    }
    pub(super) fn record(&mut self, turn: i32, phase: trace::Phase, change: Change) {
        match &change {
            Change::UnitCreated { unit, origin } => {
                match origin {
                    UnitOrigin::Built { .. } => {
                        let total = self.unit_total(unit);
                        total.created += 1;
                        total.built += 1;
                    }
                    UnitOrigin::Spawned | UnitOrigin::GreatLeader { .. } => {
                        self.unit_total(unit).created += 1
                    }
                    UnitOrigin::Transferred { .. } => self.unit_total(unit).transferred_in += 1,
                    UnitOrigin::Unobserved => {
                        self.unit_total(unit).unobserved_added += 1;
                        self.unobserved += 1;
                    }
                }
                self.units.insert(unit.id, unit.clone());
            }
            Change::UnitRemoved { unit, reason } => {
                let total = self.unit_total(unit);
                match reason {
                    UnitRemoval::FoundedCity | UnitRemoval::JoinedCity | UnitRemoval::Expended => {
                        total.consumed += 1
                    }
                    UnitRemoval::Disbanded | UnitRemoval::UpkeepDisbanded => total.disbanded += 1,
                    UnitRemoval::Transferred => total.transferred_out += 1,
                    UnitRemoval::Unobserved => {
                        total.unobserved_removed += 1;
                        self.unobserved += 1;
                    }
                    _ => total.destroyed += 1,
                }
                self.units.remove(&unit.id);
            }
            Change::UnitTransferred {
                unit,
                new_owner,
                reason,
            } => {
                if unit.owner != *new_owner {
                    self.unit_total(unit).transferred_out += 1;
                    let mut gained = unit.clone();
                    gained.owner = *new_owner;
                    self.unit_total(&gained).transferred_in += 1;
                    self.units.insert(gained.id, gained);
                }
                if *reason == TransferReason::Unobserved {
                    self.unobserved += 1;
                }
            }
            Change::UnitUpgraded { unit, new_kind } => {
                self.unit_total(unit).upgraded_out += 1;
                let mut upgraded = unit.clone();
                upgraded.kind = new_kind.clone();
                self.unit_total(&upgraded).upgraded_in += 1;
                self.units.insert(upgraded.id, upgraded);
            }
            Change::CityCreated { city } => {
                self.city_total(city.owner).created += 1;
                self.cities.insert(city.id, city.clone());
            }
            Change::CityTransferred {
                city,
                new_owner,
                reason,
            } => {
                if *reason == TransferReason::Conquest {
                    self.city_total(city.owner).lost_to_conquest += 1;
                    self.city_total(*new_owner).conquered += 1;
                } else {
                    self.city_total(city.owner).transferred_out += 1;
                    self.city_total(*new_owner).received += 1;
                }
                if *reason == TransferReason::Unobserved {
                    self.unobserved += 1;
                }
                let mut gained = city.clone();
                gained.owner = *new_owner;
                self.cities.insert(gained.id, gained);
            }
            Change::CityDestroyed { city, reason, .. } => {
                if *reason == CityRemoval::Unobserved {
                    self.city_total(city.owner).unobserved_removed += 1;
                    self.unobserved += 1;
                } else {
                    self.city_total(city.owner).destroyed += 1;
                }
                self.cities.remove(&city.id);
            }
            Change::CityUnobserved { city } => {
                self.city_total(city.owner).unobserved_added += 1;
                self.unobserved += 1;
                self.cities.insert(city.id, city.clone());
            }
        }
        self.changes.push(TimedChange {
            turn,
            phase,
            change,
        });
    }
    /// Detect uninstrumented mutations without inventing a destruction/conquest cause.
    pub(super) fn differences(&self, units: &[UnitRecord], cities: &[CityRecord]) -> Vec<Change> {
        let mut changes = Vec::new();
        let current: BTreeMap<_, _> = units.iter().map(|u| (u.id, u)).collect();
        for unit in self.units.values() {
            if !current.contains_key(&unit.id) {
                changes.push(Change::UnitRemoved {
                    unit: unit.clone(),
                    reason: UnitRemoval::Unobserved,
                });
            }
        }
        for unit in units {
            match self.units.get(&unit.id) {
                None => changes.push(Change::UnitCreated {
                    unit: unit.clone(),
                    origin: UnitOrigin::Unobserved,
                }),
                Some(old) => {
                    if old.owner != unit.owner {
                        changes.push(Change::UnitTransferred {
                            unit: old.clone(),
                            new_owner: unit.owner,
                            reason: TransferReason::Unobserved,
                        });
                    }
                    if old.kind != unit.kind {
                        // The boundary shows a type change, but does not establish how it happened.
                        changes.push(Change::UnitRemoved {
                            unit: UnitRecord {
                                owner: unit.owner,
                                ..old.clone()
                            },
                            reason: UnitRemoval::Unobserved,
                        });
                        changes.push(Change::UnitCreated {
                            unit: unit.clone(),
                            origin: UnitOrigin::Unobserved,
                        });
                    }
                }
            }
        }
        let current: BTreeMap<_, _> = cities.iter().map(|c| (c.id, c)).collect();
        for city in self.cities.values() {
            if !current.contains_key(&city.id) {
                changes.push(Change::CityDestroyed {
                    city: city.clone(),
                    reason: CityRemoval::Unobserved,
                    by_owner: None,
                });
            }
        }
        for city in cities {
            match self.cities.get(&city.id) {
                None => changes.push(Change::CityUnobserved { city: city.clone() }),
                Some(old) if old.owner != city.owner => changes.push(Change::CityTransferred {
                    city: old.clone(),
                    new_owner: city.owner,
                    reason: TransferReason::Unobserved,
                }),
                _ => {}
            }
        }
        changes
    }
    pub(super) fn refresh_rosters(&mut self, units: Vec<UnitRecord>, cities: Vec<CityRecord>) {
        self.units = units.into_iter().map(|u| (u.id, u)).collect();
        self.cities = cities.into_iter().map(|c| (c.id, c)).collect();
    }
    pub(super) fn clear_changes(&mut self) {
        self.changes.clear();
    }
    fn snapshot(&self) -> LifecycleSnapshot {
        LifecycleSnapshot {
            history_available: true,
            since_turn: Some(self.since_turn),
            complete_since_start: self.unobserved == 0,
            unobserved_changes: self.unobserved,
            unit_totals: self.unit_totals.values().cloned().collect(),
            city_totals: self.city_totals.values().cloned().collect(),
            changes: self.changes.clone(),
            ..Default::default()
        }
    }
}

impl trace::Collector {
    pub(super) fn unit_created(&mut self, turn: i32, unit: &Unit, origin: UnitOrigin) {
        self.lifecycle_change(turn, || Change::UnitCreated {
            unit: unit.into(),
            origin,
        });
    }
    pub(super) fn unit_removed(&mut self, turn: i32, unit: &Unit, reason: UnitRemoval) {
        self.lifecycle_change(turn, || Change::UnitRemoved {
            unit: unit.into(),
            reason,
        });
    }
    pub(super) fn unit_transferred(
        &mut self,
        turn: i32,
        unit: &Unit,
        new_owner: usize,
        reason: TransferReason,
    ) {
        if unit.owner != new_owner {
            self.lifecycle_change(turn, || Change::UnitTransferred {
                unit: unit.into(),
                new_owner,
                reason,
            });
        }
    }
    pub(super) fn unit_upgraded(&mut self, turn: i32, unit: &Unit, new_kind: UnitKind) {
        self.lifecycle_change(turn, || Change::UnitUpgraded {
            unit: unit.into(),
            new_kind: new_kind.name().into(),
        });
    }
    pub(super) fn city_created(&mut self, turn: i32, city: &City) {
        self.lifecycle_change(turn, || Change::CityCreated { city: city.into() });
    }
    pub(super) fn city_transferred(
        &mut self,
        turn: i32,
        city: &City,
        new_owner: usize,
        reason: TransferReason,
    ) {
        if city.owner != new_owner {
            self.lifecycle_change(turn, || Change::CityTransferred {
                city: city.into(),
                new_owner,
                reason,
            });
        }
    }
    pub(super) fn city_destroyed(
        &mut self,
        turn: i32,
        city: &City,
        reason: CityRemoval,
        by_owner: Option<usize>,
    ) {
        self.lifecycle_change(turn, || Change::CityDestroyed {
            city: city.into(),
            reason,
            by_owner,
        });
    }
}

impl Simulation {
    pub(crate) fn start_lifecycle_observation(&mut self) {
        self.trace.enable_lifecycle(
            self.turn,
            self.units.iter().map(UnitRecord::from).collect(),
            self.cities.iter().map(CityRecord::from).collect(),
        );
    }
    pub(crate) fn reconcile_lifecycle_observation(&mut self) {
        if self.trace.lifecycle().is_some() {
            self.trace.reconcile_lifecycle(
                self.turn,
                self.units.iter().map(UnitRecord::from).collect(),
                self.cities.iter().map(CityRecord::from).collect(),
            );
        }
    }
    pub(crate) fn observe_lifecycle(&self) -> LifecycleSnapshot {
        let mut snapshot = self
            .trace
            .lifecycle()
            .map(Ledger::snapshot)
            .unwrap_or_default();
        snapshot.civilizations = self
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.name.clone()))
            .collect();
        let mut units: BTreeMap<_, _> = snapshot
            .unit_totals
            .into_iter()
            .map(|row| ((row.owner, row.kind.clone()), row))
            .collect();
        for unit in &self.units {
            let kind = unit.kind.name().to_owned();
            units
                .entry((unit.owner, kind.clone()))
                .or_insert_with(|| UnitTotals {
                    owner: unit.owner,
                    kind,
                    ..Default::default()
                })
                .existing += 1;
        }
        snapshot.unit_totals = units.into_values().collect();
        let mut cities: BTreeMap<_, _> = snapshot
            .city_totals
            .into_iter()
            .map(|row| (row.owner, row))
            .collect();
        for city in &self.cities {
            cities
                .entry(city.owner)
                .or_insert_with(|| CityTotals {
                    owner: city.owner,
                    ..Default::default()
                })
                .existing += 1;
        }
        snapshot.city_totals = cities.into_values().collect();
        snapshot
    }
}

#[cfg(any(test, feature = "scenarios"))]
impl Simulation {
    pub(in crate::sim) fn lifecycle_history_playtest(
        &mut self,
    ) -> Vec<(&'static str, bool, String)> {
        if self.trace.lifecycle().is_none() {
            self.start_lifecycle_observation();
        }
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = Some(Resource::Wheat);
            tile.hut = false;
            tile.owner = None;
        }
        self.create_city(0, 10, 24);
        let factory = self.cities.last().unwrap().id;
        let index = self.city_index_by_id(factory).unwrap();
        self.cities[index].size = 2;
        self.cities[index].specialists = [0; 3];
        self.cities[index].production = Production::Unit(UnitKind::Warrior);
        self.cities[index].shields = Production::Unit(UnitKind::Warrior).cost();
        self.reconcile_city_workers();
        self.process_player_cities(0);
        let built = self
            .units
            .iter()
            .find(|u| u.home_city == Some(factory) && u.kind == UnitKind::Warrior)
            .map(|u| u.id);
        if let Some(id) = built {
            self.remove_units_with_cargo_because(vec![id], UnitRemoval::Combat);
        }
        let founder = self.spawn_unit(0, UnitKind::Settler, 26, 40);
        self.found_city(founder);
        let spare = self.spawn_unit(0, UnitKind::Worker, 10, 24);
        self.disband_unit(spare, None);

        self.create_city(1, 28, 12);
        let enemy = self.cities.last().unwrap().id;
        self.cities.last_mut().unwrap().size = 3;
        self.diplomacy.remove(&Self::relation_key(0, 1));
        let attacker = self.spawn_unit(0, UnitKind::Warrior, 28, 12);
        self.capture_city_if_needed(attacker, 28, 12);
        self.create_city(1, 20, 24);
        let traded = self.cities.last().unwrap().id;
        self.spawn_unit(1, UnitKind::Worker, 20, 24);
        let index = self.city_index_by_id(traded).unwrap();
        self.transfer_traded_city(index, 0);
        self.remove_city_because(enemy, CityRemoval::Disaster, None);
        self.reconcile_lifecycle_observation();
        let history = self.observe_lifecycle();
        let changes = &history.changes;
        vec![
            ("unit_built", changes.iter().any(|e| matches!(&e.change, Change::UnitCreated { unit, origin: UnitOrigin::Built { city_id } } if *city_id == factory && Some(unit.id) == built)), "A real city production completion records its unit ID and producing city".into()),
            ("unit_destroyed", changes.iter().any(|e| matches!(&e.change, Change::UnitRemoved { unit, reason: UnitRemoval::Combat } if Some(unit.id) == built)), "A built unit removed within the same observation interval remains in history".into()),
            ("founder_consumed", changes.iter().any(|e| matches!(&e.change, Change::UnitRemoved { unit, reason: UnitRemoval::FoundedCity } if unit.id == founder)), "Founding consumes its Settler without classifying it as destroyed".into()),
            ("unit_disbanded", changes.iter().any(|e| matches!(&e.change, Change::UnitRemoved { unit, reason: UnitRemoval::Disbanded } if unit.id == spare)), "Voluntary disbanding is distinct from destruction".into()),
            ("city_conquered", changes.iter().any(|e| matches!(&e.change, Change::CityTransferred { city, new_owner: 0, reason: TransferReason::Conquest } if city.id == enemy)), "Conquest records the previous and new owner".into()),
            ("city_traded", changes.iter().any(|e| matches!(&e.change, Change::CityTransferred { city, new_owner: 0, reason: TransferReason::Trade } if city.id == traded)), "A traded city is not classified as conquered".into()),
            ("city_destroyed", changes.iter().any(|e| matches!(&e.change, Change::CityDestroyed { city, reason: CityRemoval::Disaster, .. } if city.id == enemy)), "A conquered city can also be destroyed in the same interval".into()),
            ("history_complete", history.complete_since_start, "All changes reconcile with the live unit and city rosters".into()),
        ]
    }
}

#[cfg(test)]
mod tests;
