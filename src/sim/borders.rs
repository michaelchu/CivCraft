//! Territory ownership and border interaction rules.

use super::action_rules::SimulationRequirementContext;
use super::*;
use crate::rules::freeciv::{
    EffectContext, EffectRecord, Requirement, RequirementContext, RequirementEvaluationError,
    RequirementRange, RequirementValue, RulesetData, effect_value,
};
use std::collections::HashMap;
use std::sync::OnceLock;

#[cfg(test)]
mod tests;

const CITY_MAP_MIN_RADIUS_SQ: i64 = 5;
const CITY_MAP_MAX_RADIUS_SQ: i64 = 26;

#[derive(Clone, Copy, Debug)]
struct BorderRules {
    city_radius_sq: i64,
    city_size_effect: i64,
    permanent_radius_sq: i64,
    initial_city_radius_sq: i64,
}

impl BorderRules {
    fn selected() -> Self {
        let rules = RulesetData::campaign_default();
        Self {
            city_radius_sq: ruleset_integer_parameter(rules, "borders", "radius_sq_city"),
            city_size_effect: ruleset_integer_parameter(rules, "borders", "size_effect"),
            permanent_radius_sq: ruleset_integer_parameter(
                rules,
                "borders",
                "radius_sq_city_permanent",
            ),
            initial_city_radius_sq: ruleset_integer_parameter(
                rules,
                "civstyle",
                "init_city_radius_sq",
            ),
        }
    }

    fn city_border_radius_sq(self, city: &City) -> i64 {
        self.city_radius_sq
            + i64::from(city.size.min(CITY_MAP_MAX_RADIUS_SQ as i32)) * self.city_size_effect
    }

    fn city_work_radius_sq(self, simulation: &Simulation, city: &City) -> i64 {
        let context =
            SimulationRequirementContext::for_city_at(simulation, city, Some((city.x, city.y)));
        let bonus = effect_value(
            &RulesetData::campaign_default().effects,
            "City_Radius_Sq",
            &context,
        )
        .unwrap_or_else(|error| panic!("city radius effect evaluation failed: {error:?}"));
        (self.initial_city_radius_sq + bonus).clamp(CITY_MAP_MIN_RADIUS_SQ, CITY_MAP_MAX_RADIUS_SQ)
    }

    fn city_border_strength(self, simulation: &Simulation, city: &City) -> i64 {
        let context =
            SimulationRequirementContext::for_city_at(simulation, city, Some((city.x, city.y)));
        let percent = effect_value(
            &RulesetData::campaign_default().effects,
            "Border_Strength_Pct",
            &context,
        )
        .unwrap_or_else(|error| panic!("border strength effect evaluation failed: {error:?}"));
        (i64::from(city.size) + 2) * (100 + percent) / 100
    }
}

fn ruleset_integer_parameter(rules: &RulesetData, section: &str, key: &str) -> i64 {
    rules
        .parameters
        .iter()
        .find(|parameter| {
            parameter.file.eq_ignore_ascii_case("game.ruleset")
                && parameter.section.eq_ignore_ascii_case(section)
                && parameter.key.eq_ignore_ascii_case(key)
        })
        .unwrap_or_else(|| panic!("selected C2C3 ruleset is missing [{section}].{key}"))
        .integer_value()
        .unwrap_or_else(|error| panic!("invalid selected C2C3 [{section}].{key}: {error}"))
        .unwrap_or_else(|| panic!("selected C2C3 [{section}].{key} has no integer value"))
}

#[derive(Debug)]
struct BorderRegions {
    region_by_tile: Vec<usize>,
    sizes: Vec<usize>,
    surrounder: Vec<Option<usize>>,
}

