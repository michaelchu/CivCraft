use crate::sim::{PLAYER_COUNT, PLAYER_SLOTS, Resource, Terrain, Tile, topology::*};
use std::{collections::hash_map::RandomState, hash::BuildHasher};

#[cfg(test)]
const TILE_COUNT: usize = (MAP_W * MAP_H) as usize;
const INITIAL_DIVISIONS: i32 = 6;

pub(crate) struct GeneratedMap {
    pub(crate) seed: u64,
    pub(crate) map_wrap: MapWrap,
    pub(crate) game_random: crate::sim::FreecivRng,
    pub(crate) tiles: Vec<Tile>,
    pub(crate) starts: [(i32, i32); PLAYER_COUNT],
}

pub(crate) fn random_seed() -> u64 {
    RandomState::new().hash_one("CivCraft world")
}

// Freeciv's 56-word additive generator, with a separate state for map work.
struct MapRng {
    values: [u32; 56],
    next: usize,
}

impl MapRng {
    fn new(seed: u64) -> Self {
        let mut values = [0_u32; 56];
        values[0] = seed as u32 ^ (seed >> 32) as u32;
        for i in 1..values.len() {
            values[i] = values[i - 1].wrapping_mul(3).wrapping_add(257);
        }
        let mut rng = Self { values, next: 0 };
        for _ in 0..10_000 {
            rng.draw(u32::MAX);
        }
        rng
    }

    fn draw(&mut self, limit: u32) -> u32 {
        if limit <= 1 {
            return 0;
        }
        let divisor = u32::MAX / limit;
        loop {
            let value = self.values[self.next].wrapping_add(self.values[(self.next + 31) % 56]);
            self.values[self.next] = value;
            self.next = (self.next + 1) % 56;
            if value < limit * divisor {
                return value / divisor;
            }
        }
    }

    fn below(&mut self, limit: i32) -> i32 {
        self.draw(limit as u32) as i32
    }
}

impl Generator {
    fn index(&self, x: i32, y: i32) -> usize {
        let (x, y) = self.wrap.normalize(x, y).expect("generator tile is real");
        self.wrap.index(x, y)
    }
}

impl Generator {
    fn neighbors(&self, tile: usize) -> impl Iterator<Item = usize> {
        let (x, y) = self.wrap.position(tile);
        // Freeciv iterates the valid directions in enum order.
        [(-1, -1), (0, -1), (-1, 0), (1, 0), (0, 1), (1, 1)]
            .into_iter()
            .filter_map(move |(dx, dy)| {
                self.wrap
                    .offset(x, y, dx, dy)
                    .map(|(nx, ny)| self.index(nx, ny))
            })
    }

    fn within(&self, tile: usize, radius: i32) -> impl Iterator<Item = usize> {
        let (x, y) = self.wrap.position(tile);
        (-radius..=radius)
            .flat_map(move |dx| (-radius..=radius).map(move |dy| (dx, dy)))
            .filter(move |&(dx, dy)| isohex_vector_distance(dx, dy) <= radius)
            .filter_map(move |(dx, dy)| {
                self.wrap
                    .offset(x, y, dx, dy)
                    .map(|(nx, ny)| self.index(nx, ny))
            })
    }

    fn distance(&self, a: usize, b: usize) -> i32 {
        let (ax, ay) = self.wrap.position(a);
        let (bx, by) = self.wrap.position(b);
        let (dx, dy) = self.wrap.vector(ax, ay, bx, by);
        isohex_vector_distance(dx, dy)
    }
}

// Subdivide the initial wrapped blocks using Freeciv's edge and center
// midpoint scheme, reducing the perturbation to two thirds at each level.
impl Generator {
    fn subdivide(&self, heights: &mut [Option<i32>], rng: &mut MapRng, step: i32, rect: [i32; 4]) {
        let [left, top, right, bottom] = rect;
        if right == left || bottom == top || (right - left == 1 && bottom - top == 1) {
            return;
        }
        let mx = (left + right) / 2;
        let my = (top + bottom) / 2;
        let [nw, ne, sw, se] = [(left, top), (right, top), (left, bottom), (right, bottom)]
            .map(|(x, y)| heights[self.index(x, y)].unwrap_or(0));
        for (x, y, base) in [
            (mx, top, (nw + ne) / 2),
            (mx, bottom, (sw + se) / 2),
            (left, my, (nw + sw) / 2),
            (right, my, (ne + se) / 2),
            (mx, my, (nw + ne + sw + se) / 4),
        ] {
            let tile = self.index(x, y);
            let near_edge = !self.wrap.wraps_y() && (y < 4 || y >= self.wrap.height() - 4);
            if self.colatitude(tile) <= ICE_BASE / 2
                || (!near_edge && heights[tile].is_none_or(|height| height == 0))
            {
                heights[tile] = Some(base + rng.below(step) - step / 2);
            }
        }
        let step = (step * 2 / 3).max(1);
        for child in [
            [left, top, mx, my],
            [left, my, mx, bottom],
            [mx, top, right, my],
            [mx, my, right, bottom],
        ] {
            self.subdivide(heights, rng, step, child);
        }
    }

