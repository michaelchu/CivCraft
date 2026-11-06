//! Evidence for native visual QA. Drawing timings measure CPU submission only.
use std::{fs, io, path::Path, time::Instant};

#[derive(Default)]
struct SceneTiming {
    name: &'static str,
    setup_ms: f64,
    actions_ms: f64,
    draws: Vec<f64>,
    captures: Vec<(String, f64, u64)>,
    base_pixels: Option<u64>,
    checks: Vec<(&'static str, bool)>,
    cache: (u64, u64, usize),
}
pub(crate) struct UiReviewProfile {
    started: Instant,
    seed: u64,
    baseline_setup_ms: f64,
    scenes: Vec<SceneTiming>,
}
impl UiReviewProfile {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            started: Instant::now(),
            seed,
            baseline_setup_ms: 0.0,
            scenes: Vec::new(),
        }
    }
    pub(crate) fn prepare_output(
        dir: &str,
        names: impl IntoIterator<Item = &'static str>,
    ) -> io::Result<()> {
        fs::create_dir_all(dir)?;
        // Remove only files managed by this run, so skipped pages cannot leave
        // stale screenshots behind when a review directory is reused.
        for file in std::iter::once("report.json".to_owned()).chain(
            names
                .into_iter()
                .flat_map(|name| [format!("{name}.png"), format!("{name}-next.png")]),
        ) {
            match fs::remove_file(Path::new(dir).join(file)) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
    pub(crate) fn baseline_setup(&mut self, ms: f64) {
        self.baseline_setup_ms = ms;
    }
    pub(crate) fn setup(&mut self, scene: usize, name: &'static str, ms: f64) {
        self.scenes.resize_with(scene + 1, SceneTiming::default);
        self.scenes[scene].name = name;
        self.scenes[scene].setup_ms = ms;
    }
    pub(crate) fn draw(&mut self, scene: usize, ms: f64, cache: (u64, u64, usize)) {
        self.scenes[scene].draws.push(ms);
        self.scenes[scene].cache = cache;
    }
    pub(crate) fn actions(&mut self, scene: usize, ms: f64) {
        self.scenes[scene].actions_ms += ms;
    }
    pub(crate) fn check(&mut self, scene: usize, name: &'static str, passed: bool) {
        let checks = &mut self.scenes[scene].checks;
        if let Some((_, old)) = checks.iter_mut().find(|(key, _)| *key == name) {
            *old &= passed;
        } else {
            checks.push((name, passed));
        }
    }
    pub(crate) fn capture(
        &mut self,
        scene: usize,
        name: String,
        ms: f64,
        pixels: u64,
        next: bool,
    ) -> bool {
        let distinct = if next {
            self.scenes[scene]
                .base_pixels
                .is_some_and(|base| base != pixels)
        } else {
            true
        };
        if next {
            self.check(scene, "next_capture_has_changed_pixels", distinct);
        } else {
            self.scenes[scene].base_pixels = Some(pixels);
        }
        self.scenes[scene].captures.push((name, ms, pixels));
        distinct
    }
    pub(crate) fn finish(&self, dir: &str) -> io::Result<bool> {
        let passed = self
            .scenes
            .iter()
            .all(|s| s.checks.iter().all(|(_, ok)| *ok));
        let scenes=self.scenes.iter().filter(|s|!s.name.is_empty()).map(|s| {
            let mut samples=s.draws.clone();samples.sort_by(f64::total_cmp);
            let mean=samples.iter().sum::<f64>()/samples.len().max(1) as f64;
            let p95=samples.get((samples.len()*95/100).min(samples.len().saturating_sub(1))).copied().unwrap_or(0.0);
            let captures=s.captures.iter().map(|(file,ms,pixels)|format!("{{\"file\":{},\"ms\":{ms:.4},\"pixel_fingerprint\":\"{pixels:016x}\"}}",json_string(file))).collect::<Vec<_>>().join(",");
            let checks=s.checks.iter().map(|(label,ok)|format!("{{\"check\":{},\"passed\":{ok}}}",json_string(label))).collect::<Vec<_>>().join(",");
            let kind=if s.name=="world" || s.name.starts_with("campaign-") {"campaign"}else{"fixture"};
            format!("{{\"scene\":{},\"kind\":\"{kind}\",\"setup_ms\":{:.4},\"actions_ms\":{:.4},\"draw_mean_cpu_ms\":{mean:.4},\"draw_p95_cpu_ms\":{p95:.4},\"frames\":{},\"border_cache_hits\":{},\"border_cache_misses\":{},\"border_cache_entries\":{},\"checks\":[{checks}],\"captures\":[{captures}]}}",json_string(s.name),s.setup_ms,s.actions_ms,samples.len(),s.cache.0,s.cache.1,s.cache.2)
        }).collect::<Vec<_>>().join(",");
        fs::write(
            Path::new(dir).join("report.json"),
            format!(
                "{{\"version\":2,\"passed\":{passed},\"seed\":{},\"baseline_setup_ms\":{:.4},\"elapsed_ms\":{:.4},\"scenes\":[{scenes}]}}",
                self.seed,
                self.baseline_setup_ms,
                self.started.elapsed().as_secs_f64() * 1000.0
            ),
        )?;
        Ok(passed)
    }
}
fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => {
                use std::fmt::Write;
                write!(out, "\\u{:04x}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_reuse_removes_only_managed_images() {
        let dir = std::env::temp_dir().join(format!("civcraft-output-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["world.png", "world-next.png", "report.json", "notes.txt"] {
            std::fs::write(dir.join(name), b"previous run").unwrap();
        }
        UiReviewProfile::prepare_output(dir.to_str().unwrap(), ["world"]).unwrap();
        assert!(!dir.join("world-next.png").exists());
        assert!(!dir.join("world.png").exists());
        assert!(dir.join("notes.txt").exists());
        std::fs::remove_file(dir.join("notes.txt")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn unchanged_page_pixels_fail_the_report() {
        let mut p = UiReviewProfile::new(42);
        p.setup(0, "world", 0.0);
        assert!(p.capture(0, "world.png".into(), 0.0, 123, false));
        assert!(!p.capture(0, "world-next.png".into(), 0.0, 123, true));
        assert!(!p.scenes[0].checks[0].1);
    }
    #[test]
    fn a_later_success_cannot_hide_an_earlier_failed_check() {
        let mut p = UiReviewProfile::new(42);
        p.setup(0, "world", 0.0);
        p.check(0, "bounds", false);
        p.check(0, "bounds", true);
        assert_eq!(p.scenes[0].checks, vec![("bounds", false)]);
    }
}
