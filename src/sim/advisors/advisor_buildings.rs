//! Advisor scoring for city buildings and wonders.

use super::*;

pub(super) struct DefenseEffectContext {
    pub(super) capital: bool,
    pub(super) ocean_city: bool,
    pub(super) coastal: bool,
    pub(super) continent_threat: bool,
    pub(super) ocean_threat: bool,
    pub(super) invasions: bool,
}

impl Simulation {
    pub(super) fn building_happiness_want(&self, city: &City, building: Building) -> f64 {
        self.building_happiness_want_with_luxury(city, building, self.city_forecast(city).luxury)
    }

    fn building_happiness_want_with_luxury(
        &self,
        city: &City,
        building: Building,
        luxury: i32,
    ) -> f64 {
        // daieffects.c evaluates each active effect separately using the
        // candidate city's happiness, with its requirement range supplying c.
        let count = self.cities_owned_by(city.owner).count() as i32;
        let government = self.players[city.owner].government;
        let owns = |wonder| self.has_active_wonder(city.owner, wonder);
        let has = |other| city.has_building(other);
        let mut effects: Vec<(i32, i32, bool, i32)> = Vec::new();
        let mut content = |amount, cities| effects.push((amount, cities, false, 1));
        match building {
            Building::TEMPLE => {
                content(1, 1);
                if government != Government::Communism && self.has_tech(city.owner, Tech::MYSTICISM)
                {
                    content(1, 1);
                }
                if owns(Building::TEMPLE_OF_ARTEMIS) {
                    content(2, 1);
                }
            }
            Building::CATHEDRAL => {
                content(3, 1);
                if government != Government::Communism && self.has_tech(city.owner, Tech::THEOLOGY)
                {
                    content(1, 1);
                }
                if owns(Building::MICHELANGELOS_CHAPEL) {
                    content(3, 1);
                }
            }
            Building::AMPHITHEATER => {
                content(3, 1);
                if self.has_tech(city.owner, Tech::ELECTRICITY) {
                    content(1, 1);
                }
            }
            Building::COURTHOUSE => {
                content(1, 1);
                if owns(Building::MAUSOLEUM_OF_MAUSOLOS) {
                    content(1, 1);
                }
            }
            Building::CITY_WALLS if owns(Building::MAUSOLEUM_OF_MAUSOLOS) => content(1, 1),
            Building::POLICE_STATION => content(2, 1),
            Building::HANGING_GARDENS => content(1, count),
            Building::TEMPLE_OF_ARTEMIS if has(Building::TEMPLE) => content(2, count),
            Building::MICHELANGELOS_CHAPEL if has(Building::CATHEDRAL) => content(3, count),
            Building::MAUSOLEUM_OF_MAUSOLOS => {
                if has(Building::CITY_WALLS) {
                    content(1, count);
                }
                if has(Building::COURTHOUSE) {
                    content(1, count);
                }
            }
            _ => {}
        }
        if building == Building::POLICE_STATION {
            effects.push((1, 1, true, 1));
            if government == Government::Democracy && !owns(Building::WOMENS_SUFFRAGE) {
                effects.push((1, 1, true, 1));
            }
        }
        if matches!(
            building,
            Building::STATUE_OF_ZEUS | Building::KING_RICHARDS_CRUSADE | Building::WOMENS_SUFFRAGE
        ) {
            effects.push((1, count, true, 1));
            if building == Building::WOMENS_SUFFRAGE
                && government == Government::Democracy
                && has(Building::POLICE_STATION)
            {
                effects.push((1, count, true, -1));
            }
        }
        if effects.is_empty() {
            return 0.0;
        }
        let (_, final_unhappy, _) = self.city_feelings(city, luxury);
        let (_, before_military, _) = self.city_feelings(city, luxury);
        let max_converted = final_unhappy.min(before_military);
        let entertainers = city.specialists[2];
        let supported = self
            .units
            .iter()
            .filter(|u| u.owner == city.owner && u.home_city == Some(city.id))
            .count() as i32;
        let basis = government.empire_size();
        let mut empire_factor = 2;
        if count > basis {
            if basis > 0 {
                empire_factor += count / basis;
            }
            empire_factor += 2;
        }
        effects
            .into_iter()
            .map(|(amount, c, military, sign)| {
                let value = if military {
                    final_unhappy * amount * supported * 2 + c * (amount + 2).max(1)
                } else {
                    amount.min(max_converted + entertainers) * 35
                        + if c > 1 { empire_factor * c * amount } else { 0 }
                };
                f64::from(sign * value)
            })
            .sum()
    }

    pub(super) fn building_health_want(&self, city: &City, building: Building) -> f64 {
        self.building_health_want_with_shields(city, building, self.city_forecast(city).shields)
    }