    fn height_field(&self, rng: &mut MapRng) -> Vec<i32> {
        let mut heights = vec![None; self.wrap.tile_count()];
        let step = self.wrap.width() + self.wrap.height();
        let ymax = if self.wrap.wraps_y() {
            self.wrap.height()
        } else {
            self.wrap.height() - 1
        };
        let grid = |size, axis| size * axis / INITIAL_DIVISIONS;
        let near_edge = |y: i32| !self.wrap.wraps_y() && (y < 4 || y >= self.wrap.height() - 4);
        let avoid_edge = (100 - LAND_PERCENT) * step / 100 + step / 3;
        for x in 0..INITIAL_DIVISIONS {
            for y in 0..INITIAL_DIVISIONS + i32::from(!self.wrap.wraps_y()) {
                let y = grid(ymax, y);
                heights[self.index(grid(self.wrap.width(), x), y)] =
                    Some(rng.below(2 * step) - step - if near_edge(y) { avoid_edge } else { 0 });
            }
        }
        for x in 0..INITIAL_DIVISIONS {
            for y in 0..INITIAL_DIVISIONS {
                self.subdivide(
                    &mut heights,
                    rng,
                    step,
                    [
                        grid(self.wrap.width(), x),
                        grid(ymax, y),
                        grid(self.wrap.width(), x + 1),
                        grid(ymax, y + 1),
                    ],
                );
            }
        }
        heights
            .into_iter()
            .map(|height| 8 * height.unwrap_or(0) + rng.below(4) - 2)
            .collect()
    }

    fn components(&self, terrain: &[Terrain], water: bool) -> Vec<Vec<usize>> {
        let mut visited = vec![false; self.wrap.tile_count()];
        let mut groups = Vec::new();
        for tile in 0..self.wrap.tile_count() {
            if visited[tile] || terrain[tile].is_water() != water {
                continue;
            }
            let mut group = vec![tile];
            visited[tile] = true;
            let mut cursor = 0;
            while cursor < group.len() {
                for next in self.neighbors(group[cursor]) {
                    if !visited[next] && terrain[next].is_water() == water {
                        visited[next] = true;
                        group.push(next);
                    }
                }
                cursor += 1;
            }
            groups.push(group);
        }
        groups
    }
}

// Freeciv scales elevation by cumulative frequency, keeping ties together.
fn rank_field(values: &mut [i32], maximum: i32) {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    for value in values {
        *value =
            sorted.partition_point(|other| other <= value) as i32 * maximum / sorted.len() as i32;
    }
}

const SHORE: i32 = 1000 * (100 - LAND_PERCENT) / 100;
const LOWLAND: i32 = SHORE + (1000 - SHORE) * 28 / 100;
const HIGHLAND: i32 = SHORE + (1000 - SHORE) * 70 / 100;
const ICE_BASE: i32 = 28;
const FROZEN: i32 = 2 * ICE_BASE;
const COLD: i32 = 85;
const TROPICAL: i32 = 572;

impl Generator {
    fn colatitude(&self, tile: usize) -> i32 {
        if !self.wrap.wraps_y() {
            let (_, y) = self.wrap.position(tile);
            let latitude =
                (1000.0 - 2000.0 * f64::from(y) / f64::from(self.wrap.height() - 1)) as i32;
            return 1000 - latitude.abs();
        }
        // Freeciv's torus projection folds two circular polar regions around
        // a diagonal equator in natural (staggered) coordinates.
        let (native_x, native_y) = self.wrap.position(tile);
        let x = f64::from(2 * native_x + native_y % 2) / f64::from(2 * self.wrap.width() - 1);
        let y = f64::from(native_y) / f64::from(self.wrap.height() - 1);
        let mut x = 1.0 - 2.0 * x.min(1.0 - x);
        let mut y = 2.0 * y.min(1.0 - y);
        let flipped = x + y > 1.0;
        if flipped {
            x = 1.0 - x;
            y = 1.0 - y;
        }
        let colatitude =
            1.5 * (x * x * y + x * y * y) - 0.5 * (x * x * x + y * y * y) + 1.5 * (x * x + y * y);
        let southness = if flipped {
            (2.0 - colatitude) / 2.0
        } else {
            colatitude / 2.0
        };
        let latitude = (1000.0 * (1.0 - southness) - 1000.0 * southness) as i32;
        1000 - latitude.abs()
    }
}

fn ocean_at_depth(depth: i32) -> Terrain {
    if (depth - 35).abs() <= (depth - 70).abs() {
        Terrain::Ocean
    } else {
        Terrain::DeepOcean
    }
}

fn random_tile(count: usize, rng: &mut MapRng, eligible: impl Fn(usize) -> bool) -> Option<usize> {
    for _ in 0..count / 10 {
        let tile = rng.below(count as i32) as usize;
        if eligible(tile) {
            return Some(tile);
        }
    }
    let candidates: Vec<_> = (0..count).filter(|&tile| eligible(tile)).collect();
    if candidates.is_empty() {
        None
    } else {
        Some(candidates[rng.below(candidates.len() as i32) as usize])
    }
}

