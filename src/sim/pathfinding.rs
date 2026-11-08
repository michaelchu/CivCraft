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
