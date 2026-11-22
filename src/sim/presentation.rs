//! Optional, non-persistent render events. No RNG or gameplay decisions here.
use super::*;
use std::collections::VecDeque;

#[derive(Clone)]
pub(crate) struct CombatVisual {
    pub(crate) attacker: Unit,
    pub(crate) defender: Unit,
    pub(crate) hp: Vec<(i32, i32)>,
    /// False while an Army changes members within the same battle.
    pub(crate) battle_ended: bool,
    pub(crate) city: Option<City>,
    /// The Conquests PRTO Bombard Effects flag requests a target explosion
    /// effect even when bombardment does not kill the selected unit.
    pub(crate) bombard_effects: bool,
}

#[derive(Clone)]
pub(crate) enum AirMissionVisualKind {
    Bombard {
        /// Optional primary defending unit on the target tile with `(unit, hp_before, hp_after)`.
        defender: Option<(Unit, i32, i32)>,
        city: Option<Box<City>>,
    },
    Recon,
    Rebase,
    SamIntercept {
        shot_down: bool,
    },
    AaUnitIntercept {
        aa_unit: Unit,
        shot_down: bool,
    },
}

#[derive(Clone)]
pub(crate) struct AirMissionVisual {
    pub(crate) aircraft: Unit,
    pub(crate) from: (i32, i32),
    pub(crate) to: (i32, i32),
    pub(crate) kind: AirMissionVisualKind,
}

#[derive(Clone)]
pub(crate) enum VisualEvent {
    Move {
        unit: Unit,
        from: (i32, i32),
        to: (i32, i32),
    },
    /// Plays the unit INI's CAPTURE action, including when a missile launches.
    Capture {
        unit: Unit,
    },
    /// The Settler is consumed by founding, but its Build clip still plays.
    FoundCity {
        unit: Unit,
    },
    /// A successfully started worker action has a one-shot presentation sound.
    WorkStarted {
        unit: Unit,
        kind: WorkKind,
    },
    /// A unit successfully entered its fortifying action.
    Fortify {
        unit: Unit,
    },
    /// A unit entered a hut tile.
    EnterHut {
        unit: Unit,
    },
    Combat(Box<CombatVisual>),
    AirMission(Box<AirMissionVisual>),
}

impl AirMissionVisual {
    pub(crate) fn default_timings(&self) -> (f64, f64, f64) {
        let manhattan = (self.to.0 - self.from.0)
            .abs()
            .max((self.to.1 - self.from.1).abs());
        let fly = (0.22 + 0.03 * f64::from(manhattan)).clamp(0.22, 0.50);
        match &self.kind {
            AirMissionVisualKind::Bombard { .. } => (fly, 0.65, 0.0),
            AirMissionVisualKind::Recon => (fly, 0.10, 0.0),
            AirMissionVisualKind::Rebase => (
                (0.25 + 0.04 * f64::from(manhattan)).clamp(0.25, 0.70),
                0.0,
                0.0,
            ),
            AirMissionVisualKind::SamIntercept { shot_down }
            | AirMissionVisualKind::AaUnitIntercept { shot_down, .. } => {
                (fly, 0.55, if *shot_down { 0.65 } else { 0.0 })
            }
        }
    }

    pub(crate) fn total_duration(
        kind: &AirMissionVisualKind,
        fly: f64,
        action: f64,
        outcome: f64,
    ) -> f64 {
        match kind {
            AirMissionVisualKind::Rebase => fly,
            AirMissionVisualKind::Bombard { .. } | AirMissionVisualKind::Recon => {
                fly + action + fly
            }
            AirMissionVisualKind::SamIntercept { shot_down }
            | AirMissionVisualKind::AaUnitIntercept { shot_down, .. } => {
                if *shot_down {
                    fly + action + outcome
                } else {
                    fly + action + fly
                }
            }
        }
    }
}

