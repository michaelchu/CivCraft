//! Optional, non-persistent render events. No RNG or gameplay decisions here.
use super::*;
use std::collections::VecDeque;

#[derive(Clone)]
pub(crate) enum VisualEvent {
    Move {
        unit: Unit,
        from: (i32, i32),
        to: (i32, i32),
    },
    Combat {
        attacker: Unit,
        defender: Unit,
        hp: Vec<(i32, i32)>,
        city: Option<City>,
        territory: Vec<Option<usize>>,
    },
}
impl VisualEvent {
    pub(crate) fn actors(&self) -> impl Iterator<Item = &Unit> {
        let (first, second) = match self {
            Self::Move { unit, .. } => (unit, None),
            Self::Combat {
                attacker, defender, ..
            } => (attacker, Some(defender)),
        };
        std::iter::once(first).chain(second)
    }
    pub(crate) fn duration(&self) -> f64 {
        match self {
            Self::Move { .. } => 0.030, // native smooth_move_unit_msec
            Self::Combat { hp, .. } => {
                let first = hp.first().copied().unwrap_or_default();
                let last = hp.last().copied().unwrap_or_default();
                ((first.0 - last.0 + first.1 - last.1).max(1) as f64 * 0.010) + 0.100
            }
        }
    }
}
#[derive(Clone, Default)]
pub(super) struct VisualEvents {
    pub(super) enabled: bool,
    queue: VecDeque<VisualEvent>,
}
impl Simulation {
    pub(crate) fn enable_visual_events(&mut self) {
        self.visual_events.enabled = true;
        self.visual_events.queue.clear();
    }
    pub(crate) fn take_visual_events(&mut self) -> VecDeque<VisualEvent> {
        std::mem::take(&mut self.visual_events.queue)
    }
    pub(super) fn publish_visual_event(&mut self, event: VisualEvent) {
        if !self.visual_events.enabled {
            return;
        }
        // Combat reveals the actual participants, including the defender in
        // a city. Ordinary movement still observes stealth/cargo visibility.
        let observable = match &event {
            VisualEvent::Move { unit, to, .. } => {
                let mut observed = unit.clone();
                (observed.x, observed.y) = *to;
                observed.transport = None;
                unit.owner == HUMAN || self.unit_visible_to_human(&observed)
            }
            VisualEvent::Combat {
                attacker, defender, ..
            } => {
                attacker.owner == HUMAN
                    || defender.owner == HUMAN
                    || (self.unit_visible_to_human(attacker)
                        && self.unit_visible_to_human(defender))
            }
        };
        if !observable {
            return;
        }
        if self.visual_events.queue.len() >= 256 {
            self.visual_events.queue.pop_front();
        }
        self.visual_events.queue.push_back(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observer_records_real_results_without_changing_rng_or_saved_campaign() {
        for combat in [false, true] {
            let mut observed = Simulation::new(crate::mapgen::generate(42));
            let (id, _) = observed.ui_review_animation_setup(combat);
            let mut unobserved = observed.clone();
            unobserved.visual_events = VisualEvents::default();
            assert_eq!(
                observed.apply(Command::Move(id, 1, 0)),
                unobserved.apply(Command::Move(id, 1, 0))
            );
            assert_eq!(
                bincode::serialize(&observed).unwrap(),
                bincode::serialize(&unobserved).unwrap()
            );
            assert_eq!(observed.native_random, unobserved.native_random);
            let root = std::env::temp_dir()
                .join(format!("civcraft-observer-{}-{combat}", std::process::id()));
            std::fs::create_dir_all(&root).unwrap();
            let a = root.join("observed.civcraft");
            let b = root.join("unobserved.civcraft");
            observed.save_campaign(&a).unwrap();
            unobserved.save_campaign(&b).unwrap();
            assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
            std::fs::remove_file(a).unwrap();
            std::fs::remove_file(b).unwrap();
            std::fs::remove_dir(root).unwrap();
            let events = observed.take_visual_events();
            assert!(!events.is_empty());
            if combat {
                let VisualEvent::Combat {
                    attacker,
                    defender,
                    hp,
                    ..
                } = &events[0]
                else {
                    panic!("combat event first")
                };
                assert_eq!(hp[0], (attacker.hp, defender.hp));
                let (a, d) = hp.last().copied().unwrap();
                assert!(a == 0 || d == 0);
                for (unit, final_hp) in [(attacker, a), (defender, d)] {
                    if let Some(survivor) = observed.units.iter().find(|u| u.id == unit.id) {
                        assert_eq!(survivor.hp, final_hp);
                    } else {
                        assert_eq!(final_hp, 0);
                    }
                }
            } else {
                assert!(matches!(
                    &events[0],
                    VisualEvent::Move { from: (20, 24), .. }
                ));
            }
        }
    }
}