impl Generator {
    fn grow_patch(
        &self,
        tile: usize,
        budget: i32,
        biome: Terrain,
        remaining: &mut usize,
        available: &mut [bool],
        terrain: &mut [Terrain],
        heights: &[i32],
        eligible: &impl Fn(usize) -> bool,
        rng: &mut MapRng,
    ) {
        if *remaining == 0 {
            return;
        }
        terrain[tile] = biome;
        available[tile] = false;
        *remaining -= 1;
        for next in self.neighbors(tile) {
            let delta = (self.colatitude(tile) - self.colatitude(next)).abs() / 66
                + (heights[tile] - heights[next]).abs();
            if available[next] && eligible(next) && delta < budget && rng.below(10) > 4 {
                self.grow_patch(
                    next,
                    budget - 1 - delta,
                    biome,
                    remaining,
                    available,
                    terrain,
                    heights,
                    eligible,
                    rng,
                );
            }
        }
    }

    fn polar_height_factor(&self, tile: usize) -> i32 {
        if (FROZEN..=ICE_BASE * 5 / 2).contains(&self.colatitude(tile)) {
            10
        } else {
            1
        }
    }

    fn restored_height(&self, heights: &[i32], tile: usize) -> i32 {
        let factor = self.polar_height_factor(tile);
        heights[tile] / factor * factor
    }

