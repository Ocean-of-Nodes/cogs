mod methods;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use uuid::Uuid;

use common::*;

/// Listener is a function that will be called when graph be changed
type ListernerID = Uuid;

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
struct NodeAlreadyExistsError(NodeId);
#[derive(Debug)]
struct NodeNotFoundError(NodeId);
#[derive(Debug)]
struct EdgeNotFoundError(EdgeID);
#[derive(Debug, PartialEq, Eq)]
struct MissingEndpointsError {
    missing_endpoints: Vec<Uuid>,
}

#[derive(Debug)]
struct IncorrectTypeError {
    node_id: NodeId,
    expected_type: Vec<String>,
    actual_type: String,
}

#[derive(Debug, PartialEq, Eq)]
enum AddEdgeError {
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

/// Triplet is a link beetween two entities
#[derive(Debug, Default, PartialEq, Eq)]
struct Triplet {
    id: EdgeID,
    source: EntityId,
    target: EntityId,
}

enum EntityType {
    Node,
    Edge,
    HyperEdge,
    MetaEdge,
    AttachedObject,
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
    edges: HashMap<EdgeID, (EntityId, EntityId)>,

    /// Hold hyperedges
    hyper_edge: HashMap<HyperEdgeId, Vec<EntityId>>,
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
    /*
    /* ------------ START GETTERS ------------------- */

    /// Get subgraph by path, returns None if subgraph doesn't exist
    pub fn subgraph(&mut self, path: &PathBuf) -> Option<&mut Graph> {
        let mut current_graph = self;
        for chank in path.iter() {
            current_graph = current_graph.subgraphs.get_mut(chank.to_str()?)?;
        }
        Some(current_graph)
    }

    /// Returns `path` of current graph
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// `Sheave` is a bunch of `links` between two `Graph`s.
    ///
    /// A sheave bundles cross-graph edges (and any meta-edges built
    /// on top of them) into a single object that lives outside the
    /// two graphs it connects.
    ///
    /// ```text
    /// +---- lhs graph ----+                +---- rhs graph ----+
    /// |                   |                |                   |
    /// |  n1 ---(a)--- n2  |                |  m1 ---(x)--- m2  |
    /// |         |         |                |         |         |
    /// |        (b)        |                |        (y)        |
    /// |         |         |                |         |         |
    /// |         n3        |                |         m3        |
    /// |                   |                |                   |
    /// +-------------------+                +-------------------+
    ///          :                                    :
    ///          :   n2 ----(L1)---------------- m1   :
    ///          :              ^                     :
    ///          :             (M)  <- meta-edge      :
    ///          :              v                     :
    ///          :   n3 ----(L2)---------------- m3   :
    ///          :                                    :
    ///           \-------------- sheave -------------/
    /// ```
    ///
    /// In the picture above, `a`, `b`, `x`, `y` are internal edges
    /// of `lhs` and `rhs` and stay inside their respective graphs.
    /// `L1` and `L2` are regular edges of the sheave that cross the
    /// boundary between the two graphs. `M` is a meta-edge whose
    /// endpoints are themselves sheave edges (`L1` and `L2`).
    pub fn sheave(&self, lhs: &Graph, rhs: &Graph) -> &mut Graph {
        unimplemented!()
    }

