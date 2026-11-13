//! Advisor evaluation of spending, reserves, and gold priorities.

use super::*;
use crate::rules::freeciv::{
    ContentKind, EffectRecord, Requirement, RequirementContext, RequirementValue, RulesetData,
};

impl Simulation {
    pub(crate) fn ai_gold_reserve(&self, owner: usize) -> i32 {
        let citizens = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| city.size)
            .sum::<i32>();
        (2 * citizens).max(self.ai_purchase_reserve.get(&owner).copied().unwrap_or(0))
    }

    fn record_ai_purchase_cost(&mut self, owner: usize, price: i32) {
        let highest = self.ai_purchase_reserve.entry(owner).or_default();
        *highest = (*highest).max(price);
    }

    /// Freeciv dai_manage_cities first sells one redundant, noncritical
    /// improvement per city, then sells other noncritical improvements only
    /// when its expected income cannot cover the current treasury deficit.
    fn ai_sell_city_building(&mut self, owner: usize, city_id: u32, redundant_only: bool) -> bool {
        let Some(city) = self
            .cities
            .iter()
            .find(|city| city.owner == owner && city.id == city_id)
            .cloned()
        else {
            return false;
        };
        let Some(building) = Building::all().find(|&building| {
            Self::building_sale_price(&city, building).is_some()
                && !self.ai_building_is_crucial(&city, building)
                && (!redundant_only || self.ai_building_is_redundant(&city, building))
        }) else {
            return false;
        };
        self.sell_building(owner, city_id, building)
    }

    /// Freeciv's grave-danger path sells one eligible improvement, skipping
    /// any building with Defend_Bonus, then retries the urgent defender buy.
    fn ai_sell_one_for_emergency_defense(&mut self, owner: usize, city_id: u32) -> bool {
        let Some(city) = self
            .cities
            .iter()
            .find(|city| city.owner == owner && city.id == city_id)
            .cloned()
        else {
            return false;
        };
        let Some(building) = Building::all().find(|&building| {
            Self::building_sale_price(&city, building).is_some()
                && !Self::ai_building_has_effect(&city, building, "Defend_Bonus")
        }) else {
            return false;
        };
        self.sell_building(owner, city_id, building)
    }

    fn ai_sell_redundant_city_buildings(&mut self, owner: usize) {
        let cities: Vec<_> = self
            .player_cities_ordered(owner)
            .map(|city| city.id)
            .collect();
        for city_id in cities {
            self.ai_sell_city_building(owner, city_id, true);
        }
    }

    fn ai_sell_for_solvency(&mut self, owner: usize) -> bool {
        if self.players[owner].rates[1] < 30
            || self.ai_projected_income(owner) >= -self.players[owner].gold
        {
            return false;
        }

        // daicity.c randomizes seller order with one fc_rand(count) draw for
        // each city, swapping each slot with the selected city.
        let mut sellers: Vec<_> = self
            .player_cities_ordered(owner)
            .map(|city| city.id)
            .collect();
        for index in 0..sellers.len() {
            let replace = self.rand_below(sellers.len() as i32) as usize;
            sellers.swap(index, replace);
        }

        let mut sold_any = false;
        for city_id in sellers {
            if self.ai_projected_income(owner) >= -self.players[owner].gold {
                break;
            }
            sold_any |= self.ai_sell_city_building(owner, city_id, false);
        }
        sold_any
    }

    /// Projected per-turn income follows common/player.c's accounting split:
    /// city gold pays city-style unit upkeep, while mixed/nation styles charge
    /// the nation for paid unit upkeep. C2C3 selects Mixed.
    fn ai_projected_income(&self, owner: usize) -> i32 {
        let style = crate::sim::economy::ruleset::CityEconomySettings::campaign().gold_upkeep_style;
        let mut income = 0;
        for city in self.cities.iter().filter(|city| city.owner == owner) {
            let forecast = self.city_forecast(city);
            income += forecast.gold - forecast.building_upkeep;
            if let Production::Building(building) = city.production {
                let converts_to_gold = Self::ai_ruleset_building_record(building)
                    .and_then(|record| {
                        record.string_list_field("flags").unwrap_or_else(|error| {
                            panic!("invalid C2C3 production building flags: {error}")
                        })
                    })
                    .is_some_and(|flags| {
                        flags.iter().any(|flag| flag.eq_ignore_ascii_case("Gold"))
                    });
                if converts_to_gold {
                    income += city.shields + forecast.shield_surplus();
                }
            }
            match style {
                crate::sim::economy::ruleset::GoldUpkeepStyle::City => {
                    income -= forecast.support.gold;
                }
                crate::sim::economy::ruleset::GoldUpkeepStyle::Mixed
                | crate::sim::economy::ruleset::GoldUpkeepStyle::Nation => {
                    income -= self
                        .city_gold_upkeep(city)
                        .paid
                        .iter()
                        .map(|(_, upkeep)| upkeep)
                        .sum::<i32>();
                }
            }
        }
        if style != crate::sim::economy::ruleset::GoldUpkeepStyle::City {
            income -= self
                .homeless_unit_gold_upkeep(owner)
                .paid
                .iter()
                .map(|(_, upkeep)| upkeep)
                .sum::<i32>();
        }
        income
    }

    fn ai_building_is_crucial(&self, city: &City, building: Building) -> bool {
        Self::ai_building_has_effect(city, building, "Defend_Bonus")
            || self.ai_building_is_productive(city, building)
    }

    fn ai_building_is_productive(&self, city: &City, building: Building) -> bool {
        if Self::ai_building_is_obsolete(self, city, building) {
            return false;
        }
        let rules = RulesetData::campaign_default();
        let Some(record) = Self::ai_ruleset_building_record(building) else {
            // Missing identity data cannot establish that the improvement is
            // harmless, so preserve it during solvency sales.
            return true;
        };
        if Self::ai_building_has_gold_or_infra_flag(record)
            || Self::ai_building_has_side_effect(city, building, record)
        {
            return true;
        }
        Self::ai_building_source_effects(building, rules)
            .into_iter()
            .any(|effect| !Self::ai_effect_is_replaced(self, city, effect))
    }

    fn ai_building_is_redundant(&self, city: &City, building: Building) -> bool {
        if Self::ai_building_is_obsolete(self, city, building) {
            return true;
        }
        let Some(record) = Self::ai_ruleset_building_record(building) else {
            // Unknown ruleset content is not safe to call redundant.
            return false;
        };
        if Self::ai_building_has_gold_or_infra_flag(record)
            || Self::ai_building_has_side_effect(city, building, record)
        {
            return false;
        }
        let effects = Self::ai_building_source_effects(building, RulesetData::campaign_default());
        effects.is_empty()
            || effects
                .iter()
                .all(|effect| Self::ai_effect_is_replaced(self, city, effect))
    }

    fn ai_building_is_obsolete(simulation: &Simulation, city: &City, building: Building) -> bool {
        let Some(record) = Self::ai_ruleset_building_record(building) else {
            return false;
        };
        let context = super::super::action_rules::SimulationRequirementContext::for_city_at(
            simulation,
            city,
            Some((city.x, city.y)),
        );
        record.obsolete_by.iter().any(|requirement| {
            RequirementContext::is_present(&context, requirement)
                .is_ok_and(|present| present == requirement.present)
        })
    }

    fn ai_building_has_effect(city: &City, building: Building, kind: &str) -> bool {
        Self::ai_building_source_effects(building, RulesetData::campaign_default())
            .iter()
            .any(|effect| effect.kind.eq_ignore_ascii_case(kind))
            && city.buildings.contains(&building)
    }

    fn ai_building_source_effects(
        building: Building,
        rules: &'static RulesetData,
    ) -> Vec<&'static EffectRecord> {
        let flags = Self::ai_ruleset_building_record(building)
            .and_then(|record| {
                record
                    .string_list_field("flags")
                    .unwrap_or_else(|error| panic!("invalid C2C3 building flags: {error}"))
            })
            .unwrap_or_default();
        rules
            .effects
            .iter()
            .filter(|effect| {
                effect.requirements.iter().any(|requirement| {
                    requirement.present
                        && ((requirement.kind.eq_ignore_ascii_case("Building")
                            && Self::ai_requirement_names_building(requirement, building))
                            || (requirement.kind.eq_ignore_ascii_case("BuildingFlag")
                                && matches!(&requirement.name, RequirementValue::String(flag)
                                    if flags.iter().any(|candidate| candidate.eq_ignore_ascii_case(flag)))))
                })
            })
            .collect()
    }

    fn ai_ruleset_building_record(
        building: Building,
    ) -> Option<&'static crate::rules::freeciv::ContentRecord> {
        let rules = RulesetData::campaign_default();
        rules.content.iter().find(|record| {
            record.kind == ContentKind::Building
                && (Self::ai_building_name_matches(building, &record.rule_name)
                    || record
                        .display_name
                        .as_deref()
                        .is_some_and(|name| Self::ai_building_name_matches(building, name)))
        })
    }

    fn ai_building_name_matches(building: Building, name: &str) -> bool {
        let canonical = building.data().name;
        canonical.eq_ignore_ascii_case(name)
            || match canonical {
                "Harbor" => name.eq_ignore_ascii_case("Harbour"),
                "Shakespeare's Theater" => name.eq_ignore_ascii_case("Shakespeare's Theatre"),
                _ => false,
            }
    }

    fn ai_requirement_names_building(requirement: &Requirement, building: Building) -> bool {
        let RequirementValue::String(name) = &requirement.name else {
            return false;
        };
        Self::ai_building_name_matches(building, name)
            || Self::ai_ruleset_building_record(building).is_some_and(|record| {
                record.rule_name.eq_ignore_ascii_case(name)
                    || record
                        .display_name
                        .as_deref()
                        .is_some_and(|display| display.eq_ignore_ascii_case(name))
            })
    }

    fn ai_building_has_gold_or_infra_flag(record: &crate::rules::freeciv::ContentRecord) -> bool {
        record
            .string_list_field("flags")
            .unwrap_or_else(|error| panic!("invalid C2C3 building flags: {error}"))
            .is_some_and(|flags| {
                flags.iter().any(|flag| {
                    flag.eq_ignore_ascii_case("Gold") || flag.eq_ignore_ascii_case("Infra")
                })
            })
    }

    fn ai_building_has_side_effect(
        _city: &City,
        building: Building,
        record: &crate::rules::freeciv::ContentRecord,
    ) -> bool {
        let rules = RulesetData::campaign_default();
        let flags = record
            .string_list_field("flags")
            .unwrap_or_else(|error| panic!("invalid C2C3 building flags: {error}"))
            .unwrap_or_default();
        let requirement_needs_building = |requirement: &Requirement| {
            (requirement.kind.eq_ignore_ascii_case("Building")
                && Self::ai_requirement_names_building(requirement, building))
                || (requirement.kind.eq_ignore_ascii_case("BuildingFlag")
                    && matches!(&requirement.name, RequirementValue::String(flag)
                        if flags.iter().any(|candidate| candidate.eq_ignore_ascii_case(flag))))
        };
        rules.action_enablers.iter().any(|enabler| {
            enabler
                .actor_requirements
                .iter()
                .chain(&enabler.target_requirements)
                .any(requirement_needs_building)
        }) || rules
            .content
            .iter()
            .filter(|content| content.kind == ContentKind::Unit)
            .any(|unit| unit.requirements.iter().any(requirement_needs_building))
    }

    fn ai_effect_is_replaced(simulation: &Simulation, city: &City, effect: &EffectRecord) -> bool {
        let context = super::super::action_rules::SimulationRequirementContext::for_city_at(
            simulation,
            city,
            Some((city.x, city.y)),
        );
        effect.requirements.iter().any(|requirement| {
            !requirement.present
                && RequirementContext::is_present(&context, requirement).unwrap_or(false)
        })
    }

    /// Runs Freeciv's redundant-building and solvency sales before city
    /// production choices are calculated.
    pub(crate) fn manage_ai_city_sales(&mut self, owner: usize) {
        self.ai_sell_redundant_city_buildings(owner);
        self.ai_sell_for_solvency(owner);
    }

    pub(crate) fn spend_ai_defense_gold(&mut self, owner: usize) {
        let owned: Vec<_> = self.player_cities_ordered(owner).collect();
        let expenses: i32 = owned
            .iter()
            .map(|city| self.building_upkeep(city) + self.city_unit_upkeep(city).gold)
            .sum();
        let reserve = self.ai_gold_reserve(owner);
        let mut priorities: Vec<_> = owned
            .into_iter()
            .filter_map(|city| {
                let chosen = self.ai_build_choice(city)?;
                // dai_spend_gold consumes the final cached city choice, across
                // all advisor categories. CivCraft stores that choice as the
                // city's current production after prepare_ai_city_defenses.
                if city.production != chosen.production {
                    return None;
                }
                Some((chosen.want, city.id, chosen.production))
            })
            .collect();
        // Stable ties retain the native player-city list order.
        priorities.sort_by(|a, b| b.0.total_cmp(&a.0));
        self.ai_purchase_reserve.remove(&owner);
        let emergency_cities: Vec<_> = priorities
            .iter()
            .filter_map(|&(_, city_id, _)| {
                let city = self.city_by_id(city_id)?;
                let assessment = self.advisor_guard_city_assessment(city);
                (self.city_grave_danger(city) > 0 || assessment.urgency > 1).then_some(city_id)
            })
            .collect();
        for (want, id, target) in priorities {
            let city = self.city_by_id(id).unwrap();
            let position = (city.x, city.y);
            let urgency = self.advisor_guard_city_assessment(city).urgency;
            self.spend_ai_upgrade_gold(owner, Some(position), expenses);
            let city = self.city_by_id(id).unwrap();
            // Upgrading a garrison may change speculative new advice. Native
            // spending still uses its cached choice from city management.
            if city.production != target {
                continue;
            }
            let city = self.city_by_id(id).unwrap();
            if city.shields + self.city_forecast(city).shield_surplus() >= city.production.cost() {
                continue;
            }
            let Some(price) = self.rush_cost(city) else {
                continue;
            };
            let founder = matches!(target, Production::Unit(kind) if kind.stats().can_found);
            if let Production::Unit(kind) = target
                && kind.stats().can_found
                && (city.size <= kind.stats().pop_cost
                    || self.player_cities_ordered(owner).count() > 6
                    || self.ai_on_war_footing(owner))
            {
                continue;
            }
            let limit = if founder { reserve } else { 2 * reserve };
            let remaining = self.players[owner].gold - price;
            let expensive = city.shields == 0 || remaining < limit;
            let assessment = self.advisor_guard_city_assessment(city);
            let grave_danger = self.city_grave_danger(city) > 0;
            let emergency_override = grave_danger && assessment.defense == 0;
            let urgent_want = want > 200.0 && urgency > 1;
            if remaining >= expenses && (!expensive || emergency_override || urgent_want) {
                self.rush_production(owner, id);
            } else if grave_danger
                && matches!(target, Production::Unit(kind)
                    if super::super::action_rules::freeciv_unit_has_role(kind, "DefendGood")
                        || super::super::action_rules::freeciv_unit_has_role(kind, "DefendOk"))
                && assessment.defense == 0
            {
                if self.ai_sell_one_for_emergency_defense(owner, id)
                    && self.players[owner].gold - expenses >= price
                {
                    self.rush_production(owner, id);
                } else {
                    self.record_ai_purchase_cost(owner, price);
                }
            }
        }
        let at_war = self.ai_on_war_footing(owner);
        self.spend_ai_routine_gold(owner, reserve, expenses, at_war, &emergency_cities);
        if !at_war {
            self.spend_ai_upgrade_gold(owner, None, reserve.max(expenses));
        }
    }

    fn spend_ai_routine_gold(
        &mut self,
        owner: usize,
        reserve: i32,
        expenses: i32,
        at_war: bool,
        emergency_cities: &[u32],
    ) {
        let city_count = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .count();
        let gold = self.players[owner].gold;
        let mut purchases = Vec::new();
        for city in self.cities.iter().filter(|city| {
            city.owner == owner && city.shields > 0 && !emergency_cities.contains(&city.id)
        }) {
            let Some(price) = self.rush_cost(city) else {
                continue;
            };
            let mut cash_floor = 2 * reserve;
            if let Production::Unit(kind) = city.production {
                let stats = kind.stats();
                if city.size <= stats.pop_cost {
                    continue;
                }
                if stats.can_found {
                    if at_war || city_count > 6 {
                        continue;
                    }
                    cash_floor = reserve;
                } else if !at_war
                    && price > 2 * stats.cost
                    && kind.is_military()
                    && (stats.attack > stats.defense
                        || self.offensive_production(city) == Some(kind))
                {
                    continue;
                }
            }
            let cash_floor = cash_floor.max(expenses);
            if gold - price < cash_floor {
                continue;
            }

            let missing = city.production.cost() - city.shields;
            let surplus = self.city_forecast(city).shield_surplus();
            if missing <= surplus {
                continue;
            }
            let want = self
                .ai_build_choice(city)
                .filter(|choice| choice.production == city.production)
                .map_or(0.0, |choice| choice.want);
            purchases.push((want, price, city.id, cash_floor));
        }
        purchases.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        for (_, price, city, cash_floor) in purchases {
            if self.players[owner].gold - price >= cash_floor {
                self.rush_production(owner, city);
            }
        }
    }

    fn spend_ai_upgrade_gold(
        &mut self,
        owner: usize,
        defense_city: Option<(i32, i32)>,
        limit: i32,
    ) {
        let upgrades: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == owner
                    && match defense_city {
                        Some(position) => unit.kind.is_military() && (unit.x, unit.y) == position,
                        None => !unit.kind.is_military(),
                    }
            })
            .filter_map(|unit| {
                self.paid_upgrade(unit)
                    .map(|(target, price)| (unit.id, target, price))
            })
            .collect();
        for (unit, target, price) in upgrades {
            if self.players[owner].gold - price > limit {
                self.upgrade_unit(unit, target, price);
            } else {
                self.record_ai_purchase_cost(owner, price);
            }
        }
    }
}

