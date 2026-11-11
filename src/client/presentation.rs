//! Non-blocking playback of real simulation events; campaign state stays final.
use super::*;
use std::collections::{BTreeMap, VecDeque};

#[derive(Default)]
pub(super) struct Playback {
    events: VecDeque<VisualEvent>,
    started: f64,
    front_sound_started: bool,
    sound_requests: VecDeque<super::unit_sound::UnitSoundRequest>,
    pub(super) review_progress: Option<f32>,
}
impl Playback {
    pub(super) fn update(&mut self, sim: &mut Simulation, now: f64, active_unit: Option<u32>) {
        let incoming = sim.take_visual_events();
        if self.events.is_empty() {
            self.started = now;
        }
        self.events.extend(incoming);
        // Native clients cap simultaneous animation work too. Keep this
        // bounded even when an entire AI turn generates many visible steps.
        let mut dropped_front = false;
        while self.events.len() > 256 {
            self.events.pop_front();
            dropped_front = true;
        }
        if dropped_front {
            self.front_sound_started = false;
        }
        if self.review_progress.is_some() {
            return;
        }
        while let Some(event) = self.events.front() {
            if !self.front_sound_started {
                self.sound_requests
                    .extend(sound_requests(event, active_unit));
                self.front_sound_started = true;
            }
            if now - self.started < event.duration() {
                break;
            }
            self.started += event.duration();
            self.events.pop_front();
            self.front_sound_started = false;
        }
    }
    pub(super) fn take_sound_requests(&mut self) -> VecDeque<super::unit_sound::UnitSoundRequest> {
        std::mem::take(&mut self.sound_requests)
    }
    pub(super) fn finish(&mut self) {
        self.events.clear();
        self.sound_requests.clear();
        self.front_sound_started = false;
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
            VisualEvent::Capture { unit } | VisualEvent::FoundCity { unit } => {
                vec![(unit.clone(), vec2(0.0, 0.0))]
            }
            VisualEvent::WorkStarted { .. } | VisualEvent::Fortify { .. } => Vec::new(),
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

fn sound_requests(
    event: &VisualEvent,
    active_unit: Option<u32>,
) -> Vec<super::unit_sound::UnitSoundRequest> {
    let request =
        |unit: &Unit, action: &str, delay_seconds: f64| super::unit_sound::UnitSoundRequest {
            unit: unit.clone(),
            action: action.to_owned(),
            delay_seconds,
        };
    match event {
        VisualEvent::Move { unit, .. } => vec![request(unit, "RUN", 0.0)],
        VisualEvent::Capture { unit } => vec![request(unit, "CAPTURE", 0.0)],
        VisualEvent::FoundCity { unit } => vec![request(unit, "BUILD", 0.0)],
        VisualEvent::WorkStarted { unit, kind } => {
            let action = match kind {
                WorkKind::Road | WorkKind::Railroad => "ROAD",
                WorkKind::Mine | WorkKind::OilWell | WorkKind::OilPlatform => "MINE",
                WorkKind::Irrigation | WorkKind::Farmland | WorkKind::Cultivate => "IRRIGATE",
                WorkKind::Plant => "PLANT",
                WorkKind::BuildBase(Base::Fortress) => "FORTRESS",
                WorkKind::BuildBase(_) | WorkKind::Clean(_) => "BUILD",
                WorkKind::Pillage(_) => return Vec::new(),
            };
            vec![request(unit, action, 0.0)]
        }
        VisualEvent::Fortify { unit } => vec![request(unit, "FORTIFY", 0.0)],
        VisualEvent::Combat(combat) => {
            let steps = combat.hp.len().saturating_sub(1).max(1);
            let mut requests = Vec::with_capacity(steps * 2 + 2);
            for index in 0..steps {
                let delay = event.duration() * index as f64 / steps as f64;
                let attack = ["ATTACK1", "ATTACK2", "ATTACK3"][index % 3];
                requests.push(request(&combat.attacker, attack, delay));
                requests.push(request(&combat.defender, "DEFEND", delay));
            }
            if let Some((attacker_hp, defender_hp)) = combat.hp.last().copied() {
                let end = event.duration() * 0.90;
                if attacker_hp <= 0 {
                    requests.push(request(&combat.attacker, "DEATH", end));
                    requests.push(request(&combat.defender, "VICTORY", end));
                } else if defender_hp <= 0 {
                    requests.push(request(&combat.defender, "DEATH", end));
                    requests.push(request(&combat.attacker, "VICTORY", end));
                }
            }
            requests
        }
    }
    .into_iter()
    .filter(|request| Some(request.unit.id) == active_unit)
    .collect()
}
impl Game {
    pub(super) fn draw_playback(&self, assets: &Assets) {
        let Some(event) = self.playback.events.front() else {
            return;
        };
        if matches!(
            event,
            VisualEvent::WorkStarted { .. } | VisualEvent::Fortify { .. }
        ) {
            return;
        }
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
                VisualEvent::FoundCity { .. } => Action::Build,
                VisualEvent::Combat(combat) => {
                    let (attacker_hp, defender_hp) =
                        combat.hp.get(phase.y as usize).copied().unwrap_or_default();
                    let is_attacker = unit.id == combat.attacker.id;
                    let unit_hp = if is_attacker {
                        attacker_hp
                    } else {
                        defender_hp
                    };
                    let opponent_hp = if is_attacker {
                        defender_hp
                    } else {
                        attacker_hp
                    };
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
                VisualEvent::WorkStarted { .. } | VisualEvent::Fortify { .. } => Action::Default,
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
        playback.update(&mut sim, 10.0, Some(id));
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
        playback.update(&mut sim, 10.015, Some(id));
        assert!(playback.is_active(id));
        playback.update(&mut sim, 10.031, Some(id));
        assert!(!playback.pending());
        assert!(playback.overrides().is_empty());
        assert!(playback.city_overrides().is_empty());
    }

    #[test]
    fn movement_sound_request_is_emitted_once_when_the_event_starts() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (id, _) = sim.ui_review_animation_setup(false);
        assert!(sim.apply(Command::Move(id, 1, 0)));
        let mut playback = Playback::default();
        playback.update(&mut sim, 10.0, Some(id));
        let requests = playback.take_sound_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].unit.id, id);
        assert_eq!(requests[0].action, "RUN");
        playback.update(&mut sim, 10.01, Some(id));
        assert!(playback.take_sound_requests().is_empty());
    }

