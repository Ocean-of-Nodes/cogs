mod hyperedge;
mod incidence;
mod methods;
mod paths;

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use uuid::Uuid;

use common::*;

/// Listener is a function that will be called when graph be changed
type ListernerID = Uuid;

#[derive(Debug)]
struct HyperEdgeNotFound(HyperEdgeId);
#[derive(Debug)]
struct EntityNotFoundError(EntityId);
#[derive(Debug)]
struct UnexistentPathError {
    /// If we pass an nonexistent path, for example
    /// "/subgraph1/subgraph2/subgraph3",
    /// where subgpraph1 exist but subgraph2 doesn't,
    /// then we will return error with valid_path_part = "/subgraph1"
    valid_path_part: PathBuf,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct NodeAlreadyExistsError(NodeId);
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct HyperEdgeAlreadyExistsError(HyperEdgeId);
#[derive(Debug)]
struct NodeNotFoundError(NodeId);
#[derive(Debug)]
struct EdgeNotFoundError(EdgeID);
#[derive(Debug, PartialEq, Eq)]
struct MissingEndpointsError {
    missing_endpoints: Vec<Pointee>,
}

#[derive(PartialEq, Eq, Debug)]
struct PointeesNotFound(HashSet<Pointee>);

#[derive(PartialEq, Eq, Debug)]
enum CreateHyperEdgeError {
    PointeesNotFound(PointeesNotFound),
    HyperEdgeAlreadyExists(HyperEdgeAlreadyExistsError),
    EmptyHyperEdge,
}

#[derive(Debug)]
struct IncorrectTypeError {
    node_id: NodeId,
    expected_type: Vec<String>,
    actual_type: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AddEdgeError {
    MissingEndpointsError(MissingEndpointsError),
    EdgeAlreadyExists(EdgeID),
}

#[derive(Debug)]
enum RetargetError {
    EdgeNotFound(EdgeID),
    InvalidTarget(RetrargetEdge),
}

#[derive(Debug)]
enum AttachNodeError {
    AttachTargetNotFound,
    IncorrectType(IncorrectTypeError),
}

#[derive(Debug)]
enum GetEdgeError {
    NotFound(EntityNotFoundError),
    IncorrectType(IncorrectTypeError),
}

#[derive(Debug)]
struct MembersAlreadyExist(Vec<Pointee>);

#[derive(Debug)]
enum AddHyperedgeMembers {
    NotFound(EntityNotFoundError),
    MembersAlreadyExist(MembersAlreadyExist),
    PointeesNotFound(PointeesNotFound),
}

#[derive(Debug)]
enum RemoveHyperedgeMembers {/* TODO */}

#[derive(Debug)]
enum RemoveAttached {
    NodeOrAttachedNotFound,
}

/// Errors returned while applying an [`ObjectDelta`] in-place.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ObjectPatchError {
    /// `AddField` on a name that already exists.
    FieldAlreadyExists(String),
    /// `RemoveField`/`ReplaceField`/`ArrayDelta` referencing a name
    /// that's not in the object.
    FieldNotFound(String),
    /// A path segment landed on a non-`Object` field.
    NotAnObject(String),
    /// `ArrayDelta` named a field that is not a `Field::Array`.
    NotAnArray(String),
    /// An array index in `removed_indices`/`added_fields` is past
    /// the array's bounds at apply time.
    IndexOutOfBounds(usize),
    /// `ChangeNode`/`ChangeData` was given an empty delta vector.
    EmptyDeltaVector,
}

/// Errors returned while applying high-level object changes.
#[derive(Debug)]
pub(crate) enum DeltaError {
    /// The target entity isn't in the graph (or, for `ChangeData`,
    /// has no attached object).
    NotFound(EntityId),
    /// Failure inside the delta application itself.
    Delta(ObjectPatchError),
}

/// Errors returned by [`Graph::apply_patch`].
#[derive(Debug)]
pub(crate) enum ApplyPatchError {
    NodeAlreadyExists(NodeAlreadyExistsError),
    AddEdge(AddEdgeError),
    CreateHyperEdgeError(CreateHyperEdgeError),
    NodeNotFound(NodeNotFoundError),
    EdgeNotFound(EdgeNotFoundError),
    HyperEdgeNotFound(HyperEdgeNotFound),
    Retarget(RetargetError),
    Change(DeltaError),
    AddHyperedgeMembers(AddHyperedgeMembers),
    RemoveHyperedgeMembers(RemoveHyperedgeMembers),
    RemoveAttached(RemoveAttached),
}

/// Triplet is a link beetween two pointees
#[derive(Debug, PartialEq, Eq)]
struct Triplet {
    id: EdgeID,
    source: Pointee,
    target: Pointee,
}

enum EntityType {
    Node,
    Edge,
    HyperEdge,
    MetaEdge,
    AttachedObject,
}

/// Classification of a [`Pointee`]. Mirrors [`EntityType`] for the
/// `Pointee::EntityId` case and adds [`PointeeKind::Subobject`] for
/// the `Pointee::Path` case.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PointeeKind {
    Node,
    Edge,
    HyperEdge,
    MetaEdge,
    AttachedObject,
    Subobject,
}

impl EntityType {
    fn is_attach_target(&self) -> bool {
        match self {
            EntityType::Node | EntityType::AttachedObject => false,
            EntityType::Edge | EntityType::HyperEdge | EntityType::MetaEdge => true,
        }
    }

    fn is_can_contains_object(&self) -> bool {
        match self {
            EntityType::Node | EntityType::Edge | EntityType::HyperEdge | EntityType::MetaEdge => {
                true
            }
            EntityType::AttachedObject => false,
        }
    }
}

impl Into<&'static str> for EntityType {
    fn into(self) -> &'static str {
        match self {
            EntityType::Node => "Node",
            EntityType::Edge => "Edge",
            EntityType::HyperEdge => "HyperEdge",
            EntityType::MetaEdge => "MetaEdge",
            EntityType::AttachedObject => "AttachedObject",
        }
    }
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EntityType::Node => "Node",
            EntityType::Edge => "Edge",
            EntityType::HyperEdge => "HyperEdge",
            EntityType::MetaEdge => "MetaEdge",
            EntityType::AttachedObject => "AttachedObject",
        })
    }
}

#[derive(Default)]
struct PointeeUses {
    edges_as_source: HashSet<EdgeID>,
    edges_as_target: HashSet<EdgeID>,
    hyperedges: HashSet<HyperEdgeId>,
}

impl PointeeUses {
    fn is_empty(&self) -> bool {
        self.edges_as_source.is_empty()
            && self.edges_as_target.is_empty()
            && self.hyperedges.is_empty()
    }
}

#[derive(Default)]
struct Graph {
    /// Hold graph object (nodes and attached object)
    entities: HashMap<EntityId, Object>,

    /// Hold graph edges
    edges: HashMap<EdgeID, (Pointee, Pointee)>,

    /// Hold hyperedges
    hyper_edge: HashMap<HyperEdgeId, HashSet<Pointee>>,

    /// Pointee → {
    ///     edges-as-source,
    ///     edges-as-target,
    ///     hyperedges of which it is a member,
    /// }
    pointee_uses: HashMap<Pointee, PointeeUses>,

    /// EntityId --- Pointee
    entity_to_path_pointees: HashMap<EntityId, HashSet<Pointee>>,

    /// Patch log
    events: Delta,
}

impl Graph {
    // -------------- START INDEXES ----------------------- //
    fn track_pointee_entity(&mut self, p: &Pointee) {
        if let Pointee::Path(gp) = p {
            self.entity_to_path_pointees
                .entry(gp.entity())
                .or_default()
                .insert(p.clone());
        }
    }

    fn untrack_pointee_entity(&mut self, p: &Pointee) {
        if let Pointee::Path(gp) = p {
            if let Some(set) = self.entity_to_path_pointees.get_mut(&gp.entity()) {
                set.remove(p);
                if set.is_empty() {
                    self.entity_to_path_pointees.remove(&gp.entity());
                }
            }
        }
    }

    // -------------- END INDEXES ------------------------- //

    // ------------ START ROOTS ------------------- //
    // Roots are methods called on root graph

    /// Iterate over all entities of the whole graph
    pub fn iter_entities(&self) -> impl Iterator<Item = EntityId> {
        self.entities
            .keys()
            .copied()
            .chain(self.edges.keys().copied())
            .chain(self.hyper_edge.keys().copied())
            // We need here hashset for dedublication attached id and hyperedge/edge id
            .collect::<HashSet<_>>()
            .into_iter()
    }

    /// Iterate over all edges of the whole graph
    pub fn iter_edges(&self) -> impl Iterator<Item = EdgeID> {
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
            .filter(|id| !self.edges.contains_key(id) && !self.hyper_edge.contains_key(id))
    }

    /// Iterate over all hyperedges of whole graph
    pub fn iter_hyperedge(&self) -> impl Iterator<Item = HyperEdgeId> {
        self.hyper_edge.keys().copied()
    }

