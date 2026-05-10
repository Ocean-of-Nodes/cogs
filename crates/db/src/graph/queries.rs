//! Non-mutating reads on [`Graph`]: iterators over entities/edges/
//! hyperedges, getters by id, type-classification predicates.

use std::collections::HashSet;

use common::*;

use crate::errors::{EntityNotFoundError, GetEdgeError, IncorrectTypeError};
use crate::types::{EntityType, PointeeKind, EdgeView};
use crate::graph::Graph;

/// Which structural kind an attach-target belongs to. Drives the
/// choice between the `Edge*Data` and `HyperEdge*Data` patch families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttachKind {
    Edge,
    Hyperedge,
}


impl Graph {
    // ------------ ROOTS ------------------- //

    /// Iterate over every distinct entity id in the graph (nodes,
    /// edges, hyperedges; attached-object ids are deduplicated).
    pub fn iter_entities(&self) -> impl Iterator<Item = EntityId> {
        self.entities
            .keys()
            .copied()
            .chain(self.edges.keys().copied())
            .chain(self.hyperedges.keys().copied())
            // We need the hashset for dedup of attached id and hyperedge/edge id.
            .collect::<HashSet<_>>()
            .into_iter()
    }

    /// Iterate over all edges of the whole graph.
    pub fn iter_edges(&self) -> impl Iterator<Item = EdgeId> {
        self.edges.keys().copied()
    }

    /// Iterate over all nodes of the whole graph.
    ///
    /// A node is an id that has an attached object (key in
    /// `entities`) and does **not** also live as an edge or
    /// hyperedge. The exclusion matters because `attach_obj` can
    /// place an `Object` on an edge or hyperedge — in that case the
    /// id is in `entities` too, but it is not a node.
    pub fn iter_nodes(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.entities
            .keys()
            .copied()
            .filter(|id| !self.edges.contains_key(id) && !self.hyperedges.contains_key(id))
    }

    /// Iterate over all hyperedges of the whole graph.
    pub fn iter_hyperedges(&self) -> impl Iterator<Item = HyperedgeId> {
        self.hyperedges.keys().copied()
    }

