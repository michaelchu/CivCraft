//! Read-only goto presentation from the same observed-map search as GoTo.
//! Native client/goto.c::goto_tile_state uses zero-based arrival turns and
//! marks exhausted movement on intermediate path tiles and at the destination.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GotoPoint {
    pub(crate) x: i32,
    pub(crate) y: i32,
    /// Zero is reachable with the current movement allowance.
    pub(crate) turn: i32,
    pub(crate) moves_left: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GotoPreview {
    /// Includes the starting tile, followed by every actual planned step.
    pub(crate) points: Vec<GotoPoint>,
}

impl Simulation {
    pub(crate) fn preview_goto(&self, unit_id: u32, tx: i32, ty: i32) -> Option<GotoPreview> {
        if !(0..MAP_H).contains(&ty) {
            return None;
        }
        let unit = self
            .units
            .iter()
            .find(|unit| unit.id == unit_id && unit.owner == HUMAN)?;
        let (path, _) = self.search_route(unit, tx, ty, true, i32::MAX)?;
        let allowance = self.unit_move_rate(unit).max(1);
        let mut actor = unit.clone();
        let mut turn = 0;
        let mut remaining = unit.moves_left.max(0);
        let mut points = vec![GotoPoint {
            x: actor.x,
            y: actor.y,
            turn,
            moves_left: remaining,
        }];
        for (dx, dy) in path {
            if remaining == 0 {
                turn += 1;
                remaining = allowance;
            }
            actor.moves_left = remaining;
            let Some((x, y)) = self.map_wrap.offset(actor.x, actor.y, dx, dy) else {
                continue;
            };
            let remembered = self.observed_route_tile(x, y);
            // Match search_route's sole non-native destination exception:
            // boarding a visible allied transport on the final tile.
            let cost = self
                .observed_move_cost(&actor, x, y, remembered)
                .or_else(|| {
                    ((x, y) == (tx.rem_euclid(MAP_W), ty)
                        && self.tile(x, y).visible
                        && self.tile(x, y).terrain.is_water()
                        && self.boarding_target(unit, x, y).is_some())
                    .then_some(if unit.kind.stats().ignores_terrain {
                        2
                    } else {
                        MOVE_FRAGMENTS
                    })
                })?;
            remaining -= cost.min(remaining);
            (actor.x, actor.y) = (x, y);
            actor.moves_left = remaining;
            points.push(GotoPoint {
                x,
                y,
                turn,
                moves_left: remaining,
            });
        }
        Some(GotoPreview { points })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(kind: UnitKind) -> (Simulation, u32) {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.units.clear();
        sim.cities.clear();
        for tile in &mut sim.tiles {
            tile.terrain = Terrain::Grassland;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
            tile.base = None;
            tile.owner = None;
            tile.last_seen_owner = None;
            tile.visible = true;
            tile.seen = true;
        }
        let id = sim.spawn_unit(HUMAN, kind, 10, 24);
        sim.remember_route_tiles(false);
        (sim, id)
    }

    #[test]
    fn preview_uses_the_command_path_and_exact_movement_boundaries() {
        let (mut sim, id) = fixture(UnitKind::Warrior);
        let target = offset_native_by_map(10, 24, 3, 0);
        let unit = sim.units[0].clone();
        let path = sim.plan_route(&unit, target.0, target.1, true);
        let preview = sim.preview_goto(id, target.0, target.1).unwrap();
        assert_eq!(preview.points.len(), path.len() + 1);
        assert_eq!(
            preview.points.iter().map(|p| p.turn).collect::<Vec<_>>(),
            [0, 0, 1, 2]
        );
        assert!(preview.points[1..].iter().all(|p| p.moves_left == 0));
        assert!(sim.player_goto(id, target.0, target.1));
        assert_eq!(
            (sim.units[0].x, sim.units[0].y),
            (preview.points[1].x, preview.points[1].y)
        );
    }

    #[test]
    fn depleted_units_start_next_turn_and_roads_use_fragment_costs() {
        let (mut sim, id) = fixture(UnitKind::Warrior);
        for tile in &mut sim.tiles {
            tile.road = true;
        }
        sim.units[0].moves_left = 0;
        let target = offset_native_by_map(10, 24, 4, 0);
        let preview = sim.preview_goto(id, target.0, target.1).unwrap();
        assert_eq!(
            preview.points.iter().map(|p| p.turn).collect::<Vec<_>>(),
            [0, 1, 1, 1, 2]
        );
        assert_eq!(
            preview
                .points
                .iter()
                .map(|p| p.moves_left)
                .collect::<Vec<_>>(),
            [0, 4, 2, 0, 4]
        );
    }

    #[test]
    fn preview_is_read_only_and_does_not_reveal_unobserved_changes() {
        let (mut sim, id) = fixture(UnitKind::Warrior);
        let target = offset_native_by_map(10, 24, 2, 0);
        let original = sim.preview_goto(id, target.0, target.1);
        let rng = sim.rng;
        let native = sim.native_random.clone();
        for tile in &mut sim.tiles {
            tile.visible = false;
            tile.terrain = Terrain::Ocean;
            tile.road = true;
            tile.railroad = true;
        }
        // The actor's tile is observed in actual games; preserve it here too.
        sim.tile_mut(10, 24).terrain = Terrain::Grassland;
        assert_eq!(sim.preview_goto(id, target.0, target.1), original);
        assert_eq!(sim.rng, rng);
        assert_eq!(sim.native_random, native);
    }

    #[test]
    fn known_impassable_destinations_and_invalid_owners_have_no_preview() {
        let (mut sim, id) = fixture(UnitKind::Warrior);
        let target = offset_native_by_map(10, 24, 1, 0);
        sim.tile_mut(target.0, target.1).terrain = Terrain::Ocean;
        assert!(sim.preview_goto(id, target.0, target.1).is_none());
        assert!(sim.preview_goto(id, 0, MAP_H).is_none());
        sim.units[0].owner = 1;
        assert!(sim.preview_goto(id, 10, 24).is_none());
    }

    #[test]
    fn river_road_and_rail_previews_match_actual_goto_movement() {
        for (river, road, rail, remaining) in [
            (true, false, false, 0),
            (false, true, false, 0),
            (false, true, true, 3),
        ] {
            let (mut sim, id) = fixture(UnitKind::Warrior);
            for tile in &mut sim.tiles {
                tile.river = river;
                tile.road = road;
                tile.railroad = rail;
            }
            let target = offset_native_by_map(10, 24, 3, 0);
            let preview = sim.preview_goto(id, target.0, target.1).unwrap();
            let last = preview.points.last().unwrap();
            assert_eq!((last.turn, last.moves_left), (0, remaining));
            assert!(sim.apply(Command::GoTo(id, target.0, target.1)));
            let unit = sim.units.iter().find(|u| u.id == id).unwrap();
            assert_eq!(
                (unit.x, unit.y, unit.moves_left),
                (last.x, last.y, last.moves_left)
            );
        }
    }

    #[test]
    fn hostile_infrastructure_has_no_route_discount_in_preview_or_execution() {
        let (mut sim, id) = fixture(UnitKind::Warrior);
        for tile in &mut sim.tiles {
            tile.river = true;
            tile.road = true;
            tile.railroad = true;
            tile.owner = Some(1);
        }
        let target = offset_native_by_map(10, 24, 3, 0);
        let preview = sim.preview_goto(id, target.0, target.1).unwrap();
        assert_eq!(preview.points.last().unwrap().turn, 2);
        let first = &preview.points[1];
        assert_eq!(first.moves_left, 0);
        assert!(sim.apply(Command::GoTo(id, target.0, target.1)));
        let unit = sim.units.iter().find(|u| u.id == id).unwrap();
        assert_eq!(
            (unit.x, unit.y, unit.moves_left),
            (first.x, first.y, first.moves_left)
        );
    }

    #[test]
    fn wheeled_routes_obey_known_terrain_road_and_fort_nativity() {
        for terrain in [Terrain::Mountains, Terrain::Jungle, Terrain::Swamp] {
            let (mut sim, id) = fixture(UnitKind::Chariot);
            let target = offset_native_by_map(10, 24, 1, 0);
            sim.tile_mut(target.0, target.1).terrain = terrain;
            assert!(sim.preview_goto(id, target.0, target.1).is_none());
            for (road, rail, base) in [
                (true, false, None),
                (false, true, None),
                (false, false, Some(Base::Fort)),
                (false, false, Some(Base::Fortress)),
            ] {
                let mut allowed = sim.clone();
                let tile = allowed.tile_mut(target.0, target.1);
                tile.road = road;
                tile.railroad = rail;
                tile.base = base;
                let preview = allowed.preview_goto(id, target.0, target.1).unwrap();
                let last = preview.points.last().unwrap();
                assert!(allowed.apply(Command::GoTo(id, target.0, target.1)));
                let unit = allowed.units.iter().find(|u| u.id == id).unwrap();
                assert_eq!(
                    (unit.x, unit.y, unit.moves_left),
                    (last.x, last.y, last.moves_left)
                );
            }
        }
    }

    #[test]
    fn hidden_owner_and_base_mutations_do_not_change_observed_route_costs() {
        for kind in [UnitKind::Warrior, UnitKind::Chariot] {
            let (mut sim, id) = fixture(kind);
            let target = offset_native_by_map(10, 24, 2, 0);
            for tile in &mut sim.tiles {
                tile.terrain = Terrain::Mountains;
                tile.base = Some(Base::Fortress);
                tile.river = true;
                tile.road = true;
                tile.railroad = true;
            }
            sim.remember_route_tiles(false);
            sim.remember_render_map(&[vec![true; sim.tiles.len()]]);
            let before = sim.preview_goto(id, target.0, target.1).unwrap();
            for tile in &mut sim.tiles {
                tile.visible = false;
                tile.base = None;
                tile.owner = Some(1);
                tile.road = false;
                tile.railroad = false;
                tile.river = false;
            }
            // The actor remains visible: hidden mutations affect only the
            // upcoming path, not the actor's current movement allowance.
            let tile = sim.tile_mut(10, 24);
            tile.visible = true;
            tile.base = Some(Base::Fortress);
            tile.owner = None;
            tile.road = true;
            tile.railroad = true;
            tile.river = true;
            assert_eq!(sim.preview_goto(id, target.0, target.1), Some(before));
        }
    }

    #[test]
    fn boarding_preview_uses_the_same_nonnative_cost_as_actual_embarkation() {
        for kind in [UnitKind::Explorer, UnitKind::Warrior] {
            let (mut sim, id) = fixture(kind);
            let target = offset_native_by_map(10, 24, 1, 0);
            sim.tile_mut(target.0, target.1).terrain = Terrain::Ocean;
            let carrier = sim.spawn_unit(HUMAN, UnitKind::Transport, target.0, target.1);
            let preview = sim.preview_goto(id, target.0, target.1).unwrap();
            let last = preview.points.last().unwrap();
            assert!(sim.apply(Command::GoTo(id, target.0, target.1)));
            let unit = sim.units.iter().find(|u| u.id == id).unwrap();
            assert_eq!(unit.transport, Some(carrier));
            assert_eq!(
                (unit.x, unit.y, unit.moves_left),
                (last.x, last.y, last.moves_left)
            );
        }
    }

    #[test]
    fn remembered_road_fixture_is_independent_of_generated_rivers_and_old_ownership() {
        for seed in [42, 7, 777] {
            let mut sim = Simulation::new(crate::mapgen::generate(seed));
            // These are deliberately conflicting preconditions: neither
            // river discounts nor hostile road restrictions belong in the
            // fixture's three-step remembered-road versus two-step land test.
            for tile in &mut sim.tiles {
                tile.river = true;
                tile.owner = Some(1);
                tile.last_seen_owner = Some(1);
                tile.base = Some(Base::Fortress);
            }
            assert!(sim.remembered_route_playtest(), "seed {seed}");
        }
    }
}
