//! Typed adapter for Freeciv trade-route ruleset data and engine defaults.
//!
//! The selected campaign ruleset supplies route categories and Goods records.
//! Values omitted by that ruleset use the pinned Freeciv engine defaults; this
//! module only describes rules and does not choose a CivCraft route actor.

use super::{
    LoadError, Requirement, RuleRecord, RulesetData, SettingRecord, Value, optional_parameter,
};

// Defaults below were checked against Freeciv revision
// 9ff184b6eb128ca636c7ad6269e27aa959600006.
// Pinned source: freeciv/common/game.h, GAME_DEFAULT_TRADEMINDIST.
const FREECIV_DEFAULT_TRADE_MIN_DISTANCE: i32 = 9;
// Pinned source: freeciv/common/game.h, GAME_DEFAULT_TRADE_REVENUE_STYLE.
const FREECIV_DEFAULT_TRADE_REVENUE_STYLE: TradeRevenueStyle = TradeRevenueStyle::Classic;
// Pinned source: freeciv/common/game.h, GAME_DEFAULT_TRADEWORLDRELPCT.
const FREECIV_DEFAULT_TRADE_WORLD_RELATIVE_PERCENT: i32 = 50;
// Pinned source: freeciv/common/game.h, GAME_DEFAULT_CARAVAN_BONUS_STYLE.
const FREECIV_DEFAULT_CARAVAN_BONUS_STYLE: CaravanBonusStyle = CaravanBonusStyle::Classic;

// Pinned source: freeciv/server/ruleset/ruleload.c:7597-7608.
const FREECIV_DEFAULT_MIN_TRADE_ROUTE_VALUE: i32 = 0;
const FREECIV_DEFAULT_REVEAL_TRADE_PARTNER: bool = false;
// Pinned source: freeciv/server/ruleset/ruleload.h:100.
const FREECIV_DEFAULT_GOODS_SELECTION: GoodsSelection = GoodsSelection::Leaving;
// Pinned source: freeciv/server/ruleset/ruleload.c:7635-7644.
const FREECIV_DEFAULT_GOODS_PERCENT: i32 = 100;
const FREECIV_DEFAULT_GOODS_PRIORITY: i32 = 1;

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

impl TradeRouteType {
    const ALL: [Self; 10] = [
        Self::National,
        Self::NationalIntercontinental,
        Self::International,
        Self::InternationalIntercontinental,
        Self::Ally,
        Self::AllyIntercontinental,
        Self::Enemy,
        Self::EnemyIntercontinental,
        Self::Team,
        Self::TeamIntercontinental,
    ];

    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "national" => Some(Self::National),
            "nationalic" => Some(Self::NationalIntercontinental),
            "in" => Some(Self::International),
            "inic" => Some(Self::InternationalIntercontinental),
            "ally" => Some(Self::Ally),
            "allyic" => Some(Self::AllyIntercontinental),
            "enemy" => Some(Self::Enemy),
            "enemyic" => Some(Self::EnemyIntercontinental),
            "team" => Some(Self::Team),
            "teamic" => Some(Self::TeamIntercontinental),
            _ => None,
        }
    }

    fn ruleset_name(self) -> &'static str {
        match self {
            Self::National => "National",
            Self::NationalIntercontinental => "NationalIC",
            Self::International => "IN",
            Self::InternationalIntercontinental => "INIC",
            Self::Ally => "Ally",
            Self::AllyIntercontinental => "AllyIC",
            Self::Enemy => "Enemy",
            Self::EnemyIntercontinental => "EnemyIC",
            Self::Team => "Team",
            Self::TeamIntercontinental => "TeamIC",
        }
    }
}

/// Behavior when a previously established route becomes illegal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TradeRouteCancellation {
    Active,
    Inactive,
    Cancel,
}

/// One-time bonus granted when a route is established.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TradeRevenueStyle {
    Classic,
    Simple,
}

/// One-time caravan bonus calculation selected by the Freeciv server setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CaravanBonusStyle {
    Classic,
    Logarithmic,
}

/// When Freeciv selects the Goods carried by a route actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GoodsSelection {
    Leaving,
    Arrival,
}

/// Flags supported by pinned Freeciv `common/traderoutes.h` Goods records.
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

