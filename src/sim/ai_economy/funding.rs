//! Bounded cash funding over native tax advice. War status alone is not a bill.
use super::*;
use serde::Serialize;

pub(super) struct MilitaryNeed {
    routine_purchase: i32,
    urgent_purchase: i32,
}

impl MilitaryNeed {
    fn purchase(&self) -> i32 {
        self.routine_purchase.max(self.urgent_purchase)
    }
}

#[derive(Serialize)]
pub(in crate::sim) struct FundingDecision {
    owner: usize,
    wars: usize,
    preparing: bool,
    economic_crisis: bool,
    wants_science: bool,
    previous_income: i32,
    pub(super) rates: [i32; 3],
    native_rates: [i32; 3],
    gold: i32,
    reserve: i32,
    target: i32,
    military_purchase: i32,
    urgent_purchase: i32,
    upkeep_buffer: i32,
    required_income: i32,
    refill_science_floor: i32,
    urgent_purchase_affordable: bool,
    projected_income: i32,
    net_science: i32,
    target_met: bool,
    reason: &'static str,
}
impl trace::Payload for FundingDecision {
    const KIND: &'static str = "economy.funding_decision";
}

impl Simulation {
    /// Cash that can avert operating losses or enable a selected urgent defense.
    /// Routine reserve refills and speculative army purchases are not aid needs.
    pub(in crate::sim) fn diplomatic_funding_shortfall(&self, owner: usize) -> i32 {
        self.diplomatic_funding_target(owner)
            .saturating_sub(self.players[owner].gold)
            .max(0)
    }

    pub(in crate::sim) fn diplomatic_funding_target(&self, owner: usize) -> i32 {
        let strategy = self.strategic_priorities(owner);
        let military = self.ai_military_funding_need(owner, strategy.preparing);
        let operating = if strategy.projected_income < 0 {
            strategy.gold_reserve
        } else {
            0
        };
        let urgent = if military.urgent_purchase > 0 {
            military
                .urgent_purchase
                .saturating_add(self.strategic_upkeep_buffer(owner))
        } else {
            0
        };
        operating.max(urgent)
    }

    pub(super) fn ai_military_funding_need(&self, owner: usize, preparing: bool) -> MilitaryNeed {
        let mut need = MilitaryNeed {
            routine_purchase: 0,
            urgent_purchase: 0,
        };
        for city in self.cities_owned_by(owner) {
            let assessment = self.advisor_guard_city_assessment(city);
            let emergency = assessment.defense == 0 && self.city_grave_danger(city) > 0;
            let mut proposed = city.clone();
            let mut urgent = false;
            if (emergency || assessment.urgency > 1)
                && let Some(choice) = self.native_city_defense_choice(city).0
            {
                // Match the cash spender's emergency/urgent eligibility.
                // Ordinary suggestions are not committed purchases; the
                // city's current queue must already select the defender.
                urgent = emergency
                    || (choice.want > 200.0
                        && city.production == choice.production
                        && assessment.urgency > 1);
                if urgent {
                    proposed.production = choice.production;
                }
            }
            if !urgent
                && (!preparing
                    || city.shields <= 0
                    || !matches!(city.production, Production::Unit(kind) if kind.is_military()))
            {
                continue;
            }
            if proposed.shields + self.city_forecast(city).shield_surplus()
                >= proposed.production.cost()
            {
                continue;
            }
            if let Some(price) = self.rush_cost(&proposed) {
                if urgent {
                    need.urgent_purchase = need.urgent_purchase.max(price);
                } else {
                    need.routine_purchase = need.routine_purchase.max(price);
                }
            }
        }
        need
    }

