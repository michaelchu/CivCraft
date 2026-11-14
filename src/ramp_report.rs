//! Summarize opt-in headless turn profiles for repeatable ramp comparisons.

use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, env, fs, process::ExitCode};

fn numbers(values: &[&Value], field: &str) -> Result<Vec<f64>, String> {
    values
        .iter()
        .map(|profile| {
            profile[field]
                .as_f64()
                .ok_or_else(|| format!("turn profile is missing numeric {field}"))
        })
        .collect()
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}

fn window(profiles: &[&Value]) -> Result<Value, String> {
    let elapsed = numbers(profiles, "elapsed_seconds")?;
    let cities = numbers(profiles, "city_count")?;
    let units = numbers(profiles, "unit_count")?;
    Ok(json!({
        "turn_start": profiles.first().and_then(|p| p["turn"].as_i64()),
        "turn_end": profiles.last().and_then(|p| p["turn"].as_i64()),
        "mean_seconds": mean(&elapsed),
        "median_seconds": median(&elapsed),
        "mean_city_count": mean(&cities),
        "mean_unit_count": mean(&units),
    }))
}

fn named_means(profiles: &[&Value], field: &str, name_field: &str) -> BTreeMap<String, f64> {
    let mut totals = BTreeMap::<String, f64>::new();
    for profile in profiles {
        for item in profile[field].as_array().into_iter().flatten() {
            let Some(name) = item[name_field].as_str() else {
                continue;
            };
            if let Some(seconds) = item["elapsed_seconds"].as_f64() {
                *totals.entry(name.to_owned()).or_default() += seconds;
            }
        }
    }
    for total in totals.values_mut() {
        *total /= profiles.len() as f64;
    }
    totals
}

fn operation_means(profiles: &[&Value]) -> BTreeMap<String, f64> {
    let mut totals = BTreeMap::<String, f64>::new();
    for profile in profiles {
        for (name, count) in profile["operations"].as_object().into_iter().flatten() {
            if let Some(count) = count.as_u64() {
                *totals.entry(name.clone()).or_default() += count as f64;
            }
        }
    }
    for total in totals.values_mut() {
        *total /= profiles.len() as f64;
    }
    totals
}

fn summarize(report: &Value, source: &str) -> Result<Value, String> {
    let raw = report["turn_profiles"]
        .as_array()
        .ok_or("report is missing turn_profiles")?;
    if raw.len() < 20 {
        return Err(format!(
            "report needs at least 20 turn profiles; found {}",
            raw.len()
        ));
    }
    let profiles: Vec<_> = raw.iter().collect();
    let early = &profiles[..10];
    let late = &profiles[profiles.len() - 10..];
    let early_window = window(early)?;
    let late_window = window(late)?;
    let early_mean = early_window["mean_seconds"].as_f64().unwrap();
    let late_mean = late_window["mean_seconds"].as_f64().unwrap();
    let (worst_index, worst) = profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            profile["elapsed_seconds"]
                .as_f64()
                .map(|seconds| (index, seconds))
                .ok_or("turn profile is missing numeric elapsed_seconds")
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    let mut detail = Map::new();
    detail.insert(
        "early_phase_mean_seconds".into(),
        json!(named_means(early, "phases", "phase")),
    );
    detail.insert(
        "late_phase_mean_seconds".into(),
        json!(named_means(late, "phases", "phase")),
    );
    detail.insert(
        "early_step_mean_seconds".into(),
        json!(named_means(early, "steps", "step")),
    );
    detail.insert(
        "late_step_mean_seconds".into(),
        json!(named_means(late, "steps", "step")),
    );
    detail.insert(
        "early_operation_means".into(),
        json!(operation_means(early)),
    );
    detail.insert("late_operation_means".into(), json!(operation_means(late)));
    Ok(json!({
        "source": source,
        "git_commit": report["run"]["git_commit"],
        "working_tree_dirty": report["run"]["working_tree_dirty"],
        "turn_profiles": profiles.len(),
        "early": early_window,
        "late": late_window,
        "late_early_mean_ratio": late_mean / early_mean,
        "worst_turn": profiles[worst_index]["turn"],
        "worst_turn_seconds": worst,
        "detail": detail,
    }))
}

fn run(args: impl Iterator<Item = String>) -> Result<Value, String> {
    let paths: Vec<_> = args.collect();
    if paths.is_empty() {
        return Err("usage: ramp_report REPORT.json [REPORT.json ...]".into());
    }
    let summaries = paths
        .iter()
        .map(|path| {
            let text = fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
            let report: Value =
                serde_json::from_str(&text).map_err(|error| format!("{path}: {error}"))?;
            summarize(&report, path)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(if summaries.len() == 1 {
        summaries.into_iter().next().unwrap()
    } else {
        json!(summaries)
    })
}

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(summary) => {
            println!("{}", serde_json::to_string_pretty(&summary).unwrap());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ramp_report: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_first_and_last_ten_profiles() {
        let profiles: Vec<_> = (1..=20)
            .map(|turn| {
                json!({
                    "turn": turn,
                    "city_count": turn,
                    "unit_count": turn * 2,
                    "elapsed_seconds": turn as f64,
                    "phases": [{"phase":"ai_planning","elapsed_seconds":turn as f64 / 2.0}],
                    "steps": [{"step":"workers","elapsed_seconds":turn as f64 / 4.0}],
                    "operations": {"routes":turn},
                })
            })
            .collect();
        let summary = summarize(
            &json!({"run":{"git_commit":"abc","working_tree_dirty":false},"turn_profiles":profiles}),
            "report.json",
        )
        .unwrap();
        assert_eq!(summary["early"]["mean_seconds"], 5.5);
        assert_eq!(summary["late"]["median_seconds"], 15.5);
        assert_eq!(summary["late_early_mean_ratio"], 15.5 / 5.5);
        assert_eq!(summary["worst_turn"], 20);
        assert_eq!(summary["detail"]["late_operation_means"]["routes"], 15.5);
    }

    #[test]
    fn rejects_reports_without_two_full_windows() {
        let error = summarize(&json!({"turn_profiles":[]}), "short.json").unwrap_err();
        assert!(error.contains("at least 20"));
    }
}
