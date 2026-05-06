//! In-place mutations: `attach_obj`, `replace_node`,
//! `replace_attached_obj`, `retarget_edge`,
//! `add_hyperedge_members`, `remove_hyperedge_members`.

use std::collections::HashSet;

use uuid::Uuid;

use common::*;

use crate::errors::{
    AddHyperedgeMembersError, AttachObjectError, AttachTargetNotFoundError, EdgeNotFoundError,
    HyperEdgeNotFoundError, IncorrectTypeError, InvalidRetargetError, MembersAlreadyExistError,
    MembersNotInHyperedgeError, NoAttachedObjectError, NodeNotFoundError, PointeesNotFoundError,
    RemoveHyperedgeMembersError, RetargetError,
};
use crate::object_patch::diff_object;
use crate::types::EntityType;
use crate::graph::Graph;

impl Graph {
    pub(crate) fn silent_attach_obj(
        &mut self,
        target: AttachTargetID,
        obj: Object,
    ) -> Result<(), AttachObjectError> {
        let ty = match self.get_type(target) {
            Some(t) => t,
            None => {
                return Err(AttachObjectError::AttachTargetNotFound(
                    AttachTargetNotFoundError { id: target },
                ))
            }
        };
        if !ty.is_attach_target() {
            return Err(AttachObjectError::IncorrectType(IncorrectTypeError {
                entity_id: target,
                expected_type: vec![
                    EntityType::Edge.to_string(),
                    EntityType::HyperEdge.to_string(),
                    EntityType::MetaEdge.to_string(),
                ],
                actual_type: ty.to_string(),
            }));
        }

        self.entities.insert(target, obj);
        Ok(())
    }

    /// Attach an `Object` payload on top of an edge or hyperedge.
    /// Strict — fails with `IncorrectType` if the target already has
    /// an attached object (use `replace_attached_obj` for that).
    /// Records [`Patch::UpsertEdgeData`] or [`Patch::UpsertHyperEdgeData`].
    pub fn attach_obj(
        &mut self,
        target: AttachTargetID,
        obj: Object,
    ) -> Result<(), AttachObjectError> {
        // Resolve target type *before* mutating so we can pick the
        // right patch variant. After silent_attach_obj succeeds, the
        // type is guaranteed to be one of the attach-target kinds.
        let is_hyper = self.hyper_edge.contains_key(&target);

        self.silent_attach_obj(target, obj.clone())?;

        if is_hyper {
            self.emit_patch(Patch::UpsertHyperEdgeData { id: target, obj });
        } else {
            // MetaEdge is structurally an edge, so it lives in `self.edges`.
            self.emit_patch(Patch::UpsertEdgeData { id: target, obj });
        }
        Ok(())
    }

    pub(crate) fn silent_add_hyperedge_members(
        &mut self,
        id: HyperEdgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), AddHyperedgeMembersError> {
        if !self.hyper_edge.contains_key(&id) {
            return Err(AddHyperedgeMembersError::HyperEdgeNotFound(
                HyperEdgeNotFoundError { id },
            ));
        }

        let mut missing: HashSet<Pointee> = HashSet::new();
        for p in &m {
            if !self.is_pointee_exist(p) {
                missing.insert(p.clone());
            }
        }
        if !missing.is_empty() {
            return Err(AddHyperedgeMembersError::PointeesNotFound(
                PointeesNotFoundError { pointees: missing },
            ));
        }

        let existing = self.hyper_edge.get(&id).expect("checked above");
        let duplicates: Vec<Pointee> = m
            .iter()
            .filter(|p| existing.contains(*p))
            .cloned()
            .collect();
        if !duplicates.is_empty() {
            return Err(AddHyperedgeMembersError::MembersAlreadyExist(
                MembersAlreadyExistError {
                    members: duplicates,
                },
            ));
        }

        let members_set = self.hyper_edge.get_mut(&id).expect("checked above");
        for p in &m {
            members_set.insert(p.clone());
        }

        for p in &m {
            self.track_pointee_entity(p);
            self.pointee_uses
                .entry(p.clone())
                .or_default()
                .hyperedges
                .insert(id);
        }

        Ok(())
    }

    pub fn add_hyperedge_members(
        &mut self,
        id: HyperEdgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), AddHyperedgeMembersError> {
        self.silent_add_hyperedge_members(id, m.clone())?;
        self.emit_patch(Patch::AddElementsToHyperEdge { id, members: m });
        Ok(())
    }