    fn building_health_want_with_shields(
        &self,
        city: &City,
        building: Building,
        shields: i32,
    ) -> f64 {
        let amount = match building {
            Building::AQUEDUCT | Building::SEWER_SYSTEM => 30,
            Building::AQUEDUCT_RIVER if !city.has_building(Building::AQUEDUCT) => 30,
            Building::AQUEDUCT_LAKE
                if !city.has_building(Building::AQUEDUCT)
                    && !city.has_building(Building::AQUEDUCT_RIVER) =>
            {
                30
            }
            _ => 0,
        };
        if amount == 0 {
            return 0.0;
        }
        f64::from(5 + (amount / 5) * self.city_illness(city, shields))
    }

    pub(super) fn defense_effect_context(&self, city: &City) -> DefenseEffectContext {
        // advdata.c labels land AND ocean components, then records hostile
        // cities on continents and offensive sea-capable units on oceans.
        let mut regions = vec![0_i32; self.tiles.len()];
        let (mut lands, mut oceans) = (0, 0);
        for start in 0..self.tiles.len() {
            if regions[start] != 0 {
                continue;
            }
            let water = self.tiles[start].terrain.is_water();
            let label = if water {
                oceans += 1;
                -oceans
            } else {
                lands += 1;
                lands
            };
            regions[start] = label;
            let mut pending = vec![self.map_wrap.position(start)];
            while let Some((x, y)) = pending.pop() {
                for (nx, ny) in self.map_wrap.civ3_neighbors(x, y) {
                    let index = self.map_wrap.index(nx, ny);
                    if regions[index] == 0 && self.tiles[index].terrain.is_water() == water {
                        regions[index] = label;
                        pending.push((nx, ny));
                    }
                }
            }
        }
        let continent = regions[self.map_wrap.index(city.x, city.y)];
        let mut threatened_oceans = std::collections::BTreeSet::new();
        let continent_threat = continent > 0
            && self.cities.iter().any(|other| {
                self.advisor_player_dangerous(city.owner, other.owner)
                    && regions[self.map_wrap.index(other.x, other.y)] == continent
            });
        let mut invasions = false;
        for unit in self
            .units
            .iter()
            .filter(|unit| self.advisor_player_dangerous(city.owner, unit.owner))
        {
            // advruleset derives move domains from native terrain. Air,
            // Helicopter and Missile are native on both land and ocean.
            if !unit.kind.is_naval() && !unit.kind.is_aircraft() {
                continue;
            }
            if unit.kind.can_occupy_city()
                || unit.kind.can_carry(UnitKind::Warrior)
                || unit.kind.can_carry(UnitKind::Guerilla)
            {
                invasions = true;
            }
            if unit.kind.stats().attack > 1 {
                let label = regions[self.map_wrap.index(unit.x, unit.y)];
                if label < 0 {
                    threatened_oceans.insert(label);
                } else {
                    for (x, y) in self.map_wrap.civ3_neighbors(unit.x, unit.y) {
                        let label = regions[self.map_wrap.index(x, y)];
                        if label < 0 {
                            threatened_oceans.insert(label);
                        }
                    }
                }
            }
        }
        let coastal = self
            .map_wrap
            .civ3_neighbors(city.x, city.y)
            .any(|(x, y)| regions[self.map_wrap.index(x, y)] < 0);
        let ocean_threat = if continent < 0 {
            threatened_oceans.contains(&continent)
        } else {
            self.map_wrap
                .civ3_neighbors(city.x, city.y)
                .any(|(x, y)| threatened_oceans.contains(&regions[self.map_wrap.index(x, y)]))
        };
        DefenseEffectContext {
            capital: city.has_building(Building::PALACE)
                || city.has_building(Building::ECCLESIASTICAL_PALACE),
            ocean_city: continent < 0,
            coastal,
            continent_threat,
            ocean_threat,
            invasions,
        }
    }

    pub(super) fn native_defense_effect_value(
        context: &DefenseEffectContext,
        land: bool,
        sea: bool,
    ) -> f32 {
        // Civ2Civ3's building rows each supply amount100 and city range c1.
        // Keep integer divisions before each adv_want (float) addition.
        let mut value = 0.0_f32;
        if sea {
            if context.ocean_city {
                value += if context.ocean_threat {
                    100 / 6
                } else {
                    100 / 25
                } as f32;
            } else if context.ocean_threat {
                value += (100 / 6) as f32;
            }
        }
        value += (100 / 25 + i32::from(context.invasions) - 1) as f32;
        if (context.capital || land)
            && (context.continent_threat || context.capital || context.invasions && context.coastal)
        {
            value += if context.continent_threat {
                100 * 4 / 5
            } else {
                100 / (18 - i32::from(context.capital) * 6)
            } as f32;
        }
        // advruleset.c's igwall is false for every supported type: each
        // fails at least one mutually exclusive positive class-defense row.
        value
    }

