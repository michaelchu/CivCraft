//! C2C3 data adapters for city output and economy inputs.
//!
//! Gameplay transitions remain in the city, mood, and production modules.
//! This adapter supplies rule names, settings, and active effects so those
//! transitions share Freeciv's ruleset model without changing Civ III IDs.

use std::sync::OnceLock;

use super::*;
use crate::rules::freeciv::{ContentKind, RuleParameter, RulesetData, effect_value};
use crate::sim::action_rules::SimulationRequirementContext;
use crate::world::{Resource, Terrain};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sim) enum GoldUpkeepStyle {
    City,
    Mixed,
    Nation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::sim) struct CityEconomySettings {
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
        SETTINGS.get_or_init(|| Self::from_ruleset(RulesetData::campaign_default()))
    }

    fn from_ruleset(rules: &RulesetData) -> Self {
        let integer = |file: &str, section: &str, key: &str| {
            let parameter = parameter(rules, file, section, key);
            i32_value(parameter.integer_value(), parameter)
        };
        let granary = parameter(rules, "game.ruleset", "civstyle", "granary_food_ini");
        let granary_food_initial = granary
            .integer_list_value()
            .unwrap_or_else(|error| panic!("invalid C2C3 granary_food_ini: {error}"))
            .expect("C2C3 granary_food_ini is required")
            .into_iter()
            .map(|value| {
                i32::try_from(value).expect("C2C3 granary_food_ini fits in the campaign range")
            })
            .collect::<Vec<_>>();
        assert!(
            !granary_food_initial.is_empty(),
            "granary food list is empty"
        );

        let upkeep = parameter(rules, "game.ruleset", "civstyle", "gold_upkeep_style")
            .string_value()
            .unwrap_or_else(|error| panic!("invalid C2C3 gold_upkeep_style: {error}"))
            .expect("C2C3 gold_upkeep_style is required");
        let gold_upkeep_style = match upkeep.to_ascii_lowercase().as_str() {
            "city" => GoldUpkeepStyle::City,
            "mixed" => GoldUpkeepStyle::Mixed,
            "nation" => GoldUpkeepStyle::Nation,
            _ => panic!("unknown C2C3 gold_upkeep_style {upkeep:?}"),
        };
        let city_center_minimum = [
            "min_city_center_food",
            "min_city_center_shield",
            "min_city_center_trade",
        ]
        .map(|key| integer("game.ruleset", "civstyle", key));

        Self {
            food_cost: integer("game.ruleset", "civstyle", "food_cost"),
            happy_cost: integer("game.ruleset", "civstyle", "happy_cost"),
            granary_food_initial,
            granary_food_increment: integer("game.ruleset", "civstyle", "granary_food_inc"),
            // These are pinned Freeciv engine defaults, not ruleset parameters.
            foodbox_percent: 100,
            no_trade_size: 0,
            full_trade_size: 1,
            output_granularity: integer("game.ruleset", "civstyle", "output_granularity"),
            city_center_minimum,
            gold_upkeep_style,
            celebrate_size_limit: integer("cities.ruleset", "parameters", "celebrate_size_limit"),
        }
    }
}

fn parameter<'a>(
    rules: &'a RulesetData,
    file: &str,
    section: &str,
    key: &str,
) -> &'a RuleParameter {
    let mut matches = rules.parameters.iter().filter(|parameter| {
        parameter.file.eq_ignore_ascii_case(file)
            && parameter.section.eq_ignore_ascii_case(section)
            && parameter.key.eq_ignore_ascii_case(key)
    });
    let Some(parameter) = matches.next() else {
        panic!("selected C2C3 ruleset is missing {file} [{section}].{key}");
    };
    assert!(
        matches.next().is_none(),
        "selected C2C3 repeats {file} [{section}].{key}"
    );
    parameter
}

