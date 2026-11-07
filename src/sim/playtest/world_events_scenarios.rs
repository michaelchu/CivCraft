use super::super::terrain_work::neighbors;
use super::super::villages::BARBARIAN_ONSET;
use super::*;

impl Simulation {
    pub(crate) fn run_plague_playtest(&mut self, protected: bool) -> i32 {
        let index = self
            .cities
            .iter()
            .position(|city| city.owner == HUMAN)
            .unwrap();
        let city = &mut self.cities[index];
        city.size = 9;
        if protected {
            city.buildings.push(Building::AQUEDUCT);
        }
        self.set_random_seed(92); // Native C oracle: plague roll 27/1000.
        self.check_city_plague(index, 0);
        self.cities[index].size
    }

    pub(crate) fn run_disaster_playtest(&mut self, nuclear: bool) -> bool {
        let index = self
            .cities
            .iter()
            .position(|city| city.owner == HUMAN)
            .unwrap();
        self.cities[index].size = 2;
        let plant = if nuclear {
            Building::NUCLEAR_PLANT
        } else {
            Building::MFG_PLANT
        };
        self.cities[index].buildings.push(plant);
        // Native C oracle: the fifth/ fourth frequency draw is 183/738.
        self.set_random_seed(if nuclear { 171 } else { 1110 });
        self.check_disasters();
        let city = &self.cities[index];
        let contamination = if nuclear {
            Contamination::Fallout
        } else {
            Contamination::Pollution
        };
        city.size == 1
            && Self::city_radius(city)
                .any(|(_, _, x, y)| self.tile(x, y).has_contamination(contamination))
    }

    pub(crate) fn climate_playtest_setup(&mut self, warming: bool) {
        for (index, tile) in self.tiles.iter_mut().enumerate() {
            tile.terrain = Terrain::Plains;
            tile.river = false;
            tile.pollution = warming && index < 128;
            tile.fallout = !warming && index < 128;
        }
    }

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
        let index = self.map_wrap.index(x, y);
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
