//! Coordinates the ordered transition from one campaign turn to the next.
//!
//! `end_turn` is the phase orchestrator: it completes pending revolutions,
//! runs end-phase unit automation and city processing, applies world events,
//! advances diplomacy and the calendar, then progresses unit activities and
//! runs AI planning for the new phase. The order is gameplay behavior: later
//! phases observe earlier mutations, and movement/activity budgets cross the
//! turn boundary deliberately.
//!
//! Keep individual rule calculations in their owning modules and call them
//! here in the reference-compatible sequence. Save and playtest behavior also
//! depends on this ordering.

use super::*;
use crate::rules::freeciv::{
    EffectContext, EffectEvaluationError, Requirement, RequirementContext,
    RequirementEvaluationError, RequirementRange, RequirementValue, RulesetData, VictoryCondition,
    effect_value,
};

impl Simulation {
    pub(crate) fn victory_enabled(&self, condition: VictoryCondition) -> bool {
        RulesetData::campaign_default()
            .victory_enabled(condition)
            .unwrap_or_else(|error| panic!("invalid campaign victories setting: {error}"))
    }

    fn advance_calendar(&mut self) {
        let ruleset = RulesetData::campaign_default();
        let (year, year_0_hack) =
            next_calendar_state(self.year, self.year_0_hack, &self.global_advances, ruleset)
                .unwrap_or_else(|error| panic!("campaign calendar evaluation failed: {error:?}"));
        self.year = year;
        self.year_0_hack = year_0_hack;
    }

    fn advance_turn_clock(&mut self) {
        self.advance_diplomacy();
        self.advance_calendar();
        self.turn += 1;
    }

    pub(in crate::sim) fn end_turn(&mut self) {
        if self.game_over.is_some() {
            return;
        }
        // srv_main:end_phase also enacts changes made during this phase.
        self.finish_revolutions();
        let automated: Vec<u32> = self
            .player_units_ordered(HUMAN)
            .filter(|u| u.automated && !u.kind.stats().can_work)
            .map(|u| u.id)
            .collect();
        for id in automated {
            self.run_automated_unit(id);
        }
        // srv_main::end_phase runs auto_workers_player before AI
        // last_activities, immediately before city activities.
        for owner in 0..self.civilization_count {
            if !self.player_alive(owner) {
                continue;
            }
            self.run_ai_worker_phase(owner);
            if owner != HUMAN {
                self.run_ai_last_activities(owner);
            }
        }
        self.run_ai_worker_phase(BARBARIANS);
        self.ai_free_bulbs.clear();
        self.prune_ai_player_units();
        // Route connectivity is an actor-free CivCraft adapter for C2C3's
        // Caravan/Freight delivery step. Resolve it globally after all worker
        // and last-activity changes, before any city calculates output.
        self.reconcile_trade_routes();
        self.establish_connected_trade_routes();
        for owner in 0..self.players.len() {
            if !self.player_alive(owner) || (owner < BARBARIANS && owner >= self.civilization_count)
            {
                continue;
            }
            self.restore_player_units(owner);
            let income = self.process_player_cities(owner);
            self.finish_player_national_activities(owner, income);
            let banks = self
                .cities
                .iter()
                .filter(|city| city.owner == owner && city.has_building(Building::BANK))
                .count();
            if banks >= 5
                && self
                    .cities
                    .iter()
                    .any(|city| city.owner == owner && city.has_building(Building::WALL_STREET))
            {
                self.players[owner].gold += self.players[owner].gold.max(0) / 20;
            }
        }
        // Freeciv summons barbarians at end_turn(), after city activity. They
        // first move in ai_start_phase() at the following begin_phase().
        self.summon_barbarians();
        self.check_city_migrations(crate::sim::migration::MigrationSettings::c2c3_default());
        self.check_disasters();
        // srv_main.c::end_turn evaluates achievement cohorts after disasters
        // and before the global-warming/nuclear-winter phase.
        self.check_achievements();
        self.update_climate();
        self.refresh_diplomatic_contact_effects();
        // srv_main.c::end_turn advances diplomacy and the calendar after
        // migrations, disasters, achievements, climate, and other world
        // changes. check_for_game_over() runs immediately after end_turn().
        self.advance_turn_clock();
        // srv_main.c::check_for_game_over runs after end_turn() but before the
        // next begin_turn() refreshes score. Preserve its precedence: game
        // state, allied victory, culture victory, turn limit, spaceship.
        self.check_game_over();
        if self.game_over.is_some() {
            return;
        }
        if self.check_allied_victory() {
            return;
        }
        if self.victory_enabled(VictoryCondition::Culture) {
            self.check_cultural_victory();
        }
        if self.game_over.is_some() {
            return;
        }
        if self.check_turn_limit() {
            return;
        }
        self.check_space_race_arrivals();
        if self.game_over.is_some() {
            return;
        }
        // begin_turn() calculates score before begin-phase activities and AI.
        self.refresh_score_snapshot();
        // srv_main:begin_phase progresses activities with the previous
        // movement budget, then refills MP. Revolution completion is also
        // checked here, after activities (update_revolution at line 1381).
        for owner in 0..self.players.len() {
            self.progress_player_unit_activities(owner);
        }
        self.finish_revolutions();
        for city in &mut self.cities {
            city.airlift_used = false;
        }
        self.refresh_visibility();
        self.run_civilization_begin_phase_ai();
        self.run_barbarian_begin_phase();
        self.continue_player_gotos();
        self.rebuild_city_ownership();
        // Historical field name is retained for save compatibility; hunter
        // interception needs previous positions of land and sea units alike.
        self.previous_naval_positions = self
            .units
            .iter()
            .map(|unit| (unit.id, (unit.x, unit.y)))
            .collect();
        self.hunter_targets.retain(|hunter, target| {
            self.previous_naval_positions.contains_key(hunter)
                && self.previous_naval_positions.contains_key(target)
        });
        self.add_event(format!(
            "Turn {} begins - {}.",
            self.turn,
            year_label(self.year)
        ));
    }