    fn terrain_pass(&self, heights: &[i32], rng: &mut MapRng) -> (Vec<Terrain>, Vec<i32>) {
        let mut dummy_temperature: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| self.colatitude(tile))
            .collect();
        rank_field(&mut dummy_temperature, 1000);
        let normalized_height = |tile: usize| {
            let (_, y) = self.wrap.position(tile);
            if !self.wrap.wraps_y()
                && (y < 4 || y >= self.wrap.height() - 4)
                && self.colatitude(tile) > ICE_BASE * 5 / 2
            {
                0
            } else {
                heights[tile] / self.polar_height_factor(tile)
            }
        };
        let mut terrain: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| {
                if normalized_height(tile) >= SHORE {
                    return Terrain::Plains;
                }
                let depth = (SHORE - normalized_height(tile)) * 100 / SHORE;
                // The reference still tests the frozen-ocean fringe even
                // when this ruleset falls back to unfrozen water.
                if (FROZEN..COLD).contains(&dummy_temperature[tile]) {
                    rng.below(10);
                }
                let mut water = 0;
                let mut land = 0;
                for next in self.neighbors(tile) {
                    if normalized_height(next) >= SHORE {
                        land = 1;
                        break;
                    }
                    water += 1;
                }
                ocean_at_depth((depth + 30 * (water - land) / (water + land)).min(100))
            })
            .collect();

        let restored: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| self.restored_height(heights, tile))
            .collect();
        let heights = &restored;
        let mut temperature: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| {
                let latitude = self.colatitude(tile);
                let coast = self
                    .neighbors(tile)
                    .filter(|&next| terrain[next].is_water())
                    .count() as i32
                    * 100
                    / 6;
                let height = (-0.3 * f64::from((heights[tile] - SHORE).max(0)) / 300.0) as f32;
                let maritime = (0.15 * f64::from((60 / 100 - latitude / 1000) * 2 * coast.min(50))
                    / 100.0) as f32;
                (f64::from(latitude) * (1.0 + f64::from(maritime)) * (1.0 + f64::from(height)))
                    as i32
            })
            .collect();
        rank_field(&mut temperature, 1000);

        // Civ2Civ3 has no frozen ocean terrain. Frozen coasts can become
        // glaciers, with a broken margin where cold tiles border frozen ones.
        let original_land: Vec<_> = terrain.iter().map(|tile| !tile.is_water()).collect();
        for tile in 0..self.wrap.tile_count() {
            if original_land[tile] {
                continue;
            }
            let frozen = temperature[tile] < FROZEN
                || (temperature[tile] < COLD
                    && rng.below(10) > 7
                    && self.neighbors(tile).any(|next| temperature[next] < FROZEN));
            if frozen && self.neighbors(tile).all(|next| !original_land[next]) {
                terrain[tile] = Terrain::Plains;
            }
        }

        let mut available = vec![false; self.wrap.tile_count()];
        for tile in 0..self.wrap.tile_count() {
            if terrain[tile] != Terrain::Plains {
                continue;
            }
            let higher = self
                .within(tile, 2)
                .filter(|&next| heights[next] > heights[tile])
                .count() as i32;
            let local_peak = self.within(tile, 2).all(|next| heights[next] <= HIGHLAND)
                && self
                    .neighbors(tile)
                    .all(|next| heights[next] <= heights[tile])
                && higher <= 2
                && (HIGHLAND - SHORE) * higher <= (heights[tile] - SHORE) * 4;
            let high_plateau = self
                .within(tile, 1)
                .all(|next| heights[next] + (1000 - HIGHLAND) / 5 >= HIGHLAND);
            if (heights[tile] > HIGHLAND && (rng.below(10) > 5 || !high_plateau)) || local_peak {
                let (hills_weight, mountain_weight) = if temperature[tile] >= COLD {
                    if rng.below(10) < 4 { (30, 70) } else { (30, 0) }
                } else if rng.below(10) < 8 {
                    (0, 70)
                } else {
                    (30, 70)
                };
                terrain[tile] = if rng.below(hills_weight + mountain_weight) < hills_weight {
                    Terrain::Hills
                } else {
                    Terrain::Mountains
                };
            } else {
                available[tile] = true;
            }
        }

        let terrain_water: Vec<_> = terrain.iter().map(|tile| tile.is_water()).collect();
        let remaining = available.iter().filter(|&&open| open).count();
        // Civ2Civ3 defaults: temperature 60, wetness 50, steepness 30.
        // Quotas are relative to the land still available after relief.
        let mut patches = [
            remaining * 16 / 80,
            remaining * 4 / 80,
            remaining * 7 / 80,
            remaining * 8 / 80,
            0,
        ];
        let mut plains = remaining - patches.iter().sum::<usize>();
        while plains > 0 || patches.iter().any(|&count| count > 0) {
            for pass in 0..patches.len() {
                if patches[pass] == 0 {
                    continue;
                }
                let eligible = |tile: usize| {
                    let heat = temperature[tile];
                    let dry = (623..=832).contains(&self.colatitude(tile))
                        && self
                            .neighbors(tile)
                            .filter(|&next| terrain_water[next])
                            .count()
                            <= 2;
                    match pass {
                        0 => heat >= FROZEN,
                        1 => heat >= TROPICAL,
                        2 => heat >= COLD && !dry && heights[tile] < LOWLAND,
                        3 => heat >= FROZEN && dry && heights[tile] >= LOWLAND,
                        _ => heat >= FROZEN && heights[tile] >= LOWLAND,
                    }
                };
                if let Some(seed) = random_tile(self.wrap.tile_count(), rng, |tile| {
                    available[tile] && eligible(tile)
                }) {
                    let (biome, weight, spread) = [
                        (Terrain::Forest, 50, 60),
                        (Terrain::Jungle, 50, 50),
                        (Terrain::Swamp, 100, 50),
                        (Terrain::Desert, 100, 80),
                        (Terrain::Desert, 100, 40),
                    ][pass];
                    rng.below(weight);
                    self.grow_patch(
                        seed,
                        spread,
                        biome,
                        &mut patches[pass],
                        &mut available,
                        &mut terrain,
                        heights,
                        &eligible,
                        rng,
                    );
                } else {
                    let leftover = std::mem::take(&mut patches[pass]);
                    match pass {
                        1 | 2 => patches[0] += leftover,
                        3 => patches[4] += leftover,
                        _ => plains += leftover,
                    }
                }
            }
            if plains > 0 {
                let tile = random_tile(self.wrap.tile_count(), rng, |tile| available[tile])
                    .expect("unplaced plains quota");
                terrain[tile] = if temperature[tile] < FROZEN {
                    rng.below(100);
                    Terrain::Glacier
                } else if temperature[tile] < COLD {
                    match rng.below(80) {
                        0..20 => Terrain::Forest,
                        20..30 => Terrain::Swamp,
                        _ => Terrain::Tundra,
                    }
                } else if rng.below(100) < 50 {
                    Terrain::Grassland
                } else {
                    Terrain::Plains
                };
                available[tile] = false;
                plains -= 1;
            }
        }
        (terrain, temperature)
    }

    fn finish_water(&self, terrain: &mut [Terrain], rng: &mut MapRng) {
        for tile in 0..terrain.len() {
            if !terrain[tile].is_water() {
                continue;
            }
            if let Some(distance) = (1..=4).find(|&radius| {
                self.within(tile, radius)
                    .any(|next| !terrain[next].is_water())
            }) {
                terrain[tile] = ocean_at_depth(distance * 25 + rng.below(15));
            }
        }
        for tile in 0..terrain.len() {
            if terrain[tile].is_water() {
                let shallow = self
                    .neighbors(tile)
                    .filter(|&next| terrain[next] == Terrain::Ocean)
                    .count();
                let deep = self
                    .neighbors(tile)
                    .filter(|&next| terrain[next] == Terrain::DeepOcean)
                    .count();
                let threshold = 2 * ISOHEX_DIRS.len() / 3;
                if shallow >= threshold {
                    terrain[tile] = Terrain::Ocean;
                } else if deep >= threshold {
                    terrain[tile] = Terrain::DeepOcean;
                }
            }
        }

        let mut landmass = vec![None; self.wrap.tile_count()];
        for (id, group) in self.components(terrain, false).into_iter().enumerate() {
            for tile in group {
                landmass[tile] = Some(id);
            }
        }
        for basin in self
            .components(terrain, true)
            .into_iter()
            .filter(|group| group.len() <= 14)
        {
            let mut shore = basin
                .iter()
                .flat_map(|&tile| self.neighbors(tile))
                .filter_map(|tile| landmass[tile]);
            if let Some(continent) = shore.next()
                && shore.all(|id| id == continent)
            {
                for tile in basin {
                    terrain[tile] = Terrain::Lake;
                }
            }
        }
    }
}

