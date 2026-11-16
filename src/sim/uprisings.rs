//! Revolt risk and uprising resolution.

use super::{villages::BARBARIAN_ONSET, *};

#[derive(Clone, Copy)]
struct BarbarianRoleUnit {
    runtime_kind: UnitKind,
}

impl Simulation {
    /// Civ III's native role bindings replace Freeciv's unit records here.
    /// The role chooser still prefers a technology-gated role when more than
    /// two living civilizations can build one, then falls back to the base
    /// role, matching the original barbarian selection policy.
    fn barbarian_role_buildable_by(&self, kind: UnitKind, owner: usize) -> bool {
        owner < self.civilization_count
            && self.player_alive(owner)
            // Barbarian archetypes are not filtered through a civilization's
            // unique-unit availability mask. Native role selection asks
            // whether the world has the unit's technology, just as the old
            // role chooser did for its shared C2C3 records.
            && !kind.has_no_build()
            && kind
                .required_tech()
                .is_none_or(|tech| self.has_tech(owner, tech))
    }

    /// Read native Civ III role bindings in catalog order. Units without an
    /// explicit role are intentionally absent; alias identities that share a
    /// native archetype are collapsed to the first binding so a unique unit
    /// does not duplicate the same barbarian choice. The native TOML is the
    /// only source for barbarian archetypes.
    fn barbarian_role_units(&self, role: &str) -> Vec<BarbarianRoleUnit> {
        let mut seen_profiles = std::collections::HashSet::new();
        crate::rules::native::unit_economics()
            .bindings
            .iter()
            .filter(|binding| {
                binding
                    .roles
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(role))
            })
            .filter(|binding| seen_profiles.insert(binding.profile.to_ascii_lowercase()))
            .filter_map(|binding| {
                UnitKind::from_conquests_id(&binding.civ3_unit_id)
                    .map(|runtime_kind| BarbarianRoleUnit { runtime_kind })
            })
            .collect()
    }

    fn find_barbarian_role_unit(&mut self, role: &str, tech_role: &str) -> Option<UnitKind> {
        let tech_candidates: Vec<_> = self
            .barbarian_role_units(tech_role)
            .into_iter()
            .filter(|candidate| {
                (0..self.civilization_count)
                    .filter(|&owner| {
                        self.barbarian_role_buildable_by(candidate.runtime_kind, owner)
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
                                game.barbarian_role_buildable_by(candidate.runtime_kind, owner),
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