    /// Runs Freeciv's `ai_start_phase()` for the campaign civilizations after
    /// unit activities, movement-point refresh, and visibility have updated.
    fn run_civilization_begin_phase_ai(&mut self) {
        for owner in 1..self.civilization_count {
            if self.run_ai_player_planning(owner) {
                self.prune_ai_player_units();
            }
        }
    }

    /// Freeciv's `ai_start_phase()` runs barbarian unit planning after the
    /// end-turn uprising and after begin-phase activities and visibility.
    fn run_barbarian_begin_phase(&mut self) {
        self.run_ai_player_planning(BARBARIANS);
        self.prune_ai_player_units();
    }
}

struct CalendarContext<'a> {
    year: i32,
    global_advances: &'a [Tech],
    ruleset: &'a RulesetData,
}

impl RequirementContext for CalendarContext<'_> {
    fn is_present(&self, requirement: &Requirement) -> Result<bool, RequirementEvaluationError> {
        let unsupported = || RequirementEvaluationError::Unsupported {
            kind: requirement.kind.clone(),
            name: requirement.name.clone(),
        };
        if requirement.range != RequirementRange::World {
            return Err(unsupported());
        }
        if requirement.kind.eq_ignore_ascii_case("MinYear") {
            let threshold = match &requirement.name {
                RequirementValue::String(value) => value.parse::<i32>().ok(),
                RequirementValue::Integer(value) => i32::try_from(*value).ok(),
                RequirementValue::Bool(_) => None,
            }
            .ok_or_else(unsupported)?;
            return Ok(self.year >= threshold);
        }
        if requirement.kind.eq_ignore_ascii_case("Tech") && requirement.survives {
            let name = match &requirement.name {
                RequirementValue::String(value) => value,
                _ => return Err(unsupported()),
            };
            let binding = super::content_adapter::bind_freeciv_technology(self.ruleset, name)
                .map_err(|_| unsupported())?;
            return Ok(match binding {
                super::content_adapter::FreecivTechBinding::Bound(tech) => {
                    self.global_advances.contains(&tech)
                }
                super::content_adapter::FreecivTechBinding::MissingFromCiv3Catalog => false,
            });
        }
        Err(unsupported())
    }
}

impl EffectContext for CalendarContext<'_> {
    fn multiplier_percent(&self, _multiplier: &str) -> Option<i64> {
        None
    }
}

