//! Map path search and movement-cost evaluation.

/// Reconstruct a forward path from predecessor links, returning `None` when a
/// reachable destination has a broken link before the start node.
pub(super) fn reconstruct_path(
    previous: &[Option<(usize, (i32, i32))>],
    start: usize,
    cursor: usize,
) -> Option<Vec<(i32, i32)>> {
    let (root, path) = reconstruct_path_to_root(previous, cursor)?;
    (root == start).then_some(path)
}

/// Reconstruct a forward path by following predecessor links to its root.
pub(super) fn reconstruct_path_to_root(
    previous: &[Option<(usize, (i32, i32))>],
    mut cursor: usize,
) -> Option<(usize, Vec<(i32, i32)>)> {
    let mut path = Vec::new();
    while let Some((parent, step)) = previous.get(cursor).copied().flatten() {
        path.push(step);
        cursor = parent;
    }
    path.reverse();
    Some((cursor, path))
}

/// Movement points available for the step that begins after `spent` points of
/// a route, for a unit that starts with `initial` points and regains
/// `allowance` each turn. A step never costs more than this, which is how a
/// unit with partial movement still enters an expensive tile.
pub(super) fn moves_left_after(spent: i32, initial: i32, allowance: i32) -> i32 {
    if spent < initial {
        initial - spent
    } else {
        allowance - (spent - initial.max(0)) % allowance
    }
}

/// The open set and best-known costs of a shortest-path search over map tiles.
///
/// Every tile search in the simulation shares this bookkeeping; each supplies
/// its own neighbors, step costs and stopping rule. Tiles are settled in
/// ascending `(cost, tie, tile, tag)` order, so a search that needs a specific
/// order among equal costs states it through `K` (for example an insertion
/// sequence), and a multi-source search carries its source in `X`. That order
/// is observable in AI decisions and must not change.
pub(super) struct Frontier<C = i32, K = (), X = ()> {
    best: Vec<C>,
    queue: std::collections::BinaryHeap<std::cmp::Reverse<(C, K, usize, X)>>,
}

impl<C: Ord + Copy, K: Ord + Copy, X: Ord + Copy> Frontier<C, K, X> {
    /// An empty frontier in which every tile costs `unreached`.
    pub(super) fn new(tile_count: usize, unreached: C) -> Self {
        Self {
            best: vec![unreached; tile_count],
            queue: std::collections::BinaryHeap::new(),
        }
    }

    /// Records `cost` as the best for `tile` and queues it, superseding any
    /// entry already queued for that tile.
    pub(super) fn push(&mut self, tile: usize, cost: C, tie: K, tag: X) {
        self.best[tile] = cost;
        self.queue.push(std::cmp::Reverse((cost, tie, tile, tag)));
    }

    /// Whether `cost` beats the best known cost of `tile`.
    pub(super) fn improves(&self, tile: usize, cost: C) -> bool {
        cost < self.best[tile]
    }

    /// The next tile to settle with its cost and tag, skipping entries that a
    /// cheaper route superseded.
    pub(super) fn pop(&mut self) -> Option<(C, usize, X)> {
        while let Some(std::cmp::Reverse((cost, _, tile, tag))) = self.queue.pop() {
            if self.best[tile] == cost {
                return Some((cost, tile, tag));
            }
        }
        None
    }

    /// The best known cost of every tile.
    pub(super) fn into_costs(self) -> Vec<C> {
        self.best
    }
}

impl<C: Ord + Copy> Frontier<C> {
    /// A frontier seeded with `start` at `cost`.
    pub(super) fn from_start(tile_count: usize, unreached: C, start: usize, cost: C) -> Self {
        let mut frontier = Self::new(tile_count, unreached);
        frontier.push(start, cost, (), ());
        frontier
    }

    /// Queues `tile` at `cost` if that beats its best known cost.
    pub(super) fn relax(&mut self, tile: usize, cost: C) -> bool {
        let improves = self.improves(tile, cost);
        if improves {
            self.push(tile, cost, (), ());
        }
        improves
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_left_after_wraps_each_turn_from_the_initial_allowance() {
        assert_eq!(moves_left_after(0, 2, 6), 2);
        assert_eq!(moves_left_after(1, 2, 6), 1);
        assert_eq!(moves_left_after(2, 2, 6), 6);
        assert_eq!(moves_left_after(7, 2, 6), 1);
        // A unit already out of moves starts its first step next turn.
        assert_eq!(moves_left_after(0, 0, 6), 6);
        assert_eq!(moves_left_after(0, -3, 6), 6);
        // A full allowance is the plain per-turn cycle.
        assert_eq!(moves_left_after(13, 6, 6), 6 - 13 % 6);
    }

    #[test]
    fn frontier_settles_in_cost_then_tile_order_and_skips_superseded_entries() {
        let mut frontier = Frontier::from_start(8, i32::MAX, 3, 0);
        assert!(frontier.relax(5, 4));
        assert!(frontier.relax(1, 4));
        assert!(frontier.relax(5, 2));
        assert!(!frontier.relax(5, 2));
        let settled: Vec<_> = std::iter::from_fn(|| frontier.pop())
            .map(|(cost, tile, ())| (cost, tile))
            .collect();
        assert_eq!(settled, [(0, 3), (2, 5), (4, 1)]);
        assert_eq!(frontier.into_costs()[5], 2);
    }

    #[test]
    fn frontier_tie_key_orders_equal_costs_before_the_tile_index() {
        let mut frontier: Frontier<i32, u32> = Frontier::new(8, i32::MAX);
        frontier.push(6, 1, 0, ());
        frontier.push(2, 1, 1, ());
        let settled: Vec<_> = std::iter::from_fn(|| frontier.pop())
            .map(|(_, tile, ())| tile)
            .collect();
        assert_eq!(settled, [6, 2]);
    }
}
