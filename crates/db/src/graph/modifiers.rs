//! In-place mutations: `attach_obj`, `replace_node`,
//! `replace_attached_obj`, `retarget_edge`,
//! `add_hyperedge_members`, `remove_hyperedge_members`.

use std::collections::HashSet;

use uuid::Uuid;

use common::*;

use crate::errors::{
    AddHyperedgeMembersError, AttachObjectError, AttachTargetNotFoundError, EdgeNotFoundError,
    HyperedgeNotFoundError, IncorrectTypeError, InvalidRetargetError, MembersAlreadyExistError,
    MembersNotInHyperedgeError, NoAttachedObjectError, NodeNotFoundError, PointeesNotFoundError,
    RemoveHyperedgeMembersError, RetargetError,
};
use crate::graph::AttachKind;
use crate::graph::Graph;
use crate::object_patch::diff_object;
use crate::types::EntityType;

impl Graph {
    pub(crate) fn silent_attach_obj(
        &mut self,
        target: AttachTargetId,
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
                    EntityType::Hyperedge.to_string(),
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
    /// Records [`Patch::UpsertEdgeData`] or [`Patch::UpsertHyperedgeData`].
    pub fn attach_obj(
        &mut self,
        target: AttachTargetId,
        obj: Object,
    ) -> Result<(), AttachObjectError> {
        // Resolve target type *before* mutating so we can pick the
        // right patch variant. After silent_attach_obj succeeds, the
        // type is guaranteed to be one of the attach-target kinds.
        let is_hyper = self.hyperedges.contains_key(&target);

        self.silent_attach_obj(target, obj.clone())?;

        if is_hyper {
            self.record_patch(Patch::UpsertHyperedgeData { id: target, obj });
        } else {
            // MetaEdge is structurally an edge, so it lives in `self.edges`.
            self.record_patch(Patch::UpsertEdgeData { id: target, obj });
        }
        Ok(())
    }

    pub(crate) fn silent_add_hyperedge_members(
        &mut self,
        id: HyperedgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), AddHyperedgeMembersError> {
        if !self.hyperedges.contains_key(&id) {
            return Err(AddHyperedgeMembersError::HyperedgeNotFound(
                HyperedgeNotFoundError { id },
            ));
        }

        let missing = self.collect_missing_pointees(&m);
        if !missing.is_empty() {
            return Err(AddHyperedgeMembersError::PointeesNotFound(
                PointeesNotFoundError { pointees: missing },
            ));
        }

        let existing = self.hyperedges.get(&id).expect("checked above");
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

        let members_set = self.hyperedges.get_mut(&id).expect("checked above");
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
        id: HyperedgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), AddHyperedgeMembersError> {
        self.silent_add_hyperedge_members(id, m.clone())?;
        self.record_patch(Patch::AddHyperedgeMembers { id, members: m });
        Ok(())
    }

    pub(crate) fn silent_remove_hyperedge_members(
        &mut self,
        id: HyperedgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), RemoveHyperedgeMembersError> {
        let Some(current) = self.hyperedges.get(&id) else {
            return Err(RemoveHyperedgeMembersError::HyperedgeNotFound(
                HyperedgeNotFoundError { id },
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

        let members_set = self.hyperedges.get_mut(&id).expect("checked above");
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
            self.hyperedges.remove(&id);
            self.entities.remove(&id);
            self.cascade_remove_entity(id);
        }

        Ok(())
    }

    pub fn remove_hyperedge_members(
        &mut self,
        id: HyperedgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), RemoveHyperedgeMembersError> {
        self.silent_remove_hyperedge_members(id, m.clone())?;
        self.record_patch(Patch::RemoveHyperedgeMembers { id, members: m });
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
        self.invalidate_dead_paths_through(*id);
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
            self.record_patch(Patch::ChangeNode { id: *id, delta });
        } else {
            self.record_patch(Patch::UpsertNode { id: *id, obj });
        }

        Ok(Field::Object(old))
    }

    /// Set the attached object on `id`, regardless of whether one was
    /// already there. Used by `apply_patch` for `Upsert{Edge,HyperEdge}Data`
    /// patches — replay must work for both initial attach and replace.
    pub(crate) fn silent_upsert_attached_obj(
        &mut self,
        id: AttachTargetId,
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
                | EntityType::Hyperedge
                | EntityType::MetaEdge
                | EntityType::AttachedObject
        ) {
            return Err(AttachObjectError::IncorrectType(IncorrectTypeError {
                entity_id: id,
                expected_type: vec![
                    EntityType::Edge.to_string(),
                    EntityType::Hyperedge.to_string(),
                    EntityType::MetaEdge.to_string(),
                ],
                actual_type: ty.to_string(),
            }));
        }
        self.entities.insert(id, obj);
        // Replace case: paths through this id may no longer resolve.
        // Initial-attach case: no paths existed yet (unattached id couldn't
        // produce a valid Pointee::Path at add time), so this is a no-op.
        self.invalidate_dead_paths_through(id);
        Ok(())
    }

    /// Unified set-or-remove of the attached object on an edge or
    /// hyperedge. `Some(obj)` replaces / sets the payload, `None`
    /// removes it. Returns the previous attached object.
    ///
    /// Note the asymmetric cascade:
    /// - `None` (remove) — every `Pointee::Path` through `id` becomes
    ///   invalid (no object to navigate). All paths are killed via
    ///   [`invalidate_all_paths_through`].
    /// - `Some(obj)` (replace) — only paths that no longer resolve
    ///   under the new object are killed; surviving fields keep
    ///   their references via [`invalidate_dead_paths_through`].
    pub(crate) fn silent_set_attached_obj(
        &mut self,
        id: AttachTargetId,
        new: Option<Object>,
    ) -> Result<Option<Object>, NoAttachedObjectError> {
        if !self.has_attached_object(&id) {
            return Err(NoAttachedObjectError { id });
        }
        let old = match new {
            Some(obj) => {
                let prev = self.entities.insert(id, obj);
                self.invalidate_dead_paths_through(id);
                prev
            }
            None => {
                let prev = self.entities.remove(&id);
                self.invalidate_all_paths_through(id);
                prev
            }
        };
        Ok(old)
    }

    pub(crate) fn silent_replace_attached_obj(
        &mut self,
        id: &AttachTargetId,
        obj: Object,
    ) -> Result<Object, NoAttachedObjectError> {
        // The unified op guarantees `Some(old)` because
        // `has_attached_object` was true at the start.
        Ok(self
            .silent_set_attached_obj(*id, Some(obj))?
            .expect("has_attached_object guaranteed an old object"))
    }

    /// Replace the attached object on an edge or hyperedge. Strict —
    /// fails if the target has no attached object yet (use
    /// `attach_obj` for that). Records `ChangeEdgeData`/`UpsertEdgeData`
    /// or the HyperEdge variants based on which compresses better:
    /// delta wins when its op count is `<=` the new object's field
    /// count.
    pub fn replace_attached_obj(
        &mut self,
        id: &AttachTargetId,
        obj: Object,
    ) -> Result<Field, NoAttachedObjectError> {
        let new_field_count = obj.len();
        let kind = self.attach_kind(id);

        let old = self.silent_replace_attached_obj(id, obj.clone())?;
        let delta = diff_object(&old, &obj);
        let use_change = delta.len() <= new_field_count;

        match (kind, use_change) {
            (Some(AttachKind::Hyperedge), true) => {
                self.record_patch(Patch::ChangeHyperedgeData { id: *id, delta })
            }
            (Some(AttachKind::Hyperedge), false) => {
                self.record_patch(Patch::UpsertHyperedgeData { id: *id, obj })
            }
            (Some(AttachKind::Edge), true) => {
                self.record_patch(Patch::ChangeEdgeData { id: *id, delta })
            }
            (Some(AttachKind::Edge), false) => {
                self.record_patch(Patch::UpsertEdgeData { id: *id, obj })
            }
            // Unreachable — `silent_replace_attached_obj` would have
            // failed otherwise.
            (None, _) => {}
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
        self.record_patch(Patch::RetargetEdge {
            id: *id,
            new_target,
        });
        Ok(())
    }
}
