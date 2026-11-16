//! C2C3 data adapters for city output and economy inputs.
//!
//! Gameplay transitions remain in the city, mood, and production modules.
//! This adapter supplies rule names, settings, and active effects so those
//! transitions share Freeciv's ruleset model without changing Civ III IDs.

use std::sync::OnceLock;

use super::*;
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
pub(crate) enum CityOutputType {
    Food,
    Shield,
    Trade,
    Science,
    Gold,
    Luxury,
}

impl CityOutputType {
    fn from_str(value: &str) -> Self {
        match value {
            "Food" => Self::Food,
            "Shield" => Self::Shield,
            "Trade" => Self::Trade,
            "Science" => Self::Science,
            "Gold" => Self::Gold,
            "Luxury" => Self::Luxury,
            other => panic!("unknown city output type {other:?}"),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Food => "Food",
            Self::Shield => "Shield",
            Self::Trade => "Trade",
            Self::Science => "Science",
            Self::Gold => "Gold",
            Self::Luxury => "Luxury",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CitySpecialist {
    Scientist,
    Taxman,
    Elvis,
}

impl CitySpecialist {
    fn as_str(self) -> &'static str {
        match self {
            Self::Scientist => "Scientist",
            Self::Taxman => "Taxman",
            Self::Elvis => "Elvis",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CityRuleEffectParams {
    tile: Option<(i32, i32)>,
    output_type: Option<CityOutputType>,
    specialist: Option<CitySpecialist>,
    celebrating: bool,
    government: Government,
    granted_tech: Option<Tech>,
}

impl CityRuleEffectParams {
    pub(crate) fn new(government: Government, granted_tech: Option<Tech>) -> Self {
        Self {
            tile: None,
            output_type: None,
            specialist: None,
            celebrating: false,
            government,
            granted_tech,
        }
    }

    pub(crate) fn with_tile(mut self, tile: Option<(i32, i32)>) -> Self {
        self.tile = tile;
        self
    }

    pub(crate) fn with_output_type(mut self, output_type: CityOutputType) -> Self {
        self.output_type = Some(output_type);
        self
    }

    pub(crate) fn with_specialist(mut self, specialist: CitySpecialist) -> Self {
        self.specialist = Some(specialist);
        self
    }

    pub(crate) fn celebrating(mut self, celebrating: bool) -> Self {
        self.celebrating = celebrating;
        self
    }
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
            let gold_upkeep_style = match native.gold_upkeep_style.to_ascii_lowercase().as_str() {
                "city" => GoldUpkeepStyle::City,
                "mixed" => GoldUpkeepStyle::Mixed,
                "nation" => GoldUpkeepStyle::Nation,
                other => panic!("unknown native gold_upkeep_style {other:?}"),
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Outputs {
    food: i32,
    shield: i32,
    trade: i32,
}

impl Outputs {
    fn get(self, output: &str) -> i32 {
        match output.to_ascii_lowercase().as_str() {
            "food" => self.food,
            "shield" => self.shield,
            "trade" => self.trade,
            _ => 0,
        }
    }

    fn add(&mut self, output: &str, amount: i32) {
        match output.to_ascii_lowercase().as_str() {
            "food" => self.food += amount,
            "shield" => self.shield += amount,
            "trade" => self.trade += amount,
            _ => unreachable!("tile output must be Food, Shield, or Trade"),
        }
    }
}

fn native_terrain(name: &str) -> &'static crate::rules::native::NativeTerrain {
    crate::rules::native::terrain()
        .terrains
        .iter()
        .find(|terrain| terrain.rule_name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("native catalog has no terrain {name:?}"))
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

fn native_road(name: &str) -> &'static crate::rules::native::NativeRoad {
    crate::rules::native::economy()
        .roads
        .iter()
        .find(|road| road.id.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("native catalog has no road {name:?}"))
}

impl Simulation {
    pub(super) fn city_output_rule_cache(
        &self,
        city: &City,
        government: Government,
        granted_tech: Option<Tech>,
    ) -> CityOutputRuleCache {
        let output_types = [
            CityOutputType::Food,
            CityOutputType::Shield,
            CityOutputType::Trade,
        ];
        let rules = output_types.map(|output| {
            (
                self.city_output_bonus_percent(city, output.as_str(), government, granted_tech),
                self.city_rule_effect(
                    city,
                    EffectKind::OutputWasteByDistance,
                    CityRuleEffectParams::new(government, granted_tech).with_output_type(output),
                ),
                self.city_rule_effect(
                    city,
                    EffectKind::OutputWaste,
                    CityRuleEffectParams::new(government, granted_tech).with_output_type(output),
                ),
                self.city_rule_effect(
                    city,
                    EffectKind::OutputWastePct,
                    CityRuleEffectParams::new(government, granted_tech).with_output_type(output),
                ),
            )
        });
        let trade_output_types = [
            CityOutputType::Science,
            CityOutputType::Gold,
            CityOutputType::Luxury,
        ];
        let trade = CityTradeRuleCache {
            output_additions: trade_output_types
                .map(|output| self.c2c3_city_output_additions(city, output.as_str(), government)),
            output_bonuses: trade_output_types.map(|output| {
                self.city_output_bonus_percent(city, output.as_str(), government, granted_tech)
            }),
            happiness_to_gold: if self.city_rule_effect(
                city,
                EffectKind::HappinessToGold,
                CityRuleEffectParams::new(government, granted_tech),
            ) > 0
            {
                self.city_rule_effect(
                    city,
                    EffectKind::MakeContent,
                    CityRuleEffectParams::new(government, granted_tech),
                ) + self.city_rule_effect(
                    city,
                    EffectKind::ForceContent,
                    CityRuleEffectParams::new(government, granted_tech),
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

    pub(super) fn c2c3_finalize_city_outputs_cached(
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

    pub(in crate::sim) fn city_rule_effect(
        &self,
        city: &City,
        kind: EffectKind,
        params: CityRuleEffectParams,
    ) -> i32 {
        let mut context = SimulationRequirementContext::for_city_at(self, city, params.tile)
            .with_government(params.government)
            .with_granted_tech(params.granted_tech)
            .with_celebrating(params.celebrating);
        if let Some(output_type) = params.output_type {
            context = context.with_output_type(output_type.as_str());
        }
        if let Some(specialist) = params.specialist {
            context = context.with_specialist(specialist.as_str());
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
    pub(in crate::sim) fn c2c3_city_tile_yields(
        &self,
        city: &City,
        x: i32,
        y: i32,
        is_center: bool,
        celebrating: bool,
        granted: Option<Tech>,
        government: Government,
    ) -> (i32, i32, i32) {
        let tile_xy = (x, y);
        let tile = self.tile(x, y);
        let terrain = tile.terrain.c2c3_rugged_rule_name();
        let native_terrain = native_terrain(terrain);
        let mut output = Outputs {
            food: native_terrain.food,
            shield: native_terrain.shield,
            trade: native_terrain.trade,
        };
        if let Some(resource) = tile.active_resource().and_then(resource_name) {
            let resource = native_resource(resource);
            output.food += resource.food;
            output.shield += resource.shield;
            output.trade += resource.trade;
        }

        let irrigation = native_terrain.irrigation_food_incr;
        if irrigation != 0 {
            let pct = self.city_rule_effect(
                city,
                EffectKind::IrrigationPct,
                CityRuleEffectParams::new(government, granted)
                    .with_tile(Some(tile_xy))
                    .celebrating(celebrating),
            );
            output.food += irrigation * pct / 100;
        }
        let mining = native_terrain.mining_shield_incr;
        if mining != 0 {
            let pct = self.city_rule_effect(
                city,
                EffectKind::MiningPct,
                CityRuleEffectParams::new(government, granted)
                    .with_tile(Some(tile_xy))
                    .celebrating(celebrating),
            );
            output.shield += mining * pct / 100;
        }

        // Road-type records carry constant and terrain-percent increments,
        // followed by output percentage bonuses. A railroad remains an
        // independent represented extra, as it is in CivCraft's tile state.
        let roads = [
            (tile.road, "Road"),
            (tile.railroad, "Railroad"),
            (tile.river, "River"),
        ];
        for output_name in ["food", "shield", "trade"] {
            let mut increment = 0;
            let mut constant = 0;
            let mut bonus = 0;
            for (present, road) in roads {
                if !present {
                    continue;
                }
                let road = native_road(road);
                match output_name {
                    "food" => {
                        increment += road.food_incr;
                        constant += road.food_incr_const;
                        bonus += road.food_bonus;
                    }
                    "shield" => {
                        increment += road.shield_incr;
                        constant += road.shield_incr_const;
                        bonus += road.shield_bonus;
                    }
                    "trade" => {
                        increment += road.trade_incr;
                        constant += road.trade_incr_const;
                        bonus += road.trade_bonus;
                    }
                    _ => unreachable!(),
                }
            }
            let terrain_percent = match output_name {
                "food" => native_terrain.road_food_incr_pct,
                "shield" => native_terrain.road_shield_incr_pct,
                "trade" => native_terrain.road_trade_incr_pct,
                _ => unreachable!(),
            };
            output.add(output_name, constant + increment * terrain_percent / 100);
            output.add(output_name, output.get(output_name) * bonus / 100);

            let output_type = match output_name {
                "food" => CityOutputType::Food,
                "shield" => CityOutputType::Shield,
                _ => CityOutputType::Trade,
            };
            let effect = |kind| {
                self.city_rule_effect(
                    city,
                    kind,
                    CityRuleEffectParams::new(government, granted)
                        .with_tile(Some(tile_xy))
                        .with_output_type(output_type)
                        .celebrating(celebrating),
                )
            };
            let mut value = output.get(output_name) + effect(EffectKind::OutputAddTile);
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
                let minimum = match output_type {
                    CityOutputType::Food => CityEconomySettings::campaign().city_center_minimum[0],
                    CityOutputType::Shield => {
                        CityEconomySettings::campaign().city_center_minimum[1]
                    }
                    CityOutputType::Trade => CityEconomySettings::campaign().city_center_minimum[2],
                    _ => unreachable!("tile output only produces food, shields, and trade"),
                };
                value = value.max(minimum);
            }
            output.add(output_name, value - output.get(output_name));
        }

        (
            output.food.max(0),
            output.shield.max(0),
            output.trade.max(0),
        )
    }

    /// Science, gold, and luxury are assigned from trade, but C2C3 may add
    /// output directly to the city center or any worked tile. Freeciv's
    /// `city_tile_output()` evaluates those effects per tile before bonuses.
    pub(in crate::sim) fn c2c3_city_output_additions(
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
                    CityRuleEffectParams::new(government, None)
                        .with_tile(Some(tile))
                        .with_output_type(CityOutputType::from_str(output_type)),
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
            + i64::from(
                self.city_rule_effect(
                    city,
                    EffectKind::OutputBonus,
                    CityRuleEffectParams::new(government, granted_tech)
                        .with_output_type(CityOutputType::from_str(output_type)),
                ),
            );
        let second = 100_i64
            + i64::from(
                self.city_rule_effect(
                    city,
                    EffectKind::OutputBonus2,
                    CityRuleEffectParams::new(government, granted_tech)
                        .with_output_type(CityOutputType::from_str(output_type)),
                ),
            );
        ((first * second / 100).max(0)).min(i64::from(i32::MAX)) as i32
    }

    /// Apply Freeciv's city-wide bonus-before-waste ordering to the three
    /// worked-tile outputs after route trade has joined gross trade. C2C3's
    /// size penalty and local tax split are retained.
    pub(in crate::sim) fn c2c3_finalize_city_outputs(
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
        self.c2c3_city_output_after_bonus(city, produced, output_type, government, granted_tech)
    }

    pub(in crate::sim) fn c2c3_city_output_after_bonus(
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
            CityRuleEffectParams::new(government, granted_tech)
                .with_output_type(CityOutputType::from_str(output_type)),
        );
        let mut waste_all = false;
        let mut waste_level = self.city_rule_effect(
            city,
            EffectKind::OutputWaste,
            CityRuleEffectParams::new(government, granted_tech)
                .with_output_type(CityOutputType::from_str(output_type)),
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
                CityRuleEffectParams::new(government, granted_tech)
                    .with_output_type(CityOutputType::from_str(output_type)),
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
            CityRuleEffectParams::new(government, None),
        )
        .clamp(0, 100)
    }

    pub(in crate::sim) fn city_size_limit_from_ruleset(&self, city: &City) -> i32 {
        let government = self.players[city.owner].government;
        if self.city_rule_effect(
            city,
            EffectKind::SizeUnlimit,
            CityRuleEffectParams::new(government, None),
        ) > 0
        {
            i32::MAX
        } else {
            self.city_rule_effect(
                city,
                EffectKind::SizeAdj,
                CityRuleEffectParams::new(government, None),
            )
            .max(1)
        }
    }

    pub(in crate::sim) fn building_upkeep_free_limit(&self, city: &City) -> i32 {
        self.city_rule_effect(
            city,
            EffectKind::UpkeepFree,
            CityRuleEffectParams::new(self.players[city.owner].government, None),
        )
        .max(0)
    }
}
