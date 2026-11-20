//! Owner-authorized religion: bounded influence, automatic spread and public faith.
use super::notifications::{NotificationCategory, NotificationPriority};
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

mod ai;
mod spread;
#[cfg(test)]
mod tests;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rules {
    faiths: Vec<FaithDefinition>,
    ai: ai::AiRules,
    founding_influence: i32,
    maximum_influence: i32,
    unaligned_weight: i32,
    presence_percent: i32,
    conversion_percent: i32,
    retention_percent: i32,
    lead_percent: i32,
    decay_divisor: i32,
    holy_pressure: i32,
    community_pressure: i32,
    local_distance: i32,
    local_pressure: i32,
    trade_pressure: i32,
    incoming_cap: i32,
    cooldown_turns: i32,
    tolerant_culture: i32,
    devout_content: i32,
    devout_pressure: i32,
    missionary_pressure: i32,
    shared_attitude: i32,
    devout_difference: i32,
    missionary_difference: i32,
    ai_minimum_improvement: i32,
}
fn rules() -> &'static Rules {
    static RULES: OnceLock<Rules> = OnceLock::new();
    RULES.get_or_init(|| {
        let r: Rules = toml::from_str(include_str!(
            "../../data/rulesets/civcraft/content/religion.toml"
        ))
        .expect("religion rules");
        assert!((1..=10000).contains(&r.maximum_influence));
        assert!((1..=r.maximum_influence).contains(&r.founding_influence));
        assert!((1..=1000).contains(&r.unaligned_weight));
        assert!((1..=1000).contains(&r.decay_divisor));
        assert!((1..=100).contains(&r.presence_percent));
        assert!(
            r.presence_percent <= r.retention_percent
                && r.retention_percent < r.conversion_percent
                && r.conversion_percent <= 100
        );
        assert!((1..=100).contains(&r.lead_percent));
        for v in [
            r.holy_pressure,
            r.community_pressure,
            r.local_pressure,
            r.trade_pressure,
            r.incoming_cap,
            r.devout_pressure,
            r.missionary_pressure,
        ] {
            assert!((0..=100).contains(&v));
        }
        assert!((1..=10).contains(&r.local_distance));
        assert!((1..=100).contains(&r.cooldown_turns));
        for v in [
            r.tolerant_culture,
            r.devout_content,
            r.ai_minimum_improvement,
        ] {
            assert!((0..=10).contains(&v));
        }
        assert!((0..=20).contains(&r.shared_attitude));
        assert!(
            (-20..=0).contains(&r.devout_difference)
                && (-20..=0).contains(&r.missionary_difference)
        );
        r.ai.validate();
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        assert_eq!(r.faiths.len(), 7);
        for f in &r.faiths {
            assert!((1..=7).contains(&f.id) && ids.insert(f.id) && names.insert(f.name.clone()));
            assert!(Tech::all().any(|t| t.is_civ3_advance() && t.name() == f.tech_name()));
            for v in [
                f.content,
                f.culture,
                f.gold,
                f.science,
                f.local_pressure,
                f.outgoing_pressure,
                f.trade_pressure,
            ] {
                assert!((0..=2).contains(&v));
            }
        }
        r
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FaithDefinition {
    pub(crate) id: u32,
    pub(crate) name: String,
    tech: String,
    #[serde(default)]
    content: i32,
    #[serde(default)]
    culture: i32,
    #[serde(default)]
    gold: i32,
    #[serde(default)]
    science: i32,
    #[serde(default)]
    local_pressure: i32,
    #[serde(default)]
    outgoing_pressure: i32,
    #[serde(default)]
    trade_pressure: i32,
}
impl FaithDefinition {
    fn tech(&self) -> Tech {
        Tech::all()
            .find(|t| {
                t.name()
                    == self
                        .tech
                        .strip_prefix("TECH_")
                        .expect("religion tech ID")
                        .replace('_', " ")
            })
            .expect("religion technology")
    }
    fn tech_name(&self) -> &'static str {
        self.tech().name()
    }
    pub(crate) fn bonus_description(&self) -> String {
        let mut parts = Vec::new();
        for (v, label) in [
            (self.content, "content citizen"),
            (self.culture, "culture"),
            (self.gold, "gold"),
            (self.science, "science"),
            (self.local_pressure, "local influence"),
            (self.outgoing_pressure, "outgoing pressure"),
            (self.trade_pressure, "trade spread pressure"),
        ] {
            if v > 0 {
                parts.push(format!("+{v} {label}"));
            }
        }
        parts.join(", ")
    }
}
fn definition(id: u32) -> Option<&'static FaithDefinition> {
    rules().faiths.iter().find(|f| f.id == id)
}

