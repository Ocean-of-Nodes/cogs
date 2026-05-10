//! Remove-something operations: `remove_node`, `remove_edge`,
//! `remove_hyperedge`, `remove_attached`. Each cascades through the
//! reverse index for any references that become dangling.

use std::collections::HashSet;

use common::*;

use crate::errors::{
    EdgeNotFoundError, HyperedgeNotFoundError, NoAttachedObjectError, NodeNotFoundError,
};
use crate::graph::AttachKind;
use crate::types::EdgeView;
use crate::graph::Graph;

impl Graph {
    pub(crate) fn silent_remove_node(
        &mut self,
        id: &NodeId,
    ) -> Result<Object, NodeNotFoundError> {
        if !self.is_node(id) {
            return Err(NodeNotFoundError { id: *id });
        }
        let obj = self
            .entities
            .remove(id)
            .ok_or(NodeNotFoundError { id: *id })?;
        self.cascade_remove_entity(*id);
        Ok(obj)
    }

    /// Remove a node and cascade-delete every edge / hyperedge that
    /// referenced it (directly or via a `Pointee::Path`). Records
    /// one [`Patch::RemoveNode`] regardless of cascade depth.
    /// Returns the node's previous object as `Field::Object`.
    pub fn remove_node(&mut self, id: &NodeId) -> Result<Object, NodeNotFoundError> {
        let obj = self.silent_remove_node(id)?;
        self.record_patch(Patch::RemoveNode { id: *id });
        Ok(obj)
    }

    // NOTE: visibility was tightened from `pub` to `pub(crate)`. External
    // callers expecting `Graph::silent_remove_edge(...)` would no
    // longer compile — this op is only meant for replay through
    // `apply_patch` and for direct construction inside `remove_edge`.
    pub(crate) fn silent_remove_edge(
        &mut self,
        id: &EdgeId,
    ) -> Result<EdgeView, EdgeNotFoundError> {
        let (source, target) = self
            .edges
            .remove(id)
            .ok_or(EdgeNotFoundError { id: *id })?;

        self.unregister_edge_from_endpoints(id, &source, &target, None);

        // Drop attached object on this edge, if any.
        self.entities.remove(id);

        Ok(EdgeView {
            id: *id,
            source,
            target,
        })
    }

    /// Remove an edge by id. The edge's reverse-index entries are
    /// cleaned up. Records [`Patch::RemoveEdge`].
    /// Returns the [`EdgeView`] of the removed edge.
    pub fn remove_edge(&mut self, id: &EdgeId) -> Result<EdgeView, EdgeNotFoundError> {
        let res = self.silent_remove_edge(id)?;
        self.record_patch(Patch::RemoveEdge { id: *id });
        Ok(res)
    }

    pub(crate) fn silent_remove_hyperedge(
        &mut self,
        hid: &HyperedgeId,
    ) -> Result<HashSet<Pointee>, HyperedgeNotFoundError> {
        let members = self
            .hyperedges
            .remove(hid)
            .ok_or(HyperedgeNotFoundError { id: *hid })?;

        // Strip `hid` from each member's reverse-index bucket.
        for member in &members {
            if let Some(bucket) = self.pointee_uses.get_mut(member) {
                bucket.hyperedges.remove(hid);
                if bucket.is_empty() {
                    self.pointee_uses.remove(member);
                    self.untrack_pointee_entity(member);
                }
            }
        }

        // Drop attached object on this hyperedge, if any.
        self.entities.remove(hid);

        // Anything that pointed at `hid` (edges with EntityId/Path
        // endpoints, other hyperedges that had `hid` as member) is
        // now dangling — let the cascade clean it up.
        self.cascade_remove_entity(*hid);

        Ok(members)
    }

    /// Remove a hyperedge by id. Cascades to anything that
    /// referenced it (edges, parent hyperedges that lose this
    /// member and become empty). Records one
    /// [`Patch::RemoveHyperedge`]. Returns the previous member set.
    pub fn remove_hyperedge(
        &mut self,
        hid: &HyperedgeId,
    ) -> Result<HashSet<Pointee>, HyperedgeNotFoundError> {
        let members = self.silent_remove_hyperedge(hid)?;
        self.record_patch(Patch::RemoveHyperedge { id: *hid });
        Ok(members)
    }

    /// Remove the attached object on an edge or hyperedge. The
    /// structural element itself stays alive; only the `Object`
    /// stored on top of it is dropped, along with any `Pointee::Path`
    /// references that depended on the attached object's fields.
    pub(crate) fn silent_remove_attached(
        &mut self,
        target: AttachTargetId,
    ) -> Result<(), NoAttachedObjectError> {
        self.silent_set_attached_obj(target, None).map(|_| ())
    }

    /// Remove the attached object on an edge or hyperedge and emit
    /// the corresponding `Remove*Data` patch.
    pub fn remove_attached(
        &mut self,
        target: AttachTargetId,
    ) -> Result<(), NoAttachedObjectError> {
        // Resolve the kind *before* the silent op — the structure
        // stays alive after the call, but kind resolution is a
        // pre-condition for picking the patch variant.
        let kind = self.attach_kind(&target);

        self.silent_remove_attached(target)?;

        match kind {
            Some(AttachKind::Edge) => self.record_patch(Patch::RemoveEdgeData { id: target }),
            Some(AttachKind::Hyperedge) => {
                self.record_patch(Patch::RemoveHyperedgeData { id: target })
            }
            None => {}
        }
        Ok(())
    }
}
