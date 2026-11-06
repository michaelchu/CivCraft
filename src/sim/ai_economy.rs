use super::*;

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
        for city in self.cities.iter().filter(|city| city.owner == owner) {
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

impl Simulation {
    pub(crate) fn ai_tax_manager_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_war_plans.clear();
        self.ai_purchase_reserve.clear();
        self.ai_government_cache.clear();
        self.city_rapture.clear();
        self.players[1].known.clear();
        self.players[1].government = Government::Despotism;
        self.players[1].researching = Tech::BRONZE_WORKING;
        self.players[1].last_science = 0;
        self.players[1].research = 0;
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = true;
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
        self.create_city(1, 12, 24);
        let home = self.cities[0].id;
        for _ in 0..5 {
            let id = self.spawn_unit(1, UnitKind::Warrior, 12, 24);
            let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
            unit.home_city = Some(home);
            unit.home_order = id;
        }
        self.players[1].rates = [60, 40, 0];
        self.players[1].gold = 3;
        self.choose_ai_rates(1);
        let ordinary = self.players[1].rates == [60, 40, 0];
        self.ai_purchase_reserve.insert(1, 100);
        self.players[1].rates = [60, 40, 0];
        self.choose_ai_rates(1);
        let refill = self.players[1].rates == [50, 50, 0];
        self.players[1].gold = 1000;
        self.players[1].rates = [60, 40, 0];
        self.choose_ai_rates(1);
        let rich = self.players[1].rates == [60, 40, 0];
        self.spawn_unit(HUMAN, UnitKind::Warrior, 35, 35);
        self.ai_war_plans.insert(
            (1, HUMAN),
            diplomacy::WarPlan {
                countdown: 1,
                reason: diplomacy::WarReason::Spoils,
            },
        );
        self.players[1].rates = [60, 40, 0];
        self.choose_ai_rates(1);
        let war = self.players[1].rates == [40, 60, 0];
        self.ai_war_plans.clear();
        self.players[1].researching = Tech::FUTURE;
        self.players[1].rates = [60, 40, 0];
        self.choose_ai_rates(1);
        let future = self.players[1].rates == [0, 60, 40];
        let saved = self.players[1].rates;
        self.players[1].government = Government::Anarchy;
        self.choose_ai_rates(1);
        let anarchy = self.players[1].rates == saved;
        ordinary && refill && rich && war && future && anarchy
    }
}

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

    pub(crate) fn ai_tax_phase_playtest(&mut self) -> (bool, bool) {
        // Eight homed units create a real upkeep bill; the native manager
        // raises tax from 40 to 60. With a scientist already assigned, Library
        // initially wins, but higher tax makes Marketplace win city advice.
        // Running advice first would leave Library in the production queue.
        self.tax_phase_fixture(3, 8, [60, 40, 0], 0);
        self.cities[0].worked_tiles.pop();
        self.cities[0].specialists = [1, 0, 0];
        self.units.retain(|unit| unit.kind == UnitKind::Warrior);
        self.ai_guard_targets.clear();
        self.ai_unit_guards.clear();
        self.players[1].known.extend(
            Tech::all().filter(|tech| matches!(tech.name(), "Alphabet" | "Writing" | "Currency")),
        );
        let old_choice = self.ai_build_choice(&self.cities[0]);
        let mut expected = self.clone();
        expected.choose_ai_government(1);
        expected.choose_ai_rates(1);
        expected.arrange_ai_emergency_workers(1);
        let new_choice = expected.ai_build_choice(&expected.cities[0]);
        self.run_ai_last_activities(1);
        let rates_before_advice = old_choice
            .is_some_and(|choice| choice.production == Production::Building(Building::LIBRARY))
            && new_choice.is_some_and(|choice| {
                choice.production == Production::Building(Building::MARKETPLACE)
                    && choice.want > old_choice.unwrap().want
            })
            && self.players[1].government == Government::Despotism
            && self.players[1].rates == [40, 60, 0]
            && self.cities[0].production == Production::Building(Building::MARKETPLACE);

        // Removing luxury creates disorder. The source emergency CM must
        // assign an entertainer before routine military advice can resume.
        self.tax_phase_fixture(8, 5, [0, 60, 40], 5000);
        let mut without_rearrange = self.clone();
        without_rearrange.choose_ai_government(1);
        without_rearrange.choose_ai_rates(1);
        let no_cm_disorder = without_rearrange
            .city_forecast(&without_rearrange.cities[0])
            .disorder;
        let no_cm_advice = without_rearrange
            .ai_build_choice(&without_rearrange.cities[0])
            .is_none_or(|choice| choice.production != Production::Unit(UnitKind::Phalanx));
        self.run_ai_last_activities(1);
        let cm_before_advice = no_cm_disorder
            && no_cm_advice
            && self.players[1].government == Government::Despotism
            && self.players[1].rates == [60, 40, 0]
            && !self.city_forecast(&self.cities[0]).disorder
            && self.cities[0].specialists[2] > 0
            && self.cities[0].production == Production::Unit(UnitKind::Phalanx);
        (rates_before_advice, cm_before_advice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> TaxBudget {
        TaxBudget {
            maxrate: 80,
            saved: [60, 40, 0],
            trade: 100,
            expenses: 90,
            income: 40,
            gold: 0,
            gold_reserve: 100,
            science_upkeep: 70,
            bulbs: 0,
            last_science: 0,
            wants_science: true,
            war_footing: false,
        }
    }

    #[test]
    fn normal_tax_rates_match_sixty_four_literal_native_function_vectors() {
        // Gold/income/upkeep data are inputs to the ORIGINAL native function,
        // not expected values generated by this Rust implementation.
        for line in include_str!("../../tests/fixtures/freeciv_normal_tax_vectors.tsv")
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let x: Vec<i32> = line
                .split_whitespace()
                .map(|value| value.parse().unwrap())
                .collect();
            let decision = native_tax_decision(TaxBudget {
                maxrate: x[0],
                trade: x[1],
                expenses: x[2],
                income: x[3],
                gold: x[4],
                gold_reserve: x[5],
                science_upkeep: x[6],
                bulbs: x[7],
                last_science: x[8],
                saved: [x[9], x[10], x[11]],
                wants_science: x[12] != 0,
                war_footing: x[13] != 0,
            });
            assert_eq!(decision.rates, [x[14], x[15], x[16]], "{line}");
        }
    }

    #[test]
    fn science_estimate_uses_retained_tax_iteration_and_prior_bulb_balance() {
        let decision = native_tax_decision(budget());
        assert_eq!(decision.delta, [-20, -90]);
        assert_eq!(decision.minimum, [70, 50]);
        assert_eq!(decision.balance, [Some(70), Some(60)]);
        assert_eq!(decision.rates, [70, 30, 0]); // Science wins incompatible balances.
        let mut funded = budget();
        funded.gold = 1600;
        funded.bulbs = 160;
        let decision = native_tax_decision(funded);
        assert_eq!(decision.minimum, [60, 0]); // Exact16turn reserve denominator.
    }

    #[test]
    fn treasury_and_bulb_insolvency_switch_priority_on_war_preparation() {
        let mut state = budget();
        state.expenses = 250;
        state.science_upkeep = 120;
        assert_eq!(native_tax_decision(state).rates, [80, 20, 0]);
        let mut state = budget();
        state.expenses = 250;
        state.science_upkeep = 120;
        state.war_footing = true;
        assert_eq!(native_tax_decision(state).rates, [20, 80, 0]);
    }

    #[test]
    fn live_last_activities_apply_tax_before_building_advice_and_workers_before_defense() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_tax_phase_playtest(), (true, true));
    }

    #[test]
    fn purchase_reserve_tax_fixture_uses_an_observable_native_balance_floor() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_reserve_playtest());
    }

    #[test]
    fn emergency_worker_hook_preserves_normal_assignments_but_rearranges_full_granary() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.tax_phase_fixture(1, 0, [60, 40, 0], 1000);
        game.units.clear();
        game.cities[0].worked_tiles.clear();
        game.cities[0].specialists = [1, 0, 0];
        game.cities[0].food = 0;
        assert!(!game.city_forecast(&game.cities[0]).disorder);
        game.arrange_ai_emergency_workers(1);
        assert_eq!(game.cities[0].specialists, [1, 0, 0]);
        game.cities[0].food = Simulation::growth_box(1);
        game.arrange_ai_emergency_workers(1);
        assert_eq!(game.cities[0].specialists, [0; 3]);
        assert_eq!(game.cities[0].worked_tiles.len(), 1);
    }

    #[test]
    fn actual_city_tax_manager_covers_reserve_war_future_and_anarchy() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_tax_manager_playtest());
    }

    #[test]
    fn civ2civ3_celebrating_history_does_not_enable_absent_rapture_growth_tax_policy() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_tax_manager_playtest());
        game.players[1].government = Government::Despotism;
        game.players[1].researching = Tech::BRONZE_WORKING;
        game.players[1].rates = [60, 40, 0];
        game.cities[0].was_happy = true;
        game.city_rapture.insert(game.cities[0].id, 5);
        let workers = game.cities[0].worked_tiles.clone();
        let specialists = game.cities[0].specialists;
        game.choose_ai_rates(1);
        assert_eq!(game.players[1].rates[2], 0);
        assert_eq!(game.cities[0].worked_tiles, workers);
        assert_eq!(game.cities[0].specialists, specialists);
        assert_eq!(game.city_rapture.get(&game.cities[0].id), Some(&5));
    }
}
