//! C2C3 data adapters for city output and economy inputs.
//!
//! Gameplay transitions remain in the city, mood, and production modules.
//! This adapter supplies rule names, settings, and active effects so those
//! transitions share Freeciv's ruleset model without changing Civ III IDs.

use std::sync::OnceLock;

use super::*;
use crate::rules::terrain_catalog::{self, RoadYields, TerrainYields};
use crate::sim::action_rules::SimulationRequirementContext;
use crate::world::Resource;

#[derive(Clone, Copy)]
pub(in crate::sim) struct CityOutputRuleCache {
    rules: [(i32, i32, i32, i32); 3],
    distance: Option<i32>,
    pub(in crate::sim) trade: CityTradeRuleCache,
}

#[derive(Clone, Copy)]
pub(in crate::sim) struct CityTradeRuleCache {
    pub(in crate::sim) output_additions: [i32; 3],
    pub(in crate::sim) output_bonuses: [i32; 3],
    pub(in crate::sim) happiness_to_gold: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sim) enum GoldUpkeepStyle {
    City,
    Mixed,
    Nation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::sim) struct CityEconomySettings {
    pub(in crate::sim) city_min_distance: i32,
    pub(in crate::sim) food_cost: i32,
    pub(in crate::sim) happy_cost: i32,
    pub(in crate::sim) granary_food_initial: Vec<i32>,
    pub(in crate::sim) granary_food_increment: i32,
    pub(in crate::sim) foodbox_percent: i32,
    pub(in crate::sim) output_granularity: i32,
    pub(in crate::sim) city_center_minimum: [i32; 3],
    pub(in crate::sim) no_trade_size: i32,
    pub(in crate::sim) full_trade_size: i32,
    pub(in crate::sim) gold_upkeep_style: GoldUpkeepStyle,
    pub(in crate::sim) celebrate_size_limit: i32,
}

impl CityEconomySettings {
    pub(in crate::sim) fn campaign() -> &'static Self {
        static SETTINGS: OnceLock<CityEconomySettings> = OnceLock::new();
        SETTINGS.get_or_init(|| {
            let native = &crate::rules::native::economy().city;
            use crate::rules::native::NativeGoldUpkeepStyle as Native;
            let gold_upkeep_style = match native.gold_upkeep_style {
                Native::City => GoldUpkeepStyle::City,
                Native::Mixed => GoldUpkeepStyle::Mixed,
                Native::Nation => GoldUpkeepStyle::Nation,
            };
            assert!(!native.granary_food_initial.is_empty());
            Self {
                city_min_distance: native.city_min_distance,
                food_cost: native.food_cost,
                happy_cost: native.happy_cost,
                granary_food_initial: native.granary_food_initial.clone(),
                granary_food_increment: native.granary_food_increment,
                foodbox_percent: native.foodbox_percent,
                output_granularity: native.output_granularity,
                city_center_minimum: native.city_center_minimum,
                no_trade_size: native.no_trade_size,
                full_trade_size: native.full_trade_size,
                gold_upkeep_style,
                celebrate_size_limit: native.celebrate_size_limit,
            }
        })
    }
}

struct TileYieldRules {
    terrains: std::collections::BTreeMap<&'static str, TerrainYields>,
    roads: [RoadYields; 3],
}

impl TileYieldRules {
    fn campaign() -> &'static Self {
        static RULES: OnceLock<TileYieldRules> = OnceLock::new();
        RULES.get_or_init(|| Self {
            terrains: terrain_catalog::TERRAIN_SPECS
                .iter()
                .map(|spec| (spec.rule_name, spec.yields))
                .collect(),
            roads: ["Road", "Railroad", "River"]
                .map(|name| terrain_catalog::road_yields(name).expect("valid road yields")),
        })
    }
}

