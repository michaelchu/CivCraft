use assert_cmd::{Command, cargo::cargo_bin_cmd};

fn headless() -> Command {
    cargo_bin_cmd!("headless_sim")
}

#[test]
fn lists_registered_scenarios() {
    let output = headless().arg("--list-scenarios").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for scenario in ["generated-ai", "city-healing", "settlement-ferry"] {
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
    let directory = std::env::temp_dir().join(format!(
        "civcraft-headless-cli-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
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
    let report_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    assert_eq!(report_json["summary"]["scenario_checks_passed"], 3);
    assert_eq!(report_json["summary"]["scenario_checks_total"], 3);
    assert!(
        report_json["run"]["scenario"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|check| check["passed"] == true)
    );
    for entry in std::fs::read_dir(&directory).unwrap() {
        std::fs::remove_file(entry.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
}
