//! Village discovery outcomes and rewards.

use super::*;

// Freeciv common/game.h::GAME_DEFAULT_ONSETBARBARIAN. C2C3 does not override
// the engine setting, so both hut barbarians and uprisings start on turn 60.
pub(super) const BARBARIAN_ONSET: i32 = 60;

impl Simulation {
    pub(super) fn enter_village(&mut self, unit_id: u32, x: i32, y: i32) {
        if !self.tile(x, y).hut {
            return;
        }
        self.tile_mut(x, y).hut = false;
        let Some(unit) = self.unit_by_id(unit_id) else {
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
                    UnitKind::Knight
                } else if self.has_tech(owner, Tech::GUNPOWDER) {
                    UnitKind::Musketman
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
                self.spawn_unit(owner, UnitKind::Settler, x, y);
                "Friendly nomads joined your expedition as Settlers.".into()
            }
            12..=13 => {
                let reveal_radius = (30.0_f32.sqrt().ceil()) as i32;
                for (nx, ny, distance) in self.map_wrap.civ3_radius(x, y, reveal_radius) {
                    if distance * distance > 30 {
                        continue;
                    }
                    let tile = self.tile_mut(nx, ny);
                    tile.known_by[owner] = true;
                    if owner == HUMAN {
                        tile.seen = true;
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
        if was_researching && !self.players[owner].ai {
            let free = self.players[owner].research;
            let next = self.choose_research_target(owner);
            self.choose_research_tech(owner, next, free);
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
            let (land, water): (Vec<_>, Vec<_>) = self
                .map_wrap
                .civ3_neighbors(x, y)
                .filter(|&(nx, ny)| {
                    self.units
                        .iter()
                        .all(|unit| (unit.x, unit.y) != (nx, ny) || unit.owner == BARBARIANS)
                })
                .partition(|&(nx, ny)| !self.tile(nx, ny).terrain.is_water());

            // Advanced roles replace the basic roster once at least three
            // civilizations can build them. Each raider is drawn separately.
            let advanced: Vec<_> = [
                (UnitKind::Musketman, Tech::GUNPOWDER),
                (UnitKind::Guerilla, Tech::NATIONALISM),
                (UnitKind::Knight, Tech::CHIVALRY),
                (UnitKind::Cannon, Tech::METALLURGY),
            ]
            .into_iter()
            .filter_map(|(kind, tech)| {
                (self
                    .players
                    .iter()
                    .enumerate()
                    .filter(|(owner, _)| self.has_tech(*owner, tech))
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
                let (nx, ny) = water[self.rand_below(water.len() as i32) as usize];
                let kind = [UnitKind::Caravel, UnitKind::Galleon][self.rand_below(2) as usize];
                Some((self.spawn_unit(BARBARIANS, kind, nx, ny), kind, nx, ny))
            } else {
                None
            };
            let mut loaded = 0;
            let mut stayed = false;
            for id in raiders {
                let raider = self.unit_by_id(id).unwrap();
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
                    let (nx, ny) = exits.remove(direction);
                    let direction = CIV3_DIRS.iter().position(|&(dx, dy)| {
                        self.map_wrap.civ3_offset(x, y, dx, dy) == Some((nx, ny))
                    });
                    if direction.is_some_and(|direction| {
                        self.try_move_unit_civ3(id, direction) == MoveResult::Moved
                    }) {
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
#[path = "villages/tests.rs"]
mod tests;
