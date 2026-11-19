//! Shared expansion of a diplomatic transition across master/vassal groups.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct PactChange {
    pub(super) first: usize,
    pub(super) second: usize,
    pub(super) pact: Pact,
    pub(super) until: i32,
}
pub(super) type PactChanges = Vec<PactChange>;

impl Simulation {
    pub(super) fn diplomatic_group(&self, owner: usize) -> Vec<usize> {
        let master = self.vassal_master(owner).unwrap_or(owner);
        std::iter::once(master)
            .chain(self.vassalage.agreements.iter().filter_map(|(&vassal, a)| {
                (a.master == master && self.player_alive(vassal)).then_some(vassal)
            }))
            .collect()
    }

    pub(super) fn pact_transition_allowed(&self, actor: usize, other: usize) -> bool {
        actor != other
            && actor < self.players.len()
            && other < self.players.len()
            && self.vassal_master(actor).is_none()
            && !self.same_vassal_group(actor, other)
    }

    pub(super) fn plan_group_pact(
        &self,
        a: usize,
        b: usize,
        pact: Pact,
        until: i32,
    ) -> PactChanges {
        let mut changes = Vec::new();
        for first in self.diplomatic_group(a) {
            for second in self.diplomatic_group(b) {
                if first != second
                    && (self.contacted_pact(first, second) != Some(pact)
                        || self
                            .diplomacy
                            .get(&Self::relation_key(first, second))
                            .is_some_and(|r| r.until != until))
                {
                    let (first, second) = Self::relation_key(first, second);
                    changes.push(PactChange {
                        first,
                        second,
                        pact,
                        until,
                    });
                }
            }
        }
        changes.sort_by_key(|c| (c.first, c.second));
        changes.dedup_by_key(|c| (c.first, c.second));
        changes
    }

    pub(super) fn commit_pact_changes(&mut self, changes: PactChanges) {
        if changes.is_empty() {
            return;
        }
        // Every final pact and expiry is computed before mutation. A raw write
        // cannot recursively propagate or consult an individual subject's Senate.
        let split: Vec<_> = changes
            .iter()
            .filter(|c| self.pact(c.first, c.second) == Pact::Alliance && c.pact != Pact::Alliance)
            .map(|c| (c.first, c.second))
            .collect();
        for change in &changes {
            self.set_pact_state(change.first, change.second, change.pact, change.until);
        }
        for (a, b) in split {
            self.resolve_split_stacks(a, b);
        }
        for change in changes {
            if change.pact == Pact::War {
                self.break_mutual_alliances_after_war(change.first, change.second);
            }
        }
        self.refresh_visibility();
    }
}
