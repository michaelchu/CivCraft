//! Climate pressure and its effects on the game world.

use super::*;
use crate::rules::freeciv::{ContentKind, ContentRecord, RulesetData};

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(super) struct ClimatePressure {
    accumulated: i32,
    level: i32,
}

const MAP_AREA: i32 = MAP_W * MAP_H;

impl ClimatePressure {
    pub(super) fn new(area: usize) -> Self {
        Self {
            accumulated: 0,
            level: (area as i32 + 499) / 500,
        }
    }
}
impl Default for ClimatePressure {
    fn default() -> Self {
        Self {
            accumulated: 0,
            level: (MAP_AREA + 499) / 500,
        }
    }
}

impl Simulation {
    pub(super) fn worker_climate_urgency(&self, target: Contamination) -> i32 {
        let index = usize::from(target == Contamination::Fallout);
        let pressure = self.climate_pressure[index];
        // autoworkers.c: cached heating/cooling from the last environmental
        // update, not a recount after workers have already cleaned tiles.
        50 * self.climate_current[index] / ((pressure.level + 1) / 2).max(1) + pressure.accumulated
    }

    pub(super) fn update_climate(&mut self) {
        for (index, contamination) in [Contamination::Pollution, Contamination::Fallout]
            .into_iter()
            .enumerate()
        {
            let count = self
                .tiles
                .iter()
                .filter(|tile| tile.has_contamination(contamination))
                .count() as i32;
            // srv_main.c:update_environmental_upset, default percent=100.
            self.climate_current[index] = count;
            let mut pressure = self.climate_pressure[index];
            pressure.accumulated += count;
            if pressure.accumulated < pressure.level {
                pressure.accumulated = 0;
            } else {
                pressure.accumulated -= pressure.level;
                if self.rand_below((self.tiles.len() as i32 + 19) / 20) < pressure.accumulated {
                    let strength = self.map_wrap.width() / 10
                        + self.map_wrap.height() / 10
                        + pressure.accumulated * 5;
                    self.change_climate(contamination == Contamination::Pollution, strength);
                    pressure.accumulated = 0;
                    pressure.level += (self.tiles.len() as i32 + 999) / 1000;
                }
            }
            self.climate_pressure[index] = pressure;
        }
    }

    fn climate_target(&self, x: i32, y: i32, warming: bool) -> Option<Terrain> {
        let tile = self.tile(x, y);
        let wet = tile.river
            || self.map_wrap.civ3_neighbors(x, y).any(|(nx, ny)| {
                let adjacent = self.tile(nx, ny);
                adjacent.terrain.is_water() || adjacent.river
            });
        let terrain_name = super::movement_rules::c2c3_terrain_rule_name(tile.terrain)?;
        let record = Self::climate_terrain_record(terrain_name);
        let (wet_field, dry_field) = if warming {
            ("warmer_wetter_result", "warmer_drier_result")
        } else {
            ("cooler_wetter_result", "cooler_drier_result")
        };
        let preferred_field = if wet { wet_field } else { dry_field };
        let fallback_field = if wet { dry_field } else { wet_field };
        let climate_result = |field| {
            let result = record
                .string_field(field)
                .unwrap_or_else(|error| panic!("invalid {}.{field}: {error}", record.section))
                .unwrap_or_else(|| panic!("campaign terrain `{terrain_name}` lacks `{field}`"));
            Self::c2c3_climate_terrain(result)
        };
        // As in climate_change(), a missing preferred terrain result means
        // this terrain is not changed for its current moisture class; only a
        // present result rejected by local geometry permits trying fallback.
        let preferred = climate_result(preferred_field)?;
        if self.climate_change_allowed(x, y, preferred) {
            return Some(preferred);
        }
        let fallback = climate_result(fallback_field)?;
        self.climate_change_allowed(x, y, fallback)
            .then_some(fallback)
    }