    /// Returns the entities directly connected to `id` — that is,
    /// for every edge incident to `id`, the **other** endpoint.
    ///
    /// The edges themselves are **not** part of the result. They are
    /// the *paths* along which neighbours are reached, not the
    /// destinations. To retrieve the incident edges, use
    /// [`Graph::edges`].
    ///
    /// ```text
    ///   neighbours(n1) = [n2, n3]
    ///
    ///        n2 ----(e1)---- n1 ----(e2)---- n3
    ///        ^                ^                ^
    ///        |                |                |
    ///     returned         queried          returned
    ///
    ///        (e1) and (e2) are NOT in the result —
    ///        they are the connections, not the neighbours.
    /// ```
    ///
    /// `id` may itself be an edge. In that case the result contains
    /// entities reached via *incident meta-edges*, but never the
    /// `source` / `target` of the edge `id` itself:
    ///
    /// ```text
    ///   n1 ----(e1)---- n2
    ///           |
    ///          (e3)         e3 is a meta-edge with
    ///           |           endpoints e1 and e2.
    ///   n3 ----(e2)---- n4
    /// ```
    ///
    /// `neighbours(e1) = [e2]` — `e3` is incident to `e1`, and its
    /// *other* endpoint is `e2`. `n1` and `n2` are **not** in the
    /// result: they are `e1`'s own endpoints, not entities reached
    /// *via* another edge incident to `e1`.
    ///
    /// # Errors
    ///
    /// Returns [`EntityNotFoundError`] if `id` does not exist
    /// anywhere in this graph or its subgraphs.
    pub fn neighbours(&self, id: &EntityId) -> Result<Vec<EntityId>, EntityNotFoundError> {
        if !self.is_exist(id) {
            return Err(EntityNotFoundError(*id));
        }

        let mut neighbours = Vec::new();
        for triplet in self.edges.iter() {
            if &triplet.source == id {
                neighbours.push(triplet.target);
            } else if &triplet.target == id {
                neighbours.push(triplet.source);
            }
        }

        for triplet in self.beetween_edges.iter() {
            if &triplet.source == id {
                neighbours.push(triplet.target);
            } else if &triplet.target == id {
                neighbours.push(triplet.source);
            }
        }

        for graph in self.subgraphs.values() {
            // NOTE: we olready check that id exist in graph,
            if let Ok(subgraph_neighbours) = graph.neighbours(id) {
                neighbours.extend(subgraph_neighbours);
            }
        }

        Ok(neighbours)
    }

    /// Get all edges link with entity with id
    pub fn edges(&self, id: &EntityId) -> Result<Vec<EdgeID>, EntityNotFoundError> {
        if !self.is_exist(id) {
            return Err(EntityNotFoundError(*id));
        }

        let mut edges = Vec::new();
        for triplet in self.edges.iter() {
            if &triplet.source == id || &triplet.target == id {
                edges.push(triplet.id);
            }
        }

        for triplet in self.beetween_edges.iter() {
            if &triplet.source == id || &triplet.target == id {
                edges.push(triplet.id);
            }
        }

        for graph in self.subgraphs.values() {
            if let Ok(subgraph_edges) = graph.edges(id) {
                edges.extend(subgraph_edges);
            }
        }

        Ok(edges)
    }

    pub fn out_nodes() {
        unimplemented!()
    }

    pub fn in_nodes() {
        unimplemented!()
    }

    pub fn get_paths() {
        unimplemented!()
    }

