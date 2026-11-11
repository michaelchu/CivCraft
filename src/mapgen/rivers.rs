//! River generation and drainage selection for generated terrain.

use super::*;

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

    pub(super) fn place_rivers(
        &self,
        tiles: &mut [GeneratedTile],
        heights: &[i32],
        temperature: &[i32],
        rng: &mut MapRng,
    ) {
        let target: usize = 6 * self.wrap.tile_count() * self.land_percent() as usize / 5325;
        const MAX_TRIES: i32 = 32_767;
        let mut count = 0;
        for attempt in 0..MAX_TRIES {
            if count >= target {
                break;
            }
            let Some(spring) = random_tile(self.wrap.tile_count(), rng, |tile| {
                !tiles[tile].terrain.is_water()
                    && !tiles[tile].river
                    && temperature[tile] >= VERY_COLD
                    && heights[tile] >= self.lowland()
            }) else {
                break;
            };
            if self
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
            if let Some(course) = self.trace_river(tiles, heights, spring, rng) {
                count += course.len();
                for &tile in &course {
                    tiles[tile].river = true;
                }
                self.connect_river_course(tiles, &course, heights);
            }
        }
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

            // Prefer low, wet routes and merge into existing rivers or water.
            // The scoring is an interim heuristic; it is not evidence of
            // Civ III's original drainage algorithm.
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
            // Reserve the same four junction-link neighbors used by the
            // course graph. Blocking all eight Civ III neighbors also blocks
            // both possible turns after a diagonal step, leaving only the
            // straight-ahead continuation and producing ruler-straight rivers.
            for neighbor in self.river_neighbors(current) {
                blocked[neighbor] = true;
            }
            current = next;
        }
    }
}