    /// Climate changes use Freeciv's terrain-control parameters, while worker
    /// transforms keep the separate CivCraft legality helper in terrain_work.
    fn climate_change_allowed(&self, x: i32, y: i32, target: Terrain) -> bool {
        let Some(source_name) =
            super::movement_rules::c2c3_terrain_rule_name(self.tile(x, y).terrain)
        else {
            return false;
        };
        let Some(target_name) = super::movement_rules::c2c3_terrain_rule_name(target) else {
            return false;
        };
        let source = Self::climate_terrain_record(source_name);
        let target = Self::climate_terrain_record(target_name);
        let flags = |record: &ContentRecord| {
            record
                .string_list_field("flags")
                .unwrap_or_else(|error| panic!("invalid {}.flags: {error}", record.section))
                .unwrap_or_default()
        };
        let has_flag = |record: &ContentRecord, flag: &str| {
            flags(record)
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(flag))
        };
        if self.city_at(x, y).is_some() && has_flag(target, "NoCities") {
            return false;
        }

        let is_oceanic = |record: &ContentRecord| {
            record
                .string_field("class")
                .unwrap_or_else(|error| panic!("invalid {}.class: {error}", record.section))
                .is_some_and(|class| class.eq_ignore_ascii_case("Oceanic"))
        };
        let adjacent: Vec<_> = self.map_wrap.civ3_neighbors(x, y).collect();
        // Freeciv C2C3 uses ISO|HEX, but CivCraft uses Civ III's eight
        // directional slots. can_reclaim_ocean() counts absent/off-map slots
        // as non-ocean, so the denominator includes bounded-Y slots too.
        const DIRECTION_SLOTS: i32 = super::topology::CIV3_DIRS.len() as i32;
        let adjacent_ocean = adjacent
            .iter()
            .filter(|&&(nx, ny)| {
                super::movement_rules::c2c3_terrain_rule_name(self.tile(nx, ny).terrain)
                    .is_some_and(|name| is_oceanic(Self::climate_terrain_record(name)))
            })
            .count() as i32;

        if is_oceanic(source) && !is_oceanic(target) {
            let required =
                (Self::climate_parameter("ocean_reclaim_requirement") * DIRECTION_SLOTS + 99) / 100;
            if DIRECTION_SLOTS - adjacent_ocean < required {
                return false;
            }
        } else if !is_oceanic(source)
            && is_oceanic(target)
            && adjacent_ocean < Self::climate_parameter("land_channel_requirement")
        {
            // Freeciv compares this ruleset value as an absolute adjacent
            // ocean-tile count. C2C3 sets it to 10, so Civ III's eight slots
            // cannot channel land into ocean.
            return false;
        }