impl RulesetData {
    /// Resolve C2C3 trade-route rules with the pinned Freeciv defaults for
    /// settings omitted from the selected ruleset.
    pub(crate) fn trade_route_settings(&self) -> Result<TradeRouteRulesetSettings, LoadError> {
        let mut route_types: Vec<_> = TradeRouteType::ALL
            .into_iter()
            .map(default_route_type_settings)
            .collect();
        if let Some(parameter) = optional_parameter(self, "game.ruleset", "trade", "settings")? {
            let Value::Table { columns, rows } = &parameter.value else {
                return Err(LoadError::new(format!(
                    "game.ruleset [trade].settings must be a table, found {:?}",
                    parameter.value
                )));
            };
            parse_route_type_table(columns, rows, &mut route_types)?;
        }

        let minimum_trade_route_value = optional_parameter_integer(
            self,
            "trade",
            "min_trade_route_val",
            FREECIV_DEFAULT_MIN_TRADE_ROUTE_VALUE,
        )?;
        let reveal_trade_partner = optional_parameter_boolean(
            self,
            "trade",
            "reveal_trade_partner",
            FREECIV_DEFAULT_REVEAL_TRADE_PARTNER,
        )?;
        let goods_selection = optional_parameter_string(
            self,
            "trade",
            "goods_selection",
            FREECIV_DEFAULT_GOODS_SELECTION.ruleset_name(),
        )
        .and_then(parse_goods_selection)?;

        // These are Freeciv server settings rather than [trade] parameters.
        // C2C3 explicitly sets tradeworldrelpct to 100; its other three values
        // are inherited from the pinned server defaults.
        let minimum_city_distance =
            optional_setting_integer(self, "trademindist", FREECIV_DEFAULT_TRADE_MIN_DISTANCE)?;
        validate_setting_range("trademindist", minimum_city_distance, 1, 999)?;
        let revenue_style = optional_setting_string(
            self,
            "trade_revenue_style",
            FREECIV_DEFAULT_TRADE_REVENUE_STYLE.ruleset_name(),
        )
        .and_then(parse_revenue_style)?;
        let world_relative_percent = optional_setting_integer(
            self,
            "tradeworldrelpct",
            FREECIV_DEFAULT_TRADE_WORLD_RELATIVE_PERCENT,
        )?;
        validate_setting_range("tradeworldrelpct", world_relative_percent, 0, 100)?;
        let caravan_bonus_style = optional_setting_string(
            self,
            "caravan_bonus_style",
            FREECIV_DEFAULT_CARAVAN_BONUS_STYLE.ruleset_name(),
        )
        .and_then(parse_caravan_bonus_style)?;

        let goods = self
            .rule_records
            .iter()
            .filter(|record| record.namespace.eq_ignore_ascii_case("goods"))
            .map(parse_good_record)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(TradeRouteRulesetSettings {
            route_types,
            goods,
            minimum_trade_route_value,
            reveal_trade_partner,
            goods_selection,
            minimum_city_distance,
            revenue_style,
            world_relative_percent,
            caravan_bonus_style,
        })
    }
}

fn default_route_type_settings(route_type: TradeRouteType) -> TradeRouteTypeSettings {
    // common/traderoutes.c initializes every route percentage to 100; the
    // zero-valued C enums mean Active cancellation and no one-time bonus.
    TradeRouteTypeSettings {
        route_type,
        trade_percent: 100,
        cancellation: TradeRouteCancellation::Active,
        bonus: TradeRouteBonus::None,
    }
}

