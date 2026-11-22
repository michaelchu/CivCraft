//! Territory ownership and border interaction rules.

use super::action_rules::SimulationRequirementContext;
use super::*;
use crate::sim::culture::city_culture_border_radius;
use std::collections::HashMap;

#[cfg(test)]
mod tests;

const CITY_MAP_MIN_RADIUS_SQ: i64 = 5;
const CITY_MAP_MAX_RADIUS_SQ: i64 = 26;

/// Borders and land claims follow the Civ III culture model; the ruleset has no
/// border settings. Only the city work radius is read from it here.
#[derive(Clone, Copy, Debug)]
struct BorderRules {
    initial_city_radius_sq: i64,
}

impl BorderRules {
    fn selected() -> Self {
        static RULES: std::sync::OnceLock<BorderRules> = std::sync::OnceLock::new();
        *RULES.get_or_init(|| Self {
            initial_city_radius_sq: crate::rules::engine_rules::INITIAL_CITY_RADIUS_SQ,
        })
    }

    fn city_work_radius_sq(self, simulation: &Simulation, city: &City) -> i64 {
        let context =
            SimulationRequirementContext::for_city_at(simulation, city, Some((city.x, city.y)));
        let bonus = context.effect(EffectKind::CityRadiusSq);
        (self.initial_city_radius_sq + bonus).clamp(CITY_MAP_MIN_RADIUS_SQ, CITY_MAP_MAX_RADIUS_SQ)
    }
}

#[derive(Clone, Debug)]
struct BorderSource {
    city_id: u32,
    owner: usize,
    tile_index: usize,
    x: i32,
    y: i32,
    /// Culture border radius: 1 is the 3×3 square, larger values the fat X.
    radius: i32,
    /// Local culture score; breaks ties between equally distant cities.
    culture: i64,
}

impl BorderSource {
    fn contains(&self, wrap: MapWrap, tile_index: usize) -> bool {
        wrap.civ3_border_shape_contains((self.x, self.y), wrap.position(tile_index), self.radius)
    }
}

#[derive(Clone, Copy, Debug)]
struct BorderClaim {
    city_id: u32,
    owner: usize,
    /// Contest key; the smallest wins: culture distance, then higher culture,
    /// then the older (lower id) city.
    key: (i32, i64, u32),
}

impl Simulation {
    pub(super) fn city_work_radius_sq(&self, city: &City) -> i64 {
        BorderRules::selected().city_work_radius_sq(self, city)
    }

    /// Culture border radius and the number of tiles the city currently holds.
    pub(crate) fn city_border_extent(&self, city_id: u32) -> Option<(i32, usize)> {
        let city = self.city_by_id(city_id)?;
        let radius = city_culture_border_radius(self.city_culture_score(city));
        let tiles = self
            .tiles
            .iter()
            .filter(|tile| tile.border_city == Some(city_id))
            .count();
        Some((radius, tiles))
    }

    pub(crate) fn displayed_tile_owner(&self, x: i32, y: i32) -> Option<usize> {
        let tile = self.tile(x, y);
        if tile.visible {
            tile.owner
        } else if tile.seen {
            tile.last_seen_owner
        } else {
            None
        }
    }

    pub(super) fn remember_visible_borders(&mut self) {
        for tile in self.tiles.iter_mut().filter(|tile| tile.visible) {
            tile.last_seen_owner = tile.owner;
        }
    }

    pub(super) fn rebuild_city_ownership(&mut self) {
        self.rebuild_city_ownership_inner(None);
    }

    pub(super) fn rebuild_city_ownership_for_city_event(&mut self, city_id: u32) {
        self.rebuild_city_ownership_inner(Some(city_id));
    }

    fn rebuild_city_ownership_inner(&mut self, city_event: Option<u32>) {
        self.refresh_ai_ferries();
        self.refresh_ai_guards();
        self.city_governors.retain(|id, _| {
            self.cities
                .iter()
                .any(|city| city.id == *id && city.owner == HUMAN)
        });
        self.city_rapture
            .retain(|id, _| self.cities.iter().any(|city| city.id == *id));
        self.ai_settlement_targets.retain(|id, reservation| {
            self.units
                .iter()
                .any(|unit| unit.id == *id && unit.owner == reservation.owner)
        });
        self.ai_ground_targets
            .retain(|_, id| self.cities.iter().any(|city| city.id == *id));

        let sources: Vec<_> = self
            .cities
            .iter()
            .map(|city| BorderSource {
                city_id: city.id,
                owner: city.owner,
                tile_index: self.map_wrap.index(city.x, city.y),
                x: city.x,
                y: city.y,
                radius: city_culture_border_radius(self.city_culture_score(city)),
                culture: self.city_culture_score(city),
            })
            .collect();
        let city_center_by_tile: HashMap<_, _> = sources
            .iter()
            .map(|source| (source.tile_index, source.city_id))
            .collect();
        let mut claims: Vec<Option<BorderClaim>> = vec![None; self.tiles.len()];

        // Civ III land claim: a city's border covers every tile in its culture
        // shape, land or sea, seen or not. Of the cities whose shape reaches a
        // tile, the nearest by culture distance owns it; equally distant cities
        // go to the one with more culture, then to the older city. Nothing
        // depends on earlier ownership or scan order, so the shared edge is the
        // line of equal distance and only a larger culture moves it.
        for source in &sources {
            for (tile_index, claim_slot) in claims.iter_mut().enumerate() {
                if !source.contains(self.map_wrap, tile_index) {
                    continue;
                }
                let is_center = tile_index == source.tile_index;
                if !is_center
                    && city_center_by_tile
                        .get(&tile_index)
                        .is_some_and(|city_id| *city_id != source.city_id)
                {
                    continue;
                }
                let rank = if is_center {
                    -1
                } else {
                    let (x, y) = self.map_wrap.position(tile_index);
                    self.map_wrap.civ3_culture_rank(source.x, source.y, x, y)
                };
                let key = (rank, -source.culture, source.city_id);
                if claim_slot.is_none_or(|claim| key < claim.key) {
                    *claim_slot = Some(BorderClaim {
                        city_id: source.city_id,
                        owner: source.owner,
                        key,
                    });
                }
            }
        }

        let recipients: Vec<_> = (0..self.players.len())
            .map(|owner| self.vision_recipients(owner))
            .collect();
        let mut discovered = vec![vec![false; self.tiles.len()]; self.players.len()];
        let mut discovered_any = false;
        for (tile_index, (tile, claim)) in self.tiles.iter_mut().zip(claims).enumerate() {
            tile.border_city = claim.map(|claim| claim.city_id);
            tile.owner = claim.map(|claim| claim.owner);
            if let Some(claim) = claim {
                // A claim reveals its terrain to the owner and allies.
                for &viewer in &recipients[claim.owner] {
                    if !tile.known_by[viewer] || (viewer == HUMAN && !tile.seen) {
                        discovered[viewer][tile_index] = true;
                        discovered_any = true;
                    }
                    tile.known_by.set(viewer, true);
                    if viewer == HUMAN {
                        tile.seen = true;
                        tile.last_seen_owner = tile.owner;
                    }
                }
            }
        }
        // Claims discover terrain without granting sight of units.
        if discovered_any {
            self.remember_render_map(&discovered);
        }
        self.remember_visible_borders();
        if let Some(city_id) = city_event {
            self.arrange_city_event_workers(city_id);
        } else {
            self.reconcile_city_workers();
        }
    }
}
