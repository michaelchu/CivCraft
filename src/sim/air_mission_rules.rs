//! Editable stock Conquests rules for air mission interception.

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AirMissionRules {
    pub(super) chance_to_intercept_air_missions: i32,
    pub(super) chance_to_intercept_stealth_missions: i32,
    pub(super) rebase_operational_range_multiplier: i32,
}

pub(super) fn air_mission_rules() -> &'static AirMissionRules {
    static RULES: OnceLock<AirMissionRules> = OnceLock::new();
    RULES.get_or_init(|| {
        parse_air_mission_rules(include_str!(
            "../../data/rulesets/civ3_conquests/air_missions.tsv"
        ))
    })
}

fn parse_air_mission_rules(source: &str) -> AirMissionRules {
    let mut lines = source.lines();
    assert_eq!(
        lines.next(),
        Some("key\tvalue\tsource"),
        "invalid air mission rules header"
    );
    let mut normal = None;
    let mut stealth = None;
    let mut rebase_range = None;
    for (index, line) in lines.enumerate() {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(
            fields.len(),
            3,
            "invalid air mission rule row {}",
            index + 2
        );
        let value = fields[1]
            .parse::<i32>()
            .unwrap_or_else(|error| panic!("invalid air mission rule value: {error}"));
        match fields[0] {
            "chance_to_intercept_air_missions" => {
                assert!(fields[2].contains("Conquests/conquests.biq#RULE[0]."));
                assert!(
                    (0..=100).contains(&value),
                    "air interception chance outside 0..100"
                );
                assert!(
                    normal.replace(value).is_none(),
                    "duplicate air interception chance"
                );
            }
            "chance_to_intercept_stealth_missions" => {
                assert!(fields[2].contains("Conquests/conquests.biq#RULE[0]."));
                assert!(
                    (0..=100).contains(&value),
                    "stealth interception chance outside 0..100"
                );
                assert!(
                    stealth.replace(value).is_none(),
                    "duplicate stealth interception chance"
                );
            }
            "rebase_operational_range_multiplier" => {
                assert!(fields[2].contains("Civilopedia.txt#GCON_Air_Missions"));
                assert!(value > 0, "rebase range multiplier must be positive");
                assert!(
                    rebase_range.replace(value).is_none(),
                    "duplicate rebase range multiplier"
                );
            }
            unknown => panic!("unknown air mission rule {unknown:?}"),
        }
    }
    AirMissionRules {
        chance_to_intercept_air_missions: normal.expect("missing air interception chance"),
        chance_to_intercept_stealth_missions: stealth.expect("missing stealth interception chance"),
        rebase_operational_range_multiplier: rebase_range.expect("missing rebase range multiplier"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_conquests_air_interception_chances_are_imported_from_rules_table() {
        let rules = air_mission_rules();
        assert_eq!(rules.chance_to_intercept_air_missions, 50);
        assert_eq!(rules.chance_to_intercept_stealth_missions, 5);
        assert_eq!(rules.rebase_operational_range_multiplier, 6);
    }
}
