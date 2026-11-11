//! Focused gameplay scenarios for the parent subsystem.

use super::*;

impl super::Simulation {
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
        // Despotism supports four units for free in Civ III; seven Warriors
        // reproduce the three-unit upkeep burden this scenario needs.
        for _ in 0..7 {
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

    pub(crate) fn ai_tax_phase_playtest(&mut self) -> (bool, bool) {
        // Ten homed units create the same upkeep bill under the Civ III
        // Despotism allowance; the native manager
        // raises tax from 40 to 60. With a scientist already assigned, Library
        // initially wins, but higher tax makes Marketplace win city advice.
        // Running advice first would leave Library in the production queue.
        self.tax_phase_fixture(3, 10, [60, 40, 0], 0);
        self.cities[0].worked_tiles.pop();
        self.cities[0].specialists = [1, 0, 0];
        self.units.retain(|unit| unit.kind == UnitKind::Warrior);
        self.ai_guard_targets.clear();
        self.ai_unit_guards.clear();
        self.players[1].known.extend(
            Tech::all()
                .filter(|tech| matches!(tech.name(), "Alphabet" | "Literature" | "Currency")),
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

        // Removing luxury creates disorder. Emergency citizen rearrangement
        // must happen before the city production advisor chooses a build.
        self.tax_phase_fixture(8, 7, [0, 60, 40], 5000);
        let mut without_rearrange = self.clone();
        without_rearrange.choose_ai_government(1);
        without_rearrange.choose_ai_rates(1);
        let no_cm_disorder = without_rearrange
            .city_forecast(&without_rearrange.cities[0])
            .disorder;
        let no_cm_advice = without_rearrange
            .ai_build_choice(&without_rearrange.cities[0])
            .is_none();
        let mut with_rearrange = without_rearrange.clone();
        with_rearrange.arrange_ai_emergency_workers(1);
        let expected_production =
            with_rearrange.choose_ai_production(1, with_rearrange.cities[0].id);
        self.run_ai_last_activities(1);
        let cm_before_advice = no_cm_disorder
            && no_cm_advice
            && self.players[1].government == Government::Despotism
            && self.players[1].rates == [60, 40, 0]
            && !self.city_forecast(&self.cities[0]).disorder
            && self.cities[0].specialists.iter().sum::<i32>() > 0
            && self.cities[0].production == expected_production;
        (rates_before_advice, cm_before_advice)
    }
}