fn resource_name(resource: Resource) -> Option<&'static str> {
    Some(match resource {
        Resource::Gold => "Gold",
        Resource::Iron => "Iron",
        Resource::Game => "Game",
        Resource::Furs => "Furs",
        Resource::Coal => "Coal",
        Resource::Fish => "Fish",
        Resource::TropicalFruit => "Fruit",
        Resource::Gems => "Gems",
        Resource::Cattle => "Buffalo",
        Resource::Wheat => "Wheat",
        Resource::Oasis => "Oasis",
        Resource::Ivory => "icy_ivory",
        Resource::Silk => "Silk",
        Resource::Spice => "Spice",
        Resource::Whales => "Whales",
        Resource::Wine => "Wine",
        Resource::Oil => "Oil",
        // Civ III-only resources stay visible and retain their Civ III IDs.
        // The selected ruleset has no corresponding resource records.
        Resource::Aluminum
        | Resource::Horses
        | Resource::Rubber
        | Resource::Saltpeter
        | Resource::Uranium
        | Resource::Dyes
        | Resource::Incense
        | Resource::Sugar
        | Resource::Tobacco => return None,
    })
}

fn native_resource(name: &str) -> &'static crate::rules::native::NativeResource {
    crate::rules::native::economy()
        .resources
        .iter()
        .find(|resource| resource.id.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("native catalog has no resource {name:?}"))
}

impl Simulation {
    pub(super) fn city_output_rule_cache(
        &self,
        city: &City,
        government: Government,
        granted_tech: Option<Tech>,
    ) -> CityOutputRuleCache {
        let output_types = ["Food", "Shield", "Trade"];
        let rules = output_types.map(|output| {
            (
                self.city_output_bonus_percent(city, output, government, granted_tech),
                self.city_rule_effect(
                    city,
                    EffectKind::OutputWasteByDistance,
                    None,
                    Some(output),
                    None,
                    false,
                    government,
                    granted_tech,
                ),
                self.city_rule_effect(
                    city,
                    EffectKind::OutputWaste,
                    None,
                    Some(output),
                    None,
                    false,
                    government,
                    granted_tech,
                ),
                self.city_rule_effect(
                    city,
                    EffectKind::OutputWastePct,
                    None,
                    Some(output),
                    None,
                    false,
                    government,
                    granted_tech,
                ),
            )
        });
        let trade_output_types = ["Science", "Gold", "Luxury"];
        let trade = CityTradeRuleCache {
            output_additions: trade_output_types
                .map(|output| self.city_output_additions(city, output, government)),
            output_bonuses: trade_output_types.map(|output| {
                self.city_output_bonus_percent(city, output, government, granted_tech)
            }),
            happiness_to_gold: if self.city_rule_effect(
                city,
                EffectKind::HappinessToGold,
                None,
                None,
                None,
                false,
                government,
                granted_tech,
            ) > 0
            {
                self.city_rule_effect(
                    city,
                    EffectKind::MakeContent,
                    None,
                    None,
                    None,
                    false,
                    government,
                    granted_tech,
                ) + self.city_rule_effect(
                    city,
                    EffectKind::ForceContent,
                    None,
                    None,
                    None,
                    false,
                    government,
                    granted_tech,
                )
            } else {
                0
            },
        };
        CityOutputRuleCache {
            rules,
            distance: self.government_center_distance(city),
            trade,
        }
    }

