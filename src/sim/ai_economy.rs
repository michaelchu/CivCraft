//! Implements ai economy behavior within the sim subsystem.

use super::*;

mod playtest;

// dai/default/daihand.c:dai_manage_taxes, Normal/Civ2Civ3 configuration.
const RESERVE_TURNS: i32 = 10;
const RAPTURE_DELAY: i32 = 1;
const DELTA_TAX_PERCENT: i32 = 50;
const DELTA_SCIENCE_PERCENT: i32 = 10;

struct TaxBudget {
    maxrate: i32,
    saved: [i32; 3],
    trade: i32,
    expenses: i32,
    income: i32,
    gold: i32,
    gold_reserve: i32,
    science_upkeep: i32,
    bulbs: i32,
    last_science: i32,
    wants_science: bool,
    war_footing: bool,
}

struct TaxDecision {
    rates: [i32; 3],
    #[cfg(test)]
    delta: [i32; 2],
    #[cfg(test)]
    minimum: [i32; 2],
    #[cfg(test)]
    balance: [Option<i32>; 2],
}

fn remaining(rates: [i32; 3]) -> i32 {
    (100 - rates.iter().sum::<i32>()).max(0)
}

fn allocate_remaining(rates: &mut [i32; 3], maxrate: i32, order: [usize; 3]) {
    for index in order {
        rates[index] = maxrate.min(rates[index] + remaining(*rates));
    }
}

fn native_tax_decision(budget: TaxBudget) -> TaxDecision {
    let maxrate = budget.maxrate.clamp(34, 100);
    // Civ2Civ3 has no Rapture_Grow effect, so the CM celebration branch
    // never runs and adv->celebrate remains FALSE. The native prospective
    // celebration reserve still applies, even though that branch is skipped.
    let reserve_turns = RESERVE_TURNS + (RAPTURE_DELAY + 1) * 3;
    let mut result = Simulation::distribute_trade(budget.trade, budget.saved);
    let delta_tax = result[1] - budget.expenses - budget.income;
    let mut tax_minimum = None;
    let mut tax_balance = None;
    let mut rates = [maxrate, (100 - maxrate * 2).max(0), 0];
    rates[2] = 100 - rates[0] - rates[1];
    while rates[1] <= maxrate && rates[0] >= 0 && rates[2] >= 0 {
        result = Simulation::distribute_trade(budget.trade, rates);
        let balance = result[1] - budget.expenses - delta_tax * DELTA_TAX_PERCENT / 100;
        let minimum = -(budget.gold / reserve_turns);
        if tax_minimum.is_none() && balance > minimum {
            tax_minimum = Some(rates[1]);
        }
        if budget.expenses == 0 || balance > 0 {
            if budget.gold < budget.gold_reserve && rates[1] < maxrate {
                rates[1] += 10;
                if rates[2] > 0 {
                    rates[2] -= 10;
                } else {
                    rates[0] -= 10;
                }
            }
            tax_balance = Some(rates[1]);
            break;
        }
        rates[1] += 10;
        if rates[2] > 0 {
            rates[2] -= 10;
        } else {
            rates[0] -= 10;
        }
    }
    let tax_minimum = tax_minimum.unwrap_or(maxrate);

    // Native code deliberately retains the FINAL TAX LOOP distribution
    // here, rather than recomputing it with the saved original rates.
    let delta_science = result[0] - budget.science_upkeep - budget.last_science;
    let mut science_minimum = None;
    let mut science_balance = None;
    rates = [(100 - maxrate * 2).max(0), maxrate, 0];
    rates[2] = 100 - rates[0] - rates[1];
    while rates[0] <= maxrate && rates[1] >= 0 && rates[2] >= 0 {
        result = Simulation::distribute_trade(budget.trade, rates);
        let balance =
            result[0] - budget.science_upkeep - delta_science * DELTA_SCIENCE_PERCENT / 100;
        let minimum = -(budget.bulbs / reserve_turns);
        if science_minimum.is_none() && balance > minimum {
            science_minimum = Some(rates[0]);
        }
        if budget.science_upkeep == 0 || balance > 0 {
            science_balance = Some(rates[0]);
            break;
        }
        rates[0] += 10;
        if rates[2] > 0 {
            rates[2] -= 10;
        } else {
            rates[1] -= 10;
        }
    }
    let science_minimum = science_minimum.unwrap_or(maxrate);

    rates = [0; 3];
    match (tax_balance, science_balance) {
        (Some(tax), Some(science)) if tax + science <= 100 => {
            rates[0] = science;
            rates[1] = tax;
        }
        (_, Some(science)) => {
            rates[0] = science;
            rates[1] = maxrate.min(remaining(rates));
        }
        (Some(tax), None) => {
            rates[1] = tax;
            rates[0] = maxrate.min(remaining(rates));
        }
        (None, None) if !budget.wants_science || budget.war_footing => {
            rates[1] = maxrate;
            rates[0] = maxrate.min(remaining(rates));
        }
        (None, None) => {
            rates[0] = maxrate.max(science_minimum);
            rates[1] = maxrate.min(remaining(rates));
        }
    }
    let order = if !budget.wants_science {
        [1, 2, 0]
    } else if budget.war_footing {
        [1, 0, 2]
    } else {
        [0, 1, 2]
    };
    allocate_remaining(&mut rates, maxrate, order);
    debug_assert_eq!(rates.iter().sum::<i32>(), 100);
    debug_assert!(rates.iter().all(|&rate| (0..=maxrate).contains(&rate)));
    let _ = tax_minimum; // Used only by the unreachable Rapture_Grow branch.
    TaxDecision {
        rates,
        #[cfg(test)]
        delta: [delta_science, delta_tax],
        #[cfg(test)]
        minimum: [science_minimum, tax_minimum],
        #[cfg(test)]
        balance: [science_balance, tax_balance],
    }
}

