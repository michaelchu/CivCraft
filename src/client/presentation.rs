//! Non-blocking playback of real simulation events; campaign state stays final.
use super::*;
use std::collections::{BTreeMap, VecDeque};

#[derive(Default)]
pub(super) struct Playback {
    events: VecDeque<VisualEvent>,
    started: f64,
    pub(super) review_progress: Option<f32>,
}
impl Playback {
    pub(super) fn update(&mut self, sim: &mut Simulation, now: f64) {
        let incoming = sim.take_visual_events();
        if self.events.is_empty() {
            self.started = now;
        }
        self.events.extend(incoming);
        // Native clients cap simultaneous animation work too. Keep this
        // bounded even when an entire AI turn generates many visible steps.
        while self.events.len() > 256 {
            self.events.pop_front();
        }
        if self.review_progress.is_some() {
            return;
        }
        while let Some(event) = self.events.front() {
            if now - self.started < event.duration() {
                break;
            }
            self.started += event.duration();
            self.events.pop_front();
        }
    }
    pub(super) fn finish(&mut self) {
        self.events.clear();
        self.review_progress = None;
    }
    fn progress(&self) -> f32 {
        self.review_progress
            .unwrap_or_else(|| {
                self.events.front().map_or(1.0, |event| {
                    ((get_time() - self.started) / event.duration()) as f32
                })
            })
            .clamp(0.0, 1.0)
    }
    pub(super) fn pending(&self) -> bool {
        !self.events.is_empty()
    }
    pub(super) fn tile_owner(&self, sim: &Simulation, x: i32, y: i32) -> Option<usize> {
        if let Some(owners) = self.events.iter().find_map(|event| {
            if let VisualEvent::Combat(combat) = event {
                Some(&combat.territory)
            } else {
                None
            }
        }) {
            let index = sim.map_wrap.index(x, y);
            owners[index]
        } else {
            sim.displayed_tile_owner(x, y)
        }
    }
    pub(super) fn city_overrides(&self) -> BTreeMap<u32, City> {
        let mut cities = BTreeMap::new();
        for event in &self.events {
            if let VisualEvent::Combat(combat) = event
                && let Some(city) = &combat.city
            {
                cities.entry(city.id).or_insert_with(|| city.clone());
            }
        }
        cities
    }
    pub(super) fn overrides(&self) -> BTreeMap<u32, Unit> {
        let mut units = BTreeMap::new();
        for event in &self.events {
            for unit in event.actors() {
                units.entry(unit.id).or_insert_with(|| unit.clone());
            }
        }
        units
    }
    pub(super) fn is_active(&self, id: u32) -> bool {
        self.events
            .front()
            .is_some_and(|e| e.actors().any(|u| u.id == id))
    }
    pub(super) fn active(&self) -> Vec<(Unit, Vec2)> {
        let Some(event) = self.events.front() else {
            return Vec::new();
        };
        let p = self.progress();
        match event {
            VisualEvent::Move { unit, .. } => vec![(unit.clone(), vec2(p, 0.0))],
            VisualEvent::Capture { unit } => vec![(unit.clone(), vec2(0.0, 0.0))],
            VisualEvent::Combat(combat) => {
                // Keep the last 100 ms for the reference five-frame explosion.
                let battle_fraction = ((event.duration() - 0.100) / event.duration()) as f32;
                let phase = (p / battle_fraction.max(0.01)).min(1.0);
                let index = ((combat.hp.len() - 1) as f32 * phase).floor() as usize;
                let mut a = combat.attacker.clone();
                let mut d = combat.defender.clone();
                (a.hp, d.hp) = combat.hp[index];
                vec![(a, vec2(0.0, index as f32)), (d, vec2(0.0, index as f32))]
            }
        }
    }
}
impl Game {
    pub(super) fn draw_playback(&self, assets: &Assets) {
        let Some(event) = self.playback.events.front() else {
            return;
        };
        let project = |x, y| self.world_screen_position(x, y);
        let progress = self.playback.progress();
        for (unit, phase) in self.playback.active() {
            let center = match event {
                VisualEvent::Move { from, to, .. } => {
                    let from_center = project(from.0, from.1);
                    let to_center = project(to.0, to.1);
                    let period = self.sim.map_wrap.width() as f32 * TILE_W * self.zoom;
                    let dx = map_view::shortest_wrapped_delta(to_center.x - from_center.x, period);
                    let dy = to_center.y - from_center.y;
                    from_center + vec2(dx, dy) * phase.x
                }
                _ => project(unit.x, unit.y),
            };
            use super::tileset::Civ3UnitAction as Action;
            let action = match event {
                VisualEvent::Move { .. } => Action::Run,
                VisualEvent::Capture { .. } => Action::Capture,
                VisualEvent::Combat(combat) => {
                    let (attacker_hp, defender_hp) = combat
                        .hp
                        .get(phase.y as usize)
                        .copied()
                        .unwrap_or_default();
                    let is_attacker = unit.id == combat.attacker.id;
                    let unit_hp = if is_attacker { attacker_hp } else { defender_hp };
                    let opponent_hp = if is_attacker { defender_hp } else { attacker_hp };
                    if progress >= 0.90 && unit_hp <= 0 {
                        Action::Death
                    } else if progress >= 0.90 && opponent_hp <= 0 {
                        Action::Victory
                    } else if is_attacker {
                        match (phase.y as usize) % 3 {
                            0 => Action::Attack,
                            1 => Action::Attack2,
                            _ => Action::Attack3,
                        }
                    } else {
                        Action::Defend
                    }
                }
            };
            self.draw_unit(
                assets,
                &unit,
                center,
                1,
                action,
                progress,
                Some(event.duration() as f32),
            );
        }
        if let VisualEvent::Combat(combat) = event {
            let elapsed = self.playback.progress() as f64 * event.duration();
            let explosion_start = event.duration() - 0.100;
            if elapsed >= explosion_start {
                let final_hp = combat.hp.last().unwrap();
                let loser = if final_hp.0 == 0 {
                    Some(&combat.attacker)
                } else if final_hp.1 == 0 {
                    Some(&combat.defender)
                } else {
                    None
                };
                if combat.bombard_effects && loser.is_none() {
                    let center = project(combat.defender.x, combat.defender.y);
                    let frame = (((elapsed - explosion_start) / 0.020) as i32).min(4);
                    let radius = (7.0 + frame as f32 * 3.0) * self.zoom;
                    draw_circle(
                        center.x,
                        center.y,
                        radius,
                        Color::new(1.0, 0.42, 0.08, 0.72),
                    );
                    draw_circle_lines(
                        center.x,
                        center.y,
                        radius,
                        (2.0 * self.zoom).max(1.0),
                        Color::from_rgba(255, 222, 115, 240),
                    );
                }
                // Bombardment may leave both alive; it has no death explosion.
                if let Some(loser) = loser {
                    let center = project(loser.x, loser.y);
                    let frame = (((elapsed - explosion_start) / 0.020) as i32).min(4);
                    let radius = (7.0 + frame as f32 * 3.0) * self.zoom;
                    draw_circle(
                        center.x,
                        center.y,
                        radius,
                        Color::new(1.0, 0.42, 0.08, 0.72),
                    );
                    draw_circle_lines(
                        center.x,
                        center.y,
                        radius,
                        (2.0 * self.zoom).max(1.0),
                        Color::from_rgba(255, 222, 115, 240),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn playback_is_clock_driven_and_drops_all_overrides_at_completion() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (id, target) = sim.ui_review_animation_setup(false);
        let before = sim.unit_by_id(id).unwrap().clone();
        assert!(sim.apply(Command::Move(id, 1, 0)));
        let mut playback = Playback::default();
        playback.update(&mut sim, 10.0);
        assert!(playback.pending());
        assert_eq!(
            (playback.overrides()[&id].x, playback.overrides()[&id].y),
            (before.x, before.y)
        );
        assert_eq!(
            {
                let unit = sim.unit_by_id(id).unwrap();
                (unit.x, unit.y)
            },
            target
        );
        playback.update(&mut sim, 10.015);
        assert!(playback.is_active(id));
        playback.update(&mut sim, 10.031);
        assert!(!playback.pending());
        assert!(playback.overrides().is_empty());
        assert!(playback.city_overrides().is_empty());
    }
}