pub(crate) type ReligionId = u32;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum FaithPolicy {
    #[default]
    Tolerant,
    Devout,
    Missionary,
}
impl FaithPolicy {
    pub(crate) const ALL: [Self; 3] = [Self::Tolerant, Self::Devout, Self::Missionary];
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Tolerant => "Tolerant",
            Self::Devout => "Devout",
            Self::Missionary => "Missionary",
        }
    }
    #[cfg(feature = "desktop")]
    pub(crate) fn description(self) -> String {
        let r = rules();
        match self {
            Self::Tolerant => format!(
                "+{} culture in cities with a dominant faith; no different-faith penalty.",
                r.tolerant_culture
            ),
            Self::Devout => format!(
                "+{} content citizen, +{} local influence in declared-faith cities; {} attitude to other faiths.",
                r.devout_content, r.devout_pressure, r.devout_difference
            ),
            Self::Missionary => format!(
                "+{} outgoing declared-faith pressure; {} attitude to other faiths.",
                r.missionary_pressure, r.missionary_difference
            ),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Religion {
    pub(crate) id: ReligionId,
    pub(crate) name: String,
    pub(crate) emblem: u8,
    pub(crate) founder: usize,
    pub(crate) holy_city: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(in crate::sim) struct CityFaith {
    influence: BTreeMap<ReligionId, i32>,
    dominant: Option<ReligionId>,
    announced: BTreeSet<ReligionId>,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub(crate) struct NationalFaith {
    pub(crate) declared: Option<ReligionId>,
    pub(crate) policy: FaithPolicy,
    changed: Option<i32>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(in crate::sim) struct ReligionState {
    pub(crate) religions: Vec<Religion>,
    pub(in crate::sim) cities: BTreeMap<u32, CityFaith>,
    nations: BTreeMap<usize, NationalFaith>,
}
#[derive(Serialize)]
struct ReligionEvent {
    action: &'static str,
    owner: usize,
    city: Option<u32>,
    faith: Option<ReligionId>,
    policy: Option<FaithPolicy>,
}
impl trace::Payload for ReligionEvent {
    const KIND: &'static str = "religion.change";
}

fn dominant(state: &CityFaith) -> Option<ReligionId> {
    let r = rules();
    let total = r.unaligned_weight + state.influence.values().sum::<i32>();
    let candidate = state
        .influence
        .iter()
        .max_by_key(|(id, v)| (**v, std::cmp::Reverse(**id)));
    if let Some((&id, &value)) = candidate {
        let second = state
            .influence
            .iter()
            .filter(|(other, _)| **other != id)
            .map(|(_, v)| *v)
            .max()
            .unwrap_or(0);
        if value * 100 >= total * r.conversion_percent
            && (value - second) * 100 >= total * r.lead_percent
        {
            return Some(id);
        }
    }
    state.dominant.filter(|id| {
        state.influence.get(id).copied().unwrap_or(0) * 100 >= total * r.retention_percent
    })
}
impl Simulation {
    pub(crate) fn national_faith(&self, owner: usize) -> NationalFaith {
        self.religion
            .nations
            .get(&owner)
            .copied()
            .unwrap_or_default()
    }
    pub(crate) fn religion_name(&self, id: ReligionId) -> &str {
        self.religion
            .religions
            .iter()
            .find(|f| f.id == id)
            .map_or("Unknown faith", |f| f.name.as_str())
    }
    pub(crate) fn religion_cooldown(&self, owner: usize) -> i32 {
        self.national_faith(owner).changed.map_or(0, |t| {
            t.saturating_add(rules().cooldown_turns)
                .saturating_sub(self.turn)
                .max(0)
        })
    }
    pub(crate) fn can_found_religion(&self, owner: usize, city: u32, id: u32) -> bool {
        owner < self.civilization_count
            && owner != BARBARIANS
            && self.game_over.is_none()
            && self.city_by_id(city).is_some_and(|c| c.owner == owner)
            && !self
                .religion
                .religions
                .iter()
                .any(|f| f.founder == owner || f.id == id)
            && definition(id).is_some_and(|f| self.has_tech(owner, f.tech()))
    }
    pub(in crate::sim) fn found_religion(&mut self, owner: usize, city: u32, id: u32) -> bool {
        if !self.can_found_religion(owner, city, id) {
            return false;
        }
        let f = definition(id).unwrap();
        let name = &f.name;
        self.religion.religions.push(Religion {
            id,
            name: name.clone(),
            emblem: (id - 1) as u8,
            founder: owner,
            holy_city: city,
        });
        let state = self.religion.cities.entry(city).or_default();
        state.influence.insert(id, rules().founding_influence);
        state.dominant = dominant(state);
        state.announced.insert(id);
        self.religion_event("founded", owner, Some(city), Some(id), None);
        if owner == HUMAN {
            self.add_contextual_notification(
                format!("You founded {name}. Choose a declared faith and policy in Religion."),
                NotificationCategory::Religion,
                NotificationPriority::Important,
                Some(city),
                Some(owner),
            );
        }
        true
    }
    pub(crate) fn eligible_faiths(&self, owner: usize) -> Vec<ReligionId> {
        self.religion
            .religions
            .iter()
            .filter(|f| {
                self.cities
                    .iter()
                    .filter(|c| c.owner == owner)
                    .any(|c| self.faith_present(c.id, f.id))
            })
            .map(|f| f.id)
            .collect()
    }
    fn faith_present(&self, city: u32, id: ReligionId) -> bool {
        self.religion.cities.get(&city).is_some_and(|s| {
            s.influence.get(&id).copied().unwrap_or(0) * 100
                >= rules().presence_percent
                    * (rules().unaligned_weight + s.influence.values().sum::<i32>())
        })
    }
    pub(in crate::sim) fn declare_faith(
        &mut self,
        owner: usize,
        faith: Option<ReligionId>,
        policy: FaithPolicy,
    ) -> bool {
        if owner >= self.civilization_count
            || owner == BARBARIANS
            || self.game_over.is_some()
            || self.religion_cooldown(owner) > 0
            || faith.is_some_and(|id| !self.eligible_faiths(owner).contains(&id))
        {
            return false;
        }
        let old = self.national_faith(owner);
        if old.declared == faith && old.policy == policy {
            return false;
        }
        self.religion.nations.insert(
            owner,
            NationalFaith {
                declared: faith,
                policy,
                changed: Some(self.turn),
            },
        );
        self.religion_event("declared", owner, None, faith, Some(policy));
        if owner == HUMAN || self.contacted_pact(HUMAN, owner).is_some() {
            let subject = if owner == HUMAN {
                "You".to_owned()
            } else {
                self.players[owner].name.clone()
            };
            let label = faith.map_or("no declared faith", |id| self.religion_name(id));
            self.add_contextual_notification(
                format!("{subject}: {label}; {} policy.", policy.name()),
                NotificationCategory::Religion,
                if owner == HUMAN {
                    NotificationPriority::Important
                } else {
                    NotificationPriority::Routine
                },
                None,
                Some(owner),
            );
        }
        true
    }
    fn religion_event(
        &mut self,
        action: &'static str,
        owner: usize,
        city: Option<u32>,
        faith: Option<ReligionId>,
        policy: Option<FaithPolicy>,
    ) {
        let mut entities = vec![trace::Entity::Civilization(owner)];
        if let Some(id) = city {
            entities.push(trace::Entity::City(id));
        }
        self.trace.emit(
            self.turn,
            || entities,
            || ReligionEvent {
                action,
                owner,
                city,
                faith,
                policy,
            },
        );
    }
    pub(crate) fn religion_attitude(&self, owner: usize, other: usize) -> i32 {
        let a = self.national_faith(owner);
        let b = self.national_faith(other);
        match (a.declared, b.declared) {
            (Some(x), Some(y)) if x == y => rules().shared_attitude,
            (Some(_), Some(_)) => match a.policy {
                FaithPolicy::Tolerant => 0,
                FaithPolicy::Devout => rules().devout_difference,
                FaithPolicy::Missionary => rules().missionary_difference,
            },
            _ => 0,
        }
    }
    pub(in crate::sim) fn religion_culture(&self, city: &City) -> i32 {
        (if self.national_faith(city.owner).policy == FaithPolicy::Tolerant
            && self.city_dominant_faith(city.id).is_some()
        {
            rules().tolerant_culture
        } else {
            0
        }) + self.city_faith_definition(city).map_or(0, |f| f.culture)
    }
    pub(in crate::sim) fn religion_content(&self, city: &City) -> i32 {
        let n = self.national_faith(city.owner);
        (if n.policy == FaithPolicy::Devout
            && n.declared.is_some()
            && n.declared == self.city_dominant_faith(city.id)
        {
            rules().devout_content
        } else {
            0
        }) + self.city_faith_definition(city).map_or(0, |f| f.content)
    }
    fn city_faith_definition(&self, city: &City) -> Option<&'static FaithDefinition> {
        self.city_dominant_faith(city.id).and_then(definition)
    }
    pub(in crate::sim) fn religion_output(&self, city: &City, kind: &str) -> i32 {
        self.city_faith_definition(city).map_or(0, |f| match kind {
            "Science" => f.science,
            "Gold" => f.gold,
            _ => 0,
        })
    }
    pub(crate) fn city_dominant_faith(&self, city: u32) -> Option<ReligionId> {
        self.religion.cities.get(&city).and_then(|s| s.dominant)
    }
    pub(crate) fn religion_catalog(&self) -> &[Religion] {
        &self.religion.religions
    }
    #[cfg(feature = "desktop")]
    pub(crate) fn city_religion_summary(&self, city: u32) -> String {
        let Some(c) = self.city_by_id(city).filter(|c| c.owner == HUMAN) else {
            return "Religion: unavailable".into();
        };
        let Some(s) = self.religion.cities.get(&c.id) else {
            return "Religion: unaligned 100%".into();
        };
        let total = rules().unaligned_weight + s.influence.values().sum::<i32>();
        let mut used = 0;
        let shares: Vec<_> = s
            .influence
            .iter()
            .map(|(&id, &v)| {
                let pct = v * 100 / total;
                used += pct;
                format!(
                    "{} {}%{}",
                    self.religion_name(id),
                    pct,
                    if s.dominant == Some(id) {
                        " (dominant)"
                    } else {
                        ""
                    }
                )
            })
            .collect();
        format!("{} · Unaligned {}%", shares.join(" · "), 100 - used)
    }
    pub(in crate::sim) fn remove_city_religion(&mut self, city: u32) {
        self.religion.cities.remove(&city);
    }

    pub(in crate::sim) fn valid_religion(&self) -> bool {
        let state = &self.religion;
        let valid_id = |id| state.religions.iter().any(|r| r.id == id);
        let mut ids = BTreeSet::new();
        let mut founders = BTreeSet::new();
        let mut names = BTreeSet::new();
        if state.religions.len() > self.civilization_count
            || state.religions.iter().any(|f| {
                f.id == 0
                    || f.id > MAX_CIVILIZATIONS as u32
                    || !ids.insert(f.id)
                    || f.founder >= self.civilization_count
                    || f.founder == BARBARIANS
                    || !founders.insert(f.founder)
                    || definition(f.id)
                        .is_none_or(|d| d.name != f.name || f.emblem != (f.id - 1) as u8)
                    || f.name.trim() != f.name
                    || f.name.is_empty()
                    || f.name.chars().count() > 32
                    || f.name.chars().any(char::is_control)
                    || !names.insert(f.name.to_lowercase())
            })
        {
            return false;
        }
        state.cities.iter().all(|(id, s)| {
            self.city_by_id(*id).is_some()
                && s.influence
                    .iter()
                    .all(|(f, v)| valid_id(*f) && (1..=rules().maximum_influence).contains(v))
                && s.dominant.is_none_or(|f| {
                    valid_id(f)
                        && s.influence.get(&f).copied().unwrap_or(0) * 100
                            >= rules().retention_percent
                                * (rules().unaligned_weight + s.influence.values().sum::<i32>())
                })
                && s.announced.iter().all(|f| valid_id(*f))
        }) && state.nations.iter().all(|(owner, n)| {
            *owner < self.civilization_count
                && *owner != BARBARIANS
                && n.declared.is_none_or(valid_id)
                && n.changed.is_none_or(|t| t >= 0 && t <= self.turn)
        })
    }
}

#[cfg(feature = "desktop")]
pub(crate) struct ReligionManagementView {
    pub(crate) cities: Vec<(u32, String)>,
    pub(crate) founding: Vec<FoundingOption>,
    pub(crate) faiths: Vec<(ReligionId, String)>,
    pub(crate) current: NationalFaith,
    pub(crate) cooldown: i32,
}
#[cfg(feature = "desktop")]
impl Simulation {
    pub(crate) fn religion_management_view(&self) -> ReligionManagementView {
        let n = self.national_faith(HUMAN);
        let eligible = self.eligible_faiths(HUMAN);
        ReligionManagementView {
            cities: self
                .cities
                .iter()
                .filter(|c| c.owner == HUMAN)
                .map(|c| (c.id, c.name.clone()))
                .collect(),
            faiths: self
                .religion
                .religions
                .iter()
                .filter(|f| eligible.contains(&f.id) || n.declared == Some(f.id))
                .map(|f| (f.id, f.name.clone()))
                .collect(),
            founding: rules()
                .faiths
                .iter()
                .map(|f| FoundingOption {
                    id: f.id,
                    name: f.name.clone(),
                    requirement: f.tech_name().into(),
                    bonus: f.bonus_description(),
                    founded: self.religion.religions.iter().any(|r| r.id == f.id),
                    eligible: self
                        .cities
                        .iter()
                        .any(|c| self.can_found_religion(HUMAN, c.id, f.id)),
                })
                .collect(),
            current: n,
            cooldown: self.religion_cooldown(HUMAN),
        }
    }
    pub(crate) fn public_faith_summary(&self, owner: usize) -> String {
        if owner != HUMAN && self.contacted_pact(HUMAN, owner).is_none() {
            return String::new();
        }
        let n = self.national_faith(owner);
        let name = n
            .declared
            .map_or("No declared faith", |id| self.religion_name(id));
        format!(
            "{name} · {} · religion attitude {:+}",
            n.policy.name(),
            self.religion_attitude(owner, HUMAN)
        )
    }
}

#[derive(Clone, Serialize)]
pub(crate) struct ReligionSnapshot {
    religions: Vec<Religion>,
    nations: BTreeMap<usize, NationalFaith>,
    cities: Vec<ReligionCitySnapshot>,
}
#[derive(Clone, Serialize)]
struct ReligionCitySnapshot {
    city: u32,
    owner: usize,
    influence: BTreeMap<ReligionId, i32>,
    dominant: Option<ReligionId>,
    unaligned_weight: i32,
}
impl Simulation {
    pub(crate) fn observe_religion(&self) -> ReligionSnapshot {
        ReligionSnapshot {
            religions: self.religion_catalog().to_vec(),
            nations: (0..self.civilization_count)
                .map(|id| (id, self.national_faith(id)))
                .collect(),
            cities: self
                .cities
                .iter()
                .map(|c| {
                    let s = self.religion.cities.get(&c.id).cloned().unwrap_or_default();
                    ReligionCitySnapshot {
                        city: c.id,
                        owner: c.owner,
                        influence: s.influence,
                        dominant: s.dominant,
                        unaligned_weight: rules().unaligned_weight,
                    }
                })
                .collect(),
        }
    }
}
#[cfg(any(test, feature = "scenarios"))]
impl CityFaith {
    pub(in crate::sim) fn fixture(
        influence: BTreeMap<ReligionId, i32>,
        dominant: Option<ReligionId>,
    ) -> Self {
        Self {
            influence,
            dominant,
            ..Default::default()
        }
    }
}

#[cfg(feature = "desktop")]
pub(crate) struct FoundingOption {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) requirement: String,
    pub(crate) bonus: String,
    pub(crate) founded: bool,
    pub(crate) eligible: bool,
}

#[cfg(feature = "desktop")]
impl Simulation {
    pub(crate) fn religion_research_unlocks(tech: Tech) -> Vec<&'static str> {
        rules()
            .faiths
            .iter()
            .filter(|f| f.tech() == tech)
            .map(|f| f.name.as_str())
            .collect()
    }
}
