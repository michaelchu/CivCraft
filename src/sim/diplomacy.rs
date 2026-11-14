//! Diplomatic state, contact, offers, treaties, and war planning.

use super::*;

mod attitude;
mod contact;
mod observer;
mod offers;
mod pacts;
mod scenarios;
mod vision;
mod war_planning;
#[cfg(test)]
use war_planning::war_planning_game;
#[cfg(test)]
#[path = "diplomacy/contact_tests.rs"]
mod contact_tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Pact {
    #[default]
    War,
    Ceasefire,
    Armistice,
    Peace,
    Alliance,
}

impl Pact {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::War => "War",
            Self::Ceasefire => "Ceasefire",
            Self::Armistice => "Armistice",
            Self::Peace => "Peace",
            Self::Alliance => "Alliance",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TreatyMap {
    World,
    Sea,
}

impl TreatyMap {
    fn flag(self) -> u64 {
        match self {
            Self::World => 2,
            Self::Sea => 4,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Concessions {
    pub(crate) gold: i32,
    pub(crate) technology: Option<Tech>,
    // Shared-vision, map, and embassy flags remain packed; city clauses are a
    // list because Freeciv permits several distinct cities in one treaty.
    access: u64,
    cities: Vec<u32>,
}

impl Concessions {
    const EMBASSY_FLAG: u64 = 1 << 35;
    const VALID_MASK: u64 = 7 | Self::EMBASSY_FLAG;

    pub(crate) fn shared_vision(&self) -> bool {
        self.access & 1 != 0
    }

    pub(crate) fn set_gold(&self, gold: i32) -> Self {
        Self {
            gold: gold.max(0),
            ..self.clone()
        }
    }

    pub(crate) fn set_technology(&self, technology: Option<Tech>) -> Self {
        Self {
            technology,
            ..self.clone()
        }
    }

    pub(crate) fn set_shared_vision(&self, enabled: bool) -> Self {
        let access = if enabled {
            self.access | 1
        } else {
            self.access & !1
        };
        Self {
            access,
            ..self.clone()
        }
    }

    pub(crate) fn set_map(&self, map: Option<TreatyMap>) -> Self {
        let mut access = self.access;
        if let Some(map) = map {
            access |= map.flag();
        } else {
            access &= !(TreatyMap::World.flag() | TreatyMap::Sea.flag());
        }
        Self {
            access,
            ..self.clone()
        }
    }

    pub(crate) fn remove_map(&self, map: TreatyMap) -> Self {
        Self {
            access: self.access & !map.flag(),
            ..self.clone()
        }
    }

    pub(crate) fn set_embassy(&self, enabled: bool) -> Self {
        let access = if enabled {
            self.access | Self::EMBASSY_FLAG
        } else {
            self.access & !Self::EMBASSY_FLAG
        };
        Self {
            access,
            ..self.clone()
        }
    }

    pub(crate) fn set_city(&self, city: Option<u32>) -> Self {
        let mut changed = self.clone();
        if let Some(id) = city {
            if !changed.cities.contains(&id) {
                changed.cities.push(id);
            }
        } else {
            changed.cities.clear();
        }
        changed
    }

    pub(crate) fn remove_city(&self, city: u32) -> Self {
        let mut changed = self.clone();
        changed.cities.retain(|&id| id != city);
        changed
    }

    pub(crate) fn cities(&self) -> &[u32] {
        &self.cities
    }

    pub(crate) fn cities_are_unique(&self) -> bool {
        let mut ids = std::collections::BTreeSet::new();
        self.cities.iter().all(|&id| ids.insert(id))
    }

    pub(crate) fn embassy(&self) -> bool {
        self.access & Self::EMBASSY_FLAG != 0
    }

    fn with_embassy(&self) -> Self {
        self.set_embassy(true)
    }

    fn with_city(&self, id: u32) -> Self {
        self.set_city(Some(id))
    }

    fn valid_access(&self) -> bool {
        self.access & !Self::VALID_MASK == 0
    }

    pub(crate) fn maps(&self) -> impl Iterator<Item = TreatyMap> + '_ {
        [TreatyMap::World, TreatyMap::Sea]
            .into_iter()
            .filter(|map| self.access & map.flag() != 0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TreatyOffer {
    pub(crate) pact: Option<Pact>,
    // Terms are ordered by player ID.
    pub(crate) terms: [Concessions; 2],
}

impl TreatyOffer {
    pub(crate) fn bundled(&self) -> bool {
        self.terms != [Concessions::default(), Concessions::default()]
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(super) enum WarReason {
    Spoils,
    Hatred,
    Excuse,
    Alliance,
    Space,
    Behavior,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct WarPlan {
    pub(super) countdown: i32,
    pub(super) reason: WarReason,
}

// ai/aitraits.c clips the effective trait before daidiplomacy.c computes
// its threshold in love units. Our diplomatic trust uses ten units per love.
fn aggression_war_threshold(value: i32) -> i32 {
    let aggression = value.clamp(1, 2500) as f32;
    ((1000.0 * (0.70 + aggression.sqrt() / 50.0 / 2.0) - 1000.0) as i32) * 10
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct DiplomaticIntel {
    allied_with_enemy: Option<usize>,
    at_war_with_ally: Option<usize>,
    allied_with_ally: Option<usize>,
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct DiplomaticMemory {
    pub(super) spam: i32,
    pub(super) asked_peace: i32,
    pub(super) asked_alliance: i32,
    pub(super) asked_ceasefire: i32,
    pub(super) strongest_pact: Pact,
}

impl DiplomaticMemory {
    fn for_pair(a: usize, b: usize) -> Self {
        // daidata.c:dai_diplomacy_defaults starts spam deterministically.
        Self {
            spam: ((a + b) % 5) as i32,
            ..Self::default()
        }
    }
}

// Retain source incident categories even where the bundled rules do not
// currently grant a casus belli for that action.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(super) enum DiplomaticIncident {
    Snooping,
    Theft,
    UnitWeakening,
    UnitLoss,
    CityLoss,
    BuildingLoss,
    PopulationLoss,
    TerrainLoss,
    ExtraLoss,
    Misc,
    Nuclear,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct Relation {
    pub(super) pact: Pact,
    pub(super) until: i32,
    first_contact: Option<i32>,
    cause_until: [i32; 2],
    pub(super) trust: [i32; 2],
    pub(super) offer: Option<TreatyOffer>,
    vision: [bool; 2],
    embassy: [bool; 2],
    offer_after: i32,
}

impl Simulation {
    /// Returns a treaty only after first contact. Freeciv's no-contact
    /// diplomatic state is distinct from an explicit war despite both using
    /// `Pact::War` as this model's default storage value.
    pub(super) fn contacted_pact(&self, a: usize, b: usize) -> Option<Pact> {
        self.diplomacy
            .get(&Self::relation_key(a, b))
            .filter(|relation| relation.first_contact.is_some())
            .map(|relation| relation.pact)
    }

    pub(super) fn relation_key(a: usize, b: usize) -> (usize, usize) {
        (a.min(b), a.max(b))
    }

    #[cfg(test)]
    pub(super) fn record_diplomatic_incident(
        &mut self,
        violator: usize,
        victim: Option<usize>,
        incident: DiplomaticIncident,
        international: bool,
    ) {
        if violator >= self.players.len() {
            return;
        }
        let badness = match incident {
            DiplomaticIncident::Snooping | DiplomaticIncident::ExtraLoss => 2,
            DiplomaticIncident::Theft
            | DiplomaticIncident::UnitLoss
            | DiplomaticIncident::BuildingLoss
            | DiplomaticIncident::PopulationLoss => 5,
            DiplomaticIncident::UnitWeakening => 3,
            DiplomaticIncident::CityLoss => 10,
            DiplomaticIncident::TerrainLoss => 4,
            DiplomaticIncident::Misc => 1,
            DiplomaticIncident::Nuclear => 0,
        };
        for receiver in 0..self.players.len() {
            if receiver == violator
                || receiver == BARBARIANS
                || !self.players[receiver].ai
                || (!international && victim != Some(receiver))
            {
                continue;
            }
            let loss = if matches!(incident, DiplomaticIncident::Nuclear) {
                if victim == Some(receiver) {
                    300
                } else if victim == Some(violator) {
                    50
                } else {
                    100
                }
            } else if victim == Some(receiver) {
                badness * 1000 * if international { 2 } else { 1 } / 35
            } else if victim == Some(violator) {
                badness
            } else {
                badness * 2
            };
            let relation = self
                .diplomacy
                .entry(Self::relation_key(receiver, violator))
                .or_default();
            let love = &mut relation.trust[usize::from(receiver > violator)];
            *love = (love.saturating_div(10).saturating_sub(loss)).saturating_mul(10);
        }
    }

    fn pact_rank(pact: Pact) -> i32 {
        match pact {
            Pact::War => 0,
            Pact::Ceasefire => 1,
            Pact::Armistice | Pact::Peace => 2,
            Pact::Alliance => 3,
        }
    }

    fn record_war_incident(&mut self, violator: usize, victim: usize, prior: Pact) {
        // call_incident(VICTIM_ONLY) dispatches through the victim's callback;
        // dai_incident returns immediately when that victim is human.
        if !self.players[victim].ai {
            return;
        }
        let strongest = [(violator, victim), (victim, violator)]
            .iter()
            .filter_map(|key| {
                self.ai_diplomatic_memory
                    .get(key)
                    .map(|memory| memory.strongest_pact)
            })
            .chain([prior])
            .max_by_key(|&pact| Self::pact_rank(pact))
            .unwrap_or(Pact::War);
        for receiver in 0..self.players.len() {
            if receiver == violator || receiver == BARBARIANS || !self.players[receiver].ai {
                continue;
            }
            let loss = 33
                + match strongest {
                    Pact::Peace | Pact::Armistice => 33,
                    Pact::Alliance => 100,
                    _ => 0,
                };
            let relation = self
                .diplomacy
                .entry(Self::relation_key(receiver, violator))
                .or_default();
            let trust = &mut relation.trust[usize::from(receiver > violator)];
            let mut love = trust.saturating_div(10).saturating_sub(loss);
            if receiver == victim {
                love = love.saturating_sub(333).min(-1);
            }
            *trust = love.saturating_mul(10);
        }
    }

    pub(super) fn record_provocation(&mut self, victim: usize, offender: usize) {
        if victim != offender && victim != BARBARIANS && offender != BARBARIANS {
            self.initialize_diplomatic_contact(victim, offender);
            let relation = self
                .diplomacy
                .entry(Self::relation_key(victim, offender))
                .or_default();
            relation.first_contact.get_or_insert(self.turn);
            relation.cause_until[usize::from(victim > offender)] = self.turn + 2;
        }
    }

    pub(super) fn action_success_casus_belli(
        &mut self,
        actor: &Unit,
        action: &str,
        victim: Option<usize>,
        target: (i32, i32),
    ) {
        let rules = crate::rules::freeciv::RulesetData::campaign_default();
        let context =
            super::action_rules::SimulationRequirementContext::for_unit_at(self, actor, target)
                .with_action(action)
                .with_relation_target(victim);
        let value =
            crate::rules::freeciv::effect_value(&rules.effects, "Casus_Belli_Success", &context)
                .expect("selected C2C3 Casus_Belli_Success requirements are supported");
        if value > 0
            && let Some(victim) = victim
        {
            self.record_provocation(victim, actor.owner);
        }
    }

    pub(super) fn action_caught_casus_belli(
        &mut self,
        actor: &Unit,
        action: &str,
        victim: Option<usize>,
        target: (i32, i32),
    ) {
        let rules = crate::rules::freeciv::RulesetData::campaign_default();
        let context =
            super::action_rules::SimulationRequirementContext::for_unit_at(self, actor, target)
                .with_action(action)
                .with_relation_target(victim);
        let value =
            crate::rules::freeciv::effect_value(&rules.effects, "Casus_Belli_Caught", &context)
                .expect("selected C2C3 Casus_Belli_Caught requirements are supported");
        if value > 0
            && let Some(victim) = victim
        {
            self.record_provocation(victim, actor.owner);
        }
    }

    pub(super) fn advance_diplomacy(&mut self) {
        let mut transitions = Vec::new();
        for (&(a, b), relation) in &mut self.diplomacy {
            if relation.until > 0 && relation.until <= self.turn {
                relation.pact = if relation.pact == Pact::Armistice {
                    Pact::Peace
                } else {
                    Pact::War
                };
                relation.until = 0;
                relation.offer = None;
                transitions.push((a, b, relation.pact));
            }
        }
        for (a, b, pact) in transitions {
            if pact == Pact::War {
                self.break_mutual_alliances_after_war(a, b);
                self.refresh_visibility();
            }
            if pact == Pact::Peace {
                let expelled: Vec<_> = self
                    .units
                    .iter()
                    .filter(|unit| {
                        unit.owner == a && self.tile(unit.x, unit.y).owner == Some(b)
                            || unit.owner == b && self.tile(unit.x, unit.y).owner == Some(a)
                    })
                    .map(|unit| unit.id)
                    .collect();
                self.units.retain(|unit| {
                    !expelled.contains(&unit.id)
                        && unit.transport.is_none_or(|id| !expelled.contains(&id))
                });
                self.prune_unit_ai_state();
            }
            if a == HUMAN || b == HUMAN {
                let other = if a == HUMAN { b } else { a };
                self.add_event(format!(
                    "{} with {}.",
                    if pact == Pact::Peace {
                        "Peace entered into force"
                    } else {
                        "Ceasefire expired"
                    },
                    self.players[other].name
                ));
            }
        }

        // daidata.c applies new-phase love against the preceding intelligence,
        // then refreshes the relation pointers used by diplomatic actions.
        self.advance_ai_love();
        self.refresh_diplomatic_intelligence();
        self.advance_ai_war_plans();
        self.advance_diplomatic_offers();
    }

    pub(super) fn treaty_blocks_move(&self, unit: &Unit, x: i32, y: i32) -> bool {
        let blocked = |owner| !self.allied(unit.owner, owner) && !self.at_war(unit.owner, owner);
        let occupied = self
            .units
            .iter()
            .any(|other| (other.x, other.y) == (x, y) && blocked(other.owner))
            || self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (x, y) && blocked(city.owner));
        occupied
            || !unit.kind.is_civilian()
                && self
                    .tile(x, y)
                    .owner
                    .is_some_and(|owner| self.pact(unit.owner, owner) == Pact::Peace)
    }
}

#[cfg(test)]
#[path = "diplomacy/tests.rs"]
mod tests;
