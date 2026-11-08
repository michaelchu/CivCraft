//! River generation and drainage selection for generated terrain.

use super::*;

impl super::Generator {
    // The target counts river tiles, not river systems.

    pub(super) fn place_rivers(
        &self,
        tiles: &mut [Tile],
        heights: &[i32],
        temperature: &[i32],
        rng: &mut MapRng,
    ) {
        let target: usize = 6 * self.wrap.tile_count() * LAND_PERCENT as usize / 5325;
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
                    && heights[tile] >= LOWLAND
            }) else {
                break;
            };
            if self
                .neighbors(spring)
                .all(|next| Self::mountainous(tiles[next].terrain) > 0)
                && attempt < MAX_TRIES / 10 * 5
                || self
                    .neighbors(spring)
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
                for tile in course {
                    tiles[tile].river = true;
                }
            }
        }
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
        tiles: &[Tile],
        heights: &[i32],
        spring: usize,
        rng: &mut MapRng,
    ) -> Option<Vec<usize>> {
        let mut course = Vec::new();
        let mut blocked = vec![false; self.wrap.tile_count()];
        let mut current = spring;
        let water_count = |tile| {
            self.neighbors(tile)
                .filter(|&next| tiles[next].terrain.is_water())
                .count()
        };
        let river_count = |tile| {
            self.neighbors(tile)
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

            // Compare in the same priority order as Freeciv's river tests.
            let score = |next: usize| {
                (
                    Self::mountainous(tiles[next].terrain),
                    std::cmp::Reverse(water_count(next)),
                    std::cmp::Reverse(river_count(next)),
                    self.neighbors(next)
                        .map(|tile| Self::mountainous(tiles[tile].terrain))
                        .sum::<i32>(),
                    std::cmp::Reverse(Self::wet(tiles[next].terrain)),
                    std::cmp::Reverse(
                        self.neighbors(next)
                            .map(|tile| Self::wet(tiles[tile].terrain))
                            .sum::<i32>(),
                    ),
                    heights[next],
                )
            };
            let mut candidates: Vec<_> = self
                .neighbors(current)
                .filter(|&next| {
                    !blocked[next]
                        && river_count(next) <= 1
                        && self.neighbors(next).any(|tile| !blocked[tile])
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
            for neighbor in self.neighbors(current) {
                blocked[neighbor] = true;
            }
            current = next;
        }
    }
}
