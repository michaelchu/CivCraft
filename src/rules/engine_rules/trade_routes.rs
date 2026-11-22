//! Typed adapter for Freeciv trade-route ruleset data and engine defaults.
//!
//! The selected campaign ruleset supplies route categories and Goods records.
//! Values omitted by that ruleset use the pinned Freeciv engine defaults; this
//! module only describes rules and does not choose a CivCraft route actor.

use super::Requirement;

// Defaults below were checked against Freeciv revision
// 9ff184b6eb128ca636c7ad6269e27aa959600006.
// Pinned source: freeciv/common/game.h, GAME_DEFAULT_TRADEMINDIST.
// Pinned source: freeciv/common/game.h, GAME_DEFAULT_TRADE_REVENUE_STYLE.
// Pinned source: freeciv/common/game.h, GAME_DEFAULT_TRADEWORLDRELPCT.
// Pinned source: freeciv/common/game.h, GAME_DEFAULT_CARAVAN_BONUS_STYLE.
// Pinned source: freeciv/server/ruleset/ruleload.c:7597-7608.
// Pinned source: freeciv/server/ruleset/ruleload.h:100.
// Pinned source: freeciv/server/ruleset/ruleload.c:7635-7644.
/// Freeciv's route categories, in the enum order used by `common/traderoutes.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TradeRouteType {
    National,
    NationalIntercontinental,
    International,
    InternationalIntercontinental,
    Ally,
    AllyIntercontinental,
    Enemy,
    EnemyIntercontinental,
    Team,
    TeamIntercontinental,
}

impl TradeRouteType {}

/// Behavior when a previously established route becomes illegal.
#[allow(
    dead_code,
    reason = "complete Freeciv option set; the campaign selects a subset"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TradeRouteCancellation {
    Active,
    Inactive,
    Cancel,
}

/// One-time bonus granted when a route is established.
#[allow(
    dead_code,
    reason = "complete Freeciv option set; the campaign selects a subset"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TradeRouteBonus {
    None,
    Gold,
    Science,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TradeRouteTypeSettings {
    pub(crate) route_type: TradeRouteType,
    pub(crate) trade_percent: i32,
    pub(crate) cancellation: TradeRouteCancellation,
    pub(crate) bonus: TradeRouteBonus,
}

/// Revenue calculation selected by the Freeciv server setting.
#[allow(
    dead_code,
    reason = "complete Freeciv option set; the campaign selects a subset"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TradeRevenueStyle {
    Classic,
    Simple,
}

/// One-time caravan bonus calculation selected by the Freeciv server setting.
#[allow(
    dead_code,
    reason = "complete Freeciv option set; the campaign selects a subset"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CaravanBonusStyle {
    Classic,
    Logarithmic,
}

/// When Freeciv selects the Goods carried by a route actor.
#[allow(
    dead_code,
    reason = "complete Freeciv option set; the campaign selects a subset"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GoodsSelection {
    Leaving,
    Arrival,
}

/// Flags supported by pinned Freeciv `common/traderoutes.h` Goods records.
#[allow(
    dead_code,
    reason = "complete Freeciv option set; the campaign selects a subset"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TradeGoodFlag {
    Bidirectional,
    Depletes,
    SelfProvided,
}

/// One selected-ruleset `goods_*` record, including Freeciv's field defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TradeGoodRulesetRecord {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) requirements: Vec<Requirement>,
    pub(crate) from_percent: i32,
    pub(crate) to_percent: i32,
    pub(crate) one_time_percent: i32,
    pub(crate) select_priority: i32,
    pub(crate) replace_priority: i32,
    pub(crate) flags: Vec<TradeGoodFlag>,
}

/// Selected C2C3 route categories, Goods, and relevant Freeciv engine defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TradeRouteRulesetSettings {
    pub(crate) route_types: Vec<TradeRouteTypeSettings>,
    pub(crate) goods: Vec<TradeGoodRulesetRecord>,
    pub(crate) minimum_trade_route_value: i32,
    pub(crate) reveal_trade_partner: bool,
    pub(crate) goods_selection: GoodsSelection,
    pub(crate) minimum_city_distance: i32,
    pub(crate) revenue_style: TradeRevenueStyle,
    pub(crate) world_relative_percent: i32,
    pub(crate) caravan_bonus_style: CaravanBonusStyle,
}

impl TradeRouteRulesetSettings {
    pub(crate) fn route_type(&self, route_type: TradeRouteType) -> Option<&TradeRouteTypeSettings> {
        self.route_types
            .iter()
            .find(|settings| settings.route_type == route_type)
    }

    pub(crate) fn good(&self, id: &str) -> Option<&TradeGoodRulesetRecord> {
        self.goods
            .iter()
            .find(|good| good.id.eq_ignore_ascii_case(id))
    }
}

impl GoodsSelection {}

impl TradeRevenueStyle {}

impl CaravanBonusStyle {}
