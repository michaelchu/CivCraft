use super::unit_catalog::{UnitClass, UnitFlag, UnitRole};
use super::*;
use crate::rules::{Genus, Requirement};

const FOOD_WEIGHT: i32 = 30;
const SHIELD_WEIGHT: i32 = 17;
const TRADE_WEIGHT: i32 = 18;
const ADVISOR_POWER_DIVIDER: i32 = POWER_FACTOR * 3;

const SIMPLE_MILITARY: &[UnitKind] = &UnitKind::SIMPLE_MILITARY;

#[derive(Clone, Copy, Debug)]
pub(super) struct AiBuildChoice {
    pub(super) production: Production,
    pub(super) want: f64,
}

const DOMESTIC_WANT_MAX: f64 = 750.075;
const MILITARY_EMERGENCY_WANT: f64 = 1000.1;

#[derive(Clone, Copy)]
pub(super) struct GuardCityAssessment {
    pub(super) danger: i32,
    pub(super) urgency: i32,
    pub(super) grave: i32,
    pub(super) defense: i32,
    pub(super) defense_igwall: i32,
}

struct DefenseEffectContext {
    capital: bool,
    ocean_city: bool,
    coastal: bool,
    continent_threat: bool,
    ocean_threat: bool,
    invasions: bool,
}

impl Simulation {
    pub(super) fn advisor_tech_want(&self, owner: usize, tech: Tech) -> f32 {
        if !self.players[owner].ai {
            return 0.0;
        }
        self.ai_tech_wants
            .get(&owner)
            .and_then(|wants| wants.get(tech.0))
            .copied()
            .unwrap_or(0.0)
    }

    pub(super) fn clear_ai_tech_wants(&mut self, owner: usize) {
        self.ai_tech_wants
            .insert(owner, vec![0.0; Tech::all().count()]);
    }

    pub(super) fn add_ai_tech_want(&mut self, owner: usize, tech: Tech, want: f32) {
        let wants = self
            .ai_tech_wants
            .entry(owner)
            .or_insert_with(|| vec![0.0; Tech::all().count()]);
        if let Some(value) = wants.get_mut(tech.0) {
            *value += want;
        }
    }

    pub(super) fn record_ai_domestic_tech_wants(&mut self, owner: usize) {
        for tech in Tech::all() {
            if self.has_tech(owner, tech) {
                continue;
            }
            self.add_ai_tech_want(owner, tech, self.ferry_tech_want(owner, tech) as f32);
            if tech == Tech::ALPHABET {
                self.add_ai_tech_want(
                    owner,
                    tech,
                    self.counterintelligence_tech_want(owner) as f32,
                );
            }
        }
        // Apply every contribution to the existing phase accumulator; grouping
        // cities first would change f32 rounding after government feedback.
        let cities: Vec<_> = self.player_cities_ordered(owner).cloned().collect();
        for city in cities {
            for tech in Tech::all() {
                if !self.has_tech(owner, tech) {
                    self.add_ai_tech_want(
                        owner,
                        tech,
                        self.city_building_tech_want(&city, tech) as f32,
                    );
                }
            }
        }
    }

    pub(super) fn record_ai_first_activity_tech_wants(&mut self, owner: usize) {
        if !self.players[owner].ai {
            return;
        }
        let cities: Vec<_> = self.player_cities_ordered(owner).cloned().collect();
        for city in cities {
            for (tech, want) in self.counter_defender_tech_feedback(&city) {
                self.add_ai_tech_want(owner, tech, want);
            }
        }
    }

    fn record_ai_military_tech_wants(&mut self, city: &City) {
        for (tech, want) in self.counter_defender_tech_feedback(city) {
            self.add_ai_tech_want(city.owner, tech, want);
        }
        for tech in Tech::all() {
            self.add_ai_tech_want(
                city.owner,
                tech,
                self.production_defender_tech_want(city, tech),
            );
        }
        for (tech, want) in self.offensive_advice(city).1 {
            self.add_ai_tech_want(city.owner, tech, want);
        }
    }

    /// Explicit preview for isolated advisor scenarios. Runtime research and
    /// diplomacy read the retained phase vector instead of recomputing advice.
    pub(super) fn advisor_tech_wants(&self, owner: usize) -> Vec<f32> {
        let mut preview = self.clone();
        preview.clear_ai_tech_wants(owner);
        if let Some((tech, want)) = preview.government_research_want(owner) {
            preview.add_ai_tech_want(owner, tech, want as f32);
        }
        preview.record_ai_domestic_tech_wants(owner);
        let cities: Vec<_> = preview.player_cities_ordered(owner).cloned().collect();
        for city in cities {
            preview.record_ai_military_tech_wants(&city);
        }
        preview.record_ai_tech_effect_wants(owner);
        preview.ai_tech_wants.remove(&owner).unwrap()
    }

    pub(super) fn advisor_city_output(&self, city: &City, granted: Option<Tech>) -> i32 {
        self.advisor_city_output_exact(city, granted) as i32
    }

    pub(super) fn advisor_city_output_exact(&self, city: &City, granted: Option<Tech>) -> f64 {
        let forecast = self.city_forecast_with_tech(city, granted);
        // advdata.h stores priorities as int: 18*1.2 truncates to21.
        let (science_weight, luxury_weight) =
            if self.players[city.owner].researching != Tech::FUTURE {
                (21, 1)
            } else {
                (1, TRADE_WEIGHT)
            };
        // daicity.c:dai_city_want stores adv_want after each compound
        // assignment, so retain source f32 rounding at those boundaries.
        let mut value = 0.0_f32;
        value += (FOOD_WEIGHT * forecast.food) as f32;
        if forecast.shields != 0 {
            value += (SHIELD_WEIGHT * forecast.shields) as f32;
            value -= (self.pollution_risk(city, forecast.shields) * 20) as f32;
        }
        value += (luxury_weight * forecast.luxury) as f32;
        value += (science_weight * forecast.gross_science) as f32;
        let gold = forecast.gross_gold - forecast.building_upkeep;
        let tax = self.players[city.owner].rates[1];
        if tax > 50 {
            value = (f64::from(value) + f64::from(gold * TRADE_WEIGHT * (tax - 40)) / 14.0) as f32;
        } else {
            value += (gold * TRADE_WEIGHT) as f32;
        }
        f64::from(value)
    }

    fn city_tech_effect_want(&self, city: &City, tech: Tech) -> i32 {
        let baseline = self.advisor_city_output(city, None);
        let owner = city.owner;
        let direct = match tech {
            Tech::RAILROAD | Tech::TRADE | Tech::THE_CORPORATION => {
                self.advisor_city_output(city, Some(tech)) - baseline
            }
            Tech::MYSTICISM
                if city.has_building(Building::TEMPLE)
                    && self.players[owner].government != Government::Communism =>
            {
                TRADE_WEIGHT
            }
            _ => 0,
        };
        let mut want = direct * 14 / 8;
        for &building in &city.buildings {
            if building
                .data()
                .obsolete
                .contains(&(Requirement::Tech(tech.0), true))
            {
                let mut obsolete = city.clone();
                obsolete.buildings.retain(|&built| built != building);
                want += (self.advisor_city_output(&obsolete, Some(tech))
                    - self.advisor_city_output(city, Some(tech)))
                    * 14
                    / 8;
            }
        }
        want
    }

    pub(super) fn record_ai_tech_effect_wants(&mut self, owner: usize) {
        let cities: Vec<_> = self.player_cities_ordered(owner).cloned().collect();
        // daitech.c: effect feedback is the final contribution before selection.
        for tech in Tech::all() {
            if self.has_tech(owner, tech) {
                continue;
            }
            for city in &cities {
                self.add_ai_tech_want(owner, tech, self.city_tech_effect_want(city, tech) as f32);
            }
        }
    }

    fn city_building_tech_want(&self, city: &City, tech: Tech) -> i32 {
        let baseline = self.advisor_city_output(city, None);
        let (food, gross_shields, _) = self.city_yields(city);
        let upkeep = self.city_unit_upkeep(city);
        let food_surplus = food - city.size * CITIZEN_FOOD_COST - upkeep.food;
        let shields = (gross_shields - upkeep.shields).max(0);
        let mut want = 0;
        for building in
            Building::all().filter(|building| building.data().genus == Genus::Improvement)
        {
            let data = building.data();
            if !data
                .requirements
                .contains(&(Requirement::Tech(tech.0), true))
                || self
                    .building_blocker_with_tech(city, building, Some(tech))
                    .is_some()
            {
                continue;
            }
            let mut improved = city.clone();
            improved.buildings.push(building);
            let mut benefit = self.advisor_city_output(&improved, None) - baseline;
            benefit += self.building_growth_value(city, &improved, food_surplus);
            benefit += self.defensive_building_effect_want(city, building) as i32;
            let cost = data.cost / (shields * 10 + 1);
            want += (benefit - cost).max(0) * 14 / 8;
        }
        want
    }

    fn building_growth_value(&self, city: &City, improved: &City, food_surplus: i32) -> i32 {
        let mut benefit = 0;
        // Growth headroom matters when a city is about to reach its current limit.
        let limit = Self::city_size_limit(city);
        let new_limit = Self::city_size_limit(improved);
        if new_limit > limit && limit != i32::MAX {
            let increase = (new_limit - limit).min(8);
            benefit += increase * 4 / limit;
            if city.size >= limit {
                let stored = if city.food == Self::growth_box(city.size) {
                    city.food - Self::growth_box(city.size - 1)
                } else {
                    0
                };
                benefit += (food_surplus + stored) * (FOOD_WEIGHT * increase + 30);
            }
        }
        let retention = Self::food_retained(improved) - Self::food_retained(city);
        if retention > 0 {
            benefit +=
                8 + retention / 10 * food_surplus.max(0) + retention / 15 * (-food_surplus).max(1);
        }
        benefit
    }