/// Apply the selected ruleset's calendar effects to one transition. The
/// current C2C3 campaign sets `fragments=0`; nonzero fragments require their
/// own persistent counter before they can be enabled safely.
fn next_calendar_state(
    current_year: i32,
    mut year_0_hack: bool,
    global_advances: &[Tech],
    ruleset: &RulesetData,
) -> Result<(i32, bool), EffectEvaluationError> {
    let settings = ruleset
        .calendar_settings()
        .unwrap_or_else(|error| panic!("invalid campaign calendar: {error}"));
    assert_eq!(
        settings.fragments_per_year, 0,
        "calendar fragments need persistent fragment-count state"
    );
    let context = CalendarContext {
        year: current_year,
        global_advances,
        ruleset,
    };
    let mut increase = i32::try_from(effect_value(&ruleset.effects, "Turn_Years", &context)?)
        .map_err(|_| EffectEvaluationError::Overflow)?;

    if ruleset
        .victory_enabled(VictoryCondition::SpaceRace)
        .unwrap_or_else(|error| panic!("invalid campaign victories setting: {error}"))
    {
        let slowdown = effect_value(&ruleset.effects, "Slow_Down_Timeline", &context)?;
        if slowdown >= 3 {
            increase = increase.min(1);
        } else if slowdown >= 2 {
            increase = increase.min(2);
        } else if slowdown >= 1 {
            increase = increase.min(5);
        }
    }

    // Freeciv evaluates calendar effects from the displayed year first, then
    // resets a skipped year zero before adding that already-computed step.
    let mut next_year = current_year;
    if year_0_hack {
        next_year = 0;
        year_0_hack = false;
    }
    next_year = next_year
        .checked_add(increase)
        .ok_or(EffectEvaluationError::Overflow)?;
    if next_year == 0 && settings.skip_year_0 {
        next_year = 1;
        year_0_hack = true;
    }
    Ok((next_year, year_0_hack))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ruleset() -> &'static RulesetData {
        RulesetData::campaign_default()
    }

    fn transition(year: i32, global_advances: &[Tech]) -> (i32, bool) {
        next_calendar_state(year, false, global_advances, ruleset()).unwrap()
    }

    #[test]
    fn calendar_uses_civ2civ3_year_threshold_effects() {
        for (year, expected) in [
            (-1001, -951),
            (-1000, -975),
            (-1, 24),
            (0, 20),
            (999, 1019),
            (1000, 1010),
            (1499, 1509),
            (1500, 1505),
            (1749, 1754),
            (1750, 1752),
            (1899, 1901),
            (1900, 1901),
        ] {
            assert_eq!(transition(year, &[]), (expected, false), "year {year}");
        }
        assert_eq!(year_label(-4000), "4000 BCE");
        assert_eq!(year_label(4000), "4000 CE");
    }

    #[test]
    fn timeline_technology_effects_cap_year_increase() {
        assert_eq!(transition(1450, &[]), (1460, false));
        assert_eq!(transition(1450, &[Tech::SPACE_FLIGHT]), (1455, false));
        assert_eq!(
            transition(1450, &[Tech::SPACE_FLIGHT, Tech::SUPERCONDUCTORS]),
            (1452, false)
        );
    }

    #[test]
    fn calendar_skip_year_zero_carries_freeciv_hack_state() {
        let (year, year_0_hack) = next_calendar_state(-25, false, &[], ruleset()).unwrap();
        assert_eq!((year, year_0_hack), (1, true));
        assert_eq!(
            next_calendar_state(year, year_0_hack, &[], ruleset()).unwrap(),
            (20, false)
        );
    }

    #[test]
    fn campaign_victory_flags_match_the_explicit_c2c3_defaults() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.victory_enabled(VictoryCondition::SpaceRace));
        assert!(game.victory_enabled(VictoryCondition::Allied));
        assert!(!game.victory_enabled(VictoryCondition::Culture));
        assert!(!game.victory_enabled(VictoryCondition::WorldPeace));
    }

    #[test]
    fn diplomacy_phase_checks_expiry_before_the_turn_clock_advances() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let starting_turn = game.turn;
        let starting_year = game.year;
        let mut relation = crate::sim::diplomacy::Relation::default();
        relation.pact = Pact::Ceasefire;
        relation.until = starting_turn + 1;
        game.diplomacy.insert((HUMAN, 1), relation);

        game.advance_turn_clock();

        assert_eq!(game.turn, starting_turn + 1);
        assert_eq!(game.year, starting_year + 50);
        assert_eq!(game.diplomacy[&(HUMAN, 1)].pact, Pact::Ceasefire);
    }

    #[test]
    fn barbarians_move_in_the_begin_phase_after_the_turn_clock_advances() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.turn = 1;
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by = [true; PLAYER_SLOTS];
            tile.visible = true;
        }
        let city_position = (10, 20);
        game.create_city(HUMAN, city_position.0, city_position.1);
        let approach = game
            .map_wrap
            .civ3_neighbor(city_position.0, city_position.1, 1)
            .expect("the test city has a northeast neighbor");
        game.spawn_unit(BARBARIANS, UnitKind::ModernArmor, approach.0, approach.1);

        game.advance_turn_clock();
        game.refresh_visibility();
        game.run_barbarian_begin_phase();

        assert_eq!(game.turn, 2);
        assert_eq!(game.cities[0].owner, BARBARIANS);
        assert!(
            game.units
                .iter()
                .any(|unit| { unit.owner == BARBARIANS && (unit.x, unit.y) == city_position })
        );
    }

    #[test]
    fn elimination_waits_until_a_civilization_has_no_units_or_cities() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.units.clear();
        game.spawn_unit(HUMAN, UnitKind::Warrior, 4, 10);
        game.spawn_unit(1, UnitKind::Warrior, 20, 30);

        game.check_game_over();
        assert!(game.game_over().is_none());

        game.units.retain(|unit| unit.owner != 1);
        game.check_game_over();
        assert!(game.game_over().unwrap().contains("world is united"));

        let mut defeated_human = Simulation::new(crate::mapgen::generate(43));
        defeated_human.cities.clear();
        defeated_human.units.clear();
        defeated_human.spawn_unit(1, UnitKind::Warrior, 20, 30);
        defeated_human.check_game_over();
        assert!(
            defeated_human
                .game_over()
                .unwrap()
                .contains("passed into history")
        );
    }
}

impl Simulation {
    pub(in crate::sim) fn check_game_over(&mut self) {
        if self.game_over.is_some() {
            return;
        }
        if !self.player_alive(HUMAN) {
            self.game_over = Some("Your civilization has passed into history.".to_owned());
            return;
        }
        let rival_survives =
            (0..self.civilization_count).any(|owner| owner != HUMAN && self.player_alive(owner));
        if !rival_survives {
            self.game_over = Some("The world is united beneath your banner.".to_owned());
        }
    }
}