impl BorderRegions {
    fn from_simulation(simulation: &Simulation) -> Self {
        let mut region_by_tile = vec![usize::MAX; simulation.tiles.len()];
        let mut sizes = Vec::new();
        for start in 0..simulation.tiles.len() {
            if region_by_tile[start] != usize::MAX {
                continue;
            }
            let is_water = simulation.tiles[start].terrain.is_water();
            let region = sizes.len();
            sizes.push(0);
            region_by_tile[start] = region;
            let mut pending = vec![start];
            while let Some(index) = pending.pop() {
                sizes[region] += 1;
                let (x, y) = simulation.map_wrap.position(index);
                for (nx, ny) in simulation.map_wrap.civ3_neighbors(x, y) {
                    let neighbor = simulation.map_wrap.index(nx, ny);
                    if region_by_tile[neighbor] == usize::MAX
                        && simulation.tiles[neighbor].terrain.is_water() == is_water
                    {
                        region_by_tile[neighbor] = region;
                        pending.push(neighbor);
                    }
                }
            }
        }

        // Freeciv's Region Surrounded requirement is true only when one
        // distinct adjacent region surrounds the candidate region. Full map
        // terrain is available in this simulation, so no unknown-region
        // three-state case is needed here.
        let mut surrounder = vec![None; sizes.len()];
        let mut mixed = vec![false; sizes.len()];
        for (index, _) in simulation.tiles.iter().enumerate() {
            let region = region_by_tile[index];
            let (x, y) = simulation.map_wrap.position(index);
            for (nx, ny) in simulation.map_wrap.civ3_neighbors(x, y) {
                let neighbor_region = region_by_tile[simulation.map_wrap.index(nx, ny)];
                if neighbor_region == region {
                    continue;
                }
                if let Some(previous) = surrounder[region] {
                    mixed[region] |= previous != neighbor_region;
                } else {
                    surrounder[region] = Some(neighbor_region);
                }
            }
        }
        for (region, is_mixed) in mixed.into_iter().enumerate() {
            if is_mixed {
                surrounder[region] = None;
            }
        }
        Self {
            region_by_tile,
            sizes,
            surrounder,
        }
    }
}

#[derive(Clone, Debug)]
struct BorderSource {
    city_id: u32,
    owner: usize,
    tile_index: usize,
    x: i32,
    y: i32,
    radius_sq: i64,
    city_work_radius_sq: i64,
    strength: i64,
    distances: Vec<i32>,
}

impl BorderSource {
    fn strength_at(&self, distance: i32) -> i64 {
        if distance == 0 {
            return i64::MAX;
        }
        let distance_sq = i64::from(distance) * i64::from(distance);
        self.strength * self.strength / distance_sq
    }
}

#[derive(Clone, Copy, Debug)]
struct BorderClaim {
    city_id: u32,
    owner: usize,
    strength: i64,
}

/// The shared requirement evaluator resolves ordinary universals. This narrow
/// adapter supplies the two-tile context required by selected C2C3
/// Tile_Claimable effects and handles only their tile relation/distance/region
/// universals.
struct BorderTileClaimContext<'a> {
    simulation: &'a Simulation,
    ordinary: SimulationRequirementContext<'a>,
    regions: &'a BorderRegions,
    target: (i32, i32),
    source: (i32, i32),
}

