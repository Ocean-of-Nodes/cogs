//! Recursive removal helpers — when something gets deleted, walk
//! the reverse index and clean up everything that pointed at it.

use std::collections::HashSet;

use common::*;

use crate::graph::Graph;

impl Graph {
    /// Drains a single pointee bucket: removes referencing edges,
    /// strips the pointee from hyperedge memberships (killing
    /// hyperedges that empty out). New dangling structural ids are
    /// pushed onto `worklist` for the caller to process.
    pub(crate) fn drain_pointee_bucket(
        &mut self,
        pointee: &Pointee,
        worklist: &mut Vec<EntityId>,
    ) {
        let Some(uses) = self.pointee_uses.remove(pointee) else {
            return;
        };

        let dead_edges: HashSet<EdgeID> = uses
            .edges_as_source
            .iter()
            .chain(uses.edges_as_target.iter())
            .copied()
            .collect();
        for eid in dead_edges {
            if let Some((src, tgt)) = self.edges.remove(&eid) {
                for endpoint in [&src, &tgt] {
                    if endpoint == pointee {
                        continue;
                    }
                    if let Some(bucket) = self.pointee_uses.get_mut(endpoint) {
                        bucket.edges_as_source.remove(&eid);
                        bucket.edges_as_target.remove(&eid);
                        if bucket.is_empty() {
                            self.pointee_uses.remove(endpoint);
                            self.untrack_pointee_entity(endpoint);
                        }
                    }
                }
                self.entities.remove(&eid);
                worklist.push(eid);
            }
        }

        for hid in uses.hyperedges {
            if let Some(members) = self.hyper_edge.get_mut(&hid) {
                members.remove(pointee);
                if members.is_empty() {
                    self.hyper_edge.remove(&hid);
                    self.entities.remove(&hid);
                    worklist.push(hid);
                }
            }
        }
    }

    /// For one fully-dead entity, drain every pointee that becomes
    /// invalid: `Pointee::EntityId(dead_id)` and every `Pointee::Path`
    /// through it (looked up via `entity_to_path_pointees`).
    pub(crate) fn cascade_drain_id(&mut self, dead_id: EntityId, worklist: &mut Vec<EntityId>) {
        let mut affected: Vec<Pointee> = vec![Pointee::EntityId(dead_id)];
        if let Some(paths) = self.entity_to_path_pointees.remove(&dead_id) {
            affected.extend(paths);
        }
        for pointee in affected {
            self.drain_pointee_bucket(&pointee, worklist);
        }
    }

    /// Cascade entry point: the given entity has been removed. Walk
    /// every dangling reference and clean it up (recursively, since
    /// removed edges/hyperedges can themselves be referenced).
    pub(crate) fn cascade_remove_id(&mut self, removed: EntityId) {
        let mut worklist: Vec<EntityId> = vec![removed];
        while let Some(dead_id) = worklist.pop() {
            self.cascade_drain_id(dead_id, &mut worklist);
        }
    }

    /// Cascade-clean every reference that depended on `entity` having
    /// an attached object — i.e., every `Pointee::Path` rooted at this
    /// entity. The entity itself (edge or hyperedge) stays alive, so
    /// `Pointee::EntityId(entity)` references are preserved. Used by
    /// `remove_attached`.
    pub(crate) fn cascade_path_references_through(&mut self, entity: EntityId) {
        let Some(paths) = self.entity_to_path_pointees.remove(&entity) else {
            return;
        };
        let mut worklist: Vec<EntityId> = Vec::new();
        for pointee in paths {
            self.drain_pointee_bucket(&pointee, &mut worklist);
        }
        while let Some(dead_id) = worklist.pop() {
            self.cascade_drain_id(dead_id, &mut worklist);
        }
    }

    /// After mutating `entity`'s object, drop every `Pointee::Path`
    /// through it that no longer resolves under the new shape. The
    /// entity itself stays alive, so `Pointee::EntityId(entity)`
    /// references are preserved. Used by `replace_node` (and any
    /// future field-mutating op).
    pub(crate) fn cascade_invalid_paths_through(&mut self, entity: EntityId) {
        let candidates: Vec<Pointee> = match self.entity_to_path_pointees.get(&entity) {
            Some(set) => set.iter().cloned().collect(),
            None => return,
        };
        let dead: Vec<Pointee> = candidates
            .into_iter()
            .filter(|p| !self.is_pointee_exist(p))
            .collect();
        if dead.is_empty() {
            return;
        }
        let mut worklist: Vec<EntityId> = Vec::new();
        for p in &dead {
            self.untrack_pointee_entity(p);
            self.drain_pointee_bucket(p, &mut worklist);
        }
        while let Some(dead_id) = worklist.pop() {
            self.cascade_drain_id(dead_id, &mut worklist);
        }
    }
}
