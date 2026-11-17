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
    // Seed 1 reproduces a pre-existing failure in the ferry fixture.
    let output = headless()
        .args([
            "--scenario",
            "settlement-ferry",
            "--seed",
            "1",
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
        assert_eq!(report["version"], 7);
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
