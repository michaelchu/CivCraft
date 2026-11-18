//! Human inspection projections, not analyzer inputs or a replay protocol.
//! Unknown providers keep all fields so new adapters do not silently lose evidence.
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

type Rows = BTreeMap<String, Value>;
#[derive(Default)]
pub(super) struct ObservationChanges {
    previous: BTreeMap<String, Rows>,
}

fn project(provider: &str, mut row: Value) -> Value {
    let omitted: &[&str] = match provider {
        "cities" => &[
            "food",
            "food_output",
            "food_surplus",
            "growth_threshold",
            "food_to_growth",
            "projected_growth_turns",
            "worked_tiles",
            "support_food",
        ],
        "civilizations" => &["gold", "score"],
        "units" => &["x", "y", "moves_left", "work_left", "goto"],
        "research.civilizations" => &["progress", "cost", "science_output", "totals"],
        "governments" => &[
            "candidates",
            "evaluated_turn",
            "next_evaluation",
            "revolution_turn",
        ],
        "diplomacy" => &["trust", "war_plans", "until"],
        _ => &[],
    };
    if provider == "governments" {
        let available: Vec<_> = row
            .get("candidates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|c| c["available"] == true)
            .map(|c| c["government"].clone())
            .collect();
        row["available_governments"] = json!(available);
    }
    if provider == "diplomacy" {
        let bands: Vec<_> = row
            .get("trust")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|trust| trust.as_i64().map(|trust| trust / 1000))
            .collect();
        row["trust_bands_1000"] = json!(bands);
        let plans: Vec<_> = row.get("war_plans").and_then(Value::as_array).into_iter().flatten()
            .map(|plan| if plan.is_null() {Value::Null} else {json!({"reason":plan["reason"],
                "status":match plan["countdown"].as_i64() {Some(n) if n >= 0 => "preparing", Some(-1) => "inactive", _ => "cooldown"}})}).collect();
        row["war_plan_status"] = json!(plans);
    }
    if let Some(object) = row.as_object_mut() {
        for key in omitted {
            object.remove(*key);
        }
    }
    row
}
fn rows(provider: &str, value: &Value) -> Rows {
    match value.as_array() {
        Some(values) => {
            let collected: Rows = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let key = if provider == "lifecycle.unit_totals" {
                        json!([value["owner"], value["kind"]])
                    } else {
                        ["id", "owner", "players"]
                            .iter()
                            .find_map(|key| value.get(key))
                            .cloned()
                            .unwrap_or(json!(index))
                    };
                    (key.to_string(), project(provider, value.clone()))
                })
                .collect();
            if collected.len() == values.len() {
                collected
            } else {
                [("state".into(), value.clone())].into()
            }
        }
        None => [("state".into(), value.clone())].into(),
    }
}
impl ObservationChanges {
    pub fn observe(&mut self, snapshot: &Value) -> Vec<Value> {
        let mut sections = BTreeMap::new();
        let mut changes = Vec::new();
        for (provider, value) in snapshot.as_object().expect("snapshot is an object") {
            if ["turn", "phase", "city_count"].contains(&provider.as_str()) {
                continue;
            }
            if ["research", "lifecycle"].contains(&provider.as_str()) {
                let mut metadata = value.clone();
                for collection in ["civilizations", "unit_totals", "city_totals"] {
                    if let Some(value) = metadata.as_object_mut().and_then(|m| m.remove(collection))
                    {
                        let name = format!("{provider}.{collection}");
                        sections.insert(name.clone(), rows(&name, &value));
                    }
                }
                if let Some(events) = metadata.as_object_mut().and_then(|m| m.remove("changes")) {
                    // These are boundary-local facts, not persistent state. Never deduplicate them.
                    if events.as_array().is_some_and(|events| !events.is_empty()) {
                        changes.push(
                            json!({"provider":format!("{provider}.changes"),"events":events}),
                        );
                    }
                }
                sections.insert(provider.clone(), rows(provider, &metadata));
            } else {
                sections.insert(provider.clone(), rows(provider, value));
            }
        }
        for provider in sections
            .keys()
            .chain(self.previous.keys())
            .collect::<BTreeSet<_>>()
        {
            let empty = Rows::new();
            let before = self.previous.get(provider).unwrap_or(&empty);
            let after = sections.get(provider).unwrap_or(&empty);
            for key in before.keys().chain(after.keys()).collect::<BTreeSet<_>>() {
                if before.get(key) != after.get(key) {
                    changes.push(json!({"provider":provider,"key":key,
                        "before":before.get(key),"after":after.get(key)}));
                }
            }
        }
        self.previous = sections;
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_entity_keys_ignore_order_and_keep_creation_change_and_removal() {
        let mut output = ObservationChanges::default();
        let mut snapshot = json!({"turn":1,"cities":[{"id":1,"population":2,"food":1},{"id":2,"population":1,"food":3}]});
        assert_eq!(output.observe(&snapshot).len(), 2);
        snapshot["turn"] = json!(2);
        snapshot["cities"] =
            json!([{"id":2,"population":1,"food":8},{"id":1,"population":2,"food":9}]);
        assert!(output.observe(&snapshot).is_empty());
        snapshot["cities"][1]["population"] = json!(3);
        let changes = output.observe(&snapshot);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0]["before"]["population"], 2);
        assert_eq!(changes[0]["after"]["population"], 3);
        snapshot["cities"] = json!([]);
        assert!(
            output
                .observe(&snapshot)
                .iter()
                .all(|c| c["after"].is_null())
        );
    }
    #[test]
    fn preserves_boundary_local_history_even_if_two_boundaries_match() {
        let mut output = ObservationChanges::default();
        let snapshot = json!({"lifecycle":{"changes":[{"event":"unit_created"}]},
            "research":{"changes":[{"action":"acquired"}]}});
        output.observe(&snapshot);
        let changes = output.observe(&snapshot);
        assert_eq!(changes.len(), 2);
        assert!(
            changes
                .iter()
                .all(|c| c["events"].as_array().unwrap().len() == 1)
        );
    }
    #[test]
    fn ignores_routine_research_and_movement_but_preserves_damage_and_discoveries() {
        let mut output = ObservationChanges::default();
        let mut snapshot = json!({"units":[{"id":1,"kind":"Warrior","hp":10,"x":1,"moves_left":2}],
            "research":{"civilizations":[{"owner":0,"progress":2,"known_technology_ids":[1]}]}});
        output.observe(&snapshot);
        snapshot["units"][0]["x"] = json!(2);
        snapshot["units"][0]["moves_left"] = json!(1);
        snapshot["research"]["civilizations"][0]["progress"] = json!(3);
        assert!(output.observe(&snapshot).is_empty());
        snapshot["units"][0]["hp"] = json!(8);
        snapshot["research"]["civilizations"][0]["known_technology_ids"] = json!([1, 2]);
        assert_eq!(output.observe(&snapshot).len(), 2);
    }
    #[test]
    fn unknown_provider_fields_are_preserved() {
        let mut output = ObservationChanges::default();
        output.observe(&json!({"custom":{"tick":1,"flag":false}}));
        let changes = output.observe(&json!({"custom":{"tick":2,"flag":false}}));
        assert_eq!(changes[0]["after"], json!({"tick":2,"flag":false}));
    }
}
