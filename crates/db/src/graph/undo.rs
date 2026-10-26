//! Undo: pop the last [`Modification`] off the event log and revert
//! it using the old state carried by its [`FullPatch`]es.

use common::*;

use crate::errors::{ApplyPatchError, UndoError};
use crate::graph::{Graph, Modification};

impl Graph {
    /// Revert the most recent mutation and drop it from the event
    /// log. Returns the reverted [`Modification`], or `None` if the
    /// log is empty.
    ///
    /// The main operation is reverted first, then the cascade. Cascade
    /// patches are restored in dependency order rather than in reverse:
    /// an edge `n1 → h` is removed before the hyperedge `h = {n1}`, but
    /// it can only come back after `h` does.
    pub fn undo(&mut self) -> Result<Option<Modification>, UndoError> {
        let Some(modification) = self.events.pop() else {
            return Ok(None);
        };

        let mut cascade = modification.cascade.clone();
        match &modification.operation {
            // The removed members emptied the hyperedge, and the
            // cascade's leading `RemoveHyperedge` (with no members)
            // deleted it. Recreate it with those members in one go.
            FullPatch::RemoveHyperedgeMembers { id, members }
                if matches!(
                    cascade.first(),
                    Some(FullPatch::RemoveHyperedge { id: hid, members: left, .. })
                        if hid == id && left.is_empty()
                ) =>
            {
                let FullPatch::RemoveHyperedge { data, .. } = cascade.remove(0) else {
                    unreachable!("matched above");
                };
                self.revert(FullPatch::RemoveHyperedge {
                    id: *id,
                    members: members.clone(),
                    data,
                })?;
            }
            operation => self.revert(operation.clone())?,
        }

        // Restore every cascade patch whose dependencies are back,
        // until nothing is left or no progress is made.
        while !cascade.is_empty() {
            let (ready, blocked): (FullDelta, FullDelta) = cascade
                .into_iter()
                .partition(|patch| self.can_revert(patch));
            if ready.is_empty() {
                return Err(UndoError::UnresolvedCascade(blocked));
            }
            for patch in ready {
                self.revert(patch)?;
            }
            cascade = blocked;
        }

        Ok(Some(modification))
    }

    /// Whether everything `patch`'s revert needs is present.
    fn can_revert(&self, patch: &FullPatch) -> bool {
        match patch {
            FullPatch::RemoveEdge { source, target, .. } => {
                self.is_pointee_exist(source) && self.is_pointee_exist(target)
            }
            FullPatch::RemoveHyperedge { members, .. } => {
                members.iter().all(|m| self.is_pointee_exist(m))
            }
            FullPatch::RemoveHyperedgeMembers { id, members } => {
                self.hyperedges.contains_key(id)
                    && members.iter().all(|m| self.is_pointee_exist(m))
            }
            // Never produced by a cascade.
            _ => true,
        }
    }

    /// Apply the inverse of `patch` without recording anything. The
    /// inverse never cascades: undo is LIFO, so whatever could depend
    /// on the reverted state has already been undone.
    fn revert(&mut self, patch: FullPatch) -> Result<(), ApplyPatchError> {
        match patch {
            FullPatch::AddNode { id, .. } => {
                self.silent_remove_node(&id)?;
            }
            FullPatch::RemoveNode { id, obj } => self.silent_add_node_with_id(id, obj)?,
            FullPatch::ChangeNode { id, old, .. } | FullPatch::UpsertNode { id, old, .. } => {
                self.silent_replace_node(&id, old)?;
            }

            FullPatch::AddEdge { id, .. } => {
                self.silent_remove_edge(&id)?;
            }
            FullPatch::RemoveEdge {
                id,
                source,
                target,
                data,
            } => {
                self.silent_add_edge_with_id(id, source, target)?;
                if let Some(obj) = data {
                    self.silent_upsert_attached_obj(id, obj)?;
                }
            }
            FullPatch::RetargetEdge { id, old_target, .. } => {
                self.silent_retarget_edge(&id, old_target)?;
            }
            FullPatch::UpsertEdgeData { id, old, .. }
            | FullPatch::UpsertHyperedgeData { id, old, .. } => match old {
                Some(obj) => {
                    self.silent_upsert_attached_obj(id, obj)?;
                }
                None => {
                    self.silent_remove_attached(id)?;
                }
            },
            FullPatch::ChangeEdgeData { id, old, .. }
            | FullPatch::ChangeHyperedgeData { id, old, .. }
            | FullPatch::RemoveEdgeData { id, obj: old }
            | FullPatch::RemoveHyperedgeData { id, obj: old } => {
                self.silent_upsert_attached_obj(id, old)?;
            }

            FullPatch::CreateHyperedge { id, .. } => {
                self.silent_remove_hyperedge(&id)?;
            }
            FullPatch::RemoveHyperedge { id, members, data } => {
                self.silent_create_hyperedge_with_id(&id, members)?;
                if let Some(obj) = data {
                    self.silent_upsert_attached_obj(id, obj)?;
                }
            }
            FullPatch::AddHyperedgeMembers { id, members } => {
                self.silent_remove_hyperedge_members(id, members)?;
            }
            FullPatch::RemoveHyperedgeMembers { id, members } => {
                self.silent_add_hyperedge_members(id, members)?
            }
        }
        Ok(())
    }
}