    fn building_happiness_want(&self, city: &City, building: Building) -> f64 {
        // daieffects.c evaluates each active effect separately using the
        // candidate city's happiness, with its requirement range supplying c.
        let count = self.cities.iter().filter(|c| c.owner == city.owner).count() as i32;
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
        let luxury = self.city_forecast(city).luxury;
        let (_, final_unhappy, _) = self.city_feelings(city, luxury);
        let (_, before_military, _) = self.city_feelings_before_military(city, luxury);
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

    fn building_health_want(&self, city: &City, building: Building) -> f64 {
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
        f64::from(5 + (amount / 5) * self.city_illness(city, self.city_forecast(city).shields))
    }

    fn defense_effect_context(&self, city: &City) -> DefenseEffectContext {
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
            let mut pending = vec![Self::xy(start)];
            while let Some((x, y)) = pending.pop() {
                for (nx, ny) in self.map_wrap.neighbors(x, y) {
                    let index = Self::idx(nx, ny);
                    if regions[index] == 0 && self.tiles[index].terrain.is_water() == water {
                        regions[index] = label;
                        pending.push((nx, ny));
                    }
                }
            }
        }
        let continent = regions[Self::idx(city.x, city.y)];
        let mut threatened_oceans = std::collections::BTreeSet::new();
        let continent_threat = continent > 0
            && self.cities.iter().any(|other| {
                self.advisor_player_dangerous(city.owner, other.owner)
                    && regions[Self::idx(other.x, other.y)] == continent
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
                || unit.kind.can_carry(UnitKind::Partisan)
            {
                invasions = true;
            }
            if unit.kind.stats().attack > 1 {
                let label = regions[Self::idx(unit.x, unit.y)];
                if label < 0 {
                    threatened_oceans.insert(label);
                } else {
                    for (x, y) in self.map_wrap.neighbors(unit.x, unit.y) {
                        let label = regions[Self::idx(x, y)];
                        if label < 0 {
                            threatened_oceans.insert(label);
                        }
                    }
                }
            }
        }
        let coastal = self
            .map_wrap
            .neighbors(city.x, city.y)
            .any(|(x, y)| regions[Self::idx(x, y)] < 0);
        let ocean_threat = if continent < 0 {
            threatened_oceans.contains(&continent)
        } else {
            self.map_wrap
                .neighbors(city.x, city.y)
                .any(|(x, y)| threatened_oceans.contains(&regions[Self::idx(x, y)]))
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

    fn native_defense_effect_value(context: &DefenseEffectContext, land: bool, sea: bool) -> f32 {
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

    pub(super) fn defensive_building_effect_want(&self, city: &City, building: Building) -> f64 {
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

    fn domestic_improvement_want(&self, city: &City, building: Building) -> f64 {
        let forecast = self.city_forecast(city);
        let mut improved = city.clone();
        improved.buildings.push(building);
        // daicity.c: base_want measures weighted output with the building added.
        // Happiness is valued separately, rather than valuing all disorder losses.
        let mut want = self.advisor_city_output_exact(&improved, None)
            - self.advisor_city_output_exact(city, None)
            + f64::from(self.building_growth_value(
                city,
                &improved,
                forecast.food - city.size * CITIZEN_FOOD_COST - forecast.support.food,
            ))
            + self.building_happiness_want(city, building)
            + self.building_health_want(city, building)
            + self.defensive_building_effect_want(city, building);
        // daieffects.c values veteran construction as (3*c + affected)
        // and full healing as (5*c + affected), once per class effect.
        let own_units = || self.units.iter().filter(|unit| unit.owner == city.owner);
        let land = own_units()
            .filter(|unit| !unit.kind.is_naval() && !unit.kind.is_aircraft())
            .count() as i32;
        let naval = own_units().filter(|unit| unit.kind.is_naval()).count() as i32;
        let aircraft = own_units().filter(|unit| unit.kind.is_aircraft()).count() as i32;
        want += f64::from(match building {
            Building::BARRACKS | Building::BARRACKS_II | Building::BARRACKS_III => 8 + 2 * land,
            // Sea and Trireme each have distinct veteran/healing effects.
            Building::PORT_FACILITY => 16 + 2 * naval,
            // Air, Helicopter, Missile veteran; Air/Helicopter healing; airlift.
            Building::AIRPORT => {
                let missiles = own_units()
                    .filter(|unit| matches!(unit.kind, UnitKind::CruiseMissile | UnitKind::Nuclear))
                    .count() as i32;
                19 + 2 * aircraft - missiles + 1 + land.min(13)
            }
            _ => 0,
        });
        if building != Building::ECCLESIASTICAL_PALACE || !city.has_building(Building::PALACE) {
            want += f64::from(building.history_per_turn() * 5);
        }
        self.adjust_building_want(city, building, want)
    }

    fn adjust_building_want(&self, city: &City, building: Building, mut want: f64) -> f64 {
        // daicity.c: cost, switching loss, and impending obsolescence.
        let surplus = self.city_forecast(city).shield_surplus().max(0);
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

    fn projected_wonder_want(&self, scratch: &mut Self, city: &City, building: Building) -> f64 {
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
        let owned: Vec<_> = self
            .cities
            .iter()
            .filter(|candidate| candidate.owner == city.owner)
            .collect();
        let count = owned.len() as i32;
        // daicity.c:base_want adds the building, recomputes outputs in its
        // entire effect range, and restores it. This scratch preserves live
        // units, cities, technology, completed world effects, and RNG state.
        let mut want = 0.0;
        for original in &owned {
            let projected = scratch
                .cities
                .iter()
                .find(|candidate| candidate.id == original.id)
                .unwrap();
            want += scratch.advisor_city_output_exact(projected, None)
                - self.advisor_city_output_exact(original, None);
        }
        want += self.building_happiness_want(city, building);
        if building != Building::ECCLESIASTICAL_PALACE || !city.has_building(Building::PALACE) {
            want += f64::from(building.history_per_turn() * 5);
        }
        let owned_units = || self.units.iter().filter(|unit| unit.owner == city.owner);
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
                let labels = self.continent_labels();
                let continent = labels[Self::idx(city.x, city.y)];
                let threat = self.units.iter().any(|unit| {
                    unit.kind.is_military()
                        && !unit.kind.is_naval()
                        && !unit.kind.is_aircraft()
                        && self.ai_potentially_hostile(city.owner, unit.owner)
                        && labels[Self::idx(unit.x, unit.y)] == continent
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
            .filter(|unit| scratch.unit_move_rate(&unit) > self.unit_move_rate(&unit))
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

    pub(crate) fn ai_happiness_effects_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.completed_wonders.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
        }
        self.create_city(1, 10, 24);
        self.cities[0].size = 5;
        self.players[1].government = Government::Monarchy;
        self.players[1].known.push(Tech::MYSTICISM);
        let independent = self.building_happiness_want(&self.cities[0], Building::TEMPLE) == 70.0;
        self.cities[0].size = 4;
        self.players[1].government = Government::Republic;
        let id = self.cities[0].id;
        let field = offset_native_by_map(10, 24, 8, 0);
        for _ in 0..4 {
            self.spawn_unit(1, UnitKind::Warrior, field.0, field.1);
            self.units.last_mut().unwrap().home_city = Some(id);
        }
        let wartime = self.building_happiness_want(&self.cities[0], Building::TEMPLE) == 0.0
            && self.building_happiness_want(&self.cities[0], Building::STATUE_OF_ZEUS) == 27.0
            && self.building_happiness_want(&self.cities[0], Building::POLICE_STATION) == 27.0;
        self.players[1].government = Government::Democracy;
        self.cities[0].buildings.push(Building::POLICE_STATION);
        let removal =
            self.building_happiness_want(&self.cities[0], Building::WOMENS_SUFFRAGE) == 0.0;
        self.units.clear();
        self.cities[0].buildings.clear();
        self.cities[0].size = 1;
        self.players[1].government = Government::Monarchy;
        for step in [4, 8] {
            let (x, y) = offset_native_by_map(10, 24, step, 0);
            self.create_city(1, x, y);
            self.cities.last_mut().unwrap().size = 10;
        }
        let ranged = self.building_happiness_want(&self.cities[0], Building::HANGING_GARDENS)
            == 6.0
            && self.building_happiness_want(&self.cities[0], Building::MICHELANGELOS_CHAPEL) == 0.0;
        independent && wartime && removal && ranged
    }

    pub(crate) fn ai_wonder_advisor_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.completed_wonders.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.road = true;
            tile.river = true;
            tile.hut = false;
            tile.known_by[1] = true;
        }
        self.players[1].known = self.research_path(1, Tech(41) /* Literacy */);
        self.players[1].known.extend([Tech::TRADE, Tech::POTTERY]);
        self.players[1].government = Government::Monarchy;
        self.players[1].rates = [80, 20, 0];
        for (dx, dy) in [(0, 0), (3, 0), (6, 0), (0, 3), (3, 3), (6, 3)] {
            let (x, y) = offset_native_by_map(10, 24, dx, dy);
            self.create_city(1, x, y);
            let city = self.cities.last_mut().unwrap();
            city.size = 5;
            city.buildings.extend([Building::LIBRARY, Building::TEMPLE]);
            self.spawn_unit(1, UnitKind::Warrior, x, y);
        }
        self.reconcile_city_workers();
        let Some(id) = self.ai_wonder_city(1) else {
            return false;
        };
        let city = self
            .cities
            .iter()
            .find(|city| city.id == id)
            .unwrap()
            .clone();
        let Some(choice) = self.wonder_build_choice(&city) else {
            return false;
        };
        let Production::Building(wonder) = choice.production else {
            return false;
        };
        let chosen = self.choose_ai_production(1, id) == choice.production;
        self.change_production(id, choice.production);
        let helper = self
            .cities
            .iter()
            .find(|city| city.id != id)
            .unwrap()
            .clone();
        let requested = self.wonder_helper_production_choice(&helper).is_some();
        // Existing helpers cover all remaining shields: no more should queue.
        for _ in 0..((wonder.data().cost + 49) / 50) {
            self.spawn_unit(1, UnitKind::Caravan, helper.x, helper.y);
        }
        let covered = self.wonder_helper_production_choice(&helper).is_none();
        chosen && requested && covered
    }

    fn wonder_helper_production_choice(&self, city: &City) -> Option<(UnitKind, f64)> {
        let wonder_id = self.ai_wonder_city(city.owner)?;
        if wonder_id == city.id {
            return None;
        }
        let wonder_city = self
            .cities
            .iter()
            .find(|candidate| candidate.id == wonder_id)?;
        let Production::Building(wonder) = wonder_city.production else {
            return None;
        };
        // Civ2Civ3 ACTION_HELP_WONDER explicitly targets GreatWonder;
        // the generic advisor's is_wonder check alone is insufficient.
        if wonder.data().genus != Genus::GreatWonder
            || self.building_blocker(wonder_city, wonder).is_some()
        {
            return None;
        }
        let kind = [UnitKind::Freight, UnitKind::Caravan]
            .into_iter()
            .find(|&kind| self.unit_buildable(city, kind))?;
        let helper = self.make_unit(0, city.owner, kind, wonder_city.x, wonder_city.y);
        let allowance = self.unit_move_allowance(city.owner, kind);
        let distance = self.route_cost_within(&helper, city.x, city.y, allowance * 7)?;
        if distance <= 0 {
            return None;
        }
        let labels = self.continent_labels();
        let continent = labels[Self::idx(city.x, city.y)];
        let existing = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == city.owner
                    && matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight)
                    && labels[Self::idx(unit.x, unit.y)] == continent
            })
            .count();
        let building = self
            .cities
            .iter()
            .filter(|other| {
                other.owner == city.owner
                    && matches!(
                        other.production,
                        Production::Unit(UnitKind::Caravan | UnitKind::Freight)
                    )
                    && labels[Self::idx(other.x, other.y)] == continent
            })
            .count();
        if wonder.data().cost - wonder_city.shields
            <= kind.stats().cost * (existing + building) as i32
        {
            return None;
        }
        let mut scratch = self.clone();
        let want = self.projected_wonder_want(&mut scratch, wonder_city, wonder)
            / f64::from((distance / allowance.max(1)).max(1));
        (want > 0.0).then_some((kind, want))
    }

    fn wonder_build_choice(&self, city: &City) -> Option<AiBuildChoice> {
        if self.ai_wonder_city(city.owner) != Some(city.id) {
            return None;
        }
        let candidates: Vec<_> = Building::all()
            .filter(|building| {
                matches!(
                    building.data().genus,
                    Genus::GreatWonder | Genus::SmallWonder
                ) && self.building_blocker(city, *building).is_none()
                    && !(building.data().genus == Genus::SmallWonder
                        && self.cities.iter().any(|other| {
                            other.owner == city.owner && other.has_building(*building)
                        }))
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let mut scratch = self.clone();
        candidates
            .into_iter()
            .filter_map(|building| {
                let want = self.projected_wonder_want(&mut scratch, city, building);
                (want > 0.0).then_some(AiBuildChoice {
                    production: Production::Building(building),
                    want: want.min(DOMESTIC_WANT_MAX),
                })
            })
            .max_by(|a, b| a.want.total_cmp(&b.want))
    }

    fn ai_wonder_city(&self, owner: usize) -> Option<u32> {
        let owned: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .collect();
        // advbuilding.c keeps a productive, legal wonder already under way.
        if let Some(city) = owned.iter().find(|city| {
            self.city_forecast(city).shield_surplus() > 0
                && matches!(city.production, Production::Building(building)
                    if matches!(building.data().genus, Genus::GreatWonder | Genus::SmallWonder)
                        && self.building_blocker(city, building).is_none())
        }) {
            return Some(city.id);
        }
        // The standard ruleset has wonder helpers, so at least six cities
        // on this continent and four within four helper turns are required.
        if owned.len() < 6 {
            return None;
        }
        let labels = self.continent_labels();
        let helper_kind = if self.unit_available(owner, UnitKind::Freight) {
            UnitKind::Freight
        } else {
            UnitKind::Caravan
        };
        let helper_available = self.unit_available(owner, helper_kind);
        let range = self.unit_move_allowance(owner, helper_kind) * 4;
        let mut best = None;
        let mut best_value = 0;
        for &city in &owned {
            if self
                .city_threat_arrivals(city)
                .any(|(_, _, turns)| turns <= 1)
            {
                continue;
            }
            let continent = labels[Self::idx(city.x, city.y)];
            let count = owned
                .iter()
                .filter(|other| labels[Self::idx(other.x, other.y)] == continent)
                .count() as i32;
            if continent == 0 || count <= 5 {
                continue;
            }
            let helper = self.make_unit(0, owner, helper_kind, city.x, city.y);
            let downtown = owned
                .iter()
                .filter(|other| {
                    self.route_cost_within(&helper, other.x, other.y, range)
                        .is_some()
                })
                .count() as i32;
            if downtown <= 3 {
                continue;
            }
            let mut value = self.city_forecast(city).shield_surplus();
            if self
                .map_wrap
                .neighbors(city.x, city.y)
                .any(|(x, y)| self.tile(x, y).terrain.is_water())
            {
                value /= 2;
            }
            if helper_available {
                value += downtown + count / 8;
            }
            let threatened = self.units.iter().any(|unit| {
                self.ai_potentially_hostile(owner, unit.owner)
                    && unit.kind.is_military()
                    && labels[Self::idx(unit.x, unit.y)] == continent
            });
            if threatened {
                value -= value / 4;
            }
            if value > best_value {
                best = Some(city.id);
                best_value = value;
            }
        }
        best
    }

    fn domestic_build_choice(&self, city: &City) -> Option<AiBuildChoice> {
        let wonder_city = self.ai_wonder_city(city.owner) == Some(city.id);
        Building::all()
            .filter(|building| {
                building.data().genus == Genus::Improvement
                    && self.building_blocker(city, *building).is_none()
            })
            .filter_map(|building| {
                let want = self.domestic_improvement_want(city, building)
                    / if wonder_city { 5.0 } else { 1.0 };
                (want > 0.0).then_some(AiBuildChoice {
                    production: Production::Building(building),
                    want: want.min(DOMESTIC_WANT_MAX),
                })
            })
            .max_by(|a, b| a.want.total_cmp(&b.want))
    }

    fn space_production_want(&self, city: &City, building: Building) -> f64 {
        // advdata.c chooses the first player on tied manufacturing/tech scores.
        let leader = |score: &dyn Fn(usize) -> i32| {
            (0..self.players.len())
                .fold(None, |best: Option<(usize, i32)>, owner| {
                    let value = score(owner);
                    if best.is_none_or(|(_, old)| value > old) {
                        Some((owner, value))
                    } else {
                        best
                    }
                })
                .map(|(owner, _)| owner)
        };
        let manufacturing = leader(&|owner| {
            self.cities
                .iter()
                .filter(|candidate| candidate.owner == owner)
                .map(|candidate| self.city_forecast(candidate).shield_surplus())
                .sum()
        });
        let technology = leader(&|owner| {
            Tech::all()
                .filter(|&tech| self.has_tech(owner, tech))
                .count() as i32
                + self.players[owner].future_techs * 5 / 2
        });
        let competitive = manufacturing == Some(city.owner) || technology == Some(city.owner);
        if building == Building::APOLLO_PROGRAM {
            // daieffects.c EFT_ENABLE_SPACE; unlike parts Apollo gets base 10.
            return if competitive { 160.0 } else { 10.0 };
        }
        let race_started = self.spaceships.iter().enumerate().any(|(owner, ship)| {
            owner != BARBARIANS && self.player_alive(owner) && ship.built != [0; 3]
        });
        if !competitive && !race_started {
            return 0.0;
        }
        let base = if self.ai_wonder_city(city.owner) == Some(city.id) {
            120.0
        } else {
            210.0
        };
        let started = self.spaceships[city.owner].built != [0; 3];
        // daieffects.c lowers space priority in the wonder city, and
        // daicity.c then divides every non-wonder building want by five.
        let want = base * if started { 3.0 } else { 1.0 };
        want / if base == 120.0 { 5.0 } else { 1.0 }
    }

    pub(super) fn city_threats<'a>(
        &'a self,
        city: &'a City,
    ) -> impl Iterator<Item = (&'a Unit, i32)> + 'a {
        self.city_threat_arrivals(city)
            .map(|(unit, rating, _)| (unit, rating))
    }

    fn danger_route_turns(&self, unit: &Unit, city: &City) -> Option<i32> {
        use std::{cmp::Reverse, collections::BinaryHeap};
        // pft_fill_reverse_parameter uses a terrain-only reverse map: no
        // current movement, diplomacy, ZOC, fog or intervening-unit blockers.
        let allowance = self.unit_move_rate(&unit);
        if allowance <= 0 {
            return None;
        }
        let limit = 4 * allowance - 1; // normal assess_danger horizon: turns0..3
        let start = Self::idx(unit.x, unit.y);
        let goal = Self::idx(city.x, city.y);
        let mut distance = vec![i32::MAX; self.tiles.len()];
        let mut queue = BinaryHeap::from([Reverse((0, start))]);
        distance[start] = 0;
        let mut actor = unit.clone();
        actor.moves_left = allowance;
        while let Some(Reverse((spent, index))) = queue.pop() {
            if distance[index] != spent {
                continue;
            }
            if index == goal {
                return Some(spent / allowance);
            }
            (actor.x, actor.y) = Self::xy(index);
            let remaining = allowance - spent % allowance;
            for (x, y) in self.map_wrap.neighbors(actor.x, actor.y) {
                let index = Self::idx(x, y);
                let cost = self.movement_cost_for(&actor, x, y).or_else(|| {
                    // A ship's reverse attack map overlaps the coastal city,
                    // even when the ship cannot enter its land tile normally.
                    (index == goal && unit.kind.is_naval()).then_some(MOVE_FRAGMENTS)
                });
                let Some(cost) = cost else {
                    continue;
                };
                let candidate = spent + cost.min(remaining);
                if candidate <= limit && candidate < distance[index] {
                    distance[index] = candidate;
                    queue.push(Reverse((candidate, index)));
                }
            }
        }
        None
    }

    fn city_danger_arrival(&self, unit: &Unit, city: &City) -> Option<i32> {
        // assess_danger_unit initializes with paradrop range, then compares
        // ordinary travel and the current ferry. Paradrop capability is a
        // future action estimate, not a promise that a drop is legal now.
        let mut turns = (unit.kind == UnitKind::Paratroopers)
            .then(|| self.map_distance(unit.x, unit.y, city.x, city.y) / 10);
        if let Some(walking) = self.danger_route_turns(unit, city)
            && turns.is_none_or(|current| walking < current)
        {
            turns = Some(walking);
        }
        if let Some(ferry) = unit
            .transport
            .and_then(|id| self.units.iter().find(|ship| ship.id == id))
            && let Some(sailing) = self.danger_route_turns(ferry, city)
            && turns.is_none_or(|current| sailing < current)
        {
            turns = Some(sailing + i32::from(!unit.kind.has_marine_attack()));
        }
        turns
    }

    pub(super) fn advisor_city_defend_bonus(&self, city: &City, attacker: UnitKind) -> i32 {
        let aerial = attacker.is_aircraft();
        let naval = attacker.is_naval();
        let mut bonus = if aerial {
            match self.tile(city.x, city.y).base {
                Some(Base::Airstrip | Base::Fortress) => 50,
                Some(Base::Airbase) => 100,
                _ => 0,
            }
        } else {
            self.tile(city.x, city.y)
                .base
                .map_or(0, Base::defense_bonus)
        };
        if !aerial && !naval {
            bonus += CITY_DEFEND_BONUS;
            if city.has_building(Building::CITY_WALLS) {
                bonus += 100;
            }
            if self.has_active_wonder(city.owner, Building::GREAT_WALL) {
                bonus += 50;
            }
        }
        if !aerial && city.size >= LARGE_CITY_SIZE {
            bonus += LARGE_CITY_DEFEND_BONUS;
        }
        if naval && city.has_building(Building::COASTAL_DEFENSE) {
            bonus += 100;
        }
        // SDI supplies Nuke_Proof, not Defend_Bonus. Missiles also have
        // their own class, outside both SAM Air/Helicopter effects.
        if aerial && !attacker.is_missile() && city.has_building(Building::SAM_BATTERY) {
            bonus += 100;
        }
        bonus
    }

    /// Source assess_danger and assess_defense_quadratic, with normal AI's
    /// three-turn horizon. Ratings remain integer at native assignment points.
    pub(super) fn advisor_guard_city_assessment(&self, city: &City) -> GuardCityAssessment {
        let mut result = GuardCityAssessment {
            danger: 0,
            urgency: 0,
            grave: 0,
            defense: 0,
            defense_igwall: 0,
        };
        for (enemy, _, turns) in self.city_threat_arrivals(city) {
            let occupies = enemy.kind.can_occupy_city() || enemy.kind.can_carry(UnitKind::Warrior);
            let danger = self.advisor_threat_vulnerability(city, enemy);
            if occupies && turns <= 3 {
                result.urgency += 1;
                if turns <= 1 {
                    result.grave += 1;
                }
            }
            result.danger += danger * danger / turns.max(1);
        }
        result.urgency += 10 * result.grave;
        // Native chooses 90 whenever danger is nonzero, 5 otherwise, then
        // finds the integer ceiling square root of wallvalue*10.
        let wall_value = if result.danger > 0 { 90 } else { 5 };
        let mut walls = 0;
        while walls * walls < wall_value * 10 {
            walls += 1;
        }
        // city_got_defense_effect(NULL) calls get_city_bonus without a
        // unittype. All class-specific effects are MAYBE and fail CERTAIN.
        // Only these unconditional extra effects survive in this ruleset.
        let has_defense = matches!(
            self.tile(city.x, city.y).base,
            Some(Base::Fortress | Base::Airbase)
        );
        let mut defense = 0;
        for unit in self.tile_units_ordered(city.x, city.y) {
            let mut rating = self.advisor_garrison_defense_rating(city, unit);
            result.defense_igwall += rating * rating;
            if has_defense {
                rating = rating * walls / 10;
            }
            defense += rating;
        }
        result.defense = defense.min(1 << 15).pow(2);
        result
    }

    pub(super) fn advisor_threat_vulnerability(&self, city: &City, enemy: &Unit) -> i32 {
        let occupies = enemy.kind.can_occupy_city() || enemy.kind.can_carry(UnitKind::Warrior);
        let stats = enemy.kind.stats();
        let mut danger = percent_of(stats.attack * POWER_FACTOR, enemy.veteran_power())
            * enemy.hp
            * stats.firepower
            / ADVISOR_POWER_DIVIDER;
        // Only_Native_Attack cannot attack a land city. Zero-attack
        // occupation carriers still require any protector.
        if enemy.kind == UnitKind::Submarine {
            danger = 0;
        }
        danger = danger * 100 / (100 + self.advisor_city_defend_bonus(city, enemy.kind)).max(1);
        if occupies {
            danger = danger.max(1);
        }
        let mut specific = 0;
        for defender in self.tile_units_ordered(city.x, city.y) {
            if defender.kind == UnitKind::AegisCruiser && enemy.kind.has_flag(UnitFlag::AirAttacker)
            {
                specific = specific.max(400);
            }
            if defender.kind == UnitKind::Destroyer && enemy.kind == UnitKind::Submarine {
                specific = specific.max(100);
            }
        }
        if specific > 100 {
            specific = (specific + 100) / 2;
        }
        danger = danger * 100 / (specific + 100);
        danger
    }

    fn advisor_garrison_defense_rating(&self, city: &City, unit: &Unit) -> i32 {
        if !Self::worker_guard_for_advisor(unit.kind) {
            return 0;
        }
        let stats = unit.kind.stats();
        let mut power = percent_of(stats.defense * POWER_FACTOR, unit.veteran_power());
        if !unit.kind.is_aircraft()
            && !unit.kind.is_naval()
            && unit.kind.unit_class() != UnitClass::BigLand
        {
            power = apply_percent_bonus(power, self.tile(city.x, city.y).terrain.defense_bonus());
        }
        // With attacker==NULL native deliberately skips Defend_Bonus
        // effects. Natural River defense remains an extra bonus, only
        // for native Land/Small Land classes (not Big Land or Sea).
        if self.tile(city.x, city.y).river
            && !unit.kind.is_aircraft()
            && !unit.kind.is_naval()
            && unit.kind.unit_class() != UnitClass::BigLand
        {
            power = apply_percent_bonus(power, 25);
        }
        if unit.kind.can_fortify() {
            power = apply_percent_bonus(power, FORTIFY_DEFENSE_BONUS);
        }
        let mut rating = power * unit.hp;
        if unit.kind.has_flag(UnitFlag::BadCityDefender) {
            rating = rating * stats.firepower.min(1) / 2;
        } else {
            rating *= stats.firepower;
        }
        rating /= ADVISOR_POWER_DIVIDER;
        rating
    }

    fn worker_guard_for_advisor(kind: UnitKind) -> bool {
        !kind.is_civilian()
    }

    pub(super) fn city_grave_danger(&self, city: &City) -> usize {
        self.city_threat_arrivals(city)
            .filter(|(unit, _, turns)| {
                *turns <= 1
                    && (unit.kind.can_occupy_city() || unit.kind.can_carry(UnitKind::Warrior))
            })
            .count()
    }

    pub(super) fn city_threat_arrivals<'a>(
        &'a self,
        city: &'a City,
    ) -> impl Iterator<Item = (&'a Unit, i32, i32)> + 'a {
        (0..self.players.len())
            .flat_map(move |owner| self.player_units_ordered(owner))
            .filter_map(|unit| {
                let stats = unit.kind.stats();
                let carries_occupiers = unit.kind.can_carry(UnitKind::Warrior);
                if (!unit.kind.is_military() && !carries_occupiers)
                    || !self.advisor_player_dangerous(city.owner, unit.owner)
                {
                    return None;
                }
                let turns = self.city_danger_arrival(unit, city)?;
                let rating = percent_of(POWER_FACTOR * stats.attack, unit.veteran_power())
                    * unit.hp
                    * stats.firepower
                    / ADVISOR_POWER_DIVIDER;
                // Even an empty occupation transport requires some protection;
                // source assess_danger clamps its zero attack rating to one.
                Some((unit, rating.max(1) / turns.max(1), turns))
            })
    }

    pub(crate) fn ai_embarked_paradrop_danger_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.base = None;
            tile.hut = false;
            tile.owner = None;
            tile.known_by[1] = false;
        }
        let home = (10, 24);
        self.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
        self.create_city(1, home.0, home.1);
        self.sign_pact(1, HUMAN, Pact::War);
        let sea = offset_native_by_map(home.0, home.1, -4, 0);
        let ferry = self.spawn_unit(HUMAN, UnitKind::Transport, sea.0, sea.1);
        let infantry = self.spawn_unit(HUMAN, UnitKind::Warrior, sea.0, sea.1);
        let marine = self.spawn_unit(HUMAN, UnitKind::Marines, sea.0, sea.1);
        if !self.board_specific_ship(infantry, ferry) || !self.board_specific_ship(marine, ferry) {
            return false;
        }
        self.tile_mut(sea.0, sea.1).known_by[1] = false;
        let city = self.cities[0].clone();
        let arrival = |sim: &Self, id| {
            sim.city_threat_arrivals(&city)
                .find(|(u, _, _)| u.id == id)
                .map(|(_, _, turn)| turn)
        };
        let transported = arrival(self, infantry) == Some(1)
            && arrival(self, marine) == Some(0)
            && arrival(self, ferry) == Some(0)
            && self.city_defense_production(&city).is_some();
        self.remove_units_with_cargo(vec![ferry]);
        let airbase = offset_native_by_map(home.0, home.1, 9, 0);
        self.tile_mut(airbase.0, airbase.1).terrain = Terrain::Grassland;
        self.tile_mut(airbase.0, airbase.1).base = Some(Base::Airbase);
        self.tile_mut(airbase.0, airbase.1).owner = Some(HUMAN);
        let para = self.spawn_unit(HUMAN, UnitKind::Paratroopers, airbase.0, airbase.1);
        self.tile_mut(airbase.0, airbase.1).known_by[1] = false;
        let airborne =
            arrival(self, para) == Some(0) && self.city_defense_production(&city).is_some();
        let before = self.units.iter().find(|u| u.id == para).map(|u| (u.x, u.y));
        self.sign_pact(1, HUMAN, Pact::Peace);
        self.ai_war_plans.insert(
            (1, HUMAN),
            diplomacy::WarPlan {
                countdown: 3,
                reason: diplomacy::WarReason::Spoils,
            },
        );
        let anticipated = arrival(self, para) == Some(0);
        let after = self.units.iter().find(|u| u.id == para).map(|u| (u.x, u.y));
        transported && airborne && anticipated && before == after
    }

    pub(super) fn defender_versus(&self, city: &City, attacker: &Unit) -> Option<Unit> {
        // daimilitary.c:dai_choose_defender_versus: expected damage survived
        // per shield, rewarding surviving the complete attacking unit.
        let mut best = None;
        let mut best_want = 0.0;
        let mut best_cost = i32::MAX;
        for &kind in SIMPLE_MILITARY
            .iter()
            .filter(|&&kind| self.unit_buildable(city, kind))
        {
            let mut defender = self.make_unit(0, city.owner, kind, city.x, city.y);
            defender.home_city = Some(city.id);
            defender.veteran = self.trained_rank(city, kind);
            let defense = self.defense_power_against(&defender, city.x, city.y, attacker.kind);
            let (attack_fp, defense_fp) = self.combat_firepower(attacker, &defender);
            let denominator = Self::attack_power(attacker) * attack_fp;
            if denominator <= 0 || kind.stats().cost <= 0 {
                continue;
            }
            let loss = f64::from(defense * defender.hp * defense_fp) / f64::from(denominator);
            let want =
                (loss + (loss - f64::from(attacker.hp)).max(0.0)) / f64::from(kind.stats().cost);
            let tied = (want - best_want).abs() < 0.001 * (want.abs() + best_want.abs());
            if want > best_want || (tied && kind.stats().cost <= best_cost) {
                best_want = want;
                best_cost = kind.stats().cost;
                best = Some(defender);
            }
        }
        best
    }

    pub(super) fn anticipated_city_defender(
        &self,
        city: &City,
        attacker: &Unit,
        arrival: i32,
        production: bool,
    ) -> Option<Unit> {
        let mut defender = self
            .best_defender_id(attacker, city.x, city.y)
            .and_then(|id| self.units.iter().find(|u| u.id == id))
            .cloned();
        if production && let Some(unit) = defender.as_mut() {
            unit.hp = unit.kind.stats().hp;
        }
        if arrival <= 1 {
            return defender;
        }
        let rating = |u: &Unit| {
            self.defense_power_against(u, city.x, city.y, attacker.kind)
                * u.hp
                * u.kind.stats().firepower
                / ADVISOR_POWER_DIVIDER
        };
        // The reference assumes a buildable counter-unit beyond one turn;
        // it does not gate this conservative forecast on stock or treasury.
        if let Some(future) = self.defender_versus(city, attacker)
            && defender
                .as_ref()
                .is_none_or(|current| rating(&future) > rating(current))
        {
            defender = Some(future);
        }
        defender
    }

    fn production_defender_types() -> impl Iterator<Item = UnitKind> {
        UnitKind::SIMPLE_MILITARY
            .into_iter()
            .filter(|kind| kind.has_role(UnitRole::DefendGood) || kind.has_role(UnitRole::DefendOk))
    }

    fn production_defender_desire(kind: UnitKind, danger: i32) -> f32 {
        let good = Self::guard_defender_role(kind);
        let stats = kind.stats();
        // dai_unit_defense_desirability uses type statistics, not the
        // veteran level or the particular enemy's combat odds.
        let mut desire = stats.hp as f32;
        let firepower = if kind.has_flag(UnitFlag::BadCityDefender) {
            stats.firepower.min(1)
        } else {
            stats.firepower
        };
        desire *= firepower as f32;
        desire *= stats.defense as f32;
        desire += stats.moves as f32;
        desire += stats.attack as f32;
        let bonus = if kind == UnitKind::AegisCruiser {
            250
        } else {
            0
        };
        desire += desire * bonus as f32 / 100.0;
        if kind.has_flag(UnitFlag::GameLoss) {
            desire /= 10.0;
        }
        if !good {
            desire /= 2.0;
        }
        if kind.has_flag(UnitFlag::FieldUnit) {
            if danger == 0 {
                return -50.0;
            }
            desire /= 2.0;
        }
        desire /= (ADVISOR_POWER_DIVIDER / 2) as f32;
        desire *= desire;
        desire
    }

    fn production_city_defender(&self, city: &City, danger: i32) -> Option<UnitKind> {
        self.production_city_defender_with_assessment(
            city,
            danger,
            self.advisor_guard_city_assessment(city),
        )
    }

    fn production_city_defender_with_assessment(
        &self,
        city: &City,
        danger: i32,
        assessment: GuardCityAssessment,
    ) -> Option<UnitKind> {
        // Native simple_ai_types in ruleset order, intersected with the
        // supported DefendGood/DefendOk roles. Fuelled fighters and Carrier
        // (capacity8) are excluded even though their roles qualify.
        let wallvalue = if assessment.danger > 0 { 90 } else { 5 };
        let walls = matches!(
            self.tile(city.x, city.y).base,
            Some(Base::Fortress | Base::Airbase)
        );
        let mut best = None;
        let mut best_desire = -1.0_f32;
        let mut best_cost = 1;
        let limit = city.shields + 40;
        for kind in Self::production_defender_types() {
            if !self.unit_buildable(city, kind) {
                continue;
            }
            let mut desire = Self::production_defender_desire(kind, danger);
            if desire <= 0.0 {
                continue;
            }
            if walls && !kind.has_flag(UnitFlag::BadCityDefender) {
                desire *= wallvalue as f32;
                desire /= POWER_FACTOR as f32;
            }
            let cost = kind.stats().cost;
            let tied = f64::from(desire - best_desire).abs()
                < 0.001 * (f64::from(desire.abs()) + f64::from(best_desire.abs()));
            // Keep the literal source condition. Its final limit comparison
            // is tautological (limit == stock+40); this is NOT a hard budget.
            // A higher-desire later unit can win even when it costs over40.
            if best_cost > limit && cost < best_cost
                || desire > best_desire
                || tied && cost <= best_cost
            {
                best = Some(kind);
                best_desire = desire;
                best_cost = cost;
            }
        }
        best
    }

    pub(super) fn preferred_city_defender(&self, city: &City) -> UnitKind {
        self.production_city_defender(city, 1)
            .unwrap_or(UnitKind::Warrior)
    }

    fn production_defender_tech_want(&self, city: &City, tech: Tech) -> f32 {
        if !Self::production_defender_types().any(|kind| kind.required_tech() == Some(tech)) {
            return 0.0;
        }
        // The side effect belongs to dai_process_defender_want, not every
        // tactical call to dai_choose_defender_versus. Mirror the production
        // caller's gates before evaluating future defenders.
        let assessment = self.advisor_guard_city_assessment(city);
        let martial = self.city_martial_law_want(city) as f32;
        if assessment.danger == 0 && martial <= 0.0 {
            return 0.0;
        }
        let danger =
            Self::native_city_defense_danger(assessment, self.city_forecast(city).shield_surplus());
        let count = self.tile_units_ordered(city.x, city.y).count() as i32;
        if !(assessment.defense == 0 && danger > 0
            || danger > 0 && count <= assessment.urgency
            || martial > 0.0)
        {
            return 0.0;
        }
        let wallvalue = if assessment.danger > 0 {
            90.0_f32
        } else {
            5.0_f32
        };
        let selected = self.production_city_defender_with_assessment(city, danger, assessment);
        let (mut best, best_cost) = selected.map_or((-1.0_f32, 100), |kind| {
            let mut desire = Self::production_defender_desire(kind, danger);
            if !kind.has_flag(UnitFlag::BadCityDefender) {
                desire *= wallvalue;
                desire /= POWER_FACTOR as f32;
            }
            (desire, kind.stats().cost)
        });
        if best <= 0.0 {
            best = 1.0;
        }
        let total = danger as f32 + martial;
        let cities = self.player_cities_ordered(city.owner).count().max(1) as i32;
        let mut want = 0.0_f32;
        for kind in Self::production_defender_types() {
            if self.unit_buildable(city, kind)
                || !self.unit_buildable_later(city, kind)
                || kind.required_tech() != Some(tech)
            {
                continue;
            }
            let mut desire = Self::production_defender_desire(kind, danger);
            if desire <= 0.0 {
                continue;
            }
            // Supported unit requirements are a single reachable advance;
            // goal cost includes its still-unknown prerequisites. The source
            // overwrites (not sums) costs for multi-tech requirements.
            let tech_cost = self
                .research_path(city.owner, tech)
                .into_iter()
                .map(|required| self.target_cost(city.owner, required))
                .sum::<i32>()
                / 4
                / cities;
            if !kind.has_flag(UnitFlag::BadCityDefender) {
                desire *= wallvalue;
                desire /= POWER_FACTOR as f32;
            }
            let tech_desire = desire * total / (kind.stats().cost + tech_cost) as f32;
            if tech_desire > 0.0 {
                want += tech_desire * best_cost as f32 / best;
            }
        }
        want
    }

    pub(super) fn city_attacker_odds(
        &self,
        city: &City,
        enemy: &Unit,
        defender: &Unit,
        building: Option<Building>,
    ) -> f64 {
        let full_strength = percent_of(
            enemy.kind.stats().attack * POWER_FACTOR,
            enemy.veteran_power(),
        );
        let (attack_fp, defense_fp) = self.combat_firepower(enemy, defender);
        combat_win_chance(
            full_strength,
            enemy.hp,
            attack_fp,
            self.defense_power_with_building(defender, city.x, city.y, enemy.kind, building),
            defender.hp,
            defense_fp,
        )
    }

    fn city_martial_law_want(&self, city: &City) -> f64 {
        let amount = match self.players[city.owner].government {
            Government::Anarchy | Government::Despotism | Government::Monarchy => 1,
            Government::Communism => 2,
            _ => 0,
        };
        if amount == 0
            || city.has_building(Building::SHAKESPEARES_THEATER)
            || self.tile_units_ordered(city.x, city.y).count() >= 3
        {
            return 0.0;
        }
        // FEELING_FINAL is the sole happiness step used by the military
        // advisor. An entertainer also justifies a police unit even when the
        // present assignment has already removed the city's unhappiness.
        let (_, unhappy, _) = self.city_feelings(city, self.city_forecast(city).luxury);
        f64::from(amount.min(unhappy + city.specialists[2]) * 35)
    }

    fn native_city_defense_danger(assessment: GuardCityAssessment, surplus: i32) -> i32 {
        // Literal source positive feedback: assess_danger has squared each
        // attack rating already, and military_advisor squares its sum again.
        // Wider intermediates avoid undefined native signed overflow.
        let qdanger = i64::from(assessment.danger).pow(2);
        let defense = i64::from(assessment.defense);
        if qdanger == 0 || surplus <= 0 && defense != 0 {
            return 0;
        }
        if qdanger >= defense {
            if assessment.urgency == 0 {
                100
            } else if defense == 0 {
                200 + assessment.urgency
            } else {
                (200.min(100 * qdanger / defense)) as i32 + assessment.urgency
            }
        } else {
            (100 * qdanger / defense) as i32
        }
    }

    fn defense_building_provider(
        &self,
        city: &City,
        attacker: Option<UnitKind>,
    ) -> Option<Building> {
        // dai_find_source_building picks the greatest single effect, with
        // strict greater-than preserving ruleset order on ties. All three
        // Defend_Bonus providers have value100. A NULL type deliberately
        // skips class requirements, matching the native advisor's FIXME.
        [
            Building::CITY_WALLS,
            Building::COASTAL_DEFENSE,
            Building::SAM_BATTERY,
        ]
        .into_iter()
        .find(|&building| {
            self.building_blocker(city, building).is_none()
                && attacker.is_none_or(|kind| match building {
                    Building::CITY_WALLS => !kind.is_aircraft() && !kind.is_naval(),
                    Building::COASTAL_DEFENSE => kind.is_naval(),
                    Building::SAM_BATTERY => kind.is_aircraft() && !kind.is_missile(),
                    _ => false,
                })
        })
    }

    fn native_defense_building_want(
        &self,
        city: &City,
        building: Building,
        assessment: GuardCityAssessment,
    ) -> f64 {
        let initial = self.domestic_improvement_want(city, building) as f32;
        if initial == 0.0 {
            return 0.0;
        }
        let reduced: i64 = self
            .city_threat_arrivals(city)
            .filter(|(enemy, _, _)| {
                self.defense_building_provider(city, Some(enemy.kind)) == Some(building)
            })
            .map(|(enemy, _, turns)| {
                let vulnerability = i64::from(self.advisor_threat_vulnerability(city, enemy));
                // Native danger_reduced discounts the already time-discounted
                // vulnerability once more, independently of total danger.
                vulnerability * vulnerability / i64::from(turns.max(1)).pow(2)
            })
            .sum();
        if reduced <= 0 {
            return f64::from(initial);
        }
        let mut want = initial.max((100 + assessment.urgency) as f32);
        if assessment.urgency > 0 && reduced > i64::from(assessment.defense_igwall) * 2 {
            want += 100.0;
        } else if assessment.defense_igwall != 0 && reduced > i64::from(assessment.defense_igwall) {
            want = want.max((reduced * 100 / i64::from(assessment.defense_igwall)) as f32);
        }
        f64::from(want)
    }

    fn native_city_defense_choice(&self, city: &City) -> (Option<AiBuildChoice>, f64) {
        let assessment = self.advisor_guard_city_assessment(city);
        let martial = self.city_martial_law_want(city);
        if assessment.danger == 0 && martial <= 0.0 {
            return (None, martial);
        }
        let danger =
            Self::native_city_defense_danger(assessment, self.city_forecast(city).shield_surplus());
        let count = self.tile_units_ordered(city.x, city.y).count() as i32;
        let Some(kind) = self.production_city_defender_with_assessment(city, danger, assessment)
        else {
            return (None, martial);
        };
        if assessment.defense == 0 && danger > 0 {
            return (
                Some(AiBuildChoice {
                    production: Production::Unit(kind),
                    want: f64::from((1000 + danger) as f32),
                }),
                martial,
            );
        }
        let mut choice = None;
        if assessment.danger > 0
            && assessment.defense != 0
            && let Some(building) = self.defense_building_provider(city, None)
        {
            // This is the pure impr_buy_gold_cost quote, not the action's
            // founded-this-turn/disorder/bought-production legality gate.
            let price = 2
                * (building.data().cost - city.shields).max(0)
                * if city.shields == 0 { 2 } else { 1 };
            let affordable = self.players[city.owner].gold > price;
            let want = self.native_defense_building_want(city, building, assessment);
            if want > 0.0 && (danger < 101 || count > 1 || assessment.grave == 0 && affordable) {
                let want = f64::from((want * 0.5) as f32);
                choice = Some(AiBuildChoice {
                    production: Production::Building(building),
                    want: if assessment.urgency == 0 {
                        want.min(1000.0)
                    } else {
                        want
                    },
                });
            }
        }
        if danger > 0 && count <= assessment.urgency || martial > 0.0 {
            let want = if assessment.urgency == 0 && kind.stats().defense == 1 {
                // get_city_bonus(HP_Regen) has NULL type, so class-specific
                // Barracks/Port/Airport effects do not satisfy CERTAIN.
                danger.min(25)
            } else {
                danger
            };
            let candidate = AiBuildChoice {
                production: Production::Unit(kind),
                want: f64::from((want as f32 + martial as f32) as f32),
            };
            if choice.is_none_or(|old| candidate.want > old.want) {
                choice = Some(candidate);
            }
        }
        (choice, martial)
    }

    pub(super) fn city_defense_production(&self, city: &City) -> Option<Production> {
        self.native_city_defense_choice(city)
            .0
            .map(|choice| choice.production)
    }

    pub(crate) fn native_defensive_effects_playtest(&mut self) -> (bool, bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.base = None;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
        }
        self.create_city(1, 10, 24);
        self.cities[0].buildings.retain(|&b| b != Building::PALACE);
        let value =
            |sim: &Self, building| sim.defensive_building_effect_want(&sim.cities[0], building);
        let quiet = value(self, Building::CITY_WALLS) == 3.0
            && value(self, Building::COASTAL_DEFENSE) == 6.0
            && value(self, Building::SAM_BATTERY) == 6.0;
        // is_capital includes the secondary Capital_City value1 as well as
        // the primary Palace value2; it is not is_primary_capital.
        self.cities[0]
            .buildings
            .push(Building::ECCLESIASTICAL_PALACE);
        let quiet = quiet
            && value(self, Building::CITY_WALLS) == 11.0
            && value(self, Building::COASTAL_DEFENSE) == 22.0;
        self.cities[0]
            .buildings
            .retain(|&b| b != Building::ECCLESIASTICAL_PALACE);
        let remote = offset_native_by_map(10, 24, 4, 0);
        self.spawn_unit(HUMAN, UnitKind::Marines, remote.0, remote.1);
        let unit_only = value(self, Building::CITY_WALLS) == 3.0;
        self.create_city(HUMAN, remote.0, remote.1);
        let cities = unit_only
            && value(self, Building::CITY_WALLS) == 83.0
            && value(self, Building::SAM_BATTERY) == 166.0;
        self.cities.retain(|city| city.owner == 1);
        self.units.clear();
        let coast = offset_native_by_map(10, 24, 1, 0);
        self.tile_mut(coast.0, coast.1).terrain = Terrain::Ocean;
        self.spawn_unit(HUMAN, UnitKind::Battleship, coast.0, coast.1);
        let ocean = value(self, Building::COASTAL_DEFENSE) == 38.0
            && value(self, Building::SAM_BATTERY) == 38.0;
        self.spawn_unit(HUMAN, UnitKind::Transport, coast.0, coast.1);
        let rows = ocean
            && value(self, Building::CITY_WALLS) == 9.0
            && value(self, Building::COASTAL_DEFENSE) == 40.0
            && value(self, Building::SAM_BATTERY) == 50.0;
        self.units.clear();
        self.tile_mut(coast.0, coast.1).terrain = Terrain::Grassland;
        self.players[1].known.clear();
        self.players[1].government = Government::Despotism;
        self.spawn_unit(HUMAN, UnitKind::Warrior, coast.0, coast.1);
        // Native static defender side effect: current Warrior rating.16,
        // future Phalanx484/225, wallvalue90, danger211, Bronze30/4=7.
        // ((484/225)*9*211/27)*10/(.16*9) ~=1050.6584.
        let want = self.production_defender_tech_want(&self.cities[0], Tech::BRONZE_WORKING);
        let technology = (want - 1050.6584).abs() < 0.001
            && self.advisor_tech_wants(1)[Tech::BRONZE_WORKING.0] >= want;
        (quiet, cities, rows, technology)
    }

    pub(crate) fn native_city_defense_advisor_playtest(&mut self) -> (bool, bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.river = false;
            tile.base = None;
            tile.road = false;
            tile.railroad = false;
        }
        self.players[1].known = vec![Tech::BRONZE_WORKING];
        self.players[1].government = Government::Despotism;
        self.create_city(1, 10, 24);
        let quiet = self.native_city_defense_choice(&self.cities[0]).0.is_none();
        let near = offset_native_by_map(10, 24, 1, 0);
        self.spawn_unit(HUMAN, UnitKind::Warrior, near.0, near.1);
        let first = self
            .native_city_defense_choice(&self.cities[0])
            .0
            .is_some_and(|choice| choice.want == 1211.0);
        self.spawn_unit(1, UnitKind::Phalanx, 10, 24);
        // Danger rating2 => total4 => qdanger16; fortified Phalanx
        // defense rating10 => defense100, giving a native16 percent bid.
        let defended = self
            .native_city_defense_choice(&self.cities[0])
            .0
            .is_some_and(|choice| choice.want == 16.0);
        self.units.clear();
        self.cities[0].size = 3;
        self.cities[0].specialists[2] = 1;
        let police = self
            .native_city_defense_choice(&self.cities[0])
            .0
            .is_some_and(|choice| choice.want == 35.0);
        (quiet, first, defended, police)
    }

