//! Technology graph layout and connection routing for the research screen.

use crate::rules::{Tech, TechEra};
use macroquad::prelude::*;

pub(crate) const NODE_SIZE: Vec2 = vec2(205.0, 56.0);
const COLUMN_STEP: f32 = 285.0;

pub(crate) const RESEARCH_AGES: [(&str, TechEra); 4] = [
    ("Ancient Times", TechEra::Ancient),
    ("Middle Ages", TechEra::MiddleAges),
    ("Industrial Age", TechEra::Industrial),
    ("Modern Times", TechEra::Modern),
];

pub(crate) struct TreeEdge {
    pub(crate) from: Tech,
    pub(crate) to: Tech,
    pub(crate) points: Vec<Vec2>,
}

pub(crate) struct ResearchTree {
    pub(crate) nodes: Vec<(Tech, Vec2)>,
    pub(crate) edges: Vec<TreeEdge>,
    pub(crate) size: Vec2,
}

impl ResearchTree {
    pub(crate) fn new() -> Self {
        let advances: Vec<_> = Tech::all()
            .filter(|tech| tech.is_civ3_advance())
            .chain([Tech::FUTURE])
            .collect();
        let index = |tech| advances.iter().position(|&entry| entry == tech).unwrap();
        let mut links = Vec::new();
        for &tech in &advances {
            links.extend(tech.prerequisites().iter().map(|&req| (req, tech)));
        }
        // Future Tech requires the whole catalog. Connecting the terminal
        // advances expresses that requirement without duplicating their links.
        for tech in Tech::all().filter(|tech| tech.is_civ3_advance()) {
            if !links.iter().any(|&(from, _)| from == tech) {
                links.push((tech, Tech::FUTURE));
            }
        }

        let mut depths = vec![0; advances.len()];
        for _ in 0..advances.len() {
            let mut changed = false;
            for &(from, to) in &links {
                let next = depths[index(from)] + 1;
                let depth = &mut depths[index(to)];
                if *depth < next {
                    *depth = next;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        let mut columns = vec![Vec::new(); depths.iter().max().unwrap() + 1];
        for (node, &depth) in depths.iter().enumerate() {
            columns[depth].push(node);
        }

        // Give long links a routing slot in each intermediate column so they
        // can cross that column without passing through a technology card.
        let mut vertex_count = advances.len();
        let routes: Vec<_> = links
            .iter()
            .map(|&(from, to)| {
                let from = index(from);
                let to = index(to);
                let mut route = vec![from];
                for column in &mut columns[depths[from] + 1..depths[to]] {
                    let waypoint = vertex_count;
                    column.push(waypoint);
                    vertex_count += 1;
                    route.push(waypoint);
                }
                route.push(to);
                route
            })
            .collect();

        let mut gaps = vec![Vec::new(); columns.len() - 1];
        for ((from, _), route) in links.iter().zip(&routes) {
            for (offset, pair) in route.windows(2).enumerate() {
                gaps[depths[index(*from)] + offset].push((pair[0], pair[1]));
            }
        }
        order_columns(&mut columns, &gaps, vertex_count);

        let slot_height = |node| {
            if node < advances.len() {
                NODE_SIZE.y + 20.0
            } else {
                12.0
            }
        };
        let heights: Vec<f32> = columns
            .iter()
            .map(|col| col.iter().map(|&n| slot_height(n)).sum())
            .collect();
        let height = heights.iter().copied().fold(0.0, f32::max);
        let mut centers = vec![Vec2::ZERO; vertex_count];
        for (depth, column) in columns.iter().enumerate() {
            let mut y = (height - heights[depth]) * 0.5;
            for &node in column.iter() {
                let slot = slot_height(node);
                centers[node] = vec2(
                    depth as f32 * COLUMN_STEP + NODE_SIZE.x * 0.5,
                    y + slot * 0.5,
                );
                y += slot;
            }
        }
        let original_centers = centers.clone();
        let channels: Vec<_> = gaps
            .iter()
            .enumerate()
            .map(|(depth, gap)| route_channel(depth, gap, &original_centers, advances.len()))
            .collect();

        let mut era_ranges = [(f32::INFINITY, f32::NEG_INFINITY); 4];
        for (index, &tech) in advances.iter().enumerate() {
            let range = &mut era_ranges[tech.era().index()];
            range.0 = range.0.min(centers[index].x);
            range.1 = range.1.max(centers[index].x);
        }
        let mut era_offsets = [0.0; 4];
        let mut era_right = 0.0;
        for (era, &(left, right)) in era_ranges.iter().enumerate() {
            era_offsets[era] = era_right - left;
            era_right += (right - left).max(COLUMN_STEP) + COLUMN_STEP;
        }
        for (index, &tech) in advances.iter().enumerate() {
            centers[index].x += era_offsets[tech.era().index()];
        }
        centers[index(Tech::FUTURE)].x = era_right - COLUMN_STEP * 0.5;
        centers[index(Tech::FUTURE)].y = height * 0.5;

        let nodes = advances
            .iter()
            .enumerate()
            .map(|(i, &tech)| (tech, centers[i]))
            .collect();
        let edges = links
            .into_iter()
            .zip(routes)
            .map(|((from, to), route)| {
                if from.era() != to.era() {
                    return TreeEdge {
                        from,
                        to,
                        points: vec![centers[index(from)], centers[index(to)]],
                    };
                }
                let mut points = Vec::new();
                for (offset, pair) in route.windows(2).enumerate() {
                    let channel = &channels[depths[index(from)] + offset];
                    let segment = channel
                        .iter()
                        .find(|(key, _)| *key == (pair[0], pair[1]))
                        .unwrap()
                        .1;
                    points.extend(segment);
                }
                let offset = era_offsets[from.era().index()];
                for point in &mut points {
                    point.x += offset;
                }
                TreeEdge { from, to, points }
            })
            .collect();
        let size = vec2(era_right - COLUMN_STEP * 0.5, height);
        Self { nodes, edges, size }
    }

    pub(crate) fn center(&self, tech: Tech) -> Vec2 {
        self.nodes.iter().find(|(node, _)| *node == tech).unwrap().1
    }

    pub(crate) fn age(&self, tech: Tech) -> usize {
        tech.era().index()
    }

    pub(crate) fn bounds(&self, age: usize) -> Rect {
        let era = TechEra::ALL[age.min(TechEra::ALL.len() - 1)];
        let (left, right) = self
            .nodes
            .iter()
            .filter(|(tech, _)| tech.era() == era)
            .map(|(_, center)| (center.x - NODE_SIZE.x * 0.5, center.x + NODE_SIZE.x * 0.5))
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(left, right), bounds| {
                (left.min(bounds.0), right.max(bounds.1))
            });
        Rect::new(left, 0.0, right - left, self.size.y)
    }

    pub(crate) fn ancestors(&self, goal: Tech) -> Vec<Tech> {
        if goal == Tech::UNSET {
            return Vec::new();
        }
        let mut found = vec![goal];
        let mut cursor = 0;
        while cursor < found.len() {
            let node = found[cursor];
            for edge in self.edges.iter().filter(|edge| edge.to == node) {
                if !found.contains(&edge.from) {
                    found.push(edge.from);
                }
            }
            cursor += 1;
        }
        found
    }
}

// Keep the best ordering seen during alternating sweeps, then try adjacent
// swaps. Counting inversions across each gap penalizes crossing links.
fn order_columns(columns: &mut [Vec<usize>], gaps: &[Vec<(usize, usize)>], count: usize) {
    let mut rank = vec![0; count];
    let update = |column: &[usize], rank: &mut [usize]| {
        for (row, &node) in column.iter().enumerate() {
            rank[node] = row;
        }
    };
    for column in columns.iter() {
        update(column, &mut rank);
    }
    let crossings = |gap: &[(usize, usize)], rank: &[usize]| {
        gap.iter()
            .enumerate()
            .map(|(i, &(a, b))| {
                gap[i + 1..]
                    .iter()
                    .filter(|&&(c, d)| {
                        (rank[a] < rank[c] && rank[b] > rank[d])
                            || (rank[a] > rank[c] && rank[b] < rank[d])
                    })
                    .count()
            })
            .sum::<usize>()
    };
    let score = |rank: &[usize]| gaps.iter().map(|gap| crossings(gap, rank)).sum::<usize>();
    let mut best = columns.to_vec();
    let mut best_score = score(&rank);
    for sweep in 0..12 {
        let forward = sweep % 2 == 0;
        for step in 1..columns.len() {
            let col = if forward {
                step
            } else {
                columns.len() - 1 - step
            };
            let gap = &gaps[if forward { col - 1 } else { col }];
            columns[col].sort_by_cached_key(|&node| {
                let (sum, count) = gap
                    .iter()
                    .filter_map(|&(from, to)| {
                        if forward && to == node {
                            Some(rank[from])
                        } else if !forward && from == node {
                            Some(rank[to])
                        } else {
                            None
                        }
                    })
                    .fold((0, 0), |(sum, count), row| (sum + row, count + 1));
                if count == 0 {
                    rank[node] * 1024
                } else {
                    sum * 1024 / count
                }
            });
            update(&columns[col], &mut rank);
        }
        let current = score(&rank);
        if current < best_score {
            best_score = current;
            best.clone_from_slice(columns);
        }
    }
    columns.clone_from_slice(&best);
    for column in columns.iter() {
        update(column, &mut rank);
    }
    for _ in 0..8 {
        let mut improved = false;
        for (col, column) in columns.iter_mut().enumerate() {
            let local_score = |rank: &[usize]| {
                gaps.get(col.wrapping_sub(1))
                    .map_or(0, |gap| crossings(gap, rank))
                    + gaps.get(col).map_or(0, |gap| crossings(gap, rank))
            };
            for row in 1..column.len() {
                let before = local_score(&rank);
                let (a, b) = (column[row - 1], column[row]);
                rank.swap(a, b);
                if local_score(&rank) < before {
                    column.swap(row - 1, row);
                    improved = true;
                } else {
                    rank.swap(a, b);
                }
            }
        }
        if !improved {
            break;
        }
    }
}

fn route_channel(
    depth: usize,
    gap: &[(usize, usize)],
    centers: &[Vec2],
    advance_count: usize,
) -> Vec<((usize, usize), [Vec2; 4])> {
    // Give each link its own port on a shared card and its own bend lane.
    let port = |node: usize, other: usize, outgoing: bool| {
        if node >= advance_count {
            return centers[node].y;
        }
        let mut neighbors: Vec<_> = gap
            .iter()
            .filter_map(|&(from, to)| {
                if outgoing && from == node {
                    Some(to)
                } else if !outgoing && to == node {
                    Some(from)
                } else {
                    None
                }
            })
            .collect();
        neighbors.sort_by(|&a, &b| centers[a].y.total_cmp(&centers[b].y));
        let position = neighbors.iter().position(|&n| n == other).unwrap() as f32;
        let span = (neighbors.len() - 1) as f32;
        centers[node].y + (position - span / 2.0) * (32.0 / span.max(1.0)).min(9.0)
    };
    let mut bends: Vec<_> = gap
        .iter()
        .map(|&(from, to)| ((from, to), port(from, to, true), port(to, from, false)))
        .collect();
    bends.sort_by(|a, b| a.1.total_cmp(&b.1));
    let between = |y: f32, a: f32, b: f32| y > a.min(b) && y < a.max(b);
    let crossings = |a: &(_, f32, f32), b: &(_, f32, f32)| {
        usize::from(between(b.1, a.1, a.2)) + usize::from(between(a.2, b.1, b.2))
    };
    for _ in 0..bends.len() {
        let mut changed = false;
        for i in 1..bends.len() {
            let (a, b) = (&bends[i - 1], &bends[i]);
            if crossings(b, a) < crossings(a, b) {
                bends.swap(i - 1, i);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let left = depth as f32 * COLUMN_STEP + NODE_SIZE.x;
    let right = (depth + 1) as f32 * COLUMN_STEP;
    let spacing = (right - left) / (bends.len() + 1) as f32;
    bends
        .into_iter()
        .enumerate()
        .map(|(i, (key, y1, y2))| {
            let bend = left + (i + 1) as f32 * spacing;
            (
                key,
                [
                    vec2(left, y1),
                    vec2(bend, y1),
                    vec2(bend, y2),
                    vec2(right, y2),
                ],
            )
        })
        .collect()
}

pub(super) fn shared_tree() -> &'static ResearchTree {
    static TREE: std::sync::OnceLock<ResearchTree> = std::sync::OnceLock::new();
    TREE.get_or_init(ResearchTree::new)
}