    #[test]
    fn combat_sounds_follow_each_attack_and_terminal_result() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (attacker_id, _) = sim.ui_review_animation_setup(true);
        let attacker = sim.unit_by_id(attacker_id).unwrap().clone();
        let defender = sim
            .units()
            .iter()
            .find(|unit| unit.owner != HUMAN)
            .unwrap()
            .clone();
        let event = VisualEvent::Combat(Box::new(CombatVisual {
            attacker,
            defender,
            hp: vec![(3, 3), (2, 1), (2, 0)],
            city: None,
            bombard_effects: false,
            territory: Vec::new(),
        }));
        let requests = sound_requests(&event, Some(attacker_id));
        assert_eq!(
            requests
                .iter()
                .map(|request| request.action.as_str())
                .collect::<Vec<_>>(),
            ["ATTACK1", "ATTACK2", "VICTORY"]
        );
        assert!(requests[1].delay_seconds > requests[0].delay_seconds);
        assert!(requests[2].delay_seconds > requests[1].delay_seconds);
    }

    #[test]
    fn worker_order_uses_its_ini_action_name() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (id, _) = sim.ui_review_animation_setup(false);
        let unit = sim.unit_by_id(id).unwrap().clone();
        let requests = sound_requests(
            &VisualEvent::WorkStarted {
                unit: unit.clone(),
                kind: WorkKind::Mine,
            },
            Some(id),
        );
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].action, "MINE");
    }

    #[test]
    fn unit_events_use_the_matching_ini_sound_action() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (id, _) = sim.ui_review_animation_setup(false);
        let unit = sim.unit_by_id(id).unwrap().clone();

        for (event, expected_action) in [
            (
                VisualEvent::Move {
                    unit: unit.clone(),
                    from: (0, 0),
                    to: (1, 0),
                },
                "RUN",
            ),
            (VisualEvent::Capture { unit: unit.clone() }, "CAPTURE"),
            (VisualEvent::FoundCity { unit: unit.clone() }, "BUILD"),
            (VisualEvent::Fortify { unit: unit.clone() }, "FORTIFY"),
        ] {
            let requests = sound_requests(&event, Some(unit.id));
            assert_eq!(requests.len(), 1, "{expected_action} request count");
            assert_eq!(requests[0].unit.id, unit.id);
            assert_eq!(requests[0].action, expected_action);
            assert_eq!(requests[0].delay_seconds, 0.0);
        }
    }

    #[test]
    fn worker_orders_map_to_their_ini_sound_actions() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (id, _) = sim.ui_review_animation_setup(false);
        let unit = sim.unit_by_id(id).unwrap().clone();
        let cases = [
            (WorkKind::Road, Some("ROAD")),
            (WorkKind::Railroad, Some("ROAD")),
            (WorkKind::Mine, Some("MINE")),
            (WorkKind::OilWell, Some("MINE")),
            (WorkKind::OilPlatform, Some("MINE")),
            (WorkKind::Irrigation, Some("IRRIGATE")),
            (WorkKind::Farmland, Some("IRRIGATE")),
            (WorkKind::Cultivate, Some("IRRIGATE")),
            (WorkKind::Plant, Some("PLANT")),
            (WorkKind::BuildBase(Base::Fortress), Some("FORTRESS")),
            (WorkKind::BuildBase(Base::Fort), Some("BUILD")),
            (WorkKind::BuildBase(Base::Airstrip), Some("BUILD")),
            (WorkKind::BuildBase(Base::Airbase), Some("BUILD")),
            (WorkKind::Clean(Contamination::Pollution), Some("BUILD")),
            (WorkKind::Clean(Contamination::Fallout), Some("BUILD")),
            (WorkKind::Clean(Contamination::Craters), Some("BUILD")),
            (WorkKind::Pillage(PillageTarget::Road), None),
        ];

        for (kind, expected_action) in cases {
            let requests = sound_requests(
                &VisualEvent::WorkStarted {
                    unit: unit.clone(),
                    kind,
                },
                Some(id),
            );
            match expected_action {
                Some(action) => {
                    assert_eq!(requests.len(), 1, "{kind:?} request count");
                    assert_eq!(requests[0].unit.id, unit.id);
                    assert_eq!(requests[0].action, action, "{kind:?}");
                }
                None => assert!(requests.is_empty(), "{kind:?} should stay silent"),
            }
        }
    }

    #[test]
    fn combat_sound_uses_death_and_victory_for_either_loser() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (attacker_id, _) = sim.ui_review_animation_setup(true);
        let attacker = sim.unit_by_id(attacker_id).unwrap().clone();
        let defender = sim
            .units()
            .iter()
            .find(|unit| unit.owner != HUMAN)
            .unwrap()
            .clone();

        for (hp, loser_id, winner_id) in [
            (vec![(3, 3), (0, 2)], attacker.id, defender.id),
            (vec![(3, 3), (2, 0)], defender.id, attacker.id),
        ] {
            let event = VisualEvent::Combat(Box::new(CombatVisual {
                attacker: attacker.clone(),
                defender: defender.clone(),
                hp,
                city: None,
                bombard_effects: false,
                territory: Vec::new(),
            }));
            let requests = sound_requests(&event, Some(attacker.id));
            assert_eq!(requests.len(), 2);
            assert_eq!(requests[0].action, "ATTACK1");
            assert_eq!(
                requests[1].action,
                if winner_id == attacker.id { "VICTORY" } else { "DEATH" }
            );
            assert_eq!(requests[1].unit.id, attacker.id);
            assert_ne!(loser_id, winner_id);
        }
    }
}
