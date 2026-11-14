//! Unit support and upkeep calculations.

use super::*;
use crate::rules::freeciv::{
    ContentRecord, HappyBorders, RulesetData, UnitHappinessRulesetSettings, UnitUpkeepOutput,
    effect_value,
};
use crate::sim::action_rules::{
    SimulationRequirementContext, UnitEconomicsRecordBinding, freeciv_unit_economics_record,
};
use std::sync::OnceLock;

pub(crate) struct UnitUpkeep {
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) gold: i32,
    pub(crate) unit_happiness: i32,
    /// Source-order supported units, with both pre-allowance and post-allowance
    /// costs for the city-turn missing-upkeep resolver.
    pub(crate) per_unit: Vec<UnitUpkeepRow>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct UnitOutputCosts {
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) gold: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnitUpkeepRow {
    pub(crate) unit_id: u32,
    pub(crate) economics: UnitUpkeepMapping,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum UnitUpkeepMapping {
    Mapped {
        /// Unit upkeep after unit flags, effects, and output conversion, before
        /// the city's ordered free-upkeep allowance is allocated.
        raw: UnitOutputCosts,
        /// Cost after the city's per-output free-upkeep allowance.
        net: UnitOutputCosts,
    },
    Unsupported(UnsupportedUnitEconomics),
}

fn unit_happiness_ruleset_settings() -> &'static UnitHappinessRulesetSettings {
    static SETTINGS: OnceLock<UnitHappinessRulesetSettings> = OnceLock::new();
    SETTINGS.get_or_init(|| {
        RulesetData::campaign_default()
            .unit_happiness_settings()
            .unwrap_or_else(|error| panic!("invalid C2C3 unit-happiness settings: {error}"))
    })
}

