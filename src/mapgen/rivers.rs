//! River generation and drainage selection for generated terrain.

use super::temperature::TemperatureMap;
use super::*;

/// Source-derived density and spring elevation values for the Freeciv path.
/// The two Freeciv terrain percentages share the same `adjust_terrain_param`
/// inputs and map-size-adjusted polar term as the terrain pass.
pub(super) fn freeciv_river_parameters(
    settings: MapGenerationSettings,
    tile_count: usize,
) -> (i32, usize, i32) {
    let (_, _, _, swamp_percent, _, river_percent) =
        super::terrain::freeciv_terrain_percentages(settings, tile_count);
    let target = river_percent as usize * tile_count * settings.landmass_percent as usize / 5325;
    let shore = settings.shore_level();
    let lowland = shore + 4 * swamp_percent * (1000 - shore) / 100;
    (river_percent, target, lowland)
}

impl super::Generator {
    // The target counts river tiles, not river systems.

    /// Enumerate adjacent map tiles using Civ III's eight-direction grid.
    /// Terrain and elevation still use transitional rectangular storage, so
    /// this adapter converts positions at the generator boundary.
    pub(super) fn civ3_neighbors(&self, tile: usize) -> impl Iterator<Item = usize> + '_ {
        let (x, y) = self.wrap.position(tile);
        (0..CIV3_DIRS.len()).filter_map(move |direction| {
            self.wrap
                .civ3_neighbor(x, y, direction)
                .map(|(nx, ny)| self.wrap.index(nx, ny))
        })
    }

    /// Civ III's 4×4 river sheets encode the four diagonal connections around
    /// each junction. Cardinal tile adjacencies collapse to the point-only
    /// atlas cell and cannot draw a continuous river segment.
    fn river_neighbors(&self, tile: usize) -> impl Iterator<Item = usize> + '_ {
        let (x, y) = self.wrap.position(tile);
        (1..CIV3_DIRS.len())
            .step_by(2)
            .filter_map(move |direction| {
                self.wrap
                    .civ3_neighbor(x, y, direction)
                    .map(|(nx, ny)| self.wrap.index(nx, ny))
            })
    }

    /// Freeciv's square-grid cardinal direction array is emitted in enum
    /// order North, West, East, South. Keep that order for random ties while
    /// mapping the four slots to Civ III's renderable NE, NW, SE, SW edges.
    fn freeciv_river_neighbors(&self, tile: usize) -> impl Iterator<Item = usize> + '_ {
        const CIV3_RIVER_DIRS_IN_FREECIV_ORDER: [usize; 4] = [1, 7, 3, 5];
        let (x, y) = self.wrap.position(tile);
        CIV3_RIVER_DIRS_IN_FREECIV_ORDER
            .into_iter()
            .filter_map(move |direction| {
                self.wrap
                    .civ3_neighbor(x, y, direction)
                    .map(|(nx, ny)| self.wrap.index(nx, ny))
            })
    }

    pub(super) fn place_rivers(
        &self,
        tiles: &mut [GeneratedTile],
        heights: &[i32],
        temperature: &[i32],
        freeciv_temperature: Option<&TemperatureMap>,
        terrain_properties: Option<&[TerrainMapgenProperties]>,
        rng: &mut MapRng,
    ) {
        assert!(
            freeciv_temperature.is_none() || terrain_properties.is_some(),
            "Freeciv river generation requires parsed terrain properties"
        );
        let (target, lowland) = if let Some(freeciv_temperature) = freeciv_temperature {
            let (_, target, lowland) =
                freeciv_river_parameters(freeciv_temperature.settings, self.wrap.tile_count());
            (target, lowland)
        } else {
            (
                6 * self.wrap.tile_count() * self.land_percent() as usize / 5325,
                self.lowland(),
            )
        };
        const MAX_TRIES: i32 = 32_767;
        let mut count = 0;
        for attempt in 0..MAX_TRIES {
            if count >= target {
                break;
            }
            let eligible_spring = |tile: usize| {
                !tiles[tile].terrain.is_water()
                    && !tiles[tile].river
                    && heights[tile] >= lowland
                    && freeciv_temperature
                        .map(|map| map.get(tile).is_not_frozen())
                        .unwrap_or(temperature[tile] >= VERY_COLD)
            };
            let spring = if freeciv_temperature.is_some() {
                freeciv_rand_map_pos_filtered(self.wrap.tile_count(), rng, eligible_spring)
            } else {
                random_tile(self.wrap.tile_count(), rng, eligible_spring)
            };
            let Some(spring) = spring else {
                break;
            };

            if let Some(properties) = terrain_properties
                && freeciv_temperature.is_some()
            {
                if !self.freeciv_spring_is_acceptable(tiles, spring, properties, attempt, MAX_TRIES)
                {
                    continue;
                }
            } else if self
                .civ3_neighbors(spring)
                .all(|next| Self::mountainous(tiles[next].terrain) > 0)
                && attempt < MAX_TRIES / 10 * 5
                || self
                    .civ3_neighbors(spring)
                    .filter(|&tile| tiles[tile].river || tiles[tile].terrain.is_water())
                    .count()
                    > 1
                || (Self::mountainous(tiles[spring].terrain) > 0 && attempt < MAX_TRIES / 10 * 6)
                || (tiles[spring].terrain == Terrain::Tundra && attempt < MAX_TRIES / 10 * 8)
                || (tiles[spring].terrain == Terrain::Desert && attempt < MAX_TRIES / 10 * 9)
            {
                continue;
            }
            if let Some(course) = self.trace_river(tiles, heights, spring, terrain_properties, rng)
            {
                count += course.len();
                for &tile in &course {
                    tiles[tile].river = true;
                }
                self.connect_river_course(tiles, &course, heights);
            }
        }
    }

    pub(super) fn freeciv_spring_is_acceptable(
        &self,
        tiles: &[GeneratedTile],
        spring: usize,
        terrain_properties: &[TerrainMapgenProperties],
        attempt: i32,
        max_tries: i32,
    ) -> bool {
        let neighbors: Vec<_> = self.freeciv_river_neighbors(spring).collect();
        let adjacent_water_and_river = neighbors
            .iter()
            .filter(|&&tile| tiles[tile].terrain.is_water() || tiles[tile].river)
            .count();
        let adjacent_mountainous_percent = self.adjacent_percentage(&neighbors, |tile| {
            self.freeciv_terrain_property(tiles[tile].terrain, terrain_properties, |properties| {
                properties.mountainous
            }) > 0
        });
        let property = |tile: usize, value: fn(&TerrainMapgenProperties) -> i32| {
            self.freeciv_terrain_property(tiles[tile].terrain, terrain_properties, value)
        };

        adjacent_water_and_river <= 1
            && (adjacent_mountainous_percent < 90 || attempt >= max_tries / 10 * 5)
            && (property(spring, |properties| properties.mountainous) == 0
                || attempt >= max_tries / 10 * 6)
            && (property(spring, |properties| properties.frozen) == 0
                || attempt >= max_tries / 10 * 8)
            && (property(spring, |properties| properties.dry) == 0 || attempt >= max_tries / 10 * 9)
    }

    fn connect_river_course(&self, tiles: &mut [GeneratedTile], course: &[usize], heights: &[i32]) {
        for pair in course.windows(2) {
            self.connect_river_step(tiles, pair[0], pair[1]);
        }
        let Some(&last) = course.last() else {
            return;
        };
        let outlet = self
            .river_neighbors(last)
            .filter(|&next| tiles[next].terrain.is_water() || tiles[next].river)
            .min_by_key(|&next| (heights[next], next));
        if let Some(outlet) = outlet {
            if tiles[outlet].terrain.is_water() {
                // The mouth belongs on the shoreline shared by the final land
                // tile and the water tile. Applying the ordinary course-step
                // conversion here would anchor it on the water tile's far
                // edge, shifting the delta a full junction offshore.
                self.connect_river_edge(tiles, last, outlet);
            } else {
                self.connect_river_step(tiles, last, outlet);
            }
        }
    }

    /// Convert a river's movement through tile centers into the tile-boundary
    /// flags consumed by Civ III's junction sprites. A northeast/southwest
    /// course step occupies the destination tile's southeast edge; the other
    /// diagonal occupies its southwest edge. Recording the travel direction
    /// itself produces reciprocal data but leaves the rendered junctions
    /// visually disconnected.
    fn connect_river_step(&self, tiles: &mut [GeneratedTile], from: usize, to: usize) {
        let (x, y) = self.wrap.position(from);
        let direction = (0..CIV3_DIRS.len()).find(|&direction| {
            self.wrap.civ3_neighbor(x, y, direction) == Some(self.wrap.position(to))
        });
        let Some(direction) = direction else {
            return;
        };
        let edge_direction = match direction {
            1 | 5 => 3, // NE/SW course: southeast boundary
            3 | 7 => 5, // SE/NW course: southwest boundary
            _ => return,
        };
        let (to_x, to_y) = self.wrap.position(to);
        let Some((edge_x, edge_y)) = self.wrap.civ3_neighbor(to_x, to_y, edge_direction) else {
            return;
        };
        self.connect_river_edge(tiles, to, self.wrap.index(edge_x, edge_y));

        // A turn can otherwise leave two boundary segments ending at adjacent
        // junctions. Bridge the neighboring southern edge when that earlier
        // segment is present so both atlas pieces meet at one junction.
        let stitch = match direction {
            7 => Some((3, 3, 5)), // incoming NW: inspect SE, add its SW edge
            1 => Some((5, 5, 3)), // incoming NE: inspect SW, add its SE edge
            _ => None,
        };
        let Some((neighbor_direction, required_edge, added_edge)) = stitch else {
            return;
        };
        let Some((neighbor_x, neighbor_y)) =
            self.wrap.civ3_neighbor(to_x, to_y, neighbor_direction)
        else {
            return;
        };
        let neighbor = self.wrap.index(neighbor_x, neighbor_y);
        if tiles[neighbor].river_edges & (1 << required_edge) == 0 {
            return;
        }
        let Some((stitch_x, stitch_y)) =
            self.wrap.civ3_neighbor(neighbor_x, neighbor_y, added_edge)
        else {
            return;
        };
        self.connect_river_edge(tiles, neighbor, self.wrap.index(stitch_x, stitch_y));
    }

    fn connect_river_edge(&self, tiles: &mut [GeneratedTile], from: usize, to: usize) {
        let (x, y) = self.wrap.position(from);
        let direction = (0..CIV3_DIRS.len()).find(|&direction| {
            self.wrap.civ3_neighbor(x, y, direction) == Some(self.wrap.position(to))
        });
        let Some(direction) = direction else {
            return;
        };
        tiles[from].river_edges |= 1 << direction;
        tiles[to].river_edges |= 1 << ((direction + 4) % 8);
    }

    fn mountainous(terrain: Terrain) -> i32 {
        match terrain {
            Terrain::Hills => 30,
            Terrain::Mountains => 70,
            _ => 0,
        }
    }

    fn wet(terrain: Terrain) -> i32 {
        match terrain {
            Terrain::Forest => 20,
            Terrain::Jungle => 50,
            Terrain::Marsh => 100,
            _ => 0,
        }
    }

    pub(super) fn trace_river(
        &self,
        tiles: &[GeneratedTile],
        heights: &[i32],
        spring: usize,
        terrain_properties: Option<&[TerrainMapgenProperties]>,
        rng: &mut MapRng,
    ) -> Option<Vec<usize>> {
        if let Some(properties) = terrain_properties {
            return self.trace_freeciv_river(tiles, heights, spring, properties, rng);
        }

        self.trace_opensimplex_river(tiles, heights, spring, rng)
    }

    /// Follow Freeciv's ordered `river_test_*` selection. The source walks
    /// cardinal square-grid directions; CivCraft uses the four diagonal
    /// junction directions that have continuous Civ III river-sheet edges.
    /// All route blocking and course adjacency use this same renderable graph.
    fn trace_freeciv_river(
        &self,
        tiles: &[GeneratedTile],
        heights: &[i32],
        spring: usize,
        terrain_properties: &[TerrainMapgenProperties],
        rng: &mut MapRng,
    ) -> Option<Vec<usize>> {
        let mut course = Vec::new();
        let mut blocked = vec![false; self.wrap.tile_count()];
        let mut current = spring;
        loop {
            course.push(current);
            if self
                .freeciv_river_neighbors(current)
                .any(|next| tiles[next].river)
                || self
                    .freeciv_river_neighbors(current)
                    .any(|next| tiles[next].terrain.is_water())
                || (self.freeciv_terrain_property(
                    tiles[current].terrain,
                    terrain_properties,
                    |properties| properties.frozen,
                ) > 0
                    && self.colatitude(current) * 5 < 4 * COLD)
            {
                return Some(course);
            }

            let next = self.select_freeciv_river_step(
                tiles,
                heights,
                terrain_properties,
                &blocked,
                current,
                rng,
            )?;

            // Freeciv's river_blockmark reserves the current tile and its
            // course neighbors after selecting the outgoing direction.
            blocked[current] = true;
            for neighbor in self.freeciv_river_neighbors(current) {
                blocked[neighbor] = true;
            }
            current = next;
        }
    }

    /// Apply Freeciv's test table in source order: blocked and river-grid
    /// failures are fatal, then each remaining test keeps only minimum-valued
    /// candidates. Its `FC_INFINITY - wet` tests are represented by negative
    /// wetness sums, which preserve the exact ordering without copying the
    /// unrelated engine infinity constant.
    pub(super) fn select_freeciv_river_step(
        &self,
        tiles: &[GeneratedTile],
        heights: &[i32],
        terrain_properties: &[TerrainMapgenProperties],
        blocked: &[bool],
        current: usize,
        rng: &mut MapRng,
    ) -> Option<usize> {
        let mut candidates: Vec<_> = self.freeciv_river_neighbors(current).collect();
        if candidates.is_empty() {
            return None;
        }

        for test in 0..9 {
            let value = |candidate: usize| -> i32 {
                let course_neighbors: Vec<_> = self.freeciv_river_neighbors(candidate).collect();
                let all_neighbors: Vec<_> = self.civ3_neighbors(candidate).collect();
                match test {
                    // river_test_blocked: candidate and every possible
                    // continuation must not already be reserved.
                    0 => i32::from(
                        blocked[candidate] || course_neighbors.iter().all(|&next| blocked[next]),
                    ),
                    // river_test_rivergrid: avoid a 2x2 grid of same-river
                    // tiles by making more than one adjacent river fatal.
                    1 => i32::from(
                        course_neighbors
                            .iter()
                            .filter(|&&next| tiles[next].river)
                            .count()
                            > 1,
                    ),
                    // river_test_highlands
                    2 => self.freeciv_terrain_property(
                        tiles[candidate].terrain,
                        terrain_properties,
                        |properties| properties.mountainous,
                    ),
                    // river_test_adjacent_ocean uses cardinal adjacency in
                    // Freeciv; adapt that four-way neighborhood to the four
                    // renderable Civ III course neighbors.
                    3 => {
                        100 - self.adjacent_percentage(&course_neighbors, |next| {
                            tiles[next].terrain.is_water()
                        })
                    }
                    // river_test_adjacent_river counts cardinal neighbors in
                    // Freeciv; use CivCraft's four river-course neighbors.
                    4 => {
                        100 - self.adjacent_percentage(&course_neighbors, |next| tiles[next].river)
                    }
                    // river_test_adjacent_highlands sums all adjacency.
                    5 => all_neighbors
                        .iter()
                        .map(|&next| {
                            self.freeciv_terrain_property(
                                tiles[next].terrain,
                                terrain_properties,
                                |properties| properties.mountainous,
                            )
                        })
                        .sum(),
                    // river_test_swamp: minimize FC_INFINITY - property_wet.
                    6 => -self.freeciv_terrain_property(
                        tiles[candidate].terrain,
                        terrain_properties,
                        |properties| properties.wet,
                    ),
                    // river_test_adjacent_swamp: maximize summed wetness.
                    7 => -all_neighbors
                        .iter()
                        .map(|&next| {
                            self.freeciv_terrain_property(
                                tiles[next].terrain,
                                terrain_properties,
                                |properties| properties.wet,
                            )
                        })
                        .sum::<i32>(),
                    // river_test_height_map
                    8 => heights[candidate],
                    _ => unreachable!(),
                }
            };
            let best = candidates.iter().map(|&next| value(next)).min()?;
            if test < 2 && best > 0 {
                return None;
            }
            candidates.retain(|&next| value(next) == best);
        }

        let selected = rng.below(candidates.len() as i32) as usize;
        candidates.get(selected).copied()
    }

    fn adjacent_percentage(&self, neighbors: &[usize], matches: impl Fn(usize) -> bool) -> i32 {
        let count = neighbors.iter().filter(|&&next| matches(next)).count();
        if count == 0 || neighbors.is_empty() {
            0
        } else {
            (count * 100 / neighbors.len()) as i32
        }
    }

    fn freeciv_terrain_property(
        &self,
        terrain: Terrain,
        terrain_properties: &[TerrainMapgenProperties],
        value: impl Fn(&TerrainMapgenProperties) -> i32,
    ) -> i32 {
        let rule_name = match terrain {
            Terrain::Desert => "Desert",
            Terrain::Forest => "Forest",
            Terrain::Grassland => "Grassland",
            Terrain::Hills => "Hills",
            Terrain::Jungle => "Jungle",
            Terrain::Mountains => "Mountains",
            Terrain::Plains => "Plains",
            Terrain::Marsh => "Swamp",
            Terrain::Tundra => "Tundra",
            Terrain::Glacier => "Glacier",
            _ => return 0,
        };
        terrain_properties
            .iter()
            .find(|properties| properties.rule_name.eq_ignore_ascii_case(rule_name))
            .map(value)
            .unwrap_or(0)
    }

    fn trace_opensimplex_river(
        &self,
        tiles: &[GeneratedTile],
        heights: &[i32],
        spring: usize,
        rng: &mut MapRng,
    ) -> Option<Vec<usize>> {
        let mut course = Vec::new();
        let mut blocked = vec![false; self.wrap.tile_count()];
        let mut current = spring;
        let water_count = |tile| {
            self.river_neighbors(tile)
                .filter(|&next| tiles[next].terrain.is_water())
                .count()
        };
        let river_count = |tile| {
            self.river_neighbors(tile)
                .filter(|&next| tiles[next].river)
                .count()
        };
        loop {
            course.push(current);
            if water_count(current) > 0
                || river_count(current) > 0
                || (tiles[current].terrain == Terrain::Tundra
                    && self.colatitude(current) * 5 < 4 * COLD)
            {
                return Some(course);
            }
            let score = |next: usize| {
                (
                    Self::mountainous(tiles[next].terrain),
                    std::cmp::Reverse(water_count(next)),
                    std::cmp::Reverse(river_count(next)),
                    self.civ3_neighbors(next)
                        .map(|tile| Self::mountainous(tiles[tile].terrain))
                        .sum::<i32>(),
                    std::cmp::Reverse(Self::wet(tiles[next].terrain)),
                    std::cmp::Reverse(
                        self.civ3_neighbors(next)
                            .map(|tile| Self::wet(tiles[tile].terrain))
                            .sum::<i32>(),
                    ),
                    heights[next],
                )
            };
            let mut candidates: Vec<_> = self
                .river_neighbors(current)
                .filter(|&next| {
                    !blocked[next]
                        && river_count(next) <= 1
                        && self.river_neighbors(next).any(|tile| !blocked[tile])
                })
                .map(|next| (score(next), next))
                .collect();
            candidates.sort_by_key(|&(score, _)| score);
            let best = candidates.first()?.0;
            let ties = candidates
                .iter()
                .take_while(|(score, _)| *score == best)
                .count();
            let next = candidates[rng.below(ties as i32) as usize].1;
            blocked[current] = true;
            for neighbor in self.river_neighbors(current) {
                blocked[neighbor] = true;
            }
            current = next;
        }
    }
}
