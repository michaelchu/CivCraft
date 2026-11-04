use super::*;

const BARBARIAN_ONSET: i32 = 120;

impl Simulation {
    pub(super) fn enter_village(&mut self, unit_id: u32, x: i32, y: i32) {
        if !self.tile(x, y).hut {
            return;
        }
        self.tile_mut(x, y).hut = false;
        let Some(unit) = self.units.iter().find(|unit| unit.id == unit_id) else {
            return;
        };
        let owner = unit.owner;
        if owner == BARBARIANS {
            return;
        }

        let message = match self.rand_below(14) {
            0 => self.village_gold(owner, 25),
            1..=3 => self.village_gold(owner, 50),
            4 => self.village_gold(owner, 100),
            5..=7 => {
                let tech = self.village_tech(owner);
                format!("The village shared ancient knowledge of {}.", tech.name())
            }
            8..=9 => {
                let kind = if self.has_tech(owner, Tech::CHIVALRY) {
                    UnitKind::Knights
                } else if self.has_tech(owner, Tech::GUNPOWDER) {
                    UnitKind::Musketeers
                } else {
                    UnitKind::Archer
                };
                self.spawn_unit(owner, kind, x, y);
                format!("Friendly {} joined your expedition.", kind.name())
            }
            10 => self.village_barbarians(x, y),
            11 if self.can_found_at(x, y) => {
                let name = self.create_city(owner, x, y);
                format!("The friendly city of {name} joined your civilization.")
            }
            11 => {
                self.spawn_unit(owner, UnitKind::Settlers, x, y);
                "Friendly nomads joined your expedition as Settlers.".into()
            }
            12..=13 => {
                for dx in -5..=5 {
                    for dy in -5..=5 {
                        if isohex_vector_distance(dx, dy).pow(2) <= 30 {
                            let (nx, ny) = offset_native_by_map(x, y, dx, dy);
                            let tile = self.tile_mut(nx, ny);
                            tile.known_by[owner] = true;
                            if owner == HUMAN {
                                tile.seen = true;
                            }
                        }
                    }
                }
                "The village shared a map of the surrounding terrain.".into()
            }
            _ => unreachable!(),
        };
        if owner == HUMAN {
            self.add_event(message);
        }
        self.check_game_over();
    }

    fn village_gold(&mut self, owner: usize, gold: i32) -> String {
        self.players[owner].gold += gold;
        format!("The village gave your expedition {gold} gold.")
    }

    fn village_tech(&mut self, owner: usize) -> Tech {
        // Civ2Civ3 grants the cheapest available advance, breaking ties
        // randomly. Existing bulbs carry over because freecost is zero.
        let mut tech = Tech::FUTURE;
        let mut cheapest = i32::MAX;
        let mut ties = 0;
        for candidate in Tech::all() {
            if !self.tech_available(owner, candidate) {
                continue;
            }
            let cost = self.target_cost(owner, candidate);
            if cost < cheapest {
                cheapest = cost;
                ties = 1;
                tech = candidate;
            } else if cost == cheapest {
                ties += 1;
                if self.rand_below(ties) == 0 {
                    tech = candidate;
                }
            }
        }
        let was_researching = self.players[owner].researching == tech;
        self.discover(owner, tech);
        if was_researching {
            self.players[owner].researching = self.research_target(owner);
        }
        tech
    }

