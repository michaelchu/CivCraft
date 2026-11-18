//! Bounded directional history of actual diplomatic cooperation and breaches.
use super::*;
use serde::{Deserialize, Serialize};

const MAX_SCORE: i32 = 100;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Policy {
    trade_gain: i32,
    support_gain: i32,
    breach_grievance: i32,
    rebuild_gain: i32,
    trade_cooldown_turns: i32,
    support_cooldown_turns: i32,
    decay_turns: i32,
    positive_decay: i32,
    grievance_decay: i32,
    caution_threshold: i32,
    attitude_divisor: i32,
    trade_score_divisor: i32,
    maximum_trade_adjustment_percent: i32,
    war_threshold_per_score: i32,
}
impl Policy {
    pub(super) fn validate(&self) {
        for value in [
            self.trade_gain,
            self.support_gain,
            self.breach_grievance,
            self.rebuild_gain,
            self.positive_decay,
            self.grievance_decay,
            self.caution_threshold,
        ] {
            assert!((1..=MAX_SCORE).contains(&value));
        }
        for value in [
            self.trade_cooldown_turns,
            self.support_cooldown_turns,
            self.decay_turns,
            self.attitude_divisor,
            self.trade_score_divisor,
        ] {
            assert!((1..=1000).contains(&value));
        }
        assert!((0..=20).contains(&self.maximum_trade_adjustment_percent));
        assert!((0..=100).contains(&self.war_threshold_per_score));
    }
}
fn policy() -> &'static Policy {
    super::trade_policy::relationship_policy()
}

/// Appended to campaign saves independently of the frozen native AI timers.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub(in crate::sim) struct RelationshipMemory {
    commerce: i32,
    loyalty: i32,
    grievance: i32,
    decay_turn: i32,
    last_trade: Option<i32>,
    last_support: Option<i32>,
    last_breach: Option<i32>,
}
impl RelationshipMemory {
    fn new(turn: i32) -> Self {
        Self {
            commerce: 0,
            loyalty: 0,
            grievance: 0,
            decay_turn: turn,
            last_trade: None,
            last_support: None,
            last_breach: None,
        }
    }
    fn at(mut self, turn: i32) -> Self {
        let rules = policy();
        let steps = turn.saturating_sub(self.decay_turn).max(0) / rules.decay_turns;
        self.commerce = self
            .commerce
            .saturating_sub(steps.saturating_mul(rules.positive_decay))
            .max(0);
        self.loyalty = self
            .loyalty
            .saturating_sub(steps.saturating_mul(rules.positive_decay))
            .max(0);
        self.grievance = self
            .grievance
            .saturating_sub(steps.saturating_mul(rules.grievance_decay))
            .max(0);
        self.decay_turn = self
            .decay_turn
            .saturating_add(steps.saturating_mul(rules.decay_turns));
        self
    }
    fn valid(&self, turn: i32) -> bool {
        [self.commerce, self.loyalty, self.grievance]
            .iter()
            .all(|score| (0..=MAX_SCORE).contains(score))
            && (0..=turn).contains(&self.decay_turn)
            && [self.last_trade, self.last_support, self.last_breach]
                .iter()
                .all(|stamp| stamp.is_none_or(|stamp| (0..=turn).contains(&stamp)))
    }
}

#[derive(Clone, Serialize)]
pub(crate) struct RelationshipSnapshot {
    commerce: i32,
    loyalty: i32,
    grievance: i32,
    last_trade: Option<i32>,
    last_support: Option<i32>,
    last_breach: Option<i32>,
    attitude_bonus: i32,
    trade_adjustment_percent: i32,
}
#[derive(Serialize)]
struct MemoryEvent {
    observer: usize,
    other: usize,
    reason: &'static str,
    memory: RelationshipSnapshot,
}
impl trace::Payload for MemoryEvent {
    const KIND: &'static str = "diplomacy.relationship_memory";
}