impl BorderTileClaimContext<'_> {
    fn region_at(&self, (x, y): (i32, i32)) -> usize {
        self.regions.region_by_tile[self.simulation.map_wrap.index(x, y)]
    }

    fn tile_relation(&self, requirement: &Requirement) -> Result<bool, RequirementEvaluationError> {
        let RequirementValue::String(relation) = &requirement.name else {
            return Err(unsupported_border_requirement(requirement));
        };
        let target_region = self.region_at(self.target);
        let source_region = self.region_at(self.source);
        let mut adjacent = Vec::new();
        if matches!(requirement.range, RequirementRange::Adjacent) {
            adjacent.extend(
                self.simulation
                    .map_wrap
                    .civ3_neighbors(self.target.0, self.target.1)
                    .map(|position| self.region_at(position)),
            );
        }
        match relation.to_ascii_lowercase().as_str() {
            "same region"
                if matches!(
                    requirement.range,
                    RequirementRange::Tile | RequirementRange::Adjacent
                ) =>
            {
                Ok(target_region == source_region
                    || adjacent.into_iter().any(|region| region == source_region))
            }
            "only other region" if requirement.range == RequirementRange::Adjacent => Ok(adjacent
                .into_iter()
                .all(|region| region == target_region || region == source_region)),
            "region surrounded"
                if matches!(
                    requirement.range,
                    RequirementRange::Tile | RequirementRange::Adjacent
                ) =>
            {
                Ok(
                    self.regions.surrounder[target_region] == Some(source_region)
                        || adjacent
                            .into_iter()
                            .any(|region| self.regions.surrounder[region] == Some(source_region)),
                )
            }
            _ => Err(unsupported_border_requirement(requirement)),
        }
    }

    fn max_distance_sq(
        &self,
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        if requirement.range != RequirementRange::Tile {
            return Err(unsupported_border_requirement(requirement));
        }
        let maximum = integer_requirement_value(requirement)
            .ok_or_else(|| unsupported_border_requirement(requirement))?;
        let distance = self.simulation.map_wrap.civ3_distance(
            self.target.0,
            self.target.1,
            self.source.0,
            self.source.1,
        );
        let distance_sq = i64::from(distance) * i64::from(distance);
        Ok(distance_sq <= maximum)
    }

    fn max_region_tiles(
        &self,
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        let maximum = integer_requirement_value(requirement)
            .ok_or_else(|| unsupported_border_requirement(requirement))?;
        let target_region = self.region_at(self.target);
        let actual = match requirement.range {
            RequirementRange::Continent => self.regions.sizes[target_region] as i64,
            RequirementRange::Adjacent => {
                let same_region_neighbors = self
                    .simulation
                    .map_wrap
                    .civ3_neighbors(self.target.0, self.target.1)
                    .filter(|&(x, y)| self.region_at((x, y)) == target_region)
                    .count();
                1 + same_region_neighbors as i64
            }
            _ => return Err(unsupported_border_requirement(requirement)),
        };
        Ok(actual <= maximum)
    }
}

impl RequirementContext for BorderTileClaimContext<'_> {
    fn is_present(&self, requirement: &Requirement) -> Result<bool, RequirementEvaluationError> {
        match requirement.kind.to_ascii_lowercase().as_str() {
            "tilerel" => self.tile_relation(requirement),
            "maxdistancesq" => self.max_distance_sq(requirement),
            "maxregiontiles" => self.max_region_tiles(requirement),
            _ => self.ordinary.is_present(requirement),
        }
    }
}

impl EffectContext for BorderTileClaimContext<'_> {
    fn multiplier_percent(&self, multiplier: &str) -> Option<i64> {
        self.ordinary.multiplier_percent(multiplier)
    }
}

fn integer_requirement_value(requirement: &Requirement) -> Option<i64> {
    match &requirement.name {
        RequirementValue::Integer(value) => Some(*value),
        RequirementValue::String(value) => value.parse().ok(),
        RequirementValue::Bool(_) => None,
    }
}

fn unsupported_border_requirement(requirement: &Requirement) -> RequirementEvaluationError {
    RequirementEvaluationError::Unsupported {
        kind: requirement.kind.clone(),
        name: requirement.name.clone(),
    }
}

fn tile_claimable(
    simulation: &Simulation,
    regions: &BorderRegions,
    owner: usize,
    target: (i32, i32),
    source: (i32, i32),
) -> bool {
    if target == source {
        return true;
    }
    let ordinary = SimulationRequirementContext::for_player_at(simulation, owner, Some(target));
    let context = BorderTileClaimContext {
        simulation,
        ordinary,
        regions,
        target,
        source,
    };
    effect_value(selected_tile_claim_effects(), "Tile_Claimable", &context)
        .unwrap_or_else(|error| panic!("tile claim effect evaluation failed: {error:?}"))
        > 0
}

fn selected_tile_claim_effects() -> &'static [EffectRecord] {
    static EFFECTS: OnceLock<Vec<EffectRecord>> = OnceLock::new();
    EFFECTS.get_or_init(|| {
        RulesetData::campaign_default()
            .effects
            .iter()
            .filter(|effect| effect.kind.eq_ignore_ascii_case("Tile_Claimable"))
            .cloned()
            .collect()
    })
}