    pub(in crate::sim) fn defensive_building_effect_want(
        &self,
        city: &City,
        building: Building,
    ) -> f64 {
        let (land, sea, rows) = match building {
            Building::CITY_WALLS => (true, false, 1),
            Building::COASTAL_DEFENSE => (false, true, 2), // Sea and Trireme
            Building::SAM_BATTERY => (true, true, 2),      // Air and Helicopter
            _ => return 0.0,
        };
        let context = self.defense_effect_context(city);
        let mut value = 0.0_f32;
        for _ in 0..rows {
            value += Self::native_defense_effect_value(&context, land, sea);
        }
        f64::from(value)
    }

    pub(in crate::sim) fn domestic_improvement_want(&self, city: &City, building: Building) -> f64 {
        let forecast = self.city_forecast(city);
        let base_output = self.advisor_city_output_from_forecast(city, &forecast);
        self.domestic_improvement_want_with_forecast(city, building, &forecast, base_output)
    }

    pub(super) fn domestic_improvement_want_with_forecast(
        &self,
        city: &City,
        building: Building,
        forecast: &crate::sim::economy::CityForecast,
        base_output: f64,
    ) -> f64 {
        let mut improved = city.clone();
        improved.buildings.push(building);
        // daicity.c: base_want measures weighted output with the building added.
        // Happiness is valued separately, rather than valuing all disorder losses.
        let mut want = self.advisor_city_output_exact(&improved, None) - base_output
            + f64::from(self.building_growth_value(
                city,
                &improved,
                forecast.food - city.size * Self::citizen_food_cost() - forecast.support.food,
            ))
            + self.building_happiness_want_with_luxury(city, building, forecast.luxury)
            + self.building_health_want_with_shields(city, building, forecast.shields)
            + self.defensive_building_effect_want(city, building);
        // daieffects.c values veteran construction as (3*c + affected)
        // and full healing as (5*c + affected), once per class effect.
        want += f64::from(match building {
            Building::BARRACKS | Building::BARRACKS_II | Building::BARRACKS_III => {
                let land = self
                    .units_owned_by(city.owner)
                    .filter(|unit| !unit.kind.is_naval() && !unit.kind.is_aircraft())
                    .count() as i32;
                8 + 2 * land
            }
            // Sea and Trireme each have distinct veteran/healing effects.
            Building::PORT_FACILITY => {
                let naval = self
                    .units_owned_by(city.owner)
                    .filter(|unit| unit.kind.is_naval())
                    .count() as i32;
                16 + 2 * naval
            }
            // Air, Helicopter, Missile veteran; Air/Helicopter healing; airlift.
            Building::AIRPORT => {
                let mut land = 0;
                let mut aircraft = 0;
                let mut missiles = 0;
                for unit in self.units_owned_by(city.owner) {
                    land += i32::from(!unit.kind.is_naval() && !unit.kind.is_aircraft());
                    aircraft += i32::from(unit.kind.is_aircraft());
                    missiles += i32::from(matches!(
                        unit.kind,
                        UnitKind::CruiseMissile | UnitKind::TacticalNuke | UnitKind::ICBM
                    ));
                }
                19 + 2 * aircraft - missiles + 1 + land.min(13)
            }
            _ => 0,
        });
        self.adjust_building_want_with_surplus(city, building, want, forecast.shield_surplus())
    }

    pub(super) fn adjust_building_want(&self, city: &City, building: Building, want: f64) -> f64 {
        // daicity.c: cost, switching loss, and impending obsolescence.
        let surplus = self.city_forecast(city).shield_surplus();
        self.adjust_building_want_with_surplus(city, building, want, surplus)
    }

    fn adjust_building_want_with_surplus(
        &self,
        city: &City,
        building: Building,
        mut want: f64,
        surplus: i32,
    ) -> f64 {
        let surplus = surplus.max(0);
        want -= f64::from(building.data().cost / (surplus * 10 + 1));
        if city.production.class() != Production::Building(building).class() {
            want -= f64::from(city.shields * SHIELD_WEIGHT) / 15.0;
        }
        for &(requirement, present) in building.data().obsolete {
            if let (Requirement::Tech(index), true) = (requirement, present) {
                let remaining = self.research_path(city.owner, Tech(index)).len().max(1);
                want -= want / remaining as f64;
            }
        }
        want
    }

    pub(super) fn projected_wonder_want(
        &self,
        scratch: &mut Self,
        city: &City,
        building: Building,
    ) -> f64 {
        let owned: Vec<_> = self.cities_owned_by(city.owner).collect();
        let baseline_outputs: Vec<_> = owned
            .iter()
            .map(|owned_city| self.advisor_city_output_exact(owned_city, None))
            .collect();
        self.projected_wonder_want_with_baselines(
            scratch,
            city,
            building,
            &owned,
            &baseline_outputs,
        )
    }

