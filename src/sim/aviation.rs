//! Aircraft, fuel, airlift, and paradrop rules.

use super::normal_paths::{GotoTask, NormalPaths};
use super::*;

mod aircraft_ai;
mod paradrop;

enum CityBombardTarget {
    Unit(Unit),
    Population(u32),
    Building(u32, crate::rules::Building),
}

#[cfg(test)]
fn selected_unit_class_flag(kind: UnitKind, flag: &str) -> Option<bool> {
    crate::sim::action_rules::unit_class_flag(kind, flag)
}

/// Fighter-class air superiority per Civ III Conquests aircraft rules.
fn is_air_superiority_unit(kind: UnitKind) -> bool {
    matches!(
        kind,
        UnitKind::Fighter | UnitKind::JetFighter | UnitKind::F15 | UnitKind::StealthFighter
    )
}

fn bombardable_terrain_improvements(sim: &Simulation, x: i32, y: i32) -> Vec<PillageTarget> {
    if sim.city_at(x, y).is_some() || sim.any_unit_at(x, y, |_| true) {
        return Vec::new();
    }
    let tile = sim.tile(x, y);
    if tile.railroad {
        return vec![PillageTarget::Railroad];
    }

    let mut infrastructure = Vec::new();
    if tile.road {
        infrastructure.push(PillageTarget::Road);
    }
    if tile.mine {
        infrastructure.push(PillageTarget::Mine);
    }
    if tile.irrigation {
        infrastructure.push(PillageTarget::Irrigation);
    }
    if !infrastructure.is_empty() {
        return infrastructure;
    }

    let mut other = Vec::new();
    if tile.farmland {
        other.push(PillageTarget::Farmland);
    }
    if tile.oil_well {
        other.push(PillageTarget::OilWell);
    }
    if tile.oil_platform {
        other.push(PillageTarget::OilPlatform);
    }
    if let Some(base) = tile.base {
        other.push(PillageTarget::Base(base));
    }
    other
}

fn terrain_bombard_defense(sim: &Simulation, x: i32, y: i32) -> i32 {
    let tile = sim.tile(x, y);
    let terrain_bonus = match tile.terrain {
        Terrain::Forest | Terrain::Jungle => 25,
        Terrain::Hills => 50,
        Terrain::Mountains | Terrain::Volcano => 100,
        _ => 10,
    };
    16 + 16 * terrain_bonus / 100
        + if tile.base == Some(Base::Fortress) {
            16 * 25 / 100
        } else {
            0
        }
}

fn bombard_terrain_improvement(sim: &mut Simulation, x: i32, y: i32, target: PillageTarget) {
    let tile = sim.tile_mut(x, y);
    match target {
        PillageTarget::Road => tile.road = false,
        PillageTarget::Railroad => tile.railroad = false,
        PillageTarget::Irrigation => tile.irrigation = false,
        PillageTarget::Farmland => tile.farmland = false,
        PillageTarget::Mine => tile.mine = false,
        PillageTarget::OilWell => tile.oil_well = false,
        PillageTarget::OilPlatform => tile.oil_platform = false,
        PillageTarget::Base(base) => tile.base = base.underlying_after_removal(),
    }
}

fn city_object_bombard_defense(sim: &Simulation, city: &City, attacker: &Unit) -> i32 {
    let mut defense = terrain_bombard_defense(sim, city.x, city.y)
        - if sim.tile(city.x, city.y).base == Some(Base::Fortress) {
            4
        } else {
            0
        };
    defense += if city.size >= 13 {
        16
    } else if city.size >= 7 || city.has_building(crate::rules::Building::CITY_WALLS) {
        8
    } else {
        0
    };
    if attacker.kind.is_naval() && city.has_building(crate::rules::Building::COASTAL_DEFENSE) {
        defense += 8;
    }
    defense
}

/// Actions that take precedence over a bombardment of the same tile.
const BOMBARD_BLOCKED_BY: &[&str] = &["Capture Units"];

impl Simulation {
    /// Whether the Conquests BIQ gives this unit usable bombard strength,
    /// rate, and a tile or aircraft operational range.
    ///
    /// Civ III Conquests bombard capability.
    pub(super) fn has_bombard_capability(&self, kind: UnitKind) -> bool {
        kind.bombard_strength() > 0
            && kind.bombard_rate() > 0
            && Self::fixed_wing_bombard_range(kind) > 0
    }

    pub(crate) fn aircraft_on_base(&self, unit: &Unit) -> bool {
        self.refuel_site(unit.owner, unit.x, unit.y) || self.aboard_refueling_transport(unit)
    }

    pub(crate) fn observed_air_raid_turn(&self, victim: usize, attacker: usize) -> Option<i32> {
        self.ai_air_raid_turn.get(&(victim, attacker)).copied()
    }

    pub(crate) fn city_bombed_turn(&self, city_id: u32) -> Option<i32> {
        self.ai_city_bombed_turn.get(&city_id).copied()
    }

    fn rebase_carrier_at(&self, unit: &Unit, x: i32, y: i32) -> Option<u32> {
        self.tile_units_ordered(x, y)
            .find(|ship| {
                self.allied(unit.owner, ship.owner)
                    && ship.kind.can_carry(unit.kind)
                    && self.ruleset_transport_pair_allowed_fail_closed(ship.kind, unit.kind)
                    && (unit.transport == Some(ship.id)
                        || self.cargo_count(ship.id) < self.unit_transport_capacity(ship))
            })
            .map(|ship| ship.id)
    }

    pub(super) fn aircraft_base_at(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !(0..self.map_wrap.width()).contains(&x) || !(0..self.map_wrap.height()).contains(&y) {
            return false;
        }
        if self.any_unit_at(x, y, |other| !self.allied(unit.owner, other.owner)) {
            return false;
        }
        if self
            .city_at_cached(x, y)
            .is_some_and(|city| self.allied(unit.owner, city.owner))
        {
            return true;
        }
        let tile = self.tile(x, y);
        if matches!(tile.base, Some(Base::Airstrip | Base::Airbase))
            && tile.known_by[unit.owner]
            && !tile
                .owner
                .is_some_and(|owner| owner != unit.owner && self.at_war(unit.owner, owner))
        {
            return true;
        }
        self.rebase_carrier_at(unit, x, y).is_some()
    }

    pub(crate) fn can_begin_rebase(&self, unit: &Unit) -> bool {
        unit.kind.is_fixed_wing_aircraft() && unit.moves_left > 0 && self.aircraft_on_base(unit)
    }

    pub(crate) fn can_rebase_to(&self, unit: &Unit, x: i32, y: i32) -> bool {
        self.can_begin_rebase(unit)
            && (x, y) != (unit.x, unit.y)
            && (0..self.map_wrap.width()).contains(&x)
            && (0..self.map_wrap.height()).contains(&y)
            && self.tile(x, y).known_by[unit.owner]
            && self.map_wrap.step_distance(unit.x, unit.y, x, y) <= unit.kind.rebase_range()
            && self.aircraft_base_at(unit, x, y)
    }

    pub(crate) fn rebaseable_bases_in_range(&self, id: u32) -> Vec<(i32, i32)> {
        let Some(unit) = self.unit_by_id(id) else {
            return Vec::new();
        };
        if !self.can_begin_rebase(unit) {
            return Vec::new();
        }
        let static_sites = self.planning.airfields(&self.tiles, self.map_wrap);
        let mut bases: Vec<_> = self
            .cities
            .iter()
            .filter(|city| self.allied(unit.owner, city.owner))
            .map(|city| (city.x, city.y))
            .chain(static_sites.iter().copied())
            .chain(
                self.units
                    .iter()
                    .filter(|ship| {
                        self.allied(unit.owner, ship.owner) && ship.kind.can_carry(unit.kind)
                    })
                    .map(|ship| (ship.x, ship.y)),
            )
            .filter(|&(x, y)| self.can_rebase_to(unit, x, y))
            .collect();
        drop(static_sites);
        bases.sort_unstable_by_key(|&(x, y)| self.map_wrap.index(x, y));
        bases.dedup();
        bases
    }