fn missing_unit_upkeep_settings() -> &'static crate::rules::freeciv::MissingUnitUpkeepSettings {
    static SETTINGS: OnceLock<crate::rules::freeciv::MissingUnitUpkeepSettings> = OnceLock::new();
    SETTINGS.get_or_init(|| {
        RulesetData::campaign_default()
            .missing_unit_upkeep_settings()
            .unwrap_or_else(|error| panic!("invalid C2C3 missing-unit-upkeep settings: {error}"))
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnsupportedUnitEconomics {
    pub(crate) unit_id: u32,
    pub(crate) source_id: &'static str,
    pub(crate) basis: &'static str,
}

pub(super) struct UnitGoldUpkeep {
    pub(super) paid: Vec<(u32, i32)>,
}

struct CityUnitEconomics {
    upkeep: UnitUpkeep,
    gold_paid: Vec<(u32, i32)>,
}

impl Simulation {
    pub(crate) fn city_unit_upkeep(&self, city: &City) -> UnitUpkeep {
        self.city_unit_economics(city).upkeep
    }

    pub(super) fn city_gold_upkeep(&self, city: &City) -> UnitGoldUpkeep {
        let economics = self.city_unit_economics(city);
        UnitGoldUpkeep {
            paid: economics.gold_paid,
        }
    }

    /// National gold upkeep for units that have no home city. Freeciv charges
    /// these outside the per-city free-upkeep allocation for Mixed and Nation
    /// styles.
    pub(super) fn homeless_unit_gold_upkeep(&self, owner: usize) -> UnitGoldUpkeep {
        let government = self.players[owner].government;
        let mut paid = Vec::new();
        for unit in self
            .units
            .iter()
            .filter(|unit| unit.owner == owner && unit.home_city.is_none())
        {
            let mapping = freeciv_unit_economics_record(unit.kind);
            let record = match mapping {
                UnitEconomicsRecordBinding::Mapped { record, .. } => record,
                UnitEconomicsRecordBinding::Unsupported { .. } => continue,
            };
            let cost = self.c2c3_unit_upkeep_cost(unit, record, "Gold", government);
            if cost > 0 {
                paid.push((unit.id, cost));
            }
        }
        UnitGoldUpkeep {
            paid,
        }
    }

    fn city_unit_economics(&self, city: &City) -> CityUnitEconomics {
        let mut units: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.owner == city.owner && unit.home_city == Some(city.id))
            .collect();
        // Freeciv prepends a unit to its home city's supported list both when
        // it is created and when it is rehomed, so its resolver iteration is
        // newest supported unit first.
        units.sort_by_key(|unit| std::cmp::Reverse((unit.home_order, unit.id)));
        let mappings: Vec<_> = units
            .into_iter()
            .map(|unit| (unit, freeciv_unit_economics_record(unit.kind)))
            .collect();
        let allocation_complete = mappings.iter().all(|(_, mapping)| {
            matches!(mapping, UnitEconomicsRecordBinding::Mapped { .. })
        });
        let government = self.players[city.owner].government;
        let output_types = ["Food", "Shield", "Gold"];
        // Freeciv allocates each output's free upkeep across the city's
        // supported-unit list in order. If an unmapped unit precedes a mapped
        // one, its unknown cost makes allowance allocation unknowable; in
        // that city we charge mapped units gross values and expose the gap.
        let mut free_upkeep = if allocation_complete {
            output_types.map(|output| {
                self.city_rule_effect(
                    city,
                    "Unit_Upkeep_Free_Per_City",
                    None,
                    Some(output),
                    None,
                    false,
                    government,
                    None,
                )
                .max(0)
            })
        } else {
            [0; 3]
        };
        let mut free_unhappy = if allocation_complete {
            self.city_rule_effect(
                city,
                "Make_Content_Mil",
                None,
                None,
                None,
                false,
                government,
                None,
            )
            .max(0)
        } else {
            0
        };
        let mut upkeep = UnitUpkeep {
            food: 0,
            shields: 0,
            gold: 0,
            unit_happiness: 0,
            per_unit: Vec::with_capacity(mappings.len()),
        };
        let mut gold_paid = Vec::new();

        for (unit, mapping) in mappings {
            let record = match mapping {
                UnitEconomicsRecordBinding::Mapped { record, .. } => record,
                UnitEconomicsRecordBinding::Unsupported { source_id, basis } => {
                    upkeep.per_unit.push(UnitUpkeepRow {
                        unit_id: unit.id,
                        economics: UnitUpkeepMapping::Unsupported(UnsupportedUnitEconomics {
                            unit_id: unit.id,
                            source_id,
                            basis,
                        }),
                    });
                    continue;
                }
            };
            if record_has_flag(record, "NoHome") {
                continue;
            }

            let raw = UnitOutputCosts {
                food: self.c2c3_unit_upkeep_cost(unit, record, "Food", government),
                shields: self.c2c3_unit_upkeep_cost(unit, record, "Shield", government),
                gold: self.c2c3_unit_upkeep_cost(unit, record, "Gold", government),
            };
            let mut net = raw;
            if allocation_complete {
                net.food = use_free_upkeep(net.food, &mut free_upkeep[0]);
                net.shields = use_free_upkeep(net.shields, &mut free_upkeep[1]);
                net.gold = use_free_upkeep(net.gold, &mut free_upkeep[2]);
            }
            upkeep.food += net.food;
            upkeep.shields += net.shields;
            upkeep.gold += net.gold;
            if net.gold > 0 {
                gold_paid.push((unit.id, net.gold));
            }
            upkeep.per_unit.push(UnitUpkeepRow {
                unit_id: unit.id,
                economics: UnitUpkeepMapping::Mapped { raw, net },
            });

            upkeep.unit_happiness += self.c2c3_unit_unhappiness(
                city,
                unit,
                record,
                government,
                &mut free_unhappy,
                allocation_complete,
            );
        }
        CityUnitEconomics { upkeep, gold_paid }
    }

    fn c2c3_unit_upkeep_cost(
        &self,
        unit: &Unit,
        record: &'static ContentRecord,
        output: &str,
        government: Government,
    ) -> i32 {
        if record_has_flag(record, "Fanatic")
            && self.c2c3_unit_effect(unit, record, "Fanatics", None, government) > 0
        {
            return 0;
        }

        let field = match output {
            "Food" => "uk_food",
            "Shield" => "uk_shield",
            "Gold" => "uk_gold",
            _ => unreachable!("unit upkeep only covers Food, Shield, and Gold"),
        };
        let mut cost = record_integer(record, field);

        if matches!(output, "Gold" | "Shield") {
            let shield_to_gold =
                self.c2c3_unit_effect(unit, record, "Shield2Gold_Pct", None, government);
            if shield_to_gold > 0 {
                cost = match output {
                    "Gold" => {
                        let shield_cost = record_integer(record, "uk_shield");
                        (shield_cost * shield_to_gold + 99) / 100
                    }
                    "Shield" => 0,
                    _ => unreachable!(),
                };
            }
        }

        let upkeep_percent =
            self.c2c3_unit_effect(unit, record, "Upkeep_Pct", Some(output), government);
        cost.saturating_mul(upkeep_percent) / 100
    }

    fn c2c3_unit_unhappiness(
        &self,
        city: &City,
        unit: &Unit,
        record: &'static ContentRecord,
        government: Government,
        free_unhappy: &mut i32,
        apply_free_unhappy: bool,
    ) -> i32 {
        let happy_cost = record_integer(record, "uk_happy").saturating_mul(self.city_rule_effect(
            city,
            "Unhappy_Factor",
            None,
            None,
            None,
            false,
            government,
            None,
        ));
        if happy_cost <= 0
            || (!unit_being_aggressive(self, unit, record) && !record_has_flag(record, "FieldUnit"))
        {
            return 0;
        }

        let per_unit_content =
            self.c2c3_city_unit_effect(city, unit, record, "Make_Content_Mil_Per", government);
        let mut unhappy = (happy_cost - per_unit_content).max(0);
        if apply_free_unhappy {
            if *free_unhappy >= unhappy {
                *free_unhappy -= unhappy;
                return 0;
            }
            unhappy -= *free_unhappy;
            *free_unhappy = 0;
        }
        unhappy
    }

    fn c2c3_unit_effect(
        &self,
        unit: &Unit,
        record: &'static ContentRecord,
        kind: &str,
        output: Option<&str>,
        government: Government,
    ) -> i32 {
        let mut context = SimulationRequirementContext::for_unit(self, unit)
            .with_government(government)
            .with_unit_type_flag_record(record);
        if let Some(output) = output {
            context = context.with_output_type(output);
        }
        let value = effect_value(&RulesetData::campaign_default().effects, kind, &context)
            .unwrap_or_else(|error| panic!("invalid C2C3 unit effect {kind}: {error:?}"));
        i32::try_from(value).unwrap_or_else(|_| panic!("C2C3 unit effect {kind} exceeds i32"))
    }

    fn c2c3_city_unit_effect(
        &self,
        city: &City,
        unit: &Unit,
        record: &'static ContentRecord,
        kind: &str,
        government: Government,
    ) -> i32 {
        let context = SimulationRequirementContext::for_city_at(self, city, Some((city.x, city.y)))
            .with_unit(unit)
            .with_government(government)
            .with_unit_type_flag_record(record);
        let value = effect_value(&RulesetData::campaign_default().effects, kind, &context)
            .unwrap_or_else(|error| panic!("invalid C2C3 city-unit effect {kind}: {error:?}"));
        i32::try_from(value).unwrap_or_else(|_| panic!("C2C3 city-unit effect {kind} exceeds i32"))
    }

    pub(super) fn balance_unit_gold_upkeep(&mut self, owner: usize, paid: &[(u32, i32)]) {
        while self.players[owner].gold < 0 {
            let candidates: Vec<_> = self
                .player_units_ordered(owner)
                .filter(|unit| paid.iter().any(|&(id, _)| id == unit.id))
                .map(|unit| unit.id)
                .collect();
            if candidates.is_empty() {
                self.players[owner].gold = 0;
                break;
            }
            let count = candidates.len();
            let mut id = candidates[self.rand_below(count as i32) as usize];
            // The reference sells upkeep-paying cargo before its transport.
            while let Some(cargo) = self.units.iter().find(|unit| {
                unit.transport == Some(id) && paid.iter().any(|&(paid_id, _)| paid_id == unit.id)
            }) {
                id = cargo.id;
            }
            let cost = paid.iter().find(|&&(paid_id, _)| paid_id == id).unwrap().1;
            if self.resolve_missing_unit_upkeep(id, UnitUpkeepOutput::Gold) {
                self.players[owner].gold += cost;
            } else {
                // Freeciv stops selecting units when the ruleset's gold
                // protected/actions/wipe policy cannot remove the candidate.
                self.players[owner].gold = 0;
                break;
            }
        }
    }

    /// Apply the selected ruleset's protected flags, ordered auto actions, and
    /// final wipe policy for one unit that cannot be supported.
    pub(in crate::sim) fn resolve_missing_unit_upkeep(
        &mut self,
        unit_id: u32,
        output: UnitUpkeepOutput,
    ) -> bool {
        let Some(unit) = self.unit_by_id(unit_id).cloned() else {
            return false;
        };
        let UnitEconomicsRecordBinding::Mapped { record, .. } =
            freeciv_unit_economics_record(unit.kind)
        else {
            return false;
        };
        let settings = missing_unit_upkeep_settings().for_output(output);
        let flags = record
            .string_list_field("flags")
            .unwrap_or_else(|error| panic!("invalid {}.flags: {error}", record.section))
            .unwrap_or_default();
        let protected = settings.protected_flags.iter().any(|protected| {
            flags
                .iter()
                .any(|flag| flag.eq_ignore_ascii_case(protected))
        });
        if protected {
            return false;
        }

        for action in &settings.actions {
            let target_city = self.city_at(unit.x, unit.y);
            let requires_city = action.eq_ignore_ascii_case("Help Wonder")
                || action.eq_ignore_ascii_case("Disband Unit Recover");
            if requires_city && target_city.is_none() {
                continue;
            }
            let available = self
                .action_available_for_unit_with_record(
                    &unit,
                    action,
                    (unit.x, unit.y),
                    record,
                    target_city,
                )
                .unwrap_or_else(|error| {
                    panic!("could not evaluate C2C3 upkeep action {action}: {error:?}")
                });
            if !available {
                continue;
            }

            if requires_city {
                let target_city_id = target_city.unwrap().id;
                let shield_value = self.unit_shield_value_for_action(&unit, record, action);
                if let Some(index) = self.city_index_by_id(target_city_id) {
                    self.cities[index].shields += shield_value;
                }
            }
            self.remove_units_with_cargo(vec![unit_id]);
            if unit.owner == HUMAN {
                let reason = match output {
                    UnitUpkeepOutput::Food => "famine",
                    UnitUpkeepOutput::Shield => "shield upkeep",
                    UnitUpkeepOutput::Gold => "gold upkeep",
                };
                self.add_event(format!(
                    "{} was disbanded because it could not be supported ({reason}).",
                    unit.kind.name()
                ));
            }
            return true;
        }

        if settings.wipe {
            self.remove_units_with_cargo(vec![unit_id]);
            if unit.owner == HUMAN {
                let reason = match output {
                    UnitUpkeepOutput::Food => "famine",
                    UnitUpkeepOutput::Shield => "shield upkeep",
                    UnitUpkeepOutput::Gold => "gold upkeep",
                };
                self.add_event(format!(
                    "{} was lost because it could not be supported ({reason}).",
                    unit.kind.name()
                ));
            }
            return true;
        }
        false
    }

    pub(in crate::sim) fn unit_shield_value_for_action(
        &self,
        unit: &Unit,
        record: &'static ContentRecord,
        action: &str,
    ) -> i32 {
        let base = record
            .integer_field("build_cost")
            .unwrap_or_else(|error| panic!("invalid {}.build_cost: {error}", record.section))
            .unwrap_or_else(|| panic!("{} has no C2C3 build_cost", record.section));
        let context = SimulationRequirementContext::for_unit(self, unit)
            .with_unit_type_flag_record(record)
            .with_action(action);
        let percent = effect_value(
            &RulesetData::campaign_default().effects,
            "Unit_Shield_Value_Pct",
            &context,
        )
        .unwrap_or_else(|error| panic!("could not evaluate C2C3 unit shield value: {error:?}"));
        let value = i64::from(base) + i64::from(base) * percent / 100;
        i32::try_from(value).unwrap_or_else(|_| panic!("unit shield value exceeds i32"))
    }

    pub(crate) fn home_city_target(&self, unit: &Unit) -> Option<u32> {
        if unit.kind.has_no_home() || unit.home_city.is_none() {
            return None;
        }
        self.cities
            .iter()
            .find(|city| {
                city.owner == unit.owner
                    && city.x == unit.x
                    && city.y == unit.y
                    && unit.home_city != Some(city.id)
            })
            .map(|city| city.id)
    }

    pub(super) fn change_home_city(&mut self, unit_index: usize) -> bool {
        let Some(city_id) = self.home_city_target(&self.units[unit_index]) else {
            return false;
        };
        let unit = &mut self.units[unit_index];
        unit.home_city = Some(city_id);
        unit.home_order = self.next_unit_id;
        self.next_unit_id += 1;
        true
    }

    pub(super) fn release_city_support(&mut self, city_id: u32) {
        self.release_city_support_except(city_id, None);
    }

    pub(super) fn release_city_support_except(&mut self, city_id: u32, keep_owner: Option<usize>) {
        let mut lost = Vec::new();
        for unit in self
            .units
            .iter_mut()
            .filter(|unit| unit.home_city == Some(city_id) && Some(unit.owner) != keep_owner)
        {
            if let Some(city) = self.cities.iter().find(|city| {
                city.id != city_id
                    && city.owner == unit.owner
                    && (city.x, city.y) == (unit.x, unit.y)
            }) {
                unit.home_city = Some(city.id);
                unit.home_order = self.next_unit_id;
                self.next_unit_id += 1;
            } else {
                lost.push(unit.id);
            }
        }
        self.units.retain(|unit| {
            !lost.contains(&unit.id) && unit.transport.is_none_or(|id| !lost.contains(&id))
        });
        self.prune_unit_ai_state();
    }
}

fn record_integer(record: &ContentRecord, field: &str) -> i32 {
    let value = record
        .integer_field(field)
        .unwrap_or_else(|error| panic!("invalid {}.{field}: {error}", record.section))
        .unwrap_or(0);
    i32::try_from(value).unwrap_or_else(|_| panic!("{}.{} exceeds i32", record.section, field))
}

fn record_has_flag(record: &ContentRecord, flag: &str) -> bool {
    record
        .string_list_field("flags")
        .unwrap_or_else(|error| panic!("invalid {}.flags: {error}", record.section))
        .unwrap_or_default()
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(flag))
}