    fn village_barbarians(&mut self, x: i32, y: i32) -> String {
        if self
            .cities
            .iter()
            .any(|city| self.map_distance(x, y, city.x, city.y) <= 5)
        {
            return "The village was abandoned.".into();
        }

        if self.turn >= BARBARIAN_ONSET {
            let (land, water): (Vec<_>, Vec<_>) = ISOHEX_DIRS
                .iter()
                .copied()
                .filter(|&(dx, dy)| {
                    let (nx, ny) = offset_native_by_map(x, y, dx, dy);
                    self.units
                        .iter()
                        .all(|unit| (unit.x, unit.y) != (nx, ny) || unit.owner == BARBARIANS)
                })
                .partition(|&(dx, dy)| {
                    let (nx, ny) = offset_native_by_map(x, y, dx, dy);
                    !self.tile(nx, ny).terrain.is_water()
                });

            // Advanced roles replace the basic roster once at least three
            // civilizations can build them. Each raider is drawn separately.
            let advanced: Vec<_> = [
                (UnitKind::Musketeers, Tech::GUNPOWDER),
                (UnitKind::Partisan, Tech::GUERILLA_WARFARE),
                (UnitKind::Knights, Tech::CHIVALRY),
                (UnitKind::Cannon, Tech::METALLURGY),
            ]
            .into_iter()
            .filter_map(|(kind, tech)| {
                (self.players[..PLAYER_COUNT]
                    .iter()
                    .filter(|player| player.known.contains(&tech))
                    .count()
                    >= 3)
                    .then_some(kind)
            })
            .collect();
            let roster = if advanced.is_empty() {
                &[UnitKind::Horseman][..]
            } else {
                &advanced
            };
            let count = 3 + self.rand_below(4);
            let raiders: Vec<_> = (0..count)
                .map(|_| {
                    let kind = roster[self.rand_below(roster.len() as i32) as usize];
                    self.spawn_unit(BARBARIANS, kind, x, y)
                })
                .collect();

            let boat = if land.len() < 3 && !water.is_empty() {
                let (dx, dy) = water[self.rand_below(water.len() as i32) as usize];
                let (nx, ny) = offset_native_by_map(x, y, dx, dy);
                let kind = [UnitKind::Caravel, UnitKind::Galleon][self.rand_below(2) as usize];
                Some((self.spawn_unit(BARBARIANS, kind, nx, ny), kind, nx, ny))
            } else {
                None
            };
            let mut loaded = 0;
            let mut stayed = false;
            for id in raiders {
                let raider = self.units.iter().find(|unit| unit.id == id).unwrap();
                if let Some((boat_id, kind, nx, ny)) = boat
                    && loaded < kind.transport_capacity()
                    && kind.can_carry(raider.kind)
                {
                    let raider = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
                    raider.transport = Some(boat_id);
                    raider.x = nx;
                    raider.y = ny;
                    raider.moves_left = 0;
                    loaded += 1;
                    continue;
                }

                let mut exits = land.clone();
                let mut escaped = false;
                while !exits.is_empty() {
                    let direction = self.rand_below(exits.len() as i32) as usize;
                    let (dx, dy) = exits.remove(direction);
                    if self.try_move_unit(id, dx, dy) == MoveResult::Moved {
                        escaped = true;
                        break;
                    }
                }
                stayed |= !escaped;
            }
            if !stayed {
                return if loaded > 0 {
                    "The village unleashed barbarians, some aboard a transport!".into()
                } else {
                    "The village unleashed a horde of barbarians!".into()
                };
            }
        }
        self.units
            .retain(|unit| (unit.x, unit.y) != (x, y) || unit.owner == BARBARIANS);
        "Barbarians killed your expedition.".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approach(roll: i32) -> (Simulation, u32, (i32, i32)) {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let explorer = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Explorer)
            .unwrap();
        let (id, x, y) = (explorer.id, explorer.x, explorer.y);
        let (nx, ny) = offset_native_by_map(x, y, 0, -1);
        game.tile_mut(nx, ny).terrain = Terrain::Plains;
        game.tile_mut(nx, ny).hut = true;
        game.rng = (1..)
            .find(|&state| {
                game.rng = state;
                game.rand_below(14) == roll
            })
            .unwrap();
        (game, id, (nx, ny))
    }

    #[test]
    fn entering_a_village_resolves_every_reference_roll_once() {
        for roll in 0..14 {
            let (mut game, id, (x, y)) = approach(roll);
            let before = game.units.len();
            let known = game.players[HUMAN].known.len();
            game.players[HUMAN].research = 7;
            assert!(game.apply(Command::Move(id, 0, -1)));
            assert!(!game.tile(x, y).hut);
            match roll {
                0 => assert_eq!(game.players[HUMAN].gold, 75),
                1..=3 => assert_eq!(game.players[HUMAN].gold, 100),
                4 => assert_eq!(game.players[HUMAN].gold, 150),
                5..=7 => {
                    assert_eq!(game.players[HUMAN].known.len(), known + 1);
                    assert_eq!(game.players[HUMAN].research, 7);
                }
                8..=9 => {
                    assert_eq!(game.units.len(), before + 1);
                    assert_eq!(game.units.last().unwrap().kind, UnitKind::Archer);
                }
                10 => assert!(game.units.iter().all(|unit| unit.id != id)),
                11 => {
                    assert!(game.cities.iter().any(|city| (city.x, city.y) == (x, y)));
                    assert!(game.units.iter().any(|unit| unit.id == id));
                }
                _ => {
                    let (nx, ny) = offset_native_by_map(x, y, 5, 0);
                    let tile = game.tile(nx, ny);
                    assert!(tile.seen && tile.known_by[HUMAN] && !tile.visible);
                }
            }
            let events = game.events.clone();
            game.enter_village(id, x, y);
            assert_eq!(game.events, events);
        }
    }

