//! Expand stored evidence into ordinary JSONL for inspection tools and agents.
use super::verbose_codec::{SnapshotReader, expand_event};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader, Write},
    path::Path,
};

fn records(
    path: &Path,
    expected: usize,
    mut read: impl FnMut(Value) -> Result<(), String>,
) -> Result<(), String> {
    let mut seen = 0;
    for (index, line) in BufReader::new(File::open(path).map_err(|e| e.to_string())?)
        .lines()
        .take(expected)
        .enumerate()
    {
        let value = serde_json::from_str(&line.map_err(|e| e.to_string())?)
            .map_err(|e| format!("{} line {}: {e}", path.display(), index + 1))?;
        read(value).map_err(|e| format!("{} line {}: {e}", path.display(), index + 1))?;
        seen += 1;
    }
    if seen != expected {
        return Err(format!(
            "{} has {seen} records, expected {expected}",
            path.display()
        ));
    }
    Ok(())
}
fn count(evidence: &Value) -> Result<usize, String> {
    evidence["records"]
        .as_u64()
        .and_then(|count| usize::try_from(count).ok())
        .ok_or("missing or invalid evidence record count".into())
}
pub fn run(args: &[String]) -> Result<(), String> {
    const HELP: &str = "report_read REPORT [--stream snapshots|events] [--turn N] [--phase PHASE] [--sequence N]\nExpands raw and encoded report evidence into full JSONL. Filters apply after lossless decoding.";
    if args.is_empty() || args == ["--help"] {
        println!("{HELP}");
        return Ok(());
    }
    let path = Path::new(&args[0]);
    let options = super::config::parse_options(&args[1..], |flag| {
        ["--stream", "--turn", "--phase", "--sequence"].contains(&flag)
    })?;
    let stream = options.get("--stream").copied().unwrap_or("snapshots");
    if !["snapshots", "events"].contains(&stream) {
        return Err("--stream accepts snapshots or events".into());
    }
    if stream == "snapshots" && options.contains_key("--sequence") {
        return Err("--sequence requires --stream events".into());
    }
    for flag in ["--turn", "--sequence"] {
        if options
            .get(flag)
            .is_some_and(|value| value.parse::<u64>().is_err())
        {
            return Err(format!("{flag} requires a nonnegative integer"));
        }
    }
    let report: Value = serde_json::from_reader(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let parent = path.parent().unwrap_or(Path::new("."));
    let evidence = &report["evidence"][stream];
    let format = evidence["format"]
        .as_str()
        .ok_or("missing evidence format")?;
    let file = parent.join(evidence["path"].as_str().ok_or("missing evidence path")?);
    let mut dictionary = BTreeMap::new();
    if format == "jsonl_events_shared_v1" {
        let shared = &report["evidence"]["shared_values"];
        records(
            &parent.join(
                shared["path"]
                    .as_str()
                    .ok_or("missing shared values path")?,
            ),
            count(shared)?,
            |record| {
                let id = record["id"].as_u64().ok_or("shared value has no ID")?;
                let value = record
                    .get("value")
                    .cloned()
                    .ok_or("shared record has no value")?;
                if dictionary.insert(id, value).is_some() {
                    return Err("duplicate shared value ID".into());
                }
                Ok(())
            },
        )?;
    }
    let mut snapshots = SnapshotReader::default();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    records(&file, count(evidence)?, |record| {
        let value = match (stream, format) {
            (_, "jsonl") => record,
            ("snapshots", "jsonl_snapshot_delta_v1") => snapshots.decode(record)?,
            ("events", "jsonl_events_shared_v1") => expand_event(record, &dictionary)?,
            _ => return Err(format!("unsupported {stream} format {format}")),
        };
        for (flag, field) in [
            ("--turn", "turn"),
            ("--sequence", "sequence"),
            ("--phase", "phase"),
        ] {
            if let Some(expected) = options.get(flag) {
                let matches = if field == "phase" {
                    value[field].as_str() == Some(*expected)
                } else {
                    value[field].as_u64() == expected.parse().ok()
                };
                if !matches {
                    return Ok(());
                }
            }
        }
        serde_json::to_writer(&mut output, &value).map_err(|e| e.to_string())?;
        output.write_all(b"\n").map_err(|e| e.to_string())
    })
}