#[cfg(test)]
mod city_sale_tests {
    use super::*;

    fn obsolete_redundant_building(game: &mut Simulation) -> Building {
        game.cities[0].buildings.push(Building::BARRACKS_II);
        let city = game.cities[0].clone();
        assert!(Simulation::ai_building_is_obsolete(
            game,
            &city,
            Building::BARRACKS
        ));
        assert!(game.ai_building_is_redundant(&city, Building::BARRACKS));
        Building::BARRACKS
    }

    fn test_game() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.create_city(1, 10, 24);
        game.players[1].gold = 0;
        game
    }

    #[test]
    fn sale_pass_keeps_defensive_and_productive_buildings_and_sells_at_most_one() {
        let mut game = test_game();
        let candidate = obsolete_redundant_building(&mut game);
        game.cities[0]
            .buildings
            .extend([Building::CITY_WALLS, Building::LIBRARY, candidate]);
        let city_id = game.cities[0].id;

        assert!(Simulation::ai_building_has_effect(
            &game.cities[0],
            Building::CITY_WALLS,
            "Defend_Bonus"
        ));
        assert!(game.ai_building_is_crucial(&game.cities[0], Building::CITY_WALLS));
        assert!(game.ai_building_is_crucial(&game.cities[0], Building::LIBRARY));
        assert!(game.ai_sell_city_building(1, city_id, true));

        let city = game.city_by_id(city_id).unwrap();
        assert!(!city.buildings.contains(&candidate));
        assert!(city.buildings.contains(&Building::CITY_WALLS));
        assert!(city.buildings.contains(&Building::LIBRARY));
        assert!(city.sold_building);
        assert!(!game.ai_sell_city_building(1, city_id, false));
    }

    #[test]
    fn solvency_sales_wait_for_thirty_percent_tax_and_require_a_real_deficit() {
        let mut game = test_game();
        let candidate = obsolete_redundant_building(&mut game);
        // Large upkeep makes the forecast negative while keeping the Library
        // productive, so the sale pass has only the selected candidate.
        game.cities[0].buildings.extend([candidate]);
        game.cities[0]
            .buildings
            .extend(std::iter::repeat(Building::LIBRARY).take(20));
        game.players[1].rates = [0, 29, 71];
        assert!(game.ai_projected_income(1) < -game.players[1].gold);
        assert!(!game.ai_sell_for_solvency(1));
        assert!(game.cities[0].buildings.contains(&candidate));

        game.players[1].rates[1] = 30;
        game.players[1].gold = 1_000;
        assert!(game.ai_projected_income(1) >= -game.players[1].gold);
        assert!(!game.ai_sell_for_solvency(1));
        assert!(game.cities[0].buildings.contains(&candidate));

        game.players[1].gold = 0;
        assert!(game.ai_projected_income(1) < -game.players[1].gold);
        assert!(game.ai_sell_for_solvency(1));
        assert!(!game.cities[0].buildings.contains(&candidate));
        assert!(game.cities[0].buildings.contains(&Building::LIBRARY));
    }
}