fn use_free_upkeep(cost: i32, free: &mut i32) -> i32 {
    if cost <= 0 {
        return cost;
    }
    if *free > cost {
        *free -= cost;
        0
    } else {
        let remaining = cost - *free;
        *free = 0;
        remaining
    }
}

fn unit_being_aggressive(
    simulation: &Simulation,
    unit: &Unit,
    record: &'static ContentRecord,
) -> bool {
    // Combat identity and attack strength stay on the Civ III content record;
    // the selected C2C3 profile supplies only economic and role requirements.
    if unit.kind.stats().attack <= 0 || simulation.city_at(unit.x, unit.y).is_some() {
        return false;
    }
    let settings = unit_happiness_ruleset_settings();
    if settings.borders_enabled {
        let tile_owner = simulation.tile(unit.x, unit.y).owner;
        match settings.happy_borders {
            HappyBorders::Disabled => {}
            HappyBorders::National if tile_owner == Some(unit.owner) => return false,
            HappyBorders::Allied
                if tile_owner.is_some_and(|owner| simulation.allied(unit.owner, owner)) =>
            {
                return false;
            }
            HappyBorders::National | HappyBorders::Allied => {}
        }
    }

    let Some(base) = simulation.tile(unit.x, unit.y).base else {
        return true;
    };
    let ruleset = RulesetData::campaign_default();
    let Some(extra) = ruleset.rule_records.iter().find(|candidate| {
        candidate.namespace.eq_ignore_ascii_case("extra")
            && candidate.rule_name.eq_ignore_ascii_case(base.name())
    }) else {
        return true;
    };
    let friendliness_range = extra
        .integer_field("no_aggr_near_city")
        .unwrap_or_else(|error| panic!("invalid {}.no_aggr_near_city: {error}", extra.section))
        .unwrap_or(-1);
    if friendliness_range < 0 {
        return true;
    }
    let class = record
        .string_field("class")
        .unwrap_or_else(|error| panic!("invalid {}.class: {error}", record.section));
    let native_classes = extra
        .string_list_field("native_to")
        .unwrap_or_else(|error| panic!("invalid {}.native_to: {error}", extra.section))
        .unwrap_or_default();
    if !class.is_some_and(|class| {
        native_classes
            .iter()
            .any(|native_class| native_class.eq_ignore_ascii_case(class))
    }) {
        return true;
    }

    // `no_aggr_near_city` is supplied by the selected extra record. CivCraft
    // applies its Civ III wrapped neighbor distance here; Freeciv's pinned
    // engine searches a square radius, so the radius shape is an adaptation.
    !simulation.cities.iter().any(|city| {
        simulation.allied(unit.owner, city.owner)
            && simulation
                .map_wrap
                .civ3_distance(unit.x, unit.y, city.x, city.y)
                <= friendliness_range as i32
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::action_rules::{UnitEconomicsBindingKind, UnitEconomicsRecordBinding};

    #[test]
    fn unit_economics_crosswalk_covers_the_full_civ3_roster() {
        let mut direct = 0;
        let mut aliases = 0;
        let mut unsupported = Vec::new();
        for kind in UnitKind::ALL_NATIVE {
            match freeciv_unit_economics_record(kind) {
                UnitEconomicsRecordBinding::Mapped {
                    kind: UnitEconomicsBindingKind::Direct,
                    ..
                } => direct += 1,
                UnitEconomicsRecordBinding::Mapped {
                    kind: UnitEconomicsBindingKind::RoleAlias,
                    ..
                } => aliases += 1,
                UnitEconomicsRecordBinding::Unsupported { source_id, .. } => {
                    unsupported.push(source_id)
                }
            }
        }
        assert_eq!(UnitKind::ALL_NATIVE.len(), 92);
        assert_eq!(direct, 33);
        assert_eq!(aliases, 59);
        assert!(
            unsupported.is_empty(),
            "unmapped Civ III units: {unsupported:?}"
        );
    }

    #[test]
    fn c2c3_per_city_upkeep_applies_shield_to_gold_across_the_full_unit_roster() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 10);
        game.tile_mut(x, y).terrain = Terrain::Desert;
        game.create_city(HUMAN, x, y);
        let city_id = game.cities[0].id;
        game.units.clear();
        game.players[HUMAN].government = Government::Fundamentalism;
        let mut worker_ids = Vec::new();
        for n in 0..3 {
            worker_ids.push(add_home_unit(
                &mut game,
                city_id,
                UnitKind::Worker,
                x + 2 + n,
                y,
            ));
        }
        let first_worker = worker_ids[0];
        let gold = game.city_gold_upkeep(&game.cities[0]);
        assert_eq!(gold.paid, vec![(first_worker, 2)]);
        let upkeep = game.city_unit_upkeep(&game.cities[0]);
        assert_eq!((upkeep.food, upkeep.shields, upkeep.gold), (0, 0, 2));
        assert_eq!(upkeep.unit_happiness, 0);
        assert!(upkeep.per_unit.iter().all(|row| matches!(
            &row.economics,
            UnitUpkeepMapping::Mapped { .. }
        )));
        assert_eq!(
            upkeep
                .per_unit
                .iter()
                .map(|row| row.unit_id)
                .collect::<Vec<_>>(),
            worker_ids.iter().rev().copied().collect::<Vec<_>>()
        );
        assert_eq!(
            upkeep
                .per_unit
                .iter()
                .map(|row| match &row.economics {
                    UnitUpkeepMapping::Mapped { raw, net } => (raw.gold, net.gold),
                    UnitUpkeepMapping::Unsupported(_) => panic!("worker mapping is supported"),
                })
                .collect::<Vec<_>>(),
            vec![(2, 0), (2, 0), (2, 2)]
        );

        let army = game.spawn_unit(HUMAN, UnitKind::Army, x + 4, y);
        let unit = game.units.iter_mut().find(|unit| unit.id == army).unwrap();
        unit.home_city = Some(city_id);
        unit.home_order = army;
        let gold = game.city_gold_upkeep(&game.cities[0]);
        assert_eq!(gold.paid.len(), 2);
        assert_eq!(gold.paid.iter().map(|(_, cost)| cost).sum::<i32>(), 4);
        let upkeep = game.city_unit_upkeep(&game.cities[0]);
        assert!(upkeep.per_unit.iter().all(|row| matches!(
            &row.economics,
            UnitUpkeepMapping::Mapped { .. }
        )));
        assert!(game.units.iter().any(|unit| unit.id == army));
    }

    #[test]
    fn per_unit_upkeep_exposes_raw_and_net_costs_in_freeciv_support_order() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 10);
        game.create_city(HUMAN, x, y);
        let city_id = game.cities[0].id;
        game.players[HUMAN].government = Government::Republic;
        game.units.clear();
        let first = add_home_unit(&mut game, city_id, UnitKind::Warrior, x + 8, y);
        let second = add_home_unit(&mut game, city_id, UnitKind::Warrior, x + 9, y);

        let upkeep = game.city_unit_upkeep(&game.cities[0]);
        assert_eq!(
            upkeep
                .per_unit
                .iter()
                .map(|row| row.unit_id)
                .collect::<Vec<_>>(),
            vec![second, first]
        );
        assert_eq!(
            upkeep
                .per_unit
                .iter()
                .map(|row| match &row.economics {
                    UnitUpkeepMapping::Mapped { raw, net } => {
                        (
                            raw.food,
                            net.food,
                            raw.shields,
                            net.shields,
                            raw.gold,
                            net.gold,
                        )
                    }
                    UnitUpkeepMapping::Unsupported(_) => panic!("warrior mapping is supported"),
                })
                .collect::<Vec<_>>(),
            vec![(1, 0, 1, 0, 0, 0), (1, 0, 1, 1, 0, 0)]
        );
        assert_eq!(upkeep.food, 0);
        assert_eq!(upkeep.shields, 1);
        assert_eq!(upkeep.gold, 0);
    }

    #[test]
    fn supported_units_outside_national_borders_create_c2c3_unhappiness() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 10);
        game.create_city(HUMAN, x, y);
        game.players[HUMAN].government = Government::Democracy;
        let city_id = game.cities[0].id;
        game.units.clear();
        for n in 0..4 {
            let outside_x = x + 8 + n;
            game.tile_mut(outside_x, y).owner = None;
            add_home_unit(&mut game, city_id, UnitKind::Warrior, outside_x, y);
        }
        let upkeep = game.city_unit_upkeep(&game.cities[0]);
        assert_eq!(upkeep.unit_happiness, 6);
        assert!(upkeep.per_unit.iter().all(|row| matches!(
            &row.economics,
            UnitUpkeepMapping::Mapped { .. }
        )));
    }

    #[test]
    fn compatible_units_in_a_nearby_fortress_use_ruleset_no_aggressive_range() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 10);
        game.create_city(HUMAN, x, y);
        game.players[HUMAN].government = Government::Democracy;
        let city_id = game.cities[0].id;
        game.units.clear();
        for n in 0..4 {
            let outside_x = x + 8 + n;
            game.tile_mut(outside_x, y).owner = None;
            add_home_unit(&mut game, city_id, UnitKind::Warrior, outside_x, y);
        }
        let nearby_fortress = (x + 3, y);
        game.tile_mut(nearby_fortress.0, nearby_fortress.1).owner = None;
        game.tile_mut(nearby_fortress.0, nearby_fortress.1).base = Some(Base::Fortress);
        game.units[0].x = nearby_fortress.0;
        game.units[0].y = nearby_fortress.1;

        let upkeep = game.city_unit_upkeep(&game.cities[0]);
        assert_eq!(upkeep.unit_happiness, 4);
    }

    fn add_home_unit(game: &mut Simulation, city_id: u32, kind: UnitKind, x: i32, y: i32) -> u32 {
        let id = game.spawn_unit(HUMAN, kind, x, y);
        let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
        unit.home_city = Some(city_id);
        unit.home_order = id;
        id
    }
}
