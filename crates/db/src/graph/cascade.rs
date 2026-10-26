//! Recursive removal helpers — when something gets deleted, walk
//! the reverse index and clean up everything that pointed at it.

use std::collections::HashSet;

use common::*;

use crate::graph::Graph;

impl Graph {
    /// Strip an edge's id from both its endpoints' reverse-index
    /// buckets. If `skip` is set, the matching endpoint is left
    /// untouched (callers that already removed/will remove the
    /// `skip` bucket themselves use this).
    ///
    /// When a bucket becomes empty after the removal, it is dropped
    /// from `pointee_uses` and untracked from `entity_to_path_pointees`.
    pub(crate) fn unregister_edge_from_endpoints(
        &mut self,
        eid: &EdgeId,
        source: &Pointee,
        target: &Pointee,
        skip: Option<&Pointee>,
    ) {
        for endpoint in [source, target] {
            if Some(endpoint) == skip {
                continue;
            }
            if let Some(bucket) = self.pointee_uses.get_mut(endpoint) {
                bucket.edges_as_source.remove(eid);
                bucket.edges_as_target.remove(eid);
                if bucket.is_empty() {
                    self.pointee_uses.remove(endpoint);
                    self.untrack_pointee_entity(endpoint);
                }
            }
        }
    }

    /// Drains a single pointee bucket: removes referencing edges,
    /// strips the pointee from hyperedge memberships (killing
    /// hyperedges that empty out). Every change is appended to
    /// `cascade`. New dangling structural ids are pushed onto
    /// `worklist` for the caller to process.
    pub(crate) fn drain_pointee_bucket(
        &mut self,
        pointee: &Pointee,
        worklist: &mut Vec<EntityId>,
        cascade: &mut FullDelta,
    ) {
        let Some(uses) = self.pointee_uses.remove(pointee) else {
            return;
        };

        let dead_edges: HashSet<EdgeId> = uses
            .edges_as_source
            .iter()
            .chain(uses.edges_as_target.iter())
            .copied()
            .collect();
        for eid in dead_edges {
            if let Some((source, target)) = self.edges.remove(&eid) {
                // The drained `pointee` bucket is already gone — skip it.
                self.unregister_edge_from_endpoints(&eid, &source, &target, Some(pointee));
                let data = self.entities.remove(&eid);
                cascade.push(FullPatch::RemoveEdge {
                    id: eid,
                    source,
                    target,
                    data,
                });
                worklist.push(eid);
            }
        }

        for hid in uses.hyperedges {
            if let Some(members) = self.hyperedges.get_mut(&hid) {
                members.remove(pointee);
                let removed = HashSet::from([pointee.clone()]);
                if members.is_empty() {
                    // Record the whole hyperedge (with its last member)
                    // instead of an emptying `RemoveHyperedgeMembers`.
                    self.hyperedges.remove(&hid);
                    let data = self.entities.remove(&hid);
                    cascade.push(FullPatch::RemoveHyperedge {
                        id: hid,
                        members: removed,
                        data,
                    });
                    worklist.push(hid);
                } else {
                    cascade.push(FullPatch::RemoveHyperedgeMembers {
                        id: hid,
                        members: removed,
                    });
                }
            }
        }
    }

    /// For one fully-dead entity, drain every pointee that becomes
    /// invalid: `Pointee::EntityId(dead_id)` and every `Pointee::Path`
    /// through it (looked up via `entity_to_path_pointees`).
    pub(crate) fn drain_dead_entity(
        &mut self,
        dead_id: EntityId,
        worklist: &mut Vec<EntityId>,
        cascade: &mut FullDelta,
    ) {
        let mut affected: Vec<Pointee> = vec![Pointee::EntityId(dead_id)];
        if let Some(paths) = self.entity_to_path_pointees.remove(&dead_id) {
            affected.extend(paths);
        }
        for pointee in affected {
            self.drain_pointee_bucket(&pointee, worklist, cascade);
        }
    }

    /// Drain every entity on `worklist` (and everything that dies
    /// as a result) into `cascade`.
    fn drain_worklist(&mut self, mut worklist: Vec<EntityId>, cascade: &mut FullDelta) {
        while let Some(dead_id) = worklist.pop() {
            self.drain_dead_entity(dead_id, &mut worklist, cascade);
        }
    }

    /// Cascade entry point: the given entity has been removed. Walk
    /// every dangling reference and clean it up (recursively, since
    /// removed edges/hyperedges can themselves be referenced).
    /// Returns everything the cascade removed.
    pub(crate) fn cascade_remove_entity(&mut self, removed: EntityId) -> FullDelta {
        let mut cascade = FullDelta::new();
        self.drain_worklist(vec![removed], &mut cascade);
        cascade
    }

    /// Cascade-clean every reference that depended on `entity` having
    /// an attached object — i.e., every `Pointee::Path` rooted at this
    /// entity. The entity itself (edge or hyperedge) stays alive, so
    /// `Pointee::EntityId(entity)` references are preserved. Used by
    /// `remove_attached`. Returns everything the cascade removed.
    pub(crate) fn invalidate_all_paths_through(&mut self, entity: EntityId) -> FullDelta {
        let mut cascade = FullDelta::new();
        let Some(paths) = self.entity_to_path_pointees.remove(&entity) else {
            return cascade;
        };
        let mut worklist: Vec<EntityId> = Vec::new();
        for pointee in paths {
            self.drain_pointee_bucket(&pointee, &mut worklist, &mut cascade);
        }
        self.drain_worklist(worklist, &mut cascade);
        cascade
    }

    /// After mutating `entity`'s object, drop every `Pointee::Path`
    /// through it that no longer resolves under the new shape. The
    /// entity itself stays alive, so `Pointee::EntityId(entity)`
    /// references are preserved. Used by `replace_node` (and any
    /// future field-mutating op). Returns everything the cascade
    /// removed.
    pub(crate) fn invalidate_dead_paths_through(&mut self, entity: EntityId) -> FullDelta {
        let mut cascade = FullDelta::new();
        let candidates: Vec<Pointee> = match self.entity_to_path_pointees.get(&entity) {
            Some(set) => set.iter().cloned().collect(),
            None => return cascade,
        };
        let dead: Vec<Pointee> = candidates
            .into_iter()
            .filter(|p| !self.is_pointee_exist(p))
            .collect();
        let mut worklist: Vec<EntityId> = Vec::new();
        for p in &dead {
            self.untrack_pointee_entity(p);
            self.drain_pointee_bucket(p, &mut worklist, &mut cascade);
        }
        self.drain_worklist(worklist, &mut cascade);
        cascade
    }
}