    pub(crate) fn silent_remove_hyperedge_members(
        &mut self,
        id: HyperEdgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), RemoveHyperedgeMembersError> {
        let Some(current) = self.hyper_edge.get(&id) else {
            return Err(RemoveHyperedgeMembersError::HyperEdgeNotFound(
                HyperEdgeNotFoundError { id },
            ));
        };

        let not_present: Vec<Pointee> = m
            .iter()
            .filter(|p| !current.contains(*p))
            .cloned()
            .collect();
        if !not_present.is_empty() {
            return Err(RemoveHyperedgeMembersError::MembersNotInHyperedge(
                MembersNotInHyperedgeError {
                    members: not_present,
                },
            ));
        }

        let members_set = self.hyper_edge.get_mut(&id).expect("checked above");
        for p in &m {
            members_set.remove(p);
        }
        let now_empty = members_set.is_empty();

        // Strip `id` from each removed member's reverse-index bucket.
        for p in &m {
            if let Some(bucket) = self.pointee_uses.get_mut(p) {
                bucket.hyperedges.remove(&id);
                if bucket.is_empty() {
                    self.pointee_uses.remove(p);
                    self.untrack_pointee_entity(p);
                }
            }
        }

        // Invariant: hyperedges are never empty. If membership went to
        // zero, kill the hyperedge and cascade.
        if now_empty {
            self.hyper_edge.remove(&id);
            self.entities.remove(&id);
            self.cascade_remove_id(id);
        }

        Ok(())
    }

    pub fn remove_hyperedge_members(
        &mut self,
        id: HyperEdgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), RemoveHyperedgeMembersError> {
        self.silent_remove_hyperedge_members(id, m.clone())?;
        self.emit_patch(Patch::RemoveElementsFromHyperEdge { id, members: m });
        Ok(())
    }

    pub(crate) fn silent_replace_node(
        &mut self,
        id: &NodeId,
        obj: Object,
    ) -> Result<Object, NodeNotFoundError> {
        if !self.is_node(id) {
            return Err(NodeNotFoundError { id: *id });
        }
        // is_node checked the key exists, so insert returns Some(old).
        let old = self
            .entities
            .insert(*id, obj)
            .expect("is_node checked above");
        // Path-pointees through this node may no longer resolve
        // under the new object — cascade the dead ones.
        self.cascade_invalid_paths_through(*id);
        Ok(old)
    }

    /// Replace the object on a node. Records either `ChangeNode` (with
    /// a delta) or `UpsertNode` (full object), whichever is more
    /// compact: delta wins when the number of patch ops is `<=` the
    /// number of fields in the new object.
    pub fn replace_node(
        &mut self,
        id: &NodeId,
        obj: Object,
    ) -> Result<Field, NodeNotFoundError> {
        let new_field_count = obj.len();
        let old = self.silent_replace_node(id, obj.clone())?;

        let delta = diff_object(&old, &obj);
        if delta.len() <= new_field_count {
            self.emit_patch(Patch::ChangeNode { id: *id, delta });
        } else {
            self.emit_patch(Patch::UpsertNode { id: *id, obj });
        }

        Ok(Field::Object(old))
    }

    /// Set the attached object on `id`, regardless of whether one was
    /// already there. Used by `apply_patch` for `Upsert{Edge,HyperEdge}Data`
    /// patches — replay must work for both initial attach and replace.
    pub(crate) fn silent_upsert_attached_obj(
        &mut self,
        id: AttachTargetID,
        obj: Object,
    ) -> Result<(), AttachObjectError> {
        let ty = match self.get_type(id) {
            Some(t) => t,
            None => {
                return Err(AttachObjectError::AttachTargetNotFound(
                    AttachTargetNotFoundError { id },
                ))
            }
        };
        // Allow Edge/HyperEdge/MetaEdge (initial attach) AND
        // AttachedObject (already attached → upsert replaces).
        if !matches!(
            ty,
            EntityType::Edge
                | EntityType::HyperEdge
                | EntityType::MetaEdge
                | EntityType::AttachedObject
        ) {
            return Err(AttachObjectError::IncorrectType(IncorrectTypeError {
                entity_id: id,
                expected_type: vec![
                    EntityType::Edge.to_string(),
                    EntityType::HyperEdge.to_string(),
                    EntityType::MetaEdge.to_string(),
                ],
                actual_type: ty.to_string(),
            }));
        }
        self.entities.insert(id, obj);
        // Replace case: paths through this id may no longer resolve.
        // Initial-attach case: no paths existed yet (unattached id couldn't
        // produce a valid Pointee::Path at add time), so this is a no-op.
        self.cascade_invalid_paths_through(id);
        Ok(())
    }

