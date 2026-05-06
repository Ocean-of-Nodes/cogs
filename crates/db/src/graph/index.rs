//! Reverse-index data structure ([`PointeeUses`]) and the helpers
//! that keep `entity_to_path_pointees` in sync with `pointee_uses`.

use std::collections::HashSet;

use common::*;

use crate::graph::Graph;

/// Reverse-index bucket: which edges/hyperedges currently point at
/// a given [`Pointee`]. Maintained by every mutating op so cascade
/// removal can find dangling references in O(in-degree).
#[derive(Default)]
pub(crate) struct PointeeUses {
    /// Edges whose `source` is this pointee.
    pub(crate) edges_as_source: HashSet<EdgeId>,
    /// Edges whose `target` is this pointee.
    pub(crate) edges_as_target: HashSet<EdgeId>,
    /// Hyperedges that include this pointee as a member.
    pub(crate) hyperedges: HashSet<HyperedgeId>,
}

impl PointeeUses {
    /// True when no structural element references this pointee. The
    /// bucket is dropped from the index whenever this is true.
    pub(crate) fn is_empty(&self) -> bool {
        self.edges_as_source.is_empty()
            && self.edges_as_target.is_empty()
            && self.hyperedges.is_empty()
    }
}

impl Graph {
    /// Register a `Pointee::Path` in the secondary
    /// `entity_to_path_pointees` index. No-op for `Pointee::EntityId`.
    pub(crate) fn track_pointee_entity(&mut self, p: &Pointee) {
        if let Pointee::Path(gp) = p {
            self.entity_to_path_pointees
                .entry(gp.entity())
                .or_default()
                .insert(p.clone());
        }
    }

    /// Drop a `Pointee::Path` from the secondary index; remove the
    /// entity's entry entirely if it's now empty. No-op for
    /// `Pointee::EntityId`.
    pub(crate) fn untrack_pointee_entity(&mut self, p: &Pointee) {
        if let Pointee::Path(gp) = p {
            if let Some(set) = self.entity_to_path_pointees.get_mut(&gp.entity()) {
                set.remove(p);
                if set.is_empty() {
                    self.entity_to_path_pointees.remove(&gp.entity());
                }
            }
        }
    }
}
