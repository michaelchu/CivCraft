//! Unit support and upkeep calculations.

use super::*;
use crate::rules::engine_rules::UnitUpkeepOutput;
use crate::sim::action_rules::SimulationRequirementContext;
use std::sync::OnceLock;

pub(crate) struct UnitUpkeep {
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) gold: i32,
    /// Source-order supported units, with both pre-allowance and post-allowance
    /// costs for the city-turn missing-upkeep resolver.
    pub(crate) per_unit: Vec<UnitUpkeepRow>,
}

impl UnitUpkeep {
    /// Reuse the gold allocation already produced with a city forecast instead
    /// of scanning and sorting every supported unit a second time.
    pub(super) fn gold_upkeep(&self) -> UnitGoldUpkeep {
        let paid = self
            .per_unit
            .iter()
            .filter_map(|row| (row.costs.gold > 0).then_some((row.unit_id, row.costs.gold)))
            .collect();
        UnitGoldUpkeep { paid }
    }
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
    pub(crate) costs: UnitOutputCosts,
}

fn missing_unit_upkeep_settings() -> &'static crate::rules::engine_rules::MissingUnitUpkeepSettings
{
    use crate::rules::engine_rules::{MissingUnitUpkeepOutputSettings, MissingUnitUpkeepSettings};
    static SETTINGS: OnceLock<MissingUnitUpkeepSettings> = OnceLock::new();
    SETTINGS.get_or_init(|| MissingUnitUpkeepSettings {
        food: MissingUnitUpkeepOutputSettings {
            protected_flags: vec!["EvacuateFirst".to_owned()],
            actions: Vec::new(),
            wipe: true,
        },
        shield: MissingUnitUpkeepOutputSettings {
            protected_flags: vec!["EvacuateFirst".to_owned()],
            actions: vec![
                "Help Wonder".to_owned(),
                "Disband Unit Recover".to_owned(),
                "Disband Unit".to_owned(),
            ],
            wipe: false,
        },
        gold: MissingUnitUpkeepOutputSettings {
            protected_flags: Vec::new(),
            actions: Vec::new(),
            wipe: true,
        },
    })
}

pub(crate) struct UnitGoldUpkeep {
    pub(crate) paid: Vec<(u32, i32)>,
}

impl Simulation {
    /// Allocate national support slots once in stable unit-ID order. Home-city
    /// assignments are retained for save/UI compatibility, not for eligibility.
    pub(crate) fn national_unit_gold_upkeep(&self, owner: usize) -> UnitGoldUpkeep {
        self.national_unit_gold_upkeep_for_government(owner, self.players[owner].government)
    }

