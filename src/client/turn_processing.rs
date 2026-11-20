//! Turn ownership, nonblocking completion polling, and bounded presentation setup.
use super::*;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

pub(super) struct TurnPreparation {
    outcome: crate::sim::CompletedTurn,
    pending: VecDeque<PresentationEventView>,
    ready: VecDeque<PresentationEventView>,
}

fn prepare_batch<T>(
    pending: &mut VecDeque<T>,
    ready: &mut VecDeque<T>,
    limit: usize,
    budget: Duration,
    mut prepare: impl FnMut(&mut T),
) {
    let started = Instant::now();
    for count in 0..limit {
        if count > 0 && started.elapsed() >= budget {
            break;
        }
        let Some(mut event) = pending.pop_front() else {
            break;
        };
        prepare(&mut event);
        ready.push_back(event);
    }
}

impl Game {
    pub(crate) fn turn_busy(&self) -> bool {
        self.background_turn.is_some() || self.preparing_turn.is_some()
    }

    pub(super) fn start_background_turn(&mut self) {
        if self.turn_busy() || self.sim.game_over_view().is_some() {
            return;
        }
        match self.sim.start_background_turn(self.autosave_enabled) {
            Ok(task) => self.background_turn = Some(task),
            Err(error) => self.notify_important(format!("Could not start turn. {error}")),
        }
    }

    pub(crate) fn poll_background_turn(&mut self, assets: &Assets) -> Result<bool, String> {
        self.poll_background_turn_with(|event| {
            presentation::configure_event_combat_timing(event, &assets.civ3)
        })
    }

    fn poll_background_turn_with(
        &mut self,
        prepare: impl FnMut(&mut PresentationEventView),
    ) -> Result<bool, String> {
        if let Some(result) = self.background_turn.as_ref().and_then(|task| task.poll()) {
            self.background_turn = None;
            let mut outcome = match result {
                Ok(outcome) => outcome,
                Err(error) => {
                    self.flush_pending_notices();
                    return Err(error);
                }
            };
            self.preparing_turn = Some(TurnPreparation {
                pending: outcome.take_events(),
                outcome,
                ready: VecDeque::new(),
            });
        }
        let Some(preparing) = self.preparing_turn.as_mut() else {
            return Ok(false);
        };
        prepare_batch(
            &mut preparing.pending,
            &mut preparing.ready,
            64,
            Duration::from_millis(2),
            prepare,
        );
        if !preparing.pending.is_empty() {
            return Ok(false);
        }
        let prepared = self.preparing_turn.take().unwrap();
        let autosave = prepared.outcome.commit(&mut self.sim);
        self.flush_pending_notices();
        self.prepared_events.extend(prepared.ready);
        self.waiting_units.clear();
        self.select_next_unit();
        match autosave {
            Some(Ok(payload)) => self.pending_autosave = Some(payload),
            Some(Err(error)) => self.notify_important(format!("Autosave failed. {error}")),
            None => {}
        }
        Ok(true)
    }

    fn flush_pending_notices(&mut self) {
        for (message, important) in std::mem::take(&mut self.pending_notices) {
            if important {
                self.sim.notify_important(message);
            } else {
                self.sim.notify(message);
            }
        }
    }

    pub(crate) fn take_autosave_payload(&mut self) -> Option<Vec<u8>> {
        self.pending_autosave.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn game_freezes_gameplay_until_all_turn_events_are_ready() {
        let mut game = Game::from_simulation(crate::sim::Simulation::new(
            crate::test_support::seeded_map(),
        ));
        game.autosave_enabled = true;
        let turn = game.sim.turn_view();
        let actor = game
            .sim
            .rendered_unit_views()
            .into_iter()
            .find(|unit| unit.owner == HUMAN)
            .unwrap();
        let position = game.sim.unit_position_view(actor.id);
        game.perform_action(UiAction::EndTurn);
        assert!(game.turn_busy());
        game.perform_action(UiAction::EndTurn);
        game.perform_action(UiAction::Fortify);
        game.perform_action(UiAction::ToggleMusicMute);
        game.notify_important("Asynchronous save failure");
        assert!(
            !game
                .sim
                .campaign_notifications_view()
                .iter()
                .any(|notice| notice.text == "Asynchronous save failure")
        );
        assert!(game.music_muted);
        assert_eq!(game.sim.turn_view(), turn);
        assert_eq!(game.sim.unit_position_view(actor.id), position);
        let mut outcome = game.background_turn.take().unwrap().wait().unwrap();
        outcome.take_events();
        let event = PresentationEventView::Move {
            unit: actor,
            from: (0, 24),
            to: (1, 24),
            duration: 1.0,
        };
        game.preparing_turn = Some(TurnPreparation {
            outcome,
            pending: (0..129).map(|_| event.clone()).collect(),
            ready: VecDeque::new(),
        });
        assert!(!game.poll_background_turn_with(|_| {}).unwrap());
        assert_eq!(game.sim.turn_view(), turn);
        assert!(game.take_autosave_payload().is_none());
        let deadline = Instant::now() + Duration::from_secs(5);
        while game.turn_busy() {
            assert!(Instant::now() < deadline);
            game.poll_background_turn_with(|_| {}).unwrap();
        }
        assert_eq!(game.sim.turn_view(), turn + 1);
        assert_eq!(game.prepared_events.len(), 129);
        assert!(game.take_autosave_payload().is_some());
        assert!(
            game.sim
                .campaign_notifications_view()
                .iter()
                .any(|notice| notice.text == "Asynchronous save failure")
        );
        assert!(game.pending_notices.is_empty());
    }

    #[test]
    fn preparation_is_bounded_ordered_and_makes_progress_with_zero_budget() {
        let mut pending: VecDeque<_> = (0..10).collect();
        let mut ready = VecDeque::new();
        prepare_batch(
            &mut pending,
            &mut ready,
            3,
            Duration::from_secs(1),
            |value| *value += 10,
        );
        assert_eq!(ready, VecDeque::from([10, 11, 12]));
        assert_eq!(pending.len(), 7);
        prepare_batch(&mut pending, &mut ready, 3, Duration::ZERO, |value| {
            *value += 10
        });
        assert_eq!(pending.len(), 6);
        while !pending.is_empty() {
            prepare_batch(&mut pending, &mut ready, 3, Duration::ZERO, |value| {
                *value += 10
            });
        }
        assert_eq!(
            ready.into_iter().collect::<Vec<_>>(),
            (10..20).collect::<Vec<_>>()
        );
    }
}
