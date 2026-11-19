//! Transient refresh scheduling, separate from durable player observations.

#[derive(Clone, Default)]
pub(in crate::sim) struct VisibilityRefresh {
    deferred: bool,
    pending: bool,
    render_tiles: Vec<bool>,
    render_scope: Option<Vec<usize>>,
}

/// Restore the caller's deferral state, including an enclosing AI path batch.
#[must_use = "restore the previous visibility batch with end_batch"]
#[derive(Clone, Copy)]
pub(in crate::sim) struct VisibilityBatch {
    previously_deferred: bool,
}

impl VisibilityRefresh {
    pub(in crate::sim) fn deferred(&self) -> bool {
        self.deferred
    }

    pub(in crate::sim) fn begin_batch(&mut self, defer: bool) -> VisibilityBatch {
        let batch = VisibilityBatch {
            previously_deferred: self.deferred,
        };
        self.deferred |= defer;
        batch
    }

    /// Only the outermost restored batch may request a flush from its caller.
    pub(in crate::sim) fn end_batch(&mut self, batch: VisibilityBatch) -> bool {
        self.deferred = batch.previously_deferred;
        self.ready_to_flush()
    }

    pub(in crate::sim) fn request_refresh(&mut self) {
        self.pending = true;
    }

    fn ready_to_flush(&self) -> bool {
        !self.deferred && self.pending
    }

    pub(super) fn prepare_render_tiles(&mut self, tile_count: usize) {
        if self.render_tiles.len() != tile_count {
            self.render_tiles = vec![false; tile_count];
        }
    }

    pub(super) fn mark_render_tile(&mut self, index: usize) {
        self.render_tiles[index] = true;
    }

    pub(super) fn start_flush(&mut self) -> bool {
        if !self.ready_to_flush() {
            return false;
        }
        self.pending = false;
        self.render_scope = Some(
            self.render_tiles
                .iter_mut()
                .enumerate()
                .filter_map(|(index, dirty)| {
                    if *dirty {
                        *dirty = false;
                        Some(index)
                    } else {
                        None
                    }
                })
                .collect(),
        );
        true
    }

    pub(super) fn take_render_scope(&mut self) -> Option<Vec<usize>> {
        self.render_scope.take()
    }

    pub(super) fn finish_flush(&mut self) {
        self.render_scope = None;
    }
}

#[cfg(test)]
mod tests;
