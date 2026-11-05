use super::{terrain_work::neighbors, *};

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(super) struct ClimatePressure {
    accumulated: i32,
    level: i32,
}

const MAP_AREA: i32 = MAP_W * MAP_H;

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
            let mut pressure = self.climate_pressure[index];
            pressure.accumulated += count;
            if pressure.accumulated < pressure.level {
                pressure.accumulated = 0;
            } else {
                pressure.accumulated -= pressure.level;
                if self.rand_below((MAP_AREA + 19) / 20) < pressure.accumulated {
                    let strength = MAP_W / 10 + MAP_H / 10 + pressure.accumulated * 5;
                    self.change_climate(contamination == Contamination::Pollution, strength);
                    pressure.accumulated = 0;
                    pressure.level += (MAP_AREA + 999) / 1000;
                }
            }
            self.climate_pressure[index] = pressure;
        }
    }

    fn climate_target(&self, x: i32, y: i32, warming: bool) -> Option<Terrain> {
        let tile = self.tile(x, y);
        let wet = tile.river
            || neighbors(x, y).any(|(nx, ny)| {
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
                let x = self.rand_below(MAP_W);
                let y = self.rand_below(MAP_H);
                let index = Self::idx(x, y);
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
