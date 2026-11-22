//! CivCraft's campaign engine settings, owned directly by the code.
//!
//! These values used to be parsed at startup from the embedded `game.ruleset`
//! and `units.ruleset` text files. They are fixed for the campaign and never
//! edited at runtime, so they are compile-time constants here. The comments
//! name the pinned Freeciv `civ2civ3` ruleset section or server setting each
//! value comes from, and the Freeciv engine default where the ruleset leaves a
//! setting unspecified.

use super::{
    CalendarSettings, CaravanBonusStyle, CivilWarRulesetSettings, CombatRulesetSettings,
    FreeTechnologyStyle, GoodsSelection, ResearchCostStyle, ResearchRulesetSettings,
    RevolutionType, TechnologyLeakageStyle, TechnologyUpkeepStyle, TradeGoodRulesetRecord,
    TradeRevenueStyle, TradeRouteBonus, TradeRouteCancellation, TradeRouteRulesetSettings,
    TradeRouteType, TradeRouteTypeSettings, VeterancySettings, VictoryCondition,
};
use std::sync::OnceLock;

/// `disasters` server setting: the per-mille base chance multiplied by each
/// disaster's own frequency.
pub(crate) const DISASTER_FREQUENCY: i32 = 100;

/// `game.ruleset [civstyle] init_city_radius_sq`: the radius of a new city's
/// work area, as a squared distance.
pub(crate) const INITIAL_CITY_RADIUS_SQ: i64 = 5;

/// `game.ruleset [culture] migration_pml`: the per-mille migration weight
/// given to a city's culture.
pub(crate) const MIGRATION_PML: i32 = 50;

impl VictoryCondition {
    /// The `victories` server setting is `SPACERACE|ALLIED|CULTURE`.
    pub(crate) const fn enabled_in_campaign(self) -> bool {
        match self {
            Self::SpaceRace | Self::Allied | Self::Culture => true,
            Self::WorldPeace => false,
        }
    }
}

impl CalendarSettings {
    /// `game.ruleset [calendar]`.
    pub(crate) fn campaign() -> &'static Self {
        static SETTINGS: OnceLock<CalendarSettings> = OnceLock::new();
        SETTINGS.get_or_init(|| Self {
            start_year: -4000,
            skip_year_0: true,
            fragments_per_year: 0,
            positive_label: "CE".to_owned(),
            negative_label: "BCE".to_owned(),
        })
    }
}

impl ResearchRulesetSettings {
    /// `game.ruleset [research]`, the `[civstyle]` tech-hole switches and the
    /// research-related server settings. Settings the ruleset leaves at
    /// Freeciv engine defaults (`sciencebox`, `techpenalty`, `techleak`,
    /// `techlost_*`, `diplbulbcost`, `revolen`) are recorded explicitly.
    pub(crate) const CAMPAIGN: Self = Self {
        cost_style: ResearchCostStyle::Linear,
        base_tech_cost: 10,
        min_tech_cost: 10,
        leakage_style: TechnologyLeakageStyle::Embassies,
        upkeep_style: TechnologyUpkeepStyle::Cities,
        upkeep_divider: 6000,
        free_technology_style: FreeTechnologyStyle::Cheapest,
        tech_steal_allow_holes: false,
        tech_trade_allow_holes: false,
        tech_trade_loss_allow_holes: false,
        tech_parasite_allow_holes: false,
        tech_loss_allow_holes: false,
        sciencebox_percent: 100,
        tech_penalty_percent: 100,
        tech_leak_percent: 100,
        tech_loss_forgiveness_percent: 0,
        tech_loss_restore_percent: 50,
        tech_lost_recv_percent: 0,
        tech_lost_donor_percent: 0,
        diplomatic_bulb_cost_percent: 0,
        revolution_type: RevolutionType::RandomQuick,
        revolution_length: 5,
    };
}

impl CombatRulesetSettings {
    /// `game.ruleset [combat_rules]` and the combat server settings.
    /// `killstack`, `killcitizen`, `unreachableprotects`, `savepalace` and
    /// `homecaughtunits` are the pinned Freeciv engine defaults.
    pub(crate) const CAMPAIGN: Self = Self {
        tired_attack: true,
        only_killing_makes_veteran: false,
        only_real_fight_makes_veteran: true,
        combat_odds_scaled_veterancy: true,
        damage_reduces_bombard_rate: true,
        low_firepower_badwallattacker: 1,
        low_firepower_pearl_harbor: 1,
        low_firepower_combat_bonus: 1,
        low_firepower_nonnat_bombard: 1,
        nuke_pop_loss_pct: 49,
        nuke_defender_survival_chance_pct: 0,
        killstack: true,
        killcitizen: true,
        unreachable_protects: true,
        occupychance: 100,
        razechance: 10,
        savepalace: true,
        homecaughtunits: true,
    };
}

impl CivilWarRulesetSettings {
    /// `civilwarsize` and `maxplayers` server settings.
    pub(crate) const CAMPAIGN: Self = Self {
        city_threshold: 10,
        max_players: 500,
    };
}

impl VeterancySettings {
    /// `game.ruleset [civstyle] upgrade_veteran_loss` and
    /// `autoupgrade_veteran_loss`.
    pub(crate) const CAMPAIGN: Self = Self {
        upgrade_veteran_loss: 1,
        autoupgrade_veteran_loss: 1,
    };
}

