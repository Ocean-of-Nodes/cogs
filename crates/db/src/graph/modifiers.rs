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
    /// Records [`FullPatch::UpsertEdgeData`] or [`FullPatch::UpsertHyperedgeData`].
    pub fn attach_obj(
        &mut self,
        target: AttachTargetId,
        obj: Object,
    ) -> Result<(), AttachObjectError> {
        if !self.writable(&target) {
            return Err(AttachObjectError::AttachTargetNotFound(
                AttachTargetNotFoundError { id: target },
            ));
        }
        // Resolve target type *before* mutating so we can pick the
        // right patch variant. After silent_attach_obj succeeds, the
        // type is guaranteed to be one of the attach-target kinds.
        let is_hyper = self.hyperedges.contains_key(&target);

        self.silent_attach_obj(target, obj.clone())?;

        // Strict attach: there was no attached object before.
        if is_hyper {
            self.record(
                FullPatch::UpsertHyperedgeData {
                    id: target,
                    obj,
                    old: None,
                },
                FullDelta::new(),
            );
        } else {
            // MetaEdge is structurally an edge, so it lives in `self.edges`.
            self.record(
                FullPatch::UpsertEdgeData {
                    id: target,
                    obj,
                    old: None,
                },
                FullDelta::new(),
            );
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
        if !self.writable(&id) {
            return Err(AddHyperedgeMembersError::HyperedgeNotFound(
                HyperedgeNotFoundError { id },
            ));
        }
        let pointees = self.unwritable_pointees(&m);
        if self.hyperedges.contains_key(&id) && !pointees.is_empty() {
            return Err(AddHyperedgeMembersError::PointeesNotFound(
                PointeesNotFoundError { pointees },
            ));
        }
        self.silent_add_hyperedge_members(id, m.clone())?;
        self.record(FullPatch::AddHyperedgeMembers { id, members: m }, FullDelta::new());
        Ok(())
    }

    /// Returns what the cascade removed (the hyperedge itself, if
    /// it became empty, and everything that referenced it).
    pub(crate) fn silent_remove_hyperedge_members(
        &mut self,
        id: HyperedgeId,
        m: HashSet<Pointee>,
    ) -> Result<FullDelta, RemoveHyperedgeMembersError> {
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
        let mut cascade = FullDelta::new();
        if now_empty {
            self.hyperedges.remove(&id);
            let data = self.entities.remove(&id);
            cascade.push(FullPatch::RemoveHyperedge {
                id,
                members: HashSet::new(),
                data,
            });
            cascade.extend(self.cascade_remove_entity(id));
        }

        Ok(cascade)
    }

    pub fn remove_hyperedge_members(
        &mut self,
        id: HyperedgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), RemoveHyperedgeMembersError> {
        if !self.writable(&id) {
            return Err(RemoveHyperedgeMembersError::HyperedgeNotFound(
                HyperedgeNotFoundError { id },
            ));
        }
        let cascade = self.silent_remove_hyperedge_members(id, m.clone())?;
        self.record(FullPatch::RemoveHyperedgeMembers { id, members: m }, cascade);
        Ok(())
    }

    pub(crate) fn silent_replace_node(
        &mut self,
        id: &NodeId,
        obj: Object,
    ) -> Result<(Object, FullDelta), NodeNotFoundError> {
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
        let cascade = self.invalidate_dead_paths_through(*id);
        Ok((old, cascade))
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
        if !self.writable(id) {
            return Err(NodeNotFoundError { id: *id });
        }
        let new_field_count = obj.len();
        let (old, cascade) = self.silent_replace_node(id, obj.clone())?;

        let delta = diff_object(&old, &obj);
        let operation = if delta.len() <= new_field_count {
            FullPatch::ChangeNode {
                id: *id,
                delta,
                old: old.clone(),
            }
        } else {
            FullPatch::UpsertNode {
                id: *id,
                obj,
                old: old.clone(),
            }
        };
        self.record(operation, cascade);

        Ok(Field::Object(old))
    }

    /// Set the attached object on `id`, regardless of whether one was
    /// already there. Used by `apply_patch` for `Upsert{Edge,HyperEdge}Data`
    /// patches — replay must work for both initial attach and replace.
    pub(crate) fn silent_upsert_attached_obj(
        &mut self,
        id: AttachTargetId,
        obj: Object,
    ) -> Result<FullDelta, AttachObjectError> {
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
        Ok(self.invalidate_dead_paths_through(id))
    }

    /// Unified set-or-remove of the attached object on an edge or
    /// hyperedge. `Some(obj)` replaces / sets the payload, `None`
    /// removes it. Returns the previous attached object and what the
    /// cascade removed.
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
    ) -> Result<(Option<Object>, FullDelta), NoAttachedObjectError> {
        if !self.has_attached_object(&id) {
            return Err(NoAttachedObjectError { id });
        }
        let result = match new {
            Some(obj) => {
                let prev = self.entities.insert(id, obj);
                (prev, self.invalidate_dead_paths_through(id))
            }
            None => {
                let prev = self.entities.remove(&id);
                (prev, self.invalidate_all_paths_through(id))
            }
        };
        Ok(result)
    }

    pub(crate) fn silent_replace_attached_obj(
        &mut self,
        id: &AttachTargetId,
        obj: Object,
    ) -> Result<(Object, FullDelta), NoAttachedObjectError> {
        // The unified op guarantees `Some(old)` because
        // `has_attached_object` was true at the start.
        let (old, cascade) = self.silent_set_attached_obj(*id, Some(obj))?;
        Ok((
            old.expect("has_attached_object guaranteed an old object"),
            cascade,
        ))
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
        if !self.writable(id) {
            return Err(NoAttachedObjectError { id: *id });
        }
        let new_field_count = obj.len();
        let kind = self.attach_kind(id);

        let (old, cascade) = self.silent_replace_attached_obj(id, obj.clone())?;
        let delta = diff_object(&old, &obj);
        let use_change = delta.len() <= new_field_count;
        let id = *id;

        let operation = match (kind, use_change) {
            (Some(AttachKind::Hyperedge), true) => FullPatch::ChangeHyperedgeData {
                id,
                delta,
                old: old.clone(),
            },
            (Some(AttachKind::Hyperedge), false) => FullPatch::UpsertHyperedgeData {
                id,
                obj,
                old: Some(old.clone()),
            },
            (Some(AttachKind::Edge), true) => FullPatch::ChangeEdgeData {
                id,
                delta,
                old: old.clone(),
            },
            (Some(AttachKind::Edge), false) => FullPatch::UpsertEdgeData {
                id,
                obj,
                old: Some(old.clone()),
            },
            // Unreachable — `silent_replace_attached_obj` would have
            // failed otherwise.
            (None, _) => return Ok(Field::Object(old)),
        };
        self.record(operation, cascade);

        Ok(Field::Object(old))
    }

    pub(crate) fn silent_retarget_edge(
        &mut self,
        id: &Uuid,
        new_target: RetargetEdge,
    ) -> Result<Pointee, RetargetError> {
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
            return Ok(old_endpoint);
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

        Ok(old_endpoint)
    }

    /// Retarget one endpoint of an existing edge. Records
    /// [`FullPatch::RetargetEdge`].
    pub fn retarget_edge(
        &mut self,
        id: &Uuid,
        new_target: RetargetEdge,
    ) -> Result<(), RetargetError> {
        if !self.writable(id) {
            return Err(RetargetError::EdgeNotFound(EdgeNotFoundError { id: *id }));
        }
        let new_pointee = match &new_target {
            RetargetEdge::Source(p) | RetargetEdge::Target(p) => p,
        };
        if self.edges.contains_key(id) && !self.writable_pointee(new_pointee) {
            return Err(RetargetError::InvalidTarget(InvalidRetargetError {
                edge_id: *id,
                new_target,
            }));
        }
        let old = self.silent_retarget_edge(id, new_target.clone())?;
        let old_target = match new_target {
            RetargetEdge::Source(_) => RetargetEdge::Source(old),
            RetargetEdge::Target(_) => RetargetEdge::Target(old),
        };
        self.record(
            FullPatch::RetargetEdge {
                id: *id,
                new_target,
                old_target,
            },
            FullDelta::new(),
        );
        Ok(())
    }
}