impl Simulation {
    pub(super) fn manage_ai_taxes(&mut self, owner: usize) {
        // Fixed Civ2Civ3 allows changing taxes. Anarchy leaves saved rates
        // alone; production itself distributes all trade to luxury there.
        if self.players[owner].government == Government::Anarchy {
            return;
        }
        let (mut trade, mut expenses, mut income) = (0, 0, 0);
        for city in self.cities_owned_by(owner) {
            let forecast = self.city_forecast(city);
            trade += forecast.trade;
            // GOLD_UPKEEP_MIXED: city usage pays buildings, national usage
            // adds unit gold; city usage+surplus is actual gross gold income.
            expenses += forecast.building_upkeep + forecast.support.gold;
            income += forecast.gold;
        }
        let player = &self.players[owner];
        let decision = native_tax_decision(TaxBudget {
            // AI_LEVEL_NORMAL explicitly sets H_RATES in ai/difficulty.c.
            maxrate: player.government.max_rate(),
            saved: player.rates,
            trade,
            expenses,
            income,
            gold: player.gold,
            gold_reserve: self.ai_gold_reserve(owner),
            science_upkeep: self.technology_upkeep(owner),
            bulbs: player.research,
            last_science: player.last_science,
            // No Tech_Parasite effects exist in Civ2Civ3; Future is the
            // only supported adv_wants_science=false condition.
            wants_science: player.researching != Tech::FUTURE,
            war_footing: self.ai_on_war_footing(owner),
        });
        self.players[owner].rates = decision.rates;
    }
}

impl Simulation {}

impl Simulation {
    fn tax_phase_fixture(&mut self, size: i32, warriors: i32, rates: [i32; 3], gold: i32) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        self.ai_government_cache.clear();
        self.ai_war_plans.clear();
        self.ai_purchase_reserve.clear();
        self.ai_guard_targets.clear();
        self.ai_unit_guards.clear();
        self.unit_goto_tiles.clear();
        self.city_rapture.clear();
        self.players[1].government = Government::Despotism;
        self.players[1].known = vec![Tech::BRONZE_WORKING];
        self.players[1].researching = Tech::WARRIOR_CODE;
        self.players[1].rates = rates;
        self.players[1].gold = gold;
        self.players[1].research = 0;
        self.players[1].last_science = 0;
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.resource = None;
            tile.owner = None;
            tile.river = false;
            tile.road = true;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.oil_well = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.hut = false;
        }
        // A fully occupied island removes expansion bids without changing
        // city or military wants. Its workable radius still has ample land.
        for (_, _, x, y) in self.map_city_radius_at(12, 24) {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.river = true;
        }
        self.create_city(1, 12, 24);
        let home = self.cities[0].id;
        self.cities[0].size = size;
        self.cities[0].food = 10;
        self.cities[0]
            .buildings
            .extend([Building::BARRACKS, Building::AQUEDUCT_RIVER]);
        for _ in 0..warriors {
            let id = self.spawn_unit(1, UnitKind::Warrior, 12, 24);
            let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
            unit.home_city = Some(home);
            unit.home_order = id;
        }
        let charge = self.spawn_unit(1, UnitKind::Catapult, 12, 24);
        self.request_ai_guard(charge);
        self.reconcile_city_workers();
    }
}

#[cfg(test)]
#[path = "ai_economy/tests.rs"]
mod tests;