        let source_frozen = has_flag(source, "Frozen");
        let target_frozen = has_flag(target, "Frozen");
        let frozen_neighbors = adjacent
            .iter()
            .filter(|&&(nx, ny)| {
                super::movement_rules::c2c3_terrain_rule_name(self.tile(nx, ny).terrain)
                    .is_some_and(|name| has_flag(Self::climate_terrain_record(name), "Frozen"))
            })
            .count() as i32;
        if source_frozen && !target_frozen {
            if 100 - frozen_neighbors < Self::climate_parameter("thaw_requirement") {
                return false;
            }
        } else if !source_frozen
            && target_frozen
            && frozen_neighbors < Self::climate_parameter("freeze_requirement")
        {
            return false;
        }
        true
    }

    fn climate_terrain_record(rule_name: &str) -> &'static ContentRecord {
        let rules = RulesetData::campaign_default();
        let mut matching = rules.content.iter().filter(|record| {
            record.kind == ContentKind::Terrain && record.rule_name.eq_ignore_ascii_case(rule_name)
        });
        let record = matching
            .next()
            .unwrap_or_else(|| panic!("campaign ruleset is missing climate terrain `{rule_name}`"));
        assert!(
            matching.next().is_none(),
            "campaign ruleset repeats climate terrain `{rule_name}`"
        );
        record
    }

    fn climate_parameter(key: &str) -> i32 {
        let rules = RulesetData::campaign_default();
        let mut matching = rules.parameters.iter().filter(|parameter| {
            parameter.file.eq_ignore_ascii_case("terrain.ruleset")
                && parameter.section.eq_ignore_ascii_case("parameters")
                && parameter.key.eq_ignore_ascii_case(key)
        });
        let parameter = matching
            .next()
            .unwrap_or_else(|| panic!("campaign terrain rules are missing parameter `{key}`"));
        assert!(
            matching.next().is_none(),
            "campaign terrain rules repeat parameter `{key}`"
        );
        i32::try_from(
            parameter
                .integer_value()
                .unwrap_or_else(|error| panic!("invalid terrain parameter `{key}`: {error}"))
                .unwrap_or_else(|| panic!("campaign terrain parameter `{key}` has no value")),
        )
        .unwrap_or_else(|_| panic!("campaign terrain parameter `{key}` exceeds i32"))
    }

    fn c2c3_climate_terrain(name: &str) -> Option<Terrain> {
        Some(match name.to_ascii_lowercase().as_str() {
            "no" => return None,
            "lake" => Terrain::Lake,
            "ocean" => Terrain::Sea,
            "deep ocean" => Terrain::Ocean,
            "glacier" => Terrain::Glacier,
            "desert" => Terrain::Desert,
            "forest" => Terrain::Forest,
            "grassland" => Terrain::Grassland,
            "hills" => Terrain::Hills,
            "jungle" => Terrain::Jungle,
            "mountains" => Terrain::Mountains,
            "plains" => Terrain::Plains,
            "swamp" => Terrain::Marsh,
            "tundra" => Terrain::Tundra,
            _ => panic!("unsupported C2C3 climate terrain result `{name}`"),
        })
    }

    fn change_climate(&mut self, warming: bool, mut strength: i32) {
        let mut used = vec![false; self.tiles.len()];
        for _ in 0..self.tiles.len() {
            if strength == 0 {
                break;
            }
            let (index, x, y) = loop {
                let x = self.rand_below(self.map_wrap.width());
                let y = self.rand_below(self.map_wrap.height());
                let index = self.map_wrap.index(x, y);
                if !used[index] {
                    break (index, x, y);
                }
            };
            used[index] = true;
            if let Some(target) = self.climate_target(x, y, warming) {
                if self.tiles[index].terrain != target {
                    self.alter_terrain(x, y, target);
                }
                strength -= 1;
            }
        }
        self.add_event(if warming {
            "Global warming has occurred."
        } else {
            "Nuclear winter has occurred."
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_disaster_priority_uses_last_update_counts_until_the_next_update() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.climate_current = [6, 2];
        game.climate_pressure = [
            ClimatePressure {
                accumulated: 3,
                level: 5,
            },
            ClimatePressure {
                accumulated: 1,
                level: 4,
            },
        ];
        for tile in &mut game.tiles {
            tile.pollution = false;
            tile.fallout = false;
        }
        // Removing contamination during a worker phase does not alter the
        // environmental snapshot already supplied to that phase's advisor.
        assert_eq!(game.worker_climate_urgency(Contamination::Pollution), 103);
        assert_eq!(game.worker_climate_urgency(Contamination::Fallout), 51);
        game.update_climate();
        assert_eq!(game.climate_current, [0, 0]);
        assert_eq!(game.worker_climate_urgency(Contamination::Pollution), 0);
        assert_eq!(game.worker_climate_urgency(Contamination::Fallout), 0);
    }

    #[test]
    fn climate_obeys_moisture_and_checks_the_fallback_only_when_blocked() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        for (nx, ny) in game.map_wrap.civ3_neighbors(x, y) {
            let tile = game.tile_mut(nx, ny);
            tile.terrain = Terrain::Plains;
            tile.river = false;
        }
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        assert_eq!(game.climate_target(x, y, false), Some(Terrain::Tundra));
        game.tile_mut(x, y).river = true;
        assert_eq!(game.climate_target(x, y, false), Some(Terrain::Marsh));
        game.tile_mut(x, y).river = false;
        let (diagonal_x, diagonal_y) = game.map_wrap.civ3_neighbor(x, y, 1).unwrap();
        game.tile_mut(diagonal_x, diagonal_y).terrain = Terrain::Coast;
        assert_eq!(game.climate_target(x, y, false), Some(Terrain::Marsh));
        game.tile_mut(diagonal_x, diagonal_y).terrain = Terrain::Plains;
        game.tile_mut(x, y).terrain = Terrain::Glacier;
        assert_eq!(game.climate_target(x, y, true), Some(Terrain::Tundra));
        game.tile_mut(diagonal_x, diagonal_y).terrain = Terrain::Coast;
        game.tile_mut(x, y).terrain = Terrain::Jungle;
        assert_eq!(game.climate_target(x, y, false), None);
    }

    #[test]
    fn climate_uses_selected_terrain_results_for_civ_iii_sea_and_ocean() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        for (nx, ny) in game.map_wrap.civ3_neighbors(x, y) {
            let tile = game.tile_mut(nx, ny);
            tile.terrain = Terrain::Plains;
            tile.river = false;
        }
        let water_neighbor = game.map_wrap.civ3_neighbor(x, y, 0).unwrap();
        game.tile_mut(water_neighbor.0, water_neighbor.1).terrain = Terrain::Coast;
        game.tile_mut(x, y).terrain = Terrain::Sea;
        assert_eq!(game.climate_target(x, y, false), Some(Terrain::Glacier));

        // Civ III Ocean maps to C2C3 Deep Ocean, whose cooler results are both
        // "no". The previous terrain match incorrectly cooled Deep Ocean.
        game.tile_mut(x, y).terrain = Terrain::Ocean;
        assert_eq!(game.climate_target(x, y, false), None);
    }

    #[test]
    fn climate_uses_c2c3_land_channel_requirement_instead_of_worker_thresholds() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        for (nx, ny) in game.map_wrap.civ3_neighbors(x, y) {
            let tile = game.tile_mut(nx, ny);
            tile.terrain = Terrain::Plains;
            tile.river = false;
        }
        let wet_neighbor = game.map_wrap.civ3_neighbor(x, y, 0).unwrap();
        game.tile_mut(wet_neighbor.0, wet_neighbor.1).terrain = Terrain::Coast;
        game.tile_mut(x, y).terrain = Terrain::Glacier;

        // C2C3 sets land_channel_requirement=10, which is a neighbor count in
        // common/map.c::can_channel_land; an ISO tile has only six directions.
        // The warmer/wetter Lake result is therefore rejected and the declared
        // warmer/drier Tundra result is tried as the fallback.
        assert_eq!(game.climate_target(x, y, true), Some(Terrain::Tundra));
    }

    #[test]
    fn ocean_reclaim_counts_polar_slots_as_land_with_civ_iii_denominator() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (12, 0);
        let adjacent: Vec<_> = game.map_wrap.civ3_neighbors(x, y).collect();
        assert_eq!(adjacent.len(), 5, "three bounded-Y directions are absent");
        for (nx, ny) in adjacent {
            game.tile_mut(nx, ny).terrain = Terrain::Sea;
        }
        game.tile_mut(x, y).terrain = Terrain::Sea;

        // With eight slots, the three missing polar neighbors satisfy the
        // 30% land requirement. A six-slot Freeciv denominator would reject
        // the same neighborhood (only one of its six slots is non-ocean).
        assert!(game.climate_change_allowed(x, y, Terrain::Grassland));
    }
}
