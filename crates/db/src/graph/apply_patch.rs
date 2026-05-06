//! Patch-log replay: apply a recorded `Vec<Patch>` to a graph
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
    pub(crate) fn obj_apply_patch(
        &mut self,
        id: Uuid,
        patch: Vec<ObjectPatch>,
    ) -> Result<(), DeltaError> {
        let obj = self
            .entities
            .get_mut(&id)
            .ok_or(DeltaError::NotFound(EntityNotFoundError { id }))?;
        apply_object_patches(obj, patch).map_err(DeltaError::Delta)?;
        self.cascade_invalid_paths_through(id);
        Ok(())
    }

    /// Apply a sequence of [`Patch`]es to this graph in order. Each
    /// patch is dispatched to the matching `silent_*` op; any failure
    /// is wrapped into [`ApplyPatchError`] via the `From` impls.
    pub(crate) fn apply_patch(&mut self, delta: Delta) -> Result<(), ApplyPatchError> {
        for patch in delta {
            match patch {
                Patch::AddNode { id, obj } => self.silent_add_node_with_id(id, obj)?,
                Patch::RemoveNode { id } => {
                    self.silent_remove_node(&id)?;
                }
                Patch::ChangeNode { id, delta } => self.obj_apply_patch(id, delta)?,
                Patch::UpsertNode { id, obj } => {
                    self.silent_replace_node(&id, obj)?;
                }
                Patch::AddEdge { id, source, target } => {
                    self.silent_add_edge_with_id(id, source, target)?
                }
                Patch::RemoveEdge { id } => {
                    self.silent_remove_edge(&id)?;
                }
                Patch::RetargetEdge { id, new_target } => {
                    self.silent_retarget_edge(&id, new_target)?
                }
                Patch::UpsertEdgeData { id, obj } => self.silent_upsert_attached_obj(id, obj)?,
                Patch::ChangeEdgeData { id, delta } => self.obj_apply_patch(id, delta)?,
                Patch::RemoveEdgeData { id } => self.silent_remove_attached(id)?,
                Patch::CreateHyperEdge { id, members } => {
                    self.silent_create_hyperedge_with_id(&id, members)?
                }
                Patch::RemoveHyperEdge { id } => {
                    self.silent_remove_hyperedge(&id)?;
                }
                Patch::AddElementsToHyperEdge { id, members } => {
                    self.silent_add_hyperedge_members(id, members)?
                }
                Patch::RemoveElementsFromHyperEdge { id, members } => {
                    self.silent_remove_hyperedge_members(id, members)?
                }
                Patch::UpsertHyperEdgeData { id, obj } => {
                    self.silent_upsert_attached_obj(id, obj)?
                }
                Patch::ChangeHyperEdgeData { id, delta } => self.obj_apply_patch(id, delta)?,
                Patch::RemoveHyperEdgeData { id } => self.silent_remove_attached(id)?,
            }
        }

        Ok(())
    }
}
