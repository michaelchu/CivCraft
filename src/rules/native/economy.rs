//! City economy and resource yield overrides.

use super::*;

pub(crate) struct EconomyCatalog {
    pub(crate) city: NativeCityEconomy,
    pub(crate) resources: Vec<NativeResource>,
}

pub(crate) struct NativeResource {
    pub(crate) id: String,
    pub(crate) food: i32,
    pub(crate) shield: i32,
    pub(crate) trade: i32,
}

/// Who pays gold upkeep: each city, the nation, or a mix of both.
#[allow(
    dead_code,
    reason = "complete Freeciv option set; the campaign data selects Mixed"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeGoldUpkeepStyle {
    City,
    Mixed,
    Nation,
}

pub(crate) struct NativeCityEconomy {
    pub(crate) city_min_distance: i32,
    pub(crate) food_cost: i32,
    pub(crate) happy_cost: i32,
    pub(crate) granary_food_initial: Vec<i32>,
    pub(crate) granary_food_increment: i32,
    pub(crate) foodbox_percent: i32,
    pub(crate) output_granularity: i32,
    pub(crate) city_center_minimum: [i32; 3],
    pub(crate) no_trade_size: i32,
    pub(crate) full_trade_size: i32,
    pub(crate) gold_upkeep_style: NativeGoldUpkeepStyle,
    pub(crate) celebrate_size_limit: i32,
}

pub(crate) fn economy() -> &'static EconomyCatalog {
    static CATALOG: OnceLock<EconomyCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let res = |id: &str, food, shield, trade| NativeResource {
            id: id.to_owned(),
            food,
            shield,
            trade,
        };
        EconomyCatalog {
            city: NativeCityEconomy {
                city_min_distance: 3,
                food_cost: 2,
                happy_cost: 2,
                granary_food_initial: vec![20, 20, 20, 20, 20, 30, 30, 40],
                granary_food_increment: 0,
                foodbox_percent: 100,
                output_granularity: 1,
                city_center_minimum: [0, 0, 0],
                no_trade_size: 0,
                full_trade_size: 1,
                gold_upkeep_style: NativeGoldUpkeepStyle::Mixed,
                celebrate_size_limit: 3,
            },
            resources: vec![
                res("Gold", 0, 0, 6),
                res("Iron", 0, 3, 0),
                res("Game", 2, 1, 0),
                res("Furs", 1, 0, 3),
                res("Coal", 0, 2, 0),
                res("Fish", 2, 0, 0),
                res("Fruit", 3, 0, 1),
                res("Gems", 0, 0, 4),
                res("Buffalo", 0, 2, 0),
                res("Wheat", 2, 0, 0),
                res("Oasis", 3, 0, 0),
                res("Peat", 0, 4, 0),
                res("Pheasant", 2, 0, 0),
                res("Resources", 0, 1, 0),
                res("icy_ivory", 1, 1, 4),
                res("Silk", 0, 0, 3),
                res("Spice", 1, 0, 4),
                res("Whales", 1, 1, 0),
                res("Wine", 0, 0, 4),
                res("Oil", 0, 3, 0),
            ],
        }
    })
}