// Freeciv's Civ2Civ3 defaults produce a river percentage of six after
// the polar adjustment. The target counts river tiles, not river systems.
impl Generator {
    fn place_rivers(
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
                    && temperature[tile] >= FROZEN
                    && heights[tile] >= LOWLAND
            }) else {
                break;
            };
            if self
                .neighbors(spring)
                .all(|next| mountainous(tiles[next].terrain) > 0)
                && attempt < MAX_TRIES / 10 * 5
                || self
                    .neighbors(spring)
                    .filter(|&tile| tiles[tile].river || tiles[tile].terrain.is_water())
                    .count()
                    > 1
                || (mountainous(tiles[spring].terrain) > 0 && attempt < MAX_TRIES / 10 * 6)
                || (tiles[spring].terrain == Terrain::Glacier && attempt < MAX_TRIES / 10 * 8)
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
        Terrain::Swamp => 100,
        _ => 0,
    }
}

impl Generator {
    fn trace_river(
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
                || (tiles[current].terrain == Terrain::Glacier
                    && self.colatitude(current) * 5 < 4 * COLD)
            {
                return Some(course);
            }

            // Compare in the same priority order as Freeciv's river tests.
            let score = |next: usize| {
                (
                    mountainous(tiles[next].terrain),
                    std::cmp::Reverse(water_count(next)),
                    std::cmp::Reverse(river_count(next)),
                    self.neighbors(next)
                        .map(|tile| mountainous(tiles[tile].terrain))
                        .sum::<i32>(),
                    std::cmp::Reverse(wet(tiles[next].terrain)),
                    std::cmp::Reverse(
                        self.neighbors(next)
                            .map(|tile| wet(tiles[tile].terrain))
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

    fn place_resources(&self, tiles: &mut [Tile], rng: &mut MapRng) {
        use Resource::*;
        // Civ2Civ3 gives all listed resources equal weight, with no deep-ocean
        // resources and coastal fish and whales only near safe terrain.
        for tile in 0..tiles.len() {
            if self
                .neighbors(tile)
                .any(|next| tiles[next].resource.is_some())
                || rng.below(1000) >= 250
            {
                continue;
            }
            let choices: &[Resource] = match tiles[tile].terrain {
                Terrain::Lake => &[Fish],
                Terrain::Ocean
                    if self.neighbors(tile).any(|next| {
                        !matches!(tiles[next].terrain, Terrain::Ocean | Terrain::DeepOcean)
                    }) =>
                {
                    &[Fish, Whales]
                }
                Terrain::Ocean | Terrain::DeepOcean => &[],
                Terrain::Glacier => &[Ivory, Oil],
                Terrain::Desert => &[Oasis, Oil],
                Terrain::Forest => &[Pheasant, Silk],
                Terrain::Grassland => &[Resources],
                Terrain::Hills => &[Coal, Wine],
                Terrain::Jungle => &[Gems, Fruit],
                Terrain::Mountains => &[Gold, Iron],
                Terrain::Plains => &[Buffalo, Wheat],
                Terrain::Swamp => &[Peat, Spice],
                Terrain::Tundra => &[Game, Furs],
            };
            for (index, &resource) in choices.iter().enumerate() {
                if rng.below(index as i32 + 1) == 0 {
                    tiles[tile].resource = Some(resource);
                }
            }
        }
    }

    fn place_huts(&self, tiles: &mut [Tile], rng: &mut MapRng) {
        let target: usize = 15 * self.wrap.tile_count() / 1000;
        let mut reserved = vec![false; self.wrap.tile_count()];
        for _ in 0..target {
            let Some(pick) = random_tile(self.wrap.tile_count(), rng, |tile| !reserved[tile])
            else {
                break;
            };
            if !tiles[pick].terrain.is_water() && self.colatitude(pick) >= 20 {
                tiles[pick].hut = true;
            }
            for tile in self.within(pick, 3) {
                reserved[tile] = true;
            }
        }
    }

    fn start_positions(
        &self,
        terrain: &[Terrain],
        tiles: &[Tile],
        temperature: &[i32],
        rng: &mut MapRng,
    ) -> Option<[(i32, i32); PLAYER_COUNT]> {
        let yields: Vec<_> = tiles
            .iter()
            .map(|tile| {
                let (food, production, trade) = tile.yields();
                let road = tile.terrain.road_trade_bonus();
                let mine = if tile.terrain == Terrain::Glacier {
                    1
                } else {
                    tile.terrain.mine_bonus()
                };
                let improvement = tile.terrain.irrigation_bonus().max(mine);
                food + production + trade + (road + improvement) / 2
            })
            .collect();
        let peaks: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| {
                let mut better = 0;
                let mut worse = 0;
                for next in self.within(tile, 2) {
                    better += i32::from(yields[next] > yields[tile]);
                    worse += i32::from(yields[next] < yields[tile]);
                }
                if worse > better {
                    yields[tile] * 100
                } else {
                    0
                }
            })
            .collect();
        let smooth = |values: &[i32], horizontal: bool, weights: [f32; 5]| -> Vec<i32> {
            (0..self.wrap.tile_count())
                .map(|tile| {
                    let (x, y) = self.wrap.position(tile);
                    let mut total = 0.0;
                    for (delta, weight) in (-2..=2).zip(weights) {
                        let position = if horizontal {
                            self.wrap.normalize(x + delta, y)
                        } else {
                            self.wrap.normalize(x, y + delta)
                        };
                        if let Some((nx, ny)) = position {
                            total += weight * values[self.index(nx, ny)] as f32;
                        }
                    }
                    total as i32
                })
                .collect()
        };
        let horizontal = smooth(&peaks, true, [0.13, 0.19, 0.37, 0.19, 0.13]);
        let quality = smooth(&horizontal, false, [0.15, 0.21, 0.29, 0.21, 0.15]);
        let starter = |tile: usize| {
            matches!(
                terrain[tile],
                Terrain::Grassland | Terrain::Plains | Terrain::Forest | Terrain::Hills
            )
        };
        let groups = self.components(terrain, false);
        let mut landmass = vec![usize::MAX; self.wrap.tile_count()];
        let mut goodness = vec![0; groups.len()];
        for (id, group) in groups.iter().enumerate() {
            for &tile in group {
                landmass[tile] = id;
                if starter(tile) {
                    goodness[id] += quality[tile];
                }
            }
        }
        let mut ranked: Vec<_> = (0..groups.len()).collect();
        ranked.sort_by_key(|&id| std::cmp::Reverse(goodness[id]));
        let best = *ranked.first()?;
        let total = goodness.iter().sum::<i32>();
        let candidates: Vec<_> = (0..self.wrap.tile_count())
            .filter(|&tile| starter(tile))
            .collect();
        let mut scores: Vec<_> = candidates.iter().map(|&tile| quality[tile]).collect();
        rank_field(&mut scores, 1000);
        let mut score_by_tile = vec![0; self.wrap.tile_count()];
        for (&tile, score) in candidates.iter().zip(scores) {
            score_by_tile[tile] = score;
        }

        // startpos.c deliberately divides integers before assigning efactor.
        let efactor = (PLAYER_COUNT / (self.wrap.tile_count() / 1000).max(1) / 4) as f64;
        let single_share = (0.5 + 0.8 * efactor) / (1.0 + efactor);
        let variable_share = (0.65 + 0.8 * efactor) / (1.0 + efactor);
        let mut starts = Vec::with_capacity(PLAYER_COUNT);
        for single_continent in [true, false] {
            if single_continent
                && (goodness[best] < 1500 * PLAYER_COUNT as i32
                    || f64::from(goodness[best]) < f64::from(total) * single_share)
            {
                continue;
            }
            let mut allocated = vec![0; groups.len()];
            if single_continent {
                allocated[best] = PLAYER_COUNT;
            } else {
                let per_player = (f64::from(total) * variable_share / PLAYER_COUNT as f64) as i32;
                let mut slots = PLAYER_COUNT;
                for &id in &ranked {
                    if slots == 0 {
                        break;
                    }
                    allocated[id] = (goodness[id] / per_player.max(1)).max(1) as usize;
                    slots = slots.saturating_sub(allocated[id]);
                }
            }
            let mut remaining = allocated.clone();
            let mut threshold = 900;
            while threshold > 10 {
                let eligible = |tile: usize| {
                    if score_by_tile[tile] < threshold
                        || tiles[tile].hut
                        || temperature[tile] < COLD
                    {
                        return false;
                    }
                    let id = landmass[tile];
                    remaining[id] > 0
                        && starts.iter().all(|&other| {
                            let scaled = self.distance(tile, other) * 1000 / threshold;
                            scaled >= 5
                                && (landmass[other] != id
                                    || f64::from(scaled)
                                        > ((groups[id].len() / allocated[id]) as f64).sqrt())
                        })
                };
                if let Some(tile) = random_tile(self.wrap.tile_count(), rng, eligible) {
                    remaining[landmass[tile]] -= 1;
                    starts.push(tile);
                    if starts.len() == PLAYER_COUNT {
                        return starts
                            .into_iter()
                            .map(|tile| self.wrap.position(tile))
                            .collect::<Vec<_>>()
                            .try_into()
                            .ok();
                    }
                } else {
                    threshold = threshold * 95 / 100;
                }
            }
        }
        None
    }

    pub(crate) fn generate(&self, seed: u64) -> GeneratedMap {
        let mut rng = MapRng::new(seed);
        loop {
            let mut heights = self.height_field(&mut rng);
            rank_field(&mut heights, 1000);
            let (mut terrain, temperature) = self.terrain_pass(&heights, &mut rng);
            let mut tiles: Vec<_> = terrain
                .iter()
                .copied()
                .enumerate()
                .map(|(index, terrain)| Tile {
                    terrain,
                    altitude: heights[index],
                    hut: false,
                    river: false,
                    resource: None,
                    road: false,
                    railroad: false,
                    irrigation: false,
                    farmland: false,
                    pollution: false,
                    fallout: false,
                    base: None,
                    mine: false,
                    oil_well: false,
                    oil_platform: false,
                    owner: None,
                    border_city: None,
                    last_seen_owner: None,
                    seen: false,
                    visible: false,
                    stealth_visible: false,
                    known_by: [false; PLAYER_SLOTS],
                })
                .collect();
            let river_heights: Vec<_> = (0..self.wrap.tile_count())
                .map(|tile| self.restored_height(&heights, tile))
                .collect();
            self.place_rivers(&mut tiles, &river_heights, &temperature, &mut rng);
            self.finish_water(&mut terrain, &mut rng);
            for (tile, &terrain) in tiles.iter_mut().zip(&terrain) {
                tile.terrain = terrain;
            }
            self.place_resources(&mut tiles, &mut rng);
            self.place_huts(&mut tiles, &mut rng);
            let mut start_rng = MapRng::new(seed);
            // The default reference setup draws one nation and leader for each
            // of six AI slots, then consumes the map-seed draw before saving
            // the game RNG state used for start allocation.
            for _ in 0..2 * PLAYER_COUNT + 1 {
                start_rng.draw(u32::MAX);
            }
            if let Some(starts) =
                self.start_positions(&terrain, &tiles, &temperature, &mut start_rng)
            {
                let game_random =
                    crate::sim::FreecivRng::from_map_state(start_rng.values, start_rng.next)
                        .expect("map RNG cursor is valid");
                return GeneratedMap {
                    seed,
                    map_wrap: self.wrap,
                    game_random,
                    tiles,
                    starts,
                };
            }
            // Retry from the same RNG stream when a world has too few viable sites.
        }
    }
}

#[cfg(test)]
fn position(index: usize) -> (i32, i32) {
    (index as i32 % MAP_W, index as i32 / MAP_W)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_cursor_conversion_continues_the_same_native_stream() {
        for seed in [0, 42, u64::MAX] {
            let mut map = MapRng::new(seed);
            // Cross many ring boundaries and rejection-heavy ranges.
            for _ in 0..137 {
                map.draw(2_147_483_649);
            }
            let mut game = crate::sim::FreecivRng::from_map_state(map.values, map.next).unwrap();
            for limit in [0, 1, 2, 3, 100, 2_147_483_649, u32::MAX].repeat(23) {
                assert_eq!(game.draw(limit), map.draw(limit));
            }
        }
        assert!(crate::sim::FreecivRng::from_map_state([0; 56], 56).is_err());
    }

    #[test]
    fn generated_rivers_reach_water_or_the_poles_and_resources_are_spaced() {
        for seed in [0, 42, 12345] {
            let map = generate(seed);
            let tiles = &map.tiles;
            assert!(tiles.iter().any(|tile| tile.river));
            assert!(tiles.iter().any(|tile| tile.resource.is_some()));
            assert!(tiles.iter().any(|tile| tile.hut));
            assert!(
                map.starts
                    .iter()
                    .all(|&(x, y)| !tiles[(Generator { wrap: MapWrap::XY }).index(x, y)].hut)
            );

            let mut visited = vec![false; TILE_COUNT];
            for (index, tile) in tiles.iter().enumerate() {
                assert!((0..=1000).contains(&tile.altitude));
                if tile.hut {
                    assert!(!tile.terrain.is_water());
                    assert!(
                        (Generator { wrap: MapWrap::XY })
                            .within(index, 3)
                            .all(|next| next == index || !tiles[next].hut)
                    );
                }
                if tile.resource.is_some() {
                    assert!(
                        (Generator { wrap: MapWrap::XY })
                            .neighbors(index)
                            .all(|next| tiles[next].resource.is_none())
                    );
                    assert_ne!(tile.terrain, Terrain::DeepOcean);
                }
                if !tile.river || visited[index] {
                    continue;
                }
                let mut course = vec![index];
                visited[index] = true;
                let mut cursor = 0;
                let mut has_outlet = false;
                while cursor < course.len() {
                    let current = course[cursor];
                    assert!(!tiles[current].terrain.is_water());
                    has_outlet |= tiles[current].terrain == Terrain::Glacier;
                    for next in (Generator { wrap: MapWrap::XY }).neighbors(current) {
                        has_outlet |= tiles[next].terrain.is_water();
                        if tiles[next].river && !visited[next] {
                            visited[next] = true;
                            course.push(next);
                        }
                    }
                    cursor += 1;
                }
                assert!(has_outlet, "river at {index} in seed {seed} has no outlet");
            }
        }
    }
}

#[cfg(test)]
#[test]
fn default_worlds_match_reference() {
    // Freeciv eb8c7033, default Civ2Civ3 six-player setup with mapseed and
    // gameseed both set to the seed below. Checksums cover each native
    // tile's saved terrain identifier, altitude, and complete extras mask.
    // The coordinates are in creation order, before the save table's reordering.
    let cases = [
        (
            1,
            0x1ed9cb2004a3fa85,
            [(27, 7), (0, 12), (23, 25), (28, 23), (5, 3), (20, 2)],
        ),
        (
            42,
            0x075636906e371f20,
            [(9, 9), (5, 43), (27, 21), (8, 25), (14, 18), (1, 24)],
        ),
        (
            12345,
            0xe4beee0725f81cd8,
            [(11, 25), (23, 11), (18, 5), (15, 20), (29, 23), (4, 16)],
        ),
        (
            700001,
            0x4c4726d779e9aedf,
            [(2, 10), (23, 3), (29, 1), (20, 41), (19, 3), (15, 55)],
        ),
        (
            2147483647,
            0x8f5e4f7938dafbaf,
            [(3, 16), (12, 57), (26, 32), (25, 57), (10, 32), (18, 40)],
        ),
    ];
    for (seed, expected, starts) in cases {
        let map = generate(seed);
        let mut checksum = 0xcbf29ce484222325_u64;
        for tile in map.tiles {
            let terrain = match tile.terrain {
                Terrain::Ocean => b' ',
                Terrain::DeepOcean => b':',
                Terrain::Lake => b'+',
                Terrain::Glacier => b'a',
                Terrain::Desert => b'd',
                Terrain::Forest => b'f',
                Terrain::Grassland => b'g',
                Terrain::Hills => b'h',
                Terrain::Jungle => b'j',
                Terrain::Mountains => b'm',
                Terrain::Plains => b'p',
                Terrain::Swamp => b's',
                Terrain::Tundra => b't',
            };
            let mut extras = (u64::from(tile.hut) << 5) | (u64::from(tile.river) << 17);
            if let Some(resource) = tile.resource {
                extras |= 1 << (18 + resource as u32);
            }
            for value in [u64::from(terrain), tile.altitude as u64, extras] {
                checksum = (checksum ^ value).wrapping_mul(0x100000001b3);
            }
        }
        assert_eq!(checksum, expected, "terrain and extras for seed {seed}");
        assert_eq!(map.starts, starts, "starting positions for seed {seed}");
    }
}

struct Generator {
    wrap: MapWrap,
}
/// Historical reference preset, retained for existing oracle fixtures.
pub(crate) fn generate(seed: u64) -> GeneratedMap {
    generate_with_wrap(seed, MapWrap::XY)
}
pub(crate) fn generate_with_wrap(seed: u64, wrap: MapWrap) -> GeneratedMap {
    Generator { wrap }.generate(seed)
}

#[cfg(test)]
mod wrap_generation_tests {
    use super::*;
    #[test]
    fn cylinders_have_real_poles_and_reproducible_post_generation_starts() {
        for seed in [0, 42, 1337] {
            let a = generate_with_wrap(seed, MapWrap::X);
            let b = generate_with_wrap(seed, MapWrap::X);
            assert_eq!(a.map_wrap, MapWrap::X);
            assert_eq!(a.starts, b.starts);
            assert_eq!(
                bincode::serialize(&a.tiles).unwrap(),
                bincode::serialize(&b.tiles).unwrap()
            );
            assert!(a.tiles.iter().any(|t| t.terrain == Terrain::Glacier));
            assert!(
                a.starts
                    .iter()
                    .all(|&(x, y)| !a.tiles[a.map_wrap.index(x, y)].terrain.is_water())
            );
            let g = Generator { wrap: MapWrap::X };
            assert_eq!(g.colatitude(0), 0);
            assert_eq!(g.colatitude(g.wrap.tile_count() - 1), 0);
            assert!(g.colatitude(g.index(10, g.wrap.height() / 2)) > 950);
            assert!(
                g.neighbors(g.index(10, 0))
                    .all(|i| g.wrap.position(i).1 < g.wrap.height() / 2)
            );
            assert_ne!(
                bincode::serialize(&a.tiles).unwrap(),
                bincode::serialize(&generate(seed).tiles).unwrap()
            );
        }
    }
}

#[cfg(test)]
#[test]
fn generated_worlds_match_native_for_each_wrap() {
    for line in include_str!("../tests/fixtures/freeciv_mapgen_vectors.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<_> = line.split('\t').collect();
        let wrap: MapWrap = fields[0].parse().unwrap();
        let seed: u64 = fields[1].parse().unwrap();
        let width: i32 = fields[2].parse().unwrap();
        let height: i32 = fields[3].parse().unwrap();
        assert_eq!((wrap.width(), wrap.height()), (width, height));
        let map = generate_with_wrap(seed, wrap);
        let mut checksum = 0xcbf29ce484222325u64;
        for tile in &map.tiles {
            let id = match tile.terrain {
                Terrain::Ocean => b' ',
                Terrain::DeepOcean => b':',
                Terrain::Lake => b'+',
                Terrain::Glacier => b'a',
                Terrain::Desert => b'd',
                Terrain::Forest => b'f',
                Terrain::Grassland => b'g',
                Terrain::Hills => b'h',
                Terrain::Jungle => b'j',
                Terrain::Mountains => b'm',
                Terrain::Plains => b'p',
                Terrain::Swamp => b's',
                Terrain::Tundra => b't',
            };
            let mut extras = (u64::from(tile.hut) << 5) | (u64::from(tile.river) << 17);
            if let Some(resource) = tile.resource {
                extras |= 1 << (18 + resource as u32);
            }
            for value in [u64::from(id), tile.altitude as u64, extras] {
                checksum = (checksum ^ value).wrapping_mul(0x100000001b3);
            }
        }
        assert_eq!(
            checksum,
            u64::from_str_radix(fields[4], 16).unwrap(),
            "terrain/altitude/extras for {wrap} seed {seed}"
        );
        let mut starts = map.starts.to_vec();
        starts.sort_unstable();
        let expected: Vec<(i32, i32)> = fields[5]
            .split(';')
            .map(|pair| {
                let (x, y) = pair.split_once(',').unwrap();
                (x.parse().unwrap(), y.parse().unwrap())
            })
            .collect();
        assert_eq!(
            starts, expected,
            "starting locations for {wrap} seed {seed}"
        );
    }
}
