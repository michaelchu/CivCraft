//! Read-only relationship state and structured diplomatic transitions.
use super::*;
use serde::Serialize;

#[derive(Clone, Serialize)]
pub(crate) struct DiplomacySnapshot {
    /// All directional arrays follow this player order.
    pub players: [usize; 2],
    pub names: [String; 2],
    pub alive: [bool; 2],
    pub first_contact: Option<i32>,
    /// None means no contact, distinct from explicit War.
    pub pact: Option<Pact>,
    pub until: i32,
    pub trust: [i32; 2],
    pub shared_vision: [bool; 2],
    pub embassy: [bool; 2],
    pub offer: Option<TreatyOffer>,
    pub war_plans: [Option<WarPlanSnapshot>; 2],
}
#[derive(Clone, Serialize)]
pub(crate) struct WarPlanSnapshot {
    pub countdown: i32,
    pub reason: String,
}
impl From<WarPlan> for WarPlanSnapshot {
    fn from(plan: WarPlan) -> Self {
        Self {
            countdown: plan.countdown,
            reason: plan.reason.observation_name().to_owned(),
        }
    }
}
#[derive(Serialize)]
pub(crate) struct DiplomacyEvent {
    pub actor: usize,
    pub other: usize,
    pub action: &'static str,
    pub reason: &'static str,
    pub previous: Option<Pact>,
    pub current: Option<Pact>,
    pub offer: Option<TreatyOffer>,
}
impl trace::Payload for DiplomacyEvent {
    const KIND: &'static str = "diplomacy.transition";
}
impl Simulation {
    pub(crate) fn observe_diplomacy(&self) -> Vec<DiplomacySnapshot> {
        let mut pairs: Vec<_> = self
            .diplomacy
            .iter()
            .filter(|&(&(a, b), _)| {
                [a, b]
                    .iter()
                    .all(|&owner| owner < self.civilization_count || owner == BARBARIANS)
            })
            .collect();
        pairs.sort_by_key(|&(key, _)| *key);
        pairs
            .into_iter()
            .map(|(&(a, b), r)| DiplomacySnapshot {
                players: [a, b],
                names: [self.players[a].name.clone(), self.players[b].name.clone()],
                alive: [self.player_alive(a), self.player_alive(b)],
                first_contact: r.first_contact,
                pact: r.first_contact.map(|_| r.pact),
                until: r.until,
                trust: r.trust,
                shared_vision: r.vision,
                embassy: r.embassy,
                offer: r.offer.clone(),
                war_plans: [
                    self.ai_war_plans.get(&(a, b)).copied().map(Into::into),
                    self.ai_war_plans.get(&(b, a)).copied().map(Into::into),
                ],
            })
            .collect()
    }
    pub(super) fn trace_diplomacy(
        &mut self,
        actor: usize,
        other: usize,
        action: &'static str,
        reason: &'static str,
        previous: Option<Pact>,
        offer: Option<&TreatyOffer>,
    ) {
        let current = self.contacted_pact(actor, other);
        self.trace.emit(
            self.turn,
            || {
                vec![
                    trace::Entity::Civilization(actor),
                    trace::Entity::Civilization(other),
                ]
            },
            || DiplomacyEvent {
                actor,
                other,
                action,
                reason,
                previous,
                current,
                offer: offer.cloned(),
            },
        );
    }
}

impl WarReason {
    pub(super) fn observation_name(self) -> &'static str {
        match self {
            Self::Spoils => "spoils",
            Self::Hatred => "hatred",
            Self::Excuse => "excuse",
            Self::Alliance => "alliance",
            Self::Space => "space",
            Self::Behavior => "behavior",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ai_pairs_capture_contact_treaty_and_expiry_without_player_messages() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.trace = trace::Collector::enabled(["*".to_owned()].into(), 1000);
        game.initialize_diplomatic_contact(1, 2);
        game.sign_pact(1, 2, Pact::Peace);
        let snapshots = game.observe_diplomacy();
        let pair = snapshots.iter().find(|p| p.players == [1, 2]).unwrap();
        assert_eq!(pair.pact, Some(Pact::Armistice));
        game.turn = pair.until;
        game.advance_diplomacy();
        let events = game.trace.drain();
        assert!(events.iter().any(|e| e.payload["action"] == "contact"));
        assert!(
            events
                .iter()
                .any(|e| e.payload["reason"] == "treaty_signed"
                    && e.payload["current"] == "Armistice")
        );
        assert!(
            events
                .iter()
                .any(|e| e.payload["reason"] == "treaty_timer_expired"
                    && e.payload["current"] == "Peace")
        );
    }
    #[test]
    fn uncontacted_storage_is_not_reported_as_war_and_order_is_stable() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.diplomacy.clear();
        game.diplomacy.insert((2, 3), Relation::default());
        game.diplomacy.insert((1, 2), Relation::default());
        let pairs = game.observe_diplomacy();
        assert_eq!(pairs[0].players, [1, 2]);
        assert!(pairs.iter().all(|p| p.pact.is_none()));
        assert_eq!(
            serde_json::to_value(&pairs).unwrap(),
            serde_json::to_value(game.observe_diplomacy()).unwrap()
        );
    }
}