    pub(crate) fn silent_replace_attached_obj(
        &mut self,
        id: &AttachTargetID,
        obj: Object,
    ) -> Result<Object, NoAttachedObjectError> {
        let is_attach_target = self.entities.contains_key(id)
            && (self.edges.contains_key(id) || self.hyper_edge.contains_key(id));
        if !is_attach_target {
            return Err(NoAttachedObjectError { id: *id });
        }
        let old = self
            .entities
            .insert(*id, obj)
            .expect("is_attach_target checked above");
        self.cascade_invalid_paths_through(*id);
        Ok(old)
    }

    /// Replace the attached object on an edge or hyperedge. Strict —
    /// fails if the target has no attached object yet (use
    /// `attach_obj` for that). Records `ChangeEdgeData`/`UpsertEdgeData`
    /// or the HyperEdge variants based on which compresses better:
    /// delta wins when its op count is `<=` the new object's field
    /// count.
    pub fn replace_attached_obj(
        &mut self,
        id: &AttachTargetID,
        obj: Object,
    ) -> Result<Field, NoAttachedObjectError> {
        let new_field_count = obj.len();
        let is_hyper = self.hyper_edge.contains_key(id);

        let old = self.silent_replace_attached_obj(id, obj.clone())?;
        let delta = diff_object(&old, &obj);
        let use_change = delta.len() <= new_field_count;

        match (is_hyper, use_change) {
            (true, true) => self.emit_patch(Patch::ChangeHyperEdgeData { id: *id, delta }),
            (true, false) => self.emit_patch(Patch::UpsertHyperEdgeData { id: *id, obj }),
            (false, true) => self.emit_patch(Patch::ChangeEdgeData { id: *id, delta }),
            (false, false) => self.emit_patch(Patch::UpsertEdgeData { id: *id, obj }),
        }

        Ok(Field::Object(old))
    }

    pub(crate) fn silent_retarget_edge(
        &mut self,
        id: &Uuid,
        new_target: RetargetEdge,
    ) -> Result<(), RetargetError> {
        let (old_source, old_target) = match self.edges.get(id) {
            Some((s, t)) => (s.clone(), t.clone()),
            None => return Err(RetargetError::EdgeNotFound(EdgeNotFoundError { id: *id })),
        };

        let new_pointee = match &new_target {
            RetargetEdge::Source(p) | RetargetEdge::Target(p) => p,
        };
        if !self.is_pointee_exist(new_pointee) {
            return Err(RetargetError::InvalidTarget(InvalidRetargetError {
                edge_id: *id,
                new_target,
            }));
        }

        // Identify which endpoint is being swapped.
        let is_source = matches!(new_target, RetargetEdge::Source(_));
        let (old_endpoint, new_endpoint) = if is_source {
            (old_source.clone(), new_pointee.clone())
        } else {
            (old_target.clone(), new_pointee.clone())
        };

        // No-op — same pointee.
        if old_endpoint == new_endpoint {
            return Ok(());
        }

        // Rewrite the edge.
        let new_pair = if is_source {
            (new_endpoint.clone(), old_target)
        } else {
            (old_source, new_endpoint.clone())
        };
        self.edges.insert(*id, new_pair);

        // Strip eid from the old endpoint's bucket (might empty it).
        if let Some(bucket) = self.pointee_uses.get_mut(&old_endpoint) {
            if is_source {
                bucket.edges_as_source.remove(id);
            } else {
                bucket.edges_as_target.remove(id);
            }
            if bucket.is_empty() {
                self.pointee_uses.remove(&old_endpoint);
                self.untrack_pointee_entity(&old_endpoint);
            }
        }

        // Register eid on the new endpoint's bucket.
        self.track_pointee_entity(&new_endpoint);
        let bucket = self.pointee_uses.entry(new_endpoint).or_default();
        if is_source {
            bucket.edges_as_source.insert(*id);
        } else {
            bucket.edges_as_target.insert(*id);
        }

        Ok(())
    }

    /// Retarget one endpoint of an existing edge. Records
    /// [`Patch::RetargetEdge`].
    pub fn retarget_edge(
        &mut self,
        id: &Uuid,
        new_target: RetargetEdge,
    ) -> Result<(), RetargetError> {
        self.silent_retarget_edge(id, new_target.clone())?;
        self.emit_patch(Patch::RetargetEdge {
            id: *id,
            new_target,
        });
        Ok(())
    }
}
