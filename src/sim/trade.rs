//! Trade-route planning, merchant movement, and delivery rewards.

use super::normal_paths::GotoTask;
use super::*;

mod merchant_ferries;
mod merchant_planning;
use merchant_planning::*;
mod routes;
use crate::rules::Genus;
use std::collections::BTreeMap;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(super) struct MerchantMission {
    pub(super) city: u32,
    pub(super) wonder: bool,
    pub(super) owner: usize,
}

struct FerryDelivery {
    value: f64,
    ship: u32,
    city: u32,
    approach: (i32, i32),
    landing: Option<(i32, i32)>,
    wonder: bool,
    pickup: (i32, i32),
    pickup_walk: Vec<(i32, i32)>,
}

impl Simulation {
    pub(super) fn merchant_production_choice(&self, _city: &City) -> Option<(UnitKind, f64)> {
        // daidomestic.c:dai_choose_trade_route checks national destinations
        // first and examines foreign ones only when NationalIC > National.
        // Civ2Civ3 sets both national percentages to zero, so dest_city_found
        // remains false. Existing merchants may still trade; wonder helpers
        // have a separate production advisor.
        None
    }

    pub(super) fn run_ai_merchant(&mut self, merchant: &Unit) {
        if !self.ai_merchant_missions.contains_key(&merchant.id) {
            self.prepare_merchant_mission(merchant);
        }
        let Some(snapshot) = self
            .units
            .iter()
            .find(|unit| unit.id == merchant.id)
            .cloned()
        else {
            return;
        };
        let merchant = &snapshot;
        let Some(MerchantPlan {
            city,
            wonder,
            approach: (x, y),
            ..
        }) = self.merchant_plan(merchant, 0)
        else {
            return;
        };
        if (merchant.x, merchant.y) != (x, y) {
            self.ai_goto_unit(
                merchant.id,
                (x, y),
                if wonder {
                    GotoTask::Wonder
                } else {
                    GotoTask::Trade
                },
            );
        }
        if wonder {
            self.help_wonder(merchant.id, city);
        } else {
            self.deliver_goods(merchant.id, city);
        }
    }

    pub(super) fn deliver_goods(&mut self, unit: u32, city: u32) -> bool {
        self.prune_trade_routes();
        let Some(merchant) = self.unit_by_id(unit) else {
            return false;
        };
        let Some(to) = self
            .cities
            .iter()
            .find(|item| item.id == city && self.can_deliver_goods(merchant, item))
        else {
            return false;
        };
        let from = self.home_for_merchant(merchant).unwrap();
        let owner = merchant.owner;
        let (a, b) = (from.id, to.id);
        let replacements = self.route_replacements(from, to);
        let gold = self.delivery_gold(from, to);
        let destination = to.name.clone();
        if let Some((old_home, old_dest)) = replacements {
            if let Some(old) = old_home {
                self.remove_route(a, old);
            }
            if let Some(old) = old_dest {
                self.remove_route(b, old);
            }
            self.trade_routes.push((a, b));
        }
        self.players[owner].gold += gold;
        self.remove_units_with_cargo(vec![unit]);
        self.refresh_visibility();
        let outcome = if replacements.is_some() {
            "Trade route established"
        } else {
            "Goods delivered"
        };
        self.add_event(format!("{outcome} in {destination}. Earned {gold} gold."));
        true
    }
}

#[cfg(test)]
#[path = "trade/tests.rs"]
mod tests;