    pub(super) fn ai_funding_decision(
        &self,
        owner: usize,
        native_rates: [i32; 3],
        strategy: &ai_strategy::StrategicPriorities,
        military: MilitaryNeed,
    ) -> FundingDecision {
        let gold = self.players[owner].gold;
        // Keep upkeep cash behind the concrete purchase instead of funding a
        // purchase by emptying the next accounting phase's treasury.
        let upkeep_buffer = self.strategic_upkeep_buffer(owner);
        let routine_target = if military.routine_purchase > 0 {
            military
                .routine_purchase
                .saturating_add(strategy.gold_reserve.saturating_mul(2))
        } else {
            0
        };
        let urgent_target = if military.urgent_purchase > 0 {
            military.urgent_purchase.saturating_add(upkeep_buffer)
        } else {
            0
        };
        let target = strategy.gold_reserve.max(routine_target).max(urgent_target);
        let gap = target.saturating_sub(gold).max(0);
        let turns = ai_strategy::reserve_refill_turns();
        let mut required_income = gap / turns + i32::from(gap % turns != 0);
        if military.urgent_purchase > 0 {
            required_income = required_income.max(urgent_target.saturating_sub(gold).max(0));
        }
        let maxrate = self.players[owner].government.max_rate();
        let mut rates = native_rates;
        let (mut income, gross_science) = self.ai_projected_budget_at_rates(owner, rates);
        let science_upkeep = self.technology_upkeep(owner);
        let mut net_science = gross_science - science_upkeep;
        let baseline_science = net_science;
        // Discretionary refills share revenue according to the common
        // priorities. Scientific preferences survive an unaffordable rush
        // request; urgent funding can override this floor only if it can
        // actually enable the purchase at the next accounting boundary.
        let cash_priority = if military.purchase() > 0 {
            strategy.military
        } else {
            strategy.economy
        };
        let denominator = (strategy.science + cash_priority).max(1) * 10;
        let refill_science_floor =
            ((native_rates[0] * strategy.science + denominator - 1) / denominator * 10)
                .min(native_rates[0]);
        let mut maximum_tax = native_rates;
        let available = maximum_tax[0].min(maxrate - maximum_tax[1]);
        maximum_tax[0] -= available;
        maximum_tax[1] += available;
        let maximum_income = self.ai_projected_income_at_rates(owner, maximum_tax);
        let urgent_purchase_affordable =
            military.urgent_purchase > 0 && gold.saturating_add(maximum_income) >= urgent_target;
        let urgent_gap = urgent_target.saturating_sub(gold).max(0);
        let reason = if military.purchase() > 0 && routine_target.max(urgent_target) > gold {
            "military_purchase"
        } else if gap > 0 {
            "reserve_refill"
        } else if income < 0 {
            "upkeep"
        } else if self.players[owner].researching == Tech::FUTURE {
            "no_research"
        } else {
            "research"
        };
        // Luxury remains the native choice; only move science into tax. Search
        // is bounded to at most ten rate steps and never changes city workers.
        let mut candidate = rates;
        while income < required_income && candidate[0] > 0 && candidate[1] < maxrate {
            let step = 10.min(candidate[0]).min(maxrate - candidate[1]);
            candidate[0] -= step;
            candidate[1] += step;
            let urgent_override =
                urgent_purchase_affordable && urgent_gap > 0 && income < urgent_gap;
            if candidate[0] < refill_science_floor && income >= 0 && !urgent_override {
                break;
            }
            let (next_income, next_science) = self.ai_projected_budget_at_rates(owner, candidate);
            let next_net_science = next_science - science_upkeep;
            // Refilling discretionary reserves must not create research debt.
            // An imminent upkeep default or an unfunded urgent defense retains
            // the native willingness to sacrifice science to survive.
            if next_income > income
                && (next_net_science >= baseline_science.min(0)
                    || gold.saturating_add(income) < 0
                    || urgent_override)
            {
                rates = candidate;
                income = next_income;
                net_science = next_net_science;
            }
        }
        FundingDecision {
            owner,
            wars: strategy.wars,
            preparing: strategy.preparing,
            economic_crisis: strategy.economic_crisis,
            wants_science: self.players[owner].researching != Tech::FUTURE,
            previous_income: strategy.projected_income,
            rates,
            native_rates,
            gold,
            reserve: strategy.gold_reserve,
            target,
            military_purchase: military.purchase(),
            urgent_purchase: military.urgent_purchase,
            upkeep_buffer,
            required_income,
            refill_science_floor,
            urgent_purchase_affordable,
            projected_income: income,
            net_science,
            target_met: income >= required_income,
            reason,
        }
    }
}

mod scenarios;
#[cfg(test)]
mod tests;