    /// Iterate over ids that have an object attached on top of an
    /// edge or hyperedge (the "attach targets" — see
    /// [`AttachTargetId`]).
    ///
    /// In other words: entries in `entities` whose id is *also* a
    /// key in `edges` or `hyperedges`. This is the complement of
    /// [`Graph::iter_nodes`] within `entities`.
    pub fn iter_attached(&self) -> impl Iterator<Item = AttachTargetId> + '_ {
        self.entities
            .keys()
            .copied()
            .filter(|id| self.edges.contains_key(id) || self.hyperedges.contains_key(id))
    }

    // ------------ GETTERS ------------------- //

    /// Get the object stored at `id`, or `None` if there is none.
    /// Works for nodes and for edges/hyperedges with `attach_obj`.
    pub fn obj(&self, id: &EntityId) -> Option<&Object> {
        self.entities.get(id)
    }

    /// Get an edge as a [`EdgeView`] (id + source + target).
    ///
    /// # Errors
    ///
    /// - [`GetEdgeError::NotFound`] — `id` is not registered anywhere
    ///   in the graph.
    /// - [`GetEdgeError::IncorrectType`] — `id` exists, but it's a
    ///   different kind of entity (Node, HyperEdge, AttachedObject).
    pub fn edge(&self, id: &EdgeId) -> Result<EdgeView, GetEdgeError> {
        if let Some(pair) = self.edges.get(id) {
            return Ok(EdgeView {
                id: *id,
                source: pair.0.clone(),
                target: pair.1.clone(),
            });
        }

        match self.get_type(*id) {
            None => Err(GetEdgeError::NotFound(EntityNotFoundError { id: *id })),
            Some(ty) => Err(GetEdgeError::IncorrectType(IncorrectTypeError {
                entity_id: *id,
                expected_type: vec!["Edge".to_string()],
                actual_type: ty.to_string(),
            })),
        }
    }

    /// Direct lookup of a hyperedge's members. `None` if `id` is
    /// not a hyperedge. Companion to [`Graph::edge`].
    pub fn hyperedge_members(&self, id: &HyperedgeId) -> Option<&HashSet<Pointee>> {
        self.hyperedges.get(id)
    }

    // ------------ PREDICATES / CLASSIFICATION ------------------- //

    /// Classify `entity`. Returns `None` if the id isn't registered
    /// anywhere in the graph.
    ///
    /// Resolution order:
    /// - id in `entities` AND also in `edges` / `hyperedges`
    ///   → `AttachedObject` — the entry in `entities` represents an
    ///   object attached on top of a structural element via
    ///   [`Graph::attach_obj`]. Returning `AttachedObject` here is
    ///   what makes `attach_obj` reject double-attachment (because
    ///   `AttachedObject::is_attach_target() == false`).
    /// - id in `edges` only:
    ///     - if at least one endpoint is itself an edge or
    ///       hyperedge → `MetaEdge`
    ///     - otherwise → `Edge`
    /// - id in `hyperedges` only → `HyperEdge`
    /// - id in `entities` only → `Node`
    pub(crate) fn get_type(&self, entity: EntityId) -> Option<EntityType> {
        let in_entities = self.entities.contains_key(&entity);
        let in_edges = self.edges.contains_key(&entity);
        let in_hyper = self.hyperedges.contains_key(&entity);

        if !in_entities && !in_edges && !in_hyper {
            return None;
        }

        if in_entities && (in_edges || in_hyper) {
            return Some(EntityType::AttachedObject);
        }

        if in_edges {
            let (source, target) = self.edges.get(&entity).expect("checked above");
            // A subobject endpoint never makes the edge a metaedge:
            // it isn't an edge or a hyperedge by definition.
            let is_meta_endpoint = |e: &Pointee| match e {
                Pointee::EntityId(id) => {
                    self.edges.contains_key(id) || self.hyperedges.contains_key(id)
                }
                Pointee::Path(_) => false,
            };
            if is_meta_endpoint(source) || is_meta_endpoint(target) {
                return Some(EntityType::MetaEdge);
            }
            return Some(EntityType::Edge);
        }

        if in_hyper {
            return Some(EntityType::Hyperedge);
        }

        // Only in `entities` and not collided with any structural map.
        Some(EntityType::Node)
    }

    /// Classify a [`Pointee`]. Returns `None` if the pointee doesn't
    /// resolve to anything in the graph.
    ///
    /// - `Pointee::EntityId` — delegates to [`Graph::get_type`] and
    ///   maps each [`EntityType`] to the matching [`PointeeKind`].
    /// - `Pointee::Path` — yields [`PointeeKind::Subobject`] when the
    ///   path resolves (see [`Graph::is_pointee_exist`]).
    pub fn classify_pointee(&self, p: &Pointee) -> Option<PointeeKind> {
        match p {
            Pointee::EntityId(id) => self.get_type(*id).map(|t| match t {
                EntityType::Node => PointeeKind::Node,
                EntityType::Edge => PointeeKind::Edge,
                EntityType::Hyperedge => PointeeKind::Hyperedge,
                EntityType::MetaEdge => PointeeKind::MetaEdge,
                EntityType::AttachedObject => PointeeKind::AttachedObject,
            }),
            Pointee::Path(_) if self.is_pointee_exist(p) => Some(PointeeKind::Subobject),
            Pointee::Path(_) => None,
        }
    }

    /// Whether `id` is registered as any kind of entity in the graph.
    pub fn is_exist(&self, id: &EntityId) -> bool {
        self.entities.contains_key(id)
            || self.edges.contains_key(id)
            || self.hyperedges.contains_key(id)
    }

    /// Walk `local` through `obj`, descending into nested objects
    /// segment by segment. Returns the field reached by the last
    /// segment, or `None` if any segment is missing or attempts to
    /// traverse a non-object field.
    fn walk_path<'a>(obj: &'a Object, local: &LocalObjPath) -> Option<&'a Field> {
        let mut iter = local.iter();
        let first = iter.next()?;
        let mut current = obj.get(first)?;
        for seg in iter {
            match current {
                Field::Object(inner) => current = inner.get(seg)?,
                _ => return None,
            }
        }
        Some(current)
    }

    /// Check whether `p` resolves to something existing in the graph.
    ///
    /// - [`Pointee::EntityId`] — same as [`Graph::is_exist`].
    /// - [`Pointee::Path`] — the entity must exist *and* the local
    ///   field-chain must navigate cleanly through nested
    ///   [`Field::Object`]s and resolve a real field at the end.
    ///   Navigation fails (and the pointee is reported as missing)
    ///   if any intermediate segment hits a non-`Object` field.
    pub fn is_pointee_exist(&self, p: &Pointee) -> bool {
        match p {
            Pointee::EntityId(id) => self.is_exist(id),
            Pointee::Path(path) => self
                .entities
                .get(&path.entity())
                .and_then(|obj| Self::walk_path(obj, path.local()))
                .is_some(),
        }
    }

    /// True when `id` is a bare node (in `entities`, but not in
    /// `edges`/`hyperedges`).
    pub(crate) fn is_node(&self, id: &NodeId) -> bool {
        self.entities.contains_key(id)
            && !self.edges.contains_key(id)
            && !self.hyperedges.contains_key(id)
    }

    /// True when `id` is an edge or hyperedge that has an attached
    /// object (i.e. lives in both `entities` and `edges`/`hyperedges`).
    pub(crate) fn has_attached_object(&self, id: &AttachTargetId) -> bool {
        self.entities.contains_key(id)
            && (self.edges.contains_key(id) || self.hyperedges.contains_key(id))
    }

    /// Whether `id` is an edge or a hyperedge — used by attach-style
    /// ops to pick the right `Patch::*EdgeData` / `Patch::*HyperEdgeData`
    /// variant. Returns `None` if `id` isn't a known edge or hyperedge.
    pub(crate) fn attach_kind(&self, id: &AttachTargetId) -> Option<AttachKind> {
        if self.hyperedges.contains_key(id) {
            Some(AttachKind::Hyperedge)
        } else if self.edges.contains_key(id) {
            Some(AttachKind::Edge)
        } else {
            None
        }
    }

    /// Filter out the pointees that don't currently resolve.
    /// Used by hyperedge ops that need an "all members exist" check.
    pub(crate) fn collect_missing_pointees<'a, I>(&self, ps: I) -> HashSet<Pointee>
    where
        I: IntoIterator<Item = &'a Pointee>,
    {
        ps.into_iter()
            .filter(|p| !self.is_pointee_exist(p))
            .cloned()
            .collect()
    }
}