impl Simulation {
    fn relationship_memory(&self, owner: usize, other: usize) -> RelationshipMemory {
        self.ai_relationship_memory
            .get(&(owner, other))
            .copied()
            .unwrap_or_else(|| RelationshipMemory::new(self.turn))
            .at(self.turn)
    }
    pub(super) fn observe_relationship(&self, owner: usize, other: usize) -> RelationshipSnapshot {
        let memory = self.relationship_memory(owner, other);
        RelationshipSnapshot {
            commerce: memory.commerce,
            loyalty: memory.loyalty,
            grievance: memory.grievance,
            last_trade: memory.last_trade,
            last_support: memory.last_support,
            last_breach: memory.last_breach,
            attitude_bonus: self.relationship_attitude_bonus(owner, other),
            trade_adjustment_percent: self.relationship_trade_adjustment(owner, other),
        }
    }
    pub(in crate::sim) fn valid_relationship_memory(&self) -> bool {
        self.ai_relationship_memory
            .iter()
            .all(|(&(owner, other), memory)| {
                owner < self.players.len()
                    && other < self.players.len()
                    && owner != other
                    && owner != BARBARIANS
                    && other != BARBARIANS
                    && memory.valid(self.turn)
                    && self.contacted_pact(owner, other).is_some()
            })
    }
    pub(super) fn relationship_attitude_bonus(&self, owner: usize, other: usize) -> i32 {
        let memory = self.relationship_memory(owner, other);
        (memory.commerce + memory.loyalty - 2 * memory.grievance) / policy().attitude_divisor
    }
    pub(super) fn relationship_trade_adjustment(&self, owner: usize, other: usize) -> i32 {
        if !self.players[owner].ai {
            return 0;
        }
        let memory = self.relationship_memory(owner, other);
        ((memory.commerce + memory.loyalty - 2 * memory.grievance) / policy().trade_score_divisor)
            .clamp(
                -policy().maximum_trade_adjustment_percent,
                policy().maximum_trade_adjustment_percent,
            )
    }
    pub(super) fn relationship_war_adjustment(&self, owner: usize, other: usize) -> i32 {
        let memory = self.relationship_memory(owner, other);
        (memory.grievance - memory.commerce - memory.loyalty) * policy().war_threshold_per_score
    }
    pub(super) fn relationship_caution(&self, owner: usize, other: usize) -> bool {
        self.players[owner].ai
            && self.relationship_memory(owner, other).grievance >= policy().caution_threshold
    }
    fn remember_relationship(&mut self, observer: usize, other: usize, reason: &'static str) {
        if observer == other
            || observer >= self.players.len()
            || other >= self.players.len()
            || !self.player_alive(observer)
            || !self.player_alive(other)
            || self.player_has_no_diplomacy(observer)
            || self.player_has_no_diplomacy(other)
            || self.contacted_pact(observer, other).is_none()
        {
            return;
        }
        let mut memory = self.relationship_memory(observer, other);
        let rules = policy();
        match reason {
            "completed_trade" => {
                if memory
                    .last_trade
                    .is_some_and(|last| self.turn.saturating_sub(last) < rules.trade_cooldown_turns)
                {
                    return;
                }
                memory.commerce = (memory.commerce + rules.trade_gain).min(MAX_SCORE);
                memory.grievance = (memory.grievance - rules.rebuild_gain).max(0);
                memory.last_trade = Some(self.turn);
            }
            "ally_supported" => {
                if memory.last_support.is_some_and(|last| {
                    self.turn.saturating_sub(last) < rules.support_cooldown_turns
                }) {
                    return;
                }
                memory.loyalty = (memory.loyalty + rules.support_gain).min(MAX_SCORE);
                memory.last_support = Some(self.turn);
            }
            "pact_breached" => {
                // Alliance -> armistice -> war is one cancellation incident.
                if memory.last_breach == Some(self.turn) {
                    return;
                }
                memory.grievance = (memory.grievance + rules.breach_grievance).min(MAX_SCORE);
                memory.last_breach = Some(self.turn);
            }
            _ => unreachable!("unknown relationship incident"),
        }
        self.ai_relationship_memory
            .insert((observer, other), memory);
        let snapshot = self.observe_relationship(observer, other);
        self.trace.emit(
            self.turn,
            || {
                vec![
                    trace::Entity::Civilization(observer),
                    trace::Entity::Civilization(other),
                ]
            },
            || MemoryEvent {
                observer,
                other,
                reason,
                memory: snapshot,
            },
        );
    }
    pub(super) fn remember_completed_trade(&mut self, a: usize, b: usize, offer: &TreatyOffer) {
        // Completed technology purchases/swaps are genuine mutual exchanges.
        // Gold shuffling, gifts, empty or repeated access clauses cannot farm credit.
        let key = Self::relation_key(a, b);
        let delivered = [key.1, key.0].map(|receiver| {
            let side = usize::from(receiver == key.0);
            offer.terms[side]
                .technologies()
                .any(|tech| self.has_tech(receiver, tech))
        });
        if delivered.iter().any(|&received| received)
            && offer
                .terms
                .iter()
                .zip(delivered)
                .all(|(terms, received)| terms.gold > 0 || received)
        {
            self.remember_relationship(a, b, "completed_trade");
            self.remember_relationship(b, a, "completed_trade");
        }
    }
    pub(super) fn benefits_from_ally_support(
        &self,
        observer: usize,
        helper: usize,
        enemy: usize,
    ) -> bool {
        observer != helper
            && observer != enemy
            && self.player_alive(observer)
            && self.allied(observer, helper)
            && self.diplomatic_at_war(observer, enemy)
    }
    fn supporting_ally_against(&self, owner: usize, other: usize) -> bool {
        (0..self.players.len()).any(|ally| {
            ally != owner
                && ally != other
                && self.player_alive(ally)
                && self.allied(owner, ally)
                && self.diplomatic_at_war(ally, other)
        })
    }
    pub(super) fn relationship_breach(
        &self,
        owner: usize,
        other: usize,
        pact: Pact,
        forced: bool,
    ) -> bool {
        if forced || self.supporting_ally_against(owner, other) {
            return false;
        }
        let relation = &self.diplomacy[&Self::relation_key(owner, other)];
        if relation.cause_until[usize::from(owner > other)] > self.turn {
            return false;
        }
        pact == Pact::Alliance
            || (matches!(pact, Pact::Ceasefire | Pact::Armistice) && relation.until > self.turn)
            || (pact == Pact::Peace && {
                let history = self.relationship_memory(other, owner);
                history.commerce > 0 || history.loyalty > 0
            })
    }
    pub(super) fn remember_pact_breach(&mut self, owner: usize, other: usize) {
        self.remember_relationship(other, owner, "pact_breached");
    }
    pub(super) fn remember_ally_support(&mut self, owner: usize, enemy: usize) {
        let allies: Vec<_> = (0..self.players.len())
            .filter(|&ally| {
                ally != owner
                    && ally != enemy
                    && self.player_alive(ally)
                    && self.allied(owner, ally)
                    && self.diplomatic_at_war(ally, enemy)
            })
            .collect();
        for ally in allies {
            self.remember_relationship(ally, owner, "ally_supported");
        }
    }
}

#[cfg(any(test, feature = "scenarios"))]
mod scenarios;
#[cfg(test)]
mod tests;
