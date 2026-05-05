mod hyperedge;
mod incidence;
mod methods;
mod paths;

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
enum ApplyDeltaError {
    EdgeNotFoundError(EdgeNotFoundError),
    NodeAlreadyExists(NodeAlreadyExistsError),
    NodeNotFound(NodeNotFoundError),
    MissingTargetsError(MissingEndpointsError),
    AddEdgeError(AddEdgeError),
    RetargetError(RetargetError),
    IncorrectType(IncorrectTypeError),
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

/// Errors returned by [`Graph::apply_patch`].
///
/// Currently only `AddNode` and `AddEdge` are implemented — the
/// other `Patch` variants return `NotImplemented`. This is enough
/// to let algorithms (e.g. in `paths`) construct a result graph
/// while preserving original ids.
#[derive(Debug)]
pub(crate) enum ApplyPatchError {
    NodeAlreadyExists(NodeAlreadyExistsError),
    AddEdge(AddEdgeError),
    HyperEdgeAlreadyExists(HyperEdgeAlreadyExistsError),
    NotImplemented,
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
struct Graph {
    /// Hold graph object
    entities: HashMap<EntityId, Object>,

    /// Hold graph edges
    edges: HashMap<EdgeID, (Pointee, Pointee)>,

    /// Hold hyperedges
    hyper_edge: HashMap<HyperEdgeId, HashSet<Pointee>>,
}

impl Graph {
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

    /* ------------ END PROBS -------------------- */

    /* ------------ START CONSTRUCTORS ----------- */

    fn __create_hyperedge_with_id(
        &mut self,
        id: HyperEdgeId,
        members: HashSet<Pointee>,
    ) -> Result<(), HyperEdgeAlreadyExistsError> {
        if self.hyper_edge.contains_key(&id) {
            return Err(HyperEdgeAlreadyExistsError(id));
        }
        self.hyper_edge.insert(id, members);
        Ok(())
    }

    fn create_hyperedge(&mut self, members: HashSet<Pointee>) -> HyperEdgeId {
        let id = Uuid::new_v4();
        let _ = self.__create_hyperedge_with_id(id, members);
        id
    }

    fn __add_node_with_id(
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
        self.__add_node_with_id(id, obj);
        id
    }

    fn __add_edge_with_id(
        &mut self,
        id: Uuid,
        source: Pointee,
        target: Pointee,
    ) -> Result<(), AddEdgeError> {
        let source_exists = self.is_pointee_exist(&source);
        let target_exists = self.is_pointee_exist(&target);
        if !source_exists || !target_exists {
            let mut missing_targets = Vec::new();
            if !source_exists {
                missing_targets.push(source);
            }
            if !target_exists {
                missing_targets.push(target);
            }
            return Err(AddEdgeError::MissingEndpointsError(MissingEndpointsError {
                missing_endpoints: missing_targets,
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
        let edge_id = Uuid::new_v4();
        self.__add_edge_with_id(edge_id, source.into(), target.into())
            .map(|_| edge_id)?;
        Ok(edge_id)
    }

    /// Apply a single [`Patch`] to this graph.
    ///
    /// Currently `AddNode`, `AddEdge`, and `CreateHyperEdge` are
    /// wired up — the other variants return
    /// [`ApplyPatchError::NotImplemented`]. The minimal
    /// implementation is enough to let algorithms (e.g. in `paths`)
    /// build a result graph that preserves the original ids of
    /// nodes, edges, and hyperedges.
    pub(crate) fn apply_patch(&mut self, patch: Patch) -> Result<(), ApplyPatchError> {
        match patch {
            Patch::AddNode { id, obj } => self
                .__add_node_with_id(id, obj)
                .map_err(ApplyPatchError::NodeAlreadyExists),
            Patch::AddEdge { id, source, target } => self
                .__add_edge_with_id(id, source, target)
                .map_err(ApplyPatchError::AddEdge),
            Patch::CreateHyperEdge { id, members } => self
                .__create_hyperedge_with_id(id, members)
                .map_err(ApplyPatchError::HyperEdgeAlreadyExists),
            _ => Err(ApplyPatchError::NotImplemented),
        }
    }
    
    /* ------------ END CONSTRUCTORS ------------- */

    /* ------------ START DESTRUCTORS ----------- */
    pub fn remove_node(&mut self, id: &Uuid) -> Result<Field, NodeNotFoundError> {
        todo!()
    }
    
    pub fn remove_edge(&mut self, id: &Uuid) -> Result<Triplet, EdgeNotFoundError> {
        todo!()
    }

    pub fn remove_hyperedge(&mut self, target: &HyperEdgeId) -> Option<HyperEdgeNotFound> {
        todo!()
    }

    /* ------------ END DESTRUCTORS ------------- */

    /* ------------ START MODIFIERS ----------- */

    fn attach_obj(&mut self, target: AttachTargetID, obj: Object) -> Result<(), AttachNodeError> {
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

    pub fn replace_node(&mut self, id: &Uuid, field: Field) -> Result<Field, NodeNotFoundError> {
        todo!()
    }

    pub fn replace_attach_obj(&mut self, id: &Uuid, field: Field) -> Result<(), EdgeNotFoundError> {
        todo!()
    }

    pub fn retraget_edge(
        &mut self,
        id: &Uuid,
        new_target: RetrargetEdge,
    ) -> Result<(), RetargetError> {
        todo!()
    }

    /* ------------ END MODIFIERS ------------- */

    /* ------------ START LISTENERS ----------- */
    
    fn notify_listeners(&self, patch: Patch) {
        todo!()
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

            let h = g.create_hyperedge(m);
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

            let h = graph.create_hyperedge(m);
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

                let h = g.create_hyperedge(m);
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

            /// A hyperedge with no members yields an empty set.
            #[test]
            fn hyperedge_empty() {
                let mut g = Graph::default();
                let h = g.create_hyperedge(HashSet::new());
                let members = g.hyperedge_members(&h).unwrap();
                assert!(members.is_empty());
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
                    let h = g.create_hyperedge(m);
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

                let h = g.create_hyperedge(members.clone());
                assert!(g.is_exist(&h));
                assert_eq!(g.hyperedge_members(&h), Some(&members));
            }

            /// An empty member set is a valid hyperedge.
            #[test]
            fn create_hyperedge_empty() {
                let mut g = Graph::default();
                let h = g.create_hyperedge(HashSet::new());
                assert_eq!(g.hyperedge_members(&h), Some(&HashSet::new()));
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

                let h2 = g.create_hyperedge(members.clone());
                assert_eq!(g.hyperedge_members(&h2), Some(&members));
            }

            /// `__create_hyperedge_with_id` rejects a duplicate id.
            #[test]
            fn create_hyperedge_already_exists() {
                let mut g = Graph::default();
                let h = g.create_hyperedge(HashSet::new());
                let err = g
                    .__create_hyperedge_with_id(h, HashSet::new())
                    .unwrap_err();
                assert_eq!(err, HyperEdgeAlreadyExistsError(h));
            }

            /// Re-inserting with the same id must NOT clobber the
            /// existing members — the original hyperedge stays
            /// intact.
            #[test]
            fn create_hyperedge_already_exists_preserves_members() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone());

                // Try to overwrite with a different (empty) member set.
                let _ = g.__create_hyperedge_with_id(h, HashSet::new());

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
                let result2 = graph.__add_node_with_id(n1, obj2.clone());
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
                let err = g.__add_edge_with_id(e1, n1.into(), n2.into()).unwrap_err();
                assert_eq!(err, AddEdgeError::EdgeAlreadyExists(e1))
            }
        }
    }
}
