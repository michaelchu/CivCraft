//! Passive, run-local research observations. Technology names are catalogued once in the report.
use super::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TechnologySource {
    Research,
    Trade,
    Espionage,
    Conquest,
    Village,
    FreeAdvance,
    FirstDiscoveryBonus,
    CivilWar,
    Upkeep,
    TransferLoss,
    #[cfg(test)]
    Unspecified,
    Unobserved,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TechnologyAction {
    Acquired,
    Lost,
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct TechnologyChange {
    pub turn: i32,
    pub phase: trace::Phase,
    pub owner: usize,
    pub technology_id: usize,
    pub action: TechnologyAction,
    pub source: TechnologySource,
    pub other_owner: Option<usize>,
}
impl trace::Payload for TechnologyChange {
    const KIND: &'static str = "research.technology_changed";
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ScienceOutput {
    pub turn: i32,
    pub gross: i32,
    pub upkeep: i32,
    pub net: i32,
}
#[derive(Clone, Default, Serialize)]
pub(crate) struct ResearchTotals {
    pub initial_known_count: usize,
    pub initial_future_techs: i32,
    pub acquired: u64,
    pub lost: u64,
    pub acquired_by_source: BTreeMap<TechnologySource, u64>,
    pub lost_by_source: BTreeMap<TechnologySource, u64>,
}
#[derive(Serialize)]
pub(crate) struct CivilizationResearch {
    pub owner: usize,
    pub name: String,
    pub alive: bool,
    pub known_technology_ids: Vec<usize>,
    pub known_count: usize,
    pub future_techs: i32,
    pub current_technology_id: Option<usize>,
    pub goal_technology_id: Option<usize>,
    pub progress: i32,
    pub cost: Option<i32>,
    pub science_rate_percent: i32,
    /// Last actual accounting phase, not a forecast recomputed after discoveries.
    pub science_output: Option<ScienceOutput>,
    pub totals: Option<ResearchTotals>,
}
#[derive(Serialize)]
pub(crate) struct ResearchSnapshot {
    pub history_available: bool,
    pub since_turn: Option<i32>,
    pub complete_since_start: bool,
    pub unobserved_changes: u64,
    pub civilizations: Vec<CivilizationResearch>,
    pub changes: Vec<TechnologyChange>,
}
pub(super) type Knowledge = BTreeMap<usize, BTreeMap<usize, i32>>;
pub(super) struct ResearchLedger {
    pub since_turn: i32,
    pub known: Knowledge,
    pub changes: Vec<TechnologyChange>,
    pub unobserved: u64,
    pub output: BTreeMap<usize, ScienceOutput>,
    pub totals: BTreeMap<usize, ResearchTotals>,
}
impl ResearchLedger {
    pub fn new(turn: i32, known: Knowledge) -> Self {
        let totals = known
            .iter()
            .map(|(&owner, techs)| {
                (
                    owner,
                    ResearchTotals {
                        initial_known_count: techs
                            .keys()
                            .filter(|&&id| id != Tech::FUTURE.0)
                            .count(),
                        initial_future_techs: techs.get(&Tech::FUTURE.0).copied().unwrap_or(0),
                        ..Default::default()
                    },
                )
            })
            .collect();
        Self {
            totals,
            since_turn: turn,
            known,
            changes: vec![],
            unobserved: 0,
            output: BTreeMap::new(),
        }
    }
    pub fn record(&mut self, change: TechnologyChange) {
        let totals = self.totals.entry(change.owner).or_default();
        if change.action == TechnologyAction::Acquired {
            totals.acquired += 1;
            *totals.acquired_by_source.entry(change.source).or_default() += 1;
        } else {
            totals.lost += 1;
            *totals.lost_by_source.entry(change.source).or_default() += 1;
        }
        let n = self
            .known
            .entry(change.owner)
            .or_default()
            .entry(change.technology_id)
            .or_default();
        *n += if change.action == TechnologyAction::Acquired {
            1
        } else {
            -1
        };
        if change.source == TechnologySource::Unobserved {
            self.unobserved += 1;
        }
        self.changes.push(change);
    }
}
impl trace::Collector {
    pub(super) fn technology_changed(
        &mut self,
        turn: i32,
        owner: usize,
        tech: Tech,
        action: TechnologyAction,
        source: TechnologySource,
        other_owner: Option<usize>,
    ) {
        if self.research.is_none() && !self.wants::<TechnologyChange>() {
            return;
        }
        let change = TechnologyChange {
            turn,
            phase: self.current_phase(),
            owner,
            technology_id: tech.0,
            action,
            source,
            other_owner,
        };
        if let Some(ledger) = &mut self.research {
            ledger.record(change.clone());
        }
        self.emit(
            turn,
            || {
                let mut e = vec![trace::Entity::Civilization(owner)];
                if let Some(other) = other_owner {
                    e.push(trace::Entity::Civilization(other));
                }
                e
            },
            || change,
        );
    }
    pub(super) fn research_output(&mut self, owner: usize, turn: i32, gross: i32, upkeep: i32) {
        if let Some(ledger) = &mut self.research {
            ledger.output.insert(
                owner,
                ScienceOutput {
                    turn,
                    gross,
                    upkeep,
                    net: gross - upkeep,
                },
            );
        }
    }
    pub(crate) fn clear_research_changes(&mut self) {
        if let Some(ledger) = &mut self.research {
            ledger.changes.clear();
        }
    }
}
impl Simulation {
    fn research_knowledge(&self) -> Knowledge {
        self.players
            .iter()
            .enumerate()
            .map(|(owner, p)| {
                let mut known: BTreeMap<_, _> = p
                    .known
                    .iter()
                    .filter(|t| t.is_researchable_advance())
                    .map(|t| (t.0, 1))
                    .collect();
                if p.future_techs > 0 {
                    known.insert(Tech::FUTURE.0, p.future_techs);
                }
                (owner, known)
            })
            .collect()
    }
    #[cfg(any(test, feature = "scenarios"))]
    pub(crate) fn ensure_research_observation(&mut self) {
        if self.trace.research.is_none() {
            self.start_research_observation();
        }
    }

    pub(crate) fn start_research_observation(&mut self) {
        self.trace.research = Some(ResearchLedger::new(self.turn, self.research_knowledge()));
    }
    pub(crate) fn reconcile_research_observation(&mut self) {
        let Some(ledger) = &self.trace.research else {
            return;
        };
        let current = self.research_knowledge();
        let mut changes = vec![];
        let owners: BTreeSet<_> = current.keys().chain(ledger.known.keys()).copied().collect();
        for owner in owners {
            let old = ledger.known.get(&owner).cloned().unwrap_or_default();
            let new = current.get(&owner).cloned().unwrap_or_default();
            let ids: BTreeSet<_> = old.keys().chain(new.keys()).copied().collect();
            for id in ids {
                let delta = new.get(&id).copied().unwrap_or(0) - old.get(&id).copied().unwrap_or(0);
                for _ in 0..delta.abs() {
                    changes.push((
                        owner,
                        Tech(id),
                        if delta > 0 {
                            TechnologyAction::Acquired
                        } else {
                            TechnologyAction::Lost
                        },
                    ));
                }
            }
        }
        for (owner, tech, action) in changes {
            self.trace.technology_changed(
                self.turn,
                owner,
                tech,
                action,
                TechnologySource::Unobserved,
                None,
            );
        }
    }
    pub(crate) fn observe_research(&self) -> ResearchSnapshot {
        let ledger = self.trace.research.as_ref();
        let valid = |t: Tech| (t.is_researchable_advance() || t == Tech::FUTURE).then_some(t.0);
        ResearchSnapshot {
            history_available: ledger.is_some(),
            since_turn: ledger.map(|l| l.since_turn),
            complete_since_start: ledger.is_some_and(|l| l.unobserved == 0),
            unobserved_changes: ledger.map_or(0, |l| l.unobserved),
            changes: ledger.map(|l| l.changes.clone()).unwrap_or_default(),
            civilizations: self
                .players
                .iter()
                .enumerate()
                .filter(|(owner, _)| {
                    *owner != BARBARIANS
                        && (*owner < self.civilization_count
                            || *owner > BARBARIANS
                            || self.player_alive(*owner))
                })
                .map(|(owner, p)| {
                    let known_technology_ids: Vec<_> = p
                        .known
                        .iter()
                        .filter(|t| t.is_researchable_advance())
                        .map(|t| t.0)
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect();
                    CivilizationResearch {
                        owner,
                        name: p.name.clone(),
                        alive: self.player_alive(owner),
                        known_count: known_technology_ids.len(),
                        known_technology_ids,
                        future_techs: p.future_techs,
                        current_technology_id: valid(p.researching),
                        goal_technology_id: valid(p.research_goal),
                        progress: p.research,
                        cost: valid(p.researching).map(|_| self.target_cost(owner, p.researching)),
                        science_rate_percent: p.rates[0],
                        science_output: ledger.and_then(|l| l.output.get(&owner).cloned()),
                        totals: ledger.map(|l| l.totals.get(&owner).cloned().unwrap_or_default()),
                    }
                })
                .collect(),
        }
    }
    pub(crate) fn technology_catalog() -> serde_json::Value {
        serde_json::to_value(Tech::all().filter(|t|t.is_researchable_advance()).chain(std::iter::once(Tech::FUTURE)).map(|t|serde_json::json!({"id":t.0,"name":t.name(),"era":if t==Tech::FUTURE {None} else {Some(t.era().name())},"prerequisites":t.prerequisites().iter().map(|t|t.0).collect::<Vec<_>>()})).collect::<Vec<_>>()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> Simulation {
        let mut g = Simulation::new(crate::test_support::seeded_map());
        for p in &mut g.players {
            p.known.clear();
            p.future_techs = 0;
            p.research = 0;
        }
        g
    }
    #[test]
    fn research_trade_and_same_interval_loss_survive_disabled_or_overflowed_events() {
        let mut g = game();
        g.players[0].researching = Tech::POTTERY;
        g.trace = trace::Collector::enabled(BTreeSet::from(["*".into()]), 1);
        g.start_research_observation();
        let cost = g.target_cost(0, Tech::POTTERY);
        g.resolve_research_turn(0, cost);
        assert!(g.transfer_treaty_technology(0, 1, Tech::POTTERY));
        // Force the normal upkeep-loss path after both acquisitions in this interval.
        g.players[0].researching = Tech::BRONZE_WORKING;
        g.players[0].research = 0;
        g.apply_research_upkeep(0, -1);
        g.reconcile_research_observation();
        let s = g.observe_research();
        assert!(s.complete_since_start);
        for (owner, action, source) in [
            (0, TechnologyAction::Acquired, TechnologySource::Research),
            (1, TechnologyAction::Acquired, TechnologySource::Trade),
            (0, TechnologyAction::Lost, TechnologySource::Upkeep),
        ] {
            assert!(
                s.changes.iter().any(|c| c.owner == owner
                    && c.technology_id == Tech::POTTERY.0
                    && c.action == action
                    && c.source == source),
                "{}",
                serde_json::to_string(&s).unwrap()
            );
        }
        assert!(g.trace.dropped() > 0);
        assert!(s.civilizations[0].known_technology_ids.is_empty());
        assert!(
            s.civilizations[1]
                .known_technology_ids
                .contains(&Tech::POTTERY.0)
        );
        let science = s.civilizations[0].science_output.as_ref().unwrap();
        assert_eq!(science.net, science.gross - science.upkeep);
        assert_eq!(science.net, g.players[0].last_science);
        let before = g.encode_campaign().unwrap();
        let _ = g.observe_research();
        assert_eq!(before, g.encode_campaign().unwrap());
        g.trace.clear_research_changes();
        assert!(g.observe_research().changes.is_empty());
        assert!(!g.clone().observe_research().history_available);
    }
    #[test]
    fn baseline_future_completions_and_unknown_edits_are_honest() {
        let mut g = game();
        g.players[0].known.push(Tech::ALPHABET);
        g.start_research_observation();
        assert!(g.observe_research().changes.is_empty());
        assert_eq!(g.observe_research().civilizations[0].known_count, 1);
        g.discover_with_source(0, Tech::FUTURE, TechnologySource::Research, None);
        g.discover_with_source(0, Tech::FUTURE, TechnologySource::FreeAdvance, None);
        g.reconcile_research_observation();
        assert!(g.observe_research().complete_since_start);
        assert_eq!(g.observe_research().civilizations[0].future_techs, 2);
        assert_eq!(g.observe_research().changes.len(), 2);
        g.players[0].known.clear();
        g.reconcile_research_observation();
        let s = g.observe_research();
        assert!(!s.complete_since_start);
        assert_eq!(s.unobserved_changes, 1);
        assert!(s.changes.iter().any(
            |c| c.source == TechnologySource::Unobserved && c.action == TechnologyAction::Lost
        ));
        assert!(g.trace.drain().is_empty());
    }
}
