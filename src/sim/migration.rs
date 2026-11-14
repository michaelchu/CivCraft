//! Freeciv C2C3 citizen migration between nearby cities.
//!
//! Freeciv keeps migration disabled in its server settings by default. The
//! engine settings are represented explicitly here so a campaign can enable
//! them without changing the selected ruleset. The ruleset contributes the
//! city `Migration_Pct` effect and `migration_pml` parameter.
//!
//! Behavior follows `server/cityturn.c::city_migration_score`,
//! `do_city_migration`, and `check_city_migrations_player` at Freeciv revision
//! `9ff184b6eb128ca636c7ad6269e27aa959600006`, with culture from
//! `common/culture.c::city_culture`.

use super::*;
use crate::rules::Genus;
use crate::rules::freeciv::{RuleParameter, RulesetData};
use std::collections::HashMap;

/// Freeciv server settings that control the city migration pass.
///
/// These are engine settings, not C2C3 ruleset parameters. The C2C3 ruleset's
/// migration baseline is therefore disabled until campaign configuration
/// explicitly opts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MigrationSettings {
    pub(crate) enabled: bool,
    pub(crate) turn_interval: i32,
    pub(crate) food_needed: bool,
    pub(crate) distance: i32,
    pub(crate) nation_chance: i32,
    pub(crate) world_chance: i32,
}

impl MigrationSettings {
    /// Pinned Freeciv server defaults for the selected C2C3 ruleset.
    pub(crate) const fn c2c3_default() -> Self {
        Self {
            enabled: false,
            turn_interval: 5,
            food_needed: true,
            distance: 0,
            nation_chance: 50,
            world_chance: 10,
        }
    }
}

impl Default for MigrationSettings {
    fn default() -> Self {
        Self::c2c3_default()
    }
}

#[derive(Clone, Copy, Debug)]
struct MigrationCandidate {
    city_id: u32,
    score: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct MigrationCandidates {
    national: Option<MigrationCandidate>,
    international: Option<MigrationCandidate>,
}

#[derive(Clone, Copy, Debug)]
struct MigrationScoreInputs {
    size: i32,
    happy: i32,
    unhappy: i32,
    angry: i32,
    building_shield_cost: i32,
    trade: i32,
    luxury: i32,
    science: i32,
    culture: i64,
    culture_migration_pml: i32,
    food_surplus: i32,
    illness: i32,
    has_wonder: bool,
    is_capital: bool,
    migration_pct: i32,
}

/// Resolve one migration phase for all cities.
///
/// The turn coordinator should call this after city production and population
/// resolution, at Freeciv's `srv_main.c` city migration phase (after barbarian
/// summoning and before disasters). `true` reports a successful international
/// migration, matching `check_city_migrations()`'s return contract.
impl Simulation {
    pub(crate) fn check_city_migrations(&mut self, settings: MigrationSettings) -> bool {
        if !settings.enabled
            || settings.turn_interval <= 0
            || (settings.nation_chance <= 0 && settings.world_chance <= 0)
        {
            return false;
        }

        // Freeciv caches a city's migration score for the whole turn. A
        // migration can change its source and destination, but later checks
        // continue to use the score each city had the first time it was
        // evaluated during this phase.
        let migration_scores = self
            .cities
            .iter()
            .map(|city| (city.id, self.city_migration_score(city)))
            .collect::<HashMap<_, _>>();

        let mut international_migration = false;
        // The source iterates players and each player's city list safely. City
        // IDs let us retain stable storage order while tolerating disbanding.
        for owner in 0..self.players.len() {
            let city_ids: Vec<_> = self
                .cities
                .iter()
                .filter(|city| city.owner == owner)
                .map(|city| city.id)
                .collect();
            for city_id in city_ids {
                let Some(source) = self.city_by_id(city_id).cloned() else {
                    continue;
                };
                if self.is_migration_capital(&source)
                    || self.turn == source.founded_turn
                    || (self.turn - source.founded_turn) % settings.turn_interval != 0
                {
                    continue;
                }

                let candidates = self.migration_candidates(&source, settings, &migration_scores);
                if let Some(candidate) = candidates.national {
                    // Freeciv rolls only after finding a better same-owner
                    // destination, and gives that choice precedence over an
                    // international destination even when its roll fails.
                    if self.rand_below(100) < settings.nation_chance.clamp(0, 100)
                        && self.do_city_migration(city_id, candidate.city_id, settings)
                    {
                        // The public result intentionally excludes this case.
                    }
                    continue;
                }

                if let Some(candidate) = candidates.international
                    && self.rand_below(100) < settings.world_chance.clamp(0, 100)
                {
                    international_migration = true;
                    self.do_city_migration(city_id, candidate.city_id, settings);
                }
            }
        }
        international_migration
    }

