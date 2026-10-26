//! Patch-log replay: apply a recorded `Vec<ThinPatch>` to a graph
//! and reconstruct an identical state.

use uuid::Uuid;

use common::*;

use crate::errors::{ApplyPatchError, DeltaError, EntityNotFoundError};
use crate::object_patch::apply_object_patches;
use crate::graph::Graph;

impl Graph {
    /// Apply a sequence of `ObjectPatch` to a node's or attached
    /// object's `Object` at `id`. Cascades any `Pointee::Path`
    /// references through `id` that no longer resolve under the new
    /// shape. An empty `patch` is a successful no-op.
    pub(crate) fn apply_object_delta(
        &mut self,
        id: Uuid,
        patch: Vec<ObjectPatch>,
    ) -> Result<FullDelta, DeltaError> {
        let obj = self
            .entities
            .get_mut(&id)
            .ok_or(DeltaError::NotFound(EntityNotFoundError { id }))?;
        apply_object_patches(obj, patch).map_err(DeltaError::Delta)?;
        Ok(self.invalidate_dead_paths_through(id))
    }

    /// Apply a sequence of [`ThinPatch`]es to this graph in order.
    /// Each patch is dispatched to the matching `silent_*` op; any
    /// failure is wrapped into [`ApplyPatchError`] via the `From`
    /// impls. The cascade is recomputed by the ops, so their returned
    /// cascade deltas are dropped.
    pub(crate) fn apply_delta(&mut self, delta: Delta) -> Result<(), ApplyPatchError> {
        for patch in delta {
            match patch {
                ThinPatch::AddNode { id, obj } => self.silent_add_node_with_id(id, obj)?,
                ThinPatch::RemoveNode { id } => {
                    self.silent_remove_node(&id)?;
                }
                ThinPatch::ChangeNode { id, delta } => {
                    self.apply_object_delta(id, delta)?;
                }
                ThinPatch::UpsertNode { id, obj } => {
                    self.silent_replace_node(&id, obj)?;
                }
                ThinPatch::AddEdge { id, source, target } => {
                    self.silent_add_edge_with_id(id, source, target)?
                }
                ThinPatch::RemoveEdge { id } => {
                    self.silent_remove_edge(&id)?;
                }
                ThinPatch::RetargetEdge { id, new_target } => {
                    self.silent_retarget_edge(&id, new_target)?;
                }
                ThinPatch::UpsertEdgeData { id, obj } => {
                    self.silent_upsert_attached_obj(id, obj)?;
                }
                ThinPatch::ChangeEdgeData { id, delta } => {
                    self.apply_object_delta(id, delta)?;
                }
                ThinPatch::RemoveEdgeData { id } => {
                    self.silent_remove_attached(id)?;
                }
                ThinPatch::CreateHyperedge { id, members } => {
                    self.silent_create_hyperedge_with_id(&id, members)?
                }
                ThinPatch::RemoveHyperedge { id } => {
                    self.silent_remove_hyperedge(&id)?;
                }
                ThinPatch::AddHyperedgeMembers { id, members } => {
                    self.silent_add_hyperedge_members(id, members)?
                }
                ThinPatch::RemoveHyperedgeMembers { id, members } => {
                    self.silent_remove_hyperedge_members(id, members)?;
                }
                ThinPatch::UpsertHyperedgeData { id, obj } => {
                    self.silent_upsert_attached_obj(id, obj)?;
                }
                ThinPatch::ChangeHyperedgeData { id, delta } => {
                    self.apply_object_delta(id, delta)?;
                }
                ThinPatch::RemoveHyperedgeData { id } => {
                    self.silent_remove_attached(id)?;
                }
            }
        }

        Ok(())
    }
}