impl Simulation {
    pub(super) fn city_work_radius_sq(&self, city: &City) -> i64 {
        BorderRules::selected().city_work_radius_sq(self, city)
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
        self.city_rapture
            .retain(|id, _| self.cities.iter().any(|city| city.id == *id));
        self.ai_settlement_targets.retain(|id, reservation| {
            self.units
                .iter()
                .any(|unit| unit.id == *id && unit.owner == reservation.owner)
        });
        self.ai_ground_targets
            .retain(|_, id| self.cities.iter().any(|city| city.id == *id));

        let rules = BorderRules::selected();
        let regions = BorderRegions::from_simulation(self);
        let mut sources: Vec<_> = self
            .cities
            .iter()
            .map(|city| BorderSource {
                city_id: city.id,
                owner: city.owner,
                tile_index: self.map_wrap.index(city.x, city.y),
                x: city.x,
                y: city.y,
                radius_sq: rules.city_border_radius_sq(city),
                city_work_radius_sq: rules.city_work_radius_sq(self, city),
                strength: rules.city_border_strength(self, city),
                distances: self.map_wrap.civ3_distances_from(city.x, city.y),
            })
            .collect();
        // Freeciv map_calculate_borders() scans source tiles in map order.
        sources.sort_by_key(|source| source.tile_index);
        let source_index_by_city: HashMap<_, _> = sources
            .iter()
            .enumerate()
            .map(|(index, source)| (source.city_id, index))
            .collect();
        let city_center_by_tile: HashMap<_, _> = sources
            .iter()
            .map(|source| (source.tile_index, source.city_id))
            .collect();
        let mut claims = vec![None; self.tiles.len()];

        // Existing claims are kept so equal-strength challengers do not
        // displace the earlier claimant. Invalid, unknown, out-of-radius, or
        // no-longer-claimable claims are dropped before source processing.
        for (tile_index, tile) in self.tiles.iter().enumerate() {
            let Some(city_id) = tile.border_city else {
                continue;
            };
            let Some(&source_index) = source_index_by_city.get(&city_id) else {
                continue;
            };
            let source = &sources[source_index];
            let distance = source.distances[tile_index];
            let distance_sq = i64::from(distance) * i64::from(distance);
            if distance_sq > source.radius_sq
                || (distance != 0 && !tile.known_by[source.owner])
                || (distance != 0
                    && !tile_claimable(
                        self,
                        &regions,
                        source.owner,
                        self.map_wrap.position(tile_index),
                        (source.x, source.y),
                    ))
            {
                continue;
            }
            claims[tile_index] = Some(BorderClaim {
                city_id,
                owner: source.owner,
                strength: source.strength_at(distance),
            });
        }

        for source in &sources {
            for tile_index in 0..self.tiles.len() {
                let distance = source.distances[tile_index];
                let distance_sq = i64::from(distance) * i64::from(distance);
                if distance_sq > source.radius_sq {
                    continue;
                }
                if distance != 0
                    && city_center_by_tile
                        .get(&tile_index)
                        .is_some_and(|city_id| *city_id != source.city_id)
                {
                    continue;
                }
                if distance != 0 && !self.tiles[tile_index].known_by[source.owner] {
                    continue;
                }
                let target = self.map_wrap.position(tile_index);
                if !tile_claimable(self, &regions, source.owner, target, (source.x, source.y)) {
                    continue;
                }
                let old = claims[tile_index];
                if old.is_some_and(|claim| claim.city_id == source.city_id) {
                    continue;
                }
                if let Some(old_claim) = old
                    && distance != 0
                {
                    let old_source = &sources[source_index_by_city[&old_claim.city_id]];
                    let old_distance = old_source.distances[tile_index];
                    let old_distance_sq = i64::from(old_distance) * i64::from(old_distance);
                    if old_distance_sq <= old_source.city_work_radius_sq + rules.permanent_radius_sq
                    {
                        continue;
                    }
                    if source.strength_at(distance) <= old_claim.strength {
                        continue;
                    }
                }
                claims[tile_index] = Some(BorderClaim {
                    city_id: source.city_id,
                    owner: source.owner,
                    strength: source.strength_at(distance),
                });
            }
        }

        for (tile, claim) in self.tiles.iter_mut().zip(claims) {
            tile.border_city = claim.map(|claim| claim.city_id);
            tile.owner = claim.map(|claim| claim.owner);
        }
        self.remember_visible_borders();
        if let Some(city_id) = city_event {
            self.arrange_city_event_workers(city_id);
        } else {
            self.reconcile_city_workers();
        }
    }
}