    fn migration_candidates(
        &self,
        source: &City,
        settings: MigrationSettings,
        migration_scores: &HashMap<u32, f64>,
    ) -> MigrationCandidates {
        let source_score = migration_scores[&source.id] * 3.0;
        let max_distance = 5_i32.saturating_add(settings.distance.max(0));
        // The source iterates map tiles outward from the city center. CivCraft
        // adapts that traversal to its staggered Civ III graph; this preserves
        // deterministic distance-ring order without relying on city storage.
        let outward_positions = self.map_wrap.civ3_radius(source.x, source.y, max_distance);
        let city_at = self
            .cities
            .iter()
            .map(|city| (self.map_wrap.index(city.x, city.y), city))
            .collect::<HashMap<_, _>>();
        let mut best = MigrationCandidates::default();

        for (x, y, _) in outward_positions {
            let Some(destination) = city_at.get(&self.map_wrap.index(x, y)) else {
                continue;
            };
            if destination.id == source.id {
                continue;
            }
            let migration_distance = (self.city_work_radius_sq(destination).max(0) as f64).sqrt()
                as i32
                + settings.distance.max(0);
            let distance = self.map_distance(source.x, source.y, destination.x, destination.y);
            if distance > migration_distance {
                continue;
            }

            let weight =
                f64::from(migration_distance + 1 - distance) / f64::from(migration_distance + 1);
            let mut score = migration_scores[&destination.id] * weight;
            if destination.owner == source.owner {
                if settings.nation_chance <= 0 {
                    continue;
                }
                if score > source_score && best.national.is_none_or(|current| score > current.score)
                {
                    best.national = Some(MigrationCandidate {
                        city_id: destination.id,
                        score,
                    });
                }
            } else {
                if settings.world_chance <= 0 {
                    continue;
                }
                if selected_citizen_nationality()
                    && source
                        .foreign_citizens
                        .iter()
                        .any(|&(nation, count)| nation == destination.owner && count > 0)
                {
                    score *= 2.0;
                }
                if score > source_score
                    && best
                        .international
                        .is_none_or(|current| score > current.score)
                {
                    best.international = Some(MigrationCandidate {
                        city_id: destination.id,
                        score,
                    });
                }
            }
        }
        best
    }

    fn city_migration_score(&self, city: &City) -> f64 {
        let forecast = self.city_forecast(city);
        let (happy, unhappy, angry) = self.city_feelings(city, forecast.luxury);
        let building_shield_cost = city
            .buildings
            .iter()
            .map(|building| building.data().cost.max(0))
            .sum();
        let has_wonder = city.buildings.iter().any(|building| {
            matches!(
                building.data().genus,
                Genus::SmallWonder | Genus::GreatWonder
            )
        });
        let illness = self.city_illness(city, forecast.shields);
        let migration_pct = self.city_rule_effect(
            city,
            "Migration_Pct",
            None,
            None,
            None,
            false,
            self.players[city.owner].government,
            None,
        );
        migration_score(MigrationScoreInputs {
            size: city.size,
            happy,
            unhappy,
            angry,
            building_shield_cost,
            trade: forecast.trade,
            luxury: forecast.luxury,
            science: forecast.science,
            culture: self.city_culture_score(city),
            culture_migration_pml: selected_migration_pml(),
            food_surplus: forecast.food_surplus(city.size, forecast.support.food),
            illness,
            has_wonder,
            is_capital: self.is_migration_capital(city),
            migration_pct,
        })
    }

