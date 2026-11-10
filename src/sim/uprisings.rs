//! Revolt risk and uprising resolution.

use super::{villages::BARBARIAN_ONSET, *};

impl Simulation {
    pub(super) fn summon_barbarians(&mut self) {
        if self.turn < BARBARIAN_ONSET {
            return;
        }
        // Normal barbarian activity makes one attempt per 2,000 tiles.
        for _ in 0..(self.tiles.len() / 2000).max(1) {
            let (x, y) = self
                .map_wrap
                .position(self.rand_below(self.tiles.len() as i32) as usize);
            self.try_uprising_at(x, y);
        }
    }

    fn try_uprising_at(&mut self, x: i32, y: i32) {
        if matches!(
            self.tile(x, y).terrain,
            Terrain::Lake | Terrain::Glacier | Terrain::Tundra
        ) {
            return;
        }
        let Some(city) = self
            .cities
            .iter()
            .min_by_key(|city| self.map_distance(x, y, city.x, city.y))
        else {
            return;
        };
        if !(5..=8).contains(&self.map_distance(x, y, city.x, city.y)) {
            return;
        }
        let victim = city.owner;
        let city_name = city.name.clone();
        let Some((sx, sy)) = std::iter::once((x, y))
            .chain(self.map_wrap.civ3_neighbors(x, y))
            .find(|&(sx, sy)| !self.units.iter().any(|unit| (unit.x, unit.y) == (sx, sy)))
        else {
            return;
        };
        let tile = self.tile(sx, sy);
        let sea = tile.terrain.is_water();
        let seen = tile.visible;
        if sea {
            if !self.tiles.iter().enumerate().any(|(index, tile)| {
                let (tx, ty) = self.map_wrap.position(index);
                !tile.terrain.is_water() && self.map_distance(sx, sy, tx, ty) <= 4
            }) {
                return;
            }
        } else if !tile.known_by[victim] {
            return;
        }

        let city_count = self
            .cities
            .iter()
            .filter(|city| city.owner == victim)
            .count() as i32;
        if self.rand_below(30) >= city_count
            || self.rand_below(100) > self.players[victim].government.civil_war_chance()
        {
            return;
        }
        self.tile_mut(sx, sy).hut = false;
        let mut tier = 1;
        let mut threshold = 10;
        while threshold <= city_count {
            tier += 1;
            threshold += threshold * 12 / 10 + 10;
        }
        let count = self.rand_below(3) + tier * 2;
        let carrier = if sea {
            let boat = [UnitKind::Caravel, UnitKind::Galleon][self.rand_below(2) as usize];
            Some((self.spawn_unit(BARBARIANS, boat, sx, sy), boat))
        } else {
            None
        };

        let advanced: Vec<_> = if sea {
            &[UnitKind::Musketman, UnitKind::Marine][..]
        } else {
            &[
                UnitKind::Musketman,
                UnitKind::Guerilla,
                UnitKind::Knight,
                UnitKind::Cannon,
            ]
        }
        .iter()
        .copied()
        .filter(|&kind| {
            (0..self.players.len())
                .filter(|&owner| owner != BARBARIANS && self.unit_available(owner, kind))
                .count()
                >= 3
        })
        .collect();
        let roster = if advanced.is_empty() {
            if sea {
                &[UnitKind::Swordsman][..]
            } else {
                &[UnitKind::Horseman][..]
            }
        } else {
            &advanced
        };
        let count = carrier.map_or(count, |(_, boat)| {
            count.min(boat.transport_capacity() as i32 - 1)
        });
        for _ in 0..count {
            let kind = roster[self.rand_below(roster.len() as i32) as usize];
            if carrier.is_none_or(|(_, boat)| boat.can_carry(kind)) {
                self.spawn_unit(BARBARIANS, kind, sx, sy);
                self.units.last_mut().unwrap().transport = carrier.map(|(id, _)| id);
            }
        }
        if victim == HUMAN && (!sea || seen) {
            self.add_event(if sea {
                format!("Sea raiders spotted near {city_name}!")
            } else {
                format!("Native unrest near {city_name}!")
            });
        }
    }
}
