//! Implements nuclear behavior using the selected Freeciv action and effect
//! rules.

use super::*;
use crate::rules::freeciv::{RulesetData, effect_value, requirements_met};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NuclearActionRules {
    min_range: i32,
    max_range: Option<i32>,
    consuming_always: bool,
}

fn required_action_parameter(key: &str) -> &'static crate::rules::freeciv::RuleParameter {
    let mut matching = RulesetData::campaign_default()
        .parameters
        .iter()
        .filter(|parameter| {
            parameter.file.eq_ignore_ascii_case("actions.ruleset")
                && parameter.section.eq_ignore_ascii_case("actions")
                && parameter.key.eq_ignore_ascii_case(key)
        });
    let parameter = matching
        .next()
        .unwrap_or_else(|| panic!("selected C2C3 actions.ruleset is missing {key}"));
    assert!(
        matching.next().is_none(),
        "selected C2C3 actions.ruleset repeats {key}"
    );
    parameter
}

fn nuclear_action_rules(action: &str) -> NuclearActionRules {
    let prefix = match action {
        "Explode Nuclear" => "explode_nuclear",
        "Nuke City" => "nuke_city",
        "Nuke Units" => "nuke_units",
        _ => panic!("unsupported nuclear action {action:?}"),
    };
    let min_range = required_action_parameter(&format!("{prefix}_min_range"))
        .integer_value()
        .expect("selected C2C3 nuclear minimum range is an integer")
        .and_then(|value| i32::try_from(value).ok())
        .expect("selected C2C3 nuclear minimum range fits in i32");
    let max_parameter = required_action_parameter(&format!("{prefix}_max_range"));
    let max_range = match max_parameter.integer_value() {
        Ok(Some(value)) => {
            Some(i32::try_from(value).expect("selected C2C3 nuclear maximum range fits in i32"))
        }
        Ok(None) => panic!("selected C2C3 nuclear maximum range is empty"),
        Err(_) => match max_parameter
            .string_value()
            .expect("selected C2C3 nuclear maximum range is an integer or string")
        {
            Some(value) if value.eq_ignore_ascii_case("unlimited") => None,
            Some(value) => panic!("unsupported selected C2C3 nuclear maximum range {value:?}"),
            None => panic!("selected C2C3 nuclear maximum range is empty"),
        },
    };
    let consuming_always = required_action_parameter(&format!("{prefix}_consuming_always"))
        .boolean_value()
        .expect("selected C2C3 nuclear consuming setting is boolean")
        .expect("selected C2C3 nuclear consuming setting is not empty");
    assert!(min_range >= 0, "nuclear minimum range cannot be negative");
    assert!(
        max_range.is_none_or(|maximum| maximum >= min_range),
        "nuclear maximum range must be at least the minimum"
    );
    NuclearActionRules {
        min_range,
        max_range,
        consuming_always,
    }
}

fn freeciv_map_offsets_within_radius(radius: i32) -> Vec<(i32, i32, i32)> {
    let radius = radius.max(0);
    let mut offsets = (-radius..=radius)
        .flat_map(|dy| (-radius..=radius).map(move |dx| (dx, dy)))
        .filter_map(|(dx, dy)| {
            let distance = crate::world::topology::isohex_vector_distance(dx, dy);
            (distance <= radius).then_some((distance, dx, dy))
        })
        .collect::<Vec<_>>();
    // common/map.c:generate_map_indices and common/city.c:compare_iter_index
    // sort Freeciv circle/square iteration by distance, then dx, then dy.
    offsets.sort_unstable();
    offsets
}

impl Simulation {
    pub(super) fn manhattan_project_completed(&self) -> bool {
        self.completed_wonders
            .contains(&Building::MANHATTAN_PROJECT)
    }

