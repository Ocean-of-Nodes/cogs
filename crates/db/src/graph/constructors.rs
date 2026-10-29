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

    /// [`Graph::create_hyperedge`] with a chosen id and without the
    /// input-layer guard.
    pub(crate) fn create_hyperedge_with_id(
        &mut self,
        id: HyperedgeId,
        members: HashSet<Pointee>,
    ) -> Result<(), CreateHyperedgeError> {
        self.silent_create_hyperedge_with_id(&id, members.clone())?;
        self.record(FullPatch::CreateHyperedge { id, members }, FullDelta::new());
        Ok(())
    }

    /// Create a hyperedge containing the given non-empty set of
    /// existing pointees. Generates a new id and records
    /// [`FullPatch::CreateHyperedge`].
    pub fn create_hyperedge(
        &mut self,
        members: HashSet<Pointee>,
    ) -> Result<HyperedgeId, CreateHyperedgeError> {
        let pointees = self.unwritable_pointees(&members);
        if !pointees.is_empty() {
            return Err(CreateHyperedgeError::PointeesNotFound(
                PointeesNotFoundError { pointees },
            ));
        }
        let id = Uuid::new_v4();
        self.create_hyperedge_with_id(id, members)?;
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

    /// [`Graph::add_node`] with a chosen id.
    pub(crate) fn add_node_with_id(
        &mut self,
        id: NodeId,
        obj: Object,
    ) -> Result<(), NodeAlreadyExistsError> {
        self.silent_add_node_with_id(id, obj.clone())?;
        self.record(FullPatch::AddNode { id, obj }, FullDelta::new());
        Ok(())
    }

    /// Add a fresh node carrying `obj`. Generates a new id and
    /// records [`FullPatch::AddNode`] in the patch log.
    pub fn add_node(&mut self, obj: Object) -> NodeId {
        let id = Uuid::new_v4();
        let _ = self.add_node_with_id(id, obj);
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

    /// [`Graph::add_edge`] with a chosen id and without the
    /// input-layer guard.
    pub(crate) fn add_edge_with_id(
        &mut self,
        id: EdgeId,
        source: Pointee,
        target: Pointee,
    ) -> Result<(), AddEdgeError> {
        self.silent_add_edge_with_id(id, source.clone(), target.clone())?;
        self.record(FullPatch::AddEdge { id, source, target }, FullDelta::new());
        Ok(())
    }

    /// Add an edge from `source` to `target`. Both endpoints must
    /// resolve via [`Graph::is_pointee_exist`] at insertion time.
    /// Generates a new id and records [`FullPatch::AddEdge`].
    pub fn add_edge(
        &mut self,
        source: impl Into<Pointee>,
        target: impl Into<Pointee>,
    ) -> Result<EdgeId, AddEdgeError> {
        let source_pointee = source.into();
        let target_pointee = target.into();
        if !self.writable_pointee(&source_pointee) || !self.writable_pointee(&target_pointee) {
            let missing_endpoints = [source_pointee, target_pointee]
                .into_iter()
                .filter(|p| !self.writable_pointee(p) || !self.is_pointee_exist(p))
                .collect();
            return Err(AddEdgeError::MissingEndpoints(MissingEndpointsError {
                missing_endpoints,
            }));
        }
        let edge_id = Uuid::new_v4();
        self.add_edge_with_id(edge_id, source_pointee, target_pointee)?;
        Ok(edge_id)
    }
}
