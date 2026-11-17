use assert_cmd::{Command, cargo::cargo_bin_cmd};
use std::path::{Path, PathBuf};

fn headless() -> Command {
    cargo_bin_cmd!("headless_sim")
}

fn report_directory(label: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "civcraft-headless-{label}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn read_report(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn lists_registered_scenarios() {
    let output = headless().arg("--list-scenarios").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for scenario in [
        "generated-ai",
        "city-healing",
        "settlement-ferry",
        "ai-paradrops",
    ] {
        assert!(stdout.lines().any(|line| line == scenario), "{stdout}");
    }
}

#[test]
fn rejects_conflicting_scenario_inputs() {
    let output = headless()
        .args([
            "--scenario",
            "city-healing",
            "--load-campaign",
            "ignored.save",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("mutually exclusive")
    );
}

#[test]
fn runs_a_named_scenario_and_publishes_its_checks() {
    let directory = report_directory("passing");
    let report = directory.join("report.json");
    let output = headless()
        .args([
            "--scenario",
            "settlement-ferry",
            "--turns",
            "1",
            "--report",
            report.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report_json = read_report(&report);
    assert_eq!(report_json["summary"]["scenario_checks_passed"], 3);
    assert_eq!(report_json["summary"]["scenario_checks_total"], 3);
    assert!(
        report_json["run"]["scenario"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|check| check["passed"] == true)
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn failed_scenario_publishes_its_checks_and_exits_unsuccessfully() {
    let directory = report_directory("failing");
    let report = directory.join("report.json");
    // Seed 18 reproduces a pre-existing failure in the ferry fixture. Seed 1
    // used to fail only because France could not build units (civilization
    // masks were matched against display names), so it no longer applies.
    let output = headless()
        .args([
            "--scenario",
            "settlement-ferry",
            "--seed",
            "18",
            "--turns",
            "1",
            "--report",
        ])
        .arg(&report)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("scenario checks failed"));
    let report = read_report(&report);
    assert_eq!(report["run"]["status"], "failed");
    assert_eq!(report["run"]["stop_reason"], "scenario_check");
    assert_eq!(report["summary"]["scenario_checks_passed"], 0);
    assert_eq!(report["summary"]["scenario_checks_total"], 3);
    assert_eq!(
        report["run"]["scenario"]["checks"][0]["name"],
        "ferry_queued"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn lifecycle_snapshots_survive_disabled_event_output() {
    let directory = report_directory("lifecycle");
    let report = directory.join("report.json");
    let output = headless()
        .args([
            "--scenario",
            "lifecycle-history",
            "--turns",
            "1",
            "--analytics",
            "none",
            "--trace-domains",
            "none",
            "--snapshot-providers",
            "lifecycle,units",
            "--report",
        ])
        .arg(&report)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = read_report(&report);
    assert_eq!(report["version"], 8);
    assert_eq!(report["summary"]["lifecycle"]["history_available"], true);
    assert_eq!(report["summary"]["lifecycle"]["complete_since_start"], true);
    let snapshots = directory.join(report["evidence"]["snapshots"]["path"].as_str().unwrap());
    let snapshots = std::fs::read_to_string(snapshots).unwrap();
    let snapshots: Vec<serde_json::Value> = snapshots
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(
        snapshots[0]["lifecycle"]["changes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        snapshots
            .iter()
            .any(|s| !s["lifecycle"]["changes"].as_array().unwrap().is_empty())
    );
    assert!(
        snapshots[0]["units"]
            .as_array()
            .unwrap()
            .iter()
            .all(|u| u.get("home_city").is_some())
    );
    let events = directory.join(report["evidence"]["events"]["path"].as_str().unwrap());
    assert!(std::fs::read(events).unwrap().is_empty());
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(feature = "desktop")]
#[test]
fn minimap_images_and_local_history_viewer_follow_snapshot_boundaries() {
    let directory = report_directory("minimap with spaces");
    let report = directory.join("review #1.json");
    headless()
        .args([
            "--turns",
            "2",
            "--analytics",
            "none",
            "--trace-domains",
            "none",
            "--snapshot-providers",
            "minimap",
            "--report",
        ])
        .arg(&report)
        .assert()
        .success();
    let output = read_report(&report);
    let evidence = &output["evidence"]["minimap"];
    assert_eq!(evidence["frames"], 3);
    assert_eq!(evidence["turn"], output["run"]["final_turn"]);
    let image = image::open(directory.join(evidence["path"].as_str().unwrap())).unwrap();
    let snapshots = std::fs::read_to_string(
        directory.join(output["evidence"]["snapshots"]["path"].as_str().unwrap()),
    )
    .unwrap();
    let snapshots: Vec<serde_json::Value> = snapshots
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let map = &snapshots.last().unwrap()["minimap"];
    let width = map["width"].as_u64().unwrap();
    let height = map["height"].as_u64().unwrap();
    assert_eq!(
        image.width() as u64 * height,
        image.height() as u64 * width * 2
    );
    assert_eq!(
        map["terrain"].as_array().unwrap().len() as u64,
        width * height
    );
    let viewer =
        std::fs::read_to_string(directory.join(evidence["viewer"].as_str().unwrap())).unwrap();
    assert!(viewer.contains("review%20%231.json.minimaps/000002.png"));
    assert!(!viewer.contains("__FRAMES__"));
    let frames = directory.join("review #1.json.minimaps");
    assert_eq!(std::fs::read_dir(&frames).unwrap().count(), 3);
    assert_eq!(
        std::fs::read(directory.join(evidence["path"].as_str().unwrap())).unwrap(),
        std::fs::read(frames.join("000002.png")).unwrap()
    );
    let disabled = directory.join("disabled.json");
    headless()
        .args([
            "--turns",
            "1",
            "--analytics",
            "none",
            "--trace-domains",
            "none",
            "--snapshot-providers",
            "none",
            "--report",
        ])
        .arg(&disabled)
        .assert()
        .success();
    assert!(read_report(&disabled)["evidence"]["minimap"].is_null());
    assert!(!directory.join("disabled.json.minimaps").exists());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn world_options_validate_against_the_generator_and_report_the_effective_inputs() {
    let message = headless()
        .args(["--ocean-coverage", "60"])
        .output()
        .unwrap();
    assert!(!message.status.success());
    assert!(
        String::from_utf8_lossy(&message.stderr).contains("requires --map-generator opensimplex")
    );

    let directory = report_directory("world");
    let mut streams = Vec::new();
    for (ocean, climate, age, landform, temperature) in [
        ("60", "arid", "3", "archipelago", "cool"),
        ("80", "wet", "5", "pangaea", "warm"),
    ] {
        let report = directory.join(format!("{ocean}.json"));
        let run = headless()
            .args([
                "--seed",
                "42",
                "--turns",
                "1",
                "--map-generator",
                "opensimplex",
                "--ocean-coverage",
                ocean,
                "--climate",
                climate,
                "--world-age",
                age,
                "--landform",
                landform,
                "--temperature",
                temperature,
                "--report",
            ])
            .arg(&report)
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let report = read_report(&report);
        assert_eq!(report["version"], 8);
        let config = &report["run"]["configuration"];
        assert_eq!(config["map_generator"], "opensimplex");
        assert_eq!(
            config["world_settings"]["ocean_coverage"],
            ocean.parse::<i32>().unwrap()
        );
        assert_eq!(config["world_settings"]["climate"], climate);
        assert_eq!(
            config["world_settings"]["world_age_billions"],
            age.parse::<i32>().unwrap()
        );
        assert_eq!(config["world_settings"]["landform"], landform);
        assert_eq!(config["world_settings"]["temperature"], temperature);
        let file = report["evidence"]["snapshots"]["path"].as_str().unwrap();
        streams.push(std::fs::read(directory.join(file)).unwrap());
    }
    assert_ne!(streams[0], streams[1]);
    std::fs::remove_dir_all(directory).unwrap();
}
