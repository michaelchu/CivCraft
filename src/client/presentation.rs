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
            if let VisualEvent::Combat { territory, .. } = event {
                Some(territory)
            } else {
                None
            }
        }) {
            let index =
                y.rem_euclid(MAP_H) as usize * MAP_W as usize + x.rem_euclid(MAP_W) as usize;
            owners[index]
        } else {
            sim.displayed_tile_owner(x, y)
        }
    }
    pub(super) fn city_overrides(&self) -> BTreeMap<u32, City> {
        let mut cities = BTreeMap::new();
        for event in &self.events {
            if let VisualEvent::Combat {
                city: Some(city), ..
            } = event
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
            VisualEvent::Combat {
                attacker,
                defender,
                hp,
                ..
            } => {
                // Keep the last 100 ms for the reference five-frame explosion.
                let battle_fraction = ((event.duration() - 0.100) / event.duration()) as f32;
                let phase = (p / battle_fraction.max(0.01)).min(1.0);
                let index = ((hp.len() - 1) as f32 * phase).floor() as usize;
                let mut a = attacker.clone();
                let mut d = defender.clone();
                (a.hp, d.hp) = hp[index];
                vec![(a, Vec2::ZERO), (d, Vec2::ZERO)]
            }
        }
    }
}
impl Game {
    pub(super) fn draw_playback(&self, assets: &Assets) {
        let Some(event) = self.playback.events.front() else {
            return;
        };
        let project = |x, y| {
            world::world_screen_position(
                self.sim.map_wrap,
                Self::viewport_rect(),
                self.camera_overview_position(),
                self.zoom,
                x,
                y,
            )
        };
        for (unit, phase) in self.playback.active() {
            let center = match event {
                VisualEvent::Move { from, to, .. } => {
                    let from_center = project(from.0, from.1);
                    let to_center = project(to.0, to.1);
                    let period = MAP_W as f32 * TILE_W * self.zoom;
                    let dx = (to_center.x - from_center.x + period * 0.5).rem_euclid(period)
                        - period * 0.5;
                    let dy = to_center.y - from_center.y;
                    let period_y = MAP_H as f32 * TILE_H * 0.5 * self.zoom;
                    let dy = if self.sim.map_wrap.wraps_y() {
                        (dy + period_y * 0.5).rem_euclid(period_y) - period_y * 0.5
                    } else {
                        dy
                    };
                    from_center + vec2(dx, dy) * phase.x
                }
                _ => project(unit.x, unit.y),
            };
            self.draw_unit(assets, &unit, center, 1);
        }
        if let VisualEvent::Combat {
            attacker,
            defender,
            hp,
            ..
        } = event
        {
            let elapsed = self.playback.progress() as f64 * event.duration();
            let explosion_start = event.duration() - 0.100;
            if elapsed >= explosion_start {
                let final_hp = hp.last().unwrap();
                // Bombardment may leave both alive; it has no death explosion.
                let loser = if final_hp.0 == 0 {
                    Some(attacker)
                } else if final_hp.1 == 0 {
                    Some(defender)
                } else {
                    None
                };
                if let Some(loser) = loser {
                    let center = project(loser.x, loser.y);
                    let frame = (((elapsed - explosion_start) / 0.020) as i32).min(4);
                    draw_texture_ex(
                        &assets.explosions,
                        center.x - 15.0 * self.zoom,
                        center.y - 15.0 * self.zoom,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(vec2(30.0, 30.0) * self.zoom),
                            source: Some(Rect::new(0.0, frame as f32 * 30.0, 30.0, 30.0)),
                            ..Default::default()
                        },
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
        let before = sim.units().iter().find(|u| u.id == id).unwrap().clone();
        assert!(sim.apply(Command::Move(id, 1, 0)));
        let mut playback = Playback::default();
        playback.update(&mut sim, 10.0);
        assert!(playback.pending());
        assert_eq!(
            (playback.overrides()[&id].x, playback.overrides()[&id].y),
            (before.x, before.y)
        );
        assert_eq!(
            (
                sim.units().iter().find(|u| u.id == id).unwrap().x,
                sim.units().iter().find(|u| u.id == id).unwrap().y
            ),
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