    #[test]
    fn villages_offer_nomads_or_leave_a_city_nearby_unharmed() {
        for roll in [10, 11] {
            let (mut game, id, (x, y)) = approach(roll);
            let (cx, cy) = offset_native_by_map(x, y, 0, 1);
            game.create_city(HUMAN, cx, cy);
            let before = game.units.len();
            assert!(game.apply(Command::Move(id, 0, -1)));
            assert_eq!(game.cities.len(), 1);
            assert!(game.units.iter().any(|unit| unit.id == id));
            assert_eq!(game.units.len(), before + usize::from(roll == 11));
            if roll == 11 {
                assert_eq!(game.units.last().unwrap().kind, UnitKind::Settlers);
            }
        }
    }

    #[test]
    fn late_hostile_village_spawns_land_raiders() {
        let (mut game, id, (x, y)) = approach(10);
        game.turn = BARBARIAN_ONSET;
        for (dx, dy) in ISOHEX_DIRS {
            let (nx, ny) = offset_native_by_map(x, y, dx, dy);
            game.tile_mut(nx, ny).terrain = Terrain::Plains;
        }
        assert!(game.apply(Command::Move(id, 0, -1)));
        let raiders = game.units.iter().filter(|unit| unit.owner == BARBARIANS);
        assert!(
            raiders
                .clone()
                .all(|unit| game.map_distance(x, y, unit.x, unit.y) == 1)
        );
        assert!((3..=6).contains(&raiders.count()));
        assert!(game.units.iter().any(|unit| unit.id == id));
    }
    #[test]
    fn advanced_raiders_require_three_civilizations_with_the_technology() {
        for civilizations in [2, 3] {
            let (mut game, id, (x, y)) = approach(10);
            game.turn = BARBARIAN_ONSET;
            for player in &mut game.players[..civilizations] {
                player.known.push(Tech::METALLURGY);
            }
            for (dx, dy) in ISOHEX_DIRS {
                let (nx, ny) = offset_native_by_map(x, y, dx, dy);
                game.tile_mut(nx, ny).terrain = Terrain::Plains;
            }
            assert!(game.apply(Command::Move(id, 0, -1)));
            let expected = if civilizations == 3 {
                UnitKind::Cannon
            } else {
                UnitKind::Horseman
            };
            let raiders: Vec<_> = game
                .units
                .iter()
                .filter(|unit| unit.owner == BARBARIANS)
                .collect();
            assert!((3..=6).contains(&raiders.len()));
            assert!(raiders.iter().all(|unit| unit.kind == expected));
        }
    }

    #[test]
    fn island_transport_loads_cargo_and_carries_it_to_a_landing() {
        let (mut game, id, (x, y)) = approach(10);
        game.turn = BARBARIAN_ONSET;
        for (dx, dy) in ISOHEX_DIRS {
            let (nx, ny) = offset_native_by_map(x, y, dx, dy);
            game.tile_mut(nx, ny).terrain = if (dx, dy) == (0, 1) {
                Terrain::Plains
            } else {
                Terrain::Ocean
            };
        }
        assert!(game.apply(Command::Move(id, 0, -1)));
        let boat = game
            .units
            .iter()
            .find(|unit| unit.kind.transport_capacity() > 0)
            .unwrap()
            .clone();
        let cargo: Vec<_> = game
            .units
            .iter()
            .filter(|unit| unit.transport == Some(boat.id))
            .map(|unit| unit.id)
            .collect();
        assert!(!cargo.is_empty() && cargo.len() <= boat.kind.transport_capacity());
        let stayed = game
            .units
            .iter()
            .any(|unit| unit.owner == BARBARIANS && (unit.x, unit.y) == (x, y));
        assert_eq!(game.units.iter().any(|unit| unit.id == id), !stayed);
        let (sx, sy) = offset_native_by_map(boat.x, boat.y, 1, 0);
        game.tile_mut(sx, sy).terrain = Terrain::Ocean;
        assert_eq!(game.try_move_unit(boat.id, 1, 0), MoveResult::Moved);
        for id in cargo {
            let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
            assert_eq!((unit.x, unit.y), (sx, sy));
            unit.moves_left = MOVE_FRAGMENTS;
            let (lx, ly) = offset_native_by_map(sx, sy, 1, 0);
            game.tile_mut(lx, ly).terrain = Terrain::Plains;
            assert_eq!(game.try_move_unit(id, 1, 0), MoveResult::Moved);
            let unit = game.units.iter().find(|unit| unit.id == id).unwrap();
            assert_eq!((unit.x, unit.y, unit.transport), (lx, ly, None));
        }
    }
}