fn parse_route_type_table(
    columns: &[String],
    rows: &[Vec<Value>],
    route_types: &mut [TradeRouteTypeSettings],
) -> Result<(), LoadError> {
    let type_column = table_column(columns, "type")?.ok_or_else(|| {
        LoadError::new("game.ruleset [trade].settings table is missing `type` column")
    })?;
    let pct_column = table_column(columns, "pct")?;
    let cancellation_column = table_column(columns, "cancelling")?;
    let bonus_column = table_column(columns, "bonus")?;

    for (row_index, row) in rows.iter().enumerate() {
        let route_name = required_table_string(row, type_column, row_index, "type")?;
        let route_type = TradeRouteType::parse(route_name).ok_or_else(|| {
            LoadError::new(format!(
                "game.ruleset [trade].settings row {} has unknown route type {route_name:?}",
                row_index + 1
            ))
        })?;
        let settings = route_types
            .iter_mut()
            .find(|settings| settings.route_type == route_type)
            .expect("all Freeciv trade route types have defaults");

        // Freeciv's ruleset loader uses these per-cell defaults. Repeated rows
        // overwrite prior values, matching its ordered table traversal.
        if let Some(column) = pct_column
            && let Some(value) = row_value(row, column, row_index, "pct")?
        {
            settings.trade_percent = integer_value(value, "trade.settings.pct")?;
        }
        if let Some(column) = cancellation_column
            && let Some(value) = row_value(row, column, row_index, "cancelling")?
        {
            settings.cancellation =
                parse_cancellation(string_value(value, "trade.settings.cancelling")?)?;
        }
        if let Some(column) = bonus_column
            && let Some(value) = row_value(row, column, row_index, "bonus")?
        {
            settings.bonus = parse_route_bonus(string_value(value, "trade.settings.bonus")?)?;
        }
    }
    Ok(())
}

fn table_column(columns: &[String], name: &str) -> Result<Option<usize>, LoadError> {
    let mut matches = columns
        .iter()
        .enumerate()
        .filter(|(_, column)| column.eq_ignore_ascii_case(name));
    let column = matches.next().map(|(index, _)| index);
    if matches.next().is_some() {
        return Err(LoadError::new(format!(
            "game.ruleset [trade].settings repeats {name:?} column"
        )));
    }
    Ok(column)
}

fn row_value<'a>(
    row: &'a [Value],
    column: usize,
    row_index: usize,
    field: &str,
) -> Result<Option<&'a Value>, LoadError> {
    let value = row.get(column).ok_or_else(|| {
        LoadError::new(format!(
            "game.ruleset [trade].settings row {} has no {field:?} cell",
            row_index + 1
        ))
    })?;
    Ok((!matches!(value, Value::Empty)).then_some(value))
}

fn required_table_string<'a>(
    row: &'a [Value],
    column: usize,
    row_index: usize,
    field: &str,
) -> Result<&'a str, LoadError> {
    let value = row_value(row, column, row_index, field)?.expect("row_value returns Some");
    string_value(value, &format!("trade.settings.{field}"))
}

fn string_value<'a>(value: &'a Value, source: &str) -> Result<&'a str, LoadError> {
    match value {
        Value::String(value) => Ok(value),
        other => Err(LoadError::new(format!(
            "{source} must be a string, found {other:?}"
        ))),
    }
}

fn integer_value(value: &Value, source: &str) -> Result<i32, LoadError> {
    let Value::Integer(value) = value else {
        return Err(LoadError::new(format!(
            "{source} must be an integer, found {value:?}"
        )));
    };
    i32::try_from(*value)
        .map_err(|_| LoadError::new(format!("{source} integer {value} is outside i32 range")))
}

fn parse_cancellation(value: &str) -> Result<TradeRouteCancellation, LoadError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "active" => Ok(TradeRouteCancellation::Active),
        "inactive" => Ok(TradeRouteCancellation::Inactive),
        "cancel" => Ok(TradeRouteCancellation::Cancel),
        _ => Err(LoadError::new(format!(
            "game.ruleset [trade].settings has unknown cancellation type {value:?}"
        ))),
    }
}

fn parse_route_bonus(value: &str) -> Result<TradeRouteBonus, LoadError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(TradeRouteBonus::None),
        "gold" => Ok(TradeRouteBonus::Gold),
        "science" => Ok(TradeRouteBonus::Science),
        "both" => Ok(TradeRouteBonus::Both),
        _ => Err(LoadError::new(format!(
            "game.ruleset [trade].settings has unknown bonus type {value:?}"
        ))),
    }
}

fn parse_goods_selection(value: String) -> Result<GoodsSelection, LoadError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "leaving" => Ok(GoodsSelection::Leaving),
        "arrival" => Ok(GoodsSelection::Arrival),
        _ => Err(LoadError::new(format!(
            "game.ruleset [trade].goods_selection has unknown value {value:?}"
        ))),
    }
}

fn parse_revenue_style(value: String) -> Result<TradeRevenueStyle, LoadError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "classic" => Ok(TradeRevenueStyle::Classic),
        "simple" => Ok(TradeRevenueStyle::Simple),
        _ => Err(LoadError::new(format!(
            "server setting `trade_revenue_style` has unknown value {value:?}"
        ))),
    }
}