    fn is_migration_capital(&self, city: &City) -> bool {
        self.city_rule_effect(
            city,
            "Capital_City",
            None,
            None,
            None,
            false,
            self.players[city.owner].government,
            None,
        ) > 0
    }

    fn destination_can_accept_migrant(&self, destination: &City, food_needed: bool) -> bool {
        if destination.size + 1 > self.city_size_limit(destination) {
            return false;
        }
        if !food_needed {
            return true;
        }

        let forecast = self.city_forecast(destination);
        let surplus = forecast.food_surplus(destination.size, forecast.support.food);
        let food_cost = Self::citizen_food_cost();
        if surplus >= food_cost {
            return true;
        }

        self.map_city_radius(destination)
            .filter(|&(_, _, x, y)| self.can_assign_city_tile(destination, x, y))
            .map(|(_, _, x, y)| {
                self.c2c3_city_tile_yields(
                    destination,
                    x,
                    y,
                    false,
                    false,
                    None,
                    self.players[destination.owner].government,
                )
                .0
            })
            .max()
            .is_some_and(|best_food| surplus.saturating_add(best_food) >= food_cost)
    }

    fn do_city_migration(
        &mut self,
        source_id: u32,
        destination_id: u32,
        settings: MigrationSettings,
    ) -> bool {
        let (Some(source_index), Some(destination_index)) = (
            self.city_index_by_id(source_id),
            self.city_index_by_id(destination_id),
        ) else {
            return false;
        };
        let source = self.cities[source_index].clone();
        let destination = self.cities[destination_index].clone();
        if source.id == destination.id
            || !self.destination_can_accept_migrant(&destination, settings.food_needed)
        {
            return false;
        }

        // Freeciv selects the outgoing citizen before deciding whether a
        // size-one source can disband. Preserve that deterministic RNG order,
        // including when the city contains only foreign citizens.
        let nationality = self.migrant_nationality(&source, destination.owner);

        // A size-one city can be disbanded only when another city of the same
        // owner can take its supported units. Its wonders must never be lost.
        let replacement_city = if source.size == 1 {
            if source.buildings.iter().any(|building| {
                matches!(
                    building.data().genus,
                    Genus::SmallWonder | Genus::GreatWonder
                )
            }) {
                return false;
            }
            self.cities
                .iter()
                .filter(|city| city.owner == source.owner && city.id != source.id)
                .min_by_key(|city| {
                    (
                        self.map_distance(source.x, source.y, city.x, city.y),
                        city.id,
                    )
                })
                .map(|city| city.id)
        } else {
            None
        };
        if source.size == 1 && replacement_city.is_none() {
            return false;
        }

        if source.size == 1 {
            let replacement_id = replacement_city.expect("checked above");
            for unit in &mut self.units {
                if unit.owner == source.owner && unit.home_city == Some(source_id) {
                    unit.home_city = Some(replacement_id);
                }
            }
            self.remove_city_for_population_loss(source_id);
        } else {
            let index = self
                .city_index_by_id(source_id)
                .expect("source city exists before migration");
            let city = &mut self.cities[index];
            // Freeciv halves the food stock before reducing population.
            city.food /= 2;
            city.size -= 1;
            city.remove_citizen_assignments(1, self.map_wrap);
            remove_one_citizen_nationality(city, nationality);
            city.food = city.food.min(Self::growth_box(city.size));
            let city_id = city.id;
            self.rebuild_city_ownership_for_city_event(city_id);
        }

        let destination_index = self
            .city_index_by_id(destination_id)
            .expect("destination survives migration");
        let growth_retained = self.growth_food_retained(&self.cities[destination_index]);
        let destination_owner = self.cities[destination_index].owner;
        let destination_name = self.cities[destination_index].name.clone();
        {
            let destination = &mut self.cities[destination_index];
            destination.size += 1;
            destination.specialists[2] += 1;
            destination.food = destination.food.max(percent_of(
                Self::growth_box(destination.size),
                growth_retained,
            ));
            add_one_citizen_nationality(destination, nationality);
        }
        self.reconcile_city_workers();
        self.rebuild_city_ownership_for_city_event(destination_id);

        if source.owner == HUMAN || destination_owner == HUMAN {
            self.add_event(format!(
                "A citizen migrated from {} to {}.",
                source.name, destination_name
            ));
        }
        true
    }

