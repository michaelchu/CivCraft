//! Sentry wake-up when a non-allied unit comes into sight.

use super::*;

impl Simulation {
    pub(in crate::sim) fn wake_neighbor_sentries(&mut self, unit_id: u32) {
        let Some(moving) = self.unit_by_id(unit_id).cloned() else {
            return;
        };
        let alone_in_city = self.city_at(moving.x, moving.y).is_some()
            && self
                .units
                .iter()
                .filter(|unit| (unit.x, unit.y) == (moving.x, moving.y) && unit.transport.is_none())
                .count()
                == 1;
        let candidates: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.id != unit_id
                    && unit.state == UnitState::Sentry
                    && unit.transport.is_none()
                    && !self.allied(moving.owner, unit.owner)
                    && self.map_distance(moving.x, moving.y, unit.x, unit.y) <= 3
            })
            .map(|unit| unit.id)
            .collect();

        for sentry_id in candidates {
            let Some(sentry) = self.unit_by_id(sentry_id).cloned() else {
                continue;
            };
            let distance = self.map_distance(moving.x, moving.y, sentry.x, sentry.y);
            let radius_sq =
                self.unit_vision_radius_sq_for(sentry.owner, sentry.kind, sentry.x, sentry.y);
            if !self.within_vision_distance_sq(distance, radius_sq)
                || (!alone_in_city && !self.unit_is_visible_from_sources(sentry.owner, &moving))
            {
                continue;
            }
            if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == sentry_id) {
                unit.state = UnitState::Idle;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_map() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.base = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.hut = false;
        }
        game
    }

    fn place_sentry(game: &mut Simulation, at: (i32, i32)) -> u32 {
        let id = game.spawn_unit(1, UnitKind::Warrior, at.0, at.1);
        let unit = game.unit_by_id(id).unwrap();
        assert!(unit.moves_left > 0);
        game.units
            .iter_mut()
            .find(|unit| unit.id == id)
            .unwrap()
            .state = UnitState::Sentry;
        game.units
            .iter_mut()
            .find(|unit| unit.id == id)
            .unwrap()
            .moves_left = 0;
        id
    }

    #[test]
    fn entering_sentry_vision_wakes_the_sentry() {
        let mut game = flat_map();
        let sentry_at = (10, 20);
        let sentry = place_sentry(&mut game, sentry_at);
        let arrival = game
            .map_wrap
            .civ3_neighbor(sentry_at.0, sentry_at.1, 0)
            .unwrap();
        let start = game
            .map_wrap
            .civ3_neighbors(arrival.0, arrival.1)
            .find(|&(x, y)| {
                game.map_distance(sentry_at.0, sentry_at.1, x, y) > 1
                    && game.map_distance(sentry_at.0, sentry_at.1, x, y) <= 3
            })
            .unwrap();
        let mover = game.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);
        assert_eq!(game.unit_by_id(sentry).unwrap().state, UnitState::Sentry);
        let direction = game
            .map_wrap
            .civ3_neighbors(start.0, start.1)
            .position(|position| position == arrival)
            .unwrap();

        assert_ne!(
            game.try_move_unit_civ3(mover, direction),
            MoveResult::Blocked
        );
        assert_eq!(game.unit_by_id(sentry).unwrap().state, UnitState::Idle);
    }

    #[test]
    fn sentry_does_not_wake_for_units_outside_its_sight() {
        let mut game = flat_map();
        let sentry_at = (10, 20);
        let sentry = place_sentry(&mut game, sentry_at);
        let radius_sq =
            game.unit_vision_radius_sq_for(1, UnitKind::Warrior, sentry_at.0, sentry_at.1);
        let outside = (0..game.map_wrap.height())
            .flat_map(|y| (0..game.map_wrap.width()).map(move |x| (x, y)))
            .find(|&(x, y)| {
                let distance = game.map_distance(sentry_at.0, sentry_at.1, x, y);
                distance <= 3 && !game.within_vision_distance_sq(distance, radius_sq)
            })
            .unwrap();

        game.spawn_unit(HUMAN, UnitKind::Warrior, outside.0, outside.1);
        assert_eq!(game.unit_by_id(sentry).unwrap().state, UnitState::Sentry);
    }

    #[test]
    fn allied_units_do_not_wake_sentries() {
        let mut game = flat_map();
        let sentry_at = (10, 20);
        let sentry = place_sentry(&mut game, sentry_at);
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        let adjacent = game
            .map_wrap
            .civ3_neighbor(sentry_at.0, sentry_at.1, 0)
            .unwrap();

        game.spawn_unit(HUMAN, UnitKind::Warrior, adjacent.0, adjacent.1);
        assert_eq!(game.unit_by_id(sentry).unwrap().state, UnitState::Sentry);
    }
}
