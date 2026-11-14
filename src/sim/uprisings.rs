//! Revolt risk and uprising resolution.

use super::{villages::BARBARIAN_ONSET, *};

#[derive(Clone, Copy)]
struct BarbarianRoleUnit {
    record: &'static crate::rules::freeciv::ContentRecord,
    runtime_kind: UnitKind,
}

impl Simulation {
    /// Resolve one selected Freeciv role record to a concrete Civ III runtime
    /// identity through the explicit unit-economics crosswalk. Multiple Civ III
    /// identities may share a C2C3 record; use the first roster identity for
    /// barbarian units, which have no civilization-specific replacement.
    fn barbarian_role_runtime_kind(
        record: &crate::rules::freeciv::ContentRecord,
    ) -> Option<UnitKind> {
        UnitKind::ALL_NATIVE.into_iter().find(|&kind| {
            match crate::sim::action_rules::freeciv_unit_economics_record(kind) {
                crate::sim::action_rules::UnitEconomicsRecordBinding::Mapped {
                    record: mapped_record,
                    ..
                } => mapped_record.section.eq_ignore_ascii_case(&record.section),
                crate::sim::action_rules::UnitEconomicsRecordBinding::Unsupported { .. } => false,
            }
        })
    }

    /// Freeciv `find_a_unit_type()` checks whether a tech-role unit can be
    /// built by more than two non-barbarian players, then falls back to the
    /// non-tech role if none qualify. Its direct-build check uses player/world
    /// requirements, not city-local requirements.
    fn barbarian_role_record_buildable_by(
        &self,
        record: &'static crate::rules::freeciv::ContentRecord,
        owner: usize,
    ) -> bool {
        if owner >= self.civilization_count || !self.player_alive(owner) {
            return false;
        }
        let flags = record.string_list_field("flags");
        if flags.is_err()
            || flags.ok().flatten().is_some_and(|flags| {
                flags
                    .iter()
                    .any(|flag| flag.eq_ignore_ascii_case("NoBuild"))
            })
        {
            return false;
        }

        let identity_mapped = UnitKind::ALL_NATIVE.into_iter().any(|kind| {
            match crate::sim::action_rules::freeciv_unit_economics_record(kind) {
                crate::sim::action_rules::UnitEconomicsRecordBinding::Mapped {
                    record: mapped_record,
                    ..
                } => mapped_record.section.eq_ignore_ascii_case(&record.section),
                crate::sim::action_rules::UnitEconomicsRecordBinding::Unsupported { .. } => false,
            }
        });
        if !identity_mapped {
            return false;
        }

        // Freeciv decides direct-build eligibility from its selected unit
        // record's requirements and nation restrictions. Do not apply the
        // Civ III PRTO availability mask here: names and unique-unit policy
        // belong to the Civ III production catalog, while this role chooser
        // is selecting a C2C3 barbarian archetype.
        let player_requirements: Vec<_> = record
            .requirements
            .iter()
            .filter(|requirement| {
                matches!(
                    requirement.range,
                    crate::rules::freeciv::RequirementRange::Player
                        | crate::rules::freeciv::RequirementRange::Team
                        | crate::rules::freeciv::RequirementRange::Alliance
                        | crate::rules::freeciv::RequirementRange::World
                )
            })
            .cloned()
            .collect();
        let context = crate::sim::action_rules::SimulationRequirementContext::for_player_at(
            self, owner, None,
        );
        crate::rules::freeciv::requirements_met(&player_requirements, &context).unwrap_or(false)
    }