    */
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
    pub fn get_edge(&self, id: &EdgeID) -> Result<Triplet, GetEdgeError> {
        if let Some(pair) = self.edges.get(id) {
            return Ok(Triplet {
                id: *id,
                source: pair.0,
                target: pair.1,
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
            let is_meta_endpoint =
                |e: &EntityId| self.edges.contains_key(e) || self.hyper_edge.contains_key(e);
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

    /*
    /// Check if id is edge from the whole graph,
    /// returns true if edge exists, false otherwise
    pub fn is_edge(&self, id: &EntityId) -> bool {
        self.global_edges().any(|edge_id| &edge_id == id)
    }

    /// Check if id is node from the whole graph,
    /// returns true if node exists, false otherwise
    pub fn is_node(&self, id: &EntityId) -> bool {
        self.global_nodes().any(|node_id| &node_id == id)
    }
    */
    /// Check if id is node or edge from the whole graph,
    /// returns true if node or edge exists, false otherwise
    pub fn is_exist(&self, id: &EntityId) -> bool {
        self.iter_entities().any(|e| &e == id)
    }
    /*
    /// Check that `source` and `target` exist.
    /// Returns [`MissingEndpointsError`] with unexist endpoints
    fn __ensure_endpoints_exist(
        &self,
        source: &EntityId,
        target: &EntityId,
    ) -> Result<(), MissingEndpointsError> {
        let mut missing_endpoints = Vec::new();
        if !self.is_exist(source) {
            missing_endpoints.push(*source);
        }
        if !self.is_exist(target) {
            missing_endpoints.push(*target);
        }

        if missing_endpoints.is_empty() {
            Ok(())
        } else {
            Err(MissingEndpointsError { missing_endpoints })
        }
    }

    /// Returns `true` if a path exists between `source` and `target`
    /// while staying within entities of the **same kind** — that is,
    /// node-to-node or edge-to-edge, but never crossing between them.
    ///
    /// Two nodes count as connected when an edge has them as its
    /// endpoints. Two edges count as connected when a meta-edge — an
    /// edge whose endpoints are themselves edges — links them. A
    /// mixed query (one node and one edge) therefore always returns
    /// `Ok(false)`: there is no same-kind path between them by
    /// definition. Use [`Graph::is_linked`] for connectivity that may
    /// freely traverse both nodes and edges.
    ///
    /// # Example
    ///
    /// ```text
    ///   n1 --(e1)--> n2
    ///         ^
    ///         |
    ///        (e3)
    ///         |
    ///   n3 --(e2)--> n4
    ///         ^
    ///         |
    ///        (e5)
    ///         |
    ///   n5 --(e4)--> n6 --(e6)--> n7
    /// ```
    ///
    /// - `is_existing_path(e1, e4)` → `Ok(true)`  — via the meta-edge
    ///   chain `e1 — e3 — e2 — e5 — e4`.
    /// - `is_existing_path(n5, n7)` → `Ok(true)`  — via the node
    ///   chain `n5 — e4 — n6 — e6 — n7`.
    /// - `is_existing_path(e1, n6)` → `Ok(false)` — an edge and a
    ///   node are never on the same-kind path.
    ///
    /// # Errors
    ///
    /// Returns [`MissingEndpointsError`] if `source` or `target` does
    /// not exist anywhere in this graph or its subgraphs.
    pub fn is_existing_path(
        &self,
        source: &EntityId,
        target: &EntityId,
    ) -> Result<bool, MissingEndpointsError> {
        self.__ensure_endpoints_exist(source, target)?;

        if self.is_node(source) && self.is_node(target) {
            // Check node-to-node path
            unimplemented!()
        } else if self.is_edge(source) && self.is_edge(target) {
            // Check edge-to-edge path
            unimplemented!()
        } else {
            // Mixed query: one node and one edge
            Ok(false)
        }
    }

    /// Returns `true` if a path exists between `source` and `target`
    /// when traversal may freely cross between nodes and edges.
    ///
    /// An entity is incident with another whenever an edge — regular
    /// or meta — connects them. Unlike [`Graph::is_existing_path`],
    /// which stays within entities of one kind, `is_linked` treats
    /// nodes and edges uniformly: a node reaches an adjacent edge
    /// through that edge's endpoints, and an edge reaches another
    /// edge through any meta-edge between them.
    ///
    /// As a consequence, `is_linked` is at least as permissive as
    /// `is_existing_path` — every same-kind path is also a cross-kind
    /// path, but not every cross-kind path is same-kind.
    ///
    /// # Example
    ///
    /// ```text
    ///   n1 --(e1)--> n2
    ///         ^
    ///         |
    ///        (e3)
    ///         |
    ///   n3 --(e2)--> n4
    ///         ^
    ///         |
    ///        (e5)
    ///         |
    ///   n5 --(e4)--> n6 --(e6)--> n7
    /// ```
    ///
    /// - `is_linked(e1, e4)` → `Ok(true)`  — via `e1 — e3 — e2 — e5 — e4`
    ///   (same route as `is_existing_path`).
    /// - `is_linked(n5, n7)` → `Ok(true)`  — via `n5 — e4 — n6 — e6 — n7`
    ///   (same route as `is_existing_path`).
    /// - `is_linked(e1, n6)` → `Ok(true)`  — via
    ///   `e1 — e3 — e2 — e5 — e4 — n6`. The same query is `Ok(false)`
    ///   for [`Graph::is_existing_path`], which forbids crossing
    ///   between an edge and a node.
    ///
    /// # Errors
    ///
    /// Returns [`MissingEndpointsError`] if `source` or `target` does
    /// not exist anywhere in this graph or its subgraphs.
    pub fn is_linked(
        &self,
        source: &EntityId,
        target: &EntityId,
    ) -> Result<bool, MissingEndpointsError> {
        self.__ensure_endpoints_exist(source, target)?;

        unimplemented!()
    }

    /* ------------ END PROBS -------------------- */

    /* ------------ START CONSTRUCTORS ----------- */

    /// Walks the parent path (all components except the last);
    /// the last component is the name under which `graph` will
    /// be inserted in that parent.
    fn add_subgraph(
        &mut self,
        name: &PathBuf,
        mut graph: Graph,
    ) -> Result<(), UnexistentPathError> {
        let mut components = name.iter();
        let subgraph_name = components
            .next_back()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let mut current_graph = self;
        let mut current_path = PathBuf::new();
        for chank in components {
            let key = chank.to_str().unwrap();
            if let Some(subgraph) = current_graph.subgraphs.get_mut(key) {
                current_graph = subgraph;
                current_path.push(chank);
            } else {
                return Err(UnexistentPathError {
                    valid_path_part: current_path,
                });
            }
        }
        graph.path = name.to_path_buf();
        current_graph.subgraphs.insert(subgraph_name, graph);
        Ok(())
    }
    */
    fn create_hyperedge(&mut self, members: Vec<EntityId>) -> HyperEdgeId {
        let id = Uuid::new_v4();
        self.hyper_edge.insert(id, members);
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
        source: EntityId,
        target: EntityId,
    ) -> Result<(), AddEdgeError> {
        let source_exists = self.is_exist(&source);
        let target_exists = self.is_exist(&target);
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

    pub fn add_edge(&mut self, source: EntityId, target: EntityId) -> Result<EdgeID, AddEdgeError> {
        let edge_id = Uuid::new_v4();
        self.__add_edge_with_id(edge_id, source, target)
            .map(|_| edge_id)?;
        Ok(edge_id)
    }
    /*
    /* ------------ END CONSTRUCTORS ------------- */

    /* ------------ START DESTRUCTORS ----------- */
    pub fn remove_node(&mut self, id: &Uuid) -> Result<Field, NodeNotFoundError> {
        self.entities
            .remove(id)
            .ok_or_else(|| NodeNotFoundError(*id))
    }

    pub fn remove_edge(&mut self, id: &Uuid) -> Result<Triplet, EdgeNotFoundError> {
        self.edges
            .iter()
            .position(|triplet| &triplet.id == id)
            .map(|index| self.edges.remove(index))
            .ok_or_else(|| EdgeNotFoundError(*id))
    }

    /* ------------ END DESTRUCTORS ------------- */

    /* ------------ START MODIFIERS ----------- */
    */

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

    /*
    pub fn replace_node(&mut self, id: &Uuid, field: Field) -> Result<Field, NodeNotFoundError> {
        unimplemented!()
    }

    pub fn replace_edge_data(&mut self, id: &Uuid, field: Field) -> Result<(), EdgeNotFoundError> {
        unimplemented!()
    }

    pub fn retraget_edge(
        &mut self,
        id: &Uuid,
        new_target: RetrargetEdge,
    ) -> Result<(), RetargetError> {
        unimplemented!()
    }

    pub fn apply_delta(&mut self, id: &Uuid, delta: Patch) -> Result<(), ApplyDeltaError> {
        match delta {
            Patch::AddNode { id, obj: field } => self
                .__add_node_with_id(id, field)
                .map_err(|e| ApplyDeltaError::NodeAlreadyExists(e)),
            Patch::RemoveNode { id } => self
                .remove_node(&id)
                .map(|_| ())
                .map_err(|_| ApplyDeltaError::NodeNotFound(NodeNotFoundError(id))),
            Patch::ReplaceNode { id, field } => self
                .replace_node(&id, field)
                .map(|_| ())
                .map_err(|_| ApplyDeltaError::NodeNotFound(NodeNotFoundError(id))),
            Patch::ChangeNode { id, delta } => {
                // self.get_node(&id)
                //     .ok_or_else(|| ApplyDeltaError::NodeNotFound(NodeNotFoundError(id)))
                //     .and_then(|field| {
                //         if let Field::Object(ref mut obj) = field {
                //             // for change in delta {
                //             //    unimplemented!()
                //             // }
                //             unimplemented!()
                //         } else {
                //             Err(ApplyDeltaError::FieldDoesntObject {
                //                 node_id: id,
                //                 actual_type: format!("{:?}", field),
                //             })
                //         }
                //     })
                unimplemented!()
            }
            Patch::AddEdge { id, source, target } => self
                .__add_edge_with_id(id, source, target)
                .map_err(|e| ApplyDeltaError::AddEdgeError(e)),
            Patch::RemoveEdge { id } => self
                .remove_edge(&id)
                .map(|_| ())
                .map_err(|_| ApplyDeltaError::EdgeNotFoundError(EdgeNotFoundError(id))),
            Patch::RetrargetEdge { id, new_target } => self
                .retraget_edge(&id, new_target)
                .map_err(|e| ApplyDeltaError::RetargetError(e)),
        }
    }
    /* ------------ END MODIFIERS ------------- */

    /* ------------ START LISTENERS ----------- */
    fn notify_listeners(&self, patch: Patch) {
        for listener in self.listeners.values() {
            listener(patch.clone());
        }
    }

    pub fn subscribe_on_change(&mut self, listener: Box<dyn Fn(Patch)>) -> ListernerID {
        let id = Uuid::new_v4();
        self.listeners.insert(id, listener);
        id
    }

    pub fn unsubscribe_on_change(&mut self, id: ListernerID) {
        self.listeners.remove(&id);
    }
    /* ------------ END LISTENERS ------------- */
    */
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_simple_obj(field_name: &str) -> Object {
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
    fn create_semple_graph() -> (
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

        let h = g.create_hyperedge(vec![n1, n3]);
        let e5 = g.add_edge(h, e4).unwrap();

        (g, n1, n2, n3, n4, e1, e2, e3, e4, e5, h)
    }

    mod test_globals {
        use super::*;

        mod test_iter_entities {
            use super::*;

            /// Simple case we just check that all
            /// kind of entities iterator yeld
            #[test]
            fn test_iter_entities1() {
                let (g, n1, n2, n3, n4, e1, e2, e3, e4, e5, h) = create_semple_graph();

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
                let obj = create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());

                let e1 = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e1, obj.clone()).unwrap();

                let h = g.create_hyperedge(vec![n1, n2]);
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
                let (g, _n1, _n2, _n3, _n4, e1, e2, e3, e4, e5, _h) = create_semple_graph();

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
                let (g, n1, n2, n3, n4, _e1, _e2, _e3, _e4, _e5, _h) = create_semple_graph();

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
                let (g, _n1, _n2, _n3, _n4, _e1, _e2, _e3, _e4, _e5, h) = create_semple_graph();

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
    }
    mod test_probs {
        use super::*;

        mod test_is_exist {
            use super::*;
            /// Test exist node
            #[test]
            fn test_is_exist1() {
                let mut g = Graph::default();
                let obj = create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                assert!(g.is_exist(&n1))
            }

            /// Test exist edge
            #[test]
            fn test_is_exist2() {}

            /// Test exist hyperedge
            #[test]
            fn test_is_exist3() {}
        }
    }

    mod test_constructors {
        use super::*;

        mod test_add_node {
            use super::*;

            /// Simple case we add node and check that's is exist
            #[test]
            fn test_add_node1() {
                let mut graph = Graph::default();
                let obj = create_simple_obj("test_field");
                let node_id = graph.add_node(obj.clone());
                assert_eq!(graph.obj(&node_id), Some(&obj));
            }

            /// Check error node elready exist
            #[test]
            fn test_add_node2() {
                let mut graph = Graph::default();
                let obj = create_simple_obj("test_field");
                let n1 = graph.add_node(obj.clone());
                let obj2 = create_simple_obj("test_field2");
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
                let obj = create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let e1 = g.add_edge(n1, n2).unwrap();
                assert_eq!(
                    g.get_edge(&e1).unwrap(),
                    Triplet {
                        id: e1,
                        source: n1,
                        target: n2,
                    }
                )
            }

            /// Self-loop: an edge with both endpoints equal is allowed.
            #[test]
            fn test_add_edge2() {
                let mut g = Graph::default();
                let obj = create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());

                let e1 = g.add_edge(n1, n1).unwrap();
                assert_eq!(
                    g.get_edge(&e1).unwrap(),
                    Triplet {
                        id: e1,
                        source: n1,
                        target: n1,
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
                        missing_endpoints: vec![n1, n2],
                    })
                )
            }

            /// Edge already exist
            #[test]
            fn test_add_edge4() {
                let mut g = Graph::default();
                let obj = create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let e1 = g.add_edge(n1, n2).unwrap();
                let err = g.__add_edge_with_id(e1, n1, n2).unwrap_err();
                assert_eq!(err, AddEdgeError::EdgeAlreadyExists(e1))
            }
        }
    }

    /*
    mod test_constructors_and_getters {
        use super::*;

        /// A simple case
        #[test]
        fn test_get_neighbours_1() {
            // Built graph:
            // ```text
            //  node1 --(edge)--> node2
            //   ^
            //   |
            // (edge2)
            //   |
            //  node3
            // ```
            let mut graph = Graph::default();

            let field1 = Field::String("node1".to_string());
            let field2 = Field::String("node2".to_string());
            let field3 = Field::String("node3".to_string());
            let node_id1 = graph.add_node(field1).unwrap();
            let node_id2 = graph.add_node(field2).unwrap();
            let node_id3 = graph.add_node(field3).unwrap();
            graph.add_edge(node_id1, node_id2).unwrap();
            graph.add_edge(node_id3, node_id1).unwrap();

            let neighbours = graph.neighbours(&node_id1);
            assert_eq!(neighbours.len(), 2);
            assert!(neighbours.contains(&node_id2));
            assert!(neighbours.contains(&node_id3));
        }

        /// A more complex case with edge beetween edges
        /// Test that get_neighbours will return only nodes,
        /// but not edges, even if edge is beetween two edges
        #[test]
        fn test_get_neighbours_2() {
            // Built graph:
            // ```text
            //  node1 --(edge1)--> node2
            //            |
            //          (edge3) < -- (edge4) -- node5
            //            |
            //            v
            //  node3 --(edge2)--> node4
            // ```
            let mut graph = Graph::default();

            let field1 = Field::String("node1".to_string());
            let field2 = Field::String("node2".to_string());
            let node_id1 = graph.add_node(field1).unwrap();
            let node_id2 = graph.add_node(field2).unwrap();
            let edge1 = graph.add_edge(node_id1, node_id2).unwrap();

            let field3 = Field::String("node3".to_string());
            let field4 = Field::String("node4".to_string());
            let node_id3 = graph.add_node(field3).unwrap();
            let node_id4 = graph.add_node(field4).unwrap();
            let edge2 = graph.add_edge(node_id3, node_id4).unwrap();

            let edge3 = graph.add_edge(edge1, edge2).unwrap();

            let field5 = Field::String("node5".to_string());
            let node_id5 = graph.add_node(field5).unwrap();
            let edge4 = graph.add_edge(node_id5, edge3).unwrap();

            let neighbours = graph.neighbours(&edge3);
            assert_eq!(neighbours.len(), 1);
            assert!(neighbours.contains(&node_id5));
        }

        // TODO: get_neighbours in different subgraphs

        /// A simple case
        #[test]
        fn test_get_edges_1() {
            // Built graph:
            // ```text
            //  node1 --(edge)--> node2
            //   ^
            //   |
            // (edge2)
            //   |
            //  node3
            // ````
            let mut graph = Graph::default();

            let field1 = Field::String("node1".to_string());
            let field2 = Field::String("node2".to_string());
            let field3 = Field::String("node3".to_string());
            let node_id1 = graph.add_node(field1).unwrap();
            let node_id2 = graph.add_node(field2).unwrap();
            let node_id3 = graph.add_node(field3).unwrap();
            let edge1 = graph.add_edge(node_id1, node_id2).unwrap();
            let edge2 = graph.add_edge(node_id3, node_id1).unwrap();

            let edges = graph.edges(&node_id1);
            assert_eq!(edges.len(), 2);
            assert!(edges.contains(&edge1));
            assert!(edges.contains(&edge2));
        }

        // TODO: test_get_edges in different subgraphs

        #[test]
        fn test_add_edge() {
            let mut graph = Graph::default();
            let field1 = Field::String("node1".to_string());
            let field2 = Field::String("node2".to_string());
            let node_id1 = graph.add_node(field1).unwrap();
            let node_id2 = graph.add_node(field2).unwrap();

            let edge_result = graph.add_edge(node_id1, node_id2);
            assert!(edge_result.is_ok());
            let edge_id = edge_result.unwrap();
            assert!(graph.get_edge(&edge_id).is_some());
        }

        #[test]
        fn test_create_subgraph_and_get_it() {
            let mut graph = Graph::default();
            let graph2 = Graph::default();
            let graph3 = Graph::default();
            let mut path = PathBuf::from("subgraph1");
            graph.add_subgraph(&path, graph2).unwrap();
            path.push("subgraph2");
            graph.add_subgraph(&path, graph3).unwrap();
            assert!(graph.subgraph(&path).is_some());
        }

        #[test]
        fn test_get_field_from_subgraph() {
            let mut graph = Graph::default();
            let graph2 = Graph::default();
            let path = PathBuf::from("subgraph");
            graph.add_subgraph(&path, graph2).unwrap();
            let field = Field::String("test".to_string());
            let node_id = graph
                .subgraph(&path)
                .unwrap()
                .add_node(field.clone())
                .unwrap();
            assert_eq!(graph.subgraph(&path).unwrap().field(&node_id), Some(&field));
        }

        #[test]
        fn test_get_edge_beetween_subgraph_and_parent() {
            let mut graph = Graph::default();
            let graph2 = Graph::default();
            let path = PathBuf::from("subgraph");
            graph.add_subgraph(&path, graph2).unwrap();

            let field1 = Field::String("node1".to_string());
            let field2 = Field::String("node2".to_string());
            let node_id1 = graph.add_node(field1).unwrap();
            let node_id2 = graph.subgraph(&path).unwrap().add_node(field2).unwrap();

            let edge_id = graph.add_edge(node_id1, node_id2).unwrap();
            assert!(graph.get_edge(&edge_id).is_some());
        }

        #[test]
        fn test_get_edge_beetween_subgraphs_at_same_lvl() {
            unimplemented!()
        }

        #[test]
        fn test_get_node_of_edge_beetween_subgraphs_at_different_lvl() {
            unimplemented!()
        }

        #[test]
        fn test_get_node_of_edge_beetween_subgraph_and_parent() {
            unimplemented!()
        }

        #[test]
        fn test_get_node_of_edge_beetween_subgraphs_at_same_lvl() {
            unimplemented!()
        }

        #[test]
        fn test_get_edge_beetween_subgraphs_at_different_lvl() {
            unimplemented!()
        }
    }

    mod test_probs {
        use super::*;

        #[test]
        fn test_is_node_and_is_edge() {
            let mut graph = Graph::default();
            let field = Field::String("test".to_string());
            let node_id = graph.add_node(field).unwrap();
            assert!(graph.is_node(&node_id));
            assert!(!graph.is_edge(&node_id));

            let edge_id = graph.add_edge(node_id, node_id).unwrap();
            assert!(graph.is_edge(&edge_id));
        }

        #[test]
        fn test_is_existing_path() {
            let mut graph = Graph::default();
            let field1 = Field::String("node1".to_string());
            let field2 = Field::String("node2".to_string());
            let node_id1 = graph.add_node(field1).unwrap();
            let node_id2 = graph.add_node(field2).unwrap();
            graph.add_edge(node_id1, node_id2).unwrap();

            let field3 = Field::String("node3".to_string());
            let field4 = Field::String("node4".to_string());
            let node_id3 = graph.add_node(field3).unwrap();
            let node_id4 = graph.add_node(field4).unwrap();
            graph.add_edge(node_id3, node_id4).unwrap();

            graph.add_edge(node_id1, node_id4).unwrap();

            assert!(graph.is_existing_path(&node_id3, &node_id2).unwrap());
        }
    }

    mod bench {
        use super::*;

        /// TODO: after implementation of JIT, test that
        ///
        /// G.entities().filter(|id| G.is_edge(id)).collect()
        /// and
        /// G.edges().collect()
        /// should have same behavior after JIT
        /// and same speed
        fn eq_behavior1() {
            let mut graph = Graph::default();
            unimplemented!();
            graph
                .global_entities()
                .filter(|id| graph.is_edge(id))
                .for_each(|_| {});
            graph.global_edges().for_each(|_| {});
        }

        /// TODO: after implementation of JIT, test that
        ///
        /// G.entities().filter(|id| G.is_node(id)).collect()
        /// and
        /// G.nodes().collect()
        /// should have same behavior after JIT
        /// and same speed
        fn eq_behavior2() {
            let mut graph = Graph::default();
            unimplemented!();
            graph
                .global_entities()
                .filter(|id| graph.is_node(id))
                .for_each(|_| {});
            graph.global_nodes().for_each(|_| {});
        }
    }
    */
}