    /// Applies civ2civ3 city output finalization.
    pub(super) fn finalize_city_outputs_cached(
        &self,
        city: &City,
        gross: (i32, i32, i32),
        cache: &CityOutputRuleCache,
    ) -> (i32, i32, i32) {
        let values = [gross.0, gross.1, gross.2];
        let result: [i32; 3] = std::array::from_fn(|index| {
            let (bonus, distance_factor, base_waste, reduction) = cache.rules[index];
            let produced = values[index].wrapping_mul(bonus) / 100;
            let settings = CityEconomySettings::campaign();
            let (size_penalty, remaining) = if index == 2 {
                let no_trade = settings.no_trade_size.min(settings.full_trade_size);
                let full_trade = settings.no_trade_size.max(settings.full_trade_size);
                let penalty = if city.size <= no_trade {
                    produced
                } else if city.size >= full_trade || full_trade == no_trade {
                    0
                } else {
                    produced * (full_trade - city.size) / (full_trade - no_trade)
                };
                (penalty, produced - penalty)
            } else {
                (0, produced)
            };
            let mut waste_all = false;
            let mut waste_level = base_waste;
            if remaining > 0 && distance_factor > 0 {
                if let Some(distance) = cache.distance {
                    waste_level += distance_factor * distance / 100;
                } else {
                    waste_all = true;
                }
            }
            let mut waste = if waste_all {
                remaining
            } else if waste_level > 0 {
                remaining.wrapping_mul(waste_level) / 100
            } else {
                0
            };
            if !waste_all {
                waste -= waste * reduction / 100;
            }
            let waste = waste.clamp(0, remaining.max(0));
            (produced - size_penalty - waste).max(0)
        });
        (result[0], result[1], result[2])
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Effect evaluation needs the explicit city, tile, output, and hypothetical government context."
    )]
    pub(in crate::sim) fn city_rule_effect(
        &self,
        city: &City,
        kind: EffectKind,
        tile: Option<(i32, i32)>,
        output_type: Option<&str>,
        specialist: Option<&str>,
        celebrating: bool,
        government: Government,
        granted_tech: Option<Tech>,
    ) -> i32 {
        let mut context = SimulationRequirementContext::for_city_at(self, city, tile)
            .with_government(government)
            .with_granted_tech(granted_tech)
            .with_celebrating(celebrating);
        if let Some(output_type) = output_type {
            context = context.with_output_type(output_type);
        }
        if let Some(specialist) = specialist {
            context = context.with_specialist(specialist);
        }
        context.effect_i32(kind)
    }

    pub(in crate::sim) fn city_rule_effect_for_unit(
        &self,
        city: &City,
        unit: &super::super::Unit,
        kind: EffectKind,
        government: Government,
    ) -> i32 {
        let context = SimulationRequirementContext::for_city_at(self, city, Some((city.x, city.y)))
            .with_unit(unit)
            .with_government(government);
        context.effect_i32(kind)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Tile yield evaluation takes independent location, celebration, and hypothetical technology inputs."
    )]
    pub(in crate::sim) fn ruleset_city_tile_yields(
        &self,
        city: &City,
        tile_xy: (i32, i32),
        tile: &Tile,
        is_center: bool,
        celebrating: bool,
        granted: Option<Tech>,
        government: Government,
    ) -> (i32, i32, i32) {
        let mut context = SimulationRequirementContext::for_city_at(self, city, Some(tile_xy))
            .with_government(government)
            .with_granted_tech(granted)
            .with_celebrating(celebrating)
            .with_tile_override(tile_xy, tile);
        let rules = TileYieldRules::campaign();
        let terrain = rules.terrains[tile.terrain.engine_rugged_rule_name()];
        let mut output = terrain.base;
        if let Some(resource) = tile.active_resource().and_then(resource_name) {
            let resource = native_resource(resource);
            output[0] += resource.food;
            output[1] += resource.shield;
            output[2] += resource.trade;
        }
        if terrain.irrigation != 0 {
            output[0] += terrain.irrigation * context.effect_i32(EffectKind::IrrigationPct) / 100;
        }
        if terrain.mining != 0 {
            output[1] += terrain.mining * context.effect_i32(EffectKind::MiningPct) / 100;
        }

        // Railroads remain independent extras in CivCraft's tile state.
        let roads = [tile.road, tile.railroad, tile.river];
        for (index, output_type) in ["Food", "Shield", "Trade"].into_iter().enumerate() {
            let (mut increment, mut constant, mut bonus) = (0, 0, 0);
            for (present, road) in roads.into_iter().zip(rules.roads) {
                if present {
                    increment += road.increment[index];
                    constant += road.constant[index];
                    bonus += road.bonus[index];
                }
            }
            let base = output[index] + constant + increment * terrain.road_percent[index] / 100;
            let base = base + base * bonus / 100;
            context = context.with_output_type(output_type);
            let effect = |kind| context.effect_i32(kind);
            let mut value = base + effect(EffectKind::OutputAddTile);
            if value > 0 {
                let penalty_threshold = effect(EffectKind::OutputPenaltyTile);
                if value >= CityEconomySettings::campaign().output_granularity {
                    value += effect(EffectKind::OutputIncTile);
                    if celebrating {
                        value += effect(EffectKind::OutputIncTileCelebrate);
                    }
                }
                value += value * effect(EffectKind::OutputPerTile) / 100;
                if penalty_threshold > 0 && value > penalty_threshold {
                    value = if value <= CityEconomySettings::campaign().output_granularity {
                        0
                    } else {
                        value - CityEconomySettings::campaign().output_granularity
                    };
                }
            }
            value -= value * effect(EffectKind::OutputTilePunishPct) / 100;
            if is_center {
                value = value.max(CityEconomySettings::campaign().city_center_minimum[index]);
            }
            output[index] = value.max(0);
        }

        (output[0], output[1], output[2])
    }

    /// Science, gold, and luxury are assigned from trade, but C2C3 may add
    /// output directly to the city center or any worked tile. Freeciv's
    /// `city_tile_output()` evaluates those effects per tile before bonuses.
    pub(in crate::sim) fn city_output_additions(
        &self,
        city: &City,
        output_type: &str,
        government: Government,
    ) -> i32 {
        std::iter::once((city.x, city.y))
            .chain(
                city.worked_tiles
                    .iter()
                    .copied()
                    .filter(|&tile| tile != (city.x, city.y)),
            )
            .map(|tile| {
                self.city_rule_effect(
                    city,
                    EffectKind::OutputAddTile,
                    Some(tile),
                    Some(output_type),
                    None,
                    false,
                    government,
                    None,
                )
            })
            .sum()
    }

    pub(in crate::sim) fn city_output_bonus_percent(
        &self,
        city: &City,
        output_type: &str,
        government: Government,
        granted_tech: Option<Tech>,
    ) -> i32 {
        let first = 100_i64
            + i64::from(self.city_rule_effect(
                city,
                EffectKind::OutputBonus,
                None,
                Some(output_type),
                None,
                false,
                government,
                granted_tech,
            ));
        let second = 100_i64
            + i64::from(self.city_rule_effect(
                city,
                EffectKind::OutputBonus2,
                None,
                Some(output_type),
                None,
                false,
                government,
                granted_tech,
            ));
        ((first * second / 100).max(0)).min(i64::from(i32::MAX)) as i32
    }

    /// Apply Freeciv's city-wide bonus-before-waste ordering to the three
    /// worked-tile outputs after route trade has joined gross trade. C2C3's
    /// size penalty and local tax split are retained.
    ///
    /// Applies civ2civ3 city output finalization.
    pub(in crate::sim) fn finalize_city_outputs(
        &self,
        city: &City,
        gross: (i32, i32, i32),
        government: Government,
        granted_tech: Option<Tech>,
    ) -> (i32, i32, i32) {
        let food = self.apply_city_output(city, gross.0, "Food", government, granted_tech);
        let shield = self.apply_city_output(city, gross.1, "Shield", government, granted_tech);
        let trade = self.apply_city_output(city, gross.2, "Trade", government, granted_tech);
        (food, shield, trade)
    }

    fn apply_city_output(
        &self,
        city: &City,
        gross: i32,
        output_type: &str,
        government: Government,
        granted_tech: Option<Tech>,
    ) -> i32 {
        let bonus = self.city_output_bonus_percent(city, output_type, government, granted_tech);
        // city.c keeps these outputs in native C `int` arithmetic. Wrapping
        // before division matches the pinned build for the city-manager's
        // synthetic sentinel values and is identical for ordinary outputs.
        let produced = gross.wrapping_mul(bonus) / 100;
        self.city_output_after_bonus(city, produced, output_type, government, granted_tech)
    }

    pub(in crate::sim) fn city_output_after_bonus(
        &self,
        city: &City,
        produced: i32,
        output_type: &str,
        government: Government,
        granted_tech: Option<Tech>,
    ) -> i32 {
        let settings = CityEconomySettings::campaign();
        let (size_penalty, remaining) = if output_type.eq_ignore_ascii_case("Trade") {
            let no_trade = settings.no_trade_size.min(settings.full_trade_size);
            let full_trade = settings.no_trade_size.max(settings.full_trade_size);
            let penalty = if city.size <= no_trade {
                produced
            } else if city.size >= full_trade || full_trade == no_trade {
                0
            } else {
                produced * (full_trade - city.size) / (full_trade - no_trade)
            };
            (penalty, produced - penalty)
        } else {
            (0, produced)
        };
        let distance_factor = self.city_rule_effect(
            city,
            EffectKind::OutputWasteByDistance,
            None,
            Some(output_type),
            None,
            false,
            government,
            granted_tech,
        );
        let mut waste_all = false;
        let mut waste_level = self.city_rule_effect(
            city,
            EffectKind::OutputWaste,
            None,
            Some(output_type),
            None,
            false,
            government,
            granted_tech,
        );
        if remaining > 0 && distance_factor > 0 {
            if let Some(distance) = self.government_center_distance(city) {
                waste_level += distance_factor * distance / 100;
            } else {
                waste_all = true;
            }
        }
        let mut waste = if waste_all {
            remaining
        } else if waste_level > 0 {
            // Freeciv's C implementation performs this multiplication in
            // the native int width. Keep that behavior for the synthetic
            // city-manager sentinels as well as ordinary production values.
            remaining.wrapping_mul(waste_level) / 100
        } else {
            0
        };
        if !waste_all {
            let reduction = self.city_rule_effect(
                city,
                EffectKind::OutputWastePct,
                None,
                Some(output_type),
                None,
                false,
                government,
                granted_tech,
            );
            waste -= waste * reduction / 100;
        }
        let waste = waste.clamp(0, remaining.max(0));
        (produced - size_penalty - waste).max(0)
    }

    pub(in crate::sim) fn city_growth_retention(&self, city: &City, shrinking: bool) -> i32 {
        let government = self.players[city.owner].government;
        self.city_rule_effect(
            city,
            if shrinking {
                EffectKind::ShrinkFood
            } else {
                EffectKind::GrowthFood
            },
            None,
            None,
            None,
            false,
            government,
            None,
        )
        .clamp(0, 100)
    }

    /// Civ III city size caps: 6 without an Aqueduct, 12 with one (a city beside
    /// fresh water, a river or lake, never needs it), and 16 with a Hospital or
    /// Shakespeare's Theater (Conquests Civilopedia).
    pub(in crate::sim) fn city_size_limit_from_ruleset(&self, city: &City) -> i32 {
        if city.has_building(Building::HOSPITAL)
            || city.has_building(Building::SHAKESPEARES_THEATER)
        {
            return 16;
        }
        let fresh_water = std::iter::once((city.x, city.y))
            .chain(self.map_wrap.eight_way_neighbors(city.x, city.y))
            .any(|(x, y)| {
                let tile = self.tile(x, y);
                tile.river || tile.terrain == Terrain::Lake
            });
        if fresh_water || city.has_building(Building::AQUEDUCT) {
            12
        } else {
            6
        }
    }

    pub(in crate::sim) fn building_upkeep_free_limit(&self, city: &City) -> i32 {
        self.city_rule_effect(
            city,
            EffectKind::UpkeepFree,
            None,
            None,
            None,
            false,
            self.players[city.owner].government,
            None,
        )
        .max(0)
    }
}