    pub(in crate::sim) fn national_unit_gold_upkeep_for_government(
        &self,
        owner: usize,
        government: Government,
    ) -> UnitGoldUpkeep {
        let government = government
            .rule_name()
            .and_then(|name| crate::rules::native::governments().get(name))
            .expect("every campaign government has a native rule");
        let mut free = 0i64;
        for city in self.cities.iter().filter(|city| city.owner == owner) {
            let allowance = match city.size {
                ..=6 => government.town_free_units,
                7..=12 => government.city_free_units,
                _ => government.metropolis_free_units,
            };
            if allowance < 0 {
                return UnitGoldUpkeep { paid: Vec::new() };
            }
            free += i64::from(allowance);
        }
        let mut supported: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.owner == owner && unit.kind.conquests_rule().require_support != 0)
            .collect();
        supported.sort_by_key(|unit| unit.id);
        let paid = supported
            .into_iter()
            .skip(usize::try_from(free).unwrap_or(usize::MAX))
            .filter_map(|unit| {
                (government.cost_per_excess_unit_gold > 0)
                    .then_some((unit.id, government.cost_per_excess_unit_gold))
            })
            .collect();
        UnitGoldUpkeep { paid }
    }

    pub(crate) fn city_unit_upkeep(&self, city: &City) -> UnitUpkeep {
        let paid = self.national_unit_gold_upkeep(city.owner).paid;
        let mut per_unit = Vec::new();
        let mut gold = 0;
        for unit in self
            .units
            .iter()
            .filter(|unit| unit.owner == city.owner && unit.home_city == Some(city.id))
        {
            let net_gold = paid
                .iter()
                .find(|(id, _)| *id == unit.id)
                .map_or(0, |(_, cost)| *cost);
            gold += net_gold;
            per_unit.push(UnitUpkeepRow {
                unit_id: unit.id,
                costs: UnitOutputCosts {
                    gold: net_gold,
                    ..Default::default()
                },
            });
        }
        UnitUpkeep {
            food: 0,
            shields: 0,
            gold,
            per_unit,
        }
    }

    pub(crate) fn city_unit_happiness(&self, _city: &City) -> i32 {
        // Civ III uses war weariness, not per-home-city field-unit unhappiness.
        // A full war-weariness model is separate from this unit-stat migration.
        0
    }

    pub(super) fn city_gold_upkeep(&self, city: &City) -> UnitGoldUpkeep {
        self.city_unit_upkeep(city).gold_upkeep()
    }

    pub(super) fn homeless_unit_gold_upkeep(&self, owner: usize) -> UnitGoldUpkeep {
        let mut upkeep = self.national_unit_gold_upkeep(owner);
        upkeep.paid.retain(|(id, _)| {
            self.unit_by_id(*id)
                .is_some_and(|unit| unit.home_city.is_none())
        });
        upkeep
    }

    pub(super) fn virtual_unit_raw_food_upkeep(&self, _city: &City, _unit: &Unit) -> Option<i32> {
        Some(0)
    }

    pub(in crate::sim) fn balance_unit_gold_upkeep(&mut self, owner: usize, paid: &[(u32, i32)]) {
        while self.players[owner].gold < 0 {
            let candidates: Vec<_> = self
                .player_units_ordered(owner)
                .filter(|unit| paid.iter().any(|&(id, _)| id == unit.id))
                .map(|unit| unit.id)
                .collect();
            if candidates.is_empty() {
                let before = self.players[owner].gold;
                self.players[owner].gold = 0;
                self.trace_upkeep_shortfall(owner, "unpaid_unit_upkeep", None, None, before);
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
            let before = self.players[owner].gold;
            let kind = self.unit_by_id(id).map(|unit| unit.kind.name());
            if self.resolve_missing_unit_upkeep(id, UnitUpkeepOutput::Gold) {
                self.players[owner].gold += cost;
                self.trace_upkeep_shortfall(
                    owner,
                    "unit_upkeep_action",
                    Some(trace::Entity::Unit(id)),
                    kind,
                    before,
                );
            } else {
                // Freeciv stops selecting units when the ruleset's gold
                // protected/actions/wipe policy cannot remove the candidate.
                self.players[owner].gold = 0;
                self.trace_upkeep_shortfall(
                    owner,
                    "unpaid_unit_upkeep",
                    Some(trace::Entity::Unit(id)),
                    kind,
                    before,
                );
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
        let native = super::native_unit_rules::get(unit.kind);
        let settings = missing_unit_upkeep_settings().for_output(output);
        let protected = settings.protected_flags.iter().any(|protected| {
            native
                .flags
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
                .action_available_for_unit(&unit, action, (unit.x, unit.y))
                .unwrap_or_else(|error| {
                    panic!("could not evaluate C2C3 upkeep action {action}: {error:?}")
                });
            if !available {
                continue;
            }

            if requires_city {
                let target_city_id = target_city.unwrap().id;
                let shield_value = self.unit_shield_value_for_action(&unit, action);
                if let Some(index) = self.city_index_by_id(target_city_id) {
                    self.cities[index].shields += shield_value;
                }
            }
            self.remove_units_with_cargo_because(
                vec![unit_id],
                lifecycle::UnitRemoval::UpkeepDisbanded,
            );
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
            self.remove_units_with_cargo_because(
                vec![unit_id],
                lifecycle::UnitRemoval::UpkeepDisbanded,
            );
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

    pub(in crate::sim) fn unit_shield_value_for_action(&self, unit: &Unit, action: &str) -> i32 {
        let base = super::action_rules::unit_build_cost(unit.kind);
        let context = SimulationRequirementContext::for_unit(self, unit).with_action(action);
        let percent = context.effect(EffectKind::UnitShieldValuePct);
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
        // Army members can be nested inside a naval transport. Remove the
        // complete unsupported cargo tree, not just its first generation.
        let mut index = 0;
        while let Some(&carrier) = lost.get(index) {
            let cargo: Vec<_> = self
                .units
                .iter()
                .filter(|u| u.transport == Some(carrier) && !lost.contains(&u.id))
                .map(|u| u.id)
                .collect();
            lost.extend(cargo);
            index += 1;
        }
        for unit in self.units.iter().filter(|unit| lost.contains(&unit.id)) {
            self.trace
                .unit_removed(self.turn, unit, lifecycle::UnitRemoval::CityLost);
        }
        self.units.retain(|unit| {
            !lost.contains(&unit.id) && unit.transport.is_none_or(|id| !lost.contains(&id))
        });
        self.rebuild_armies();
        self.prune_unit_ai_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_support_is_national_gold_and_independent_of_home_city() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.create_city(HUMAN, 10, 10);
        game.players[HUMAN].government = Government::DESPOTISM;
        let city_id = game.cities[0].id;
        let ids: Vec<_> = (0..6)
            .map(|_| game.spawn_unit(HUMAN, UnitKind::Worker, 10, 10))
            .collect();
        assert_eq!(
            game.national_unit_gold_upkeep(HUMAN).paid,
            vec![(ids[4], 1), (ids[5], 1)]
        );
        for unit in &mut game.units {
            unit.home_city = Some(city_id);
        }
        let upkeep = game.city_unit_upkeep(&game.cities[0]);
        assert_eq!(
            (
                upkeep.food,
                upkeep.shields,
                upkeep.gold,
                game.city_unit_happiness(&game.cities[0])
            ),
            (0, 0, 2, 0)
        );
        for unit in &mut game.units {
            unit.home_city = None;
        }
        assert_eq!(
            game.homeless_unit_gold_upkeep(HUMAN).paid,
            vec![(ids[4], 1), (ids[5], 1)]
        );
        game.cities[0].size = 13;
        game.players[HUMAN].government = Government::MONARCHY;
        assert!(game.national_unit_gold_upkeep(HUMAN).paid.is_empty());
        game.players[HUMAN].government = Government::DEMOCRACY;
        assert_eq!(game.national_unit_gold_upkeep(HUMAN).paid.len(), 6);
    }

    #[test]
    fn support_exemption_comes_from_native_require_support() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.players[HUMAN].government = Government::DEMOCRACY;
        for kind in UnitKind::ALL_NATIVE {
            game.spawn_unit(HUMAN, kind, 10, 10);
        }
        let paid = game.national_unit_gold_upkeep(HUMAN).paid;
        for unit in &game.units {
            assert_eq!(
                paid.iter().any(|(id, _)| *id == unit.id),
                unit.kind.conquests_rule().require_support != 0
            );
        }
    }
}