    /// Read selected C2C3 roles in ruleset order and retain only records with
    /// an explicit Civ III runtime mapping. Unmapped roles (including C2C3's
    /// Barbarian Leader, which has no Civ III roster identity) fail closed.
    fn barbarian_role_units(&self, role: &str) -> Vec<BarbarianRoleUnit> {
        let mut candidates = Vec::new();
        for record in &crate::rules::freeciv::RulesetData::campaign_default().content {
            if record.kind != crate::rules::freeciv::ContentKind::Unit {
                continue;
            }
            let roles = record
                .string_list_field("roles")
                .expect("selected C2C3 unit roles are valid")
                .unwrap_or_default();
            if !roles
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(role))
                || candidates.iter().any(|candidate: &BarbarianRoleUnit| {
                    candidate
                        .record
                        .section
                        .eq_ignore_ascii_case(&record.section)
                })
            {
                continue;
            }
            if let Some(runtime_kind) = Self::barbarian_role_runtime_kind(record) {
                candidates.push(BarbarianRoleUnit {
                    record,
                    runtime_kind,
                });
            }
        }
        candidates
    }

    fn find_barbarian_role_unit(&mut self, role: &str, tech_role: &str) -> Option<UnitKind> {
        let tech_candidates: Vec<_> = self
            .barbarian_role_units(tech_role)
            .into_iter()
            .filter(|candidate| {
                (0..self.civilization_count)
                    .filter(|&owner| {
                        self.barbarian_role_record_buildable_by(candidate.record, owner)
                    })
                    .count()
                    > 2
            })
            .collect();
        let candidates = if tech_candidates.is_empty() {
            self.barbarian_role_units(role)
        } else {
            tech_candidates
        };
        if candidates.is_empty() {
            return None;
        }
        let choice = self.rand_below(candidates.len() as i32) as usize;
        Some(candidates[choice].runtime_kind)
    }

    fn spawn_barbarian_unit(
        &mut self,
        kind: UnitKind,
        x: i32,
        y: i32,
        transport: Option<u32>,
    ) -> u32 {
        let id = self.spawn_unit(BARBARIANS, kind, x, y);
        if let Some(boat) = transport {
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .expect("new barbarian unit exists")
                .transport = Some(boat);
            self.record_transport_change(id, Some(boat));
        }
        id
    }

    pub(super) fn summon_barbarians(&mut self) {
        if self.turn < BARBARIAN_ONSET {
            return;
        }
        // Normal barbarian activity makes one attempt per 2,000 tiles.
        for _ in 0..(self.tiles.len() / 2000).max(1) {
            let (x, y) = self
                .map_wrap
                .position(self.rand_below(self.tiles.len() as i32) as usize);
            self.try_uprising_at(x, y);
        }
    }

    fn try_uprising_at(&mut self, x: i32, y: i32) {
        if matches!(
            self.tile(x, y).terrain,
            Terrain::Lake | Terrain::Glacier | Terrain::Tundra
        ) {
            return;
        }
        let Some(city) = self
            .cities
            .iter()
            .min_by_key(|city| self.map_distance(x, y, city.x, city.y))
        else {
            return;
        };
        if !(5..=8).contains(&self.map_distance(x, y, city.x, city.y)) {
            return;
        }
        let victim = city.owner;
        let city_name = city.name.clone();
        let Some((sx, sy)) = std::iter::once((x, y))
            .chain(self.map_wrap.civ3_neighbors(x, y))
            .find(|&(sx, sy)| !self.units.iter().any(|unit| (unit.x, unit.y) == (sx, sy)))
        else {
            return;
        };
        let tile = self.tile(sx, sy);
        let sea = tile.terrain.is_water();
        let seen = tile.visible;
        if sea {
            if !self.tiles.iter().enumerate().any(|(index, tile)| {
                let (tx, ty) = self.map_wrap.position(index);
                !tile.terrain.is_water() && self.map_distance(sx, sy, tx, ty) <= 4
            }) {
                return;
            }
        } else if !tile.known_by[victim] {
            return;
        }

        let city_count = self
            .cities
            .iter()
            .filter(|city| city.owner == victim)
            .count() as i32;
        if self.rand_below(30) >= city_count
            || self.rand_below(100) > self.players[victim].government.civil_war_chance()
        {
            return;
        }
        self.tile_mut(sx, sy).hut = false;
        let mut tier = 1;
        let mut threshold = 10;
        while threshold <= city_count {
            tier += 1;
            threshold += threshold * 12 / 10 + 10;
        }
        let count = self.rand_below(3) + tier * 2;
        if sea {
            let boat = self.find_barbarian_role_unit("BarbarianBoat", "BarbarianBoatTech");
            if let Some(boat) = boat.filter(|&kind| self.pf_native_tile(kind, sx, sy)) {
                let boat_id = self.spawn_unit(BARBARIANS, boat, sx, sy);
                let capacity = self.ruleset_transport_capacity(boat).unwrap_or(0);
                // Freeciv leaves one slot open for the barbarian leader. The
                // C2C3 leader role currently has no mapped Civ III identity,
                // so the reserved slot stays empty in this runtime.
                for _ in 0..(count.max(0) as usize).min(capacity.saturating_sub(1)) {
                    let Some(kind) =
                        self.find_barbarian_role_unit("BarbarianSea", "BarbarianSeaTech")
                    else {
                        break;
                    };
                    if self.ruleset_transport_pair_allowed_fail_closed(boat, kind) {
                        self.spawn_barbarian_unit(kind, sx, sy, Some(boat_id));
                    }
                }
            }
        } else {
            for _ in 0..count {
                let Some(kind) = self.find_barbarian_role_unit("Barbarian", "BarbarianTech") else {
                    break;
                };
                if self.pf_native_tile(kind, sx, sy) {
                    self.spawn_barbarian_unit(kind, sx, sy, None);
                }
            }
        }
        if victim == HUMAN && (!sea || seen) {
            self.add_event(if sea {
                format!("Sea raiders spotted near {city_name}!")
            } else {
                format!("Native unrest near {city_name}!")
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advanced_sea_raider_role_uses_c2c3_buildable_marines() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for owner in 0..game.civilization_count {
            game.players[owner].known = vec![Tech::AMPHIBIOUS_WARFARE];
        }
        let selected = game.find_barbarian_role_unit("BarbarianSea", "BarbarianSeaTech");
        let candidates = game.barbarian_role_units("BarbarianSeaTech");
        let eligibility = candidates
            .iter()
            .map(|candidate| {
                (
                    candidate.record.section.as_str(),
                    candidate.runtime_kind,
                    (0..game.civilization_count)
                        .map(|owner| {
                            (
                                game.players[owner].name.as_str(),
                                game.player_alive(owner),
                                candidate
                                    .runtime_kind
                                    .conquests_rule()
                                    .available_to(&game.players[owner].name),
                                game.barbarian_role_record_buildable_by(candidate.record, owner),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            selected,
            Some(UnitKind::Marine),
            "eligibility={eligibility:?}"
        );
    }
}
