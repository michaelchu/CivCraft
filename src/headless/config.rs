use crate::{mapgen, sim::trace::Entity};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub(crate) struct Config {
    pub seed: u64,
    pub turns: u32,
    pub map_size: mapgen::Civ3MapSize,
    pub world: mapgen::Civ3WorldSettings,
    pub checkpoint_every: usize,
    pub report: String,
    pub analyzers: Vec<String>,
    pub growth_stall_turns: u32,
    pub settlement_every: i32,
    pub trace_domains: BTreeSet<String>,
    pub trace_entities: BTreeSet<Entity>,
    pub buffer_events: usize,
    pub max_events: usize,
    pub max_event_bytes: usize,
    pub providers: BTreeSet<String>,
    pub stop_on_finding: String,
    pub scenario: Option<String>,
    pub load_campaign: Option<PathBuf>,
    pub profile_turns: bool,
}
fn list(text: &str) -> Vec<String> {
    if text == "none" {
        vec![]
    } else {
        text.split(',').map(str::to_owned).collect()
    }
}
impl Config {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        const FLAGS: &[&str] = &[
            "--seed",
            "--turns",
            "--map-size",
            "--ocean-coverage",
            "--climate",
            "--world-age",
            "--landform",
            "--temperature",
            "--checkpoint-every",
            "--report",
            "--analytics",
            "--growth-stall-turns",
            "--settlement-check-every",
            "--trace-domains",
            "--trace-entities",
            "--trace-buffer-events",
            "--trace-max-events",
            "--trace-max-bytes",
            "--snapshot-providers",
            "--stop-on-finding",
            "--scenario",
            "--load-campaign",
            "--profile-turns",
        ];
        let mut values = BTreeMap::new();
        let mut iter = args.iter();
        while let Some(flag) = iter.next() {
            if !FLAGS.contains(&flag.as_str()) {
                return Err(format!("unknown option {flag}; use --help"));
            }
            let value = iter
                .next()
                .filter(|v| !v.starts_with("--"))
                .ok_or_else(|| format!("{flag} requires a value"))?;
            if values.insert(flag.as_str(), value.as_str()).is_some() {
                return Err(format!("duplicate option {flag}"));
            }
        }
        let value = |key, default| *values.get(key).unwrap_or(&default);
        let positive = |key, default| -> Result<usize, String> {
            value(key, default)
                .parse::<usize>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| format!("{key} must be a positive integer"))
        };
        let mut providers = BTreeSet::new();
        for name in list(value("--snapshot-providers", "cities,civilizations,units")) {
            if !crate::analytics::providers::IDS.contains(&name.as_str()) {
                return Err(format!("unknown snapshot provider {name}"));
            }
            providers.insert(name);
        }
        let mut entities = BTreeSet::new();
        let entity_text = value("--trace-entities", "all");
        if entity_text != "all" {
            for text in list(entity_text) {
                let (kind, id) = text
                    .split_once(':')
                    .ok_or("trace entities use city:N, unit:N, or civilization:N")?;
                let id = id
                    .parse::<u32>()
                    .map_err(|_| "entity ID must be a nonnegative integer")?;
                entities.insert(match kind {
                    "city" => Entity::City(id),
                    "unit" => Entity::Unit(id),
                    "civilization" => Entity::Civilization(id as usize),
                    _ => return Err(format!("unknown entity kind {kind}")),
                });
            }
            if entities.is_empty() {
                return Err("--trace-entities must be all or a nonempty entity list".into());
            }
        }
        let stop = value("--stop-on-finding", "none");
        if !["none", "any", "invariant"].contains(&stop) {
            return Err("--stop-on-finding accepts none, any, invariant".into());
        }
        let profile_turns = match value("--profile-turns", "false") {
            "true" => true,
            "false" => false,
            _ => return Err("--profile-turns accepts true or false".into()),
        };
        let turns =
            u32::try_from(positive("--turns", "30")?).map_err(|_| "turn count is too large")?;
        let growth_stall_turns = u32::try_from(positive("--growth-stall-turns", "2")?)
            .map_err(|_| "growth threshold is too large")?;
        let settlement_every = i32::try_from(positive("--settlement-check-every", "10")?)
            .map_err(|_| "settlement interval is too large")?;
        let scenario = values.get("--scenario").map(|value| (*value).to_owned());
        let load_campaign = values
            .get("--load-campaign")
            .map(|value| PathBuf::from(*value));
        if scenario.is_some() && load_campaign.is_some() {
            return Err("--scenario and --load-campaign are mutually exclusive".into());
        }
        Ok(Self {
            seed: value("--seed", "42")
                .parse()
                .map_err(|_| "--seed must be a nonnegative integer")?,
            turns,
            map_size: value("--map-size", "tiny").parse()?,
            world: mapgen::Civ3WorldSettings {
                ocean_coverage: value("--ocean-coverage", "70").parse()?,
                climate: value("--climate", "normal").parse()?,
                age: value("--world-age", "4").parse()?,
                landform: value("--landform", "continents").parse()?,
                temperature: value("--temperature", "temperate").parse()?,
            },
            checkpoint_every: positive("--checkpoint-every", "10")?,
            report: value("--report", "headless-simulation.json").into(),
            analyzers: list(value("--analytics", "growth,settlement,production")),
            growth_stall_turns,
            settlement_every,
            trace_domains: list(value("--trace-domains", "all")).into_iter().collect(),
            trace_entities: entities,
            buffer_events: positive("--trace-buffer-events", "10000")?,
            max_events: positive("--trace-max-events", "50000")?,
            max_event_bytes: positive("--trace-max-bytes", "16777216")?,
            providers,
            stop_on_finding: stop.into(),
            scenario,
            load_campaign,
            profile_turns,
        })
    }
    pub fn retains(&self, event: &crate::sim::trace::Event) -> bool {
        (self.trace_domains.contains("all") || self.trace_domains.contains(&event.source))
            && (self.trace_entities.is_empty()
                || event
                    .entities
                    .iter()
                    .any(|e| self.trace_entities.contains(e)))
    }
    pub fn metadata(&self) -> serde_json::Value {
        json!({"seed":self.seed,"map_size":self.map_size.to_string(),"requested_turns":self.turns,
            "scenario":self.scenario,"load_campaign":self.load_campaign,
            "world_settings":{"ocean_coverage":self.world.ocean_coverage.percent(),
                "climate":format!("{:?}",self.world.climate).to_ascii_lowercase(),
                "world_age_billions":match self.world.age {mapgen::WorldAge::Billion3=>3,mapgen::WorldAge::Billion4=>4,mapgen::WorldAge::Billion5=>5},
                "landform":format!("{:?}",self.world.landform).to_ascii_lowercase(),
                "temperature":format!("{:?}",self.world.temperature).to_ascii_lowercase()},
            "checkpoint_every":self.checkpoint_every,"trace_domains":self.trace_domains,"trace_entities":self.trace_entities,
            "trace_buffer_events":self.buffer_events,"trace_max_events":self.max_events,"trace_max_bytes":self.max_event_bytes,
            "snapshot_providers":self.providers,"stop_on_finding":self.stop_on_finding,
            "profile_turns":self.profile_turns})
    }
}
pub(crate) const HELP: &str = "Headless AI simulator\n\
--seed 42 --turns 30 --map-size tiny --report report.json\n\
--analytics growth,settlement,production (or none); --list-analyzers\n\
--growth-stall-turns 2 --settlement-check-every 10\n\
--trace-domains all|none|cities,production,settlement\n\
--trace-entities all|city:1,unit:7,civilization:0\n\
--trace-buffer-events 10000 --trace-max-events 50000 --trace-max-bytes 16777216\n\
--snapshot-providers cities,civilizations,units (or none; analyzer requirements are added)\n\
--stop-on-finding none|any|invariant --checkpoint-every 10\n\
--profile-turns true|false (records per-turn phase timings)\n\
--scenario NAME | --load-campaign PATH; --list-scenarios\n\
World setup: --ocean-coverage 60|70|80 --climate arid|normal|wet --world-age 3|4|5\n\
--landform archipelago|continents|pangaea --temperature cool|temperate|warm";

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }
    #[test]
    fn rejects_ambiguous_or_invalid_configuration() {
        for values in [
            vec!["--turns", "0"],
            vec!["--trace-entities", "unit:bad"],
            vec!["--turns", "1", "--turns", "2"],
            vec!["--missing", "x"],
            vec!["--snapshot-providers", "wrong"],
            vec!["--stop-on-finding", "oops"],
            vec!["--profile-turns", "sometimes"],
            vec![
                "--scenario",
                "generated-ai",
                "--load-campaign",
                "campaign.save",
            ],
        ] {
            assert!(Config::parse(&args(&values)).is_err(), "{values:?}");
        }
    }
    #[test]
    fn filters_match_any_participant_and_do_not_select_other_domains() {
        let config = Config::parse(&args(&[
            "--trace-domains",
            "combat",
            "--trace-entities",
            "unit:7",
        ]))
        .unwrap();
        let mut event = crate::sim::trace::Event {
            sequence: 1,
            turn: 1,
            phase: crate::sim::trace::Phase::Initial,
            source: "combat".into(),
            kind: "combat.resolved".into(),
            schema_version: 1,
            entities: vec![Entity::Unit(1), Entity::Unit(7)],
            cause: None,
            payload: json!({}),
        };
        assert!(config.retains(&event));
        event.entities = vec![Entity::Unit(1)];
        assert!(!config.retains(&event));
        event.entities = vec![Entity::Unit(7)];
        event.source = "cities".into();
        assert!(!config.retains(&event));
    }
}