impl VisualEvent {
    pub(crate) fn duration(&self) -> f64 {
        match self {
            Self::Move { .. } => 0.030, // native smooth_move_unit_msec
            Self::WorkStarted { .. } | Self::Fortify { .. } | Self::EnterHut { .. } => 0.0,
            // The original Settler, Worker, ICBM, and Tactical Nuke INIs use
            // a 0.5 second CAPTURE action; for nuclear units this is launch.
            Self::Capture { .. } => 0.5,
            // The Settler Build clip's source timing is 0.5 seconds. The
            // presentation pacing config stretches the complete clip to 1s.
            Self::FoundCity { .. } => 1.0,
            Self::Combat(combat) => {
                combat.hp.len().saturating_sub(1).max(1) as f64 * 0.5
                    + 0.5
                    + if combat.bombard_effects { 0.100 } else { 0.0 }
            }
            Self::AirMission(mission) => {
                let (fly, action, outcome) = mission.default_timings();
                AirMissionVisual::total_duration(&mission.kind, fly, action, outcome)
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

    pub(crate) fn take_presentation_event_views(&mut self) -> VecDeque<PresentationEventView> {
        std::mem::take(&mut self.visual_events.queue)
            .into_iter()
            .map(|event| self.presentation_event_view(event))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn presentation_event_view_for_test(
        &self,
        event: VisualEvent,
    ) -> PresentationEventView {
        self.presentation_event_view(event)
    }

    fn presentation_event_view(&self, event: VisualEvent) -> PresentationEventView {
        let duration = event.duration();
        match event {
            VisualEvent::Move { unit, from, to } => PresentationEventView::Move {
                unit: self.rendered_unit_override_view(&unit),
                from,
                to,
                duration,
            },
            VisualEvent::Capture { unit } => PresentationEventView::Capture {
                unit: self.rendered_unit_override_view(&unit),
                duration,
            },
            VisualEvent::FoundCity { unit } => PresentationEventView::FoundCity {
                unit: self.rendered_unit_override_view(&unit),
                duration,
            },
            VisualEvent::WorkStarted { unit, kind } => PresentationEventView::WorkStarted {
                unit: self.rendered_unit_override_view(&unit),
                kind,
                duration,
            },
            VisualEvent::Fortify { unit } => PresentationEventView::Fortify {
                unit: self.rendered_unit_override_view(&unit),
                duration,
            },
            VisualEvent::EnterHut { unit } => PresentationEventView::EnterHut {
                unit: self.rendered_unit_override_view(&unit),
                duration,
            },
            VisualEvent::Combat(combat) => {
                let mut attacker = self.rendered_unit_override_view(&combat.attacker);
                let mut defender = self.rendered_unit_override_view(&combat.defender);
                // Intentional Civ III-style presentation: face combatants
                // toward each other. Freeciv keeps their stored facings.
                // FLC rows use SW, S, SE, E, NE, N, NW, W order. Combatants
                // should face one another regardless of their last move.
                if let Some(direction) = (0..super::topology::CIV3_DIRS.len()).find(|&direction| {
                    self.map_wrap
                        .civ3_neighbor(attacker.x, attacker.y, direction)
                        == Some((defender.x, defender.y))
                }) && let Some(facing) = self.map_wrap.civ3_flic_direction_index(direction)
                {
                    attacker.facing = facing;
                    defender.facing = (facing + 4) % 8;
                }
                let round_durations = vec![0.5; combat.hp.len().saturating_sub(1).max(1)];
                PresentationEventView::Combat(Box::new(CombatPresentationView {
                    attacker,
                    defender,
                    hp: combat.hp,
                    city: combat
                        .city
                        .as_ref()
                        .map(|city| self.rendered_city_override_view(city)),
                    bombard_effects: combat.bombard_effects,
                    battle_ended: combat.battle_ended,
                    round_durations,
                    result_duration: 0.5,
                    impact_duration: if combat.bombard_effects { 0.100 } else { 0.0 },
                    duration,
                }))
            }
            VisualEvent::AirMission(mission) => {
                let (fly_duration, action_duration, outcome_duration) = mission.default_timings();
                let outbound_facing = self
                    .civ3_facing_between(mission.from, mission.to)
                    .unwrap_or(mission.aircraft.facing);
                let return_facing = (outbound_facing + 4) % 8;
                let mut aircraft = self.rendered_unit_override_view(&mission.aircraft);
                aircraft.facing = outbound_facing;
                let kind = match mission.kind {
                    AirMissionVisualKind::Bombard { defender, city } => {
                        let defender = defender.map(|(unit, hp_before, hp_after)| {
                            let mut view = self.rendered_unit_override_view(&unit);
                            view.hp = hp_before;
                            (view, hp_before, hp_after)
                        });
                        AirMissionPresentationKind::Bombard {
                            defender,
                            city: city
                                .as_deref()
                                .map(|city| self.rendered_city_override_view(city)),
                        }
                    }
                    AirMissionVisualKind::Recon => AirMissionPresentationKind::Recon,
                    AirMissionVisualKind::Rebase => AirMissionPresentationKind::Rebase,
                    AirMissionVisualKind::SamIntercept { shot_down } => {
                        AirMissionPresentationKind::SamIntercept { shot_down }
                    }
                    AirMissionVisualKind::AaUnitIntercept { aa_unit, shot_down } => {
                        let mut view = self.rendered_unit_override_view(&aa_unit);
                        view.facing = return_facing;
                        AirMissionPresentationKind::AaUnitIntercept {
                            aa_unit: view,
                            shot_down,
                        }
                    }
                };
                PresentationEventView::AirMission(Box::new(AirMissionPresentationView {
                    aircraft,
                    from: mission.from,
                    to: mission.to,
                    outbound_facing,
                    return_facing,
                    kind,
                    fly_duration,
                    action_duration,
                    outcome_duration,
                    duration,
                }))
            }
        }
    }

    pub(super) fn civ3_facing_between(&self, from: (i32, i32), to: (i32, i32)) -> Option<u8> {
        if from == to {
            return None;
        }
        let span = self.map_wrap.width() * 2;
        let half = self.map_wrap.width();
        let sx = 2 * from.0 + from.1.rem_euclid(2);
        let tx = 2 * to.0 + to.1.rem_euclid(2);
        let dx = if span > 0 {
            (tx - sx + half).rem_euclid(span) - half
        } else {
            tx - sx
        };
        let dy = to.1 - from.1;
        if dx == 0 && dy == 0 {
            return None;
        }
        let octant =
            (f64::from(dy).atan2(f64::from(dx)) / std::f64::consts::FRAC_PI_4).round() as i32;
        Some(((3 - octant).rem_euclid(8)) as u8)
    }

    /// Reposition the most recently queued combat visual (used when a CAP
    /// fighter intercepts an incoming aircraft over a contested target tile).
    pub(super) fn relocate_last_combat_visual(
        &mut self,
        attacker_id: u32,
        defender_id: u32,
        attacker_pos: (i32, i32),
        defender_pos: (i32, i32),
    ) {
        if let Some(VisualEvent::Combat(combat)) = self.visual_events.queue.back_mut()
            && combat.attacker.id == attacker_id
            && combat.defender.id == defender_id
        {
            (combat.attacker.x, combat.attacker.y) = attacker_pos;
            (combat.defender.x, combat.defender.y) = defender_pos;
        }
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
            VisualEvent::Capture { unit } | VisualEvent::FoundCity { unit } => {
                unit.owner == HUMAN || self.unit_visible_to_human(unit)
            }
            VisualEvent::WorkStarted { unit, .. }
            | VisualEvent::Fortify { unit }
            | VisualEvent::EnterHut { unit } => {
                unit.owner == HUMAN || self.unit_visible_to_human(unit)
            }
            VisualEvent::Combat(combat) => {
                let attacker = &combat.attacker;
                let defender = &combat.defender;
                attacker.owner == HUMAN
                    || defender.owner == HUMAN
                    || (self.unit_visible_to_human(attacker)
                        && self.unit_visible_to_human(defender))
            }
            VisualEvent::AirMission(mission) => {
                let target_visible = self.tile(mission.to.0, mission.to.1).visible;
                let origin_visible = self.tile(mission.from.0, mission.from.1).visible;
                let human_involved = mission.aircraft.owner == HUMAN
                    || self
                        .city_at(mission.to.0, mission.to.1)
                        .is_some_and(|city| city.owner == HUMAN)
                    || match &mission.kind {
                        AirMissionVisualKind::Bombard {
                            defender: Some((unit, _, _)),
                            ..
                        } => unit.owner == HUMAN,
                        AirMissionVisualKind::AaUnitIntercept { aa_unit, .. } => {
                            aa_unit.owner == HUMAN
                        }
                        _ => false,
                    };
                human_involved || target_visible || origin_visible
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

    fn combat_view_for(sim: &Simulation, attacker: Unit, defender: Unit) -> CombatPresentationView {
        let event = VisualEvent::Combat(Box::new(CombatVisual {
            attacker,
            defender,
            hp: vec![(10, 10)],
            city: None,
            bombard_effects: false,
            battle_ended: true,
        }));
        let PresentationEventView::Combat(combat) = sim.presentation_event_view_for_test(event)
        else {
            unreachable!("combat event remains a combat presentation")
        };
        *combat
    }

    #[test]
    fn combat_presentation_faces_adjacent_units_toward_each_other_in_all_directions() {
        let sim = Simulation::new(crate::mapgen::generate(42));
        let attacker_at = (10, 24);

        for direction in 0..super::super::topology::CIV3_DIRS.len() {
            let defender_at = sim
                .map_wrap
                .civ3_neighbor(attacker_at.0, attacker_at.1, direction)
                .expect("all directions are available away from map edges");
            let mut attacker = Unit::new(1, HUMAN, UnitKind::Warrior, attacker_at.0, attacker_at.1);
            let mut defender = Unit::new(2, 1, UnitKind::Warrior, defender_at.0, defender_at.1);
            attacker.facing = 7;
            defender.facing = 3;

            let combat = combat_view_for(&sim, attacker, defender);
            let expected_attacker_facing =
                sim.map_wrap.civ3_flic_direction_index(direction).unwrap();
            assert_eq!(combat.attacker.facing, expected_attacker_facing);
            assert_eq!(combat.defender.facing, (expected_attacker_facing + 4) % 8);
        }
    }

    #[test]
    fn combat_presentation_preserves_facing_for_nonadjacent_units() {
        let sim = Simulation::new(crate::mapgen::generate(42));
        let mut attacker = Unit::new(1, HUMAN, UnitKind::Warrior, 10, 24);
        let mut defender = Unit::new(2, 1, UnitKind::Warrior, 10, 30);
        attacker.facing = 7;
        defender.facing = 3;

        let combat = combat_view_for(&sim, attacker, defender);

        assert_eq!(combat.attacker.facing, 7);
        assert_eq!(combat.defender.facing, 3);
    }

    #[test]
    fn observer_records_real_results_without_changing_rng_or_saved_campaign() {
        for combat in [false, true] {
            let mut observed = Simulation::new(crate::mapgen::generate(42));
            let (id, _) = crate::sim::ui_review::ui_review_animation_setup(&mut observed, combat);
            let mut unobserved = observed.clone();
            unobserved.visual_events = VisualEvents::default();
            assert_eq!(
                observed.apply(Command::move_civ3_by_offset(&observed, id, 1, 0)),
                unobserved.apply(Command::move_civ3_by_offset(&unobserved, id, 1, 0))
            );
            assert_eq!(
                observed.encode_campaign().unwrap(),
                unobserved.encode_campaign().unwrap()
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
                let VisualEvent::Combat(combat) = &events[0] else {
                    panic!("combat event first")
                };
                let CombatVisual {
                    attacker,
                    defender,
                    hp,
                    ..
                } = combat.as_ref();
                assert_eq!(hp[0], (attacker.hp, defender.hp));
                let (a, d) = hp.last().copied().unwrap();
                assert!(a == 0 || d == 0);
                for (unit, final_hp) in [(attacker, a), (defender, d)] {
                    if let Some(survivor) = observed.units.iter().find(|u| u.id == unit.id) {
                        // Combat HP captures the round result before the
                        // source promotion check. A promotion adds exactly
                        // the new rank's HP to the surviving unit.
                        let rank_hp = survivor.max_hit_points() - unit.max_hit_points();
                        assert!(
                            rank_hp == 0 || (rank_hp == 1 && survivor.veteran == unit.veteran + 1)
                        );
                        assert_eq!(survivor.hp, final_hp + rank_hp);
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
    #[test]
    fn army_member_playback_preserves_selected_troops_health_and_battle_outcome_without_changing_rules()
     {
        for attacking in [true, false] {
            let mut observed = Simulation::new(crate::mapgen::generate(42));
            let (actor, _, army) =
                crate::sim::ui_review::ui_review_army_combat_setup(&mut observed, attacking);
            let mut unobserved = observed.clone();
            unobserved.visual_events = VisualEvents::default();
            assert!(observed.apply(Command::move_civ3_by_offset(&observed, actor, 1, 0)));
            assert!(unobserved.apply(Command::move_civ3_by_offset(&unobserved, actor, 1, 0)));
            assert_eq!(observed.native_random, unobserved.native_random);
            assert_eq!(
                observed.encode_campaign().unwrap(),
                unobserved.encode_campaign().unwrap()
            );
            let combats: Vec<_> = observed
                .take_presentation_event_views()
                .into_iter()
                .filter_map(|event| {
                    if let PresentationEventView::Combat(combat) = event {
                        Some(combat)
                    } else {
                        None
                    }
                })
                .collect();
            let expected = if attacking {
                [UnitKind::Swordsman, UnitKind::Horseman, UnitKind::Spearman]
            } else {
                [UnitKind::Swordsman, UnitKind::Spearman, UnitKind::Horseman]
            };
            assert_eq!(combats.len(), 3);
            for (index, combat) in combats.iter().enumerate() {
                let member = if attacking {
                    &combat.attacker
                } else {
                    &combat.defender
                };
                assert_eq!(member.id, army, "carrier remains the playback anchor");
                assert_eq!(member.kind, expected[index]);
                assert_eq!(member.hp, 1);
                assert_eq!(member.max_hp, expected[index].stats().hp);
                assert!(member.transport.is_none());
                assert_eq!(combat.battle_ended, index == 2);
                assert_eq!(combat.attacker.facing, (combat.defender.facing + 4) % 8);
                let hp = combat.hp.last().unwrap();
                assert_eq!(if attacking { hp.0 } else { hp.1 }, 0);
            }
            assert!(
                observed.unit_by_id(army).is_none(),
                "queued member snapshots survive the Army's removal"
            );
        }
    }

    #[test]
    fn civ3_facing_between_matches_all_eight_neighbor_directions() {
        let sim = Simulation::new(crate::mapgen::generate(42));
        let origin = (10, 24);
        for direction in 0..super::topology::CIV3_DIRS.len() {
            let target = sim
                .map_wrap
                .civ3_neighbor(origin.0, origin.1, direction)
                .unwrap();
            let expected = sim.map_wrap.civ3_flic_direction_index(direction).unwrap();
            assert_eq!(sim.civ3_facing_between(origin, target), Some(expected));
        }
    }

    #[test]
    fn real_air_missions_emit_visual_events_for_bombard_recon_rebase_sam_aa_and_cap() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.units.clear();
        sim.cities.clear();
        let base = (10, 20);
        let alt_base = (12, 20);
        let target = (13, 20);
        let road_tile = (11, 20);
        for (x, y) in [base, alt_base, target, road_tile] {
            sim.tile_mut(x, y).terrain = Terrain::Grassland;
            sim.tile_mut(x, y).visible = true;
            sim.tile_mut(x, y).seen = true;
        }
        sim.tile_mut(road_tile.0, road_tile.1).road = true;
        sim.create_city(HUMAN, base.0, base.1);
        sim.create_city(HUMAN, alt_base.0, alt_base.1);
        let _ = sim.create_city(1, target.0, target.1);
        let enemy_city = sim.city_at(target.0, target.1).unwrap().id;
        sim.sign_pact(HUMAN, 1, Pact::War);
        sim.enable_visual_events();

        // 1. Rebase emits AirMission::Rebase.
        let fighter = sim.spawn_unit(HUMAN, UnitKind::Fighter, base.0, base.1);
        assert!(sim.apply(Command::Rebase(fighter, alt_base.0, alt_base.1)));
        assert!(matches!(
            sim.take_visual_events().front(),
            Some(VisualEvent::AirMission(m))
                if matches!(m.kind, AirMissionVisualKind::Rebase)
                    && m.from == base
                    && m.to == alt_base
        ));

        // 2. Recon emits AirMission::Recon.
        let recon_fighter = sim.spawn_unit(HUMAN, UnitKind::Fighter, base.0, base.1);
        assert!(sim.apply(Command::Recon(recon_fighter, target.0, target.1)));
        assert!(matches!(
            sim.take_visual_events().front(),
            Some(VisualEvent::AirMission(m))
                if matches!(m.kind, AirMissionVisualKind::Recon)
                    && m.from == base
                    && m.to == target
        ));

        // 3. Bombard against terrain improvement (no defenders) emits AirMission::Bombard.
        let terrain_bomber = sim.spawn_unit(HUMAN, UnitKind::Bomber, base.0, base.1);
        assert!(sim.apply(Command::Bombard(terrain_bomber, road_tile.0, road_tile.1)));
        assert!(matches!(
            sim.take_visual_events().front(),
            Some(VisualEvent::AirMission(m))
                if matches!(
                    m.kind,
                    AirMissionVisualKind::Bombard {
                        defender: None,
                        city: None,
                    }
                )
        ));

        // 4. Bombard against empty enemy city (population/building) emits AirMission::Bombard.
        let city_bomber = sim.spawn_unit(HUMAN, UnitKind::Bomber, base.0, base.1);
        assert!(sim.apply(Command::Bombard(city_bomber, target.0, target.1)));
        assert!(matches!(
            sim.take_visual_events().front(),
            Some(VisualEvent::AirMission(m))
                if matches!(
                    m.kind,
                    AirMissionVisualKind::Bombard {
                        defender: None,
                        city: Some(_),
                    }
                )
        ));

        // 5. City SAM Battery interception emits AirMission::SamIntercept.
        sim.cities
            .iter_mut()
            .find(|c| c.id == enemy_city)
            .unwrap()
            .buildings
            .push(crate::rules::Building::SAM_BATTERY);
        let mut saw_sam = false;
        for seed in 0..40_u64 {
            sim.native_random = None;
            sim.rng = seed;
            let sam_bomber = sim.spawn_unit(HUMAN, UnitKind::Bomber, base.0, base.1);
            let _ = sim.apply(Command::Bombard(sam_bomber, target.0, target.1));
            if sim.take_visual_events().into_iter().any(|event| {
                matches!(
                    event,
                    VisualEvent::AirMission(m)
                        if matches!(m.kind, AirMissionVisualKind::SamIntercept { .. })
                )
            }) {
                saw_sam = true;
                break;
            }
        }
        assert!(
            saw_sam,
            "expected City SAM Battery to emit SamIntercept visual"
        );

        // 6. Tile AA unit (Mobile SAM) shoots down bomber and emits AirMission::AaUnitIntercept.
        sim.cities
            .iter_mut()
            .find(|c| c.id == enemy_city)
            .unwrap()
            .buildings
            .clear();
        let mobile_sam = sim.spawn_unit(1, UnitKind::MobileSAM, target.0, target.1);
        let mut saw_aa = false;
        for seed in 0..80_u64 {
            sim.native_random = None;
            sim.rng = seed;
            let aa_bomber = sim.spawn_unit(HUMAN, UnitKind::Bomber, base.0, base.1);
            let _ = sim.apply(Command::Bombard(aa_bomber, target.0, target.1));
            if sim.take_visual_events().into_iter().any(|event| {
                matches!(
                    event,
                    VisualEvent::AirMission(m)
                        if matches!(
                            &m.kind,
                            AirMissionVisualKind::AaUnitIntercept {
                                aa_unit,
                                shot_down: true,
                            } if aa_unit.id == mobile_sam
                        )
                )
            }) {
                saw_aa = true;
                break;
            }
        }
        assert!(saw_aa, "expected Mobile SAM to emit AaUnitIntercept visual");

        // 7. Air Superiority CAP dogfight relocates both aircraft to the contested target airspace.
        sim.units.retain(|u| u.id != mobile_sam);
        let cap_base = (15, 20);
        sim.tile_mut(cap_base.0, cap_base.1).terrain = Terrain::Grassland;
        sim.tile_mut(cap_base.0, cap_base.1).visible = true;
        sim.create_city(1, cap_base.0, cap_base.1);
        let cap_fighter = sim.spawn_unit(1, UnitKind::JetFighter, cap_base.0, cap_base.1);
        assert!(sim.start_air_superiority(cap_fighter));
        let mut saw_dogfight = false;
        for seed in 0..40_u64 {
            sim.native_random = None;
            sim.rng = seed;
            if let Some(u) = sim.units.iter_mut().find(|u| u.id == cap_fighter) {
                u.air_superiority_shot = false;
                u.hp = u.max_hit_points();
            }
            let incoming = sim.spawn_unit(HUMAN, UnitKind::Bomber, base.0, base.1);
            let _ = sim.apply(Command::Bombard(incoming, target.0, target.1));
            for event in sim.take_visual_events() {
                if let VisualEvent::Combat(combat) = event
                    && combat.attacker.id == cap_fighter
                    && combat.defender.id == incoming
                {
                    assert_eq!((combat.attacker.x, combat.attacker.y), target);
                    assert_eq!(
                        sim.map_wrap.civ3_distance(
                            combat.attacker.x,
                            combat.attacker.y,
                            combat.defender.x,
                            combat.defender.y,
                        ),
                        1,
                        "dogfight combatants should face each other adjacent over the target tile"
                    );
                    saw_dogfight = true;
                }
            }
            if saw_dogfight {
                break;
            }
        }
        assert!(
            saw_dogfight,
            "expected CAP interception dogfight over target"
        );
    }
}
