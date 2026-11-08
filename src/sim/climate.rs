//! Climate pressure and its effects on the game world.

#[cfg(test)]
use super::terrain_work::neighbors;
use super::*;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(super) struct ClimatePressure {
    accumulated: i32,
    level: i32,
}

const MAP_AREA: i32 = MAP_W * MAP_H;

impl ClimatePressure {
    pub(super) fn new(area: usize) -> Self {
        Self {
            accumulated: 0,
            level: (area as i32 + 499) / 500,
        }
    }
}
impl Default for ClimatePressure {
    fn default() -> Self {
        Self {
            accumulated: 0,
            level: (MAP_AREA + 499) / 500,
        }
    }
}

impl Terrain {
    fn climate_results(self, warming: bool) -> (Option<Self>, Option<Self>) {
        use Terrain::*;
        match (warming, self) {
            (true, Lake | Desert) => (Some(Swamp), None),
            (true, Glacier) => (Some(Lake), Some(Tundra)),
            (true, Forest) => (Some(Jungle), None),
            (true, Grassland | Tundra) => (Some(Swamp), Some(Swamp)),
            (true, Plains) => (Some(Swamp), Some(Desert)),
            (true, Swamp) => (Some(Desert), None),
            (false, Lake | Ocean) => (Some(Glacier), None),
            (false, Desert) => (Some(Tundra), Some(Tundra)),
            (false, Grassland) => (Some(Swamp), Some(Tundra)),
            (false, Jungle) => (None, Some(Forest)),
            (false, Plains) => (Some(Desert), Some(Desert)),
            (false, Swamp) => (Some(Tundra), Some(Desert)),
            (false, Tundra) => (Some(Glacier), Some(Desert)),
            _ => (None, None),
        }
    }
}

impl Simulation {
    pub(super) fn worker_climate_urgency(&self, target: Contamination) -> i32 {
        let index = usize::from(target == Contamination::Fallout);
        let pressure = self.climate_pressure[index];
        // autoworkers.c: cached heating/cooling from the last environmental
        // update, not a recount after workers have already cleaned tiles.
        50 * self.climate_current[index] / ((pressure.level + 1) / 2).max(1) + pressure.accumulated
    }

    pub(super) fn update_climate(&mut self) {
        for (index, contamination) in [Contamination::Pollution, Contamination::Fallout]
            .into_iter()
            .enumerate()
        {
            let count = self
                .tiles
                .iter()
                .filter(|tile| tile.has_contamination(contamination))
                .count() as i32;
            // srv_main.c:update_environmental_upset, default percent=100.
            self.climate_current[index] = count;
            let mut pressure = self.climate_pressure[index];
            pressure.accumulated += count;
            if pressure.accumulated < pressure.level {
                pressure.accumulated = 0;
            } else {
                pressure.accumulated -= pressure.level;
                if self.rand_below((self.tiles.len() as i32 + 19) / 20) < pressure.accumulated {
                    let strength = self.map_wrap.width() / 10
                        + self.map_wrap.height() / 10
                        + pressure.accumulated * 5;
                    self.change_climate(contamination == Contamination::Pollution, strength);
                    pressure.accumulated = 0;
                    pressure.level += (self.tiles.len() as i32 + 999) / 1000;
                }
            }
            self.climate_pressure[index] = pressure;
        }
    }

    fn climate_target(&self, x: i32, y: i32, warming: bool) -> Option<Terrain> {
        let tile = self.tile(x, y);
        let wet = tile.river
            || self.map_wrap.neighbors(x, y).any(|(nx, ny)| {
                let adjacent = self.tile(nx, ny);
                adjacent.terrain.is_water() || adjacent.river
            });
        let (wetter, drier) = tile.terrain.climate_results(warming);
        let (preferred, fallback) = if wet {
            (wetter, drier)
        } else {
            (drier, wetter)
        };
        let preferred = preferred?;
        if self.terrain_change_allowed(x, y, preferred) {
            return Some(preferred);
        }
        fallback.filter(|&target| self.terrain_change_allowed(x, y, target))
    }

    fn change_climate(&mut self, warming: bool, mut strength: i32) {
        let mut used = vec![false; self.tiles.len()];
        for _ in 0..self.tiles.len() {
            if strength == 0 {
                break;
            }
            let (index, x, y) = loop {
                let x = self.rand_below(self.map_wrap.width());
                let y = self.rand_below(self.map_wrap.height());
                let index = self.map_wrap.index(x, y);
                if !used[index] {
                    break (index, x, y);
                }
            };
            used[index] = true;
            if let Some(target) = self.climate_target(x, y, warming) {
                if self.tiles[index].terrain != target {
                    self.alter_terrain(x, y, target);
                }
                strength -= 1;
            }
        }
        self.add_event(if warming {
            "Global warming has occurred."
        } else {
            "Nuclear winter has occurred."
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_disaster_priority_uses_last_update_counts_until_the_next_update() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.climate_current = [6, 2];
        game.climate_pressure = [
            ClimatePressure {
                accumulated: 3,
                level: 5,
            },
            ClimatePressure {
                accumulated: 1,
                level: 4,
            },
        ];
        for tile in &mut game.tiles {
            tile.pollution = false;
            tile.fallout = false;
        }
        // Removing contamination during a worker phase does not alter the
        // environmental snapshot already supplied to that phase's advisor.
        assert_eq!(game.worker_climate_urgency(Contamination::Pollution), 103);
        assert_eq!(game.worker_climate_urgency(Contamination::Fallout), 51);
        game.update_climate();
        assert_eq!(game.climate_current, [0, 0]);
        assert_eq!(game.worker_climate_urgency(Contamination::Pollution), 0);
        assert_eq!(game.worker_climate_urgency(Contamination::Fallout), 0);
    }

    #[test]
    fn climate_obeys_moisture_and_checks_the_fallback_only_when_blocked() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        for (nx, ny) in neighbors(x, y) {
            let tile = game.tile_mut(nx, ny);
            tile.terrain = Terrain::Plains;
            tile.river = false;
        }
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        assert_eq!(game.climate_target(x, y, false), Some(Terrain::Tundra));
        game.tile_mut(x, y).river = true;
        assert_eq!(game.climate_target(x, y, false), Some(Terrain::Swamp));
        game.tile_mut(x, y).terrain = Terrain::Glacier;
        assert_eq!(game.climate_target(x, y, true), Some(Terrain::Tundra));
        game.tile_mut(x, y).terrain = Terrain::Jungle;
        assert_eq!(game.climate_target(x, y, false), None);
    }
}
