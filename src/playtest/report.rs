//! Structured results and JSON output for automated playtests.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Passed,
    Failed,
    Skipped,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

struct ScenarioResult {
    step: &'static str,
    status: Status,
    detail: String,
}

/// Collects named scenario outcomes and emits a deterministic self-test report.
pub(crate) struct PlaytestReport {
    seed: u64,
    steps: Vec<ScenarioResult>,
}

impl PlaytestReport {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            seed,
            steps: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, (step, passed, detail): (&'static str, bool, String)) {
        self.steps.push(ScenarioResult {
            step,
            status: if passed {
                Status::Passed
            } else {
                Status::Failed
            },
            detail,
        });
    }

    pub(crate) fn extend<I>(&mut self, results: I)
    where
        I: IntoIterator<Item = (&'static str, bool, String)>,
    {
        for result in results {
            self.push(result);
        }
    }

    pub(crate) fn skip(&mut self, step: &'static str, detail: String) {
        self.steps.push(ScenarioResult {
            step,
            status: Status::Skipped,
            detail,
        });
    }

    pub(crate) fn all_passed(&self) -> bool {
        !self.steps.is_empty()
            && self
                .steps
                .iter()
                .all(|result| result.status == Status::Passed)
    }

    pub(crate) fn to_json(&self) -> String {
        let steps = self
            .steps
            .iter()
            .map(|result| {
                let passed = result.status == Status::Passed;
                format!(
                    "{{\"step\":\"{}\",\"passed\":{passed},\"status\":\"{}\",\"detail\":\"{}\"}}",
                    json_escape(result.step),
                    result.status.as_str(),
                    json_escape(&result.detail),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"version\":2,\"seed\":{},\"steps\":[{steps}]}}",
            self.seed
        )
    }
}

fn json_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if control <= '\u{1f}' => {
                use std::fmt::Write as _;
                write!(escaped, "\\u{:04x}", control as u32).unwrap();
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_seed_and_json_safe_details() {
        let mut report = PlaytestReport::new(42);
        report.push((
            "movement",
            true,
            "line one\nline two\tquoted \"text\"".into(),
        ));
        let json = report.to_json();
        assert!(json.starts_with("{\"version\":2,\"seed\":42,"));
        assert!(json.contains("line one\\nline two\\tquoted \\\"text\\\""));
    }

    #[test]
    fn skipped_prerequisites_do_not_report_as_passes() {
        let mut report = PlaytestReport::new(42);
        report.push(("foundation", false, "failed".into()));
        report.skip("economy", "Skipped because foundation failed".into());
        assert!(!report.all_passed());
        assert!(report.to_json().contains("\"status\":\"skipped\""));
    }
}
