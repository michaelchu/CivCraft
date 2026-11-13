//! Automated gameplay scenarios covering world events behavior.

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
        let nearby_contamination =
            super::super::pollution::C2C3_CITY_MAP_RADIUS_5
                .iter()
                .any(|&(dx, dy)| {
                    self.map_wrap
                        .offset(city.x, city.y, dx, dy)
                        .is_some_and(|(x, y)| self.tile(x, y).has_contamination(contamination))
                });
        city.size == 1 && nearby_contamination
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
        self.cities.clear();
        if advanced {
            for owner in 0..3 {
                // Isolate the BarbarianSeaTech roster from Musketeer techs so
                // the seed has a single eligible advanced sea-raider type.
                self.players[owner].known = vec![Tech::AMPHIBIOUS_WARFARE];
            }
        }

        // The fixed seed was selected for the historical random stream. Pick
        // its sampled tile from the active topology so this fixture survives
        // map-dimension changes without changing the random draw sequence.
        self.set_random_seed(42_907);
        let random_before_probe = self.random_snapshot();
        let index = self.rand_below(self.tiles.len() as i32) as usize;
        self.restore_random(random_before_probe);
        let (x, y) = self.map_wrap.position(index);
        let (city_x, city_y) = (0..self.tiles.len())
            .map(|index| self.map_wrap.position(index))
            .min_by_key(|&(cx, cy)| (self.map_distance(x, y, cx, cy) - 6).abs())
            .expect("the map contains a city site six tiles from the uprising");
        let tile = self.tile_mut(x, y);
        tile.terrain = if sea {
            Terrain::Sea
        } else {
            Terrain::Grassland
        };
        tile.known_by[HUMAN] = !sea;
        tile.hut = true;
        let (lx, ly) = neighbors(x, y).next().unwrap();
        self.tile_mut(lx, ly).terrain = Terrain::Grassland;
        self.tile_mut(city_x, city_y).terrain = Terrain::Grassland;
        self.create_city(HUMAN, city_x, city_y);
        if advanced {
            // Freeciv's find_a_unit_type() only uses a technology role when
            // more than two living civilizations can build it. This fixture
            // cleared the normal roster, so keep two additional civilizations
            // alive outside the uprising area to exercise that source path.
            let random_before_setup = self.random_snapshot();
            for owner in 1..3 {
                let (px, py) = (0..self.tiles.len())
                    .map(|index| self.map_wrap.position(index))
                    .find(|&(px, py)| {
                        self.map_distance(x, y, px, py) >= 8
                            && self.map_distance(city_x, city_y, px, py) >= 8
                            && !self.units.iter().any(|unit| (unit.x, unit.y) == (px, py))
                    })
                    .expect("the map contains distant sites for uprising eligibility players");
                self.tile_mut(px, py).terrain = Terrain::Grassland;
                self.spawn_unit(owner, UnitKind::Warrior, px, py);
            }
            self.restore_random(random_before_setup);
        }

        // The seed also yields an open uprising roll and a successful
        // Despotism unrest roll. Check before the real call, then restore so
        // the onset gate and spawn consume the same draws as an actual turn.
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
        let gated = !self.units.iter().any(|unit| unit.owner == BARBARIANS) && self.tile(x, y).hut;
        self.turn += 1;
        self.summon_barbarians();
        let boat = self.units.iter().find(|unit| {
            unit.owner == BARBARIANS && unit.kind.is_naval() && unit.kind.transport_capacity() > 0
        });
        let composition = if sea {
            boat.is_some_and(|boat| {
                (2..=boat.kind.transport_capacity()).contains(&self.cargo_count(boat.id))
            })
        } else {
            (2..=4).contains(
                &self
                    .units
                    .iter()
                    .filter(|unit| unit.kind == UnitKind::Horseman)
                    .count(),
            )
        };
        let expected = if advanced {
            UnitKind::Marine
        } else {
            UnitKind::Swordsman
        };
        let sea_roster = !sea
            || boat.is_some_and(|boat| {
                self.units
                    .iter()
                    .any(|unit| unit.kind == expected && unit.transport == Some(boat.id))
            });
        gated && composition && sea_roster && !self.tile(x, y).hut
    }
}
