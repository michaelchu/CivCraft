//! Lossless verbose storage. Decoded observations remain identical to analyzer inputs.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Segment {
    Key(String),
    Index(usize),
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Edit {
    Set { path: Vec<Segment>, value: Value },
    Remove { path: Vec<Segment> },
    Truncate { path: Vec<Segment>, length: usize },
}
fn differences(before: &Value, after: &Value, path: &mut Vec<Segment>, edits: &mut Vec<Edit>) {
    if before == after {
        return;
    }
    match (before, after) {
        (Value::Object(a), Value::Object(b)) => {
            for key in a.keys().filter(|key| !b.contains_key(*key)) {
                path.push(Segment::Key(key.clone()));
                edits.push(Edit::Remove { path: path.clone() });
                path.pop();
            }
            for (key, value) in b {
                path.push(Segment::Key(key.clone()));
                if let Some(old) = a.get(key) {
                    differences(old, value, path, edits);
                } else {
                    edits.push(Edit::Set {
                        path: path.clone(),
                        value: value.clone(),
                    });
                }
                path.pop();
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for (index, value) in b.iter().enumerate() {
                path.push(Segment::Index(index));
                if let Some(old) = a.get(index) {
                    differences(old, value, path, edits);
                } else {
                    edits.push(Edit::Set {
                        path: path.clone(),
                        value: value.clone(),
                    });
                }
                path.pop();
            }
            if b.len() < a.len() {
                edits.push(Edit::Truncate {
                    path: path.clone(),
                    length: b.len(),
                });
            }
        }
        _ => edits.push(Edit::Set {
            path: path.clone(),
            value: after.clone(),
        }),
    }
}
fn at<'a>(mut value: &'a mut Value, path: &[Segment]) -> Result<&'a mut Value, String> {
    for segment in path {
        value = match segment {
            Segment::Key(key) => value.as_object_mut().and_then(|v| v.get_mut(key)),
            Segment::Index(index) => value.as_array_mut().and_then(|v| v.get_mut(*index)),
        }
        .ok_or("verbose edit references a missing path")?;
    }
    Ok(value)
}
fn apply(state: &mut Value, edit: Edit) -> Result<(), String> {
    match edit {
        Edit::Set { path, value } if path.is_empty() => *state = value,
        Edit::Set { path, value } => {
            let (last, parent) = path.split_last().ok_or("empty edit path")?;
            match (last, at(state, parent)?) {
                (Segment::Key(key), Value::Object(object)) => {
                    object.insert(key.clone(), value);
                }
                (Segment::Index(index), Value::Array(array)) if *index <= array.len() => {
                    if *index == array.len() {
                        array.push(value);
                    } else {
                        array[*index] = value;
                    }
                }
                _ => return Err("invalid verbose set target".into()),
            }
        }
        Edit::Remove { path } => {
            let (last, parent) = path.split_last().ok_or("cannot remove snapshot root")?;
            match (last, at(state, parent)?) {
                (Segment::Key(key), Value::Object(object)) if object.contains_key(key) => {
                    object.remove(key);
                }
                _ => return Err("invalid verbose remove target".into()),
            }
        }
        Edit::Truncate { path, length } => match at(state, &path)? {
            Value::Array(array) if length <= array.len() => array.truncate(length),
            _ => return Err("invalid verbose array length".into()),
        },
    }
    Ok(())
}
#[derive(Default)]
pub(super) struct Snapshots {
    previous: Option<Value>,
    index: u64,
}
impl Snapshots {
    pub fn encode(&mut self, snapshot: Value) -> Value {
        let mut edits = Vec::new();
        if let Some(previous) = &self.previous {
            differences(previous, &snapshot, &mut Vec::new(), &mut edits);
        }
        let full = json!({"record":"full","index":self.index,"snapshot":snapshot});
        let delta = json!({"record":"delta","index":self.index,"base":self.index.saturating_sub(1),
            "turn":snapshot["turn"],"phase":snapshot["phase"],"edits":edits});
        // Periodic baselines bound dependency chains; replacing a large array can also be cheaper.
        let record = if self.previous.is_none()
            || self.index.is_multiple_of(64)
            || full.to_string().len() <= delta.to_string().len()
        {
            full
        } else {
            delta
        };
        self.previous = Some(snapshot);
        self.index += 1;
        record
    }
}
#[derive(Default)]
pub(super) struct SnapshotReader {
    previous: Option<Value>,
    index: Option<u64>,
}
impl SnapshotReader {
    pub fn decode(&mut self, record: Value) -> Result<Value, String> {
        let index = record["index"]
            .as_u64()
            .ok_or("verbose snapshot has no index")?;
        if index != self.index.map_or(0, |index| index + 1) {
            return Err("verbose snapshot sequence gap".into());
        }
        let state = match record["record"].as_str() {
            Some("full") => record
                .get("snapshot")
                .cloned()
                .ok_or("missing full snapshot")?,
            Some("delta") => {
                if record["base"].as_u64() != self.index {
                    return Err("verbose snapshot base mismatch".into());
                }
                let mut state = self
                    .previous
                    .clone()
                    .ok_or("delta before snapshot baseline")?;
                let edits: Vec<Edit> =
                    serde_json::from_value(record["edits"].clone()).map_err(|e| e.to_string())?;
                for edit in edits {
                    apply(&mut state, edit)?;
                }
                if state["turn"] != record["turn"] || state["phase"] != record["phase"] {
                    return Err("verbose snapshot boundary mismatch".into());
                }
                state
            }
            _ => return Err("unknown verbose snapshot record".into()),
        };
        self.previous = Some(state.clone());
        self.index = Some(index);
        Ok(state)
    }
}
#[derive(Default)]
pub(super) struct SharedValues {
    ids: BTreeMap<String, u64>,
}
pub(super) struct EncodedEvent {
    pub bytes: Vec<u8>,
    pub values: Vec<(String, Vec<u8>)>,
}
fn line(value: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}
impl SharedValues {
    pub fn plan(&self, mut event: Value) -> Result<EncodedEvent, String> {
        let mut shared = BTreeMap::new();
        let mut values: Vec<(String, Vec<u8>)> = Vec::new();
        if let Some(payload) = event.get_mut("payload").and_then(Value::as_object_mut) {
            for key in payload.keys().cloned().collect::<Vec<_>>() {
                let value = &payload[&key];
                if !value.is_array() && !value.is_object() {
                    continue;
                }
                let serialized = serde_json::to_string(value).map_err(|e| e.to_string())?;
                if serialized.len() < 128 {
                    continue;
                }
                let id = if let Some(&id) = self.ids.get(&serialized) {
                    id
                } else if let Some(index) = values.iter().position(|(key, _)| *key == serialized) {
                    self.ids.len() as u64 + index as u64
                } else {
                    let id = (self.ids.len() + values.len()) as u64;
                    values.push((serialized, line(&json!({"id":id,"value":value}))?));
                    id
                };
                payload.remove(&key);
                shared.insert(key, id);
            }
        }
        Ok(EncodedEvent {
            bytes: line(&json!({"event":event,"shared":shared}))?,
            values,
        })
    }
    pub fn commit(&mut self, values: &[(String, Vec<u8>)]) {
        for (key, _) in values {
            let id = self.ids.len() as u64;
            self.ids.insert(key.clone(), id);
        }
    }
}
pub(super) fn expand_event(record: Value, values: &BTreeMap<u64, Value>) -> Result<Value, String> {
    let shared: BTreeMap<String, u64> =
        serde_json::from_value(record["shared"].clone()).map_err(|e| e.to_string())?;
    let mut event = record
        .get("event")
        .cloned()
        .ok_or("missing verbose event")?;
    for (key, id) in shared {
        let value = values
            .get(&id)
            .ok_or("event references a missing shared value")?;
        let payload = event
            .get_mut("payload")
            .and_then(Value::as_object_mut)
            .ok_or("shared value requires an object payload")?;
        if payload.insert(key, value.clone()).is_some() {
            return Err("shared value conflicts with an inline field".into());
        }
    }
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_round_trip_every_field_across_creation_removal_reordering_and_baselines() {
        let mut encoder = Snapshots::default();
        let mut decoder = SnapshotReader::default();
        let mut deltas = 0;
        for turn in 0..130 {
            let mut rows: Vec<_> = (0..(turn % 5 + 1))
                .map(|id| {
                    json!({"id":id,"food":turn,
                "static":"unchanged repeated static data".repeat(12),"hp":10,"optional":null})
                })
                .collect();
            if turn % 2 == 0 {
                rows.reverse();
            }
            let mut snapshot = json!({"turn":turn,"phase":"turn_end","cities":rows,
                "nested":{"exact_float":105.95744323730469,"large_integer":u64::MAX,"empty":[]}});
            if turn % 3 == 0 {
                snapshot["nested"].as_object_mut().unwrap().remove("empty");
            }
            let encoded = encoder.encode(snapshot.clone());
            if encoded["record"] == "delta" {
                deltas += 1;
            }
            if turn % 64 == 0 {
                assert_eq!(encoded["record"], "full");
            }
            // Include serialization/parsing so precision and field types are tested on disk format.
            assert_eq!(
                decoder
                    .decode(serde_json::from_str(&encoded.to_string()).unwrap())
                    .unwrap(),
                snapshot
            );
        }
        assert!(deltas > 50);
    }
    #[test]
    fn event_values_are_shared_without_losing_decisions_or_inline_fields() {
        let mut encoder = SharedValues::default();
        let mut dictionary = BTreeMap::new();
        let advice = json!({"scores":vec![123.125; 100],"name":"Nation"});
        for sequence in 0..3 {
            let event = json!({"sequence":sequence,"payload":{"advice":advice,"selected":"Monarchy","null":null}});
            let encoded = encoder.plan(event.clone()).unwrap();
            assert_eq!(encoded.values.len(), usize::from(sequence == 0));
            for (_, bytes) in &encoded.values {
                let record: Value = serde_json::from_slice(bytes).unwrap();
                dictionary.insert(record["id"].as_u64().unwrap(), record["value"].clone());
            }
            assert_eq!(
                expand_event(serde_json::from_slice(&encoded.bytes).unwrap(), &dictionary).unwrap(),
                event
            );
            encoder.commit(&encoded.values);
        }
        assert_eq!(dictionary.len(), 1);
    }
    #[test]
    fn rejected_event_plans_do_not_create_dangling_dictionary_references() {
        let mut encoder = SharedValues::default();
        let event = json!({"payload":{"advice":{"text":"a".repeat(256)}}});
        let rejected = encoder.plan(event.clone()).unwrap();
        let retry = encoder.plan(event).unwrap();
        assert_eq!(rejected.values, retry.values);
        encoder.commit(&retry.values);
        let later = encoder
            .plan(json!({"payload":{"advice":{"text":"b".repeat(256)}}}))
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&later.values[0].1).unwrap()["id"],
            1
        );
    }
    #[test]
    fn malformed_dependencies_fail_explicitly() {
        let mut decoder = SnapshotReader::default();
        assert!(
            decoder
                .decode(json!({"record":"delta","index":0,"base":0,"edits":[]}))
                .is_err()
        );
        decoder
            .decode(json!({"record":"full","index":0,"snapshot":{"turn":1}}))
            .unwrap();
        assert!(
            decoder
                .decode(json!({"record":"delta","index":2,"base":0,"edits":[]}))
                .is_err()
        );
        assert!(
            expand_event(
                json!({"event":{"payload":{}},"shared":{"advice":5}}),
                &BTreeMap::new()
            )
            .is_err()
        );
    }
}