    pub(super) fn projected_wonder_want_with_baselines(
        &self,
        scratch: &mut Self,
        city: &City,
        building: Building,
        owned: &[&City],
        baseline_outputs: &[f64],
    ) -> f64 {
        let government_want = if building == Building::STATUE_OF_LIBERTY {
            scratch
                .government_research_want(city.owner)
                .map(|(tech, value)| {
                    value
                        .min(65)
                        .min(self.research_path(city.owner, tech).len() as i32 * 10)
                })
                .unwrap_or(0)
        } else {
            0
        };
        let index = scratch
            .cities
            .iter()
            .position(|candidate| candidate.id == city.id)
            .unwrap();
        scratch.cities[index].buildings.push(building);
        if building.data().genus == Genus::GreatWonder {
            scratch.completed_wonders.push(building);
        }
        let count = owned.len() as i32;
        // daicity.c:base_want adds the building, recomputes outputs in its
        // entire effect range, and restores it. This scratch preserves live
        // units, cities, technology, completed world effects, and RNG state.
        let mut want = 0.0;
        for (original, &baseline) in owned.iter().zip(baseline_outputs) {
            let projected = scratch
                .cities
                .iter()
                .find(|candidate| candidate.id == original.id)
                .unwrap();
            want += scratch.advisor_city_output_exact(projected, None) - baseline;
        }
        want += self.building_happiness_want(city, building);
        let owned_units = || self.units_owned_by(city.owner);
        let land = owned_units()
            .filter(|unit| !unit.kind.is_aircraft() && !unit.kind.is_naval())
            .count() as i32;
        want += f64::from(match building {
            Building::GREAT_LIBRARY | Building::ISAAC_NEWTONS_COLLEGE | Building::INTERNET
                if self.players[city.owner].researching != Tech::FUTURE =>
            {
                101
            }
            Building::SUN_TZUS_WAR_ACADEMY => 3 * count + land,
            Building::MANHATTAN_PROJECT if !self.manhattan_project_completed() => {
                20 + 5 * owned_units()
                    .filter(|unit| unit.kind == UnitKind::CruiseMissile)
                    .count() as i32
            }
            Building::MARCO_POLOS_EMBASSY => 2 * self.players.len() as i32,
            Building::STATUE_OF_LIBERTY => government_want,
            Building::LEONARDOS_WORKSHOP => {
                2 * owned_units()
                    .filter(|unit| self.upgrade_target(unit).is_some())
                    .count() as i32
            }
            Building::LIGHTHOUSE => {
                // Two class-specific veteran effects: Sea and Trireme.
                6 * count + owned_units().filter(|unit| unit.kind.is_naval()).count() as i32
            }
            Building::GREAT_WALL => {
                let labels = self.advisor_continent_labels();
                let continent = labels[self.map_wrap.index(city.x, city.y)];
                let threat = self.units.iter().any(|unit| {
                    unit.kind.is_military()
                        && !unit.kind.is_naval()
                        && !unit.kind.is_aircraft()
                        && self.ai_potentially_hostile(city.owner, unit.owner)
                        && labels[self.map_wrap.index(unit.x, unit.y)] == continent
                });
                // daieffects.c Defend_Bonus50: ranged base plus continental
                // threat bonus; Unit_No_Lose_Pop adds two per local unit.
                count
                    + if threat { 40 } else { 0 }
                    + 2 * owned_units()
                        .filter(|unit| (unit.x, unit.y) == (city.x, city.y))
                        .count() as i32
            }
            Building::CURE_FOR_CANCER => {
                5 * count + 2 * self.city_illness(city, self.city_forecast(city).shields)
            }
            _ => 0,
        });
        if building == Building::APOLLO_PROGRAM {
            want += self.space_production_want(city, building);
        }
        // Reference Move_Bonus uses the number of affected units. Compare the
        // actual rules-driven allowances instead of assigning a fixed ship bonus.
        want += owned_units()
            .filter(|unit| scratch.unit_move_rate(unit) > self.unit_move_rate(unit))
            .count() as f64;
        // Remove the exact hypothetical addition; no completion callbacks or
        // random draws run while evaluating an option.
        scratch.cities[index].buildings.pop();
        if building.data().genus == Genus::GreatWonder {
            scratch.completed_wonders.pop();
        }
        want = self.adjust_building_want(city, building, want);
        if building.data().genus == Genus::GreatWonder {
            want *= 1.5; // deny competitors the great wonder
            if matches!(city.production, Production::Building(current) if current.data().genus == Genus::GreatWonder)
            {
                want *= 1.25; // prefer continuing a great-wonder project
            }
        }
        want
    }
}
