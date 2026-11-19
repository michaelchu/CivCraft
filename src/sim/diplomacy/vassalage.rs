//! Owner-authorized Civ IV-style subordination, independent of symmetric pacts.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod negotiation;
mod persistence;
mod policy;
use super::transitions::PactChange;
pub(in crate::sim) use persistence::SavedVassalage;
use policy::policy;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum VassalKind {
    Voluntary,
    Capitulation,
}
impl VassalKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Voluntary => "Voluntary vassalage",
            Self::Capitulation => "Capitulation",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct VassalTerms {
    pub(crate) master: usize,
    pub(crate) kind: VassalKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct VassalAgreement {
    pub(crate) master: usize,
    pub(crate) kind: VassalKind,
    pub(crate) started: i32,
    pub(crate) starting_land: usize,
    pub(crate) independence_eligible: bool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub(in crate::sim) struct VassalState {
    pub(in crate::sim) agreements: BTreeMap<usize, VassalAgreement>,
    offer_after: BTreeMap<(usize, usize), i32>,
}

impl Simulation {
    pub(crate) fn vassal_master(&self, owner: usize) -> Option<usize> {
        self.vassalage.agreements.get(&owner).map(|a| a.master)
    }

    pub(crate) fn same_vassal_group(&self, a: usize, b: usize) -> bool {
        a != b && self.vassal_master(a).unwrap_or(a) == self.vassal_master(b).unwrap_or(b)
    }

    /// Access deliberately does not confer alliance scoring or transport rights.
    pub(crate) fn military_access(&self, a: usize, b: usize) -> bool {
        self.allied(a, b) || self.same_vassal_group(a, b)
    }

    fn vassal_land(&self, owner: usize) -> usize {
        self.tiles
            .iter()
            .filter(|t| t.owner == Some(owner) && !t.terrain.is_water())
            .count()
    }

    fn vassal_population(&self, owner: usize) -> i64 {
        self.cities
            .iter()
            .filter(|c| c.owner == owner)
            .map(|c| i64::from(c.size))
            .sum()
    }

    pub(crate) fn vassal_independence_reason(&self, vassal: usize) -> Option<&'static str> {
        let a = self.vassalage.agreements.get(&vassal)?;
        if !self.player_alive(a.master) {
            return Some("master_eliminated");
        }
        let p = policy();
        if a.kind == VassalKind::Voluntary {
            return (self.turn - a.started >= p.voluntary_minimum_turns)
                .then_some("voluntary_term_completed");
        }
        let land = self.vassal_land(vassal) as i64;
        if land * 100 < a.starting_land as i64 * p.retained_land_percent {
            return Some("protection_failed");
        }
        (land * 100 > self.vassal_land(a.master) as i64 * p.independence_size_percent
            && self.vassal_population(vassal) * 100
                > self.vassal_population(a.master) * p.independence_size_percent)
            .then_some("relative_size")
    }

    pub(crate) fn vassal_offer_view(&self, other: usize) -> Result<TreatyOffer, String> {
        let terms = VassalTerms {
            master: HUMAN,
            kind: if self.at_war(HUMAN, other) {
                VassalKind::Capitulation
            } else {
                VassalKind::Voluntary
            },
        };
        self.plan_vassalage(HUMAN, other, terms)
            .map_err(str::to_owned)?;
        if !self.ai_accepts_vassalage(HUMAN, other, terms) {
            return Err("They are not willing to surrender their independence.".to_owned());
        }
        Ok(TreatyOffer {
            vassalage: Some(terms),
            ..TreatyOffer::default()
        })
    }

    pub(crate) fn vassal_offer_label(&self, terms: VassalTerms) -> String {
        format!(
            "{} to {}",
            terms.kind.name(),
            self.players
                .get(terms.master)
                .map_or("Unknown", |p| p.name.as_str())
        )
    }

    pub(crate) fn vassal_war_preview(&self, a: usize, b: usize, terms: VassalTerms) -> String {
        match self.plan_vassalage(a, b, terms) {
            Err(reason) => reason.to_owned(),
            Ok(changes) => {
                let mut names = Vec::new();
                for PactChange {
                    first,
                    second,
                    pact,
                    ..
                } in changes
                {
                    if pact == Pact::War {
                        let third = if [a, b].contains(&first) {
                            second
                        } else {
                            first
                        };
                        if ![a, b].contains(&third) {
                            names.push(self.players[third].name.clone());
                        }
                    }
                }
                names.sort();
                names.dedup();
                if names.is_empty() {
                    "The master controls war and peace.".to_owned()
                } else {
                    format!("Wars inherited: {}", names.join(", "))
                }
            }
        }
    }

    pub(crate) fn vassal_status(&self, owner: usize) -> Option<String> {
        let a = self.vassalage.agreements.get(&owner)?;
        let eligibility = match self.vassal_independence_reason(owner) {
            Some(_) => "May seek independence",
            None => "Bound by agreement",
        };
        Some(format!(
            "Vassal of {} · {} · {eligibility}",
            self.players[a.master].name,
            a.kind.name()
        ))
    }

    /// Computes the entire settlement before changing any relationship.
    pub(super) fn plan_vassalage(
        &self,
        a: usize,
        b: usize,
        terms: VassalTerms,
    ) -> Result<super::transitions::PactChanges, &'static str> {
        if a == b
            || a >= self.civilization_count
            || b >= self.civilization_count
            || ![a, b].contains(&terms.master)
            || [a, b].contains(&BARBARIANS)
        {
            return Err("Invalid participants.");
        }
        let vassal = if terms.master == a { b } else { a };
        if !self.players[vassal].ai
            || !self.player_alive(a)
            || !self.player_alive(b)
            || self.player_has_no_diplomacy(a)
            || self.player_has_no_diplomacy(b)
            || self.vassal_master(a).is_some()
            || self.vassal_master(b).is_some()
            || self
                .vassalage
                .agreements
                .values()
                .any(|r| r.master == vassal)
        {
            return Err("Only an independent AI civilization without vassals can submit.");
        }
        if self.contacted_pact(a, b).is_none() {
            return Err("We have not made contact.");
        }
        if !Tech::all().any(|t| {
            t.name() == policy().required_technology && (self.has_tech(a, t) || self.has_tech(b, t))
        }) {
            return Err("Vassalage requires Feudalism.");
        }
        if self.at_war(a, b) != (terms.kind == VassalKind::Capitulation) {
            return Err("Capitulation requires war; voluntary submission requires peace.");
        }
        let mut group = self.diplomatic_group(terms.master);
        group.push(vassal);
        let mut changes = Vec::new();
        for third in 0..self.civilization_count {
            if group.contains(&third) || !self.player_alive(third) {
                continue;
            }
            let inherited_war = terms.kind == VassalKind::Voluntary
                && self.contacted_pact(vassal, third) == Some(Pact::War);
            let desired = if inherited_war {
                Pact::War
            } else {
                self.contacted_pact(terms.master, third)
                    .unwrap_or(Pact::Peace)
            };
            if inherited_war
                && self.contacted_pact(terms.master, third) != Some(Pact::War)
                && (self.allied(terms.master, third)
                    || self.senate_blocks_cancellation(terms.master, third))
            {
                return Err("The master cannot accept the inherited war.");
            }
            // A settlement cannot silently break an external ally's protection.
            if desired == Pact::War && self.allied(vassal, third) {
                return Err("Submission conflicts with an existing alliance.");
            }
            let until = if matches!(desired, Pact::Ceasefire | Pact::Armistice) {
                self.diplomacy
                    .get(&Self::relation_key(terms.master, third))
                    .map_or(0, |r| r.until)
            } else {
                0
            };
            for &member in &group {
                if self.contacted_pact(member, third) != Some(desired)
                    || self
                        .diplomacy
                        .get(&Self::relation_key(member, third))
                        .is_some_and(|r| r.until != until)
                {
                    let (first, second) = Self::relation_key(member, third);
                    changes.push(PactChange {
                        first,
                        second,
                        pact: desired,
                        until,
                    });
                }
            }
        }
        for (i, &first) in group.iter().enumerate() {
            for &second in &group[..i] {
                let (first, second) = Self::relation_key(first, second);
                changes.push(PactChange {
                    first,
                    second,
                    pact: Pact::Peace,
                    until: 0,
                });
            }
        }
        changes.sort_by_key(|c| (c.first, c.second));
        Ok(changes)
    }

    pub(super) fn establish_vassalage(
        &mut self,
        a: usize,
        b: usize,
        terms: VassalTerms,
        changes: super::transitions::PactChanges,
    ) {
        let vassal = if terms.master == a { b } else { a };
        self.vassalage.agreements.insert(
            vassal,
            VassalAgreement {
                master: terms.master,
                kind: terms.kind,
                started: self.turn,
                starting_land: self.vassal_land(vassal),
                independence_eligible: false,
            },
        );
        self.commit_pact_changes(changes);
        let group = self.diplomatic_group(terms.master);
        self.ai_war_plans.retain(|&(owner, other), _| {
            owner != vassal && !(group.contains(&owner) && group.contains(&other))
        });
        self.ai_pending_war_targets.remove(&vassal);
        self.trace_diplomacy(
            terms.master,
            vassal,
            "vassalage_started",
            terms.kind.name(),
            None,
            None,
        );
        self.add_event(format!(
            "{} became a vassal of {} ({}).",
            self.players[vassal].name,
            self.players[terms.master].name,
            terms.kind.name()
        ));
        self.refresh_visibility();
    }

    pub(in crate::sim) fn release_vassal(&mut self, master: usize, vassal: usize) -> bool {
        if self.vassal_master(vassal) != Some(master) {
            return false;
        }
        self.end_vassalage(vassal, "released_by_master");
        true
    }

    pub(super) fn end_vassalage(&mut self, vassal: usize, reason: &'static str) {
        let old_group = self.diplomatic_group(vassal);
        let Some(a) = self.vassalage.agreements.remove(&vassal) else {
            return;
        };
        self.vassalage.offer_after.insert(
            Self::relation_key(a.master, vassal),
            self.turn + policy().negotiation_cooldown_turns,
        );
        self.trace_diplomacy(vassal, a.master, "vassalage_ended", reason, None, None);
        self.add_event(format!(
            "{} is independent of {}.",
            self.players[vassal].name, self.players[a.master].name
        ));
        // Existing wars and remembered knowledge survive independence.
        for other in old_group {
            if other != vassal && self.player_alive(vassal) && self.player_alive(other) {
                // Use the existing withdrawal period before ordinary border rules resume.
                self.set_pact_state(vassal, other, Pact::Armistice, self.turn + 16);
                self.resolve_split_stacks(vassal, other);
            }
        }
        self.refresh_visibility();
    }

    pub(in crate::sim) fn advance_vassalage(&mut self) {
        let eligibility: Vec<_> = self
            .vassalage
            .agreements
            .iter()
            .filter_map(|(&v, a)| {
                let reason = self.vassal_independence_reason(v);
                (reason.is_some() != a.independence_eligible).then_some((v, a.master, reason))
            })
            .collect();
        for (v, master, reason) in eligibility {
            self.vassalage
                .agreements
                .get_mut(&v)
                .expect("agreement exists")
                .independence_eligible = reason.is_some();
            self.trace_diplomacy(
                v,
                master,
                "vassalage_eligibility",
                reason.unwrap_or("bound_by_agreement"),
                None,
                None,
            );
        }
        let ended: Vec<_> = self
            .vassalage
            .agreements
            .iter()
            .filter_map(|(&v, a)| {
                if !self.player_alive(v) {
                    Some((v, "vassal_eliminated"))
                } else if !self.player_alive(a.master) {
                    Some((v, "master_eliminated"))
                } else if let Some(reason) = self.vassal_independence_reason(v) {
                    // Eligibility is distinct from the AI's decision to leave.
                    let can_survive = self.diplomatic_military_force(v, v)
                        >= self.diplomatic_military_force(v, a.master).max(1)
                        && !(0..self.civilization_count).any(|enemy| {
                            self.contacted_pact(v, enemy) == Some(Pact::War)
                                && self.diplomatic_peace_value(v, enemy)
                                    >= policy().minimum_peace_value
                        });
                    (reason == "protection_failed"
                        || (self.diplomatic_love(v, a.master) < 0 && can_survive))
                        .then_some((v, reason))
                } else {
                    None
                }
            })
            .collect();
        for (v, reason) in ended {
            self.end_vassalage(v, reason);
        }
    }

    pub(in crate::sim) fn valid_vassalage(&self) -> bool {
        self.vassalage.agreements.iter().all(|(&v, a)| {
            v < self.civilization_count
                && v < self.players.len()
                && a.master < self.civilization_count
                && a.master < self.players.len()
                && v != a.master
                && v != BARBARIANS
                && a.master != BARBARIANS
                && self.players[v].ai
                && self.vassal_master(a.master).is_none()
                && (1..=self.turn).contains(&a.started)
                && a.starting_land <= self.tiles.len()
                && self.contacted_pact(v, a.master) == Some(Pact::Peace)
                && self.diplomacy[&Self::relation_key(v, a.master)].until == 0
                && (0..self.civilization_count).all(|third| {
                    third == v
                        || third == a.master
                        || !self.player_alive(third)
                        || if self.same_vassal_group(v, third) {
                            self.contacted_pact(v, third) == Some(Pact::Peace)
                                && self.diplomacy[&Self::relation_key(v, third)].until == 0
                        } else {
                            self.contacted_pact(v, third) == self.contacted_pact(a.master, third)
                                && self
                                    .diplomacy
                                    .get(&Self::relation_key(v, third))
                                    .map(|r| r.until)
                                    == self
                                        .diplomacy
                                        .get(&Self::relation_key(a.master, third))
                                        .map(|r| r.until)
                        }
                })
        }) && self
            .vassalage
            .offer_after
            .iter()
            .all(|(&(a, b), &t)| a < b && b < self.civilization_count && t >= 0)
    }
}

#[cfg(test)]
mod tests;