fn i32_value(value: Result<Option<i64>, String>, parameter: &RuleParameter) -> i32 {
    let value = value
        .unwrap_or_else(|error| panic!("invalid C2C3 parameter {}: {error}", parameter.key))
        .unwrap_or_else(|| panic!("C2C3 parameter {} has no value", parameter.key));
    i32::try_from(value)
        .unwrap_or_else(|_| panic!("C2C3 parameter {} does not fit in i32", parameter.key))
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

fn content_integer(rules: &RulesetData, kind: ContentKind, name: &str, field: &str) -> i32 {
    let record = rules
        .content
        .iter()
        .find(|record| record.kind == kind && record.rule_name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("C2C3 has no {kind:?} record {name:?}"));
    let value = record
        .integer_field(field)
        .unwrap_or_else(|error| panic!("invalid C2C3 {}.{field}: {error}", record.section))
        .unwrap_or(0);
    i32::try_from(value).unwrap_or_else(|_| panic!("C2C3 {}.{field} exceeds i32", record.section))
}

fn rule_integer(rules: &RulesetData, namespace: &str, name: &str, field: &str) -> i32 {
    let record = rules
        .rule_records
        .iter()
        .find(|record| {
            record.namespace.eq_ignore_ascii_case(namespace)
                && (record.rule_name.eq_ignore_ascii_case(name)
                    || record
                        .display_name
                        .as_deref()
                        .is_some_and(|value| value.eq_ignore_ascii_case(name)))
        })
        .unwrap_or_else(|| panic!("C2C3 has no {namespace} record {name:?}"));
    let value = record
        .integer_field(field)
        .unwrap_or_else(|error| panic!("invalid C2C3 {}.{field}: {error}", record.section))
        .unwrap_or(0);
    i32::try_from(value).unwrap_or_else(|_| panic!("C2C3 {}.{field} exceeds i32", record.section))
}

fn terrain_name(terrain: Terrain) -> &'static str {
    if terrain == Terrain::Volcano {
        // CivCraft keeps the visual identity; the existing C2C3 movement
        // crosswalk maps this rugged tile to Mountains.
        return "Mountains";
    }
    super::super::movement_rules::c2c3_terrain_rule_name(terrain)
        .expect("campaign terrain has an explicit C2C3 output crosswalk")
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

impl Simulation {
    pub(in crate::sim) fn city_rule_effect(
        &self,
        city: &City,
        kind: &str,
        tile: Option<(i32, i32)>,
        output_type: Option<&str>,
        specialist: Option<&str>,
        celebrating: bool,
        government: Government,
        granted_tech: Option<Tech>,
    ) -> i32 {
        let rules = RulesetData::campaign_default();
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
        self.evaluate_city_effect(rules, kind, &context)
    }

    pub(in crate::sim) fn city_rule_effect_for_unit(
        &self,
        city: &City,
        unit: &super::super::Unit,
        kind: &str,
        government: Government,
    ) -> i32 {
        let rules = RulesetData::campaign_default();
        let context = SimulationRequirementContext::for_city_at(self, city, Some((city.x, city.y)))
            .with_unit(unit)
            .with_government(government);
        self.evaluate_city_effect(rules, kind, &context)
    }

    fn evaluate_city_effect(
        &self,
        rules: &RulesetData,
        kind: &str,
        context: &SimulationRequirementContext<'_>,
    ) -> i32 {
        let value = effect_value(&rules.effects, kind, context).unwrap_or_else(|error| {
            panic!("could not evaluate selected C2C3 city effect {kind}: {error:?}")
        });
        i32::try_from(value).unwrap_or_else(|_| panic!("C2C3 city effect {kind} exceeds i32"))
    }

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
        let rules = RulesetData::campaign_default();
        let tile_xy = (x, y);
        let tile = self.tile(x, y);
        let terrain = terrain_name(tile.terrain);
        let mut output = Outputs {
            food: content_integer(rules, ContentKind::Terrain, terrain, "food"),
            shield: content_integer(rules, ContentKind::Terrain, terrain, "shield"),
            trade: content_integer(rules, ContentKind::Terrain, terrain, "trade"),
        };
        if let Some(resource) = tile.active_resource().and_then(resource_name) {
            output.food += rule_integer(rules, "resource", resource, "food");
            output.shield += rule_integer(rules, "resource", resource, "shield");
            output.trade += rule_integer(rules, "resource", resource, "trade");
        }

        let irrigation =
            content_integer(rules, ContentKind::Terrain, terrain, "irrigation_food_incr");
        if irrigation != 0 {
            let pct = self.city_rule_effect(
                city,
                "Irrigation_Pct",
                Some(tile_xy),
                None,
                None,
                celebrating,
                government,
                granted,
            );
            output.food += irrigation * pct / 100;
        }
        let mining = content_integer(rules, ContentKind::Terrain, terrain, "mining_shield_incr");
        if mining != 0 {
            let pct = self.city_rule_effect(
                city,
                "Mining_Pct",
                Some(tile_xy),
                None,
                None,
                celebrating,
                government,
                granted,
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
                increment += rule_integer(rules, "road", road, &format!("{output_name}_incr"));
                constant += rule_integer(rules, "road", road, &format!("{output_name}_incr_const"));
                bonus += rule_integer(rules, "road", road, &format!("{output_name}_bonus"));
            }
            let terrain_percent = content_integer(
                rules,
                ContentKind::Terrain,
                terrain,
                &format!("road_{output_name}_incr_pct"),
            );
            output.add(output_name, constant + increment * terrain_percent / 100);
            output.add(output_name, output.get(output_name) * bonus / 100);

            let output_type = match output_name {
                "food" => "Food",
                "shield" => "Shield",
                _ => "Trade",
            };
            let effect = |kind| {
                self.city_rule_effect(
                    city,
                    kind,
                    Some(tile_xy),
                    Some(output_type),
                    None,
                    celebrating,
                    government,
                    granted,
                )
            };
            let mut value = output.get(output_name) + effect("Output_Add_Tile");
            if value > 0 {
                let penalty_threshold = effect("Output_Penalty_Tile");
                if value >= CityEconomySettings::campaign().output_granularity {
                    value += effect("Output_Inc_Tile");
                    if celebrating {
                        value += effect("Output_Inc_Tile_Celebrate");
                    }
                }
                value += value * effect("Output_Per_Tile") / 100;
                if penalty_threshold > 0 && value > penalty_threshold {
                    value = if value <= CityEconomySettings::campaign().output_granularity {
                        0
                    } else {
                        value - CityEconomySettings::campaign().output_granularity
                    };
                }
            }
            value -= value * effect("Output_Tile_Punish_Pct") / 100;
            if is_center {
                let minimum = match output_type {
                    "Food" => CityEconomySettings::campaign().city_center_minimum[0],
                    "Shield" => CityEconomySettings::campaign().city_center_minimum[1],
                    _ => CityEconomySettings::campaign().city_center_minimum[2],
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
                "Output_Bonus",
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
                "Output_Bonus_2",
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
            "Output_Waste_By_Distance",
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
            "Output_Waste",
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
                "Output_Waste_Pct",
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
                "Shrink_Food"
            } else {
                "Growth_Food"
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

    pub(in crate::sim) fn city_size_limit_from_ruleset(&self, city: &City) -> i32 {
        let government = self.players[city.owner].government;
        if self.city_rule_effect(
            city,
            "Size_Unlimit",
            None,
            None,
            None,
            false,
            government,
            None,
        ) > 0
        {
            i32::MAX
        } else {
            self.city_rule_effect(city, "Size_Adj", None, None, None, false, government, None)
                .max(1)
        }
    }

    pub(in crate::sim) fn building_upkeep_free_limit(&self, city: &City) -> i32 {
        self.city_rule_effect(
            city,
            "Upkeep_Free",
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