    pub(crate) fn ai_production_priorities_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.river = true;
            tile.hut = false;
        }
        self.create_city(1, 10, 24);
        self.cities[0].size = 8;
        self.cities[0].buildings.extend([
            Building::TEMPLE,
            Building::CATHEDRAL,
            Building::AMPHITHEATER,
        ]);
        self.spawn_unit(1, UnitKind::Warrior, 10, 24);
        let id = self.cities[0].id;
        self.players[1].known.push(Tech::POTTERY);
        self.reconcile_city_workers();
        let expansion_available = self.settlement_production_choice(&self.cities[0]).is_some();
        let growth_priority =
            self.choose_ai_production(1, id) == Production::Building(Building::AQUEDUCT_RIVER);
        self.prepare_ai_city_defenses(1);
        let selected = self.cities[0].production == Production::Building(Building::AQUEDUCT_RIVER);
        self.turn += 1;
        self.cities[0].shields = 1;
        self.players[1].gold = 500;
        self.spend_ai_defense_gold(1);
        expansion_available && growth_priority && selected && self.cities[0].bought_production
    }

    pub(super) fn bodyguard_production_choice(
        &self,
        city: &City,
        sea: bool,
    ) -> Option<(UnitKind, f64)> {
        // daimilitary.c:dai_choose_bodyguard deliberately chooses the best
        // defensive type BEFORE checking whether it can serve any charge.
        // The source computes allow_gold from tax and ACTUAL homed upkeep,
        // then queries utype_upkeep_cost(type,NULL,player,Gold). With no unit
        // context HasHomeCity is MAYBE, so every provided Gold Upkeep_Pct
        // row fails CERTAIN and all candidate type gold costs are zero.
        let mut best = None;
        let mut best_desire = 0.0_f32;
        for &kind in SIMPLE_MILITARY {
            if !Self::guard_defender_role(kind)
                || kind.is_civilian()
                || kind.is_naval() != sea
                || !self.unit_buildable(city, kind)
            {
                continue;
            }
            let stats = kind.stats();
            let mut desire = stats.hp as f32;
            desire *= stats.firepower as f32; // all supported DefendGood types have FP1
            desire *= stats.defense as f32;
            desire += stats.moves as f32;
            desire += stats.attack as f32;
            let bonus = if kind == UnitKind::AegisCruiser {
                250
            } else {
                0
            };
            desire += desire * bonus as f32 / 100.0;
            let tied = (f64::from(desire - best_desire)).abs()
                < 0.001 * (f64::from(desire.abs()) + f64::from(best_desire.abs()));
            if desire > best_desire
                || (tied && best.is_none_or(|old: UnitKind| stats.cost <= old.stats().cost))
            {
                best = Some(kind);
                best_desire = desire;
            }
        }
        let kind = best?;
        let mut virtual_unit = self.make_unit(0, city.owner, kind, city.x, city.y);
        virtual_unit.veteran = self.trained_rank(city, kind);
        virtual_unit.home_city = Some(city.id);
        self.look_for_ai_charge(&virtual_unit)
            .and_then(|(_, want)| (want > 0.0).then_some((kind, f64::from(want))))
    }

    fn consider_bodyguard_production(
        &self,
        city: &City,
        sea: bool,
        choice: &mut Option<AiBuildChoice>,
    ) -> bool {
        // dai_unit_consider_bodyguard never overrides a military emergency.
        if choice.is_some_and(|old| old.want >= MILITARY_EMERGENCY_WANT) {
            return false;
        }
        if let Some((kind, want)) = self.bodyguard_production_choice(city, sea)
            && choice.is_none_or(|old| want > old.want)
        {
            *choice = Some(AiBuildChoice {
                production: Production::Unit(kind),
                want,
            });
            true
        } else {
            false
        }
    }

    pub(crate) fn ai_bodyguard_production_playtest(&mut self) -> (bool, bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        self.ai_guard_targets.clear();
        self.ai_unit_guards.clear();
        self.unit_goto_tiles.clear();
        self.players[1].known = vec![Tech::BRONZE_WORKING];
        self.players[1].government = Government::Despotism;
        self.players[1].rates = [40, 60, 0];
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
            tile.base = None;
        }
        self.create_city(1, 10, 24);
        self.cities[0].buildings.push(Building::BARRACKS);
        let city = self.cities[0].clone();
        let charge = self.spawn_unit(1, UnitKind::Catapult, 10, 24);
        self.request_ai_guard(charge);
        let mut routine = None;
        let selected = self.consider_bodyguard_production(&city, false, &mut routine)
            && routine.is_some_and(|c| {
                c.production == Production::Unit(UnitKind::Phalanx) && c.want == 91.0
            })
            && self
                .ai_build_choice(&city)
                .is_some_and(|choice| choice.production == Production::Unit(UnitKind::Phalanx));
        if let Some(choice) = routine {
            self.cities[0].production = choice.production;
        }
        for _ in 0..3 {
            self.spawn_unit(1, UnitKind::Warrior, 10, 24);
            self.units.last_mut().unwrap().home_city = Some(city.id);
        }
        let high_tax = !self.city_gold_upkeep(&city).is_empty()
            && self.bodyguard_production_choice(&city, false).is_some();
        self.players[1].rates = [50, 50, 0];
        let null_type_upkeep = high_tax && self.bodyguard_production_choice(&city, false).is_some();
        let near = offset_native_by_map(10, 24, 1, 0);
        for _ in 0..8 {
            self.spawn_unit(HUMAN, UnitKind::Marines, near.0, near.1);
        }
        let guard_want = self
            .bodyguard_production_choice(&city, false)
            .map_or(0.0, |(_, want)| want);
        let mut emergency = Some(AiBuildChoice {
            production: Production::Unit(UnitKind::Warrior),
            want: MILITARY_EMERGENCY_WANT,
        });
        let protected = guard_want > MILITARY_EMERGENCY_WANT
            && !self.consider_bodyguard_production(&city, false, &mut emergency)
            && emergency.is_some_and(|c| c.production == Production::Unit(UnitKind::Warrior));
        self.units.retain(|u| u.owner == 1);
        self.players[1].known = Tech::all().collect();
        let native_best_type = self.unit_buildable(&city, UnitKind::Marines)
            && self.bodyguard_production_choice(&city, false).is_none();
        (selected, null_type_upkeep, protected, native_best_type)
    }

    fn military_training_building(&self, city: &City, kind: UnitKind) -> Option<Building> {
        // dai_find_source_building(EFT_VETERAN_BUILD): effects are examined
        // in ruleset order; only buildable ordinary improvements qualify.
        // All supported provider effects have value1. Wonders/governments
        // are not infrastructure providers, though existing bonuses train.
        let candidates: &[Building] = if kind.is_aircraft() || kind.is_missile() {
            &[Building::AIRPORT]
        } else if kind.is_naval() {
            &[Building::PORT_FACILITY]
        } else {
            &[
                Building::BARRACKS,
                Building::BARRACKS_II,
                Building::BARRACKS_III,
            ]
        };
        candidates.iter().copied().find(|&building| {
            building.data().genus == Genus::Improvement
                && self.building_blocker(city, building).is_none()
        })
    }

    fn adjust_military_training_choice(&self, city: &City, choice: &mut AiBuildChoice) {
        let Production::Unit(kind) = choice.production else {
            return;
        };
        if kind.is_civilian() || self.trained_rank(city, kind) > 0 {
            return;
        }
        if let Some(building) = self.military_training_building(city, kind) {
            choice.production = Production::Building(building);
            // All six civilizations use the default Builder50. Native int divisions
            // 50/TRAIT_DEFAULT_VALUE(50)/2 yield0, before adding double0.5.
            choice.want = (f64::from(choice.want as f32) * 0.5) as f32 as f64;
        }
    }

    pub(super) fn routine_military_advice_allowed(&self, city: &City) -> bool {
        let forecast = self.city_forecast(city);
        let (_, final_unhappy, _) = self.city_feelings(city, forecast.luxury);
        let (_, effect_unhappy, _) = self.city_feelings_after_effects(city, forecast.luxury);
        forecast.shield_surplus() > 0
            && final_unhappy <= effect_unhappy
            && self.ai_wonder_city(city.owner) != Some(city.id)
    }

    pub(crate) fn military_training_advisor_playtest(&mut self) -> (bool, bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        self.ai_guard_targets.clear();
        self.ai_unit_guards.clear();
        self.unit_goto_tiles.clear();
        self.players[1].known = vec![Tech::BRONZE_WORKING];
        self.players[1].government = Government::Despotism;
        self.players[1].rates = [40, 60, 0];
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.resource = None;
            tile.hut = false;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
            tile.base = None;
        }
        self.tile_mut(10, 24).terrain = Terrain::Hills;
        self.tile_mut(10, 24).mine = true;
        self.create_city(1, 10, 24);
        let charge = self.spawn_unit(1, UnitKind::Catapult, 10, 24);
        self.request_ai_guard(charge);
        let city = self.cities[0].clone();
        let (before, after) = self.military_build_advice(&city);
        let replaced = before
            .is_some_and(|c| c.production == Production::Unit(UnitKind::Phalanx) && c.want == 75.0)
            && after.is_some_and(|c| {
                c.production == Production::Building(Building::BARRACKS) && c.want == 37.5
            })
            && self
                .ai_build_choice(&city)
                .is_some_and(|c| c.production == Production::Building(Building::BARRACKS));
        let mut investment = true;
        for stock in [0, 19, 20, 200] {
            for gold in [0, 10000] {
                let mut projected = city.clone();
                projected.shields = stock;
                self.players[1].gold = gold;
                let mut choice = AiBuildChoice {
                    production: Production::Unit(UnitKind::Phalanx),
                    want: 75.0,
                };
                self.adjust_military_training_choice(&projected, &mut choice);
                investment &= choice.production == Production::Building(Building::BARRACKS)
                    && choice.want == 37.5;
            }
        }
        let near = offset_native_by_map(10, 24, 1, 0);
        self.spawn_unit(HUMAN, UnitKind::Marines, near.0, near.1);
        let (before, after) = self.military_build_advice(&city);
        let urgent = matches!((before,after),(Some(a),Some(b)) if a.production==b.production
            && a.want==b.want && a.want>=MILITARY_EMERGENCY_WANT && matches!(b.production,Production::Unit(_)));
        self.cities[0]
            .buildings
            .push(Building::SUN_TZUS_WAR_ACADEMY);
        self.completed_wonders.push(Building::SUN_TZUS_WAR_ACADEMY);
        let veteran_city = self.cities[0].clone();
        let mut trained = AiBuildChoice {
            production: Production::Unit(UnitKind::Phalanx),
            want: 75.0,
        };
        self.adjust_military_training_choice(&veteran_city, &mut trained);
        let global = self.trained_rank(&veteran_city, UnitKind::Phalanx) > 0
            && trained.production == Production::Unit(UnitKind::Phalanx)
            && trained.want == 75.0;
        (replaced, investment, urgent, global)
    }

    /// Shared native stages preceding the sea/land offensive callbacks.
    /// Returns (current choice, immediate return, severe-defense return).
    pub(super) fn pre_offensive_military_advice(
        &self,
        city: &City,
    ) -> (Option<AiBuildChoice>, bool, bool) {
        if let Some(production) = self.counterintelligence_production(city) {
            let choice = Some(AiBuildChoice {
                production,
                want: 16000.0,
            });
            return (choice, true, false);
        }
        let (mut military, martial) = self.native_city_defense_choice(city);
        // Source returns its life-saving defense advice before all routine
        // candidates and training when production/happiness/wonder gates fail.
        if !self.routine_military_advice_allowed(city) {
            return (military, true, false);
        }
        let mut defensive = military.is_some();
        if self.consider_bodyguard_production(city, false, &mut military) {
            defensive = true;
        }
        // Source order: land guard, agent, sea guard, aircraft, ordinary
        // attackers, then hunter. Strict wants preserve earlier equal bids.
        let consider_unit = |choice: &mut Option<AiBuildChoice>,
                             candidate: Option<(UnitKind, f64)>| {
            if let Some((kind, want)) = candidate
                && want > 0.0
                && choice.is_none_or(|old| want > old.want)
            {
                *choice = Some(AiBuildChoice {
                    production: Production::Unit(kind),
                    want,
                });
                true
            } else {
                false
            }
        };
        let severe = military.is_some_and(|old| old.want - martial >= MILITARY_EMERGENCY_WANT)
            && self.city_grave_danger(city) > 0;
        if !severe {
            if consider_unit(&mut military, self.offensive_agent_choice(city)) {
                defensive = false;
            }
            if self.consider_bodyguard_production(city, true, &mut military) {
                defensive = true;
            }
            if !(defensive && military.is_some_and(|old| old.want >= MILITARY_EMERGENCY_WANT)) {
                consider_unit(&mut military, self.aircraft_production_choice(city));
            }
        }
        (military, false, severe)
    }

    fn military_build_advice(&self, city: &City) -> (Option<AiBuildChoice>, Option<AiBuildChoice>) {
        let (mut military, early, severe) = self.pre_offensive_military_advice(city);
        if early {
            return (military, military);
        }
        if !severe {
            // kill_something_with performs its comparison BEFORE a ferry
            // replacement truncates want. Accept its final choice directly.
            military = self.offensive_advice_with_prior(city, military).0;
            if let Some((kind, want)) = self.hunter_production_choice(city)
                && want > 0.0
                && military.is_none_or(|old| want > old.want)
            {
                military = Some(AiBuildChoice {
                    production: Production::Unit(kind),
                    want,
                });
            }
        }
        let before = military;
        if !severe && let Some(choice) = military.as_mut() {
            self.adjust_military_training_choice(city, choice);
        }
        (before, military)
    }

    pub(super) fn ai_build_choice(&self, city: &City) -> Option<AiBuildChoice> {
        let (_, military) = self.military_build_advice(city);
        if military.is_some() && self.ai_on_war_footing(city.owner) {
            return military;
        }

        let mut best = military;
        let mut consider = |production, want: f64| {
            let want = want.min(DOMESTIC_WANT_MAX);
            if want > 0.0 && best.is_none_or(|choice| want > choice.want) {
                best = Some(AiBuildChoice { production, want });
            }
        };
        if let Some(choice) = self.domestic_build_choice(city) {
            consider(choice.production, choice.want);
        }
        if let Some(choice) = self.wonder_build_choice(city) {
            consider(choice.production, choice.want);
        }
        if let Some((kind, want)) = self.settlement_production_choice(city) {
            let want = if self.ai_wonder_city(city.owner) == Some(city.id) {
                want / if kind.stats().pop_cost > 0 { 5.0 } else { 2.0 }
            } else {
                want
            };
            consider(Production::Unit(kind), want);
        }
        if let Some((kind, want)) = self.merchant_production_choice(city) {
            consider(Production::Unit(kind), want);
        }
        if let Some((kind, want)) = self.wonder_helper_production_choice(city) {
            consider(Production::Unit(kind), want);
        }
        for building in [
            Building::SPACE_MODULE,
            Building::SPACE_COMPONENT,
            Building::SPACE_STRUCTURAL,
        ] {
            if self.building_blocker(city, building).is_none() {
                consider(
                    Production::Building(building),
                    self.space_production_want(city, building),
                );
                break;
            }
        }
        best
    }

    pub(super) fn prepare_ai_city_defenses(&mut self, owner: usize) {
        let ids: Vec<_> = self
            .player_cities_ordered(owner)
            .map(|city| city.id)
            .collect();
        for id in ids {
            let Some(city) = self.cities.iter().find(|city| city.id == id).cloned() else {
                continue;
            };
            self.record_ai_military_tech_wants(&city);
            if let Some(choice) = self.ai_build_choice(&city)
                && city.production != choice.production
            {
                self.change_production(id, choice.production);
            }
        }
    }

    pub(super) fn ai_gold_reserve(&self, owner: usize) -> i32 {
        let citizens = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| city.size)
            .sum::<i32>();
        (2 * citizens).max(self.ai_purchase_reserve.get(&owner).copied().unwrap_or(0))
    }

    fn record_ai_purchase_cost(&mut self, owner: usize, price: i32) {
        let highest = self.ai_purchase_reserve.entry(owner).or_default();
        *highest = (*highest).max(price);
    }

    pub(super) fn spend_ai_defense_gold(&mut self, owner: usize) {
        let owned: Vec<_> = self.player_cities_ordered(owner).collect();
        let expenses: i32 = owned
            .iter()
            .map(|city| self.building_upkeep(city) + self.city_unit_upkeep(city).gold)
            .sum();
        let reserve = self.ai_gold_reserve(owner);
        let mut priorities: Vec<_> = owned
            .into_iter()
            .filter_map(|city| {
                let chosen = self.ai_build_choice(city)?;
                // daicity.c:dai_spend_gold consumes the FINAL city choice.
                // A lower defense-only bid must never replace a selected
                // hunter, ammunition, wonder or domestic improvement here.
                if self.city_defense_production(city) != Some(chosen.production)
                    || city.production != chosen.production
                {
                    return None;
                }
                let immediate = self
                    .city_threat_arrivals(city)
                    .filter(|&(_, _, turns)| turns <= 1)
                    .map(|(_, rating, _)| rating)
                    .sum::<i32>();
                (immediate > 0).then_some((chosen.want, city.id, chosen.production))
            })
            .collect();
        // Stable ties retain the native player-city list order.
        priorities.sort_by(|a, b| b.0.total_cmp(&a.0));
        self.ai_purchase_reserve.remove(&owner);
        let emergency_cities: Vec<_> = priorities.iter().map(|&(_, city, _)| city).collect();
        for (_, id, target) in priorities {
            let city = self.cities.iter().find(|city| city.id == id).unwrap();
            let position = (city.x, city.y);
            self.spend_ai_upgrade_gold(owner, Some(position), expenses);
            let city = self.cities.iter().find(|city| city.id == id).unwrap();
            // Upgrading a garrison may change speculative new advice. Native
            // spending still uses its cached choice from city management.
            if city.production != target {
                continue;
            }
            let city = self.cities.iter().find(|city| city.id == id).unwrap();
            if city.shields + self.city_forecast(city).shield_surplus() >= city.production.cost() {
                continue;
            }
            let Some(price) = self.rush_cost(city) else {
                continue;
            };
            let undefended = !self.units.iter().any(|unit| {
                unit.owner == owner
                    && unit.transport.is_none()
                    && unit.kind.is_military()
                    && (unit.x, unit.y) == position
            });
            let invested = city.shields > 0;
            if undefended
                && matches!(target, Production::Unit(_))
                && self.players[owner].gold - price < expenses
            {
                let sale = Building::all().find(|&building| {
                    !matches!(
                        building,
                        Building::CITY_WALLS
                            | Building::COASTAL_DEFENSE
                            | Building::SAM_BATTERY
                            | Building::SDI_DEFENSE
                    ) && Self::building_sale_price(city, building).is_some()
                });
                if let Some(building) = sale {
                    self.sell_building(owner, id, building);
                }
            }
            let remaining = self.players[owner].gold - price;
            if remaining >= expenses && (undefended || (invested && remaining >= 2 * reserve)) {
                self.rush_production(owner, id);
            } else if undefended && matches!(target, Production::Unit(_)) {
                self.record_ai_purchase_cost(owner, price);
            }
        }
        let at_war = self.ai_on_war_footing(owner);
        self.spend_ai_routine_gold(owner, reserve, expenses, at_war, &emergency_cities);
        if !at_war {
            self.spend_ai_upgrade_gold(owner, None, reserve.max(expenses));
        }
    }

    fn spend_ai_routine_gold(
        &mut self,
        owner: usize,
        reserve: i32,
        expenses: i32,
        at_war: bool,
        emergency_cities: &[u32],
    ) {
        let city_count = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .count();
        let gold = self.players[owner].gold;
        let mut purchases = Vec::new();
        for city in self.cities.iter().filter(|city| {
            city.owner == owner && city.shields > 0 && !emergency_cities.contains(&city.id)
        }) {
            let Some(price) = self.rush_cost(city) else {
                continue;
            };
            let mut cash_floor = 2 * reserve;
            if let Production::Unit(kind) = city.production {
                let stats = kind.stats();
                if city.size <= stats.pop_cost {
                    continue;
                }
                if stats.can_found {
                    if at_war || city_count > 6 {
                        continue;
                    }
                    cash_floor = reserve;
                } else if !at_war
                    && price > 2 * stats.cost
                    && kind.is_military()
                    && (stats.attack > stats.defense
                        || self.offensive_production(city) == Some(kind))
                {
                    continue;
                }
            }
            let cash_floor = cash_floor.max(expenses);
            if gold - price < cash_floor {
                continue;
            }

            let missing = city.production.cost() - city.shields;
            let surplus = self.city_forecast(city).shield_surplus();
            if missing <= surplus {
                continue;
            }
            let want = self
                .ai_build_choice(city)
                .filter(|choice| choice.production == city.production)
                .map_or(0.0, |choice| choice.want);
            purchases.push((want, price, city.id, cash_floor));
        }
        purchases.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        for (_, price, city, cash_floor) in purchases {
            if self.players[owner].gold - price >= cash_floor {
                self.rush_production(owner, city);
            }
        }
    }

    fn spend_ai_upgrade_gold(
        &mut self,
        owner: usize,
        defense_city: Option<(i32, i32)>,
        limit: i32,
    ) {
        let upgrades: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == owner
                    && match defense_city {
                        Some(position) => unit.kind.is_military() && (unit.x, unit.y) == position,
                        None => !unit.kind.is_military(),
                    }
            })
            .filter_map(|unit| {
                self.paid_upgrade(unit)
                    .map(|(target, price)| (unit.id, target, price))
            })
            .collect();
        for (unit, target, price) in upgrades {
            if self.players[owner].gold - price > limit {
                self.upgrade_unit(unit, target, price);
            } else {
                self.record_ai_purchase_cost(owner, price);
            }
        }
    }

    pub(super) fn offensive_production(&self, city: &City) -> Option<UnitKind> {
        self.offensive_build_choice(city)
            .map(|choice| match choice.production {
                Production::Unit(kind) => kind,
                Production::Building(_) => unreachable!(),
            })
    }

    fn offensive_build_choice(&self, city: &City) -> Option<AiBuildChoice> {
        self.offensive_advice(city).0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gold_spending_preserves_final_nondefense_advice() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (city, _) = game.hunter_production_playtest_setup();
        let position = {
            let port = game.cities.iter().find(|c| c.id == city).unwrap();
            (port.x, port.y)
        };
        game.players[1].known.extend([
            Tech::all().find(|tech| tech.name() == "Writing").unwrap(),
            Tech::CONSTRUCTION,
        ]);
        game.players[1].rates = [0, 100, 0];
        for (_, _, x, y) in Simulation::city_radius_at(position.0, position.1) {
            if game.tile(x, y).terrain.is_water() {
                continue;
            }
            let tile = game.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.road = true;
            tile.river = true;
            tile.irrigation = true;
        }
        game.cities.iter_mut().find(|c| c.id == city).unwrap().size = 8;
        game.reconcile_city_workers();
        let port = game.cities.iter().find(|c| c.id == city).unwrap();
        let choice = game.ai_build_choice(port).unwrap();
        let defense = game.city_defense_production(port);
        assert_eq!(choice.production, Production::Unit(UnitKind::Settlers));
        assert_eq!(defense, Some(Production::Unit(UnitKind::AegisCruiser)));
        assert!(choice.want > 700.0);
        assert_ne!(Some(choice.production), defense);
        game.cities
            .iter_mut()
            .find(|c| c.id == city)
            .unwrap()
            .production = choice.production;
        game.players[1].gold = 1000;
        game.spend_ai_defense_gold(1);
        assert_eq!(
            game.cities
                .iter()
                .find(|c| c.id == city)
                .unwrap()
                .production,
            choice.production
        );
    }

    #[test]
    fn native_defense_effects_and_static_research_match_independent_numeric_cases() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.native_defensive_effects_playtest(),
            (true, true, true, true)
        );
    }

    #[test]
    fn defense_effect_capital_and_ocean_city_preserve_native_integer_divisions() {
        let mut context = DefenseEffectContext {
            capital: false,
            ocean_city: false,
            coastal: false,
            continent_threat: false,
            ocean_threat: false,
            invasions: false,
        };
        assert_eq!(
            Simulation::native_defense_effect_value(&context, true, false),
            3.0
        );
        context.capital = true;
        assert_eq!(
            Simulation::native_defense_effect_value(&context, false, false),
            11.0
        );
        context.capital = false;
        context.ocean_city = true;
        assert_eq!(
            Simulation::native_defense_effect_value(&context, false, true),
            7.0
        );
        context.ocean_threat = true;
        assert_eq!(
            Simulation::native_defense_effect_value(&context, false, true),
            19.0
        );
    }

    #[test]
    fn static_defender_research_is_gated_by_real_production_need_and_keeps_police_value() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.native_defensive_effects_playtest(),
            (true, true, true, true)
        );
        game.units.clear();
        assert_eq!(
            game.production_defender_tech_want(&game.cities[0], Tech::BRONZE_WORKING),
            0.0
        );
        game.cities[0].size = 3;
        game.cities[0].specialists[2] = 1;
        let police = game.production_defender_tech_want(&game.cities[0], Tech::BRONZE_WORKING);
        assert!((police - 174.27983).abs() < 0.001);
        game.players[1].known.push(Tech::BRONZE_WORKING);
        assert_eq!(
            game.production_defender_tech_want(&game.cities[0], Tech::BRONZE_WORKING),
            0.0
        );
    }

    #[test]
    fn high_tax_actual_upkeep_does_not_make_null_context_type_upkeep_positive() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
        }
        game.players[1].known = vec![Tech::BRONZE_WORKING];
        game.players[1].government = Government::Despotism;
        game.players[1].rates = [40, 60, 0];
        game.create_city(1, 10, 24);
        game.cities[0].buildings.push(Building::BARRACKS);
        let city = game.cities[0].clone();
        let charge = game.spawn_unit(1, UnitKind::Catapult, 10, 24);
        game.request_ai_guard(charge);
        for _ in 0..20 {
            game.spawn_unit(1, UnitKind::Warrior, 10, 24);
            game.units.last_mut().unwrap().home_city = Some(city.id);
        }
        assert!(!game.city_gold_upkeep(&city).is_empty());
        assert_eq!(
            game.bodyguard_production_choice(&city, false)
                .map(|choice| choice.0),
            Some(UnitKind::Phalanx)
        );
    }

    #[test]
    fn defense_baseline_counts_untrusted_armistice_and_trusted_ceasefire_cities() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.native_defensive_effects_playtest(),
            (true, true, true, true)
        );
        game.units.clear();
        let other = offset_native_by_map(10, 24, 4, 0);
        game.create_city(HUMAN, other.0, other.1);
        game.sign_pact(1, HUMAN, Pact::Peace);
        assert_eq!(game.pact(1, HUMAN), Pact::Armistice);
        assert!(game.advisor_player_dangerous(1, HUMAN));
        assert_eq!(
            game.defensive_building_effect_want(&game.cities[0], Building::CITY_WALLS),
            83.0
        );
        game.diplomacy_playtest_friendship();
        // The reusable friendship setup also creates a disliked alternate
        // rival. Isolate this baseline comparison to the treaty partner.
        game.units.retain(|unit| unit.owner != 2);
        game.cities.retain(|city| city.owner != 2);
        assert!(!game.advisor_player_dangerous(1, HUMAN));
        assert_eq!(
            game.defensive_building_effect_want(&game.cities[0], Building::CITY_WALLS),
            3.0
        );
        game.sign_pact(1, HUMAN, Pact::Ceasefire);
        assert!(game.advisor_player_dangerous(1, HUMAN));
        assert_eq!(
            game.defensive_building_effect_want(&game.cities[0], Building::CITY_WALLS),
            83.0
        );
    }

    #[test]
    fn desktop_city_defense_fixture_checks_independent_native_values() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.native_city_defense_advisor_playtest(),
            (true, true, true, true)
        );
    }

    #[test]
    fn city_defense_percent_matches_literal_native_feedback_and_urgency() {
        let assessment = |danger, defense, urgency| GuardCityAssessment {
            danger,
            defense,
            urgency,
            grave: 0,
            defense_igwall: defense,
        };
        assert_eq!(
            Simulation::native_city_defense_danger(assessment(10, 400, 3), 1),
            25
        );
        assert_eq!(
            Simulation::native_city_defense_danger(assessment(30, 400, 3), 1),
            203
        );
        assert_eq!(
            Simulation::native_city_defense_danger(assessment(30, 400, 0), 1),
            100
        );
        assert_eq!(
            Simulation::native_city_defense_danger(assessment(30, 0, 11), 0),
            211
        );
        assert_eq!(
            Simulation::native_city_defense_danger(assessment(30, 400, 3), 0),
            0
        );
        assert_eq!(
            Simulation::native_city_defense_danger(assessment(0, 0, 0), 1),
            0
        );
    }

    #[test]
    fn quiet_empty_city_has_no_defense_bid_but_entertainer_can_request_police() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.river = false;
            tile.base = None;
        }
        game.create_city(1, 10, 24);
        assert!(game.native_city_defense_choice(&game.cities[0]).0.is_none());
        game.cities[0].size = 3;
        game.cities[0].specialists[2] = 1;
        let (choice, martial) = game.native_city_defense_choice(&game.cities[0]);
        assert_eq!(martial, 35.0);
        assert_eq!(choice.unwrap().want, 35.0);
        game.players[1].government = Government::Communism;
        assert_eq!(game.city_martial_law_want(&game.cities[0]), 35.0);
        // Native unit_list_size counts civilians too for the maximum check.
        for _ in 0..3 {
            game.spawn_unit(1, UnitKind::Caravan, 10, 24);
        }
        assert_eq!(game.city_martial_law_want(&game.cities[0]), 0.0);
    }

    #[test]
    fn defense_building_uses_native_provider_order_and_counts_all_tile_units() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.native_city_defense_advisor_playtest(),
            (true, true, true, true)
        );
        game.units.clear();
        game.cities[0].size = 1;
        game.cities[0].specialists = [0; 3];
        game.players[1].known.push(Tech::MASONRY);
        game.spawn_unit(1, UnitKind::Phalanx, 10, 24);
        for _ in 0..11 {
            game.spawn_unit(1, UnitKind::Caravan, 10, 24);
        }
        let threat = offset_native_by_map(10, 24, 1, 0);
        game.spawn_unit(HUMAN, UnitKind::Cannon, threat.0, threat.1);
        let city = &game.cities[0];
        assert_eq!(
            game.defense_building_provider(city, None),
            Some(Building::CITY_WALLS)
        );
        let assessment = game.advisor_guard_city_assessment(city);
        assert_eq!(assessment.urgency, 11);
        assert_eq!(assessment.defense, 100);
        let choice = game.native_city_defense_choice(city).0.unwrap();
        assert_eq!(
            choice.production,
            Production::Building(Building::CITY_WALLS)
        );
        assert_eq!(choice.want, 105.5);
    }

    #[test]
    fn big_land_city_defense_does_not_receive_terrain_defense() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        game.tile_mut(10, 24).river = false;
        game.tile_mut(10, 24).base = None;
        game.create_city(1, 10, 24);
        let city = game.cities[0].clone();
        for kind in [UnitKind::Chariot, UnitKind::Catapult, UnitKind::Cannon] {
            game.units.clear();
            game.spawn_unit(1, kind, 10, 24);
            game.tile_mut(10, 24).terrain = Terrain::Grassland;
            let plain = game.advisor_guard_city_assessment(&city).defense;
            game.tile_mut(10, 24).terrain = Terrain::Hills;
            assert!(plain > 0);
            assert_eq!(
                game.advisor_guard_city_assessment(&city).defense,
                plain,
                "provided Big Land class omits TerrainDefense"
            );
        }
    }

    #[test]
    fn suicide_attack_actions_contribute_missile_city_danger() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.diplomacy.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.base = None;
        }
        game.create_city(1, 10, 24);
        let city = game.cities[0].clone();
        let near = offset_native_by_map(10, 24, 1, 0);
        game.spawn_unit(HUMAN, UnitKind::CruiseMissile, near.0, near.1);
        let danger = game.advisor_guard_city_assessment(&city);
        assert_eq!(
            (danger.danger, danger.urgency, danger.grave),
            (3600, 0, 0),
            "Suicide Attack has native ACTRES_ATTACK"
        );
        game.units.clear();
        game.players[HUMAN].known.push(Tech::ADVANCED_FLIGHT);
        game.spawn_unit(HUMAN, UnitKind::Nuclear, near.0, near.1);
        assert_eq!(
            game.advisor_guard_city_assessment(&city).danger,
            108900,
            "native tactical action assessment scans enablers without action-blocker checks"
        );
        let coast = offset_native_by_map(10, 24, -1, 0);
        game.tile_mut(coast.0, coast.1).terrain = Terrain::Ocean;
        game.spawn_unit(1, UnitKind::AegisCruiser, 10, 24);
        assert_eq!(
            game.advisor_guard_city_assessment(&city).danger,
            108900,
            "Nuclear lacks the AirAttacker flag; Aegis bonus must not apply"
        );
        game.units
            .iter_mut()
            .find(|u| u.owner == HUMAN)
            .unwrap()
            .kind = UnitKind::CruiseMissile;
        assert_eq!(
            game.advisor_guard_city_assessment(&city).danger,
            289,
            "Cruise Missile has AirAttacker; native specific defense bonus is damped to250%"
        );
    }

    #[test]
    fn source_training_replaces_routine_advice_ignores_investment_and_respects_grave_emergency() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.military_training_advisor_playtest(),
            (true, true, true, true)
        );
    }

    #[test]
    fn routine_military_gates_skip_training_but_non_grave_high_want_can_train() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.diplomacy.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.resource = None;
            tile.base = None;
        }
        game.players[1].known = vec![Tech::BRONZE_WORKING];
        game.players[1].government = Government::Despotism;
        game.tile_mut(10, 24).terrain = Terrain::Hills;
        game.tile_mut(10, 24).mine = true;
        game.create_city(1, 10, 24);
        let city = game.cities[0].clone();
        // A two-turn threat is urgent enough to request the first defender,
        // but native grave-danger early return does not apply yet.
        let far = offset_native_by_map(10, 24, 2, 0);
        game.spawn_unit(HUMAN, UnitKind::Marines, far.0, far.1);
        assert_eq!(game.city_grave_danger(&city), 0);
        let (before, after) = game.military_build_advice(&city);
        assert!(before.is_some_and(|c| c.want >= MILITARY_EMERGENCY_WANT));
        assert!(
            after.is_some_and(|c| c.production == Production::Building(Building::BARRACKS)
                && c.want == f64::from((before.unwrap().want * 0.5) as f32))
        );
        game.units.clear();
        let charge = game.spawn_unit(1, UnitKind::Catapult, 10, 24);
        game.request_ai_guard(charge);
        game.players[1].known.push(Tech::MASONRY);
        game.cities[0].production = Production::Building(Building::PYRAMIDS);
        assert!(!game.routine_military_advice_allowed(&game.cities[0]));
        assert!(
            game.military_build_advice(&game.cities[0]).1.is_none(),
            "wonder-city stage returns before bodyguard/training advice"
        );
        game.cities[0].production = Production::Unit(UnitKind::Warrior);
        game.players[1].government = Government::Republic;
        for _ in 0..5 {
            game.spawn_unit(1, UnitKind::Warrior, 10, 24);
            game.units.last_mut().unwrap().home_city = Some(city.id);
        }
        assert_eq!(game.city_forecast(&game.cities[0]).shield_surplus(), 0);
        assert!(game.military_build_advice(&game.cities[0]).1.is_none());
        game.units.retain(|u| u.id == charge);
        game.units[0].home_city = None;
        game.cities[0].size = 4;
        game.cities[0].specialists = [0, 0, 2]; // two happy workers remain after luxury
        let abroad = offset_native_by_map(10, 24, 8, 0);
        for _ in 0..3 {
            game.spawn_unit(1, UnitKind::Warrior, abroad.0, abroad.1);
            game.units.last_mut().unwrap().home_city = Some(city.id);
        }
        let forecast = game.city_forecast(&game.cities[0]);
        let (_, effect, _) = game.city_feelings_after_effects(&game.cities[0], forecast.luxury);
        let (_, final_unhappy, _) = game.city_feelings(&game.cities[0], forecast.luxury);
        assert!(
            forecast.shield_surplus() > 0 && final_unhappy > effect,
            "orderly positive-production city must still be gated by unrest added after effects"
        );
        assert!(
            game.military_build_advice(&game.cities[0]).1.is_none(),
            "unrest introduced after effects prevents non-life-saving advice"
        );
    }

    #[test]
    fn training_sources_match_effect_classes_availability_and_global_veterans() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
        }
        game.create_city(1, 10, 24);
        game.players[1].known.clear();
        let input = |kind| AiBuildChoice {
            production: Production::Unit(kind),
            want: 93.3,
        };
        let choose = |game: &Simulation, kind| {
            let mut choice = input(kind);
            game.adjust_military_training_choice(&game.cities[0], &mut choice);
            choice
        };
        assert_eq!(
            choose(&game, UnitKind::Phalanx).production,
            Production::Building(Building::BARRACKS)
        );
        assert_eq!(
            choose(&game, UnitKind::Phalanx).want,
            46.650001525878906,
            "native f32 assignment then Builder integer division"
        );
        game.players[1].known.push(Tech::GUNPOWDER);
        assert_eq!(
            choose(&game, UnitKind::Musketeers).production,
            Production::Building(Building::BARRACKS_II)
        );
        game.players[1].known.push(Tech::MOBILE_WARFARE);
        assert_eq!(
            choose(&game, UnitKind::Marines).production,
            Production::Building(Building::BARRACKS_III)
        );
        assert_eq!(
            choose(&game, UnitKind::Fighter).production,
            Production::Unit(UnitKind::Fighter),
            "no Radio means no Airport provider"
        );
        game.players[1].known.push(Tech::RADIO);
        for kind in [
            UnitKind::Fighter,
            UnitKind::Helicopter,
            UnitKind::CruiseMissile,
            UnitKind::Nuclear,
        ] {
            assert_eq!(
                choose(&game, kind).production,
                Production::Building(Building::AIRPORT)
            );
        }
        // Native max veteran level zero does not block Nuclear's provider search.
        assert_eq!(game.trained_rank(&game.cities[0], UnitKind::Nuclear), 0);
        game.cities[0].buildings.push(Building::AIRPORT);
        assert_eq!(
            choose(&game, UnitKind::Fighter).production,
            Production::Unit(UnitKind::Fighter)
        );
        let coast = offset_native_by_map(10, 24, 1, 0);
        game.tile_mut(coast.0, coast.1).terrain = Terrain::Ocean;
        game.players[1].known.push(Tech::AMPHIBIOUS_WARFARE);
        assert_eq!(
            choose(&game, UnitKind::Destroyer).production,
            Production::Building(Building::PORT_FACILITY)
        );
        assert_eq!(
            choose(&game, UnitKind::Carrier).production,
            Production::Building(Building::PORT_FACILITY),
            "zero attack is not NonMil"
        );
        for kind in [
            UnitKind::Caravel,
            UnitKind::Transport,
            UnitKind::Diplomat,
            UnitKind::Worker,
        ] {
            assert_eq!(
                choose(&game, kind).production,
                Production::Unit(kind),
                "NonMil choices are exempt"
            );
        }
        game.cities[0].buildings.push(Building::LIGHTHOUSE);
        assert_eq!(
            choose(&game, UnitKind::Destroyer).production,
            Production::Unit(UnitKind::Destroyer)
        );
        game.cities[0]
            .buildings
            .retain(|b| *b != Building::LIGHTHOUSE);
        game.cities[0]
            .buildings
            .push(Building::MAGELLANS_EXPEDITION);
        assert_eq!(
            choose(&game, UnitKind::Destroyer).production,
            Production::Building(Building::PORT_FACILITY),
            "combat promotion bonus is not veteran construction"
        );
        game.cities[0].buildings.push(Building::PORT_FACILITY);
        assert_eq!(
            choose(&game, UnitKind::Destroyer).production,
            Production::Unit(UnitKind::Destroyer)
        );
        let mut building = AiBuildChoice {
            production: Production::Building(Building::TEMPLE),
            want: 100.0,
        };
        game.adjust_military_training_choice(&game.cities[0], &mut building);
        assert_eq!(building.production, Production::Building(Building::TEMPLE));
    }

    #[test]
    fn bodyguard_production_keeps_null_type_upkeep_and_emergency_precedence() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.ai_bodyguard_production_playtest(),
            (true, true, true, true)
        );
    }

    #[test]
    fn sea_bodyguard_requires_defendgood_role_and_compatible_charge() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.diplomacy.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.owner = None;
            tile.hut = false;
        }
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        game.create_city(1, 10, 24);
        game.players[1].known = Tech::all().collect();
        let city = game.cities[0].clone();
        let charge = game.spawn_unit(1, UnitKind::Transport, 10, 24);
        game.request_ai_guard(charge);
        assert_eq!(
            game.bodyguard_production_choice(&city, true)
                .map(|(kind, _)| kind),
            Some(UnitKind::AegisCruiser)
        );
        game.units.iter_mut().find(|u| u.id == charge).unwrap().kind = UnitKind::Trireme;
        assert!(
            game.bodyguard_production_choice(&city, true).is_none(),
            "Sea Aegis cannot follow Trireme's River-native class"
        );
    }

    #[test]
    fn small_wonder_projects_government_center_effects_across_empire() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.river = true;
        }
        game.create_city(1, 5, 8);
        game.create_city(1, 20, 40);
        game.create_city(1, 25, 40);
        game.players[1].known = vec![Tech::THEOLOGY];
        game.players[1].government = Government::Despotism;
        game.players[1].rates = [80, 20, 0];
        for city in &mut game.cities {
            city.size = 4;
        }
        game.reconcile_city_workers();
        game.cities[1].production = Production::Building(Building::ECCLESIASTICAL_PALACE);
        let city = game.cities[1].clone();
        let original = game.advisor_city_output(&game.cities[2], None);
        let mut scratch = game.clone();
        let want = game.projected_wonder_want(&mut scratch, &city, Building::ECCLESIASTICAL_PALACE);
        assert!(want > 0.0);
        scratch.cities[1]
            .buildings
            .push(Building::ECCLESIASTICAL_PALACE);
        assert!(scratch.advisor_city_output(&scratch.cities[2], None) > original);
        assert_eq!(
            game.wonder_build_choice(&city).unwrap().production,
            Production::Building(Building::ECCLESIASTICAL_PALACE)
        );
        game.players[1].known.push(Tech::TRADE);
        assert!(game.unit_available(1, UnitKind::Caravan));
        assert!(
            game.wonder_helper_production_choice(&game.cities[2])
                .is_none()
        );
        game.cities[2]
            .buildings
            .push(Building::ECCLESIASTICAL_PALACE);
        assert!(game.wonder_build_choice(&city).is_none_or(
            |choice| choice.production != Production::Building(Building::ECCLESIASTICAL_PALACE)
        ));
    }

    #[test]
    fn wonder_production_and_helpers_compete_using_empire_effects() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_wonder_advisor_playtest());
    }

    #[test]
    fn projected_wonder_counts_other_cities_and_restores_scratch_state() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.river = true;
        }
        game.create_city(1, 10, 24);
        game.players[1].known = game.research_path(1, Tech(41) /* Literacy */);
        game.players[1].rates = [80, 20, 0];
        game.cities[0].buildings.push(Building::LIBRARY);
        let city = game.cities[0].clone();
        let mut scratch = game.clone();
        let before = bincode::serialize(&scratch).unwrap();
        let single = game.projected_wonder_want(&mut scratch, &city, Building::GREAT_LIBRARY);
        assert_eq!(bincode::serialize(&scratch).unwrap(), before);
        game.create_city(1, 20, 24);
        game.cities[1].size = 5;
        game.cities[1].buildings.push(Building::LIBRARY);
        game.reconcile_city_workers();
        scratch = game.clone();
        let empire = game.projected_wonder_want(&mut scratch, &city, Building::GREAT_LIBRARY);
        assert!(empire > single);
        // Empire-wide surviving science boost is still projected when the
        // building city itself has no extra workers or specialist output.
        assert!(!game.completed_wonders.contains(&Building::GREAT_LIBRARY));
        assert!(!game.cities[0].has_building(Building::GREAT_LIBRARY));
    }

    #[test]
    fn wonder_helpers_respect_seven_turn_route_and_queued_contributions() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_wonder_advisor_playtest());
        // Fixture ends with enough helpers; moving them onto another
        // continent removes their contribution to this continent's count.
        let id = game.ai_wonder_city(1).unwrap();
        let helper_city = game
            .cities
            .iter()
            .find(|city| city.id != id)
            .unwrap()
            .clone();
        game.units
            .retain(|unit| !matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight));
        assert!(game.wonder_helper_production_choice(&helper_city).is_some());
        let wonder_city = game
            .cities
            .iter()
            .find(|city| city.id == id)
            .unwrap()
            .clone();
        for tile in &mut game.tiles {
            tile.road = false;
            tile.river = false;
        }
        let mut distant = helper_city.clone();
        (distant.x, distant.y) = offset_native_by_map(wonder_city.x, wonder_city.y, 20, 15);
        assert!(game.wonder_helper_production_choice(&distant).is_none());
    }

    #[test]
    fn profitable_tactical_aircraft_are_excluded_from_production_by_native_hp_loss() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (fighter, victim) = game.ai_aircraft_advisor_playtest_setup();
        // Keep the profitable industrial scenario independent of the new
        // Despotism default's per-tile output penalty.
        game.players[1].government = Government::Communism;
        game.units.retain(|unit| unit.id != fighter);
        let target = game
            .units
            .iter()
            .find(|unit| unit.id == victim)
            .unwrap()
            .clone();
        // Naval stacks lose only their defender. Use an exposed land stack
        // whose native stack-loss benefit really grows with its value.
        game.units.retain(|unit| unit.id != victim);
        game.tile_mut(target.x, target.y).terrain = Terrain::Grassland;
        for _ in 0..20 {
            game.spawn_unit(HUMAN, UnitKind::Marines, target.x, target.y);
        }
        // Keep a positive tactical opportunity independently of the native
        // unit-type HP-loss check that excludes offensive production.
        let home = game.cities[0].clone();
        let workers: Vec<_> = Simulation::city_radius(&home)
            .filter(|&(_, _, x, y)| (x, y) != (target.x, target.y))
            .map(|(_, _, x, y)| {
                let tile = game.tile_mut(x, y);
                tile.terrain = Terrain::Hills;
                tile.mine = true;
                tile.owner = Some(1);
                (x, y)
            })
            .filter(|&position| position != (home.x, home.y))
            .take(4)
            .collect();
        game.cities[0].worked_tiles = workers;
        game.cities[0].buildings.extend([
            Building::TEMPLE,
            Building::CATHEDRAL,
            Building::AMPHITHEATER,
            Building::BARRACKS,
            Building::BARRACKS_II,
            Building::BARRACKS_III,
            Building::AIRPORT,
            Building::PORT_FACILITY,
        ]);
        let invasion = offset_native_by_map(home.x, home.y, 1, 0);
        game.tile_mut(invasion.0, invasion.1).terrain = Terrain::Grassland;
        game.spawn_unit(HUMAN, UnitKind::Marines, invasion.0, invasion.1);
        let city = game.cities[0].clone();
        let mut tactical = game.make_unit(0, 1, UnitKind::Fighter, city.x, city.y);
        tactical.home_city = Some(city.id);
        tactical.veteran = game.trained_rank(&city, UnitKind::Fighter);
        assert!(game.aircraft_target_value(&tactical, target.x, target.y) > 0.0);
        assert!(game.aircraft_production_choice(&city).is_none());
        let defense = game.native_city_defense_choice(&city).0.unwrap();
        assert!(defense.want >= MILITARY_EMERGENCY_WANT);
        assert!(game.advisor_guard_city_assessment(&city).grave > 0);
        let choice = game.ai_build_choice(&city).unwrap();
        // A Fighter can itself be a CT_DEFENDER. Compare the role's exact
        // bid, rather than assuming every aircraft kind is an attacker.
        assert_eq!(choice.production, defense.production);
        assert_eq!(choice.want, defense.want);
    }

    #[test]
    fn wonder_city_requires_a_helper_cluster_and_rejects_immediate_danger() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Plains;
            tile.road = true;
            tile.river = true;
            tile.hut = false;
            tile.known_by[1] = true;
        }
        for (dx, dy) in [(0, 0), (2, 0), (4, 0), (0, 2), (2, 2)] {
            let (x, y) = offset_native_by_map(10, 24, dx, dy);
            game.create_city(1, x, y);
        }
        assert_eq!(game.ai_wonder_city(1), None);
        let (x, y) = offset_native_by_map(10, 24, 4, 2);
        game.create_city(1, x, y);
        game.players[1].known.extend([Tech::TRADE, Tech::POTTERY]);
        let chosen = game.ai_wonder_city(1).expect("six connected helper cities");
        let city = game
            .cities
            .iter()
            .find(|city| city.id == chosen)
            .unwrap()
            .clone();
        let (x, y) = offset_native_by_map(city.x, city.y, 1, 0);
        game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
        assert_ne!(game.ai_wonder_city(1), Some(chosen));
    }

    #[test]
    fn productive_wonder_city_retains_project_and_suppresses_space_parts() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Plains;
        }
        game.create_city(1, 10, 24);
        game.players[1].known = Tech::all().collect();
        game.cities[0].production = Production::Building(Building::APOLLO_PROGRAM);
        let city = game.cities[0].clone();
        assert_eq!(game.ai_wonder_city(1), Some(city.id));
        assert_eq!(
            game.space_production_want(&city, Building::SPACE_MODULE),
            24.0
        );
        game.spaceships[1].built[0] = 1;
        assert_eq!(
            game.space_production_want(&city, Building::SPACE_MODULE),
            72.0
        );
        game.cities[0].production = Production::Unit(UnitKind::Warrior);
        assert_eq!(game.ai_wonder_city(1), None);
    }

    #[test]
    fn space_parts_need_a_competitive_empire_or_an_active_race() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Hills;
            tile.mine = true;
        }
        game.create_city(1, 10, 24);
        game.create_city(HUMAN, 20, 24);
        game.cities[1].size = 4;
        game.reconcile_city_workers();
        // Rival leads technology and manufacturing (ties go to player zero).
        game.players[HUMAN].known = Tech::all().collect();
        game.cities[0].specialists = [1, 0, 0];
        game.cities[0].worked_tiles.clear();
        let city = game.cities[0].clone();
        assert_eq!(
            game.space_production_want(&city, Building::SPACE_MODULE),
            0.0
        );
        assert_eq!(
            game.space_production_want(&city, Building::APOLLO_PROGRAM),
            10.0
        );
        game.spaceships[HUMAN].built[0] = 1;
        assert_eq!(
            game.space_production_want(&city, Building::SPACE_MODULE),
            210.0
        );
        game.spaceships[1].built[0] = 1;
        assert_eq!(
            game.space_production_want(&city, Building::SPACE_MODULE),
            630.0
        );
        game.spaceships[HUMAN].built = [0; 3];
        game.spaceships[1].built = [0; 3];
        game.players[1].known = Tech::all().collect();
        game.players[1].future_techs = 1;
        assert_eq!(
            game.space_production_want(&city, Building::SPACE_MODULE),
            210.0
        );
        assert_eq!(
            game.space_production_want(&city, Building::APOLLO_PROGRAM),
            160.0
        );
    }

    #[test]
    fn military_training_scales_with_army_but_not_when_next_tech_obsoletes_it() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.create_city(1, 10, 24);
        let city = game.cities[0].clone();
        let small_army = game.domestic_improvement_want(&city, Building::BARRACKS);
        for _ in 0..40 {
            game.spawn_unit(1, UnitKind::Warrior, 10, 24);
        }
        let large_army = game.domestic_improvement_want(&city, Building::BARRACKS);
        assert!(large_army > small_army && large_army > 0.0);
        game.players[1].known = game
            .research_path(1, Tech::GUNPOWDER)
            .into_iter()
            .filter(|&tech| tech != Tech::GUNPOWDER)
            .collect();
        assert_eq!(
            game.domestic_improvement_want(&city, Building::BARRACKS),
            0.0
        );
    }

    #[test]
    fn domestic_wants_penalize_discarding_invested_unit_production() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(1, 10, 24);
        let mut city = game.cities[0].clone();
        city.production = Production::Unit(UnitKind::Warrior);
        city.shields = 0;
        let uninvested = game.domestic_improvement_want(&city, Building::GRANARY);
        city.shields = 60;
        let invested = game.domestic_improvement_want(&city, Building::GRANARY);
        assert!(invested < uninvested);
        city.production = Production::Building(Building::TEMPLE);
        assert_eq!(
            game.domestic_improvement_want(&city, Building::GRANARY),
            uninvested
        );
    }

    #[test]
    fn growth_constrained_city_prefers_aqueduct_to_available_expansion() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_production_priorities_playtest());
    }

    #[test]
    fn production_compares_domestic_and_military_wants_on_one_scale() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Plains;
            tile.river = true;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.hut = false;
            tile.known_by[1] = true;
        }
        game.create_city(1, 10, 24);
        game.cities[0].size = 3;
        game.cities[0].food = Simulation::growth_box(3) - 1;
        game.players[1]
            .known
            .extend([Tech::POTTERY, Tech::GUNPOWDER]);
        game.spawn_unit(1, UnitKind::Musketeers, 10, 24);
        let (x, y) = offset_native_by_map(10, 24, 4, 0);
        game.create_city(HUMAN, x, y);
        let domestic = game.domestic_build_choice(&game.cities[0]).unwrap();
        let military = game.military_build_advice(&game.cities[0]).1.unwrap();
        let chosen = game.ai_build_choice(&game.cities[0]).unwrap();
        assert!(chosen.want >= domestic.want);
        assert!(chosen.want >= military.want);
        assert_eq!(
            game.choose_ai_production(1, game.cities[0].id),
            chosen.production
        );

        game.ai_war_plans.insert(
            (1, HUMAN),
            super::super::diplomacy::WarPlan {
                countdown: 2,
                reason: super::super::diplomacy::WarReason::Space,
            },
        );
        assert_eq!(
            game.ai_build_choice(&game.cities[0]).unwrap().production,
            military.production,
            "war footing retains military advice, including its training-building replacement"
        );
    }

    #[test]
    fn routine_rush_preserves_reserve_and_requires_invested_shields() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.players[1].known.push(Tech::POTTERY);
        game.create_city(1, 20, 20);
        game.turn += 1;
        game.cities[0].production = Production::Building(Building::GRANARY);
        game.players[1].gold = 500;
        game.spend_ai_defense_gold(1);
        assert!(!game.cities[0].bought_production);

        game.cities[0].shields = 1;
        let price = game.rush_cost(&game.cities[0]).unwrap();
        let reserve = 2 * game.ai_gold_reserve(1);
        game.players[1].gold = price + reserve - 1;
        game.spend_ai_defense_gold(1);
        assert!(!game.cities[0].bought_production);

        game.players[1].gold += 1;
        game.spend_ai_defense_gold(1);
        assert!(game.cities[0].bought_production);
        assert_eq!(game.players[1].gold, reserve);
    }

    #[test]
    fn routine_settler_rush_requires_no_war_preparation_and_enough_population() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.create_city(1, 20, 20);
        game.turn += 1;
        game.cities[0].production = Production::Unit(UnitKind::Settlers);
        game.cities[0].shields = 1;
        game.cities[0].size = UnitKind::Settlers.stats().pop_cost;
        game.players[1].gold = 500;
        game.spend_ai_defense_gold(1);
        assert!(!game.cities[0].bought_production);

        game.cities[0].size += 1;
        game.create_city(HUMAN, 5, 5);
        game.sign_pact(1, HUMAN, Pact::War);
        game.ai_war_plans.insert(
            (1, HUMAN),
            super::super::diplomacy::WarPlan {
                countdown: 0,
                reason: super::super::diplomacy::WarReason::Space,
            },
        );
        game.spend_ai_defense_gold(1);
        assert!(!game.cities[0].bought_production);

        game.sign_pact(1, HUMAN, Pact::Peace);
        game.ai_war_plans.remove(&(1, HUMAN));
        let price = game.rush_cost(&game.cities[0]).unwrap();
        let reserve = game.ai_gold_reserve(1);
        game.players[1].gold = price + reserve;
        game.spend_ai_defense_gold(1);
        assert!(game.cities[0].bought_production);
        assert_eq!(game.players[1].gold, reserve);
    }

    #[test]
    fn city_output_priorities_raise_the_bid_for_a_library_technology() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
        game.players[HUMAN].known = vec![Tech::ALPHABET, writing];
        game.players[1].known = vec![Tech::ALPHABET];
        let no_cities = game.tech_sell_price(HUMAN, 1, writing).unwrap();
        game.create_city(1, 20, 20);
        game.cities.last_mut().unwrap().size = 3;
        game.cities.last_mut().unwrap().specialists = [2, 0, 0];
        assert_eq!(game.tech_sell_price(HUMAN, 1, writing).unwrap(), no_cities);
        game.ai_tech_wants.insert(1, game.advisor_tech_wants(1));
        assert!(game.tech_sell_price(HUMAN, 1, writing).unwrap() > no_cities);
        assert!(!game.has_tech(1, writing));
    }

    #[test]
    fn a_reachable_enemy_raises_demand_for_a_defender_upgrade() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[HUMAN].known = vec![Tech::BRONZE_WORKING];
        game.players[1].known.clear();
        let (x, y) = (20, 20);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.create_city(1, x, y);
        game.ai_tech_wants.insert(1, game.advisor_tech_wants(1));
        let peaceful_price = game
            .tech_sell_price(HUMAN, 1, Tech::BRONZE_WORKING)
            .unwrap();
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(nx, ny).terrain = Terrain::Grassland;
        game.tile_mut(nx, ny).known_by[1] = true;
        game.spawn_unit(HUMAN, UnitKind::Legion, nx, ny);
        assert_eq!(
            game.tech_sell_price(HUMAN, 1, Tech::BRONZE_WORKING)
                .unwrap(),
            peaceful_price
        );
        game.ai_tech_wants.insert(1, game.advisor_tech_wants(1));
        assert!(
            game.tech_sell_price(HUMAN, 1, Tech::BRONZE_WORKING)
                .unwrap()
                > peaceful_price
        );
    }
    #[test]
    fn defender_forecast_preserves_stronger_veterans_and_production_heals_estimates() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        game.create_city(HUMAN, 10, 24);
        game.players[HUMAN].known = vec![Tech::GUNPOWDER];
        let city = game.cities[0].clone();
        let id = game.spawn_unit(HUMAN, UnitKind::Marines, city.x, city.y);
        game.units.last_mut().unwrap().veteran = 3;
        let attacker = game.make_unit(0, 1, UnitKind::Chariot, city.x, city.y);
        assert_eq!(
            game.anticipated_city_defender(&city, &attacker, 3, false)
                .unwrap()
                .id,
            id
        );
        game.units.last_mut().unwrap().hp = 1;
        let actual = game
            .anticipated_city_defender(&city, &attacker, 1, false)
            .unwrap();
        let production = game
            .anticipated_city_defender(&city, &attacker, 1, true)
            .unwrap();
        assert_eq!(actual.hp, 1);
        assert_eq!(production.hp, production.kind.stats().hp);
        assert_eq!(game.units.last().unwrap().hp, 1);
    }
    #[test]
    fn counter_defender_selection_accounts_for_attacker_class_and_naval_city_penalty() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
        }
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        game.create_city(HUMAN, 10, 24);
        game.players[HUMAN].known = vec![Tech::GUNPOWDER, Tech::ROCKETRY];
        let city = &game.cities[0];
        let air = game.make_unit(0, 1, UnitKind::Fighter, city.x, city.y);
        let land = game.make_unit(0, 1, UnitKind::Legion, city.x, city.y);
        assert_eq!(
            game.defender_versus(city, &air).unwrap().kind,
            UnitKind::AegisCruiser
        );
        assert_eq!(
            game.defender_versus(city, &land).unwrap().kind,
            UnitKind::Musketeers
        );
        assert_eq!(
            game.production_city_defender(city, 100),
            Some(UnitKind::AegisCruiser),
            "production ranks generic type desirability, while the tactical land counter is Musketeers"
        );
    }
    #[test]
    fn threat_arrival_uses_source_pf_floor_and_retains_road_turn_zero() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.river = false;
            tile.railroad = false;
            tile.known_by[1] = true;
        }
        let home = (10, 24);
        game.create_city(1, home.0, home.1);
        let attack = offset_native_by_map(home.0, home.1, 1, 0);
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, attack.0, attack.1);
        let turn = game
            .city_threat_arrivals(&game.cities[0])
            .find(|(u, _, _)| u.id == id)
            .unwrap()
            .2;
        assert_eq!(turn, 1);
        game.tile_mut(attack.0, attack.1).road = true;
        let turn = game
            .city_threat_arrivals(&game.cities[0])
            .find(|(u, _, _)| u.id == id)
            .unwrap()
            .2;
        // Enemy-owned infrastructure is unavailable to the approaching
        // Warrior: the city's final step still costs a full movement point.
        assert_eq!(turn, 1);
        game.units.iter_mut().find(|u| u.id == id).unwrap().kind = UnitKind::Horseman;
        let turn = game
            .city_threat_arrivals(&game.cities[0])
            .find(|(u, _, _)| u.id == id)
            .unwrap()
            .2;
        assert_eq!(turn, 0);
        let third = offset_native_by_map(home.0, home.1, 3, 0);
        let unit = game.units.iter_mut().find(|u| u.id == id).unwrap();
        unit.kind = UnitKind::Warrior;
        (unit.x, unit.y) = third;
        game.tile_mut(attack.0, attack.1).road = false;
        let turn = game
            .city_threat_arrivals(&game.cities[0])
            .find(|(u, _, _)| u.id == id)
            .unwrap()
            .2;
        assert_eq!(turn, 3);
        assert!(game.city_defense_production(&game.cities[0]).is_some());
    }
    #[test]
    fn native_production_defender_keeps_source_cost_fallback_order_and_ignores_training() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.river = false;
            tile.base = None;
        }
        game.players[1].known = vec![Tech::GUNPOWDER, Tech::AMPHIBIOUS_WARFARE];
        game.create_city(1, 10, 24);
        assert_eq!(
            game.production_city_defender(&game.cities[0], 100),
            Some(UnitKind::Marines)
        );
        game.cities[0].buildings.push(Building::BARRACKS_II);
        assert_eq!(
            game.production_city_defender(&game.cities[0], 100),
            Some(UnitKind::Marines)
        );
        // Knights occur AFTER Marines in native simple_ai_types. At stock0
        // the cheaper40-shield candidate replaces a60-shield best even though
        // it has lower static desirability; at stock20 this fallback ends.
        game.players[1].known.push(Tech::CHIVALRY);
        assert_eq!(
            game.production_city_defender(&game.cities[0], 100),
            Some(UnitKind::Knights)
        );
        game.cities[0].shields = 20;
        assert_eq!(
            game.production_city_defender(&game.cities[0], 100),
            Some(UnitKind::Marines)
        );
        // Fuelled DefendGood aircraft are not in the source simple list.
        game.players[1].known.extend([Tech::FLIGHT, Tech::STEALTH]);
        assert!(
            !game
                .production_city_defender(&game.cities[0], 100)
                .unwrap()
                .is_aircraft()
        );
    }

    #[test]
    fn production_defender_keeps_generic_choice_with_or_without_training() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (stock, quiet, _) = game.ai_city_defense_playtest();
        assert!(stock && quiet);
        assert_eq!(
            game.native_city_defense_choice(&game.cities[0])
                .0
                .unwrap()
                .production,
            Production::Unit(UnitKind::Marines)
        );
        game.cities[0]
            .buildings
            .retain(|&b| b != Building::BARRACKS_II);
        assert_eq!(
            game.city_defense_production(&game.cities[0]),
            Some(Production::Unit(UnitKind::Marines))
        );
    }

    #[test]
    fn offensive_choice_amortizes_balanced_risk_even_with_existing_shield_stock() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (selected, advanced, blocked) = game.offensive_production_playtest();
        assert!(selected && advanced && blocked);
    }
    #[test]
    fn transported_troops_and_paradrops_enter_city_danger_without_illegal_moves() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_embarked_paradrop_danger_playtest());
    }

    #[test]
    fn cargo_uses_carrier_speed_and_disembark_delay_and_can_outpace_walking() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.river = false;
            tile.base = None;
        }
        let home = (10, 24);
        game.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
        game.create_city(1, home.0, home.1);
        let sea = offset_native_by_map(home.0, home.1, -4, 0);
        let ferry = game.spawn_unit(HUMAN, UnitKind::Transport, sea.0, sea.1);
        let troop = game.spawn_unit(HUMAN, UnitKind::Warrior, sea.0, sea.1);
        assert!(game.board_specific_ship(troop, ferry));
        let city = game.cities[0].clone();
        let soldier = game.units.iter().find(|u| u.id == troop).unwrap();
        assert!(game.danger_route_turns(soldier, &city).is_none());
        assert_eq!(game.city_danger_arrival(soldier, &city), Some(1));
        let far = offset_native_by_map(home.0, home.1, 25, 0);
        for unit in game
            .units
            .iter_mut()
            .filter(|u| u.id == troop || u.id == ferry)
        {
            (unit.x, unit.y) = far;
        }
        let soldier = game.units.iter().find(|u| u.id == troop).unwrap();
        assert!(game.city_danger_arrival(soldier, &city).is_none());
    }

    #[test]
    fn paradrop_estimate_uses_source_integer_range_and_full_movement_assumption() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
        }
        let home = (10, 24);
        game.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
        game.create_city(1, home.0, home.1);
        let city = game.cities[0].clone();
        for (distance, expected) in [(9, 0), (10, 1), (20, 2)] {
            let point = offset_native_by_map(home.0, home.1, distance, 0);
            let para = game.make_unit(0, HUMAN, UnitKind::Paratroopers, point.0, point.1);
            assert_eq!(game.city_danger_arrival(&para, &city), Some(expected));
        }
        let point = offset_native_by_map(home.0, home.1, 10, 0);
        let mut para = game.make_unit(0, HUMAN, UnitKind::Paratroopers, point.0, point.1);
        for step in 0..=10 {
            let (x, y) = offset_native_by_map(home.0, home.1, step, 0);
            let tile = game.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.railroad = true;
        }
        para.moves_left = 0;
        assert_eq!(game.city_danger_arrival(&para, &city), Some(1));
    }
    #[test]
    fn danger_counter_choice_assumes_the_next_full_turn_without_mutating_the_enemy() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_embarked_paradrop_danger_playtest());
        game.players[1]
            .known
            .extend([Tech::GUNPOWDER, Tech::AMPHIBIOUS_WARFARE]);
        let enemy = game
            .units
            .iter_mut()
            .find(|u| u.kind == UnitKind::Paratroopers)
            .unwrap();
        enemy.moves_left = 0;
        let id = enemy.id;
        assert_eq!(
            game.city_defense_production(&game.cities[0]),
            Some(Production::Unit(UnitKind::Marines))
        );
        assert_eq!(
            game.units.iter().find(|u| u.id == id).unwrap().moves_left,
            0
        );
    }
    #[test]
    fn happiness_effects_are_independent_stage_capped_and_range_aware() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_happiness_effects_playtest());
    }

    #[test]
    fn aqueduct_health_has_value_and_science_fraction_is_preserved() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.create_city(1, 10, 24);
        let city = &game.cities[0];
        assert_eq!(game.building_health_want(city, Building::AQUEDUCT), 5.0);
        let mut redundant = city.clone();
        redundant.buildings.push(Building::AQUEDUCT);
        assert_eq!(
            game.building_health_want(&redundant, Building::AQUEDUCT_RIVER),
            0.0
        );
        assert_eq!(
            game.building_health_want(&redundant, Building::AQUEDUCT_LAKE),
            0.0
        );
        assert_eq!(
            game.advisor_city_output_exact(city, None),
            f64::from(game.advisor_city_output(city, None))
        );
    }
    #[test]
    fn science_priority_truncates_to_twenty_one_before_source_float_accumulation() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        let tile = game.tile_mut(10, 24);
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
        tile.irrigation = false;
        tile.mine = false;
        tile.river = true;
        tile.pollution = false;
        tile.fallout = false;
        game.players[1].government = Government::Communism;
        game.players[1].rates = [100, 0, 0];
        game.players[1].researching = Tech::BRONZE_WORKING;
        game.create_city(1, 10, 24);
        game.cities[0].worked_tiles.clear();
        assert_eq!(game.city_forecast(&game.cities[0]).gross_science, 2);
        assert_eq!(game.advisor_city_output_exact(&game.cities[0], None), 149.0);
        game.players[1].researching = Tech::FUTURE;
        assert_eq!(game.advisor_city_output_exact(&game.cities[0], None), 109.0);
    }
}