fn parse_caravan_bonus_style(value: String) -> Result<CaravanBonusStyle, LoadError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "classic" => Ok(CaravanBonusStyle::Classic),
        "logarithmic" => Ok(CaravanBonusStyle::Logarithmic),
        _ => Err(LoadError::new(format!(
            "server setting `caravan_bonus_style` has unknown value {value:?}"
        ))),
    }
}

fn optional_parameter_integer(
    rules: &RulesetData,
    section: &str,
    key: &str,
    default: i32,
) -> Result<i32, LoadError> {
    let Some(parameter) = optional_parameter(rules, "game.ruleset", section, key)? else {
        return Ok(default);
    };
    match parameter
        .integer_value()
        .map_err(|error| LoadError::new(format!("invalid {section}.{key}: {error}")))?
    {
        Some(value) => i32::try_from(value).map_err(|_| {
            LoadError::new(format!(
                "game.ruleset [{section}].{key} is outside i32 range"
            ))
        }),
        None => Ok(default),
    }
}

fn optional_parameter_boolean(
    rules: &RulesetData,
    section: &str,
    key: &str,
    default: bool,
) -> Result<bool, LoadError> {
    let Some(parameter) = optional_parameter(rules, "game.ruleset", section, key)? else {
        return Ok(default);
    };
    parameter
        .boolean_value()
        .map_err(|error| LoadError::new(format!("invalid {section}.{key}: {error}")))?
        .ok_or_else(|| LoadError::new(format!("game.ruleset [{section}].{key} has no value")))
}

fn optional_parameter_string(
    rules: &RulesetData,
    section: &str,
    key: &str,
    default: &str,
) -> Result<String, LoadError> {
    let Some(parameter) = optional_parameter(rules, "game.ruleset", section, key)? else {
        return Ok(default.to_owned());
    };
    parameter
        .string_value()
        .map_err(|error| LoadError::new(format!("invalid {section}.{key}: {error}")))?
        .map(str::to_owned)
        .ok_or_else(|| LoadError::new(format!("game.ruleset [{section}].{key} has no value")))
}

fn optional_setting<'a>(
    rules: &'a RulesetData,
    name: &str,
) -> Result<Option<&'a SettingRecord>, LoadError> {
    let mut matches = rules
        .settings
        .iter()
        .filter(|setting| setting.name.eq_ignore_ascii_case(name));
    let setting = matches.next();
    if matches.next().is_some() {
        return Err(LoadError::new(format!(
            "selected ruleset repeats `{name}` setting"
        )));
    }
    Ok(setting)
}

fn optional_setting_integer(
    rules: &RulesetData,
    name: &str,
    default: i32,
) -> Result<i32, LoadError> {
    let Some(setting) = optional_setting(rules, name)? else {
        return Ok(default);
    };
    integer_value(&setting.value, &format!("server setting `{name}`"))
}

fn optional_setting_string(
    rules: &RulesetData,
    name: &str,
    default: &str,
) -> Result<String, LoadError> {
    let Some(setting) = optional_setting(rules, name)? else {
        return Ok(default.to_owned());
    };
    Ok(string_value(&setting.value, &format!("server setting `{name}`"))?.to_owned())
}

fn validate_setting_range(
    name: &str,
    value: i32,
    minimum: i32,
    maximum: i32,
) -> Result<(), LoadError> {
    if !(minimum..=maximum).contains(&value) {
        return Err(LoadError::new(format!(
            "server setting `{name}` must be in {minimum}..={maximum}, found {value}"
        )));
    }
    Ok(())
}

