//! Automated gameplay scenarios covering unit catalog behavior.

use super::*;

impl Simulation {
    pub(crate) fn native_unit_catalog_playtest(&self) -> (bool, bool, bool) {
        let rules = crate::sim::civ3_unit_rules::all();
        let roster = UnitKind::ALL_NATIVE;
        let source_matches = roster.iter().zip(rules).all(|(kind, rule)| {
            kind.conquests_id() == Some(rule.id) && kind.name() == rule.name && {
                let stats = kind.stats();
                (
                    stats.cost,
                    stats.pop_cost,
                    stats.attack,
                    stats.defense,
                    stats.moves,
                ) == (
                    rule.cost,
                    rule.population_cost,
                    rule.attack,
                    rule.defense,
                    rule.moves,
                )
            }
        });
        let civ_rules_complete = roster.iter().all(|kind| {
            kind.conquests_id()
                .and_then(crate::sim::civ3_unit_rules::get)
                .is_some_and(|rule| {
                    rule.category == "wonder_created"
                        || !rule.available_to.is_empty()
                        || matches!(rule.id, "army" | "leader")
                })
        });
        let order = roster.len() == 92
            && rules.len() == 92
            && roster
                .iter()
                .map(|kind| kind.conquests_id())
                .eq(rules.iter().map(|rule| Some(rule.id)));
        (source_matches, civ_rules_complete, order)
    }
}