impl TradeRouteRulesetSettings {
    /// `game.ruleset [trade]` route categories and the single `Goods`
    /// record, with Freeciv's engine defaults for everything the ruleset
    /// leaves unset (`trademindist`, `trade_revenue_style`,
    /// `caravan_bonus_style`). The ruleset sets `tradeworldrelpct` to 100.
    pub(crate) fn campaign() -> &'static Self {
        static SETTINGS: OnceLock<TradeRouteRulesetSettings> = OnceLock::new();
        SETTINGS.get_or_init(|| {
            let route = |route_type, trade_percent, bonus| TradeRouteTypeSettings {
                route_type,
                trade_percent,
                cancellation: TradeRouteCancellation::Cancel,
                bonus,
            };
            Self {
                route_types: vec![
                    route(TradeRouteType::National, 0, TradeRouteBonus::None),
                    route(
                        TradeRouteType::NationalIntercontinental,
                        0,
                        TradeRouteBonus::None,
                    ),
                    route(TradeRouteType::International, 100, TradeRouteBonus::Gold),
                    route(
                        TradeRouteType::InternationalIntercontinental,
                        200,
                        TradeRouteBonus::Gold,
                    ),
                    route(TradeRouteType::Ally, 100, TradeRouteBonus::Gold),
                    route(
                        TradeRouteType::AllyIntercontinental,
                        200,
                        TradeRouteBonus::Gold,
                    ),
                    route(TradeRouteType::Enemy, 0, TradeRouteBonus::Gold),
                    route(
                        TradeRouteType::EnemyIntercontinental,
                        0,
                        TradeRouteBonus::Gold,
                    ),
                    route(TradeRouteType::Team, 100, TradeRouteBonus::Gold),
                    route(
                        TradeRouteType::TeamIntercontinental,
                        200,
                        TradeRouteBonus::Gold,
                    ),
                ],
                goods: vec![TradeGoodRulesetRecord {
                    id: "Goods".to_owned(),
                    name: "Goods".to_owned(),
                    requirements: Vec::new(),
                    from_percent: 100,
                    to_percent: 100,
                    one_time_percent: 100,
                    select_priority: 1,
                    replace_priority: 1,
                    flags: Vec::new(),
                }],
                minimum_trade_route_value: 0,
                reveal_trade_partner: false,
                goods_selection: GoodsSelection::Leaving,
                minimum_city_distance: 9,
                revenue_style: TradeRevenueStyle::Classic,
                world_relative_percent: 100,
                caravan_bonus_style: CaravanBonusStyle::Classic,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_settings_match_the_pinned_ruleset() {
        let settings = ResearchRulesetSettings::CAMPAIGN;
        assert_eq!(settings.cost_style, ResearchCostStyle::Linear);
        assert_eq!((settings.base_tech_cost, settings.min_tech_cost), (10, 10));
        assert_eq!(settings.upkeep_divider, 6000);
        assert_eq!(settings.tech_loss_restore_percent, 50);
        assert_eq!(settings.revolution_type, RevolutionType::RandomQuick);
        assert_eq!(settings.revolution_length, 5);
        assert!(!settings.tech_trade_allow_holes);
    }

    #[test]
    fn calendar_runs_from_4000_bce_without_a_year_zero() {
        let calendar = CalendarSettings::campaign();
        assert_eq!(calendar.start_year, -4000);
        assert!(calendar.skip_year_0);
        assert_eq!(calendar.fragments_per_year, 0);
        assert_eq!(
            (
                calendar.positive_label.as_str(),
                calendar.negative_label.as_str()
            ),
            ("CE", "BCE")
        );
    }

    #[test]
    fn combat_and_civil_war_limits_match_the_pinned_defaults() {
        let combat = CombatRulesetSettings::CAMPAIGN;
        assert!(combat.killstack && combat.killcitizen && combat.unreachable_protects);
        assert_eq!((combat.occupychance, combat.razechance), (100, 10));
        assert_eq!(combat.nuke_pop_loss_pct, 49);
        assert_eq!(
            CivilWarRulesetSettings::CAMPAIGN,
            CivilWarRulesetSettings {
                city_threshold: 10,
                max_players: 500,
            }
        );
    }

    #[test]
    fn trade_routes_pay_only_between_distinct_nations() {
        let settings = TradeRouteRulesetSettings::campaign();
        assert_eq!(settings.route_types.len(), 10);
        let percent = |route_type| settings.route_type(route_type).unwrap().trade_percent;
        assert_eq!(percent(TradeRouteType::National), 0);
        assert_eq!(percent(TradeRouteType::International), 100);
        assert_eq!(percent(TradeRouteType::InternationalIntercontinental), 200);
        assert_eq!(settings.minimum_city_distance, 9);
        assert_eq!(settings.goods.len(), 1);
        assert!(settings.good("goods").is_some());
    }

    #[test]
    fn victories_are_space_race_allied_and_culture() {
        assert!(VictoryCondition::SpaceRace.enabled_in_campaign());
        assert!(VictoryCondition::Allied.enabled_in_campaign());
        assert!(VictoryCondition::Culture.enabled_in_campaign());
        assert!(!VictoryCondition::WorldPeace.enabled_in_campaign());
    }
}
