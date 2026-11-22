//! Implements Civilization III: Conquests nuclear weapon mechanics (3x3 blast
//! and fallout, city population halving, 50% city improvement destruction, 50%
//! unit kill with survivor HP halving, and empire-wide SDI Defense).

use super::*;
/// Every nuclear action consumes the weapon: `explode_nuclear`, `nuke_city`
/// and `nuke_units` are all "consuming always" in the campaign ruleset.
fn nuclear_action_consumes_weapon(action: &str) -> bool {
    match action {
        "Explode Nuclear" | "Nuke City" | "Nuke Units" => true,
        _ => panic!("unsupported nuclear action {action:?}"),
    }
}

impl Simulation {
    pub(super) fn manhattan_project_completed(&self) -> bool {
        self.completed_wonders
            .contains(&Building::MANHATTAN_PROJECT)
    }

    fn nuclear_action_for_target(&self, unit: &Unit, x: i32, y: i32) -> Option<&'static str> {
        if (x, y) == (unit.x, unit.y) {
            Some("Explode Nuclear")
        } else if self.known_tile_for(unit.owner, x, y).is_some() {
            Some(if self.observed_city_owner(unit.owner, x, y).is_some() {
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
        if !self.can_begin_nuclear_strike(&unit) || !self.players[unit.owner].ai {
            return;
        }
        let city_target = self
            .cities
            .iter()
            .filter(|city| city.owner != unit.owner && self.at_war(unit.owner, city.owner))
            .filter(|city| self.can_launch_nuclear_strike_at(&unit, city.x, city.y))
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
                .filter(|other| self.can_launch_nuclear_strike_at(&unit, other.x, other.y))
                .max_by_key(|other| other.kind.stats().attack * 3 + other.kind.stats().defense * 2)
                .map(|other| (other.x, other.y))
        });
        if let Some((x, y)) = target {
            self.launch_nuclear_strike(id, x, y);
        }
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
        // Nuclear launch distance comes from the campaign unit. The ICBM's
        // zero range denotes a map-wide launch; other zero-range units can
        // only detonate in place.
        let range = unit.kind.bombard_range();
        if !(unit.kind == UnitKind::ICBM && range == 0)
            && self.map_wrap.step_distance(unit.x, unit.y, x, y) > range
        {
            return false;
        }
        let position = (x, y);
        let target_state = if position == (unit.x, unit.y) {
            // The actor's own tile is known to its owner, even if the test or
            // load path has not refreshed the broader observed-map snapshot.
            self.tile(x, y)
        } else {
            let Some(known) = self.known_tile_for(unit.owner, x, y) else {
                return false;
            };
            known
        };
        let target_is_city_center = self.observed_city_owner(unit.owner, x, y).is_some();
        let Some(action) = self.nuclear_action_for_target(unit, x, y) else {
            return false;
        };
        if !self.nuclear_action_available(
            unit,
            action,
            position,
            target_state,
            target_is_city_center,
        ) {
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

    fn nuclear_blast_tiles(&self, x: i32, y: i32) -> Vec<(i32, i32)> {
        // Recorded owner exception (Civ III): nuclear detonations affect the
        // full 3x3 square (ground zero plus all 8 Civ III compass neighbors).
        self.map_wrap
            .tiles_within_steps(x, y, 1)
            .into_iter()
            .map(|(tx, ty, _)| (tx, ty))
            .collect()
    }

    /// Civ III nuclear city strike: halves population (never razing a size-1
    /// city) and destroys each non-wonder, non-Palace improvement with a 50%
    /// chance. Returns the number of destroyed improvements.
    fn apply_nuclear_city_damage(&mut self, city_id: u32) -> i32 {
        let Some(index) = self.city_index_by_id(city_id) else {
            return 0;
        };
        let size = self.cities[index].size;
        let loss = (size / 2).min((size - 1).max(0));
        if loss > 0 {
            self.cities[index].size -= loss;
            self.cities[index].food = self.cities[index]
                .food
                .min(Self::growth_box(self.cities[index].size));
            self.cities[index].remove_citizen_assignments(loss, self.map_wrap);
            self.remove_citizen_nationalities(index, loss);
            self.rebuild_city_ownership_for_city_event(city_id);
            self.arrange_city_event_workers(city_id);
        }

        let Some(index) = self.city_index_by_id(city_id) else {
            return 0;
        };
        let candidates: Vec<Building> = self.cities[index]
            .buildings
            .iter()
            .copied()
            .filter(|&building| {
                building != Building::PALACE
                    && building.data().genus == crate::rules::Genus::Improvement
            })
            .collect();
        let mut destroyed = Vec::new();
        for building in candidates {
            if self.rand_below(2) == 0 {
                destroyed.push(building);
            }
        }
        if !destroyed.is_empty()
            && let Some(city) = self.cities.iter_mut().find(|city| city.id == city_id)
        {
            for &building in &destroyed {
                city.buildings.retain(|&current| current != building);
                city.forget_culture_building(building);
            }
        }
        destroyed.len() as i32
    }

    /// Civ III Small Wonder SDI Defense: protects the entire civilization
    /// against incoming nuclear weapons targeting any of its cities, territory,
    /// or units within the 3x3 blast zone. Interception rate is 75% against
    /// ICBMs and 37.5% (75% detection with 50% tactical evasion) against
    /// Tactical Nukes.
    fn nuclear_sdi_intercepts(&mut self, weapon: &Unit, x: i32, y: i32) -> Option<usize> {
        let affected = self.nuclear_blast_tiles(x, y);
        let mut defenders = Vec::new();
        for &(tx, ty) in &affected {
            if let Some(city) = self.city_at(tx, ty)
                && city.owner != weapon.owner
                && !defenders.contains(&city.owner)
            {
                defenders.push(city.owner);
            }
            if let Some(owner) = self.tile(tx, ty).owner
                && owner != weapon.owner
                && !defenders.contains(&owner)
            {
                defenders.push(owner);
            }
            for unit in self.tile_units_ordered(tx, ty) {
                if unit.owner != weapon.owner && !defenders.contains(&unit.owner) {
                    defenders.push(unit.owner);
                }
            }
        }
        defenders.sort_unstable();
        for defender in defenders {
            let has_sdi = self
                .cities
                .iter()
                .any(|city| city.owner == defender && city.has_building(Building::SDI_DEFENSE));
            if !has_sdi {
                continue;
            }
            if self.rand_below(100) < 75 {
                let evaded = weapon.kind == UnitKind::TacticalNuke && self.rand_below(2) == 0;
                if !evaded {
                    return Some(defender);
                }
            }
        }
        None
    }

    fn place_nuclear_fallout(&mut self, x: i32, y: i32) {
        // Recorded owner exception (Civ III): every non-water, non-city land
        // tile inside the 3x3 blast radius receives nuclear fallout and loses
        // its terrain improvements. Water tiles and surviving city centers do
        // not receive tile fallout.
        if self.tile(x, y).terrain.is_water() || self.city_at(x, y).is_some() {
            return;
        }
        let tile = self.tile_mut(x, y);
        tile.fallout = true;
        tile.road = false;
        tile.railroad = false;
        tile.mine = false;
        tile.irrigation = false;
        tile.farmland = false;
        tile.oil_well = false;
        tile.base = None;
    }

    pub(super) fn nuclear_strike(&mut self, weapon: &Unit, action: &str, x: i32, y: i32) {
        let target_city = self.city_at(x, y).map(|city| (city.id, city.owner));
        let initial_target_owner = target_city
            .map(|(_, owner)| owner)
            .or_else(|| self.tile(x, y).owner)
            .or_else(|| {
                self.tile_units_ordered(x, y)
                    .find(|unit| unit.owner != weapon.owner)
                    .map(|unit| unit.owner)
            });
        if let Some(defending_city_owner) = self.nuclear_sdi_intercepts(weapon, x, y) {
            self.publish_visual_event(VisualEvent::AirMission(Box::new(
                presentation::AirMissionVisual {
                    aircraft: weapon.clone(),
                    from: (weapon.x, weapon.y),
                    to: (x, y),
                    kind: presentation::AirMissionVisualKind::NuclearStrike {
                        intercepted_by_sdi: true,
                    },
                },
            )));
            // server/unithand.c:unit_nuke applies the caught-action
            // consequence against the defending city owner before wiping
            // the intercepted weapon.
            self.action_caught_casus_belli(weapon, action, Some(defending_city_owner), (x, y));
            self.remove_units_with_cargo_because(vec![weapon.id], lifecycle::UnitRemoval::Combat);
            self.add_event("SDI Defense intercepted the incoming nuclear attack.");
            if self
                .trace
                .wants::<military_observation::NuclearStrikeEvent>()
            {
                self.trace_nuclear_strike(military_observation::NuclearStrikeEvent {
                    owner: weapon.owner,
                    weapon_id: weapon.id,
                    kind: weapon.kind.name(),
                    action: match action {
                        "Explode Nuclear" => "explode_nuclear",
                        "Nuke City" => "nuke_city",
                        "Nuke Units" => "nuke_units",
                        _ => "other",
                    },
                    tile: (x, y),
                    city_id: target_city.map(|(id, _)| id),
                    target_owner: initial_target_owner.or(Some(defending_city_owner)),
                    intercepted_by_sdi: Some(defending_city_owner),
                    units_killed: 0,
                    cities_hit: 0,
                    fallout_tiles: 0,
                });
            }
            return;
        }

        self.publish_visual_event(VisualEvent::AirMission(Box::new(
            presentation::AirMissionVisual {
                aircraft: weapon.clone(),
                from: (weapon.x, weapon.y),
                to: (x, y),
                kind: presentation::AirMissionVisualKind::NuclearStrike {
                    intercepted_by_sdi: false,
                },
            },
        )));

        if nuclear_action_consumes_weapon(action) {
            self.remove_units_with_cargo_because(vec![weapon.id], lifecycle::UnitRemoval::Expended);
        }

        let affected = self.nuclear_blast_tiles(x, y);

        let mut casualties = 0;
        let mut units_damaged = 0;
        let mut cities_hit = 0;
        let mut buildings_destroyed = 0;
        let mut fallout_tiles = 0;
        for &(tx, ty) in &affected {
            let unit_ids = self
                .units
                .iter()
                .filter(|unit| (unit.x, unit.y) == (tx, ty))
                .map(|unit| unit.id)
                .collect::<Vec<_>>();
            for unit_id in unit_ids {
                if self
                    .unit_by_id(unit_id)
                    .is_none_or(|unit| (unit.x, unit.y) != (tx, ty))
                {
                    continue;
                }
                // Civ III nuclear blast: each unit in the 3x3 rolls 50% chance
                // of destruction; survivors lose half their remaining HP
                // (rounded up, floored at 1 HP).
                if self.rand_below(2) == 0 {
                    let before = self.units.len();
                    self.remove_units_with_cargo_because(
                        vec![unit_id],
                        lifecycle::UnitRemoval::Nuclear,
                    );
                    let removed = before.saturating_sub(self.units.len()) as i32;
                    casualties += removed;
                    self.nation_scores[weapon.owner].units_killed += removed;
                } else if let Some(unit) = self.units.iter_mut().find(|u| u.id == unit_id) {
                    let damage = (unit.hp + 1) / 2;
                    unit.hp = (unit.hp - damage).max(1);
                    units_damaged += 1;
                    self.sync_army_damage(unit_id);
                }
            }
            if let Some(index) = self.city_index_at(tx, ty) {
                cities_hit += 1;
                let city_id = self.cities[index].id;
                buildings_destroyed += self.apply_nuclear_city_damage(city_id);
            }
            let had_fallout = self.tile(tx, ty).fallout;
            self.place_nuclear_fallout(tx, ty);
            if !had_fallout && self.tile(tx, ty).fallout {
                fallout_tiles += 1;
            }
        }
        self.refresh_visibility();
        // server/unithand.c applies action_consequence_success only after the
        // nuclear explosion has completed. Read the target owner after its
        // city and territory effects, and let the selected C2C3
        // Casus_Belli_Success effect determine whether a provocation is due.
        self.action_success_casus_belli(weapon, action, self.tile(x, y).owner, (x, y));
        self.check_game_over();
        let detail = if buildings_destroyed > 0 {
            format!(
                "A nuclear weapon detonated: {casualties} units destroyed, {units_damaged} damaged, {buildings_destroyed} improvements destroyed."
            )
        } else {
            format!(
                "A nuclear weapon detonated: {casualties} units destroyed, {units_damaged} damaged."
            )
        };
        self.add_event(detail);
        if self
            .trace
            .wants::<military_observation::NuclearStrikeEvent>()
        {
            self.trace_nuclear_strike(military_observation::NuclearStrikeEvent {
                owner: weapon.owner,
                weapon_id: weapon.id,
                kind: weapon.kind.name(),
                action: match action {
                    "Explode Nuclear" => "explode_nuclear",
                    "Nuke City" => "nuke_city",
                    "Nuke Units" => "nuke_units",
                    _ => "other",
                },
                tile: (x, y),
                city_id: target_city.map(|(id, _)| id),
                target_owner: initial_target_owner,
                intercepted_by_sdi: None,
                units_killed: casualties,
                cities_hit,
                fallout_tiles,
            });
        }
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
        game.tile_mut(x, y).known_by.set(HUMAN, true);
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

    /// Mirrors the civ2civ3 ruleset behavior.
    #[test]
    fn nuclear_actions_use_unit_ranges_and_selected_ruleset_enablers() {
        let mut game = nuclear_game();
        let (x, y) = (10, 20);
        game.sign_pact(HUMAN, 1, Pact::War);
        let nuke = make_nuke(&mut game, x, y);
        let unit = game.unit_by_id(nuke).unwrap().clone();
        assert!(game.can_launch_nuclear_strike_at(&unit, x, y));

        let adjacent_city = game.map_wrap.eight_way_offset(x, y, 2, 0).unwrap();
        game.create_city(1, adjacent_city.0, adjacent_city.1);
        game.tile_mut(adjacent_city.0, adjacent_city.1)
            .known_by
            .set(HUMAN, true);
        let city_nuke = game.unit_by_id(nuke).unwrap().clone();
        assert!(game.can_launch_nuclear_strike_at(&city_nuke, adjacent_city.0, adjacent_city.1));

        let empty_units_tile = game.map_wrap.eight_way_offset(x, y, 0, 2).unwrap();
        game.tile_mut(empty_units_tile.0, empty_units_tile.1).owner = Some(1);
        game.tile_mut(empty_units_tile.0, empty_units_tile.1)
            .known_by
            .set(HUMAN, true);
        let empty_stack_nuke = game.unit_by_id(nuke).unwrap().clone();
        assert!(
            game.can_launch_nuclear_strike_at(
                &empty_stack_nuke,
                empty_units_tile.0,
                empty_units_tile.1
            ),
            "ACTRES_NUKE_UNITS accepts an empty physical stack when its tile enabler passes"
        );

        let adjacent_units = game.map_wrap.eight_way_offset(x, y, -2, 0).unwrap();
        game.tile_mut(adjacent_units.0, adjacent_units.1).owner = Some(1);
        game.tile_mut(adjacent_units.0, adjacent_units.1)
            .known_by
            .set(HUMAN, true);
        game.spawn_unit(1, UnitKind::Warrior, adjacent_units.0, adjacent_units.1);
        let units_nuke = game.unit_by_id(nuke).unwrap().clone();
        assert!(game.can_launch_nuclear_strike_at(&units_nuke, adjacent_units.0, adjacent_units.1));

        let mixed_stack = game.map_wrap.offset(x, y, 2, -2).unwrap();
        game.tile_mut(mixed_stack.0, mixed_stack.1).owner = Some(1);
        game.tile_mut(mixed_stack.0, mixed_stack.1)
            .known_by
            .set(HUMAN, true);
        game.spawn_unit(1, UnitKind::Warrior, mixed_stack.0, mixed_stack.1);
        game.spawn_unit(1, UnitKind::Fighter, mixed_stack.0, mixed_stack.1);
        let mixed_stack_nuke = game.unit_by_id(nuke).unwrap().clone();
        assert!(
            !game.can_launch_nuclear_strike_at(&mixed_stack_nuke, mixed_stack.0, mixed_stack.1),
            "unreachable aircraft protects the reachable ground unit under unreachableprotects"
        );

        let width = game.map_wrap.width();
        let edge = ((x - 6).rem_euclid(width), y);
        let beyond = ((x - 7).rem_euclid(width), y);
        for (tx, ty) in [edge, beyond] {
            game.create_city(1, tx, ty);
        }
        game.spawn_unit(HUMAN, UnitKind::Warrior, beyond.0, y + 1);
        game.refresh_visibility();
        assert_eq!(game.observed_city_owner(HUMAN, beyond.0, beyond.1), Some(1));
        assert!(!game.apply(Command::NuclearStrike(nuke, beyond.0, beyond.1)));
        assert!(game.apply(Command::NuclearStrike(nuke, edge.0, edge.1)));
        assert!(game.unit_by_id(nuke).is_none());
    }

    #[test]
    fn icbm_can_launch_across_the_map_for_player_and_ai() {
        for ai in [false, true] {
            let mut game = nuclear_game();
            let owner = if ai { 1 } else { HUMAN };
            let enemy = if ai { HUMAN } else { 1 };
            let target = (game.map_wrap.width() / 2, game.map_wrap.height() - 2);
            game.create_city(enemy, target.0, target.1);
            game.sign_pact(owner, enemy, Pact::War);
            let id = game.spawn_unit(owner, UnitKind::ICBM, 0, 2);
            game.spawn_unit(owner, UnitKind::Warrior, target.0, target.1 - 1);
            game.refresh_visibility();
            assert_eq!(
                game.observed_city_owner(owner, target.0, target.1),
                Some(enemy)
            );
            if ai {
                game.run_ai_nuclear_strike(id);
            } else {
                assert!(game.apply(Command::NuclearStrike(id, target.0, target.1)));
            }
            assert!(game.unit_by_id(id).is_none());
        }
    }

    #[test]
    fn nuclear_action_selection_uses_remembered_city_state() {
        let mut game = nuclear_game();
        game.observed_map = Default::default();
        let (x, y) = (10, 20);
        game.sign_pact(HUMAN, 1, Pact::War);
        let nuke = make_nuke(&mut game, x, y);
        let unit = game.unit_by_id(nuke).unwrap().clone();
        let target = game.map_wrap.eight_way_offset(x, y, 2, 0).unwrap();

        // The remembered tile belongs to the enemy but contains no known city.
        // A city is added at the same position after the observation, hidden
        // from the actor. The live site must not change the selected action.
        let target_index = game.map_wrap.index(target.0, target.1);
        let tile = game.tile_mut(target.0, target.1);
        tile.owner = Some(1);
        tile.known_by.set(HUMAN, true);
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
        assert!(
            game.observed_city_owner(HUMAN, target.0, target.1)
                .is_none()
        );
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
        game.players[1].government = Government::DEMOCRACY;
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

    /// Mirrors Civ III behavior.
    #[test]
    fn native_nuclear_blast_covers_full_three_by_three_square() {
        let game = nuclear_game();
        let (x, y) = (10, 20);
        let affected = game.nuclear_blast_tiles(x, y);
        assert_eq!(affected.len(), 9);
        assert!(affected.contains(&(x, y)));
        assert!(
            affected
                .iter()
                .all(|&(tx, ty)| game.map_wrap.step_distance(x, y, tx, ty) <= 1)
        );
    }

    /// Mirrors Civ III behavior.
    #[test]
    fn native_nuke_halves_population_kills_or_redlines_units_destroys_half_improvements_and_pollutes_3x3()
     {
        let mut game = nuclear_game();
        let (x, y) = (10, 20);
        game.create_city(HUMAN, x, y);
        let city_index = game.city_index_at(x, y).unwrap();
        game.cities[city_index].size = 10;
        game.cities[city_index].food = 100;
        // Palace, Great Wonder, Small Wonder must always survive; regular
        // improvements each roll 50% destruction.
        game.cities[city_index].buildings.extend([
            Building::PALACE,
            Building::GREAT_LIBRARY,
            Building::PENTAGON,
            Building::BARRACKS_III,
            Building::TEMPLE,
            Building::MARKETPLACE,
            Building::LIBRARY,
            Building::UNIVERSITY,
            Building::BANK,
            Building::FACTORY,
            Building::CATHEDRAL,
        ]);
        let nearby = game.map_wrap.eight_way_neighbor(x, y, 2).unwrap();
        game.tile_mut(nearby.0, nearby.1).owner = Some(HUMAN);
        game.tile_mut(nearby.0, nearby.1).road = true;
        game.tile_mut(nearby.0, nearby.1).mine = true;
        let ocean = game.map_wrap.eight_way_neighbor(x, y, 6).unwrap();
        game.tile_mut(ocean.0, ocean.1).terrain = Terrain::Ocean;
        let coast = game.map_wrap.eight_way_neighbor(x, y, 0).unwrap();
        game.tile_mut(coast.0, coast.1).terrain = Terrain::Coast;
        let nuke = make_nuke(&mut game, x, y);
        let mut tracked_units = Vec::new();
        for _ in 0..10 {
            let id = game.spawn_unit(HUMAN, UnitKind::Infantry, x, y);
            game.units.iter_mut().find(|u| u.id == id).unwrap().hp = 4;
            tracked_units.push(id);
        }
        let exposed = game.spawn_unit(HUMAN, UnitKind::Infantry, nearby.0, nearby.1);
        game.units.iter_mut().find(|u| u.id == exposed).unwrap().hp = 4;
        tracked_units.push(exposed);
        game.native_random = Some(FreecivRng::new(87));

        let blast_tiles = game.nuclear_blast_tiles(x, y);
        assert_eq!(blast_tiles.len(), 9);
        assert!(game.apply(Command::NuclearStrike(nuke, x, y)));
        assert_eq!(game.cities[city_index].size, 5);
        assert_eq!(
            game.cities[city_index].food,
            Simulation::growth_box(5),
            "granary food stock is capped to the reduced city size"
        );
        // Palace, Great Wonder, and Small Wonder always survive.
        let city = &game.cities[city_index];
        assert!(city.has_building(Building::PALACE));
        assert!(city.has_building(Building::GREAT_LIBRARY));
        assert!(city.has_building(Building::PENTAGON));
        let remaining_regular = city
            .buildings
            .iter()
            .filter(|b| {
                **b != Building::PALACE && b.data().genus == crate::rules::Genus::Improvement
            })
            .count();
        assert!(
            remaining_regular < 8,
            "expected some regular improvements destroyed by 50% roll; remaining={remaining_regular}"
        );
        // Units roll 50% destruction; any survivor is halved from 4 HP -> 2 HP.
        let mut killed = 0;
        let mut survived = 0;
        for id in tracked_units {
            if let Some(u) = game.unit_by_id(id) {
                survived += 1;
                assert_eq!(u.hp, 2, "surviving 4-HP unit should lose half its HP");
            } else {
                killed += 1;
            }
        }
        assert!(
            killed > 0 && survived > 0,
            "expected mixed unit kills ({killed}) and survivors ({survived})"
        );
        // Every non-water, non-city tile in the 3x3 gets fallout; water tiles
        // (both Coast and Ocean) and the surviving city center stay clear.
        for (tx, ty) in blast_tiles {
            let expected_fallout = (tx, ty) != (x, y) && (tx, ty) != ocean && (tx, ty) != coast;
            assert_eq!(
                game.tile(tx, ty).fallout,
                expected_fallout,
                "tile ({tx}, {ty}) fallout mismatch"
            );
        }
        assert!(!game.tile(nearby.0, nearby.1).road);
        assert!(!game.tile(nearby.0, nearby.1).mine);
    }

    /// Mirrors Civ III behavior.
    #[test]
    fn native_nuclear_strike_never_razes_a_size_one_city() {
        let mut game = nuclear_game();
        game.create_city(HUMAN, 10, 20);
        let city_id = game.cities[0].id;
        game.cities[0].size = 1;
        game.players[HUMAN].gold = 0;

        game.apply_nuclear_city_damage(city_id);

        let city = game
            .city_by_id(city_id)
            .expect("Civ III nukes never raze cities");
        assert_eq!(city.size, 1);
    }

    /// Mirrors Civ III behavior.
    #[test]
    fn native_sdi_small_wonder_protects_entire_empire_with_icbm_75_and_tactical_37_5_odds() {
        // Place SDI Defense in a distant city (15 tiles away from the target)
        // belonging to civ 1; it still protects civ 1's target city.
        let mut icbm_intercepts = 0;
        let mut tact_intercepts = 0;
        let trials = 400;
        for seed in 0..trials {
            for (kind, counter) in [
                (UnitKind::ICBM, &mut icbm_intercepts),
                (UnitKind::TacticalNuke, &mut tact_intercepts),
            ] {
                let mut game = nuclear_game();
                let target = (10, 20);
                let distant_sdi = (25, 20);
                game.sign_pact(HUMAN, 1, Pact::War);
                game.create_city(1, target.0, target.1);
                game.create_city(1, distant_sdi.0, distant_sdi.1);
                let sdi_index = game.city_index_at(distant_sdi.0, distant_sdi.1).unwrap();
                game.cities[sdi_index].buildings.push(Building::SDI_DEFENSE);
                let weapon = Unit::new(99, HUMAN, kind, 10, 20);
                game.native_random = Some(FreecivRng::new(seed + 1));
                if game.nuclear_sdi_intercepts(&weapon, target.0, target.1) == Some(1) {
                    *counter += 1;
                }
            }
        }
        // Expected: ~300/400 (75%) for ICBM and ~150/400 (37.5%) for Tactical Nuke.
        assert!(
            (250..=350).contains(&icbm_intercepts),
            "ICBM SDI intercept count {icbm_intercepts}/{trials} out of 75% band"
        );
        assert!(
            (105..=195).contains(&tact_intercepts),
            "Tactical Nuke SDI intercept count {tact_intercepts}/{trials} out of 37.5% band"
        );
    }

    #[test]
    fn own_sdi_defense_does_not_intercept_own_nuclear_strike() {
        let mut game = nuclear_game();
        let (x, y) = (10, 20);
        game.sign_pact(HUMAN, 1, Pact::War);
        let nuke = make_nuke(&mut game, x, y);
        game.create_city(HUMAN, x, y);
        let city_index = game.city_index_at(x, y).unwrap();
        game.cities[city_index]
            .buildings
            .push(Building::SDI_DEFENSE);
        game.native_random = Some(FreecivRng::new(32));
        assert!(game.apply(Command::NuclearStrike(nuke, x, y)));
        assert!(game.events.iter().any(|event| event.contains("detonated")));
    }
}
