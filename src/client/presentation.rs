//! Non-blocking playback of real simulation events; campaign state stays final.
use super::*;
use std::collections::{BTreeMap, VecDeque};

const COMBAT_EXPLOSION_SECONDS: f64 = 0.100;

fn combat_impact_duration(combat: &crate::sim::CombatPresentationView) -> f64 {
    if combat.bombard_effects {
        if combat.impact_duration > 0.0 {
            combat.impact_duration
        } else {
            COMBAT_EXPLOSION_SECONDS
        }
    } else {
        0.0
    }
}

fn air_mission_total_duration(
    kind: &crate::sim::AirMissionPresentationKind,
    fly: f64,
    action: f64,
    outcome: f64,
) -> f64 {
    match kind {
        crate::sim::AirMissionPresentationKind::Rebase => fly,
        crate::sim::AirMissionPresentationKind::CruiseMissileStrike { .. } => fly + action,
        crate::sim::AirMissionPresentationKind::NuclearStrike { .. } => fly + action + outcome,
        crate::sim::AirMissionPresentationKind::Bombard { .. }
        | crate::sim::AirMissionPresentationKind::Recon => fly + action + fly,
        crate::sim::AirMissionPresentationKind::SamIntercept { shot_down }
        | crate::sim::AirMissionPresentationKind::AaUnitIntercept { shot_down, .. } => {
            if *shot_down {
                fly + action + outcome
            } else {
                fly + action + fly
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum AirMissionPhase {
    Outbound(f32),
    Action(f32),
    Crash(f32),
    Return(f32),
}

fn air_mission_phase(
    mission: &crate::sim::AirMissionPresentationView,
    elapsed: f64,
) -> AirMissionPhase {
    let t = elapsed.max(0.0);
    if matches!(mission.kind, crate::sim::AirMissionPresentationKind::Rebase) {
        return AirMissionPhase::Outbound((t / mission.fly_duration.max(0.001)).min(1.0) as f32);
    }
    if t < mission.fly_duration {
        return AirMissionPhase::Outbound((t / mission.fly_duration.max(0.001)).min(1.0) as f32);
    }
    let after_out = t - mission.fly_duration;
    if matches!(
        mission.kind,
        crate::sim::AirMissionPresentationKind::CruiseMissileStrike { .. }
    ) {
        return AirMissionPhase::Action(
            (after_out / mission.action_duration.max(0.001)).min(1.0) as f32
        );
    }
    if after_out < mission.action_duration {
        return AirMissionPhase::Action(
            (after_out / mission.action_duration.max(0.001)).min(1.0) as f32
        );
    }
    let after_action = after_out - mission.action_duration;
    let ends_at_target = matches!(
        mission.kind,
        crate::sim::AirMissionPresentationKind::NuclearStrike { .. }
            | crate::sim::AirMissionPresentationKind::SamIntercept { shot_down: true }
            | crate::sim::AirMissionPresentationKind::AaUnitIntercept {
                shot_down: true,
                ..
            }
    );
    if ends_at_target {
        AirMissionPhase::Crash((after_action / mission.outcome_duration.max(0.001)).min(1.0) as f32)
    } else {
        AirMissionPhase::Return((after_action / mission.fly_duration.max(0.001)).min(1.0) as f32)
    }
}

fn attack_action(round: usize) -> super::tileset::Civ3UnitAction {
    use super::tileset::Civ3UnitAction::*;
    [Attack, Attack2, Attack3][round % 3]
}

pub(super) fn configure_combat_timing(
    events: &mut VecDeque<PresentationEventView>,
    art: &super::tileset::Civ3,
) {
    for event in events {
        configure_event_combat_timing(event, art);
    }
}

pub(super) fn configure_event_combat_timing(
    event: &mut PresentationEventView,
    art: &super::tileset::Civ3,
) {
    use super::tileset::Civ3UnitAction::*;
    match event {
        PresentationEventView::Combat(combat) => {
            combat.round_durations = (0..combat.hp.len().saturating_sub(1).max(1))
                .map(|round| {
                    art.combat_clip_seconds(&combat.attacker, attack_action(round))
                        .max(art.combat_clip_seconds(&combat.defender, Defend))
                })
                .collect();
            let (a, d) = combat.hp.last().copied().unwrap_or((1, 1));
            combat.result_duration = if a <= 0 {
                art.combat_clip_seconds(&combat.attacker, Death)
                    .max(if combat.battle_ended {
                        art.combat_clip_seconds(&combat.defender, Victory)
                    } else {
                        0.0
                    })
            } else if d <= 0 {
                art.combat_clip_seconds(&combat.defender, Death)
                    .max(if combat.battle_ended {
                        art.combat_clip_seconds(&combat.attacker, Victory)
                    } else {
                        0.0
                    })
            } else {
                0.0
            };
            combat.impact_duration = if combat.bombard_effects {
                art.bomb_effect_duration_seconds()
                    .unwrap_or(COMBAT_EXPLOSION_SECONDS)
            } else {
                0.0
            };
            combat.duration = combat.round_durations.iter().sum::<f64>()
                + combat.result_duration
                + combat_impact_duration(combat);
        }
        PresentationEventView::AirMission(mission) => {
            match &mission.kind {
                crate::sim::AirMissionPresentationKind::Bombard { defender, .. } => {
                    let bomb_seconds = art.combat_clip_seconds(&mission.aircraft, Victory);
                    let defender_death = defender
                        .as_ref()
                        .filter(|(_, _, hp_after)| *hp_after <= 0)
                        .map_or(0.0, |(unit, _, _)| art.combat_clip_seconds(unit, Death));
                    mission.action_duration = bomb_seconds.max(defender_death).max(0.5);
                }
                crate::sim::AirMissionPresentationKind::CruiseMissileStrike {
                    defender, ..
                } => {
                    let impact_seconds = art.combat_clip_seconds(&mission.aircraft, Death);
                    let defender_death = defender
                        .as_ref()
                        .filter(|(_, _, hp_after)| *hp_after <= 0)
                        .map_or(0.0, |(unit, _, _)| art.combat_clip_seconds(unit, Death));
                    mission.action_duration = impact_seconds.max(defender_death).max(0.65);
                }
                crate::sim::AirMissionPresentationKind::NuclearStrike { intercepted_by_sdi } => {
                    mission.fly_duration = art
                        .combat_clip_seconds(&mission.aircraft, Capture)
                        .max(0.75);
                    let trail_seconds = art.combat_clip_seconds(&mission.aircraft, Run).max(0.5);
                    mission.action_duration = if *intercepted_by_sdi {
                        trail_seconds.max(art.sdi_effect_duration_seconds().unwrap_or(0.55))
                    } else {
                        trail_seconds
                    };
                    // Civ III stores the nuclear mushroom cloud in DEATH (`ICBMBomb.flc` /
                    // `TacticalICBMBomb.flc`) and mid-air SDI destruction in VICTORY.
                    let blast_action = if *intercepted_by_sdi { Victory } else { Death };
                    mission.outcome_duration = art
                        .combat_clip_seconds(&mission.aircraft, blast_action)
                        .max(1.0);
                }
                crate::sim::AirMissionPresentationKind::SamIntercept { shot_down } => {
                    mission.action_duration = art.sam_effect_duration_seconds().unwrap_or(0.55);
                    if *shot_down {
                        mission.outcome_duration =
                            art.combat_clip_seconds(&mission.aircraft, Death).max(0.5);
                    }
                }
                crate::sim::AirMissionPresentationKind::AaUnitIntercept { aa_unit, shot_down } => {
                    mission.action_duration = art.combat_clip_seconds(aa_unit, Attack).max(0.5);
                    if *shot_down {
                        mission.outcome_duration =
                            art.combat_clip_seconds(&mission.aircraft, Death).max(0.5);
                    }
                }
                crate::sim::AirMissionPresentationKind::Recon
                | crate::sim::AirMissionPresentationKind::Rebase => {}
            }
            mission.duration = air_mission_total_duration(
                &mission.kind,
                mission.fly_duration,
                mission.action_duration,
                mission.outcome_duration,
            );
        }
        _ => {}
    }
}

/// HP changes only after a complete exchange; each clip receives its own clock.
fn combat_phase(
    combat: &crate::sim::CombatPresentationView,
    elapsed: f64,
) -> (usize, f32, f32, bool) {
    let mut remaining = elapsed.max(0.0);
    for (round, &seconds) in combat.round_durations.iter().enumerate() {
        if remaining < seconds {
            return (round, (remaining / seconds) as f32, seconds as f32, false);
        }
        remaining -= seconds;
    }
    let seconds = combat.result_duration.max(0.001);
    (
        combat.hp.len().saturating_sub(1),
        (remaining / seconds).min(1.0) as f32,
        seconds as f32,
        true,
    )
}

#[derive(Default)]
pub(super) struct Playback {
    events: VecDeque<PresentationEventView>,
    unit_overrides: BTreeMap<u32, RenderedUnitView>,
    city_overrides: BTreeMap<u32, RenderedCityView>,
    started: f64,
    front_sound_started: bool,
    sound_requests: VecDeque<super::unit_sound::UnitSoundRequest>,
    pub(super) review_progress: Option<f32>,
}
impl Playback {
    pub(super) fn update(
        &mut self,
        incoming: VecDeque<PresentationEventView>,
        now: f64,
        active_unit: Option<u32>,
    ) {
        if self.events.is_empty() {
            self.started = now;
        }
        let mut changed = !incoming.is_empty();
        self.events.extend(incoming);
        // Native clients cap simultaneous animation work too. Keep this
        // bounded even when an entire AI turn generates many visible steps.
        let mut dropped_front = false;
        while self.events.len() > 256 {
            self.events.pop_front();
            dropped_front = true;
            changed = true;
        }
        if dropped_front {
            self.front_sound_started = false;
        }
        if self.review_progress.is_some() {
            if changed {
                self.rebuild_overrides();
            }
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
            changed = true;
            self.front_sound_started = false;
        }
        if changed {
            self.rebuild_overrides();
        }
    }
    pub(super) fn take_sound_requests(&mut self) -> VecDeque<super::unit_sound::UnitSoundRequest> {
        std::mem::take(&mut self.sound_requests)
    }
    pub(super) fn finish(&mut self) {
        self.events.clear();
        self.unit_overrides.clear();
        self.city_overrides.clear();
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
    pub(super) fn review_first_combat_clip(&mut self, fraction: f64) -> bool {
        let Some(PresentationEventView::Combat(combat)) = self.events.front() else {
            return false;
        };
        let seconds = combat.round_durations.first().copied().unwrap_or(0.5);
        self.review_progress = Some((fraction * seconds / combat.duration) as f32);
        true
    }

    pub(super) fn review_combat_exchange(&mut self, exchange: usize, fraction: f64) -> bool {
        let Some(index) = self
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| matches!(event, PresentationEventView::Combat(_)))
            .nth(exchange)
            .map(|(index, _)| index)
        else {
            return false;
        };
        self.events.drain(..index);
        self.front_sound_started = false;
        self.rebuild_overrides();
        self.review_first_combat_clip(fraction)
    }

    pub(super) fn review_first_air_mission(&mut self, progress: f32) -> bool {
        let Some(PresentationEventView::AirMission(_)) = self.events.front() else {
            return false;
        };
        self.review_progress = Some(progress.clamp(0.0, 1.0));
        true
    }

    pub(super) fn pending(&self) -> bool {
        !self.events.is_empty()
    }
    fn rebuild_overrides(&mut self) {
        self.unit_overrides.clear();
        self.city_overrides.clear();
        for event in &self.events {
            for unit in event.actors() {
                self.unit_overrides
                    .entry(unit.id)
                    .or_insert_with(|| unit.clone());
            }
            if let PresentationEventView::Combat(combat) = event
                && let Some(city) = &combat.city
            {
                self.city_overrides
                    .entry(city.id)
                    .or_insert_with(|| city.clone());
            } else if let PresentationEventView::AirMission(mission) = event
                && let crate::sim::AirMissionPresentationKind::Bombard {
                    city: Some(city), ..
                }
                | crate::sim::AirMissionPresentationKind::CruiseMissileStrike {
                    city: Some(city),
                    ..
                } = &mission.kind
            {
                self.city_overrides
                    .entry(city.id)
                    .or_insert_with(|| city.clone());
            }
        }
    }
    pub(super) fn city_overrides(&self) -> &BTreeMap<u32, RenderedCityView> {
        &self.city_overrides
    }
    pub(super) fn overrides(&self) -> &BTreeMap<u32, RenderedUnitView> {
        &self.unit_overrides
    }
    pub(super) fn is_active(&self, id: u32) -> bool {
        self.events
            .front()
            .is_some_and(|event| event.actors().any(|unit| unit.id == id))
    }
    pub(super) fn active(&self) -> Vec<(RenderedUnitView, Vec2)> {
        let Some(event) = self.events.front() else {
            return Vec::new();
        };
        let p = self.progress();
        match event {
            PresentationEventView::Move { unit, .. } => vec![(unit.clone(), vec2(p, 0.0))],
            PresentationEventView::Capture { unit, .. }
            | PresentationEventView::FoundCity { unit, .. } => {
                vec![(unit.clone(), vec2(0.0, 0.0))]
            }
            PresentationEventView::WorkStarted { .. }
            | PresentationEventView::Fortify { .. }
            | PresentationEventView::EnterHut { .. } => Vec::new(),
            PresentationEventView::Combat(combat) => {
                let (index, _, _, _) = combat_phase(combat, f64::from(p) * event.duration());
                let mut a = combat.attacker.clone();
                let mut d = combat.defender.clone();
                (a.hp, d.hp) = combat.hp[index];
                vec![(a, vec2(0.0, index as f32)), (d, vec2(0.0, index as f32))]
            }
            PresentationEventView::AirMission(mission) => {
                let phase = air_mission_phase(mission, f64::from(p) * mission.duration);
                let mut aircraft = mission.aircraft.clone();
                aircraft.facing = match phase {
                    AirMissionPhase::Return(_) => mission.return_facing,
                    _ => mission.outbound_facing,
                };
                let mut result = Vec::with_capacity(2);
                match &mission.kind {
                    crate::sim::AirMissionPresentationKind::Bombard {
                        defender: Some((defender, hp_before, hp_after)),
                        ..
                    }
                    | crate::sim::AirMissionPresentationKind::CruiseMissileStrike {
                        defender: Some((defender, hp_before, hp_after)),
                        ..
                    } => {
                        let mut def = defender.clone();
                        def.hp = match phase {
                            AirMissionPhase::Outbound(_) => *hp_before,
                            AirMissionPhase::Action(t) if t < 0.5 => *hp_before,
                            _ => *hp_after,
                        };
                        result.push((def, vec2(0.0, 1.0)));
                    }
                    crate::sim::AirMissionPresentationKind::AaUnitIntercept { aa_unit, .. } => {
                        result.push((aa_unit.clone(), vec2(0.0, 2.0)));
                    }
                    _ => {}
                }
                result.push((aircraft, vec2(p, 0.0)));
                result
            }
        }
    }
}

fn sound_requests(
    event: &PresentationEventView,
    active_unit: Option<u32>,
) -> Vec<super::unit_sound::UnitSoundRequest> {
    if let PresentationEventView::AirMission(mission) = event {
        let selected_matches =
            active_unit.is_some_and(|id| event.actors().any(|unit| unit.id == id));
        let human_involved = mission.aircraft.owner == HUMAN
            || match &mission.kind {
                crate::sim::AirMissionPresentationKind::Bombard {
                    defender: Some((defender, _, _)),
                    ..
                }
                | crate::sim::AirMissionPresentationKind::CruiseMissileStrike {
                    defender: Some((defender, _, _)),
                    ..
                } => defender.owner == HUMAN,
                crate::sim::AirMissionPresentationKind::Bombard {
                    city: Some(city), ..
                }
                | crate::sim::AirMissionPresentationKind::CruiseMissileStrike {
                    city: Some(city),
                    ..
                } => city.owner == HUMAN,
                crate::sim::AirMissionPresentationKind::AaUnitIntercept { aa_unit, .. } => {
                    aa_unit.owner == HUMAN
                }
                crate::sim::AirMissionPresentationKind::SamIntercept { .. }
                | crate::sim::AirMissionPresentationKind::NuclearStrike { .. } => true,
                _ => false,
            };
        if !selected_matches && !human_involved {
            return Vec::new();
        }
        let mut requests = Vec::with_capacity(4);
        let mut push = |unit: &RenderedUnitView, action: &str, delay_seconds: f64| {
            requests.push(super::unit_sound::UnitSoundRequest {
                #[cfg(test)]
                unit_id: unit.id,
                kind: unit.kind,
                era_index: unit.era_index,
                action: action.to_owned(),
                delay_seconds,
            });
        };
        match &mission.kind {
            crate::sim::AirMissionPresentationKind::NuclearStrike { intercepted_by_sdi } => {
                // Civ III ICBM / Tactical Nuke INIs play RUN on descent, SDI Laser on
                // intercept, VICTORY (`ICBM Death.wav`) on shootdown, or DEATH (`ICBMBomb.wav`)
                // on mushroom-cloud detonation.
                push(&mission.aircraft, "RUN", mission.fly_duration);
                if *intercepted_by_sdi {
                    push(&mission.aircraft, "SDI_EFFECT", mission.fly_duration);
                    push(
                        &mission.aircraft,
                        "VICTORY",
                        mission.fly_duration + mission.action_duration,
                    );
                } else {
                    push(
                        &mission.aircraft,
                        "DEATH",
                        mission.fly_duration + mission.action_duration,
                    );
                }
                return requests;
            }
            crate::sim::AirMissionPresentationKind::CruiseMissileStrike { defender, .. } => {
                push(&mission.aircraft, "RUN", 0.0);
                push(&mission.aircraft, "DEATH", mission.fly_duration);
                if let Some((defender, _, hp_after)) = defender
                    && *hp_after <= 0
                {
                    push(
                        defender,
                        "DEATH",
                        mission.fly_duration + mission.action_duration * 0.45,
                    );
                }
                return requests;
            }
            _ => {}
        }
        push(&mission.aircraft, "RUN", 0.0);
        match &mission.kind {
            crate::sim::AirMissionPresentationKind::Bombard { defender, .. } => {
                // Civ III stores each aircraft's bombing-run sound under VICTORY.
                push(&mission.aircraft, "VICTORY", mission.fly_duration);
                if let Some((defender, _, hp_after)) = defender
                    && *hp_after <= 0
                {
                    push(
                        defender,
                        "DEATH",
                        mission.fly_duration + mission.action_duration * 0.45,
                    );
                }
            }
            crate::sim::AirMissionPresentationKind::Recon
            | crate::sim::AirMissionPresentationKind::Rebase => {}
            crate::sim::AirMissionPresentationKind::SamIntercept { shot_down } => {
                push(&mission.aircraft, "SAM_EFFECT", mission.fly_duration);
                if *shot_down {
                    push(
                        &mission.aircraft,
                        "DEATH",
                        mission.fly_duration + mission.action_duration,
                    );
                }
            }
            crate::sim::AirMissionPresentationKind::AaUnitIntercept { aa_unit, shot_down } => {
                push(aa_unit, "ATTACK1", mission.fly_duration);
                if *shot_down {
                    push(
                        &mission.aircraft,
                        "DEATH",
                        mission.fly_duration + mission.action_duration,
                    );
                }
            }
            crate::sim::AirMissionPresentationKind::NuclearStrike { .. }
            | crate::sim::AirMissionPresentationKind::CruiseMissileStrike { .. } => {
                unreachable!("handled above")
            }
        }
        return requests;
    }
    let is_hut = matches!(event, PresentationEventView::EnterHut { .. });
    let Some(active_unit) = active_unit
        .filter(|id| event.actors().any(|unit| unit.id == *id))
        .or_else(|| {
            is_hut
                .then(|| event.actors().next().map(|unit| unit.id))
                .flatten()
        })
    else {
        return Vec::new();
    };
    let capacity = match event {
        PresentationEventView::Combat(combat) => combat.hp.len().saturating_sub(1).max(1) + 2,
        _ => 1,
    };
    let mut requests = Vec::with_capacity(capacity);
    let mut request = |unit: &RenderedUnitView, action: &str, delay_seconds: f64| {
        if unit.id == active_unit {
            requests.push(super::unit_sound::UnitSoundRequest {
                #[cfg(test)]
                unit_id: unit.id,
                kind: unit.kind,
                era_index: unit.era_index,
                action: action.to_owned(),
                delay_seconds,
            });
        }
    };
    match event {
        PresentationEventView::Move { unit, .. } => request(unit, "RUN", 0.0),
        PresentationEventView::Capture { unit, .. } => request(unit, "CAPTURE", 0.0),
        PresentationEventView::FoundCity { unit, .. } => request(unit, "BUILD", 0.0),
        PresentationEventView::WorkStarted { unit, kind, .. } => {
            let action = match kind {
                WorkKind::Road | WorkKind::Railroad => "ROAD",
                WorkKind::Mine | WorkKind::OilWell | WorkKind::OilPlatform => "MINE",
                WorkKind::Irrigation | WorkKind::Farmland | WorkKind::Cultivate => "IRRIGATE",
                WorkKind::Plant => "PLANT",
                WorkKind::BuildBase(Base::Fortress) => "FORTRESS",
                WorkKind::BuildBase(_) | WorkKind::Clean(_) => "BUILD",
                WorkKind::Pillage(_) => return Vec::new(),
            };
            request(unit, action, 0.0);
        }
        PresentationEventView::Fortify { unit, .. } => request(unit, "FORTIFY", 0.0),
        PresentationEventView::EnterHut { unit, .. } => {
            requests.push(super::unit_sound::UnitSoundRequest {
                #[cfg(test)]
                unit_id: unit.id,
                kind: unit.kind,
                era_index: unit.era_index,
                action: "HUT".to_owned(),
                delay_seconds: 0.0,
            });
        }
        PresentationEventView::Combat(combat) => {
            let steps = combat.hp.len().saturating_sub(1).max(1);
            let mut delay = 0.0;
            for index in 0..steps {
                let attack = ["ATTACK1", "ATTACK2", "ATTACK3"][index % 3];
                request(&combat.attacker, attack, delay);
                request(&combat.defender, "DEFEND", delay);
                delay += combat.round_durations.get(index).copied().unwrap_or(0.0);
            }
            if let Some((attacker_hp, defender_hp)) = combat.hp.last().copied() {
                let end = combat.round_durations.iter().sum();
                if attacker_hp <= 0 {
                    request(&combat.attacker, "DEATH", end);
                    if combat.battle_ended {
                        request(&combat.defender, "VICTORY", end);
                    }
                } else if defender_hp <= 0 {
                    request(&combat.defender, "DEATH", end);
                    if combat.battle_ended {
                        request(&combat.attacker, "VICTORY", end);
                    }
                }
            }
            if combat.bombard_effects {
                let impact_start = (combat.duration - combat_impact_duration(combat)).max(0.0);
                request(&combat.attacker, "BOMB_EFFECT", impact_start);
            }
        }
        PresentationEventView::AirMission(_) => unreachable!("handled above"),
    }
    requests
}
impl Game {
    pub(super) fn draw_playback(&self, assets: &Assets, view: world::WorldMapView) {
        let Some(event) = self.playback.events.front() else {
            return;
        };
        if matches!(
            event,
            PresentationEventView::WorkStarted { .. }
                | PresentationEventView::Fortify { .. }
                | PresentationEventView::EnterHut { .. }
        ) {
            return;
        }
        let project = |x, y| view.project(self.sim.map_wrap(), x, y);
        let progress = self.playback.progress();
        if let PresentationEventView::AirMission(mission) = event {
            use super::tileset::Civ3UnitAction as Action;
            let elapsed = f64::from(progress) * mission.duration;
            let phase = air_mission_phase(mission, elapsed);
            let from_center = project(mission.from.0, mission.from.1);
            let to_center = project(mission.to.0, mission.to.1);
            let period = self.sim.map_wrap().width() as f32 * TILE_W * view.zoom;
            let dx = map_view::shortest_wrapped_delta(to_center.x - from_center.x, period);
            let dy = to_center.y - from_center.y;
            let flight_vector = vec2(dx, dy);

            // 0. Dedicated Civ III nuclear launch -> incoming trail / SDI laser -> mushroom cloud sequence.
            if let crate::sim::AirMissionPresentationKind::NuclearStrike { intercepted_by_sdi } =
                mission.kind
            {
                let palette_index = self.sim.player_team_palette_view(mission.aircraft.owner);
                let unit_zoom = view.zoom * WORLD_ART_SCALE;
                match phase {
                    AirMissionPhase::Outbound(t) => {
                        let _ = assets.civ3.draw_unit(
                            mission.aircraft.kind,
                            mission.aircraft.owner,
                            palette_index,
                            0,
                            mission.aircraft.era_index,
                            Action::Capture,
                            t,
                            Some(mission.fly_duration as f32),
                            from_center,
                            unit_zoom,
                            false,
                        );
                    }
                    AirMissionPhase::Action(t) => {
                        let _ = assets.civ3.draw_unit(
                            mission.aircraft.kind,
                            mission.aircraft.owner,
                            palette_index,
                            0,
                            mission.aircraft.era_index,
                            Action::Run,
                            t,
                            Some(mission.action_duration as f32),
                            to_center,
                            unit_zoom,
                            false,
                        );
                        if intercepted_by_sdi {
                            let drawn = assets.civ3.draw_sdi_effect(0, t, to_center, view.zoom);
                            if !drawn {
                                draw_line(
                                    to_center.x + 80.0 * view.zoom,
                                    to_center.y - 120.0 * view.zoom,
                                    to_center.x,
                                    to_center.y - 12.0 * view.zoom,
                                    (3.0 * view.zoom).max(2.0),
                                    Color::from_rgba(120, 235, 255, 240),
                                );
                            }
                        }
                    }
                    AirMissionPhase::Crash(t) | AirMissionPhase::Return(t) => {
                        let blast_action = if intercepted_by_sdi {
                            Action::Victory
                        } else {
                            Action::Death
                        };
                        let _ = assets.civ3.draw_unit(
                            mission.aircraft.kind,
                            mission.aircraft.owner,
                            palette_index,
                            0,
                            mission.aircraft.era_index,
                            blast_action,
                            t,
                            Some(mission.outcome_duration as f32),
                            to_center,
                            unit_zoom,
                            false,
                        );
                    }
                }
                return;
            }

            // 1. Draw any ground/naval target or firing AA unit underneath the aircraft.
            match &mission.kind {
                crate::sim::AirMissionPresentationKind::Bombard {
                    defender: Some((defender, hp_before, hp_after)),
                    ..
                }
                | crate::sim::AirMissionPresentationKind::CruiseMissileStrike {
                    defender: Some((defender, hp_before, hp_after)),
                    ..
                } => {
                    let mut def = defender.clone();
                    let (def_action, def_progress, def_duration) = match phase {
                        AirMissionPhase::Outbound(_) => (Action::Default, 0.0, None),
                        AirMissionPhase::Action(t) if t < 0.5 => {
                            def.hp = *hp_before;
                            (
                                Action::Defend,
                                t * 2.0,
                                Some((mission.action_duration * 0.5) as f32),
                            )
                        }
                        AirMissionPhase::Action(t) => {
                            def.hp = *hp_after;
                            if *hp_after <= 0 {
                                (
                                    Action::Death,
                                    (t - 0.5) * 2.0,
                                    Some((mission.action_duration * 0.5) as f32),
                                )
                            } else {
                                (
                                    Action::Defend,
                                    1.0,
                                    Some((mission.action_duration * 0.5) as f32),
                                )
                            }
                        }
                        AirMissionPhase::Return(_) | AirMissionPhase::Crash(_) => {
                            def.hp = *hp_after;
                            if *hp_after <= 0 {
                                (Action::Death, 1.0, Some(0.5))
                            } else {
                                (Action::Default, 0.0, None)
                            }
                        }
                    };
                    if !(*hp_after <= 0 && matches!(phase, AirMissionPhase::Return(_))) {
                        self.draw_unit_at_zoom(
                            assets,
                            &def,
                            to_center,
                            1,
                            def_action,
                            def_progress,
                            def_duration,
                            view.zoom,
                        );
                        self.draw_unit_health_at_zoom(assets, &def, to_center, view.zoom);
                    }
                }
                crate::sim::AirMissionPresentationKind::AaUnitIntercept { aa_unit, .. } => {
                    let (aa_action, aa_progress, aa_duration) = match phase {
                        AirMissionPhase::Action(t) => {
                            (Action::Attack, t, Some(mission.action_duration as f32))
                        }
                        _ => (Action::Default, 0.0, None),
                    };
                    self.draw_unit_at_zoom(
                        assets,
                        aa_unit,
                        to_center,
                        1,
                        aa_action,
                        aa_progress,
                        aa_duration,
                        view.zoom,
                    );
                    self.draw_unit_health_at_zoom(assets, aa_unit, to_center, view.zoom);
                }
                _ => {}
            }

            // 2. Draw the target explosion / SAM launch animation on the target tile.
            if let AirMissionPhase::Action(t) = phase {
                match &mission.kind {
                    crate::sim::AirMissionPresentationKind::Bombard { .. } => {
                        let palette_index =
                            self.sim.player_team_palette_view(mission.aircraft.owner);
                        // Civ III stores each aircraft's dedicated bomb explosion
                        // clip (`BomberBomb.flc`, `FighterBomb.flc`, etc.) in VICTORY.
                        let drawn = assets.civ3.draw_unit(
                            mission.aircraft.kind,
                            mission.aircraft.owner,
                            palette_index,
                            mission.outbound_facing,
                            mission.aircraft.era_index,
                            Action::Victory,
                            t,
                            Some(mission.action_duration as f32),
                            to_center,
                            view.zoom * WORLD_ART_SCALE,
                            false,
                        );
                        if !drawn {
                            let _ = assets.civ3.draw_bomb_effect(
                                mission.outbound_facing,
                                t,
                                to_center,
                                view.zoom,
                            );
                        }
                    }
                    crate::sim::AirMissionPresentationKind::CruiseMissileStrike { .. } => {
                        let palette_index =
                            self.sim.player_team_palette_view(mission.aircraft.owner);
                        let drawn = assets.civ3.draw_unit(
                            mission.aircraft.kind,
                            mission.aircraft.owner,
                            palette_index,
                            0,
                            mission.aircraft.era_index,
                            Action::Death,
                            t,
                            Some(mission.action_duration as f32),
                            to_center,
                            view.zoom * WORLD_ART_SCALE,
                            false,
                        );
                        if !drawn {
                            let _ = assets.civ3.draw_bomb_effect(
                                mission.outbound_facing,
                                t,
                                to_center,
                                view.zoom,
                            );
                        }
                        return;
                    }
                    crate::sim::AirMissionPresentationKind::SamIntercept { .. } => {
                        let drawn = assets.civ3.draw_sam_effect(
                            mission.outbound_facing,
                            t,
                            to_center,
                            view.zoom,
                        );
                        if !drawn {
                            let tail = to_center + vec2(0.0, 22.0 * view.zoom);
                            let tip = to_center + vec2(0.0, (22.0 - 46.0 * t) * view.zoom);
                            draw_line(
                                tail.x,
                                tail.y,
                                tip.x,
                                tip.y,
                                (2.5 * view.zoom).max(1.5),
                                Color::from_rgba(255, 240, 180, 235),
                            );
                        }
                    }
                    _ => {}
                }
            }

            // 3. Draw the aircraft flying its outbound / over-target / crash / return path.
            let mut aircraft = mission.aircraft.clone();
            let (plane_center, plane_action, plane_progress, plane_duration) = match phase {
                AirMissionPhase::Outbound(t) => {
                    aircraft.facing = mission.outbound_facing;
                    (
                        from_center + flight_vector * t,
                        Action::Run,
                        t,
                        mission.fly_duration as f32,
                    )
                }
                AirMissionPhase::Action(t) => {
                    aircraft.facing = mission.outbound_facing;
                    let drift = flight_vector.normalize_or_zero() * ((t - 0.5) * 18.0 * view.zoom);
                    (
                        to_center + drift,
                        Action::Run,
                        t,
                        mission.action_duration as f32,
                    )
                }
                AirMissionPhase::Crash(t) => {
                    aircraft.facing = mission.outbound_facing;
                    aircraft.hp = 0;
                    (to_center, Action::Death, t, mission.outcome_duration as f32)
                }
                AirMissionPhase::Return(t) => {
                    aircraft.facing = mission.return_facing;
                    (
                        to_center - flight_vector * t,
                        Action::Run,
                        t,
                        mission.fly_duration as f32,
                    )
                }
            };
            self.draw_unit_at_zoom(
                assets,
                &aircraft,
                plane_center,
                1,
                plane_action,
                plane_progress,
                Some(plane_duration.max(0.05)),
                view.zoom,
            );
            return;
        }
        for (unit, phase) in self.playback.active() {
            let center = match event {
                PresentationEventView::Move { from, to, .. } => {
                    let from_center = project(from.0, from.1);
                    let to_center = project(to.0, to.1);
                    let period = self.sim.map_wrap().width() as f32 * TILE_W * view.zoom;
                    let dx = map_view::shortest_wrapped_delta(to_center.x - from_center.x, period);
                    let dy = to_center.y - from_center.y;
                    from_center + vec2(dx, dy) * phase.x
                }
                _ => project(unit.x, unit.y),
            };
            use super::tileset::Civ3UnitAction as Action;
            let mut clip_progress = progress;
            let mut clip_duration = event.duration() as f32;
            let action = match event {
                PresentationEventView::Move { .. } => Action::Run,
                PresentationEventView::Capture { .. } => Action::Capture,
                PresentationEventView::FoundCity { .. } => Action::Build,
                PresentationEventView::Combat(combat) => {
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
                    let (round, local_progress, local_duration, result) =
                        combat_phase(combat, f64::from(progress) * event.duration());
                    clip_progress = local_progress;
                    clip_duration = local_duration;
                    if result && unit_hp <= 0 {
                        Action::Death
                    } else if result && opponent_hp <= 0 && combat.battle_ended {
                        Action::Victory
                    } else if result {
                        Action::Default
                    } else if is_attacker {
                        attack_action(round)
                    } else {
                        Action::Defend
                    }
                }
                PresentationEventView::WorkStarted { .. }
                | PresentationEventView::Fortify { .. }
                | PresentationEventView::EnterHut { .. }
                | PresentationEventView::AirMission(_) => Action::Default,
            };
            self.draw_unit_at_zoom(
                assets,
                &unit,
                center,
                1,
                action,
                clip_progress,
                Some(clip_duration),
                view.zoom,
            );
            if matches!(event, PresentationEventView::Combat(_)) {
                self.draw_unit_health_at_zoom(assets, &unit, center, view.zoom);
            }
        }
        if let PresentationEventView::Combat(combat) = event
            && combat.bombard_effects
        {
            let impact_duration = combat_impact_duration(combat);
            let elapsed = self.playback.progress() as f64 * event.duration();
            let explosion_start = event.duration() - impact_duration;
            if elapsed >= explosion_start {
                let center = project(combat.defender.x, combat.defender.y);
                let effect_progress = ((elapsed - explosion_start) / impact_duration.max(0.001))
                    .clamp(0.0, 1.0) as f32;
                if !assets.civ3.draw_bomb_effect(
                    combat.attacker.facing,
                    effect_progress,
                    center,
                    view.zoom,
                ) {
                    let frame = (((elapsed - explosion_start) / 0.020) as i32).min(4);
                    let radius = (7.0 + frame as f32 * 3.0) * view.zoom;
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
                        (2.0 * view.zoom).max(1.0),
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
    use crate::sim::{Contamination, PillageTarget, Simulation};

    fn sound_requests_for(
        sim: &Simulation,
        event: VisualEvent,
        active_unit: Option<u32>,
    ) -> Vec<super::super::unit_sound::UnitSoundRequest> {
        let event = sim.presentation_event_view_for_test(event);
        sound_requests(&event, active_unit)
    }

    #[test]
    fn playback_is_clock_driven_and_drops_all_overrides_at_completion() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (id, target) = crate::sim::ui_review::ui_review_animation_setup(&mut sim, false);
        let before = sim.unit_by_id(id).unwrap().clone();
        assert!(sim.apply(Command::move_by_offset(&sim, id, 1, 0)));
        let mut playback = Playback::default();
        playback.update(sim.take_presentation_event_views(), 10.0, Some(id));
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
        playback.update(sim.take_presentation_event_views(), 10.015, Some(id));
        assert!(playback.is_active(id));
        playback.update(sim.take_presentation_event_views(), 10.031, Some(id));
        assert!(!playback.pending());
        assert!(playback.overrides().is_empty());
        assert!(playback.city_overrides().is_empty());
    }

    #[test]
    fn playback_overrides_follow_queue_append_completion_and_finish() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (id, target) = crate::sim::ui_review::ui_review_animation_setup(&mut sim, true);
        let attacker = sim.unit_by_id(id).unwrap().clone();
        let defender = sim
            .units()
            .iter()
            .find(|unit| unit.owner != HUMAN)
            .unwrap()
            .clone();
        let city = sim
            .cities()
            .iter()
            .find(|city| (city.x, city.y) == target)
            .unwrap()
            .clone();
        let city_id = city.id;
        let mut first =
            sim.presentation_event_view_for_test(VisualEvent::Combat(Box::new(CombatVisual {
                attacker,
                defender,
                hp: vec![(3, 3), (2, 0)],
                city: Some(city),
                bombard_effects: false,
                battle_ended: true,
            })));
        let PresentationEventView::Combat(combat) = &mut first else {
            unreachable!()
        };
        combat.duration = 1.0;
        let initial_size = combat.city.as_ref().unwrap().size;
        let mut second = first.clone();
        let PresentationEventView::Combat(combat) = &mut second else {
            unreachable!()
        };
        combat.city.as_mut().unwrap().size += 1;
        let mut playback = Playback::default();
        playback.update(VecDeque::from([first]), 0.0, None);
        playback.update(VecDeque::from([second]), 0.2, None);
        assert_eq!(playback.city_overrides()[&city_id].size, initial_size);
        assert_eq!(playback.overrides().len(), 2);
        playback.update(VecDeque::new(), 1.1, None);
        assert_eq!(playback.city_overrides()[&city_id].size, initial_size + 1);
        playback.finish();
        assert!(playback.overrides().is_empty());
        assert!(playback.city_overrides().is_empty());
    }

    #[test]
    fn playback_overrides_preserve_first_retained_snapshot_when_queue_is_capped() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (id, _) = crate::sim::ui_review::ui_review_animation_setup(&mut sim, false);
        let unit = sim.rendered_unit_override_view(sim.unit_by_id(id).unwrap());
        let event = |x| {
            let mut unit = unit.clone();
            unit.x = x;
            PresentationEventView::Move {
                unit,
                from: (0, 24),
                to: (1, 24),
                duration: 1.0,
            }
        };
        let mut playback = Playback {
            review_progress: Some(0.5),
            ..Default::default()
        };
        playback.update((0..300).map(event).collect(), 0.0, None);
        assert_eq!(playback.events.len(), 256);
        assert_eq!(playback.overrides()[&id].x, 44);
        playback.update(VecDeque::from([event(300)]), 0.1, None);
        assert_eq!(playback.overrides()[&id].x, 45);
        playback.review_progress = None;
        playback.update(VecDeque::new(), 1.1, None);
        assert_eq!(playback.overrides()[&id].x, 46);
    }

    #[test]
    fn movement_sound_request_is_emitted_once_when_the_event_starts() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (id, _) = crate::sim::ui_review::ui_review_animation_setup(&mut sim, false);
        assert!(sim.apply(Command::move_by_offset(&sim, id, 1, 0)));
        let mut playback = Playback::default();
        playback.update(sim.take_presentation_event_views(), 10.0, Some(id));
        let requests = playback.take_sound_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].unit_id, id);
        assert_eq!(requests[0].action, "RUN");
        playback.update(sim.take_presentation_event_views(), 10.01, Some(id));
        assert!(playback.take_sound_requests().is_empty());
    }

    #[test]
    fn combat_rounds_complete_before_damage_and_result_clips_start() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (id, _) = crate::sim::ui_review::ui_review_animation_setup(&mut sim, true);
        let attacker = sim.unit_by_id(id).unwrap().clone();
        let defender = sim
            .units()
            .iter()
            .find(|u| u.owner != HUMAN)
            .unwrap()
            .clone();
        let event = VisualEvent::Combat(Box::new(CombatVisual {
            attacker,
            defender,
            hp: vec![(3, 3), (2, 3), (2, 0)],
            city: None,
            bombard_effects: false,
            battle_ended: true,
        }));
        let PresentationEventView::Combat(mut combat) = sim.presentation_event_view_for_test(event)
        else {
            unreachable!()
        };
        // Ordinary lethal combat ends with the unit's own death clip.
        assert_eq!(combat_impact_duration(&combat), 0.0);
        combat.bombard_effects = true;
        assert_eq!(combat_impact_duration(&combat), 0.1);
        combat.bombard_effects = false;
        combat.round_durations = vec![0.5, 0.75];
        combat.result_duration = 0.8;
        combat.duration = 2.15;
        assert_eq!(combat_phase(&combat, 0.0), (0, 0.0, 0.5, false));
        let (hp_index, progress, _, result) = combat_phase(&combat, 0.49);
        assert_eq!(hp_index, 0);
        assert!(progress > 0.97 && !result);
        assert_eq!(combat_phase(&combat, 0.5), (1, 0.0, 0.75, false));
        assert_eq!(combat_phase(&combat, 1.25), (2, 0.0, 0.8, true));
        assert_eq!(combat_phase(&combat, 2.10), (2, 1.0, 0.8, true));
        let requests = sound_requests(&PresentationEventView::Combat(combat), Some(id));
        assert_eq!(
            requests.iter().map(|r| r.delay_seconds).collect::<Vec<_>>(),
            vec![0.0, 0.5, 1.25]
        );
    }

    #[test]
    fn combat_sounds_follow_each_attack_and_terminal_result() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (attacker_id, _) = crate::sim::ui_review::ui_review_animation_setup(&mut sim, true);
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
            battle_ended: true,
        }));
        let requests = sound_requests_for(&sim, event, Some(attacker_id));
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
        let (id, _) = crate::sim::ui_review::ui_review_animation_setup(&mut sim, false);
        let unit = sim.unit_by_id(id).unwrap().clone();
        let requests = sound_requests_for(
            &sim,
            VisualEvent::WorkStarted {
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
        let (id, _) = crate::sim::ui_review::ui_review_animation_setup(&mut sim, false);
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
            (VisualEvent::EnterHut { unit: unit.clone() }, "HUT"),
        ] {
            let requests = sound_requests_for(&sim, event, Some(unit.id));
            assert_eq!(requests.len(), 1, "{expected_action} request count");
            assert_eq!(requests[0].unit_id, unit.id);
            assert_eq!(requests[0].action, expected_action);
            assert_eq!(requests[0].delay_seconds, 0.0);
        }
        let unselected_hut =
            sound_requests_for(&sim, VisualEvent::EnterHut { unit: unit.clone() }, None);
        assert_eq!(unselected_hut.len(), 1);
        assert_eq!(unselected_hut[0].action, "HUT");
    }

    #[test]
    fn worker_orders_map_to_their_ini_sound_actions() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (id, _) = crate::sim::ui_review::ui_review_animation_setup(&mut sim, false);
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
            let requests = sound_requests_for(
                &sim,
                VisualEvent::WorkStarted {
                    unit: unit.clone(),
                    kind,
                },
                Some(id),
            );
            match expected_action {
                Some(action) => {
                    assert_eq!(requests.len(), 1, "{kind:?} request count");
                    assert_eq!(requests[0].unit_id, unit.id);
                    assert_eq!(requests[0].action, action, "{kind:?}");
                }
                None => assert!(requests.is_empty(), "{kind:?} should stay silent"),
            }
        }
    }

    #[test]
    fn combat_sound_uses_death_and_victory_for_either_loser() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (attacker_id, _) = crate::sim::ui_review::ui_review_animation_setup(&mut sim, true);
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
                battle_ended: true,
            }));
            let requests = sound_requests_for(&sim, event, Some(attacker.id));
            assert_eq!(requests.len(), 2);
            assert_eq!(requests[0].action, "ATTACK1");
            assert_eq!(
                requests[1].action,
                if winner_id == attacker.id {
                    "VICTORY"
                } else {
                    "DEATH"
                }
            );
            assert_eq!(requests[1].unit_id, attacker.id);
            assert_ne!(loser_id, winner_id);
        }
    }
    #[test]
    fn army_playback_switches_member_art_health_and_sounds_and_delays_victory_until_the_battle_ends()
     {
        for attacking in [true, false] {
            let mut sim = Simulation::new(crate::mapgen::generate(42));
            let (actor, _, army) =
                crate::sim::ui_review::ui_review_army_combat_setup(&mut sim, attacking);
            assert!(sim.apply(Command::move_by_offset(&sim, actor, 1, 0)));
            let mut playback = Playback {
                review_progress: Some(0.0),
                ..Default::default()
            };
            playback.update(sim.take_presentation_event_views(), 0.0, Some(army));
            let expected = if attacking {
                [UnitKind::Swordsman, UnitKind::Horseman, UnitKind::Spearman]
            } else {
                [UnitKind::Swordsman, UnitKind::Spearman, UnitKind::Horseman]
            };
            for (index, kind) in expected.into_iter().enumerate() {
                if index > 0 {
                    playback.events.pop_front();
                    playback.rebuild_overrides();
                }
                assert!(playback.is_active(army));
                assert_eq!(playback.overrides()[&army].kind, kind);
                let active = playback.active();
                assert_eq!(active.len(), 2);
                let member = &active[usize::from(!attacking)].0;
                assert_eq!(member.kind, kind);
                assert_eq!(member.hp, 1);
                let event = playback.events.front().unwrap();
                let sounds = sound_requests(event, Some(army));
                assert!(sounds.iter().all(|sound| sound.kind == kind));
                assert!(
                    sounds
                        .iter()
                        .any(|sound| sound.action == if attacking { "ATTACK1" } else { "DEFEND" })
                );
                assert!(sounds.iter().any(|sound| sound.action == "DEATH"));
                // The opponent has not won while the next member can still fight.
                let opponent = if attacking {
                    if let PresentationEventView::Combat(c) = event {
                        c.defender.id
                    } else {
                        unreachable!()
                    }
                } else {
                    actor
                };
                assert_eq!(
                    sound_requests(event, Some(opponent))
                        .iter()
                        .any(|s| s.action == "VICTORY"),
                    index == 2
                );
            }
            playback.finish();
            assert!(playback.overrides().is_empty());
        }
    }

    #[test]
    fn air_mission_playback_schedules_authentic_civ3_sounds_and_phases() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let (bomber, (tx, ty)) = crate::sim::ui_review::ui_review_air_combat_setup(&mut sim, false);
        let bomber_unit = sim.unit_by_id(bomber).unwrap().clone();
        let (sx, sy) = (bomber_unit.x, bomber_unit.y);
        let defender_unit = sim
            .units()
            .iter()
            .find(|u| u.owner == 1 && (u.x, u.y) == (tx, ty))
            .unwrap()
            .clone();
        let defender = defender_unit.id;
        let flak = 103;
        let mut flak_unit = defender_unit.clone();
        flak_unit.id = flak;
        flak_unit.kind = UnitKind::MobileSAM;
        let artillery = 104;
        let mut artillery_unit = bomber_unit.clone();
        artillery_unit.id = artillery;
        artillery_unit.kind = UnitKind::Artillery;

        // 1. Bombing run killing a defender schedules RUN -> VICTORY (*Bomb.wav) -> defender DEATH.
        let bombard = VisualEvent::AirMission(Box::new(crate::sim::AirMissionVisual {
            aircraft: bomber_unit.clone(),
            from: (sx, sy),
            to: (tx, ty),
            kind: crate::sim::AirMissionVisualKind::Bombard {
                defender: Some((defender_unit.clone(), 3, 0)),
                city: None,
            },
        }));
        let bombard_view = sim.presentation_event_view_for_test(bombard);
        let PresentationEventView::AirMission(mission) = &bombard_view else {
            unreachable!()
        };
        assert_eq!(mission.outbound_facing, 3); // East
        assert_eq!(mission.return_facing, 7); // West
        assert!(matches!(
            air_mission_phase(mission, 0.05),
            AirMissionPhase::Outbound(_)
        ));
        assert!(matches!(
            air_mission_phase(mission, mission.fly_duration + 0.1),
            AirMissionPhase::Action(_)
        ));
        assert!(matches!(
            air_mission_phase(
                mission,
                mission.fly_duration + mission.action_duration + 0.05
            ),
            AirMissionPhase::Return(_)
        ));
        let sounds = sound_requests(&bombard_view, Some(bomber));
        assert_eq!(
            sounds
                .iter()
                .map(|r| (r.unit_id, r.action.as_str()))
                .collect::<Vec<_>>(),
            vec![(bomber, "RUN"), (bomber, "VICTORY"), (defender, "DEATH")]
        );
        assert!(sounds[1].delay_seconds > sounds[0].delay_seconds);
        assert!(sounds[2].delay_seconds > sounds[1].delay_seconds);

        // 2. City SAM Battery interception schedules RUN -> SAM_EFFECT -> aircraft DEATH when shot down.
        let sam = VisualEvent::AirMission(Box::new(crate::sim::AirMissionVisual {
            aircraft: bomber_unit.clone(),
            from: (sx, sy),
            to: (tx, ty),
            kind: crate::sim::AirMissionVisualKind::SamIntercept { shot_down: true },
        }));
        let sam_view = sim.presentation_event_view_for_test(sam);
        let PresentationEventView::AirMission(sam_mission) = &sam_view else {
            unreachable!()
        };
        assert!(matches!(
            air_mission_phase(
                sam_mission,
                sam_mission.fly_duration + sam_mission.action_duration + 0.05
            ),
            AirMissionPhase::Crash(_)
        ));
        let sam_sounds = sound_requests(&sam_view, Some(bomber));
        assert_eq!(
            sam_sounds
                .iter()
                .map(|r| r.action.as_str())
                .collect::<Vec<_>>(),
            vec!["RUN", "SAM_EFFECT", "DEATH"]
        );

        // 3. Tile AA unit (Mobile SAM / Flak) shoots down bomber: RUN -> AA ATTACK1 -> bomber DEATH.
        let aa = VisualEvent::AirMission(Box::new(crate::sim::AirMissionVisual {
            aircraft: bomber_unit.clone(),
            from: (sx, sy),
            to: (tx, ty),
            kind: crate::sim::AirMissionVisualKind::AaUnitIntercept {
                aa_unit: flak_unit,
                shot_down: true,
            },
        }));
        let aa_sounds = sound_requests_for(&sim, aa, Some(bomber));
        assert_eq!(
            aa_sounds
                .iter()
                .map(|r| (r.unit_id, r.kind, r.action.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (bomber, UnitKind::Bomber, "RUN"),
                (flak, UnitKind::MobileSAM, "ATTACK1"),
                (bomber, UnitKind::Bomber, "DEATH"),
            ]
        );

        // 4. Recon & Rebase schedule RUN with smooth flight duration.
        let recon_view = sim.presentation_event_view_for_test(VisualEvent::AirMission(Box::new(
            crate::sim::AirMissionVisual {
                aircraft: bomber_unit.clone(),
                from: (sx, sy),
                to: (tx, ty),
                kind: crate::sim::AirMissionVisualKind::Recon,
            },
        )));
        assert!(recon_view.duration() >= 0.5);
        assert_eq!(
            sound_requests(&recon_view, Some(bomber))
                .iter()
                .map(|r| r.action.as_str())
                .collect::<Vec<_>>(),
            vec!["RUN"]
        );

        // 5. Non-aircraft artillery bombardment schedules BOMB_EFFECT alongside ATTACK1.
        let art_sounds = sound_requests_for(
            &sim,
            VisualEvent::Combat(Box::new(CombatVisual {
                attacker: artillery_unit,
                defender: defender_unit.clone(),
                hp: vec![(3, 3), (3, 2)],
                city: None,
                bombard_effects: true,
                battle_ended: true,
            })),
            Some(artillery),
        );
        assert_eq!(
            art_sounds
                .iter()
                .map(|r| r.action.as_str())
                .collect::<Vec<_>>(),
            vec!["ATTACK1", "BOMB_EFFECT"]
        );

        // 6. ICBM detonation schedules RUN (trail) -> DEATH (mushroom cloud);
        //    SDI interception schedules RUN -> SDI_EFFECT (Laser) -> VICTORY (mid-air blast).
        let mut icbm_unit = bomber_unit.clone();
        icbm_unit.id = 105;
        icbm_unit.kind = UnitKind::ICBM;
        let nuke_det = sim.presentation_event_view_for_test(VisualEvent::AirMission(Box::new(
            crate::sim::AirMissionVisual {
                aircraft: icbm_unit.clone(),
                from: (sx, sy),
                to: (tx, ty),
                kind: crate::sim::AirMissionVisualKind::NuclearStrike {
                    intercepted_by_sdi: false,
                },
            },
        )));
        let PresentationEventView::AirMission(nuke_mission) = &nuke_det else {
            unreachable!()
        };
        assert!(matches!(
            air_mission_phase(nuke_mission, 0.1),
            AirMissionPhase::Outbound(_)
        ));
        assert!(matches!(
            air_mission_phase(nuke_mission, nuke_mission.fly_duration + 0.1),
            AirMissionPhase::Action(_)
        ));
        assert!(matches!(
            air_mission_phase(
                nuke_mission,
                nuke_mission.fly_duration + nuke_mission.action_duration + 0.1
            ),
            AirMissionPhase::Crash(_)
        ));
        assert_eq!(
            sound_requests(&nuke_det, None)
                .iter()
                .map(|r| r.action.as_str())
                .collect::<Vec<_>>(),
            vec!["RUN", "DEATH"]
        );

        let nuke_sdi = sim.presentation_event_view_for_test(VisualEvent::AirMission(Box::new(
            crate::sim::AirMissionVisual {
                aircraft: icbm_unit,
                from: (sx, sy),
                to: (tx, ty),
                kind: crate::sim::AirMissionVisualKind::NuclearStrike {
                    intercepted_by_sdi: true,
                },
            },
        )));
        assert_eq!(
            sound_requests(&nuke_sdi, None)
                .iter()
                .map(|r| r.action.as_str())
                .collect::<Vec<_>>(),
            vec!["RUN", "SDI_EFFECT", "VICTORY"]
        );

        // 7. Cruise Missile strike schedules RUN -> DEATH (impact explosion) -> defender DEATH.
        let mut cruise_unit = bomber_unit;
        cruise_unit.id = 106;
        cruise_unit.kind = UnitKind::CruiseMissile;
        let cruise_sounds = sound_requests_for(
            &sim,
            VisualEvent::AirMission(Box::new(crate::sim::AirMissionVisual {
                aircraft: cruise_unit,
                from: (sx, sy),
                to: (tx, ty),
                kind: crate::sim::AirMissionVisualKind::CruiseMissileStrike {
                    defender: Some((defender_unit, 3, 0)),
                    city: None,
                },
            })),
            Some(106),
        );
        assert_eq!(
            cruise_sounds
                .iter()
                .map(|r| (r.unit_id, r.action.as_str()))
                .collect::<Vec<_>>(),
            vec![(106, "RUN"), (106, "DEATH"), (defender, "DEATH")]
        );
    }
}