    /// Iterate over ids that have an object attached on top of an
    /// edge or hyperedge (the "attach targets" — see
    /// [`AttachTargetID`]).
    ///
    /// In other words: entries in `entities` whose id is *also* a
    /// key in `edges` or `hyper_edge`. This is the complement of
    /// [`Graph::iter_nodes`] within `entities`.
    pub fn iter_attached(&self) -> impl Iterator<Item = AttachTargetID> + '_ {
        self.entities
            .keys()
            .copied()
            .filter(|id| self.edges.contains_key(id) || self.hyper_edge.contains_key(id))
    }
    // ------------ END ROOTS ------------------- //

    /* ------------ START GETTERS ------------------- */

    /// Get obj by entity id from the whole graph,
    /// returns None if field doesn't exist
    pub fn obj(&self, id: &EntityId) -> Option<&Object> {
        self.entities.get(id)
    }

    /// Get a triplet by id.
    ///
    /// # Errors
    ///
    /// - [`GetEdgeError::NotFound`] — `id` is not registered anywhere
    ///   in the graph.
    /// - [`GetEdgeError::IncorrectType`] — `id` exists, but it's a
    ///   different kind of entity (Node, HyperEdge, AttachedObject).
    pub fn edge(&self, id: &EdgeID) -> Result<Triplet, GetEdgeError> {
        if let Some(pair) = self.edges.get(id) {
            return Ok(Triplet {
                id: *id,
                source: pair.0.clone(),
                target: pair.1.clone(),
            });
        }

        match self.get_type(*id) {
            None => Err(GetEdgeError::NotFound(EntityNotFoundError(*id))),
            Some(ty) => Err(GetEdgeError::IncorrectType(IncorrectTypeError {
                node_id: *id,
                expected_type: vec!["Edge".to_string()],
                actual_type: ty.to_string(),
            })),
        }
    }

    /// Direct lookup of a hyperedge's members. `None` if `id` is
    /// not a hyperedge. Companion to [`Graph::edge`].
    pub fn hyperedge_members(&self, id: &HyperEdgeId) -> Option<&HashSet<Pointee>> {
        self.hyper_edge.get(id)
    }

    /* ------------ END GETTERS -------------------- */

    /* ------------ START PROBS -------------------- */

    /// Classify `entity`. Returns `None` if the id isn't registered
    /// anywhere in the graph.
    ///
    /// Resolution order:
    /// - id in `entities` AND also in `edges` / `hyper_edge`
    ///   → `AttachedObject` — the entry in `entities` represents an
    ///   object attached on top of a structural element via
    ///   [`attach_obj`]. Returning `AttachedObject` here is what
    ///   makes `attach_obj` reject double-attachment (because
    ///   `AttachedObject::is_attach_target() == false`).
    /// - id in `edges` only:
    ///     - if at least one endpoint is itself an edge or
    ///       hyperedge → `MetaEdge`
    ///     - otherwise → `Edge`
    /// - id in `hyper_edge` only → `HyperEdge`
    /// - id in `entities` only → `Node`
    pub fn get_type(&self, entity: EntityId) -> Option<EntityType> {
        let in_entities = self.entities.contains_key(&entity);
        let in_edges = self.edges.contains_key(&entity);
        let in_hyper = self.hyper_edge.contains_key(&entity);

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
                    self.edges.contains_key(id) || self.hyper_edge.contains_key(id)
                }
                Pointee::Path(_) => false,
            };
            if is_meta_endpoint(source) || is_meta_endpoint(target) {
                return Some(EntityType::MetaEdge);
            }
            return Some(EntityType::Edge);
        }

        if in_hyper {
            return Some(EntityType::HyperEdge);
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
                EntityType::HyperEdge => PointeeKind::HyperEdge,
                EntityType::MetaEdge => PointeeKind::MetaEdge,
                EntityType::AttachedObject => PointeeKind::AttachedObject,
            }),
            Pointee::Path(_) if self.is_pointee_exist(p) => Some(PointeeKind::Subobject),
            Pointee::Path(_) => None,
        }
    }

    /// Check if id is node or edge from the whole graph,
    /// returns true if node or edge exists, false otherwise
    pub fn is_exist(&self, id: &EntityId) -> bool {
        self.iter_entities().any(|e| &e == id)
    }

    /// Walk `local` through `obj`, descending into nested objects
    /// segment by segment. Returns the field reached by the last
    /// segment, or `None` if any segment is missing or attempts to
    /// traverse a non-object field.
    fn navigate_object<'a>(obj: &'a Object, local: &LocalObjPath) -> Option<&'a Field> {
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
                .and_then(|obj| Self::navigate_object(obj, path.local()))
                .is_some(),
        }
    }

    fn is_node(&self, id: &NodeId) -> bool {
        self.entities.contains_key(id)
            && !self.edges.contains_key(id)
            && !self.hyper_edge.contains_key(id)
    }

    /* ------------ END PROBS -------------------- */

    /* ------------ START CONSTRUCTORS ----------- */

    fn silent_create_hyperedge_with_id(
        &mut self,
        id: &HyperEdgeId,
        members: HashSet<Pointee>,
    ) -> Result<(), CreateHyperEdgeError> {
        if members.is_empty() {
            return Err(CreateHyperEdgeError::EmptyHyperEdge);
        }

        if self.hyper_edge.contains_key(id) {
            return Err(CreateHyperEdgeError::HyperEdgeAlreadyExists(
                HyperEdgeAlreadyExistsError(id.clone()),
            ));
        }

        let mut unexist = HashSet::new();
        for member in members.iter() {
            if !self.is_pointee_exist(member) {
                unexist.insert(member.clone());
            }
        }

        if !unexist.is_empty() {
            return Err(CreateHyperEdgeError::PointeesNotFound(PointeesNotFound(unexist)));
        }

        self.hyper_edge.insert(id.clone(), members);
        Ok(())
    }

    pub fn create_hyperedge(
        &mut self,
        members: HashSet<Pointee>,
    ) -> Result<HyperEdgeId, CreateHyperEdgeError> {
        // ---- Create hyperedge ----
        let id = Uuid::new_v4();
        self.silent_create_hyperedge_with_id(&id, members.clone())?;

        // ---- Indexes -----
        for member in members.iter() {
            self.track_pointee_entity(member);
            self.pointee_uses
                .entry(member.clone())
                .or_default()
                .hyperedges
                .insert(id);
        }

        // ---- Record Patch ----
        self.record_patch(Patch::CreateHyperEdge {
            id: id.clone(),
            members,
        });

        Ok(id)
    }

    fn silent_add_node_with_id(
        &mut self,
        id: NodeId,
        obj: Object,
    ) -> Result<(), NodeAlreadyExistsError> {
        if self.entities.contains_key(&id) {
            return Err(NodeAlreadyExistsError(id));
        }

        self.entities.insert(id, obj);
        Ok(())
    }

    pub fn add_node(&mut self, obj: Object) -> NodeId {
        let id = Uuid::new_v4();
        self.silent_add_node_with_id(id, obj.clone());
        self.record_patch(Patch::AddNode { id, obj: obj });
        id
    }

    fn silent_add_edge_with_id(
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
            return Err(AddEdgeError::MissingEndpointsError(MissingEndpointsError {
                missing_endpoints,
            }));
        }

        if self.edges.contains_key(&id) {
            return Err(AddEdgeError::EdgeAlreadyExists(id));
        }

        self.edges.insert(id, (source, target));
        Ok(())
    }

    pub fn add_edge(
        &mut self,
        source: impl Into<Pointee>,
        target: impl Into<Pointee>,
    ) -> Result<EdgeID, AddEdgeError> {
        // --- Creating ----
        let source_pointee = source.into();
        let target_pointee = target.into();
        let edge_id = Uuid::new_v4();
        self.silent_add_edge_with_id(edge_id, source_pointee.clone(), target_pointee.clone())?;

        // ---- Indexes ----
        self.track_pointee_entity(&source_pointee);
        self.pointee_uses
            .entry(source_pointee.clone())
            .or_default()
            .edges_as_source
            .insert(edge_id);

        self.track_pointee_entity(&target_pointee);
        self.pointee_uses
            .entry(target_pointee.clone())
            .or_default()
            .edges_as_target
            .insert(edge_id);

        // ---- Record patch ----
        self.record_patch(Patch::AddEdge {
            id: edge_id,
            source: source_pointee,
            target: target_pointee,
        });

        Ok(edge_id)
    }

    /* ------------ END CONSTRUCTORS ------------- */

    /* ------------ START DESTRUCTORS ----------- */

    /// Clears the reverse index and cascades deletes all
    /// that become dangling due to the disappearance of this
    /// id (or its subpaths).
    /// Drains a single pointee bucket: removes referencing edges,
    /// strips the pointee from hyperedge memberships (killing
    /// hyperedges that empty out). New dangling structural ids are
    /// pushed onto `worklist` for the caller to process.
    fn drain_pointee_bucket(&mut self, pointee: &Pointee, worklist: &mut Vec<EntityId>) {
        let Some(uses) = self.pointee_uses.remove(pointee) else {
            return;
        };

        let dead_edges: HashSet<EdgeID> = uses
            .edges_as_source
            .iter()
            .chain(uses.edges_as_target.iter())
            .copied()
            .collect();
        for eid in dead_edges {
            if let Some((src, tgt)) = self.edges.remove(&eid) {
                for endpoint in [&src, &tgt] {
                    if endpoint == pointee {
                        continue;
                    }
                    if let Some(bucket) = self.pointee_uses.get_mut(endpoint) {
                        bucket.edges_as_source.remove(&eid);
                        bucket.edges_as_target.remove(&eid);
                        if bucket.is_empty() {
                            self.pointee_uses.remove(endpoint);
                            self.untrack_pointee_entity(endpoint);
                        }
                    }
                }
                self.entities.remove(&eid);
                worklist.push(eid);
            }
        }

        for hid in uses.hyperedges {
            if let Some(members) = self.hyper_edge.get_mut(&hid) {
                members.remove(pointee);
                if members.is_empty() {
                    self.hyper_edge.remove(&hid);
                    self.entities.remove(&hid);
                    worklist.push(hid);
                }
            }
        }
    }

    /// For one fully-dead entity, drain every pointee that becomes
    /// invalid: `Pointee::EntityId(dead_id)` and every `Pointee::Path`
    /// through it (looked up via `entity_to_path_pointees`).
    fn cascade_drain_id(&mut self, dead_id: EntityId, worklist: &mut Vec<EntityId>) {
        let mut affected: Vec<Pointee> = vec![Pointee::EntityId(dead_id)];
        if let Some(paths) = self.entity_to_path_pointees.remove(&dead_id) {
            affected.extend(paths);
        }
        for pointee in affected {
            self.drain_pointee_bucket(&pointee, worklist);
        }
    }

    fn cascade_remove_id(&mut self, removed: EntityId) {
        let mut worklist: Vec<EntityId> = vec![removed];
        while let Some(dead_id) = worklist.pop() {
            self.cascade_drain_id(dead_id, &mut worklist);
        }
    }

    /// Cascade-clean every reference that depended on `entity` having
    /// an attached object — i.e., every `Pointee::Path` rooted at this
    /// entity. The entity itself (edge or hyperedge) stays alive, so
    /// `Pointee::EntityId(entity)` references are preserved. Used by
    /// `remove_attached`.
    fn cascade_path_references_through(&mut self, entity: EntityId) {
        let Some(paths) = self.entity_to_path_pointees.remove(&entity) else {
            return;
        };
        let mut worklist: Vec<EntityId> = Vec::new();
        for pointee in paths {
            self.drain_pointee_bucket(&pointee, &mut worklist);
        }
        while let Some(dead_id) = worklist.pop() {
            self.cascade_drain_id(dead_id, &mut worklist);
        }
    }

    fn silent_remove_node(&mut self, id: &NodeId) -> Result<Field, NodeNotFoundError> {
        if !self.is_node(id) {
            return Err(NodeNotFoundError(*id));
        }
        let obj = self.entities.remove(id).ok_or(NodeNotFoundError(*id))?;
        self.cascade_remove_id(*id);
        Ok(Field::Object(obj))
    }

    pub fn remove_node(&mut self, id: &NodeId) -> Result<Field, NodeNotFoundError> {
        let field = self.silent_remove_node(id)?;
        self.record_patch(Patch::RemoveNode { id: *id });
        Ok(field)
    }

    pub fn silent_remove_edge(&mut self, id: &EdgeID) -> Result<Triplet, EdgeNotFoundError> {
        let (source, target) = self.edges.remove(id).ok_or(EdgeNotFoundError(*id))?;
        Ok(Triplet {
            id: *id,
            source,
            target,
        })
    }

    pub fn remove_edge(&mut self, id: &EdgeID) -> Result<Triplet, EdgeNotFoundError> {
        let res = self.silent_remove_edge(id)?;
        self.record_patch(Patch::RemoveEdge { id: *id });
        Ok(res)
    }

    fn silent_remove_hyperedge(
        &mut self,
        hid: &HyperEdgeId,
    ) -> Result<HashSet<Pointee>, HyperEdgeNotFound> {
        let members = self.hyper_edge.remove(hid).ok_or(HyperEdgeNotFound(*hid))?;

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
        self.cascade_remove_id(*hid);

        Ok(members)
    }

    pub fn remove_hyperedge(
        &mut self,
        hid: &HyperEdgeId,
    ) -> Result<HashSet<Pointee>, HyperEdgeNotFound> {
        let members = self.silent_remove_hyperedge(hid)?;
        self.record_patch(Patch::RemoveHyperEdge { id: *hid });
        Ok(members)
    }

    /// Remove the attached object on an edge or hyperedge. The
    /// structural element itself stays alive; only the `Object`
    /// stored on top of it is dropped, along with any `Pointee::Path`
    /// references that depended on the attached object's fields.
    pub fn silent_remove_attached(&mut self, target: AttachTargetID) -> Result<(), RemoveAttached> {
        let is_attach_target = self.entities.contains_key(&target)
            && (self.edges.contains_key(&target) || self.hyper_edge.contains_key(&target));
        if !is_attach_target {
            return Err(RemoveAttached::NodeOrAttachedNotFound);
        }

        self.entities.remove(&target);
        self.cascade_path_references_through(target);
        Ok(())
    }

    pub fn remove_attached(&mut self, target: AttachTargetID) -> Result<(), RemoveAttached> {
        // Determine the patch variant *before* the silent op — both
        // structures stay alive after the call, but resolving the
        // type is conceptually a pre-condition.
        let is_edge = self.edges.contains_key(&target);
        let is_hyper = self.hyper_edge.contains_key(&target);

        self.silent_remove_attached(target)?;

        if is_edge {
            self.record_patch(Patch::RemoveEdgeData { id: target });
        } else if is_hyper {
            self.record_patch(Patch::RemoveHyperEdgeData { id: target });
        }
        Ok(())
    }

    /* ------------ END DESTRUCTORS ------------- */

    /* ------------ START MODIFIERS ----------- */

    pub fn attach_obj(
        &mut self,
        target: AttachTargetID,
        obj: Object,
    ) -> Result<(), AttachNodeError> {
        let ty = match self.get_type(target) {
            Some(t) => t,
            None => return Err(AttachNodeError::AttachTargetNotFound),
        };
        if !ty.is_attach_target() {
            return Err(AttachNodeError::IncorrectType(IncorrectTypeError {
                node_id: target,
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

    fn silent_add_hyperedge_members(
        &mut self,
        id: HyperEdgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), AddHyperedgeMembers> {
        if !self.hyper_edge.contains_key(&id) {
            return Err(AddHyperedgeMembers::NotFound(EntityNotFoundError(id)));
        }

        let mut missing: HashSet<Pointee> = HashSet::new();
        for p in &m {
            if !self.is_pointee_exist(p) {
                missing.insert(p.clone());
            }
        }
        if !missing.is_empty() {
            return Err(AddHyperedgeMembers::PointeesNotFound(PointeesNotFound(
                missing,
            )));
        }

        let existing = self.hyper_edge.get(&id).expect("checked above");
        let duplicates: Vec<Pointee> =
            m.iter().filter(|p| existing.contains(*p)).cloned().collect();
        if !duplicates.is_empty() {
            return Err(AddHyperedgeMembers::MembersAlreadyExist(
                MembersAlreadyExist(duplicates),
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
    ) -> Result<(), AddHyperedgeMembers> {
        self.silent_add_hyperedge_members(id, m.clone())?;
        self.record_patch(Patch::AddElementsToHyperEdge { id, members: m });
        Ok(())
    }

    fn silent_remove_hyperedge_members(
        &mut self,
        id: HyperEdgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), RemoveHyperedgeMembers> {
        todo!()
    }

    fn remove_hyperedge_members(
        &mut self,
        id: HyperEdgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), RemoveHyperedgeMembers> {
        todo!()
    }

    pub fn replace_node(&mut self, id: &NodeId, obj: Object) -> Result<Field, NodeNotFoundError> {
        todo!()
    }

    pub fn replace_attached_obj(
        &mut self,
        id: &AttachTargetID,
        obj: Object,
    ) -> Result<(), EdgeNotFoundError> {
        todo!()
    }

    fn silent_retraget_edge(
        &mut self,
        id: &Uuid,
        new_target: RetrargetEdge,
    ) -> Result<(), RetargetError> {
        todo!()
    }

    pub fn retraget_edge(
        &mut self,
        id: &Uuid,
        new_target: RetrargetEdge,
    ) -> Result<(), RetargetError> {
        todo!()
    }

    /// Apply patch to node or attached object
    fn obj_apply_patch(&mut self, id: Uuid, patch: Vec<ObjectPatch>) -> Result<(), DeltaError> {
        todo!()
    }

    /// Apply a single [`Patch`] to this graph.
    ///
    pub(crate) fn apply_patch(&mut self, delta: Delta) -> Result<(), ApplyPatchError> {
        for patch in delta {
            match patch {
                Patch::AddNode { id, obj } => {
                    self.silent_add_node_with_id(id, obj)
                        .map_err(ApplyPatchError::NodeAlreadyExists)?;
                }
                Patch::RemoveNode { id } => {
                    self.silent_remove_node(&id)
                        .map_err(ApplyPatchError::NodeNotFound)?;
                }
                Patch::ChangeNode { id, delta } => {
                    self.obj_apply_patch(id, delta)
                        .map_err(ApplyPatchError::Change)?;
                }
                Patch::UpsertNode { id, obj } => todo!(),
                Patch::AddEdge { id, source, target } => {
                    self.silent_add_edge_with_id(id, source, target)
                        .map_err(ApplyPatchError::AddEdge)?;
                }
                Patch::RemoveEdge { id } => {
                    self.silent_remove_edge(&id)
                        .map_err(ApplyPatchError::EdgeNotFound)?;
                }
                Patch::RetrargetEdge { id, new_target } => {
                    self.silent_retraget_edge(&id, new_target)
                        .map_err(ApplyPatchError::Retarget)?;
                }
                Patch::UpsertEdgeData { id, obj } => todo!(),
                Patch::ChangeEdgeData { id, delta } => {
                    self.obj_apply_patch(id, delta)
                        .map_err(ApplyPatchError::Change)?;
                }
                Patch::RemoveEdgeData { id } => {
                    self.silent_remove_attached(id)
                        .map_err(ApplyPatchError::RemoveAttached)?;
                }
                Patch::CreateHyperEdge { id, members } => {
                    self.silent_create_hyperedge_with_id(&id, members)
                        .map_err(ApplyPatchError::CreateHyperEdgeError)?;
                }
                Patch::RemoveHyperEdge { id } => {
                    self.silent_remove_hyperedge(&id)
                        .map_err(ApplyPatchError::HyperEdgeNotFound)?;
                }
                Patch::AddElementsToHyperEdge { id, members } => {
                    self.silent_add_hyperedge_members(id, members)
                        .map_err(ApplyPatchError::AddHyperedgeMembers)?;
                }
                Patch::RemoveElementsFromHyperEdge { id, members } => {
                    self.silent_remove_hyperedge_members(id, members)
                        .map_err(ApplyPatchError::RemoveHyperedgeMembers)?;
                }
                Patch::UpsertHyperEdgeData { id, obj } => todo!(),
                Patch::ChangeHyperEdgeData { id, delta } => {
                    self.obj_apply_patch(id, delta)
                        .map_err(ApplyPatchError::Change)?;
                }
                Patch::RemoveHyperEdgeData { id } => {
                    self.silent_remove_attached(id)
                        .map_err(ApplyPatchError::RemoveAttached)?;
                }
            }
        }

        Ok(())
    }

    /* ------------ END MODIFIERS ------------- */

    /* ------------ START LISTENERS ----------- */

    fn record_patch(&mut self, patch: Patch) {
        self.events.push(patch);
    }

    pub fn subscribe_on_change(&mut self, listener: Box<dyn Fn(Patch)>) -> ListernerID {
        todo!()
    }

    pub fn unsubscribe_on_change(&mut self, id: ListernerID) {
        todo!()
    }

    /* ------------ END LISTENERS ------------- */
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) mod test_utils {
        use super::*;

        pub fn create_simple_obj(field_name: &str) -> Object {
            let mut obj = Object::new();
            obj.insert(field_name.into(), Field::Null);
            obj
        }

        /// Cross-check `pointee_uses` and `entity_to_path_pointees`
        /// against `edges` / `hyper_edge`. Panics on the first
        /// inconsistency. Call after any mutation to assert the
        /// reverse-index invariants are intact.
        pub fn check_index_invariant(g: &Graph) {
            // 1) Every Pointee::Path key in pointee_uses must be
            //    tracked in entity_to_path_pointees under its entity.
            for key in g.pointee_uses.keys() {
                if let Pointee::Path(gp) = key {
                    let tracked = g
                        .entity_to_path_pointees
                        .get(&gp.entity())
                        .is_some_and(|s| s.contains(key));
                    assert!(
                        tracked,
                        "path pointee {:?} present in pointee_uses but missing \
                         from entity_to_path_pointees",
                        key
                    );
                }
            }

            // 2) entity_to_path_pointees has no stale or empty entries.
            for (entity, paths) in &g.entity_to_path_pointees {
                assert!(
                    !paths.is_empty(),
                    "entity_to_path_pointees[{}] is empty (should have been removed)",
                    entity
                );
                for p in paths {
                    assert!(
                        g.pointee_uses.contains_key(p),
                        "stale entry in entity_to_path_pointees[{}]: {:?} not in pointee_uses",
                        entity,
                        p
                    );
                }
            }

            // 3) Every edge has both endpoints registered in pointee_uses.
            for (eid, (src, tgt)) in &g.edges {
                let src_ok = g
                    .pointee_uses
                    .get(src)
                    .is_some_and(|b| b.edges_as_source.contains(eid));
                assert!(src_ok, "edge {} not registered in source bucket {:?}", eid, src);
                let tgt_ok = g
                    .pointee_uses
                    .get(tgt)
                    .is_some_and(|b| b.edges_as_target.contains(eid));
                assert!(tgt_ok, "edge {} not registered in target bucket {:?}", eid, tgt);
            }

            // 4) Every hyperedge member has the hyperedge registered.
            for (hid, members) in &g.hyper_edge {
                for m in members {
                    let ok = g
                        .pointee_uses
                        .get(m)
                        .is_some_and(|b| b.hyperedges.contains(hid));
                    assert!(ok, "hyperedge {} not registered in member bucket {:?}", hid, m);
                }
            }

            // 5) No empty buckets — they must have been removed.
            for (k, b) in &g.pointee_uses {
                assert!(
                    !b.is_empty(),
                    "pointee_uses[{:?}] is empty (should have been removed)",
                    k
                );
            }

            // 6) Reverse direction: every (eid, source/target/hyperedge)
            //    in pointee_uses corresponds to a live structural element.
            for (pointee, uses) in &g.pointee_uses {
                for eid in &uses.edges_as_source {
                    let edge = g.edges.get(eid);
                    assert!(
                        edge.is_some_and(|(s, _)| s == pointee),
                        "pointee_uses[{:?}].edges_as_source has stale eid {}",
                        pointee,
                        eid
                    );
                }
                for eid in &uses.edges_as_target {
                    let edge = g.edges.get(eid);
                    assert!(
                        edge.is_some_and(|(_, t)| t == pointee),
                        "pointee_uses[{:?}].edges_as_target has stale eid {}",
                        pointee,
                        eid
                    );
                }
                for hid in &uses.hyperedges {
                    let members = g.hyper_edge.get(hid);
                    assert!(
                        members.is_some_and(|ms| ms.contains(pointee)),
                        "pointee_uses[{:?}].hyperedges has stale hid {}",
                        pointee,
                        hid
                    );
                }
            }
        }

        // Built graph:
        // ```text
        //  ---------
        //  | n1 ---|----e1---n2
        //  |       |    |    |
        //  |       |    |    |
        //  |       |    e3---e4-----
        //  |       |    |          |
        //  | n3 ---|---e2----n4    |
        //  --h------               |
        //    |                     |
        //    |----------------------
        // ````
        pub fn create_semple_graph1() -> (
            Graph,
            NodeId,
            NodeId,
            NodeId,
            NodeId,
            EdgeID,
            EdgeID,
            EdgeID,
            EdgeID,
            EdgeID,
            HyperEdgeId,
        ) {
            let mut g = Graph::default();
            let obj = create_simple_obj("test_field");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());
            let n3 = g.add_node(obj.clone());
            let n4 = g.add_node(obj.clone());

            let e1 = g.add_edge(n1, n2).unwrap();
            let e2 = g.add_edge(n3, n4).unwrap();
            let e3 = g.add_edge(e1, e2).unwrap();
            let e4 = g.add_edge(e3, n2).unwrap();

            let mut m = HashSet::new();
            m.insert(n1.into());
            m.insert(n3.into());

            let h = g.create_hyperedge(m).unwrap();
            let e5 = g.add_edge(h, e4).unwrap();

            (g, n1, n2, n3, n4, e1, e2, e3, e4, e5, h)
        }

        // Built graph:
        // ```text
        //  n1 ---- e_a ----- n2
        //  |\
        //  |  ----------------
        //  |                 |
        //  edge_to_h     meta_edge
        //  |                 |
        //  -------           |     --------
        //  |  n3-|----------e_b----|---n4 |
        //  |     |-----------------|      |
        //  |                              |
        //  |-----------h------------------|
        // ```
        pub fn create_semple_graph2() -> (
            Graph,
            NodeId,
            NodeId,
            NodeId,
            NodeId,
            EdgeID,
            EdgeID,
            EdgeID,
            EdgeID,
            HyperEdgeId,
        ) {
            let mut graph = Graph::default();
            let obj = test_utils::create_simple_obj("test_field");

            let n1 = graph.add_node(obj.clone());
            let n2 = graph.add_node(obj.clone());
            let n3 = graph.add_node(obj.clone());
            let n4 = graph.add_node(obj.clone());

            let e_a = graph.add_edge(n1, n2).unwrap();
            let e_b = graph.add_edge(n3, n4).unwrap();
            let meta_edge = graph.add_edge(n1, e_b).unwrap();

            let mut m = HashSet::new();
            m.insert(n3.into());
            m.insert(n4.into());

            let h = graph.create_hyperedge(m).unwrap();
            let edge_to_h = graph.add_edge(n1, h).unwrap();

            (graph, n1, n2, n3, n4, e_a, e_b, meta_edge, edge_to_h, h)
        }

        /// Built graph (note: `e2` is intentionally directed `n3 → n1`,
        /// not `n1 → n3`):
        ///
        /// ```text
        ///
        ///  n1 ----------- e1 ---------- n2
        ///   ^                          / |
        ///    \         /----- e3 -----   |
        ///     -- e2 - n3 -------- e4 ----
        /// ```
        pub fn create_semple_graph3() -> (
            Graph,
            NodeId,
            NodeId,
            NodeId,
            EdgeID,
            EdgeID,
            EdgeID,
            EdgeID,
        ) {
            let mut graph = Graph::default();
            let obj = test_utils::create_simple_obj("attached");

            let n1 = graph.add_node(obj.clone());
            let n2 = graph.add_node(obj.clone());
            let n3 = graph.add_node(obj.clone());

            let e1 = graph.add_edge(n1, n2).unwrap();
            let e2 = graph.add_edge(n3, n1).unwrap(); // reversed on purpose
            let e3 = graph.add_edge(n3, n2).unwrap();
            let e4 = graph.add_edge(n3, n2).unwrap(); // parallel to e3

            (graph, n1, n2, n3, e1, e2, e3, e4)
        }
    }

    mod test_globals {
        use super::*;

        mod test_iter_entities {
            use super::*;

            /// Simple case we just check that all
            /// kind of entities iterator yeld
            #[test]
            fn test_iter_entities1() {
                let (g, n1, n2, n3, n4, e1, e2, e3, e4, e5, h) = test_utils::create_semple_graph1();

                let mut expected = HashSet::new();
                expected.insert(n1);
                expected.insert(n2);
                expected.insert(n3);
                expected.insert(n4);
                expected.insert(e1);
                expected.insert(e2);
                expected.insert(e3);
                expected.insert(e4);
                expected.insert(e5);
                expected.insert(h);

                let actual: Vec<_> = g.iter_entities().collect();

                // 1. No duplicates: every entity appears at most once.
                let mut counts: HashMap<EntityId, usize> = HashMap::new();
                for e in &actual {
                    *counts.entry(*e).or_insert(0) += 1;
                }
                let duplicates: Vec<_> = counts
                    .iter()
                    .filter(|(_, c)| **c > 1)
                    .map(|(e, c)| (*e, *c))
                    .collect();
                if !duplicates.is_empty() {
                    panic!("global_entities returned duplicates: {:?}", duplicates);
                }

                // 2. Coverage matches exactly: no missing, no extras.
                let actual_set: HashSet<_> = actual.iter().copied().collect();
                let missing: Vec<_> = expected.difference(&actual_set).copied().collect();
                let unexpected: Vec<_> = actual_set.difference(&expected).copied().collect();
                if !missing.is_empty() || !unexpected.is_empty() {
                    panic!(
                        "global_entities mismatch — missing: {:?}, unexpected: {:?}",
                        missing, unexpected
                    );
                }
            }

            /// Verify deduplication: an id that lives in more than one
            /// storage map must be yielded by `iter_entities` only
            /// once.
            ///
            /// Setup creates two cross-map collisions on purpose:
            /// - `e1` lives as an edge *and*, after `attach_obj`, as
            ///   an entity (object attached to that edge).
            /// - `h`  lives as a hyperedge *and*, after `attach_obj`,
            ///   as an entity.
            ///
            /// If `iter_entities` ever stops deduplicating (e.g.
            /// someone removes the trailing `collect::<HashSet<_>>()`),
            /// `e1` and `h` would each show up twice and this test
            /// catches it.
            #[test]
            fn test_iter_entities2() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());

                let e1 = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e1, obj.clone()).unwrap();

                let mut m = HashSet::new();
                m.insert(n1.into());
                m.insert(n2.into());

                let h = g.create_hyperedge(m).unwrap();
                g.attach_obj(h, obj.clone()).unwrap();

                let actual: Vec<_> = g.iter_entities().collect();

                // 1. No id appears more than once — in particular,
                //    e1 and h, which sit in two maps each.
                let mut counts: HashMap<EntityId, usize> = HashMap::new();
                for id in &actual {
                    *counts.entry(*id).or_insert(0) += 1;
                }
                assert_eq!(
                    counts.get(&e1).copied(),
                    Some(1),
                    "e1 yielded {:?} times, expected 1",
                    counts.get(&e1)
                );
                assert_eq!(
                    counts.get(&h).copied(),
                    Some(1),
                    "h yielded {:?} times, expected 1",
                    counts.get(&h)
                );

                // 2. Coverage: exactly the four distinct ids.
                let actual_set: HashSet<_> = actual.iter().copied().collect();
                let expected: HashSet<_> = [n1, n2, e1, h].into_iter().collect();
                assert_eq!(actual_set, expected);
            }
        }

        mod test_iter_edges {
            use super::*;

            #[test]
            fn iter_edges1() {
                let (g, _n1, _n2, _n3, _n4, e1, e2, e3, e4, e5, _h) =
                    test_utils::create_semple_graph1();

                let actual: Vec<_> = g.iter_edges().collect();

                // No duplicates (HashMap keys can't repeat, but we
                // assert anyway in case the impl changes).
                let mut counts: HashMap<EdgeID, usize> = HashMap::new();
                for id in &actual {
                    *counts.entry(*id).or_insert(0) += 1;
                }
                let duplicates: Vec<_> = counts
                    .iter()
                    .filter(|(_, c)| **c > 1)
                    .map(|(e, c)| (*e, *c))
                    .collect();
                if !duplicates.is_empty() {
                    panic!("iter_edges returned duplicates: {:?}", duplicates);
                }

                // Exactly the five edges added (e1..e5). Nodes and
                // the hyperedge `h` must NOT appear.
                let actual_set: HashSet<_> = actual.iter().copied().collect();
                let expected: HashSet<_> = [e1, e2, e3, e4, e5].into_iter().collect();
                assert_eq!(actual_set, expected);
            }
        }

        mod test_iter_nodes {
            use super::*;

            #[test]
            fn iter_nodes() {
                let (g, n1, n2, n3, n4, _e1, _e2, _e3, _e4, _e5, _h) =
                    test_utils::create_semple_graph1();

                let actual: Vec<_> = g.iter_nodes().collect();

                // No duplicates — entities is a HashMap, but assert
                // anyway in case the impl chains in extra sources later.
                let mut counts: HashMap<NodeId, usize> = HashMap::new();
                for id in &actual {
                    *counts.entry(*id).or_insert(0) += 1;
                }
                let duplicates: Vec<_> = counts
                    .iter()
                    .filter(|(_, c)| **c > 1)
                    .map(|(e, c)| (*e, *c))
                    .collect();
                if !duplicates.is_empty() {
                    panic!("iter_nodes returned duplicates: {:?}", duplicates);
                }

                // Exactly the four node ids — edges (e1..e5) and the
                // hyperedge `h` must NOT appear.
                let actual_set: HashSet<_> = actual.iter().copied().collect();
                let expected: HashSet<_> = [n1, n2, n3, n4].into_iter().collect();
                assert_eq!(actual_set, expected);
            }
        }

        mod test_iter_hyperedge {
            use super::*;

            #[test]
            fn test_iter_hyperedge() {
                let (g, _n1, _n2, _n3, _n4, _e1, _e2, _e3, _e4, _e5, h) =
                    test_utils::create_semple_graph1();

                let actual: Vec<_> = g.iter_hyperedge().collect();

                // No duplicates — `hyper_edge` is a HashMap, but
                // assert anyway in case the impl chains in extra
                // sources later.
                let mut counts: HashMap<HyperEdgeId, usize> = HashMap::new();
                for id in &actual {
                    *counts.entry(*id).or_insert(0) += 1;
                }
                let duplicates: Vec<_> = counts
                    .iter()
                    .filter(|(_, c)| **c > 1)
                    .map(|(e, c)| (*e, *c))
                    .collect();
                if !duplicates.is_empty() {
                    panic!("iter_hyperedge returned duplicates: {:?}", duplicates);
                }

                // Exactly the one hyperedge — nodes and edges must
                // NOT appear.
                let actual_set: HashSet<_> = actual.iter().copied().collect();
                let expected: HashSet<_> = [h].into_iter().collect();
                assert_eq!(actual_set, expected);
            }
        }

        mod test_iter_attached {
            use super::*;

            /// No attached objects yet — only nodes live in
            /// `entities`, so `iter_attached` is empty.
            #[test]
            fn empty_when_only_nodes() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                g.add_node(obj.clone());
                g.add_node(obj);
                assert_eq!(g.iter_attached().count(), 0);
            }

            /// Yields exactly the ids on which `attach_obj` placed an
            /// object — both edges and hyperedges qualify.
            #[test]
            fn yields_attach_targets() {
                let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_semple_graph2();
                let obj = test_utils::create_simple_obj("attached");

                g.attach_obj(e_a, obj.clone()).unwrap();
                g.attach_obj(h, obj).unwrap();

                let actual: HashSet<_> = g.iter_attached().collect();
                let expected: HashSet<_> = [e_a, h].into_iter().collect();
                assert_eq!(actual, expected);
            }

            /// Complement of `iter_nodes`: a node id, even if it has
            /// an object, must NOT appear in `iter_attached`.
            #[test]
            fn excludes_nodes() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj);
                let actual: HashSet<_> = g.iter_attached().collect();
                assert!(!actual.contains(&n1));
            }
        }
    }

    mod test_getters {
        use super::*;

        mod test_obj {
            use super::*;

            // Test all kind of object holder: an object can be
            // attached to a regular edge, a meta-edge, and an
            // edge-to-hyperedge alike. Hitting all three structural
            // shapes here exercises every `is_attach_target() == true`
            // branch of `EntityType`.
            //
            // n1 is used to verify that nodes — the "default" holder
            // (object goes in via `add_node`) — keep their object
            // unchanged after these attaches.
            #[test]
            fn test_obj() {
                let (mut graph, n1, _n2, _n3, _n4, e_a, _e_b, meta_edge, edge_to_h, _h) =
                    test_utils::create_semple_graph2();

                let obj = test_utils::create_simple_obj("attached");

                graph.attach_obj(e_a, obj.clone()).unwrap();
                graph.attach_obj(meta_edge, obj.clone()).unwrap();
                graph.attach_obj(edge_to_h, obj.clone()).unwrap();

                assert_eq!(graph.obj(&e_a), Some(&obj));
                assert_eq!(graph.obj(&meta_edge), Some(&obj));
                assert_eq!(graph.obj(&edge_to_h), Some(&obj));

                // n1 is a Node: its object came from `add_node`,
                // independent of any attach. It must still be there.
                assert!(graph.obj(&n1).is_some());
            }

            /// Unknown id resolves to `None`.
            #[test]
            fn obj_unknown() {
                let g = Graph::default();
                assert!(g.obj(&Uuid::new_v4()).is_none());
            }

            /// A bare edge (no `attach_obj` call) has no object.
            #[test]
            fn obj_bare_edge_is_none() {
                let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                    test_utils::create_semple_graph2();
                assert!(g.obj(&e_a).is_none());
            }

            /// A bare hyperedge has no object.
            #[test]
            fn obj_bare_hyperedge_is_none() {
                let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_semple_graph2();
                assert!(g.obj(&h).is_none());
            }
        }

        mod test_edge {
            use super::*;

            /// Regular node-to-node edge round-trips through `edge`.
            #[test]
            fn edge1() {
                let (graph, n1, n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                    test_utils::create_semple_graph2();

                let u = graph.edge(&e_a).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: e_a,
                        source: n1.into(),
                        target: n2.into()
                    }
                )
            }

            /// Meta-edge: target is another edge — still in the
            /// `edges` map, so `edge` returns its triplet.
            #[test]
            fn edge2() {
                let (graph, n1, _n2, _n3, _n4, _e_a, e_b, meta_edge, _edge_to_h, _h) =
                    test_utils::create_semple_graph2();

                let u = graph.edge(&meta_edge).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: meta_edge,
                        source: n1.into(),
                        target: e_b.into(),
                    }
                )
            }

            /// Edge whose target is a hyperedge — also lives in the
            /// `edges` map.
            #[test]
            fn edge3() {
                let (graph, n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, edge_to_h, h) =
                    test_utils::create_semple_graph2();

                let u = graph.edge(&edge_to_h).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: edge_to_h,
                        source: n1.into(),
                        target: h.into(),
                    }
                )
            }

            /// Self-loop is a valid edge.
            #[test]
            fn edge4() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj);
                let e1 = g.add_edge(n1, n1).unwrap();

                let u = g.edge(&e1).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: e1,
                        source: n1.into(),
                        target: n1.into(),
                    }
                )
            }

            /// Endpoints can be sub-object paths; `edge` returns
            /// them verbatim.
            #[test]
            fn edge5() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let p1 = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
                let p2 = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let e1 = g.add_edge(p1.clone(), p2.clone()).unwrap();

                let u = g.edge(&e1).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: e1,
                        source: p1,
                        target: p2,
                    }
                )
            }

            /// Unknown id → `NotFound`.
            #[test]
            fn edge_not_found() {
                let g = Graph::default();
                let id = Uuid::new_v4();
                let err = g.edge(&id).unwrap_err();
                assert!(matches!(
                    err,
                    GetEdgeError::NotFound(EntityNotFoundError(x)) if x == id
                ));
            }

            /// A node id is a known entity but not an edge →
            /// `IncorrectType("Node")`.
            #[test]
            fn edge_incorrect_type_node() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj);

                let err = g.edge(&n1).unwrap_err();
                match err {
                    GetEdgeError::IncorrectType(e) => {
                        assert_eq!(e.node_id, n1);
                        assert_eq!(e.actual_type, "Node");
                    }
                    other => panic!("expected IncorrectType, got {other:?}"),
                }
            }

            /// A hyperedge id → `IncorrectType("HyperEdge")`.
            #[test]
            fn edge_incorrect_type_hyperedge() {
                let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_semple_graph2();

                let err = g.edge(&h).unwrap_err();
                match err {
                    GetEdgeError::IncorrectType(e) => {
                        assert_eq!(e.node_id, h);
                        assert_eq!(e.actual_type, "HyperEdge");
                    }
                    other => panic!("expected IncorrectType, got {other:?}"),
                }
            }

            /// An attached-object id (object placed on top of a
            /// hyperedge) → `IncorrectType("AttachedObject")`. The
            /// hyperedge map lookup wins over the edges map only
            /// because attaching to a hyperedge keeps the id outside
            /// `edges`.
            #[test]
            fn edge_incorrect_type_attached() {
                let (mut g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_semple_graph2();
                let obj = test_utils::create_simple_obj("attached");
                g.attach_obj(h, obj).unwrap();

                let err = g.edge(&h).unwrap_err();
                match err {
                    GetEdgeError::IncorrectType(e) => {
                        assert_eq!(e.node_id, h);
                        assert_eq!(e.actual_type, "AttachedObject");
                    }
                    other => panic!("expected IncorrectType, got {other:?}"),
                }
            }
        }

        mod test_hyperedge {
            use super::*;

            #[test]
            fn hyperedge1() {
                let (graph, _n1, _n2, n3, n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_semple_graph2();

                let members = graph.hyperedge_members(&h).unwrap();
                let expected: HashSet<Pointee> = [n3.into(), n4.into()].into_iter().collect();
                assert_eq!(members, &expected);
            }

            /// Unknown id → None.
            #[test]
            fn hyperedge_unknown() {
                let g = Graph::default();
                assert!(g.hyperedge_members(&Uuid::new_v4()).is_none());
            }

            /// An edge id is not a hyperedge → None.
            #[test]
            fn hyperedge_for_edge_id() {
                let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                    test_utils::create_semple_graph2();
                assert!(g.hyperedge_members(&e_a).is_none());
            }

            /// `hyperedge_members` returns `None` for an unknown id.
            #[test]
            fn hyperedge_members_unknown() {
                let g = Graph::default();
                assert!(g.hyperedge_members(&Uuid::new_v4()).is_none());
            }
        }

        mod test_probs {
            use super::*;

            mod test_get_type {
                use super::*;

                /// Unknown id resolves to `None`.
                #[test]
                fn unknown_is_none() {
                    let g = Graph::default();
                    assert!(g.get_type(Uuid::new_v4()).is_none());
                }

                /// Pure node — only in `entities`.
                #[test]
                fn node() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    assert!(matches!(g.get_type(n1), Some(EntityType::Node)));
                }

                /// Regular edge — both endpoints are nodes.
                #[test]
                fn edge() {
                    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_semple_graph2();
                    assert!(matches!(g.get_type(e_a), Some(EntityType::Edge)));
                }

                /// Edge whose endpoint is another edge → MetaEdge.
                #[test]
                fn meta_edge_with_edge_endpoint() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, meta_edge, _edge_to_h, _h) =
                        test_utils::create_semple_graph2();
                    assert!(matches!(g.get_type(meta_edge), Some(EntityType::MetaEdge)));
                }

                /// Edge whose endpoint is a hyperedge → MetaEdge.
                #[test]
                fn meta_edge_with_hyperedge_endpoint() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, edge_to_h, _h) =
                        test_utils::create_semple_graph2();
                    assert!(matches!(g.get_type(edge_to_h), Some(EntityType::MetaEdge)));
                }

                /// Pure hyperedge — only in `hyper_edge`.
                #[test]
                fn hyperedge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                        test_utils::create_semple_graph2();
                    assert!(matches!(g.get_type(h), Some(EntityType::HyperEdge)));
                }

                /// Object attached on top of an edge — id collides
                /// in both `entities` and `edges` → AttachedObject.
                #[test]
                fn attached_on_edge() {
                    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_semple_graph2();
                    let obj = test_utils::create_simple_obj("attached");
                    g.attach_obj(e_a, obj).unwrap();
                    assert!(matches!(g.get_type(e_a), Some(EntityType::AttachedObject)));
                }

                /// Object attached on top of a hyperedge — id
                /// collides in both `entities` and `hyper_edge` →
                /// AttachedObject.
                #[test]
                fn attached_on_hyperedge() {
                    let (mut g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                        test_utils::create_semple_graph2();
                    let obj = test_utils::create_simple_obj("attached");
                    g.attach_obj(h, obj).unwrap();
                    assert!(matches!(g.get_type(h), Some(EntityType::AttachedObject)));
                }

                /// `Pointee::Path` endpoint must NOT promote an edge
                /// to MetaEdge — only edge/hyperedge endpoints do.
                #[test]
                fn path_endpoint_stays_edge() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    let n2 = g.add_node(obj);

                    let p1 = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
                    let e1 = g.add_edge(p1, n2).unwrap();
                    assert!(matches!(g.get_type(e1), Some(EntityType::Edge)));
                }
            }

            mod test_classify_pointee {
                use super::*;

                #[test]
                fn entity_node() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    assert_eq!(g.classify_pointee(&n1.into()), Some(PointeeKind::Node));
                }

                #[test]
                fn entity_edge() {
                    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_semple_graph2();
                    assert_eq!(g.classify_pointee(&e_a.into()), Some(PointeeKind::Edge));
                }

                #[test]
                fn entity_hyperedge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                        test_utils::create_semple_graph2();
                    assert_eq!(g.classify_pointee(&h.into()), Some(PointeeKind::HyperEdge));
                }

                #[test]
                fn entity_meta_edge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, meta_edge, _edge_to_h, _h) =
                        test_utils::create_semple_graph2();
                    assert_eq!(
                        g.classify_pointee(&meta_edge.into()),
                        Some(PointeeKind::MetaEdge)
                    );
                }

                #[test]
                fn entity_attached() {
                    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_semple_graph2();
                    let obj = test_utils::create_simple_obj("attached");
                    g.attach_obj(e_a, obj).unwrap();
                    assert_eq!(
                        g.classify_pointee(&e_a.into()),
                        Some(PointeeKind::AttachedObject)
                    );
                }

                #[test]
                fn entity_unknown() {
                    let g = Graph::default();
                    let unknown: Pointee = Uuid::new_v4().into();
                    assert_eq!(g.classify_pointee(&unknown), None);
                }

                /// Path that resolves to a real field → Subobject.
                #[test]
                fn path_resolves() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    let p = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
                    assert_eq!(g.classify_pointee(&p), Some(PointeeKind::Subobject));
                }

                /// Path whose entity isn't in the graph → None.
                #[test]
                fn path_unknown_entity() {
                    let g = Graph::default();
                    let p = Pointee::Path(GlobalObjPath::new(Uuid::new_v4(), "x").unwrap());
                    assert_eq!(g.classify_pointee(&p), None);
                }

                /// Path whose entity exists but the field is missing
                /// → None.
                #[test]
                fn path_missing_field() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    let p = Pointee::Path(GlobalObjPath::new(n1, "no_such_field").unwrap());
                    assert_eq!(g.classify_pointee(&p), None);
                }
            }

            mod test_is_exist {
                use super::*;
                /// Test exist node
                #[test]
                fn test_is_exist1() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    assert!(g.is_exist(&n1))
                }

                /// Test exist edge
                #[test]
                fn test_is_exist2() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    let n2 = g.add_node(obj);
                    let e1 = g.add_edge(n1, n2).unwrap();
                    assert!(g.is_exist(&e1))
                }

                /// Test exist hyperedge
                #[test]
                fn test_is_exist3() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    let n2 = g.add_node(obj);
                    let mut m = HashSet::new();
                    m.insert(n1.into());
                    m.insert(n2.into());
                    let h = g.create_hyperedge(m).unwrap();
                    assert!(g.is_exist(&h))
                }

                /// Test exist metaedge (an edge whose endpoint is another edge)
                #[test]
                fn test_is_exist4() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    let n2 = g.add_node(obj);
                    let e1 = g.add_edge(n1, n2).unwrap();
                    let meta_edge = g.add_edge(n1, e1).unwrap();
                    assert!(g.is_exist(&meta_edge))
                }
            }

            mod test_is_pointee_exist {
                use super::*;

                #[test]
                fn entity_node() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    assert!(g.is_pointee_exist(&n1.into()));
                }

                #[test]
                fn entity_edge() {
                    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_semple_graph2();
                    assert!(g.is_pointee_exist(&e_a.into()));
                }

                #[test]
                fn entity_hyperedge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                        test_utils::create_semple_graph2();
                    assert!(g.is_pointee_exist(&h.into()));
                }

                #[test]
                fn entity_meta_edge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, meta_edge, _edge_to_h, _h) =
                        test_utils::create_semple_graph2();
                    assert!(g.is_pointee_exist(&meta_edge.into()));
                }

                #[test]
                fn entity_attached() {
                    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_semple_graph2();
                    let obj = test_utils::create_simple_obj("attached");
                    g.attach_obj(e_a, obj).unwrap();
                    assert!(g.is_pointee_exist(&e_a.into()));
                }

                #[test]
                fn entity_unknown() {
                    let g = Graph::default();
                    let unknown: Pointee = Uuid::new_v4().into();
                    assert!(!g.is_pointee_exist(&unknown));
                }

                /// Path resolves to a real top-level field.
                #[test]
                fn path_resolves() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    let p = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
                    assert!(g.is_pointee_exist(&p));
                }

                /// Path descends through a nested `Field::Object`.
                #[test]
                fn path_resolves_nested() {
                    let mut g = Graph::default();
                    let mut inner = Object::new();
                    inner.insert("leaf".into(), Field::Null);
                    let mut obj = Object::new();
                    obj.insert("nested".into(), Field::Object(inner));
                    let n1 = g.add_node(obj);

                    let mut path = GlobalObjPath::new(n1, "nested").unwrap();
                    path.push("leaf").unwrap();
                    let p = Pointee::Path(path);
                    assert!(g.is_pointee_exist(&p));
                }

                /// Entity isn't in the graph.
                #[test]
                fn path_unknown_entity() {
                    let g = Graph::default();
                    let p = Pointee::Path(GlobalObjPath::new(Uuid::new_v4(), "x").unwrap());
                    assert!(!g.is_pointee_exist(&p));
                }

                /// Entity exists but the first segment doesn't match
                /// any top-level field.
                #[test]
                fn path_missing_field() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    let p = Pointee::Path(GlobalObjPath::new(n1, "no_such_field").unwrap());
                    assert!(!g.is_pointee_exist(&p));
                }

                /// Walking a path through a non-Object field
                /// (e.g. `Field::Null`) must fail.
                #[test]
                fn path_through_non_object_fails() {
                    let mut g = Graph::default();
                    // "test_field" is a Field::Null — it is not an
                    // Object, so any further descent must fail.
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);

                    let mut path = GlobalObjPath::new(n1, "test_field").unwrap();
                    path.push("anything").unwrap();
                    let p = Pointee::Path(path);
                    assert!(!g.is_pointee_exist(&p));
                }

                /// Edge id with a sub-path doesn't navigate — only
                /// `entities` is consulted, and a bare edge has no
                /// attached object.
                #[test]
                fn path_on_bare_edge_fails() {
                    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_semple_graph2();
                    let p = Pointee::Path(GlobalObjPath::new(e_a, "x").unwrap());
                    assert!(!g.is_pointee_exist(&p));
                }
            }
        }
    }

    mod test_constructors {
        use super::*;

        mod test_create_hyperedge {
            use super::*;

            /// Create a hyperedge with two members and verify
            /// both id presence and members round-trip.
            #[test]
            fn create_hyperedge1() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut members = HashSet::new();
                members.insert(n1.into());
                members.insert(n2.into());

                let h = g.create_hyperedge(members.clone()).unwrap();
                assert!(g.is_exist(&h));
                assert_eq!(g.hyperedge_members(&h), Some(&members));
            }

            /// An empty member set is rejected — every hyperedge
            /// must have at least one member.
            #[test]
            fn create_hyperedge_empty_rejected() {
                let mut g = Graph::default();
                let err = g.create_hyperedge(HashSet::new()).unwrap_err();
                assert_eq!(err, CreateHyperEdgeError::EmptyHyperEdge);
            }

            /// Members may include other hyperedges (nesting) and
            /// edge ids — `create_hyperedge` doesn't validate
            /// membership shape.
            #[test]
            fn create_hyperedge_with_edge_and_hyperedge_members() {
                let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_semple_graph2();

                let mut members = HashSet::new();
                members.insert(e_a.into());
                members.insert(h.into());

                let h2 = g.create_hyperedge(members.clone()).unwrap();
                assert_eq!(g.hyperedge_members(&h2), Some(&members));
            }

            /// `__create_hyperedge_with_id` rejects a duplicate id.
            #[test]
            fn create_hyperedge_already_exists() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let mut members = HashSet::new();
                members.insert(n1.into());
                let h = g.create_hyperedge(members.clone()).unwrap();
                let err = g
                    .silent_create_hyperedge_with_id(&h, members)
                    .unwrap_err();
                assert_eq!(
                    err,
                    CreateHyperEdgeError::HyperEdgeAlreadyExists(HyperEdgeAlreadyExistsError(h))
                );
            }

            /// Re-inserting with the same id must NOT clobber the
            /// existing members — the original hyperedge stays
            /// intact.
            #[test]
            fn create_hyperedge_already_exists_preserves_members() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                // Try to overwrite with a different (non-empty) member set.
                let mut other = HashSet::new();
                other.insert(n2.into());
                let _ = g.silent_create_hyperedge_with_id(&h, other);

                assert_eq!(g.hyperedge_members(&h), Some(&original));
            }
        }

        mod test_add_node {
            use super::*;

            /// Simple case we add node and check that's is exist
            #[test]
            fn test_add_node1() {
                let mut graph = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let node_id = graph.add_node(obj.clone());
                assert_eq!(graph.obj(&node_id), Some(&obj));
            }

            /// Check error node elready exist
            #[test]
            fn test_add_node2() {
                let mut graph = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = graph.add_node(obj.clone());
                let obj2 = test_utils::create_simple_obj("test_field2");
                let result2 = graph.silent_add_node_with_id(n1, obj2.clone());
                assert_eq!(
                    result2.clone().unwrap_err(),
                    NodeAlreadyExistsError(result2.unwrap_err().0)
                );
                // Check thats change doesnt apply
                assert_eq!(graph.obj(&n1), Some(&obj))
            }
        }

        mod test_add_edge {
            use super::*;

            /// Create simple edge
            #[test]
            fn test_add_edge1() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let e1 = g.add_edge(n1, n2).unwrap();
                assert_eq!(
                    g.edge(&e1).unwrap(),
                    Triplet {
                        id: e1,
                        source: Pointee::EntityId(n1),
                        target: Pointee::EntityId(n2),
                    }
                )
            }

            /// Self-loop: an edge with both endpoints equal is allowed.
            #[test]
            fn test_add_edge2() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());

                let e1 = g.add_edge(n1, n1).unwrap();
                assert_eq!(
                    g.edge(&e1).unwrap(),
                    Triplet {
                        id: e1,
                        source: Pointee::EntityId(n1),
                        target: Pointee::EntityId(n1),
                    }
                )
            }

            /// Unexisten targets
            #[test]
            fn test_add_edge3() {
                let mut g = Graph::default();
                let n1 = Uuid::new_v4();
                let n2 = Uuid::new_v4();

                let err = g.add_edge(n1, n2).unwrap_err();
                assert_eq!(
                    err,
                    AddEdgeError::MissingEndpointsError(MissingEndpointsError {
                        missing_endpoints: vec![Pointee::EntityId(n1), Pointee::EntityId(n2)],
                    })
                )
            }

            /// Edge already exist
            #[test]
            fn test_add_edge4() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let e1 = g.add_edge(n1, n2).unwrap();
                let err = g
                    .silent_add_edge_with_id(e1, n1.into(), n2.into())
                    .unwrap_err();
                assert_eq!(err, AddEdgeError::EdgeAlreadyExists(e1))
            }
        }
    }

    mod test_destructors {
        use super::*;

        mod test_remove_node {
            use super::*;

            /// `remove_node` rejects an unknown id.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g.remove_node(&Uuid::new_v4()).unwrap_err();
                assert!(matches!(err, NodeNotFoundError(_)));
            }

            /// Removing a node returns its attached object as Field::Object.
            #[test]
            fn returns_object() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let returned = g.remove_node(&n1).unwrap();
                assert_eq!(returned, Field::Object(obj));
                assert!(!g.is_exist(&n1));
                test_utils::check_index_invariant(&g);
            }

            /// Removing `n1` cascades to `e: n1 → n2`.
            /// `n2` survives but its target bucket no longer contains `e`.
            #[test]
            fn cascades_direct_edge_reference() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.is_exist(&n1));
                assert!(!g.edges.contains_key(&e));
                assert!(g.is_exist(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// Removing `n2` cascades to `e: n1 → n2/field` (Path-pointee).
            /// Verifies the `entity_to_path_pointees` lookup path.
            #[test]
            fn cascades_path_reference() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let e = g.add_edge(n1, path.clone()).unwrap();

                g.remove_node(&n2).unwrap();

                assert!(!g.is_exist(&n2));
                assert!(!g.edges.contains_key(&e));
                assert!(g.is_exist(&n1));
                assert!(!g.entity_to_path_pointees.contains_key(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// Self-loop `e: n1 → n1`: index is fully drained on removal.
            #[test]
            fn cascades_self_loop() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj);
                let e = g.add_edge(n1, n1).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.edges.contains_key(&e));
                assert!(g.pointee_uses.is_empty());
                assert!(g.entity_to_path_pointees.is_empty());
                test_utils::check_index_invariant(&g);
            }

            /// Chain: `e1: n1 → n2`, `e2: n3 → e1`. Removing `n1` must
            /// cascade to `e1` and then to `e2` (since `e1` is `e2`'s target).
            #[test]
            fn cascades_through_meta_edge() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);

                let e1 = g.add_edge(n1, n2).unwrap();
                let e2 = g.add_edge(n3, e1).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.edges.contains_key(&e1));
                assert!(!g.edges.contains_key(&e2));
                assert!(g.is_exist(&n2));
                assert!(g.is_exist(&n3));
                test_utils::check_index_invariant(&g);
            }

            /// Hyperedge with two members loses one — survives with the other.
            #[test]
            fn hyperedge_loses_member_but_survives() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                m.insert(n2.into());
                let h = g.create_hyperedge(m).unwrap();

                g.remove_node(&n1).unwrap();

                let mut expected = HashSet::new();
                expected.insert(n2.into());
                assert_eq!(g.hyperedge_members(&h), Some(&expected));
                test_utils::check_index_invariant(&g);
            }

            /// Hyperedge with the only member `n1` becomes empty when `n1` dies
            /// and is itself cascade-removed.
            #[test]
            fn hyperedge_empties_and_dies() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.hyper_edge.contains_key(&h));
                test_utils::check_index_invariant(&g);
            }

            /// Cascade reaches edges that pointed at a hyperedge that
            /// itself died from emptying.
            #[test]
            fn cascade_through_dead_hyperedge() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                // edge from n2 to the soon-to-die hyperedge
                let e = g.add_edge(n2, h).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.hyper_edge.contains_key(&h));
                assert!(!g.edges.contains_key(&e));
                assert!(g.is_exist(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// `remove_node` rejects ids that live in `entities` because of
            /// `attach_obj` on an edge — those are NOT nodes.
            #[test]
            fn rejects_attached_object_id() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj).unwrap();

                let err = g.remove_node(&e).unwrap_err();
                assert!(matches!(err, NodeNotFoundError(_)));
                // Edge itself untouched.
                assert!(g.edges.contains_key(&e));
                test_utils::check_index_invariant(&g);
            }

            /// Records exactly one `Patch::RemoveNode` even on a cascade.
            #[test]
            fn records_single_patch_on_cascade() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let _e = g.add_edge(n1, n2).unwrap();

                g.remove_node(&n1).unwrap();

                let last = g.events.last().unwrap();
                assert_eq!(*last, Patch::RemoveNode { id: n1 });
                let remove_count = g
                    .events
                    .iter()
                    .filter(|p| matches!(p, Patch::RemoveNode { .. }))
                    .count();
                assert_eq!(remove_count, 1);
            }
        }

        mod test_remove_hyperedge {
            use super::*;

            /// Removing an unknown id returns an error.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g.remove_hyperedge(&Uuid::new_v4()).unwrap_err();
                assert!(matches!(err, HyperEdgeNotFound(_)));
            }

            /// Plain remove: hyperedge gone, members survive without
            /// `hid` in their `hyperedges` set.
            #[test]
            fn removes_and_unregisters_members() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                m.insert(n2.into());
                let h = g.create_hyperedge(m.clone()).unwrap();

                let returned = g.remove_hyperedge(&h).unwrap();

                assert_eq!(returned, m);
                assert!(!g.hyper_edge.contains_key(&h));
                assert!(g.is_exist(&n1));
                assert!(g.is_exist(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// Edges that pointed at the hyperedge cascade away.
            #[test]
            fn cascades_to_referencing_edges() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                let e = g.add_edge(n2, h).unwrap();

                g.remove_hyperedge(&h).unwrap();

                assert!(!g.hyper_edge.contains_key(&h));
                assert!(!g.edges.contains_key(&e));
                assert!(g.is_exist(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// Removing a hyperedge that's a member of another hyperedge:
            /// the parent loses this member; if parent becomes empty, it
            /// dies too.
            #[test]
            fn cascades_to_parent_hyperedge_when_emptied() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj);

                let mut inner_m = HashSet::new();
                inner_m.insert(n1.into());
                let inner = g.create_hyperedge(inner_m).unwrap();

                let mut outer_m = HashSet::new();
                outer_m.insert(inner.into());
                let outer = g.create_hyperedge(outer_m).unwrap();

                g.remove_hyperedge(&inner).unwrap();

                assert!(!g.hyper_edge.contains_key(&inner));
                assert!(!g.hyper_edge.contains_key(&outer));
                test_utils::check_index_invariant(&g);
            }

            /// Parent hyperedge with multiple members loses one — survives.
            #[test]
            fn parent_hyperedge_loses_only_this_member() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut inner_m = HashSet::new();
                inner_m.insert(n1.into());
                let inner = g.create_hyperedge(inner_m).unwrap();

                let mut outer_m = HashSet::new();
                outer_m.insert(inner.into());
                outer_m.insert(n2.into());
                let outer = g.create_hyperedge(outer_m).unwrap();

                g.remove_hyperedge(&inner).unwrap();

                let mut expected = HashSet::new();
                expected.insert(n2.into());
                assert_eq!(g.hyperedge_members(&outer), Some(&expected));
                test_utils::check_index_invariant(&g);
            }

            /// Attached object on the hyperedge is dropped along with it.
            #[test]
            fn drops_attached_object() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                g.attach_obj(h, obj).unwrap();
                assert!(g.entities.contains_key(&h));

                g.remove_hyperedge(&h).unwrap();

                assert!(!g.entities.contains_key(&h));
                test_utils::check_index_invariant(&g);
            }

            /// Records exactly one `Patch::RemoveHyperEdge` even on a cascade.
            #[test]
            fn records_single_patch_on_cascade() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                let _e = g.add_edge(n2, h).unwrap();

                g.remove_hyperedge(&h).unwrap();

                let last = g.events.last().unwrap();
                assert_eq!(*last, Patch::RemoveHyperEdge { id: h });
                let count = g
                    .events
                    .iter()
                    .filter(|p| matches!(p, Patch::RemoveHyperEdge { .. }))
                    .count();
                assert_eq!(count, 1);
            }
        }

        mod test_remove_attached {
            use super::*;

            /// Unknown id — neither node nor attach target.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g.remove_attached(Uuid::new_v4()).unwrap_err();
                assert!(matches!(err, RemoveAttached::NodeOrAttachedNotFound));
            }

            /// A bare node has no attached object — removal must fail
            /// rather than silently delete the node.
            #[test]
            fn rejects_node_id() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));

                let err = g.remove_attached(n1).unwrap_err();
                assert!(matches!(err, RemoveAttached::NodeOrAttachedNotFound));
                assert!(g.is_exist(&n1));
                assert!(g.entities.contains_key(&n1));
            }

            /// A bare edge (no attach_obj called) — nothing to remove.
            #[test]
            fn rejects_bare_edge() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                let err = g.remove_attached(e).unwrap_err();
                assert!(matches!(err, RemoveAttached::NodeOrAttachedNotFound));
                assert!(g.edges.contains_key(&e));
            }

            /// Attached object on an edge: edge stays alive, attached
            /// object gone, RemoveEdgeData patch recorded.
            #[test]
            fn removes_edge_attached() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj).unwrap();

                g.remove_attached(e).unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(!g.entities.contains_key(&e));
                assert_eq!(*g.events.last().unwrap(), Patch::RemoveEdgeData { id: e });
                test_utils::check_index_invariant(&g);
            }

            /// Attached object on a hyperedge: hyperedge stays alive,
            /// attached object gone, RemoveHyperEdgeData patch recorded.
            #[test]
            fn removes_hyperedge_attached() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                g.attach_obj(h, obj).unwrap();

                g.remove_attached(h).unwrap();

                assert!(g.hyper_edge.contains_key(&h));
                assert!(!g.entities.contains_key(&h));
                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::RemoveHyperEdgeData { id: h }
                );
                test_utils::check_index_invariant(&g);
            }

            /// EntityId references survive — only Path references die.
            #[test]
            fn entity_id_references_survive() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj).unwrap();

                // Edge that points at `e` as a whole entity.
                let n3 = g.add_node(test_utils::create_simple_obj("g"));
                let meta = g.add_edge(n3, e).unwrap();

                g.remove_attached(e).unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(g.edges.contains_key(&meta));
                test_utils::check_index_invariant(&g);
            }

            /// Path references through the attach target die.
            #[test]
            fn cascades_path_references() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                let attached = test_utils::create_simple_obj("data");
                g.attach_obj(e, attached).unwrap();

                // Edge whose endpoint is a path through `e`'s attached object.
                let n3 = g.add_node(test_utils::create_simple_obj("g"));
                let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
                let dangling = g.add_edge(n3, path).unwrap();

                g.remove_attached(e).unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(!g.edges.contains_key(&dangling));
                assert!(!g.entity_to_path_pointees.contains_key(&e));
                test_utils::check_index_invariant(&g);
            }

            /// Records exactly one data-removal patch even when the
            /// path-cascade kills downstream structures.
            #[test]
            fn records_single_patch_on_cascade() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, test_utils::create_simple_obj("data"))
                    .unwrap();

                let n3 = g.add_node(obj);
                let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
                let _dangling = g.add_edge(n3, path).unwrap();

                g.remove_attached(e).unwrap();

                let count = g
                    .events
                    .iter()
                    .filter(|p| {
                        matches!(
                            p,
                            Patch::RemoveEdgeData { .. } | Patch::RemoveHyperEdgeData { .. }
                        )
                    })
                    .count();
                assert_eq!(count, 1);
            }
        }
    }

    mod test_modifiers {
        use super::*;

        mod test_add_hyperedge_members {
            use super::*;

            /// Unknown hyperedge id is rejected.
            #[test]
            fn unknown_hyperedge() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let mut m = HashSet::new();
                m.insert(n1.into());
                let err = g.add_hyperedge_members(Uuid::new_v4(), m).unwrap_err();
                assert!(matches!(err, AddHyperedgeMembers::NotFound(_)));
            }

            /// Members that don't exist as pointees are rejected.
            #[test]
            fn missing_pointee() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original).unwrap();

                let mut m = HashSet::new();
                m.insert(Pointee::EntityId(Uuid::new_v4()));
                let err = g.add_hyperedge_members(h, m).unwrap_err();
                assert!(matches!(err, AddHyperedgeMembers::PointeesNotFound(_)));
            }

            /// Adding a member that's already there is rejected,
            /// and nothing is partially applied.
            #[test]
            fn duplicate_member_rejected_atomically() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                let mut m = HashSet::new();
                m.insert(n1.into()); // duplicate
                m.insert(n2.into()); // would-be new
                let err = g.add_hyperedge_members(h, m).unwrap_err();
                assert!(matches!(err, AddHyperedgeMembers::MembersAlreadyExist(_)));

                // Atomicity: n2 was NOT added.
                assert_eq!(g.hyperedge_members(&h), Some(&original));
                test_utils::check_index_invariant(&g);
            }

            /// Successful add: members extended, reverse index updated,
            /// patch recorded.
            #[test]
            fn adds_members_and_records_patch() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original).unwrap();

                let mut to_add = HashSet::new();
                to_add.insert(n2.into());
                to_add.insert(n3.into());
                g.add_hyperedge_members(h, to_add.clone()).unwrap();

                let mut expected = HashSet::new();
                expected.insert(n1.into());
                expected.insert(n2.into());
                expected.insert(n3.into());
                assert_eq!(g.hyperedge_members(&h), Some(&expected));

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::AddElementsToHyperEdge {
                        id: h,
                        members: to_add,
                    }
                );
                test_utils::check_index_invariant(&g);
            }

            /// Adding a Path-pointee tracks it in `entity_to_path_pointees`.
            #[test]
            fn tracks_path_member() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original).unwrap();

                let path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let mut to_add = HashSet::new();
                to_add.insert(path.clone());
                g.add_hyperedge_members(h, to_add).unwrap();

                assert!(g
                    .entity_to_path_pointees
                    .get(&n2)
                    .is_some_and(|s| s.contains(&path)));
                test_utils::check_index_invariant(&g);
            }

            /// Empty input is a no-op success.
            #[test]
            fn empty_input_is_noop() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj);
                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                g.add_hyperedge_members(h, HashSet::new()).unwrap();

                assert_eq!(g.hyperedge_members(&h), Some(&original));
                test_utils::check_index_invariant(&g);
            }
        }
    }

    mod lift_events {
        use super::*;

        #[test]
        fn test_create_hyperedge() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("field");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj);

            let mut m = HashSet::new();
            m.insert(n1.into());
            m.insert(n2.into());

            let h = g.create_hyperedge(m.clone()).unwrap();

            assert_eq!(
                *g.events.last().unwrap(),
                Patch::CreateHyperEdge { id: h, members: m }
            )
        }

        #[test]
        fn test_add_node() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("field");
            let n1 = g.add_node(obj.clone());

            assert_eq!(*g.events.last().unwrap(), Patch::AddNode { id: n1, obj })
        }

        #[test]
        fn test_add_edge() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("field");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj);

            let e = g.add_edge(n1, n2).unwrap();

            assert_eq!(
                *g.events.last().unwrap(),
                Patch::AddEdge {
                    id: e,
                    source: n1.into(),
                    target: n2.into()
                }
            )
        }

        #[test]
        fn test_remove_node() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("field");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());

            let e = g.add_edge(n1, n2).unwrap();

            g.remove_node(&n1).unwrap();

            // To ensure that remove doesn't produce remove event's
            assert_eq!(
                g.events[0],
                Patch::AddNode {
                    id: n1,
                    obj: obj.clone()
                }
            );
            assert_eq!(g.events[1], Patch::AddNode { id: n2, obj: obj });
            assert_eq!(
                g.events[2],
                Patch::AddEdge {
                    id: e,
                    source: n1.into(),
                    target: n2.into()
                }
            );
            assert_eq!(g.events[3], Patch::RemoveNode { id: n1 })
        }
    }

    #[test]
    fn test_remove_edge() {
        let mut g = Graph::default();
        let obj = test_utils::create_simple_obj("field");
        let n1 = g.add_node(obj.clone());
        let n2 = g.add_node(obj.clone());

        let e = g.add_edge(n1, n2).unwrap();

        g.remove_edge(&e).unwrap();

        assert_eq!(*g.events.last().unwrap(), Patch::RemoveEdge { id: e });
    }
}