    pub(super) fn rebase_aircraft(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return false;
        };
        if !self.can_rebase_to(&unit, x, y) {
            return false;
        }
        let target_carrier = if self.refuel_site(unit.owner, x, y) {
            None
        } else {
            let Some(carrier_id) = self.rebase_carrier_at(&unit, x, y) else {
                return false;
            };
            Some(carrier_id)
        };
        let from = (unit.x, unit.y);
        if unit.transport.is_some() {
            self.release_ai_ferry(id);
        }
        let visual_mover = self.visual_events.enabled.then(|| unit.clone());
        let Some(index) = self.unit_index_by_id(id) else {
            return false;
        };
        {
            let live = &mut self.units[index];
            live.x = x;
            live.y = y;
            live.transport = target_carrier;
            live.moves_left = 0;
            live.moved = true;
            live.state = UnitState::Idle;
            live.order = None;
            live.automated = false;
            live.clear_work();
            self.list_order.unit(live);
        }
        self.unit_goto_tiles.remove(&id);
        self.record_transport_change(id, target_carrier);
        self.refresh_visibility();
        if let Some(visual_unit) = visual_mover {
            self.publish_visual_event(VisualEvent::AirMission(Box::new(
                presentation::AirMissionVisual {
                    aircraft: visual_unit,
                    from,
                    to: (x, y),
                    kind: presentation::AirMissionVisualKind::Rebase,
                },
            )));
        }
        if self.trace.wants::<military_observation::AirMissionEvent>() {
            self.trace_air_mission(military_observation::AirMissionEvent {
                owner: unit.owner,
                unit_id: id,
                kind: unit.kind.name(),
                role: military_observation::military_unit_role(unit.kind),
                mission: "rebase",
                from,
                to: Some((x, y)),
                target_owner: None,
                succeeded: true,
            });
        }
        true
    }

    pub(crate) fn can_begin_recon(&self, unit: &Unit) -> bool {
        unit.kind.is_fixed_wing_aircraft()
            && unit.kind.operational_range() > 0
            && unit.moves_left > 0
            && self.aircraft_on_base(unit)
    }

    pub(crate) fn can_recon_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        self.can_begin_recon(unit)
            && (x, y) != (unit.x, unit.y)
            && (0..self.map_wrap.width()).contains(&x)
            && (0..self.map_wrap.height()).contains(&y)
            && self.map_wrap.step_distance(unit.x, unit.y, x, y) <= unit.kind.operational_range()
    }

    pub(crate) fn recon_targets_in_range(&self, id: u32) -> Vec<(i32, i32)> {
        let Some(unit) = self.unit_by_id(id) else {
            return Vec::new();
        };
        if !self.can_begin_recon(unit) {
            return Vec::new();
        }
        self.map_wrap
            .tiles_within_steps(unit.x, unit.y, unit.kind.operational_range())
            .into_iter()
            .filter_map(|(x, y, _)| self.can_recon_tile(unit, x, y).then_some((x, y)))
            .collect()
    }

    pub(super) fn recon_aircraft(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return false;
        };
        if !self.can_recon_tile(&unit, x, y) {
            return false;
        }
        let from = (unit.x, unit.y);
        self.cancel_unit_order(id);
        if !self.try_air_superiority_interceptions(id, (x, y)) {
            if self.trace.wants::<military_observation::AirMissionEvent>() {
                self.trace_air_mission(military_observation::AirMissionEvent {
                    owner: unit.owner,
                    unit_id: id,
                    kind: unit.kind.name(),
                    role: military_observation::military_unit_role(unit.kind),
                    mission: "recon",
                    from,
                    to: Some((x, y)),
                    target_owner: self.tile(x, y).owner,
                    succeeded: false,
                });
            }
            return true;
        }
        // Civ III Recon reveals the target tile and all 8 adjacent tiles (3x3)
        // for the remainder of the current turn, enabling follow-up strikes.
        self.reveal_map_circle(unit.owner, x, y, 1);
        let patch: Vec<(i32, i32)> = self
            .map_wrap
            .tiles_within_steps(x, y, 1)
            .into_iter()
            .map(|(tx, ty, _)| (tx, ty))
            .collect();
        {
            let entry = self.recon_revealed_tiles.entry(unit.owner).or_default();
            for &coord in &patch {
                if !entry.contains(&coord) {
                    entry.push(coord);
                }
            }
        }
        if self.vision_recipients(unit.owner).contains(&HUMAN) {
            for (tx, ty) in patch {
                let tile = self.tile_mut(tx, ty);
                tile.seen = true;
                tile.visible = true;
            }
        }
        if let Some(live) = self.units.iter_mut().find(|candidate| candidate.id == id) {
            live.moves_left = 0;
            live.moved = true;
            live.state = UnitState::Idle;
        }
        if self.visual_events.enabled {
            self.publish_visual_event(VisualEvent::AirMission(Box::new(
                presentation::AirMissionVisual {
                    aircraft: unit.clone(),
                    from,
                    to: (x, y),
                    kind: presentation::AirMissionVisualKind::Recon,
                },
            )));
        }
        if unit.owner == HUMAN {
            self.add_event(format!("{} completed a recon mission.", unit.kind.name()));
        }
        if self.trace.wants::<military_observation::AirMissionEvent>() {
            self.trace_air_mission(military_observation::AirMissionEvent {
                owner: unit.owner,
                unit_id: id,
                kind: unit.kind.name(),
                role: military_observation::military_unit_role(unit.kind),
                mission: "recon",
                from,
                to: Some((x, y)),
                target_owner: self.tile(x, y).owner,
                succeeded: true,
            });
        }
        true
    }

    pub(crate) fn can_set_air_superiority(&self, unit: &Unit) -> bool {
        is_air_superiority_unit(unit.kind)
            && unit.kind.operational_range() >= 2
            && self.aircraft_on_base(unit)
    }

    pub(crate) fn can_precision_bomb(&self, unit: &Unit) -> bool {
        matches!(
            unit.kind,
            UnitKind::StealthBomber | UnitKind::StealthFighter
        ) && self.has_tech(unit.owner, crate::rules::Tech::SMART_WEAPONS)
            && self.can_begin_bombard(unit)
    }

    pub(super) fn start_air_superiority(&mut self, id: u32) -> bool {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return false;
        };
        if !self.can_set_air_superiority(&unit) {
            return false;
        }
        let Some(index) = self.unit_index_by_id(id) else {
            return false;
        };
        self.units[index].order = Some(UnitOrder::AirSuperiority {
            x: unit.x,
            y: unit.y,
        });
        self.units[index].air_superiority_shot = false;
        self.unit_goto_tiles.remove(&id);
        true
    }

    /// Resolve Conquests-style Air Superiority detection for a mission target.
    /// The manual gives the defensive radius but not detection odds; the PTW
    /// guide's 50% normal / 5% stealth values are used as supplementary data.
    pub(super) fn try_air_superiority_interceptions(
        &mut self,
        attacking_id: u32,
        target: (i32, i32),
    ) -> bool {
        let Some(incoming) = self.unit_by_id(attacking_id).cloned() else {
            return false;
        };
        if !incoming.kind.is_aircraft() {
            return true;
        }
        let interceptors: Vec<_> = self
            .units
            .iter()
            .filter_map(|unit| {
                let Some(UnitOrder::AirSuperiority { x, y }) = unit.order else {
                    return None;
                };
                (self.at_war(unit.owner, incoming.owner)
                    && !unit.air_superiority_shot
                    && is_air_superiority_unit(unit.kind)
                    && (unit.x, unit.y) == (x, y)
                    && self.can_set_air_superiority(unit)
                    && self.map_wrap.step_distance(x, y, target.0, target.1)
                        <= unit.kind.operational_range() / 2)
                    .then_some((unit.id, unit.kind))
            })
            .collect();
        for (interceptor_id, _) in interceptors {
            if self.unit_by_id(interceptor_id).is_none() {
                continue;
            }
            let detection_chance = if incoming.kind.is_stealth_aircraft() {
                5
            } else {
                50
            };
            if self.rand_below(100) >= detection_chance {
                continue;
            }
            let interceptor_info = self
                .unit_by_id(interceptor_id)
                .map(|unit| (unit.owner, unit.kind, (unit.x, unit.y), unit.moved));
            if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == interceptor_id) {
                unit.air_superiority_shot = true;
            }
            self.resolve_combat_inner(
                interceptor_id,
                attacking_id,
                target.0,
                target.1,
                None,
                false,
            );
            // Position the air-to-air dogfight over the contested target tile so
            // both aircraft face each other in combat instead of shooting from
            // distant airfields.
            if self.visual_events.enabled {
                let incoming_pos = self
                    .map_wrap
                    .eight_way_neighbors(target.0, target.1)
                    .min_by_key(|&(nx, ny)| {
                        self.map_wrap.step_distance(nx, ny, incoming.x, incoming.y)
                    })
                    .unwrap_or(target);
                self.relocate_last_combat_visual(
                    interceptor_id,
                    attacking_id,
                    target,
                    incoming_pos,
                );
            }
            // A defending CAP interceptor scrambled on the opponent's turn;
            // preserve its own-turn `moved` flag so it heals normally at base.
            if let Some((_, _, _, was_moved)) = interceptor_info
                && let Some(interceptor) =
                    self.units.iter_mut().find(|unit| unit.id == interceptor_id)
            {
                interceptor.moved = was_moved;
            }
            let shot_down = self.unit_by_id(attacking_id).is_none();
            // Civ III rule: any Air Superiority interception aborts the
            // incoming aircraft's mission, even if the attacker survives.
            if let Some(live) = self.units.iter_mut().find(|unit| unit.id == attacking_id) {
                live.moves_left = 0;
                live.moved = true;
                live.state = UnitState::Idle;
            }
            if let Some((owner, kind, from, _)) = interceptor_info {
                self.record_air_threat_contact(owner, incoming.owner, None);
                self.record_air_threat_contact(incoming.owner, owner, None);
                if owner == HUMAN || incoming.owner == HUMAN {
                    let message = if shot_down {
                        format!(
                            "{} {} shot down {} {} during interception!",
                            self.players[owner].name,
                            kind.name(),
                            self.players[incoming.owner].name,
                            incoming.kind.name()
                        )
                    } else {
                        format!(
                            "{} {} intercepted {} {} and aborted its mission.",
                            self.players[owner].name,
                            kind.name(),
                            self.players[incoming.owner].name,
                            incoming.kind.name()
                        )
                    };
                    self.add_event(message);
                }
                if self.trace.wants::<military_observation::AirMissionEvent>() {
                    self.trace_air_mission(military_observation::AirMissionEvent {
                        owner,
                        unit_id: interceptor_id,
                        kind: kind.name(),
                        role: military_observation::military_unit_role(kind),
                        mission: "interception",
                        from,
                        to: Some(target),
                        target_owner: Some(incoming.owner),
                        succeeded: shot_down,
                    });
                }
            }
            return false;
        }
        self.unit_by_id(attacking_id).is_some()
    }

    /// Resolve Civ III: Conquests passive anti-aircraft defense for a fixed-wing
    /// bombard run: city SAM Battery first (50% normal / 5% stealth fire chance,
    /// AA 8 vs aircraft defense; firing aborts the run even if the plane
    /// survives), followed by up to 4 strongest hostile tile AA units
    /// (`air_defense > 0`), each rolling `air_defense / (air_defense + 10 * D)`.
    fn try_anti_aircraft_defense(
        &mut self,
        attacker_id: u32,
        x: i32,
        y: i32,
    ) -> Option<&'static str> {
        let attacker = self.unit_by_id(attacker_id).cloned()?;
        if !attacker.kind.is_fixed_wing_aircraft() {
            return None;
        }
        let plane_defense = attacker.kind.stats().defense.max(1);

        // 1. City SAM Battery check.
        if let Some((city_owner, city_name)) = self
            .city_at(x, y)
            .filter(|city| {
                city.owner != attacker.owner
                    && self.at_war(attacker.owner, city.owner)
                    && city.has_building(crate::rules::Building::SAM_BATTERY)
            })
            .map(|city| (city.owner, city.name.clone()))
        {
            let fire_chance = if attacker.kind.is_stealth_aircraft() {
                5
            } else {
                50
            };
            if self.rand_below(100) < fire_chance {
                const SAM_BATTERY_AA: i32 = 8;
                let shot_down = self.rand_below(SAM_BATTERY_AA + plane_defense) < SAM_BATTERY_AA;
                if shot_down {
                    self.remove_units_with_cargo_because(
                        vec![attacker.id],
                        lifecycle::UnitRemoval::Combat,
                    );
                    self.nation_scores[city_owner].units_killed += 1;
                } else if let Some(live) = self.units.iter_mut().find(|unit| unit.id == attacker.id)
                {
                    live.moves_left = 0;
                    live.moved = true;
                    live.state = UnitState::Idle;
                }
                if self.visual_events.enabled {
                    self.publish_visual_event(VisualEvent::AirMission(Box::new(
                        presentation::AirMissionVisual {
                            aircraft: attacker.clone(),
                            from: (attacker.x, attacker.y),
                            to: (x, y),
                            kind: presentation::AirMissionVisualKind::SamIntercept { shot_down },
                        },
                    )));
                }
                if attacker.owner == HUMAN || city_owner == HUMAN {
                    let message = if shot_down {
                        format!(
                            "{}'s SAM Battery shot down {} {}!",
                            city_name,
                            self.players[attacker.owner].name,
                            attacker.kind.name()
                        )
                    } else {
                        format!(
                            "{}'s SAM Battery drove off {} {}.",
                            city_name,
                            self.players[attacker.owner].name,
                            attacker.kind.name()
                        )
                    };
                    self.add_event(message);
                }
                return Some("intercepted_sam");
            }
        }

        // 2. Tile AA units (up to 4 strongest hostile units with air_defense > 0).
        let mut aa_units: Vec<_> = self
            .tile_units_ordered(x, y)
            .filter(|unit| {
                unit.owner != attacker.owner
                    && self.at_war(attacker.owner, unit.owner)
                    && unit.transport.is_none()
                    && unit.kind.air_defense() > 0
            })
            .map(|unit| (unit.kind.air_defense(), unit.id, unit.owner, unit.kind))
            .collect();
        aa_units.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        aa_units.truncate(4);
        for (aa_value, aa_id, aa_owner, aa_kind) in aa_units {
            let denom = aa_value + 10 * plane_defense;
            if self.rand_below(denom) < aa_value {
                let aa_unit = self.unit_by_id(aa_id).cloned();
                self.remove_units_with_cargo_because(
                    vec![attacker.id],
                    lifecycle::UnitRemoval::Combat,
                );
                self.nation_scores[aa_owner].units_killed += 1;
                if self.visual_events.enabled
                    && let Some(aa_unit) = aa_unit
                {
                    self.publish_visual_event(VisualEvent::AirMission(Box::new(
                        presentation::AirMissionVisual {
                            aircraft: attacker.clone(),
                            from: (attacker.x, attacker.y),
                            to: (x, y),
                            kind: presentation::AirMissionVisualKind::AaUnitIntercept {
                                aa_unit,
                                shot_down: true,
                            },
                        },
                    )));
                }
                if attacker.owner == HUMAN || aa_owner == HUMAN {
                    self.add_event(format!(
                        "{} {} shot down {} {} with anti-aircraft fire!",
                        self.players[aa_owner].name,
                        aa_kind.name(),
                        self.players[attacker.owner].name,
                        attacker.kind.name()
                    ));
                }
                return Some("intercepted_aa");
            }
        }
        None
    }

    /// Civ III Conquests bombard range.
    fn fixed_wing_bombard_range(kind: UnitKind) -> i32 {
        if kind.bombard_range() > 0 {
            kind.bombard_range()
        } else if kind.is_aircraft() {
            kind.operational_range()
        } else {
            0
        }
    }

    fn bombard_blocked_by_action(&self, actor: &Unit, x: i32, y: i32, blocked_by: &[&str]) -> bool {
        self.tile_units_ordered(x, y).any(|target| {
            if !self.unit_visible_to(actor.owner, target) {
                return false;
            }
            blocked_by.iter().any(|action| {
                self.action_available_for_unit_against_unit(actor, action, target)
                    .unwrap_or(true)
            })
        })
    }

    fn ai_bombard_preempted_by_priority_action(&self, actor: &Unit, x: i32, y: i32) -> bool {
        ["Capture Units", "Collect Ransom"].iter().any(|action| {
            self.tile_units_ordered(x, y).any(|target| {
                self.unit_visible_to(actor.owner, target)
                    && self
                        .action_available_for_unit_against_unit(actor, action, target)
                        .is_ok_and(|available| available)
            })
        })
    }

    pub(crate) fn can_begin_bombard(&self, unit: &Unit) -> bool {
        (unit.owner == HUMAN || self.players.get(unit.owner).is_some_and(|player| player.ai))
            && self.has_bombard_capability(unit.kind)
            && unit.moves_left > 0
            && (unit.kind == UnitKind::CruiseMissile
                || unit.kind.fuel_capacity() == 0
                || self.refuel_site(unit.owner, unit.x, unit.y)
                || self.aboard_refueling_transport(unit))
    }

    fn bombard_action_available_at_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        let city_action = self.city_at(x, y).is_some_and(|city| {
            self.tile(x, y).known_by[unit.owner]
                && city.owner != unit.owner
                && self.at_war(unit.owner, city.owner)
        });
        let unit_action = self.tile_units_ordered(x, y).any(|target| {
            target.owner != unit.owner
                && self.at_war(unit.owner, target.owner)
                && target.transport.is_none()
                && !(unit.kind.fuel_capacity() > 0 && target.kind.fuel_capacity() > 0)
                && self.unit_visible_to(unit.owner, target)
                && self.combat_target_reachable(unit.kind, target.kind, x, y)
        });
        let terrain_action = !city_action
            && !self.any_unit_at(x, y, |_| true)
            && !bombardable_terrain_improvements(self, x, y).is_empty();
        city_action || unit_action || terrain_action
    }

    fn bombard_target_available_for_order(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.has_bombard_capability(unit.kind)
            || (unit.transport.is_some() && !self.aboard_refueling_transport(unit))
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
            || (x, y) == (unit.x, unit.y)
            || self.map_wrap.step_distance(unit.x, unit.y, x, y)
                > Self::fixed_wing_bombard_range(unit.kind)
        {
            return false;
        }
        !self.bombard_blocked_by_action(unit, x, y, BOMBARD_BLOCKED_BY)
            && self.bombard_action_available_at_tile(unit, x, y)
    }

    fn bombard_action_available_for_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        unit.moves_left >= 1 && self.bombard_target_available_for_order(unit, x, y)
    }

    pub(super) fn repeated_bombard_target_remains(&self, unit: &Unit, x: i32, y: i32) -> bool {
        (if unit.owner == HUMAN {
            self.tile(x, y).visible
        } else {
            self.tile(x, y).known_by[unit.owner]
        }) && self.bombard_target_available_for_order(unit, x, y)
    }

    pub(super) fn repeated_precision_bombard_target_remains(
        &self,
        unit: &Unit,
        x: i32,
        y: i32,
    ) -> bool {
        (if unit.owner == HUMAN {
            self.tile(x, y).visible
        } else {
            self.tile(x, y).known_by[unit.owner]
        }) && self.can_precision_bomb_tile(unit, x, y)
    }

    pub(crate) fn can_bombard_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.can_begin_bombard(unit)
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
            || !(if unit.owner == HUMAN {
                self.tile(x, y).visible
            } else {
                self.tile(x, y).known_by[unit.owner]
            })
        {
            return false;
        }
        self.bombard_action_available_for_tile(unit, x, y)
    }

    pub(crate) fn bombardable_targets_in_range(&self, id: u32) -> Vec<(i32, i32)> {
        let Some(unit) = self.unit_by_id(id) else {
            return Vec::new();
        };
        if !self.can_begin_bombard(unit) {
            return Vec::new();
        }
        self.map_wrap
            .tiles_within_steps(unit.x, unit.y, Self::fixed_wing_bombard_range(unit.kind))
            .into_iter()
            .filter_map(|(x, y, _)| self.can_bombard_tile(unit, x, y).then_some((x, y)))
            .collect()
    }

    pub(super) fn start_repeated_bombard(&mut self, id: u32, x: i32, y: i32) -> bool {
        if self.try_ranged_bombard(id, x, y) == MoveResult::Blocked {
            return false;
        }
        let Some(index) = self.unit_index_by_id(id) else {
            return true;
        };
        self.units[index].order = Some(UnitOrder::Bombard { x, y });
        true
    }

    pub(super) fn start_repeated_precision_bombard(&mut self, id: u32, x: i32, y: i32) -> bool {
        if self.try_precision_bombard(id, x, y) == MoveResult::Blocked {
            return false;
        }
        if let Some(index) = self.unit_index_by_id(id) {
            self.units[index].order = Some(UnitOrder::PrecisionBombard { x, y });
        }
        true
    }

    pub(crate) fn precision_bombardable_targets_in_range(&self, id: u32) -> Vec<(i32, i32)> {
        let Some(unit) = self.unit_by_id(id) else {
            return Vec::new();
        };
        if !self.can_precision_bomb(unit) {
            return Vec::new();
        }
        self.map_wrap
            .tiles_within_steps(unit.x, unit.y, Self::fixed_wing_bombard_range(unit.kind))
            .into_iter()
            .filter_map(|(x, y, _)| self.can_precision_bomb_tile(unit, x, y).then_some((x, y)))
            .collect()
    }

    fn can_precision_bomb_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.can_precision_bomb(unit)
            || (unit.x, unit.y) == (x, y)
            || self.map_wrap.step_distance(unit.x, unit.y, x, y)
                > Self::fixed_wing_bombard_range(unit.kind)
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
            || !self.tile(x, y).known_by[unit.owner]
        {
            return false;
        }
        self.city_at(x, y).is_some_and(|city| {
            city.owner != unit.owner
                && self.at_war(unit.owner, city.owner)
                && (city.size > 1
                    || city
                        .buildings
                        .iter()
                        .any(|b| b.data().genus == crate::rules::Genus::Improvement))
        })
    }

    pub(super) fn try_precision_bombard(&mut self, id: u32, x: i32, y: i32) -> MoveResult {
        let Some(attacker) = self.unit_by_id(id).cloned() else {
            return MoveResult::Blocked;
        };
        if attacker.moves_left < 1 || !self.can_precision_bomb_tile(&attacker, x, y) {
            return MoveResult::Blocked;
        }
        self.try_bombard_mode(&attacker, x, y, true)
    }

    pub(super) fn try_ranged_bombard(&mut self, id: u32, x: i32, y: i32) -> MoveResult {
        let Some(attacker) = self.unit_by_id(id).cloned() else {
            return MoveResult::Blocked;
        };
        if !self.can_bombard_tile(&attacker, x, y) {
            return MoveResult::Blocked;
        }
        self.try_bombard(&attacker, x, y)
    }

    /// Freeciv's dai_unit_attack selects Bombard after capture/ransom when
    /// evaluating a reachable attack tile. Civ III bombard ranges exceed the
    /// adjacent C2C3 action range, so scan the unit's legal firing radius
    /// before its normal movement/guard task consumes the turn.
    pub(super) fn run_ai_bombard(&mut self, unit: &Unit) -> bool {
        if !self.players.get(unit.owner).is_some_and(|player| player.ai)
            || !self.can_begin_bombard(unit)
        {
            return false;
        }
        let (_, war_mask, _) = self.pact_masks(unit.owner);
        if war_mask == 0
            && !(0..self.players.len())
                .any(|other| other != unit.owner && self.at_war(unit.owner, other))
        {
            return false;
        }
        let range = Self::fixed_wing_bombard_range(unit.kind);
        let mut best: Option<(f64, i32, i32)> = None;
        for (x, y, _) in self.map_wrap.tiles_within_steps(unit.x, unit.y, range) {
            let tile_owner_war = self.tile(x, y).owner.is_some_and(|owner| {
                owner != unit.owner
                    && ((owner < 64 && ((war_mask >> owner) & 1) != 0)
                        || (owner >= 64 && self.at_war(unit.owner, owner)))
            });
            let city_war = self.city_at_cached(x, y).is_some_and(|city| {
                city.owner != unit.owner
                    && ((city.owner < 64 && ((war_mask >> city.owner) & 1) != 0)
                        || (city.owner >= 64 && self.at_war(unit.owner, city.owner)))
            });
            let unit_war = self.any_unit_at(x, y, |other| {
                other.owner != unit.owner
                    && ((other.owner < 64 && ((war_mask >> other.owner) & 1) != 0)
                        || (other.owner >= 64 && self.at_war(unit.owner, other.owner)))
            });
            if !tile_owner_war && !city_war && !unit_war {
                continue;
            }
            if !self.can_bombard_tile(unit, x, y)
                || self.ai_bombard_preempted_by_priority_action(unit, x, y)
            {
                continue;
            }
            let value = self.ai_bombard_tile_value(unit, x, y);
            if value <= 0.0 {
                continue;
            }
            let better = best.is_none_or(|(best_val, best_x, best_y)| {
                value
                    .total_cmp(&best_val)
                    .then_with(|| best_y.cmp(&y))
                    .then_with(|| best_x.cmp(&x))
                    .is_gt()
            });
            if better {
                best = Some((value, x, y));
            }
        }
        let Some((_, x, y)) = best else {
            return false;
        };
        if self.can_precision_bomb(unit)
            && self.city_at(x, y).is_some()
            && self.city_at(x, y).is_some_and(|city| {
                city.buildings
                    .iter()
                    .any(|b| b.data().genus == crate::rules::Genus::Improvement)
            })
        {
            return matches!(
                self.try_precision_bombard(unit.id, x, y),
                MoveResult::Fought
            );
        }
        matches!(self.try_ranged_bombard(unit.id, x, y), MoveResult::Fought)
    }

    /// Move conventional bombarders toward a legal firing position when no
    /// current shot is available. Re-evaluate the visible targets each turn;
    /// this deliberately does not turn an aircraft into a ground-style
    /// artillery unit.
    pub(super) fn run_ai_bombard_approach(&mut self, unit: &Unit) -> bool {
        if !self.players.get(unit.owner).is_some_and(|player| player.ai)
            || unit.kind.fuel_capacity() > 0
            || !self.can_begin_bombard(unit)
            || !(0..self.players.len())
                .any(|other| other != unit.owner && self.at_war(unit.owner, other))
        {
            return false;
        }
        let blocked_by = BOMBARD_BLOCKED_BY;
        let range = Self::fixed_wing_bombard_range(unit.kind);
        let mut target_tiles = std::collections::BTreeSet::new();
        for city in self.cities.iter().filter(|city| {
            city.owner != unit.owner
                && self.at_war(unit.owner, city.owner)
                && self.tile(city.x, city.y).known_by[unit.owner]
        }) {
            target_tiles.insert((city.x, city.y));
        }
        for target in self.units.iter().filter(|target| {
            target.owner != unit.owner
                && self.at_war(unit.owner, target.owner)
                && target.transport.is_none()
                && self.unit_visible_to(unit.owner, target)
        }) {
            target_tiles.insert((target.x, target.y));
        }
        for (index, _) in self.tiles.iter().enumerate().filter(|(_, tile)| {
            tile.owner
                .is_some_and(|owner| owner != unit.owner && self.at_war(unit.owner, owner))
                && tile.known_by[unit.owner]
        }) {
            let (x, y) = (
                index as i32 % self.map_wrap.width(),
                index as i32 / self.map_wrap.width(),
            );
            if !bombardable_terrain_improvements(self, x, y).is_empty() {
                target_tiles.insert((x, y));
            }
        }
        if target_tiles.is_empty() {
            return false;
        }

        // Pre-filter and score each target tile once: both target availability
        // and tile value are independent of the firing position (px, py).
        // Sorting by value descending enables exact branch-and-bound pruning.
        let connectivity = self.movement_connectivity(unit.owner);
        let mut scored_targets = Vec::with_capacity(target_tiles.len());
        for (tx, ty) in target_tiles {
            let has_reachable_firing_component = self
                .map_wrap
                .tiles_within_steps(tx, ty, range)
                .into_iter()
                .any(|(px, py, distance)| {
                    distance > 0
                        && (px, py) != (unit.x, unit.y)
                        && self.tile(px, py).known_by[unit.owner]
                        && connectivity.can_directly_reach_tile(
                            self,
                            unit.kind,
                            (unit.x, unit.y),
                            (px, py),
                            false,
                        )
                });
            if !has_reachable_firing_component {
                continue;
            }
            if !self.bombard_action_available_at_tile(unit, tx, ty) {
                continue;
            }
            let value = self.ai_bombard_tile_value(unit, tx, ty);
            if value <= 0.0 {
                continue;
            }
            let has_visible_units = self
                .tile_units_ordered(tx, ty)
                .any(|target| self.unit_visible_to(unit.owner, target));
            scored_targets.push((value, (tx, ty), has_visible_units));
        }
        if scored_targets.is_empty() {
            return false;
        }
        scored_targets.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

        // Tie-break exact (score, cost) matches identically to BTreeSet + tiles_within_steps order.
        type ScoredFiringPosition = (f64, i32, (i32, i32), usize, (i32, i32));
        let mut best: Option<ScoredFiringPosition> = None;
        let mut maps = None;
        let move_rate = f64::from(self.unit_move_rate(unit));
        let mut firing_unit = unit.clone();
        for (value, (tx, ty), has_visible_units) in scored_targets {
            if let Some((best_score, _, _, _, _)) = best
                && value < best_score
            {
                break;
            }
            let max_cost = if let Some((best_score, _, _, _, _)) = best {
                let bound = ((value / best_score - 1.0) * move_rate).ceil();
                if bound < 0.0 {
                    continue;
                }
                (bound as i32).saturating_add(1)
            } else {
                i32::MAX
            };
            for (radius_idx, (px, py, distance)) in self
                .map_wrap
                .tiles_within_steps(tx, ty, range)
                .into_iter()
                .enumerate()
            {
                if distance == 0
                    || (px, py) == (unit.x, unit.y)
                    || !self.tile(px, py).known_by[unit.owner]
                    || !connectivity.can_directly_reach_tile(
                        self,
                        unit.kind,
                        (unit.x, unit.y),
                        (px, py),
                        false,
                    )
                    || self
                        .city_at(px, py)
                        .is_some_and(|city| city.owner != unit.owner)
                    || self
                        .tile_units_ordered(px, py)
                        .any(|other| other.id != unit.id && self.at_war(unit.owner, other.owner))
                {
                    continue;
                }
                // Check reachability within the current branch-and-bound cost
                // limit before running Freeciv action-enabler checks.
                let (known, routes) = maps.get_or_insert_with(|| {
                    self.trace.count_operation("bombard.shared_map_pairs", 1);
                    (
                        NormalPaths::goto_sanity_map(self, unit),
                        NormalPaths::goto_map(self, unit, GotoTask::Attack),
                    )
                });
                self.trace
                    .count_operation("bombard.candidate_route_queries", 1);
                let Some(cost) = routes.movement_cost_within(self, px, py, max_cost) else {
                    continue;
                };
                let score = value / (1.0 + f64::from(cost) / move_rate);
                if let Some((best_score, best_cost, best_target, best_radius_idx, _)) = best
                    && (score < best_score
                        || (score == best_score
                            && (cost > best_cost
                                || (cost == best_cost
                                    && ((tx, ty) > best_target
                                        || ((tx, ty) == best_target
                                            && radius_idx >= best_radius_idx))))))
                {
                    continue;
                }
                if known.movement_cost(self, px, py).is_none() {
                    continue;
                }
                if has_visible_units {
                    (firing_unit.x, firing_unit.y) = (px, py);
                    if self.bombard_blocked_by_action(&firing_unit, tx, ty, blocked_by)
                        || self.ai_bombard_preempted_by_priority_action(&firing_unit, tx, ty)
                    {
                        continue;
                    }
                }
                best = Some((score, cost, (tx, ty), radius_idx, (px, py)));
            }
        }
        let (Some((_, _, _, _, (px, py))), Some((_, mut routes))) = (best, maps) else {
            return false;
        };
        routes
            .path(self, px, py)
            .is_some_and(|(path, _)| self.follow_source_ai_path(unit.id, path).moved)
    }

    fn choose_city_bombard_target(
        &mut self,
        city: &City,
        unit_targets: &[Unit],
    ) -> Option<CityBombardTarget> {
        // Prima's PTW combat chapter describes 25% population, 25% buildings,
        // and 50% garrison selection. Conquests confirms those target classes
        // but not the weights, so keep this source-derived split isolated.
        match self.rand_below(100) {
            0..=24 if city.size > 1 => Some(CityBombardTarget::Population(city.id)),
            25..=49 => {
                let buildings: Vec<_> = city
                    .buildings
                    .iter()
                    .copied()
                    .filter(|building| building.data().genus == crate::rules::Genus::Improvement)
                    .collect();
                (!buildings.is_empty()).then(|| {
                    let index = self.rand_below(buildings.len() as i32) as usize;
                    CityBombardTarget::Building(city.id, buildings[index])
                })
            }
            50..=99 => (!unit_targets.is_empty()).then(|| {
                let index = self.rand_below(unit_targets.len() as i32) as usize;
                CityBombardTarget::Unit(unit_targets[index].clone())
            }),
            _ => None,
        }
    }

    fn choose_precision_city_bombard_target(&mut self, city: &City) -> Option<CityBombardTarget> {
        let buildings: Vec<_> = city
            .buildings
            .iter()
            .copied()
            .filter(|building| building.data().genus == crate::rules::Genus::Improvement)
            .collect();
        if buildings.is_empty() {
            return (city.size > 1).then_some(CityBombardTarget::Population(city.id));
        }
        let index = self.rand_below(buildings.len() as i32) as usize;
        Some(CityBombardTarget::Building(city.id, buildings[index]))
    }

    pub(super) fn record_air_threat_contact(
        &mut self,
        victim_owner: usize,
        attacker_owner: usize,
        city_id: Option<u32>,
    ) {
        if victim_owner != attacker_owner {
            self.ai_air_raid_turn
                .insert((victim_owner, attacker_owner), self.turn);
            if let Some(cid) = city_id {
                self.ai_city_bombed_turn.insert(cid, self.turn);
            }
        }
    }

    fn ai_bombard_tile_value(&self, attacker: &Unit, x: i32, y: i32) -> f64 {
        let hostile_city = self
            .city_at(x, y)
            .filter(|city| city.owner != attacker.owner && self.at_war(attacker.owner, city.owner));
        let city_unit_weight = if hostile_city.is_some() { 0.5 } else { 1.0 };
        let mut attack_and_rate = None;
        let mut value = 0.0;
        for target in self.tile_units_ordered(x, y).filter(|target| {
            target.owner != attacker.owner
                && self.at_war(attacker.owner, target.owner)
                && target.transport.is_none()
                && self.unit_visible_to(attacker.owner, target)
                && self.combat_target_reachable(attacker.kind, target.kind, x, y)
        }) {
            let (attack, rate) = *attack_and_rate.get_or_insert_with(|| {
                let attack = self.attack_power_against_with_base_for_action(
                    attacker,
                    x,
                    y,
                    attacker.kind.bombard_strength(),
                    "Bombard",
                );
                let rate = if crate::sim::combat_rules::selected().damage_reduces_bombard_rate
                    && attacker.kind.bombard_rate() > 0
                {
                    (attacker.kind.bombard_rate() * attacker.hp / attacker.max_hit_points()).max(1)
                } else {
                    attacker.kind.bombard_rate()
                };
                (attack, rate)
            });
            let defense = self.defense_power_against(target, x, y, attacker.kind);
            let firepower = self.combat_firepower(attacker, target).0;
            if attack + defense <= 0 {
                continue;
            }
            let expected_damage = (f64::from(attack) / f64::from(attack + defense)
                * f64::from(rate * firepower))
            .min(f64::from(target.hp));
            let lethal = attacker.kind.has_lethal_bombard_against(target.kind);
            let lethal_bonus = if lethal && expected_damage >= f64::from(target.hp) {
                f64::from(target.kind.stats().cost) * 0.5
            } else {
                0.0
            };
            let priority_mult = if target.kind == UnitKind::Army {
                1.4
            } else {
                1.0
            };
            value += city_unit_weight
                * priority_mult
                * ((f64::from(target.kind.stats().cost) + 1.0) * expected_damage
                    / f64::from(target.hp.max(1))
                    + lethal_bonus);
        }
        if let Some(city) = hostile_city {
            let improvements = city
                .buildings
                .iter()
                .filter(|building| building.data().genus == crate::rules::Genus::Improvement)
                .count();
            let mut city_value = 0.0;
            if city.size > 1 {
                city_value += 0.25 * f64::from(city.size - 1);
            }
            city_value += 0.5 * improvements as f64;
            let garrison_count = self
                .tile_units_ordered(x, y)
                .filter(|target| {
                    target.owner != attacker.owner
                        && self.at_war(attacker.owner, target.owner)
                        && target.transport.is_none()
                        && self.combat_target_reachable(attacker.kind, target.kind, x, y)
                })
                .count();
            if garrison_count > 0 && value + city_value < 2.0 {
                city_value += 4.0 * (garrison_count.min(4) as f64);
            }
            // Combined-arms coordination: prioritize softening enemy cities
            // targeted by our active ground invasion force.
            if city_value > 0.0
                && self
                    .ai_ground_targets
                    .values()
                    .any(|&target_city| target_city == city.id)
            {
                city_value *= 1.45;
            }
            value += city_value;
        }
        if self
            .tile(x, y)
            .owner
            .is_some_and(|owner| owner != attacker.owner && self.at_war(attacker.owner, owner))
        {
            value += bombardable_terrain_improvements(self, x, y).len() as f64;
        }
        // Observable AA risk awareness for fixed-wing aircraft: prefer softer
        // targets over heavily AA-defended stacks (Stealth aircraft take far
        // less SAM/AA risk than conventional bombers/fighters).
        if value > 0.0 && attacker.kind.is_fixed_wing_aircraft() {
            let plane_def = f64::from(attacker.kind.stats().defense.max(1));
            let mut visible_aa: Vec<i32> = self
                .tile_units_ordered(x, y)
                .filter(|u| {
                    u.owner != attacker.owner
                        && self.at_war(attacker.owner, u.owner)
                        && u.transport.is_none()
                        && u.kind.air_defense() > 0
                        && self.unit_visible_to(attacker.owner, u)
                })
                .map(|u| u.kind.air_defense())
                .collect();
            visible_aa.sort_unstable_by(|a, b| b.cmp(a));
            visible_aa.truncate(4);
            let mut survival = 1.0_f64;
            for aa in visible_aa {
                let aa_f = f64::from(aa);
                survival *= 1.0 - aa_f / (aa_f + 10.0 * plane_def);
            }
            if let Some(city) = hostile_city
                && city.has_building(crate::rules::Building::SAM_BATTERY)
                && self.ai_city_bombed_turn.contains_key(&city.id)
            {
                let sam_factor = if attacker.kind.is_stealth_aircraft() {
                    0.95
                } else {
                    0.55
                };
                survival *= sam_factor;
            }
            if survival < 1.0 {
                let expected_loss = (1.0 - survival) * f64::from(attacker.kind.stats().cost) * 1.5;
                value = (value * survival * survival - expected_loss).max(value * 0.15);
            }
        }
        value
    }

    pub(super) fn refuel_site(&self, owner: usize, x: i32, y: i32) -> bool {
        self.city_at_cached(x, y)
            .is_some_and(|city| self.allied(owner, city.owner))
            || (matches!(self.tile(x, y).base, Some(Base::Airstrip | Base::Airbase))
                && self.tile(x, y).known_by[owner])
    }

    fn aboard_refueling_transport(&self, unit: &Unit) -> bool {
        unit.transport.is_some_and(|id| {
            self.unit_by_id(id).is_some_and(|ship| {
                self.ruleset_transport_pair_allowed_fail_closed(ship.kind, unit.kind)
                    && self.allied(unit.owner, ship.owner)
            })
        })
    }

    pub(super) fn run_source_air_ferry(&mut self, carrier: &Unit) -> (bool, Vec<u32>) {
        let mut done = Vec::new();
        if carrier.moves_left <= 0 {
            return (true, done);
        }
        if carrier.hp < carrier.max_hit_points()
            && self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (carrier.x, carrier.y))
        {
            return (true, done);
        }
        self.refresh_ferry_boss(carrier.id);
        loop {
            let Some(boss) = self
                .ferry_boss(carrier.id)
                .and_then(|id| self.unit_by_id(id))
                .cloned()
            else {
                return (false, done);
            };
            if self
                .map_wrap
                .step_distance(carrier.x, carrier.y, boss.x, boss.y)
                > 1
            {
                // Native temporarily abandons a distant boss and falls back
                // to pickup/exploration if no cargo replaces it. The restored
                // reservation must not activate that remote passenger.
                return (false, done);
            }
            // Land passengers retain their ordinary founder/worker/military
            // dispatch; fueled cargo uses the same manager as a land airbase.
            if boss.kind.fuel_capacity() == 0 {
                return (false, done);
            }
            if boss.moves_left > 0 && !self.ai_guard_targets.contains_key(&boss.id) {
                self.run_ai_strike_aircraft(&boss);
            }
            let Some(ship) = self
                .units
                .iter()
                .find(|unit| unit.id == carrier.id)
                .cloned()
            else {
                return (true, done);
            };
            if self
                .units
                .iter()
                .any(|unit| unit.id == boss.id && (unit.x, unit.y) == (ship.x, ship.y))
            {
                // daiferry.c: the boss remains on the boat, so hold station
                // and mark it done for the owner's remaining unit loop.
                done.push(boss.id);
                return (true, done);
            }
            if ship.moves_left <= 0 {
                return (true, done);
            }
            if self.cargo_count(ship.id) == 0 {
                return (false, done);
            }
            self.release_ai_ferry(boss.id);
            self.refresh_ferry_boss(ship.id);
        }
    }

    pub(super) fn try_bombard(&mut self, attacker: &Unit, x: i32, y: i32) -> MoveResult {
        self.try_bombard_mode(attacker, x, y, false)
    }

    fn try_bombard_mode(&mut self, attacker: &Unit, x: i32, y: i32, precision: bool) -> MoveResult {
        if !(if precision {
            self.can_precision_bomb_tile(attacker, x, y) && attacker.moves_left > 0
        } else {
            self.bombard_action_available_for_tile(attacker, x, y)
        }) {
            return MoveResult::Blocked;
        }
        if attacker.kind.is_fixed_wing_aircraft() {
            let raided_city = self
                .city_at(x, y)
                .filter(|city| city.owner != attacker.owner)
                .map(|city| (city.owner, city.id));
            if let Some((victim_owner, city_id)) = raided_city {
                self.record_air_threat_contact(victim_owner, attacker.owner, Some(city_id));
            } else {
                let victims: Vec<usize> = self
                    .tile_units_ordered(x, y)
                    .filter(|u| u.owner != attacker.owner && self.at_war(attacker.owner, u.owner))
                    .map(|u| u.owner)
                    .chain(
                        self.tile(x, y)
                            .owner
                            .filter(|&owner| owner != attacker.owner),
                    )
                    .collect();
                for victim_owner in victims {
                    self.record_air_threat_contact(victim_owner, attacker.owner, None);
                }
            }
        }
        if !self.try_air_superiority_interceptions(attacker.id, (x, y)) {
            if self.trace.wants::<military_observation::BombardEvent>() {
                let target_city = self.city_at(x, y).map(|c| (c.id, c.owner));
                let target_owner = target_city
                    .map(|(_, owner)| owner)
                    .or_else(|| self.tile(x, y).owner);
                self.trace_bombard(military_observation::BombardEvent {
                    attacker_id: attacker.id,
                    attacker_owner: attacker.owner,
                    attacker_kind: attacker.kind.name(),
                    attacker_role: military_observation::military_unit_role(attacker.kind),
                    precision,
                    tile: (x, y),
                    city_id: target_city.map(|(id, _)| id),
                    target_owner,
                    target_kind: "intercepted",
                    intercepted: true,
                    hp_damage: 0,
                    units_hit: 0,
                    units_killed: 0,
                    building_destroyed: None,
                    population_lost: 0,
                    terrain_destroyed: Vec::new(),
                    missile_expended: false,
                });
            }
            return MoveResult::Fought;
        }
        if let Some(aa_kind) = self.try_anti_aircraft_defense(attacker.id, x, y) {
            if self.trace.wants::<military_observation::BombardEvent>() {
                let target_city = self.city_at(x, y).map(|c| (c.id, c.owner));
                let target_owner = target_city
                    .map(|(_, owner)| owner)
                    .or_else(|| self.tile(x, y).owner);
                self.trace_bombard(military_observation::BombardEvent {
                    attacker_id: attacker.id,
                    attacker_owner: attacker.owner,
                    attacker_kind: attacker.kind.name(),
                    attacker_role: military_observation::military_unit_role(attacker.kind),
                    precision,
                    tile: (x, y),
                    city_id: target_city.map(|(id, _)| id),
                    target_owner,
                    target_kind: aa_kind,
                    intercepted: true,
                    hp_damage: 0,
                    units_hit: 0,
                    units_killed: 0,
                    building_destroyed: None,
                    population_lost: 0,
                    terrain_destroyed: Vec::new(),
                    missile_expended: false,
                });
            }
            return MoveResult::Fought;
        }
        let Some(attacker) = self.unit_by_id(attacker.id).cloned() else {
            return MoveResult::Fought;
        };
        let mapped_bombard_strength = attacker.kind.bombard_strength();

        let city = self
            .city_at(x, y)
            .filter(|city| city.owner != attacker.owner && self.at_war(attacker.owner, city.owner))
            .cloned();
        let raw_targets: Vec<_> = self
            .tile_units_ordered(x, y)
            .filter(|target| {
                target.owner != attacker.owner
                    && self.at_war(attacker.owner, target.owner)
                    && target.transport.is_none()
                    && (city.is_some()
                        || !(attacker.kind.fuel_capacity() > 0 && target.kind.fuel_capacity() > 0))
                    && (city.is_some() || self.unit_visible_to(attacker.owner, target))
                    && self.combat_target_reachable(attacker.kind, target.kind, x, y)
            })
            .cloned()
            .collect();
        let city_action = city.is_some();
        let city_target = city.as_ref().and_then(|city| {
            if precision {
                self.choose_precision_city_bombard_target(city)
            } else {
                self.choose_city_bombard_target(city, &raw_targets)
            }
        });
        let targets: Vec<_> = if city_action {
            match &city_target {
                Some(CityBombardTarget::Unit(target)) => vec![target.clone()],
                _ => Vec::new(),
            }
        } else {
            raw_targets
        };
        let terrain_targets = bombardable_terrain_improvements(self, x, y);
        let has_unit_targets = !targets.is_empty();
        if !has_unit_targets && !city_action && terrain_targets.is_empty() {
            return MoveResult::Blocked;
        }
        // The preserved Civ III unit record stores bombard strength separately
        // from its ordinary attack value (a Bomber has attack 0 and bombard
        // strength 12). Feed that explicit content mapping through the shared
        // selected-ruleset tired-attack/effect pipeline.
        let attack = self.attack_power_against_with_base_for_action(
            &attacker,
            x,
            y,
            mapped_bombard_strength,
            "Bombard",
        );
        let base_rate = attacker.kind.bombard_rate();
        let rate =
            if crate::sim::combat_rules::selected().damage_reduces_bombard_rate && base_rate > 0 {
                (base_rate * attacker.hp / attacker.max_hit_points()).max(1)
            } else {
                base_rate
            };
        let mut total_damage = 0;
        let mut units_hit = 0;
        let mut units_killed = 0;
        let mut building_destroyed = None;
        let mut population_lost = 0;
        let mut observed_targets = Vec::new();
        let is_fixed_wing = attacker.kind.is_fixed_wing_aircraft();
        let is_cruise_missile = attacker.kind == UnitKind::CruiseMissile;
        let mut air_bombard_defender: Option<(Unit, i32, i32)> = None;

        // Freeciv's unit_bombard() walks every unit on the target tile and
        // fires one rate of attacks at each reachable defender. ACTION_BOMBARD
        // is marked non-lethal by the pinned engine's ruleset loader.
        for target in targets {
            let defense = self.defense_power_against(&target, x, y, attacker.kind);
            let (attack_firepower, _) = self.combat_firepower(&attacker, &target);
            let mut hp = target.hp;
            let mut hp_steps = vec![(attacker.hp, hp)];
            let lethal = attacker.kind.has_lethal_bombard_against(target.kind);
            for _ in 0..rate {
                if self.rand_below(attack + defense) < attack {
                    hp = if lethal {
                        (hp - attack_firepower).max(0)
                    } else {
                        (hp - attack_firepower).max(1)
                    };
                    hp_steps.push((attacker.hp, hp));
                    if hp == 0 {
                        break;
                    }
                }
            }
            let dealt = target.hp - hp;
            total_damage += dealt;
            if dealt > 0 {
                units_hit += 1;
            }
            if hp == 0 {
                units_killed += 1;
                self.remove_units_with_cargo_because(
                    vec![target.id],
                    lifecycle::UnitRemoval::Combat,
                );
            } else if let Some(defender) = self.units.iter_mut().find(|unit| unit.id == target.id) {
                defender.hp = hp;
            }
            self.sync_army_damage(target.id);
            if self.visual_events.enabled {
                if is_fixed_wing || is_cruise_missile {
                    if air_bombard_defender.is_none() || dealt > 0 {
                        air_bombard_defender = Some((target.clone(), target.hp, hp));
                    }
                } else {
                    self.publish_visual_event(VisualEvent::Combat(Box::new(
                        presentation::CombatVisual {
                            attacker: attacker.clone(),
                            defender: target.clone(),
                            hp: hp_steps,
                            city: self.city_at(x, y).cloned(),
                            bombard_effects: attacker.kind.has_bombard_effects(),
                            battle_ended: true,
                        },
                    )));
                }
            }
            observed_targets.push((target.kind, target.owner));
        }

        if let Some(
            target @ (CityBombardTarget::Population(_) | CityBombardTarget::Building(_, _)),
        ) = city_target.as_ref()
        {
            let Some(city) = city.as_ref() else {
                unreachable!("city bombard target requires its city");
            };
            let defense = city_object_bombard_defense(self, city, &attacker);
            for _ in 0..rate {
                if attack + defense > 0 && self.rand_below(attack + defense) < attack {
                    match *target {
                        CityBombardTarget::Population(city_id) => {
                            let before_size =
                                self.city_by_id(city_id).map_or(0, |current| current.size);
                            self.reduce_city_population_from_bombard(city_id, attacker.owner);
                            let after_size =
                                self.city_by_id(city_id).map_or(0, |current| current.size);
                            population_lost += (before_size - after_size).max(0);
                        }
                        CityBombardTarget::Building(city_id, building) => {
                            if let Some(city) =
                                self.cities.iter_mut().find(|city| city.id == city_id)
                            {
                                city.buildings.retain(|&current| current != building);
                                city.forget_culture_building(building);
                                building_destroyed = Some(building.data().name);
                            }
                            if attacker.owner == HUMAN || city.owner == HUMAN {
                                self.add_event(format!(
                                    "{}'s {} was destroyed by bombardment.",
                                    city.name,
                                    building.data().name
                                ));
                            }
                        }
                        CityBombardTarget::Unit(_) => unreachable!("unit target handled above"),
                    }
                    break;
                }
            }
        }

        let mut terrain_damage = Vec::new();
        if !has_unit_targets && !city_action && !terrain_targets.is_empty() {
            // Prima's PTW guide gives terrain improvements 16 base defense,
            // one hit point, and the tile's applicable terrain defense bonus.
            // Its priority is railroad first, then roads plus
            // mine/irrigation, then other improvements. Conquests confirmation
            // of these exact priorities is still outstanding.
            for _ in 0..rate {
                let current_targets = bombardable_terrain_improvements(self, x, y);
                let defense = terrain_bombard_defense(self, x, y);
                if current_targets.is_empty() || attack + defense <= 0 {
                    break;
                }
                if self.rand_below(attack + defense) < attack {
                    for target in current_targets {
                        bombard_terrain_improvement(self, x, y, target);
                        terrain_damage.push(target);
                    }
                }
            }
        }

        let missile_expended = attacker.kind == UnitKind::CruiseMissile;
        if missile_expended {
            self.remove_units_with_cargo_because(
                vec![attacker.id],
                lifecycle::UnitRemoval::Expended,
            );
        } else if let Some(bomber) = self.units.iter_mut().find(|unit| unit.id == attacker.id) {
            bomber.moves_left = 0;
            bomber.moved = true;
            bomber.state = UnitState::Idle;
            bomber.work_left = 0;
        }
        if self.visual_events.enabled && (is_fixed_wing || is_cruise_missile) {
            let kind = if is_cruise_missile {
                presentation::AirMissionVisualKind::CruiseMissileStrike {
                    defender: air_bombard_defender,
                    city: city.clone().map(Box::new),
                }
            } else {
                presentation::AirMissionVisualKind::Bombard {
                    defender: air_bombard_defender,
                    city: city.clone().map(Box::new),
                }
            };
            self.publish_visual_event(VisualEvent::AirMission(Box::new(
                presentation::AirMissionVisual {
                    aircraft: attacker.clone(),
                    from: (attacker.x, attacker.y),
                    to: (x, y),
                    kind,
                },
            )));
        }
        if self.trace.wants::<military_observation::BombardEvent>() {
            let target_kind = if city_action {
                match city_target {
                    Some(CityBombardTarget::Unit(_)) => "city_unit",
                    Some(CityBombardTarget::Building(_, _)) => "city_building",
                    Some(CityBombardTarget::Population(_)) => "city_population",
                    None => "city_empty",
                }
            } else if has_unit_targets {
                "field_units"
            } else {
                "terrain"
            };
            let target_owner = city
                .as_ref()
                .map(|c| c.owner)
                .or_else(|| observed_targets.first().map(|&(_, owner)| owner))
                .or_else(|| self.tile(x, y).owner);
            self.trace_bombard(military_observation::BombardEvent {
                attacker_id: attacker.id,
                attacker_owner: attacker.owner,
                attacker_kind: attacker.kind.name(),
                attacker_role: military_observation::military_unit_role(attacker.kind),
                precision,
                tile: (x, y),
                city_id: city.as_ref().map(|c| c.id),
                target_owner,
                target_kind,
                intercepted: false,
                hp_damage: total_damage,
                units_hit,
                units_killed,
                building_destroyed,
                population_lost,
                terrain_destroyed: terrain_damage.iter().map(|t| t.name()).collect(),
                missile_expended,
            });
        }
        if attacker.owner == HUMAN || observed_targets.iter().any(|(_, owner)| *owner == HUMAN) {
            if !terrain_damage.is_empty() {
                let names = terrain_damage
                    .iter()
                    .map(|target| target.name())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.add_event(format!(
                    "A {} destroyed {names} with bombardment.",
                    attacker.kind.name(),
                ));
            } else if observed_targets.is_empty() {
                if let Some(city) = self.city_at(x, y) {
                    self.add_event(format!(
                        "A {} bombarded {}.",
                        attacker.kind.name(),
                        city.name
                    ));
                }
            } else {
                let names = observed_targets
                    .iter()
                    .map(|(kind, _)| kind.name())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.add_event(format!(
                    "A {} dealt {total_damage} damage to {names}.",
                    attacker.kind.name(),
                ));
            }
        }
        MoveResult::Fought
    }

    #[cfg(test)]
    pub(super) fn restore_aircraft(&mut self) {
        for owner in 0..self.players.len() {
            let ids: Vec<_> = self
                .player_units_ordered(owner)
                .filter(|unit| unit.kind.is_aircraft())
                .map(|unit| unit.id)
                .collect();
            for id in ids {
                self.restore_aircraft_unit(id);
            }
            self.finish_aircraft_fuel_deaths(owner);
        }
    }

    pub(super) fn finish_aircraft_fuel_deaths(&mut self, owner: usize) {
        let dead: Vec<_> = self
            .player_units_ordered(owner)
            .filter(|unit| {
                unit.kind.fuel_capacity() > 0
                    && !unit.kind.is_fixed_wing_aircraft()
                    && unit.fuel == 0
            })
            .map(|unit| (unit.id, unit.kind))
            .collect();
        for (id, kind) in dead {
            if !self.units.iter().any(|unit| unit.id == id) {
                continue;
            }
            self.remove_units_with_cargo_because(vec![id], lifecycle::UnitRemoval::FuelExhausted);
            if owner == HUMAN {
                self.add_event(format!("Your {} crashed.", kind.name()));
            }
        }
    }

    pub(super) fn restore_aircraft_unit(&mut self, id: u32) {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return;
        };
        let max_fuel = unit.kind.fuel_capacity();
        if unit.kind.is_fixed_wing_aircraft() {
            // Civ III fixed-wing aircraft are base-only, heal at base, and never
            // auto-fly or crash from fuel exhaustion.
            let hp = unit.hp + self.unit_hitpoint_gain_at(&unit, unit.x, unit.y);
            if let Some(live) = self.units.iter_mut().find(|actor| actor.id == id) {
                live.hp = hp.max(1);
                live.moved = false;
                live.paradropped = false;
                live.air_superiority_shot = false;
                live.fuel = max_fuel;
            }
            return;
        }
        let site = self.refuel_site(unit.owner, unit.x, unit.y);
        let refueled = site || self.aboard_refueling_transport(&unit);
        // player_restore_units calls unit_gain_hitpoints before refueling.
        // The same ruleset effects now drive aircraft and other unit recovery.
        let hp = unit.hp + self.unit_hitpoint_gain_at(&unit, unit.x, unit.y);
        if let Some(live) = self.units.iter_mut().find(|actor| actor.id == id) {
            live.hp = hp;
            live.moved = false;
            live.paradropped = false;
            live.air_superiority_shot = false;
            if refueled {
                live.fuel = max_fuel;
            }
        }
        if hp <= 0 {
            self.remove_units_with_cargo_because(vec![id], lifecycle::UnitRemoval::Attrition);
            if unit.owner == HUMAN {
                self.add_event(format!("Your {} crashed.", unit.kind.name()));
            }
            return;
        }
        if !refueled
            && max_fuel > 0
            && unit.fuel <= 1
            && self.boarding_target(&unit, unit.x, unit.y).is_some()
        {
            self.board_or_deboard(id);
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .fuel = max_fuel;
            return;
        }
        if !refueled
            && max_fuel > 0
            && unit.fuel <= 1
            && unit.moves_left > 0
            && let Some(mut map) =
                super::fuel_paths::FuelPaths::new(self, &unit, self.players[unit.owner].ai)
        {
            let destination = map
                .reachable_within(self, unit.moves_left, false, true)
                .into_iter()
                .find(|&(x, y, _)| {
                    !self
                        .tile_units_ordered(x, y)
                        .any(|other| !self.allied(unit.owner, other.owner))
                        && (self.refuel_site(unit.owner, x, y)
                            || self.boarding_target(&unit, x, y).is_some())
                });
            if let Some((x, y, _)) = destination
                && let Some(path) = map.path(self, x, y)
            {
                self.follow_source_ai_path(id, path);
                if let Some(live) = self.unit_by_id(id).cloned() {
                    if (live.x, live.y) == (x, y)
                        && self.boarding_target(&live, x, y).is_some()
                        && !self.refuel_site(live.owner, x, y)
                    {
                        self.board_or_deboard(id);
                    }
                    if let Some(actor) = self.units.iter_mut().find(|actor| actor.id == id) {
                        actor.state = UnitState::Idle;
                    }
                }
            }
        }

        let Some(index) = self.unit_index_by_id(id) else {
            return;
        };
        let unit = &self.units[index];
        let refueled =
            self.refuel_site(unit.owner, unit.x, unit.y) || self.aboard_refueling_transport(unit);
        let unit = &mut self.units[index];
        unit.fuel = if refueled {
            max_fuel
        } else {
            unit.fuel.saturating_sub(1)
        };
    }
}

#[cfg(test)]
#[path = "aviation/tests.rs"]
mod tests;