    fn nuclear_action_for_target(&self, unit: &Unit, x: i32, y: i32) -> Option<&'static str> {
        if (x, y) == (unit.x, unit.y) {
            Some("Explode Nuclear")
        } else if self.known_target_tile(unit.owner, x, y).is_some() {
            Some(if self.known_target_is_city(unit.owner, x, y) {
                "Nuke City"
            } else {
                "Nuke Units"
            })
        } else {
            None
        }
    }

    pub(crate) fn can_begin_nuclear_strike(&self, unit: &Unit) -> bool {
        unit.owner < self.players.len() && unit.kind.is_nuclear_weapon()
    }

    pub(super) fn run_ai_nuclear_strike(&mut self, id: u32) {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return;
        };
        if unit.owner == HUMAN || !self.can_begin_nuclear_strike(&unit) {
            return;
        }
        let city_target = self
            .cities
            .iter()
            .filter(|city| city.owner != unit.owner && self.at_war(unit.owner, city.owner))
            .filter(|city| self.ai_air_target_reachable(&unit, city.x, city.y))
            .max_by_key(|city| city.size)
            .map(|city| (city.x, city.y));
        let target = city_target.or_else(|| {
            self.units
                .iter()
                .filter(|other| {
                    other.owner != unit.owner
                        && self.at_war(unit.owner, other.owner)
                        && other.transport.is_none()
                })
                .filter(|other| self.ai_air_target_reachable(&unit, other.x, other.y))
                .max_by_key(|other| other.kind.stats().attack * 3 + other.kind.stats().defense * 2)
                .map(|other| (other.x, other.y))
        });
        if let Some((x, y)) = target {
            self.launch_nuclear_strike(id, x, y);
        }
    }

    fn ai_air_target_reachable(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.can_launch_nuclear_strike_at(unit, x, y) {
            return false;
        }
        let mut paths = super::normal_paths::NormalPaths::enemy_map(self, unit);
        paths
            .movement_cost(self, x, y)
            .is_some_and(|cost| cost < unit.moves_left)
    }

    fn nuclear_action_in_range(&self, unit: &Unit, action: &str, x: i32, y: i32) -> bool {
        let rules = nuclear_action_rules(action);
        let distance = self.map_wrap.civ3_distance(unit.x, unit.y, x, y);
        distance >= rules.min_range && rules.max_range.is_none_or(|maximum| distance <= maximum)
    }

    fn nuclear_action_available(
        &self,
        unit: &Unit,
        action: &str,
        position: (i32, i32),
        target_state: &Tile,
        target_is_city_center: bool,
    ) -> bool {
        match action {
            "Explode Nuclear" | "Nuke City" => self
                .action_available_for_unit_with_known_target_state_fail_closed(
                    unit,
                    action,
                    position,
                    target_state,
                    target_is_city_center,
                ),
            // ACTRES_NUKE_UNITS has an actor/tile enabler; the physical stack
            // hard requirement deliberately accepts an empty tile.
            "Nuke Units" => self.action_available_for_unit_with_known_target_state_fail_closed(
                unit,
                action,
                position,
                target_state,
                target_is_city_center,
            ),
            _ => false,
        }
    }

    pub(crate) fn can_launch_nuclear_strike_at(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.can_begin_nuclear_strike(unit)
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
            || !self.tile(x, y).known_by[unit.owner]
        {
            return false;
        }
        let position = (x, y);
        let target_state = if position == (unit.x, unit.y) {
            // The actor's own tile is known to its owner, even if the test or
            // load path has not refreshed the broader observed-map snapshot.
            self.tile(x, y).clone()
        } else {
            let Some(known) = self.known_target_tile(unit.owner, x, y) else {
                return false;
            };
            known
        };
        let target_is_city_center = self.known_target_is_city(unit.owner, x, y);
        let Some(action) = self.nuclear_action_for_target(unit, x, y) else {
            return false;
        };
        if !self.nuclear_action_in_range(unit, action, x, y)
            || !self.nuclear_action_available(
                unit,
                action,
                position,
                &target_state,
                target_is_city_center,
            )
        {
            return false;
        }
        if action == "Nuke Units" && !self.physical_attack_stack_allowed(unit, x, y) {
            // common/actions.c:ACTRES_NUKE_UNITS uses the ordinary
            // unreachableprotects-aware physical stack gate. An empty stack
            // is valid; a mixed reachable/unreachable stack may not be.
            return false;
        }
        true
    }

    pub(super) fn launch_nuclear_strike(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == id).cloned() else {
            return false;
        };
        if !self.can_launch_nuclear_strike_at(&unit, x, y) {
            return false;
        }
        let Some(action) = self.nuclear_action_for_target(&unit, x, y) else {
            return false;
        };
        self.nuclear_strike(&unit, action, x, y);
        true
    }

    fn nuclear_blast_radius_sq(&self, weapon: &Unit, action: &str, x: i32, y: i32) -> i32 {
        let context =
            super::action_rules::SimulationRequirementContext::for_unit_at(self, weapon, (x, y))
                .with_action(action);
        i32::try_from(
            effect_value(
                &RulesetData::campaign_default().effects,
                "Nuke_Blast_Radius_1_Sq",
                &context,
            )
            .expect("selected C2C3 nuclear blast-radius effects are supported"),
        )
        .expect("selected C2C3 nuclear blast radius fits in i32")
    }

    fn nuclear_blast_tiles(&self, x: i32, y: i32, radius_sq: i32) -> Vec<(i32, i32)> {
        let search_radius = (radius_sq.max(0) as f64).sqrt() as i32;
        let mut seen = HashSet::new();
        freeciv_map_offsets_within_radius(search_radius)
            .into_iter()
            .filter(|(distance, _, _)| distance * distance <= radius_sq)
            .filter_map(|(_, dx, dy)| self.map_wrap.offset(x, y, dx, dy))
            .filter(|position| seen.insert(*position))
            .collect()
    }

    fn nuke_proof_chance(
        &self,
        weapon: &Unit,
        action: &str,
        blast: (i32, i32),
        city: &super::City,
    ) -> i32 {
        let context =
            super::action_rules::SimulationRequirementContext::for_city_at(self, city, Some(blast))
                .with_relation_target(Some(weapon.owner))
                .with_action(action);
        let value = effect_value(
            &RulesetData::campaign_default().effects,
            "Nuke_Proof",
            &context,
        )
        .expect("selected C2C3 Nuke_Proof requirements are supported");
        i32::try_from(value).expect("selected C2C3 Nuke_Proof value fits in i32")
    }

    fn apply_nuclear_city_population_loss(&mut self, city_id: u32, loss_pct: i32) {
        let Some(index) = self.city_index_by_id(city_id) else {
            return;
        };
        let size = self.cities[index].size;
        let loss = ((i64::from(loss_pct) * i64::from(size) + 50) / 100) as i32;
        if loss <= 0 {
            return;
        }
        if loss >= size {
            // cityturn.c:city_reduce_size removes a city when the requested
            // population loss is at least its current size.
            self.remove_city_for_population_loss(city_id);
            return;
        }

        // Match cityturn.c:city_reduce_size: reduce population, cap the
        // granary, remove specialists before workers, update nationalities,
        // then refresh borders and the worker allocation.
        self.cities[index].size -= loss;
        self.cities[index].food = self.cities[index]
            .food
            .min(Self::growth_box(self.cities[index].size));
        self.cities[index].remove_citizen_assignments(loss, self.map_wrap);
        self.remove_citizen_nationalities(index, loss);
        self.rebuild_city_ownership_for_city_event(city_id);
        self.arrange_city_event_workers(city_id);
    }

    fn nuclear_sdi_intercepts(
        &mut self,
        weapon: &Unit,
        action: &str,
        x: i32,
        y: i32,
    ) -> Option<usize> {
        // common/combat.c:sdi_try_defend uses square_iterate with radius 2,
        // which walks in Freeciv distance order (rather than just checking
        // the city on the detonated tile). CivCraft's campaign uses the
        // selected ISO|HEX map distance on its horizontally wrapped grid.
        for (_, dx, dy) in freeciv_map_offsets_within_radius(2) {
            let Some((cx, cy)) = self.map_wrap.offset(x, y, dx, dy) else {
                continue;
            };
            let Some(city_index) = self.city_index_at(cx, cy) else {
                continue;
            };
            let chance = self.nuke_proof_chance(weapon, action, (x, y), &self.cities[city_index]);
            // The source calls fc_rand(100) for every city in the searched
            // area even when Nuke_Proof is zero, and stops at the first hit.
            if self.rand_below(100) < chance {
                return Some(self.cities[city_index].owner);
            }
        }
        None
    }

    fn place_nuclear_fallout(&mut self, owner: usize, x: i32, y: i32) {
        // server/unittools.c:do_nuke_tile gates the extra selection with
        // fc_rand(2). rand_extra_for_tile then chooses among selected C2C3
        // extras native to the tile; a one-option list consumes no RNG.
        if self.rand_below(2) != 1 {
            return;
        }
        let rules = RulesetData::campaign_default();
        let context = super::action_rules::SimulationRequirementContext::for_player_at(
            self,
            owner,
            Some((x, y)),
        );
        let potential = rules
            .rule_records
            .iter()
            .filter(|record| record.namespace.eq_ignore_ascii_case("extra"))
            .filter(|record| {
                record
                    .string_list_field("causes")
                    .expect("selected C2C3 extra causes are valid")
                    .unwrap_or_default()
                    .iter()
                    .any(|cause| cause.eq_ignore_ascii_case("Fallout"))
            })
            .filter(|record| {
                requirements_met(&record.requirements, &context)
                    .expect("selected C2C3 Fallout extra requirements are supported")
            })
            .collect::<Vec<_>>();
        if potential.is_empty() {
            return;
        }
        let selected = self.rand_below(potential.len() as i32) as usize;
        if potential[selected]
            .rule_name
            .eq_ignore_ascii_case("Fallout")
            && !self.tile(x, y).fallout
        {
            self.tile_mut(x, y).fallout = true;
        }
    }

    pub(super) fn nuclear_strike(&mut self, weapon: &Unit, action: &str, x: i32, y: i32) {
        self.publish_visual_event(VisualEvent::Capture {
            unit: weapon.clone(),
        });
        if let Some(defending_city_owner) = self.nuclear_sdi_intercepts(weapon, action, x, y) {
            // server/unithand.c:unit_nuke applies the caught-action
            // consequence against the defending city owner before wiping
            // the intercepted weapon.
            self.action_caught_casus_belli(weapon, action, Some(defending_city_owner), (x, y));
            self.remove_units_with_cargo(vec![weapon.id]);
            self.add_event("SDI Defense intercepted the incoming nuclear attack.");
            return;
        }

        if nuclear_action_rules(action).consuming_always {
            self.remove_units_with_cargo(vec![weapon.id]);
        }

        let radius_sq = self.nuclear_blast_radius_sq(weapon, action, x, y);
        let affected = self.nuclear_blast_tiles(x, y, radius_sq);
        let combat_rules = RulesetData::campaign_default()
            .combat_settings()
            .expect("selected C2C3 nuclear combat settings are valid");

        let mut casualties = 0;
        for &(tx, ty) in &affected {
            let city_tile = self.city_at(tx, ty).is_some();
            let unit_ids = self
                .units
                .iter()
                .filter(|unit| (unit.x, unit.y) == (tx, ty))
                .map(|unit| unit.id)
                .collect::<Vec<_>>();
            for unit_id in unit_ids {
                // A previous carrier loss can unload or rescue this unit.
                // Freeciv iterates the live tile list, so units moved off the
                // blast tile are no longer casualties in this iteration.
                if !self
                    .unit_by_id(unit_id)
                    .is_some_and(|unit| (unit.x, unit.y) == (tx, ty))
                {
                    continue;
                }
                if city_tile
                    && self.rand_below(100) < combat_rules.nuke_defender_survival_chance_pct
                {
                    continue;
                }
                let before = self.units.len();
                self.remove_units_with_cargo(vec![unit_id]);
                let removed = before.saturating_sub(self.units.len()) as i32;
                casualties += removed;
                self.nation_scores[weapon.owner].units_killed += removed;
            }
            if let Some(index) = self.city_index_at(tx, ty) {
                let city_id = self.cities[index].id;
                self.apply_nuclear_city_population_loss(city_id, combat_rules.nuke_pop_loss_pct);
            }
            self.place_nuclear_fallout(weapon.owner, tx, ty);
        }
        self.refresh_visibility();
        // server/unithand.c applies action_consequence_success only after the
        // nuclear explosion has completed. Read the target owner after its
        // city and territory effects, and let the selected C2C3
        // Casus_Belli_Success effect determine whether a provocation is due.
        self.action_success_casus_belli(weapon, action, self.tile(x, y).owner, (x, y));
        self.check_game_over();
        self.add_event(format!(
            "A nuclear weapon detonated, destroying {casualties} units."
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::FreecivRng;

    fn nuclear_game() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.owner = None;
            tile.terrain = Terrain::Grassland;
            tile.fallout = false;
        }
        game
    }

    fn make_nuke(game: &mut Simulation, x: i32, y: i32) -> u32 {
        game.tile_mut(x, y).known_by[HUMAN] = true;
        game.spawn_unit(HUMAN, UnitKind::TacticalNuke, x, y)
    }

    #[test]
    fn tactical_nuke_uses_source_technology_and_movement_rules() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[HUMAN].known.push(Tech::SPACE_FLIGHT);
        assert!(!game.unit_available(HUMAN, UnitKind::TacticalNuke));
        game.completed_wonders.push(Building::MANHATTAN_PROJECT);
        assert!(game.unit_available(HUMAN, UnitKind::TacticalNuke));
        let unit = game.make_unit(0, HUMAN, UnitKind::TacticalNuke, 10, 20);
        assert_eq!(
            game.unit_move_rate(&unit),
            UnitKind::TacticalNuke.stats().moves * MOVE_FRAGMENTS
        );
    }

    #[test]
    fn nuclear_actions_use_selected_c2c3_target_ranges_and_enablers() {
        let mut game = nuclear_game();
        let (x, y) = (10, 20);
        game.sign_pact(HUMAN, 1, Pact::War);
        let nuke = make_nuke(&mut game, x, y);
        let unit = game.unit_by_id(nuke).unwrap().clone();
        assert!(game.can_launch_nuclear_strike_at(&unit, x, y));

        let adjacent_city = game.map_wrap.civ3_offset(x, y, 2, 0).unwrap();
        game.create_city(1, adjacent_city.0, adjacent_city.1);
        game.tile_mut(adjacent_city.0, adjacent_city.1).known_by[HUMAN] = true;
        let city_nuke = game.unit_by_id(nuke).unwrap().clone();
        assert!(game.can_launch_nuclear_strike_at(&city_nuke, adjacent_city.0, adjacent_city.1));

        let empty_units_tile = game.map_wrap.civ3_offset(x, y, 0, 2).unwrap();
        game.tile_mut(empty_units_tile.0, empty_units_tile.1).owner = Some(1);
        game.tile_mut(empty_units_tile.0, empty_units_tile.1)
            .known_by[HUMAN] = true;
        let empty_stack_nuke = game.unit_by_id(nuke).unwrap().clone();
        assert!(
            game.can_launch_nuclear_strike_at(
                &empty_stack_nuke,
                empty_units_tile.0,
                empty_units_tile.1
            ),
            "ACTRES_NUKE_UNITS accepts an empty physical stack when its tile enabler passes"
        );

        let adjacent_units = game.map_wrap.civ3_offset(x, y, -2, 0).unwrap();
        game.tile_mut(adjacent_units.0, adjacent_units.1).owner = Some(1);
        game.tile_mut(adjacent_units.0, adjacent_units.1).known_by[HUMAN] = true;
        game.spawn_unit(1, UnitKind::Warrior, adjacent_units.0, adjacent_units.1);
        let units_nuke = game.unit_by_id(nuke).unwrap().clone();
        assert!(game.can_launch_nuclear_strike_at(&units_nuke, adjacent_units.0, adjacent_units.1));

        let mixed_stack = game.map_wrap.offset(x, y, 2, -2).unwrap();
        game.tile_mut(mixed_stack.0, mixed_stack.1).owner = Some(1);
        game.tile_mut(mixed_stack.0, mixed_stack.1).known_by[HUMAN] = true;
        game.spawn_unit(1, UnitKind::Warrior, mixed_stack.0, mixed_stack.1);
        game.spawn_unit(1, UnitKind::Fighter, mixed_stack.0, mixed_stack.1);
        let mixed_stack_nuke = game.unit_by_id(nuke).unwrap().clone();
        assert!(
            !game.can_launch_nuclear_strike_at(&mixed_stack_nuke, mixed_stack.0, mixed_stack.1),
            "unreachable aircraft protects the reachable ground unit under unreachableprotects"
        );

        let distant_city = game
            .map_wrap
            .civ3_offset(adjacent_city.0, adjacent_city.1, 2, 0)
            .unwrap();
        game.create_city(1, distant_city.0, distant_city.1);
        game.tile_mut(distant_city.0, distant_city.1).known_by[HUMAN] = true;
        let distant_nuke = game.unit_by_id(nuke).unwrap().clone();
        assert!(!game.can_launch_nuclear_strike_at(&distant_nuke, distant_city.0, distant_city.1));
    }

    #[test]
    fn nuclear_action_selection_uses_remembered_city_state() {
        let mut game = nuclear_game();
        game.observed_map = Default::default();
        let (x, y) = (10, 20);
        game.sign_pact(HUMAN, 1, Pact::War);
        let nuke = make_nuke(&mut game, x, y);
        let unit = game.unit_by_id(nuke).unwrap().clone();
        let target = game.map_wrap.civ3_offset(x, y, 2, 0).unwrap();

        // The remembered tile belongs to the enemy but contains no known city.
        // A city is added at the same position after the observation, hidden
        // from the actor. The live site must not change the selected action.
        let target_index = game.map_wrap.index(target.0, target.1);
        let tile = game.tile_mut(target.0, target.1);
        tile.owner = Some(1);
        tile.known_by[HUMAN] = true;
        tile.seen = true;
        tile.visible = false;
        let mut remembered = vec![vec![false; game.tiles.len()]; game.players.len()];
        remembered[HUMAN][target_index] = true;
        game.remember_render_map(&remembered);

        game.cities.push(City::new(
            10_000,
            1,
            target.0,
            target.1,
            "Hidden City".to_owned(),
            game.turn,
        ));
        game.tile_mut(target.0, target.1).visible = false;
        assert!(!game.known_target_is_city(HUMAN, target.0, target.1));
        assert_eq!(
            game.nuclear_action_for_target(&unit, target.0, target.1),
            Some("Nuke Units")
        );
        assert!(game.can_launch_nuclear_strike_at(&unit, target.0, target.1));
    }

    #[test]
    fn caught_action_effect_records_ruleset_casus_belli() {
        let mut game = nuclear_game();
        let (x, y) = (10, 20);
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.players[1].government = Government::Democracy;
        let nuke = make_nuke(&mut game, x, y);
        let unit = game.unit_by_id(nuke).unwrap().clone();
        assert!(game.senate_blocks_cancellation(1, HUMAN));

        // C2C3 assigns a caught casus belli to Steal Tech while at peace.
        // This exercises the same selected ruleset consequence used by SDI
        // interception, even though C2C3 defines no caught effect for nukes.
        game.action_caught_casus_belli(&unit, "Steal Tech", Some(1), (x, y));

        assert!(
            !game.senate_blocks_cancellation(1, HUMAN),
            "the selected caught effect records a current casus belli"
        );
    }

    #[test]
    fn c2c3_nuclear_effect_radius_maps_squared_radius_to_hex_neighbors() {
        let mut game = nuclear_game();
        let (x, y) = (10, 20);
        let nuke = make_nuke(&mut game, x, y);
        let unit = game.unit_by_id(nuke).unwrap().clone();
        let radius = game.nuclear_blast_radius_sq(&unit, "Explode Nuclear", x, y);
        assert_eq!(radius, 2);
        let affected = game.nuclear_blast_tiles(x, y, radius);
        assert_eq!(affected.len(), 7);
        assert!(affected.contains(&(x, y)));
        assert!(
            affected
                .iter()
                .all(|&(tx, ty)| game.map_wrap.civ3_distance(x, y, tx, ty) <= 1)
        );
    }

    #[test]
    fn c2c3_nuke_survival_population_loss_and_rng_order_match_source() {
        let mut game = nuclear_game();
        let (x, y) = (10, 20);
        game.create_city(HUMAN, x, y);
        let city_index = game.city_index_at(x, y).unwrap();
        game.cities[city_index].size = 10;
        game.cities[city_index].food = 100;
        let nearby = game.map_wrap.offset(x, y, 1, 0).unwrap();
        game.tile_mut(nearby.0, nearby.1).owner = Some(HUMAN);
        let no_pollution = game.map_wrap.offset(x, y, -1, 0).unwrap();
        game.tile_mut(no_pollution.0, no_pollution.1).terrain = Terrain::Ocean;
        let nuke = make_nuke(&mut game, x, y);
        let city_defender = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        let exposed = game.spawn_unit(HUMAN, UnitKind::Warrior, nearby.0, nearby.1);
        let carrier = game.spawn_unit(HUMAN, UnitKind::Carrier, x, y);
        let passenger = game.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
        game.units
            .iter_mut()
            .find(|unit| unit.id == passenger)
            .unwrap()
            .transport = Some(carrier);
        let preexisting_fallout = game.map_wrap.offset(x, y, 0, 1).unwrap();
        game.tile_mut(preexisting_fallout.0, preexisting_fallout.1)
            .fallout = true;
        game.native_random = Some(FreecivRng::new(87));
        let mut expected_random = game.native_random.clone().unwrap();
        // sdi_try_defend rolls once for the city even with no Nuke_Proof;
        // do_nuke_tile rolls survival for each unit inside the city,
        // including a passenger after its carrier has been destroyed;
        // each tile rolls the Fallout gate. Eligible non-ocean tiles each
        // have one extra, so fc_rand(1) consumes no additional state.
        expected_random.draw(100);
        expected_random.draw(100);
        expected_random.draw(100);
        expected_random.draw(100);
        let blast_tiles = game.nuclear_blast_tiles(x, y, 2);
        let fallout_rolls = blast_tiles
            .iter()
            .map(|_| expected_random.draw(2) == 1)
            .collect::<Vec<_>>();

        assert!(game.apply(Command::NuclearStrike(nuke, x, y)));
        assert_eq!(game.cities[city_index].size, 5);
        assert!(game.unit_by_id(city_defender).is_none());
        assert!(game.unit_by_id(carrier).is_none());
        assert!(game.unit_by_id(passenger).is_none());
        assert!(game.unit_by_id(exposed).is_none());
        assert_eq!(
            game.cities[city_index].food,
            Simulation::growth_box(5),
            "cityturn.c caps food stock to the reduced granary"
        );
        for ((tx, ty), fallout_roll) in blast_tiles.into_iter().zip(fallout_rolls) {
            assert_eq!(
                game.tile(tx, ty).fallout,
                (tx, ty) == preexisting_fallout || ((tx, ty) != no_pollution && fallout_roll)
            );
        }
        assert_eq!(game.native_random, Some(expected_random));
    }

    #[test]
    fn nuclear_population_loss_removes_a_city_when_loss_reaches_its_size() {
        let mut game = nuclear_game();
        game.create_city(HUMAN, 10, 20);
        let city_id = game.cities[0].id;
        game.cities[0].size = 2;
        game.players[HUMAN].gold = 0;

        game.apply_nuclear_city_population_loss(city_id, 100);

        assert!(game.city_index_by_id(city_id).is_none());
        assert!(
            game.units
                .iter()
                .all(|unit| unit.home_city != Some(city_id))
        );
    }

    #[test]
    fn sdi_uses_nuke_proof_effects_across_freeciv_square_radius_and_stops_rng() {
        let mut game = nuclear_game();
        let (x, y) = (10, 20);
        game.sign_pact(HUMAN, 1, Pact::War);
        let nuke = make_nuke(&mut game, x, y);
        let no_sdi = game.map_wrap.offset(x, y, 1, 0).unwrap();
        game.create_city(1, no_sdi.0, no_sdi.1);
        let sdi = game.map_wrap.offset(x, y, 2, 0).unwrap();
        game.create_city(1, sdi.0, sdi.1);
        let city_index = game.city_index_at(sdi.0, sdi.1).unwrap();
        game.cities[city_index]
            .buildings
            .push(Building::SDI_DEFENSE);
        game.native_random = Some(FreecivRng::new(91));
        let mut expected_random = game.native_random.clone().unwrap();
        expected_random.draw(100); // Nearby city with zero Nuke_Proof.
        expected_random.draw(100); // SDI city, guaranteed Nuke_Proof.

        let intercepting_city = game.unit_by_id(nuke).unwrap().clone();
        assert_eq!(
            game.nuclear_sdi_intercepts(&intercepting_city, "Explode Nuclear", x, y),
            Some(1),
            "the first successful SDI city supplies the caught-action victim"
        );
        game.native_random = Some(FreecivRng::new(91));

        assert!(game.apply(Command::NuclearStrike(nuke, x, y)));
        assert!(game.unit_by_id(nuke).is_none());
        assert!(
            game.events
                .iter()
                .any(|event| event.contains("SDI Defense"))
        );
        assert!(!game.events.iter().any(|event| event.contains("detonated")));
        assert_eq!(game.native_random, Some(expected_random));
    }

    #[test]
    fn sdi_outside_freeciv_radius_and_nonforeign_sdi_do_not_intercept() {
        for (owner, distance) in [(HUMAN, 1), (1, 3)] {
            let mut game = nuclear_game();
            let (x, y) = (10, 20);
            game.sign_pact(HUMAN, 1, Pact::War);
            let nuke = make_nuke(&mut game, x, y);
            let position = game.map_wrap.offset(x, y, distance, 0).unwrap();
            game.create_city(owner, position.0, position.1);
            let city_index = game.city_index_at(position.0, position.1).unwrap();
            game.cities[city_index]
                .buildings
                .push(Building::SDI_DEFENSE);
            game.native_random = Some(FreecivRng::new(32));
            assert!(game.apply(Command::NuclearStrike(nuke, x, y)));
            assert!(game.events.iter().any(|event| event.contains("detonated")));
        }
    }
}