    fn migrant_nationality(&mut self, source: &City, destination_owner: usize) -> usize {
        if source.size == 1 {
            // Freeciv samples the sole citizen before checking whether the
            // source can be disbanded. The `fc_rand(1)` call is a no-op, but
            // the selected nationality must remain the sole resident's.
            return source
                .foreign_citizens
                .iter()
                .find_map(|&(nationality, count)| (count > 0).then_some(nationality))
                .unwrap_or(source.owner);
        }
        if selected_citizen_nationality()
            && source
                .foreign_citizens
                .iter()
                .any(|&(nation, count)| nation == destination_owner && count > 0)
        {
            return destination_owner;
        }
        if source.domestic_citizens() > 0 {
            return source.owner;
        }
        let foreign_total: i32 = source
            .foreign_citizens
            .iter()
            .map(|&(_, count)| count.max(0))
            .sum();
        if foreign_total <= 0 {
            return source.owner;
        }
        let mut remaining = self.rand_below(foreign_total);
        for &(nationality, count) in &source.foreign_citizens {
            let count = count.max(0);
            if remaining < count {
                return nationality;
            }
            remaining -= count;
        }
        source.owner
    }
}

fn migration_score(inputs: MigrationScoreInputs) -> f64 {
    let mut score = f64::from(inputs.size) + f64::from(inputs.happy)
        - 0.25 * f64::from(inputs.unhappy)
        - 0.50 * f64::from(inputs.angry);
    score *= 1.0 + (1.0 - (-(inputs.building_shield_cost.max(0) as f64) / 1000.0).exp()) / 5.0;
    for surplus in [inputs.trade, inputs.luxury, inputs.science] {
        score *= 1.0 + (1.0 - (-(surplus.max(0) as f64) / 100.0).exp()) / 5.0;
    }
    score += (inputs
        .culture
        .max(0)
        .saturating_mul(i64::from(inputs.culture_migration_pml.max(0)))
        / 1000) as f64;
    score *= 1.0 + f64::from(inputs.food_surplus.clamp(-10, 20)) / 10.0;
    score *= 100.0 - f64::from(inputs.illness) / 25.0;
    if inputs.has_wonder {
        score *= 1.25;
    }
    if inputs.is_capital {
        score *= 1.25;
    }
    score * (1.0 + f64::from(inputs.migration_pct) / 100.0)
}

fn remove_one_citizen_nationality(city: &mut City, nationality: usize) {
    if nationality == city.owner {
        return;
    }
    if let Ok(index) = city
        .foreign_citizens
        .binary_search_by_key(&nationality, |&(nation, _)| nation)
    {
        city.foreign_citizens[index].1 -= 1;
        if city.foreign_citizens[index].1 <= 0 {
            city.foreign_citizens.remove(index);
        }
    }
}

fn add_one_citizen_nationality(city: &mut City, nationality: usize) {
    if nationality == city.owner {
        return;
    }
    match city
        .foreign_citizens
        .binary_search_by_key(&nationality, |&(nation, _)| nation)
    {
        Ok(index) => city.foreign_citizens[index].1 += 1,
        Err(index) => city.foreign_citizens.insert(index, (nationality, 1)),
    }
}

fn selected_migration_pml() -> i32 {
    selected_integer_parameter("game.ruleset", "culture", "migration_pml")
}

pub(crate) fn selected_integer_parameter(file: &str, section: &str, key: &str) -> i32 {
    let rules = RulesetData::campaign_default();
    let parameter = selected_parameter(rules, file, section, key)
        .unwrap_or_else(|| panic!("selected C2C3 ruleset is missing {file} [{section}].{key}"));
    let value = parameter
        .integer_value()
        .unwrap_or_else(|error| panic!("invalid C2C3 parameter {}: {error}", parameter.key))
        .unwrap_or_else(|| panic!("C2C3 parameter {} has no value", parameter.key));
    i32::try_from(value)
        .unwrap_or_else(|_| panic!("C2C3 parameter {} does not fit in i32", parameter.key))
}

pub(crate) fn selected_citizen_nationality() -> bool {
    selected_parameter(
        RulesetData::campaign_default(),
        "cities.ruleset",
        "citizen",
        "nationality",
    )
    .map(|parameter| {
        parameter
            .boolean_value()
            .expect("selected [citizen].nationality is a boolean")
            .expect("selected [citizen].nationality is present")
    })
    .unwrap_or(false)
}

fn selected_parameter<'a>(
    rules: &'a RulesetData,
    file: &str,
    section: &str,
    key: &str,
) -> Option<&'a RuleParameter> {
    let mut matching = rules.parameters.iter().filter(|parameter| {
        parameter.file.eq_ignore_ascii_case(file)
            && parameter.section.eq_ignore_ascii_case(section)
            && parameter.key.eq_ignore_ascii_case(key)
    });
    let found = matching.next();
    assert!(
        matching.next().is_none(),
        "selected C2C3 ruleset repeats {file} [{section}].{key}"
    );
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_inputs() -> MigrationScoreInputs {
        MigrationScoreInputs {
            size: 10,
            happy: 2,
            unhappy: 1,
            angry: 1,
            building_shield_cost: 0,
            trade: 0,
            luxury: 0,
            science: 0,
            culture: 0,
            culture_migration_pml: 50,
            food_surplus: 0,
            illness: 0,
            has_wonder: false,
            is_capital: false,
            migration_pct: 0,
        }
    }

    #[test]
    fn c2c3_engine_defaults_keep_migration_disabled() {
        assert_eq!(
            MigrationSettings::default(),
            MigrationSettings {
                enabled: false,
                turn_interval: 5,
                food_needed: true,
                distance: 0,
                nation_chance: 50,
                world_chance: 10,
            }
        );
    }

    #[test]
    fn migration_score_applies_feelings_and_each_freeciv_multiplier_in_order() {
        let inputs = MigrationScoreInputs {
            building_shield_cost: 1000,
            trade: 100,
            luxury: 100,
            science: 100,
            culture: 200,
            food_surplus: 20,
            illness: 250,
            has_wonder: true,
            is_capital: true,
            migration_pct: 10,
            ..base_inputs()
        };
        let base = 10.0 + 2.0 - 0.25 - 0.5;
        let saturation = 1.0 + (1.0 - (-1.0_f64).exp()) / 5.0;
        let expected = ((((base * saturation.powi(4) + 10.0) * 3.0) * 90.0) * 1.25 * 1.25) * 1.1;
        assert!((migration_score(inputs) - expected).abs() < 1e-9);
    }

    #[test]
    fn migration_score_ignores_negative_outputs_and_clamps_food() {
        let mut negative_outputs = base_inputs();
        negative_outputs.trade = -50;
        negative_outputs.luxury = -50;
        negative_outputs.science = -50;
        let mut zero_outputs = base_inputs();
        assert_eq!(
            migration_score(negative_outputs),
            migration_score(zero_outputs)
        );

        zero_outputs.food_surplus = 30;
        let mut upper_clamp = base_inputs();
        upper_clamp.food_surplus = 20;
        assert_eq!(migration_score(zero_outputs), migration_score(upper_clamp));
    }

    #[test]
    fn selected_c2c3_migration_parameter_and_nationality_are_present() {
        assert_eq!(selected_migration_pml(), 50);
        assert!(selected_citizen_nationality());
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let mut city = game.cities[0].clone();
        assert_eq!(game.city_work_radius_sq(&city), 5);
        city.size = 20;
        assert_eq!(game.city_work_radius_sq(&city), 5);
    }

    #[test]
    fn culture_migration_term_truncates_like_freeciv_integer_arithmetic() {
        let mut fractional = base_inputs();
        fractional.size = 0;
        fractional.happy = 0;
        fractional.unhappy = 0;
        fractional.angry = 0;
        fractional.culture = 19;
        fractional.culture_migration_pml = 50;
        let mut zero = fractional;
        zero.culture = 0;

        assert_eq!(migration_score(fractional), migration_score(zero));
    }

    #[test]
    fn selected_culture_cohort_wins_except_for_single_citizen_source() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.create_city(1, 10, 20);
        let mut source = game.cities[0].clone();
        source.size = 1;
        source.foreign_citizens = vec![(2, 1)];
        assert_eq!(game.migrant_nationality(&source, HUMAN), 2);

        source.size = 4;
        source.foreign_citizens = vec![(HUMAN, 1)];
        assert_eq!(game.migrant_nationality(&source, HUMAN), HUMAN);
    }

    fn enabled_migration_settings() -> MigrationSettings {
        MigrationSettings {
            enabled: true,
            turn_interval: 1,
            food_needed: false,
            distance: 8,
            nation_chance: 100,
            world_chance: 100,
        }
    }

    #[test]
    fn national_migration_moves_population_and_halves_source_food() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.units.clear();
        game.create_city(HUMAN, 12, 20);
        game.create_city(HUMAN, 10, 20);
        game.turn = 6;
        let source_id = game.cities[1].id;
        let destination_id = game.cities[0].id;
        game.cities[1].size = 4;
        game.cities[1].food = 50;
        game.cities[1].founded_turn = 5;
        game.cities[1].foreign_citizens = vec![(2, 1)];
        game.cities[0].size = 7;
        game.cities[0].history = 20_000;

        assert!(!game.check_city_migrations(MigrationSettings {
            world_chance: 0,
            ..enabled_migration_settings()
        }));

        let source = game.city_by_id(source_id).unwrap();
        let destination = game.city_by_id(destination_id).unwrap();
        assert_eq!(source.size, 3);
        assert_eq!(source.foreign_citizens, [(2, 1)]);
        assert_eq!(source.food, 25.min(Simulation::growth_box(3)));
        assert_eq!(destination.size, 8);
    }

    #[test]
    fn international_migration_prefers_the_destination_nationality() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.units.clear();
        game.create_city(1, 2, 2);
        game.create_city(1, 10, 20);
        game.create_city(HUMAN, 16, 20);
        game.turn = 6;
        game.cities[1].size = 4;
        game.cities[1].founded_turn = 5;
        game.cities[1].foreign_citizens = vec![(HUMAN, 1)];
        game.cities[2].size = 7;
        game.cities[2].history = 20_000;
        let source_id = game.cities[1].id;
        let destination_id = game.cities[2].id;

        assert!(game.check_city_migrations(MigrationSettings {
            nation_chance: 0,
            ..enabled_migration_settings()
        }));

        let source = game.city_by_id(source_id).unwrap();
        let destination = game.city_by_id(destination_id).unwrap();
        assert_eq!(source.size, 3);
        assert!(source.foreign_citizens.is_empty());
        assert_eq!(destination.size, 8);
        assert!(destination.foreign_citizens.is_empty());
    }

    #[test]
    fn size_one_migration_preserves_sole_nationality_and_rehomes_supported_units() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.units.clear();
        game.create_city(1, 2, 2);
        game.create_city(1, 10, 20);
        game.create_city(HUMAN, 16, 20);
        let replacement_id = game.cities[0].id;
        let source_id = game.cities[1].id;
        let destination_id = game.cities[2].id;
        game.cities[1].size = 1;
        game.cities[1].foreign_citizens = vec![(2, 1)];
        game.cities[2].size = 7;
        game.cities[2].history = 20_000;
        let unit = game.spawn_unit(1, UnitKind::Warrior, 10, 20);
        let unit_index = game.unit_index_by_id(unit).unwrap();
        game.units[unit_index].home_city = Some(source_id);

        assert!(game.do_city_migration(source_id, destination_id, enabled_migration_settings()));

        assert!(game.city_by_id(source_id).is_none());
        assert_eq!(
            game.units[game.unit_index_by_id(unit).unwrap()].home_city,
            Some(replacement_id)
        );
        assert_eq!(game.city_by_id(destination_id).unwrap().size, 8);
        assert_eq!(
            game.city_by_id(destination_id).unwrap().foreign_citizens,
            [(2, 1)]
        );
    }

    #[test]
    fn equal_migration_scores_keep_first_destination_in_map_order() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.create_city(1, 2, 2);
        game.create_city(1, 10, 20);
        game.create_city(1, 8, 20);
        game.create_city(1, 12, 20);
        game.turn = 6;
        game.cities[1].size = 2;
        game.cities[1].founded_turn = 5;
        for city in &mut game.cities[2..] {
            city.size = 7;
            city.history = 20_000;
        }
        let source = game.cities[1].clone();
        let scores = game
            .cities
            .iter()
            .map(|city| (city.id, game.city_migration_score(city)))
            .collect::<HashMap<_, _>>();
        let candidate = game
            .migration_candidates(&source, enabled_migration_settings(), &scores)
            .national
            .unwrap();

        assert_eq!(game.city_by_id(candidate.city_id).unwrap().x, 12);
    }

    #[test]
    fn migration_candidate_scores_are_a_per_turn_snapshot() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.create_city(1, 2, 2);
        game.create_city(1, 10, 20);
        game.create_city(1, 12, 20);
        game.turn = 6;
        game.cities[1].size = 2;
        game.cities[1].founded_turn = 5;
        game.cities[2].size = 2;
        let source = game.cities[1].clone();
        let before_migration_scores = game
            .cities
            .iter()
            .map(|city| (city.id, game.city_migration_score(city)))
            .collect::<HashMap<_, _>>();
        game.cities[2].history = 20_000;

        assert!(
            game.migration_candidates(
                &source,
                enabled_migration_settings(),
                &before_migration_scores
            )
            .national
            .is_none()
        );
        let updated_scores = game
            .cities
            .iter()
            .map(|city| (city.id, game.city_migration_score(city)))
            .collect::<HashMap<_, _>>();
        assert!(
            game.migration_candidates(&source, enabled_migration_settings(), &updated_scores)
                .national
                .is_some()
        );
    }

    #[test]
    fn migration_obeys_the_selected_city_size_limit() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.create_city(1, 2, 2);
        game.create_city(1, 10, 20);
        game.create_city(HUMAN, 12, 20);
        let source_id = game.cities[1].id;
        let destination_id = game.cities[2].id;
        game.cities[1].size = 4;
        game.cities[2].size = game.city_size_limit(&game.cities[2]);

        assert!(!game.do_city_migration(source_id, destination_id, enabled_migration_settings()));
        assert_eq!(game.city_by_id(source_id).unwrap().size, 4);
        assert_eq!(
            game.city_by_id(destination_id).unwrap().size,
            game.city_size_limit(game.city_by_id(destination_id).unwrap())
        );
    }
}
