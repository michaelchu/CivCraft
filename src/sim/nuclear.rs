//! Implements nuclear behavior using the selected Freeciv action and effect
//! rules.

use super::*;
use crate::rules::engine_rules::RulesetData;

fn nuclear_action_consumes_weapon(action: &str) -> bool {
    static CONSUMES: std::sync::OnceLock<[bool; 3]> = std::sync::OnceLock::new();
    let cached = CONSUMES.get_or_init(|| {
        let rules = RulesetData::campaign_default();
        let read = |key: &str| {
            rules
                .required_boolean_parameter("actions.ruleset", "actions", key)
                .unwrap_or_else(|error| panic!("{error}"))
        };
        [
            read("explode_nuclear_consuming_always"),
            read("nuke_city_consuming_always"),
            read("nuke_units_consuming_always"),
        ]
    });
    match action {
        "Explode Nuclear" => cached[0],
        "Nuke City" => cached[1],
        "Nuke Units" => cached[2],
        _ => panic!("unsupported nuclear action {action:?}"),
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
            && self.map_wrap.civ3_distance(unit.x, unit.y, x, y) > range
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
            .civ3_radius(x, y, 1)
            .into_iter()
            .map(|(tx, ty, _)| (tx, ty))
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
        let value = context.effect(EffectKind::NukeProof);
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
            self.remove_city_because(city_id, lifecycle::CityRemoval::Nuclear, None);
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
        if let Some(defending_city_owner) = self.nuclear_sdi_intercepts(weapon, action, x, y) {
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
        let combat_rules = RulesetData::campaign_default()
            .combat_settings()
            .expect("selected C2C3 nuclear combat settings are valid");

        let mut casualties = 0;
        let mut cities_hit = 0;
        let mut fallout_tiles = 0;
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
                if self
                    .unit_by_id(unit_id)
                    .is_none_or(|unit| (unit.x, unit.y) != (tx, ty))
                {
                    continue;
                }
                if city_tile
                    && self.rand_below(100) < combat_rules.nuke_defender_survival_chance_pct
                {
                    continue;
                }
                let before = self.units.len();
                self.remove_units_with_cargo_because(
                    vec![unit_id],
                    lifecycle::UnitRemoval::Nuclear,
                );
                let removed = before.saturating_sub(self.units.len()) as i32;
                casualties += removed;
                self.nation_scores[weapon.owner].units_killed += removed;
            }
            if let Some(index) = self.city_index_at(tx, ty) {
                cities_hit += 1;
                let city_id = self.cities[index].id;
                self.apply_nuclear_city_population_loss(city_id, combat_rules.nuke_pop_loss_pct);
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
        self.add_event(format!(
            "A nuclear weapon detonated, destroying {casualties} units."
        ));
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

    #[test]
    fn nuclear_actions_use_unit_ranges_and_selected_c2c3_enablers() {
        let mut game = nuclear_game();
        let (x, y) = (10, 20);
        game.sign_pact(HUMAN, 1, Pact::War);
        let nuke = make_nuke(&mut game, x, y);
        let unit = game.unit_by_id(nuke).unwrap().clone();
        assert!(game.can_launch_nuclear_strike_at(&unit, x, y));

        let adjacent_city = game.map_wrap.civ3_offset(x, y, 2, 0).unwrap();
        game.create_city(1, adjacent_city.0, adjacent_city.1);
        game.tile_mut(adjacent_city.0, adjacent_city.1)
            .known_by
            .set(HUMAN, true);
        let city_nuke = game.unit_by_id(nuke).unwrap().clone();
        assert!(game.can_launch_nuclear_strike_at(&city_nuke, adjacent_city.0, adjacent_city.1));

        let empty_units_tile = game.map_wrap.civ3_offset(x, y, 0, 2).unwrap();
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

        let adjacent_units = game.map_wrap.civ3_offset(x, y, -2, 0).unwrap();
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
        let target = game.map_wrap.civ3_offset(x, y, 2, 0).unwrap();

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

    #[test]
    fn civ3_nuclear_blast_covers_full_three_by_three_square() {
        let game = nuclear_game();
        let (x, y) = (10, 20);
        let affected = game.nuclear_blast_tiles(x, y);
        assert_eq!(affected.len(), 9);
        assert!(affected.contains(&(x, y)));
        assert!(
            affected
                .iter()
                .all(|&(tx, ty)| game.map_wrap.civ3_distance(x, y, tx, ty) <= 1)
        );
    }

    #[test]
    fn civ3_nuke_pollutes_all_nonwater_noncity_land_in_three_by_three_and_clears_improvements() {
        let mut game = nuclear_game();
        let (x, y) = (10, 20);
        game.create_city(HUMAN, x, y);
        let city_index = game.city_index_at(x, y).unwrap();
        game.cities[city_index].size = 10;
        game.cities[city_index].food = 100;
        let nearby = game.map_wrap.civ3_neighbor(x, y, 2).unwrap();
        game.tile_mut(nearby.0, nearby.1).owner = Some(HUMAN);
        game.tile_mut(nearby.0, nearby.1).road = true;
        game.tile_mut(nearby.0, nearby.1).mine = true;
        let ocean = game.map_wrap.civ3_neighbor(x, y, 6).unwrap();
        game.tile_mut(ocean.0, ocean.1).terrain = Terrain::Ocean;
        let coast = game.map_wrap.civ3_neighbor(x, y, 0).unwrap();
        game.tile_mut(coast.0, coast.1).terrain = Terrain::Coast;
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
        game.native_random = Some(FreecivRng::new(87));

        let blast_tiles = game.nuclear_blast_tiles(x, y);
        assert_eq!(blast_tiles.len(), 9);
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
