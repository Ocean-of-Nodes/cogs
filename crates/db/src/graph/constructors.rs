//! Add-something operations: `add_node`, `add_edge`, `create_hyperedge`
//! and their `silent_*` cousins for replay.

use std::collections::HashSet;

use uuid::Uuid;

use common::*;

use crate::errors::{
    AddEdgeError, CreateHyperedgeError, EdgeAlreadyExistsError, HyperedgeAlreadyExistsError,
    MissingEndpointsError, NodeAlreadyExistsError, PointeesNotFoundError,
};
use crate::graph::Graph;

impl Graph {
    pub(crate) fn silent_create_hyperedge_with_id(
        &mut self,
        id: &HyperedgeId,
        members: HashSet<Pointee>,
    ) -> Result<(), CreateHyperedgeError> {
        if members.is_empty() {
            return Err(CreateHyperedgeError::EmptyHyperedge);
        }

        if self.hyperedges.contains_key(id) {
            return Err(CreateHyperedgeError::HyperedgeAlreadyExists(
                HyperedgeAlreadyExistsError { id: id.clone() },
            ));
        }

        let missing = self.collect_missing_pointees(&members);
        if !missing.is_empty() {
            return Err(CreateHyperedgeError::PointeesNotFound(
                PointeesNotFoundError { pointees: missing },
            ));
        }

        // Update reverse index BEFORE storing — `track_pointee_entity`
        // needs the original `members` set; we register each member's
        // bucket and entity-to-paths secondary.
        for member in &members {
            self.track_pointee_entity(member);
            self.pointee_uses
                .entry(member.clone())
                .or_default()
                .hyperedges
                .insert(*id);
        }

        self.hyperedges.insert(id.clone(), members);
        Ok(())
    }

    /// Create a hyperedge containing the given non-empty set of
    /// existing pointees. Generates a new id and records
    /// [`Patch::CreateHyperedge`].
    pub fn create_hyperedge(
        &mut self,
        members: HashSet<Pointee>,
    ) -> Result<HyperedgeId, CreateHyperedgeError> {
        let id = Uuid::new_v4();
        self.silent_create_hyperedge_with_id(&id, members.clone())?;
        self.record_patch(Patch::CreateHyperedge { id, members });
        Ok(id)
    }

    pub(crate) fn silent_add_node_with_id(
        &mut self,
        id: NodeId,
        obj: Object,
    ) -> Result<(), NodeAlreadyExistsError> {
        if self.entities.contains_key(&id) {
            return Err(NodeAlreadyExistsError { id });
        }

        self.entities.insert(id, obj);
        Ok(())
    }

    /// Add a fresh node carrying `obj`. Generates a new id and
    /// records [`Patch::AddNode`] in the patch log.
    pub fn add_node(&mut self, obj: Object) -> NodeId {
        let id = Uuid::new_v4();
        let _ = self.silent_add_node_with_id(id, obj.clone());
        self.record_patch(Patch::AddNode { id, obj });
        id
    }

    pub(crate) fn silent_add_edge_with_id(
        &mut self,
        id: Uuid,
        source: Pointee,
        target: Pointee,
    ) -> Result<(), AddEdgeError> {
        let source_exists = self.is_pointee_exist(&source);
        let target_exists = self.is_pointee_exist(&target);
        if !source_exists || !target_exists {
            let mut missing_endpoints = Vec::new();
            if !source_exists {
                missing_endpoints.push(source);
            }
            if !target_exists {
                missing_endpoints.push(target);
            }
            return Err(AddEdgeError::MissingEndpoints(MissingEndpointsError {
                missing_endpoints,
            }));
        }

        if self.edges.contains_key(&id) {
            return Err(AddEdgeError::EdgeAlreadyExists(EdgeAlreadyExistsError {
                id,
            }));
        }

        // Update reverse index alongside the structural insert.
        self.track_pointee_entity(&source);
        self.pointee_uses
            .entry(source.clone())
            .or_default()
            .edges_as_source
            .insert(id);

        self.track_pointee_entity(&target);
        self.pointee_uses
            .entry(target.clone())
            .or_default()
            .edges_as_target
            .insert(id);

        self.edges.insert(id, (source, target));
        Ok(())
    }

    /// Add an edge from `source` to `target`. Both endpoints must
    /// resolve via [`Graph::is_pointee_exist`] at insertion time.
    /// Generates a new id and records [`Patch::AddEdge`].
    pub fn add_edge(
        &mut self,
        source: impl Into<Pointee>,
        target: impl Into<Pointee>,
    ) -> Result<EdgeId, AddEdgeError> {
        let source_pointee = source.into();
        let target_pointee = target.into();
        let edge_id = Uuid::new_v4();
        self.silent_add_edge_with_id(edge_id, source_pointee.clone(), target_pointee.clone())?;
        self.record_patch(Patch::AddEdge {
            id: edge_id,
            source: source_pointee,
            target: target_pointee,
        });
        Ok(edge_id)
    }
}