fn parse_good_record(record: &RuleRecord) -> Result<TradeGoodRulesetRecord, LoadError> {
    let integer = |key: &str, default: i32| -> Result<i32, LoadError> {
        let value = record.integer_field(key).map_err(|error| {
            LoadError::new(format!("invalid [{}].{key}: {error}", record.section))
        })?;
        match value {
            Some(value) => i32::try_from(value).map_err(|_| {
                LoadError::new(format!("[{}].{key} is outside i32 range", record.section))
            }),
            None => Ok(default),
        }
    };
    let flags = record
        .string_list_field("flags")
        .map_err(|error| LoadError::new(format!("invalid [{}].flags: {error}", record.section)))?
        .unwrap_or_default()
        .into_iter()
        .map(|flag| parse_good_flag(&flag, &record.section))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TradeGoodRulesetRecord {
        id: record.rule_name.clone(),
        name: record
            .display_name
            .clone()
            .unwrap_or_else(|| record.rule_name.clone()),
        requirements: record.requirements.clone(),
        from_percent: integer("from_pct", FREECIV_DEFAULT_GOODS_PERCENT)?,
        to_percent: integer("to_pct", FREECIV_DEFAULT_GOODS_PERCENT)?,
        one_time_percent: integer("onetime_pct", FREECIV_DEFAULT_GOODS_PERCENT)?,
        select_priority: integer("select_priority", FREECIV_DEFAULT_GOODS_PRIORITY)?,
        replace_priority: integer("replace_priority", FREECIV_DEFAULT_GOODS_PRIORITY)?,
        flags,
    })
}

fn parse_good_flag(value: &str, section: &str) -> Result<TradeGoodFlag, LoadError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "bidirectional" => Ok(TradeGoodFlag::Bidirectional),
        "depletes" => Ok(TradeGoodFlag::Depletes),
        "self-provided" => Ok(TradeGoodFlag::SelfProvided),
        _ => Err(LoadError::new(format!(
            "[{section}] has unknown Goods flag {value:?}"
        ))),
    }
}

impl GoodsSelection {
    fn ruleset_name(self) -> &'static str {
        match self {
            Self::Leaving => "Leaving",
            Self::Arrival => "Arrival",
        }
    }
}

impl TradeRevenueStyle {
    fn ruleset_name(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::Simple => "Simple",
        }
    }
}

impl CaravanBonusStyle {
    fn ruleset_name(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::Logarithmic => "Logarithmic",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_c2c3_trade_routes_are_loaded_with_engine_defaults() {
        let settings = RulesetData::campaign_default()
            .trade_route_settings()
            .unwrap();

        assert_eq!(settings.route_types.len(), 10);
        assert_eq!(
            settings.route_type(TradeRouteType::National),
            Some(&TradeRouteTypeSettings {
                route_type: TradeRouteType::National,
                trade_percent: 0,
                cancellation: TradeRouteCancellation::Cancel,
                bonus: TradeRouteBonus::None,
            })
        );
        assert_eq!(
            settings.route_type(TradeRouteType::InternationalIntercontinental),
            Some(&TradeRouteTypeSettings {
                route_type: TradeRouteType::InternationalIntercontinental,
                trade_percent: 200,
                cancellation: TradeRouteCancellation::Cancel,
                bonus: TradeRouteBonus::Gold,
            })
        );
        assert_eq!(settings.minimum_trade_route_value, 0);
        assert!(!settings.reveal_trade_partner);
        assert_eq!(settings.goods_selection, GoodsSelection::Leaving);
        assert_eq!(settings.minimum_city_distance, 9);
        assert_eq!(settings.revenue_style, TradeRevenueStyle::Classic);
        assert_eq!(settings.world_relative_percent, 100);
        assert_eq!(settings.caravan_bonus_style, CaravanBonusStyle::Classic);

        assert_eq!(settings.goods.len(), 1);
        assert_eq!(
            settings.good("Goods"),
            Some(&TradeGoodRulesetRecord {
                id: "Goods".to_owned(),
                name: "Goods".to_owned(),
                requirements: Vec::new(),
                from_percent: 100,
                to_percent: 100,
                one_time_percent: 100,
                select_priority: 1,
                replace_priority: 1,
                flags: Vec::new(),
            })
        );
    }

    #[test]
    fn omitted_server_trade_settings_use_pinned_engine_defaults() {
        let mut rules = RulesetData::campaign_default().clone();
        rules.settings.retain(|setting| {
            ![
                "trademindist",
                "trade_revenue_style",
                "tradeworldrelpct",
                "caravan_bonus_style",
            ]
            .iter()
            .any(|name| setting.name.eq_ignore_ascii_case(name))
        });

        let settings = rules.trade_route_settings().unwrap();
        assert_eq!(settings.minimum_city_distance, 9);
        assert_eq!(settings.revenue_style, TradeRevenueStyle::Classic);
        assert_eq!(settings.world_relative_percent, 50);
        assert_eq!(settings.caravan_bonus_style, CaravanBonusStyle::Classic);
    }
}
