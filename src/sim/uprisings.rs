use super::{terrain_work::neighbors, villages::BARBARIAN_ONSET, *};

impl Simulation {
    pub(super) fn summon_barbarians(&mut self) {
        if self.turn < BARBARIAN_ONSET {
            return;
        }
        // Normal barbarian activity makes one attempt per 2,000 tiles.
        for _ in 0..(self.tiles.len() / 2000).max(1) {
            let (x, y) = Self::xy(self.rand_below(self.tiles.len() as i32) as usize);
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
            .chain(neighbors(x, y))
            .find(|&(sx, sy)| !self.units.iter().any(|unit| (unit.x, unit.y) == (sx, sy)))
        else {
            return;
        };
        let tile = self.tile(sx, sy);
        let sea = tile.terrain.is_water();
        let seen = tile.visible;
        if sea {
            if !self.tiles.iter().enumerate().any(|(index, tile)| {
                let (tx, ty) = Self::xy(index);
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
            &[UnitKind::Musketeers, UnitKind::Marines][..]
        } else {
            &[
                UnitKind::Musketeers,
                UnitKind::Partisan,
                UnitKind::Knights,
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
                &[UnitKind::Legion][..]
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
        self.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, sx, sy);
        self.units.last_mut().unwrap().transport = carrier.map(|(id, _)| id);
        self.players[BARBARIANS].gold += 100;
        if victim == HUMAN && (!sea || seen) {
            self.add_event(if sea {
                format!("Sea raiders spotted near {city_name}!")
            } else {
                format!("Native unrest near {city_name}!")
            });
        }
    }
}

impl Simulation {
    pub(crate) fn uprising_playtest(&mut self, sea: bool, advanced: bool) -> bool {
        self.units.clear();
        if advanced {
            for owner in 0..3 {
                self.players[owner].known.push(Tech::AMPHIBIOUS_WARFARE);
            }
        }
        self.create_city(HUMAN, 12, 24);
        let (x, y) = offset_native_by_map(12, 24, 6, 0);
        let tile = self.tile_mut(x, y);
        tile.terrain = if sea {
            Terrain::Ocean
        } else {
            Terrain::Grassland
        };
        tile.known_by[HUMAN] = !sea;
        tile.hut = true;
        let (lx, ly) = neighbors(x, y).next().unwrap();
        self.tile_mut(lx, ly).terrain = Terrain::Grassland;
        let index = Self::idx(x, y);
        // Native C-oracle seed: tile 975 of 2048, uprising gate 0/30,
        // and a successful Despotism unrest roll. Check instead of a costly
        // debug-build seed search (each seed heats the table 10000 times).
        self.set_random_seed(42_907);
        let snapshot = self.random_snapshot();
        let targeted = self.rand_below(self.tiles.len() as i32) as usize == index
            && self.rand_below(30) == 0
            && self.rand_below(100) <= self.players[HUMAN].government.civil_war_chance();
        self.restore_random(snapshot);
        if !targeted {
            return false;
        }
        self.turn = BARBARIAN_ONSET - 1;
        self.summon_barbarians();
        let gated = self.units.is_empty() && self.tile(x, y).hut;
        self.turn += 1;
        self.summon_barbarians();
        let leader = self
            .units
            .iter()
            .find(|unit| unit.kind == UnitKind::BarbarianLeader);
        let Some(leader) = leader else {
            return false;
        };
        let composition = if sea {
            leader.transport.is_some_and(|id| {
                self.units.iter().any(|boat| {
                    boat.id == id
                        && boat.kind.is_naval()
                        && (2..=boat.kind.transport_capacity()).contains(&self.cargo_count(id))
                })
            })
        } else {
            leader.transport.is_none()
                && (2..=4).contains(
                    &self
                        .units
                        .iter()
                        .filter(|unit| unit.kind == UnitKind::Horseman)
                        .count(),
                )
        };
        let expected = if advanced {
            UnitKind::Marines
        } else {
            UnitKind::Legion
        };
        let sea_roster = !sea
            || self
                .units
                .iter()
                .any(|unit| unit.kind == expected && unit.transport == leader.transport);
        gated && composition && sea_roster && !self.tile(x, y).hut
    }
}
