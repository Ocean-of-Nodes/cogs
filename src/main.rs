use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use uuid::Uuid;

/// Entity is a common type for nodes and edges
type EntityId = Uuid;
/// Node is an ends of edges that's not edge,
/// but contains data as field
type NodeId = Uuid;
/// Edge is an entity that contains data as
/// field and link between two entities
type EdgeID = Uuid;

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
#[derive(Debug)]
struct NodeAlreadyExistsError(NodeId);
#[derive(Debug)]
struct NodeNotFoundError(NodeId);
#[derive(Debug)]
struct EdgeNotFoundError(EdgeID);
#[derive(Debug)]
struct MissingEndpointsError {
    missing_endpoints: Vec<Uuid>,
}

#[derive(Debug)]
struct IncorrectTypeError {
    node_id: NodeId,
    expected_type: String,
    actual_type: String,
}

#[derive(Debug)]
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

#[derive(Debug, Clone)]
enum ObjectDelta {
    AddField {
        name: String,
        field: Field,
    },
    RemoveField {
        name: String,
    },
    ReplaceField {
        name: String,
        field: Field,
    },
    ArrayDelta {
        name: String,
        removed_indices: Vec<usize>,
        added_fields: Vec<(usize, Field)>,
    },
    SubObjectDelta {
        /// Path is a slash-separated string representing the path to the nested object
        path: PathBuf,
        delta: Vec<ObjectDelta>,
    },
}

#[derive(Debug, Clone)]
enum RetrargetEdge {
    Source(Uuid),
    Target(Uuid),
}

#[derive(Debug, Clone)]
enum Patch {
    AddNode {
        id: Uuid,
        field: Field,
    },
    RemoveNode {
        id: Uuid,
    },
    /// When field fundamental type replaced
    ReplaceNode {
        id: Uuid,
        field: Field,
    },
    /// Diff for object node, contains list of changes in fields
    ChangeNode {
        id: Uuid,
        delta: Vec<ObjectDelta>,
    },

    AddEdge {
        id: Uuid,
        source: Uuid,
        target: Uuid,
    },
    RemoveEdge {
        id: Uuid,
    },
    RetrargetEdge {
        id: Uuid,
        new_target: RetrargetEdge,
    },
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Field {
    // Composite types
    Array(Vec<Field>),
    Object(HashMap<String, Field>),
    // Fundamental types
    String(String),
    Bool(bool),
    Number(i128),
    Null,
}

/// Triplet is a link beetween two entities
#[derive(Debug, Default)]
struct Triplet {
    id: EdgeID,
    source: EntityId,
    target: EntityId,
}

#[derive(Default)]
struct Graph {
    /// Triplet is a link beetween two entities
    edges: Vec<Triplet>,
    /// Storage for data attached to entity id.
    entities: HashMap<EntityId, Field>,

    /// Edges beetween subgraphs and parent graph
    /// or beetween subgraphs, regardless level for example
    /// parent have two subgraphs: subgraph1 and subgraph2,
    /// and edge beetween them, then it will be in beetween_edges
    beetween_edges: Vec<Triplet>,
    /// Storage for data of edges beetween subgraphs and
    /// parent graph or beetween subgraphs
    beetween_edges_entities: HashMap<Uuid, Field>,
    subgraphs: HashMap<String, Graph>,

    /// Listeners thats will trigger when graph or holded
    /// subgraph will be changed.
    listeners: HashMap<ListernerID, Box<dyn Fn(Patch)>>,
}

impl Graph {
    // ------------ START ROOTS ------------------- //
    // Roots are methods called on root graph

    /// Iterate over all entities of the whole graph
    pub fn global_entities(&self) -> impl Iterator<Item = EntityId> {
        let local_edges = self
            .edges
            .iter()
            .flat_map(|edge| [edge.id, edge.source, edge.target]);
        let beetween = self
            .beetween_edges
            .iter()
            .flat_map(|edge| [edge.id, edge.source, edge.target]);
        let mut iter: Box<dyn Iterator<Item = EntityId>> = Box::new(
            local_edges
                .chain(beetween)
                .chain(self.entities.keys().copied())
                .chain(self.beetween_edges_entities.keys().copied()),
        );

        for subgraph in self.subgraphs.values() {
            iter = Box::new(iter.chain(subgraph.global_entities()));
        }

        iter.collect::<HashSet<_>>().into_iter()
    }

    /// Iterate over all edges of the whole graph
    pub fn global_edges(&self) -> impl Iterator<Item = EdgeID> {
        let mut iter: Option<Box<dyn Iterator<Item = EdgeID>>> = None;

        let edges = self.edges.iter().map(|edge| edge.id);
        let beetween_edges = self.beetween_edges.iter().map(|edge| edge.id);
        iter = Some(Box::new(edges.chain(beetween_edges)));

        for subgraph in self.subgraphs.values() {
            let subgraph_edges = subgraph.global_edges();
            iter = Some(Box::new(iter.unwrap().chain(subgraph_edges)));
        }

        iter.unwrap()
    }

    /// Iterate over all nodes of the whole graph
    pub fn global_nodes(&self) -> impl Iterator<Item = NodeId> {
        let edges = self.global_edges().collect::<HashSet<EdgeID>>();
        let mut iter: Box<dyn Iterator<Item = NodeId>> = Box::new(
            self.entities
                .keys()
                .copied()
                .filter(move |id| !edges.contains(id)),
        );

        for subgraph in self.subgraphs.values() {
            let subgraph_nodes = subgraph.global_nodes();
            iter = Box::new(iter.chain(subgraph_nodes));
        }

        iter
    }

    /// Get subgraph by path, returns None if subgraph doesn't exist
    pub fn subgraph(&mut self, path: &PathBuf) -> Option<&mut Graph> {
        let mut current_graph = self;
        for chank in path.iter() {
            current_graph = current_graph.subgraphs.get_mut(chank.to_str()?)?;
        }
        Some(current_graph)
    }

    /* ------------ START GETTERS ------------------- */

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
            let subgraph_edges = graph.edges(id);
            edges.extend(subgraph_edges);
        }

        Ok(edges)
    }

    /// Get field by entity id from the whole graph (including subgraphs),
    /// returns None if field doesn't exist
    pub fn field(&self, id: &EntityId) -> Option<&Field> {
        if let Some(field) = self.entities.get(id) {
            return Some(field);
        }

        if let Some(field) = self.beetween_edges_entities.get(id) {
            return Some(field);
        }

        for graph in self.subgraphs.values() {
            if let Some(field) = graph.field(id) {
                return Some(field);
            }
        }
        None
    }

    /// Get triplet by id from the whole graph (including subgraphs),
    /// returns None if edge doesn't exist
    pub fn get_edge(&self, id: &EdgeID) -> Result<&Triplet, IncorrectTypeError> {
        if !self.is_edge(id) {
            return Err(IncorrectTypeError {
                node_id: *id,
                expected_type: "Edge".to_string(),
                actual_type: match self.field(id) {
                    Some(field) => format!("{:?}", field),
                    _ => "None".to_string(),
                },
            });
        }

        if let Some(triplet) = self.edges.iter().find(|triplet| &triplet.id == id) {
            return Some(triplet);
        }

        if let Some(triplet) = self.beetween_edges.iter().find(|triplet| &triplet.id == id) {
            return Some(triplet);
        }

        for graph in self.subgraphs.values() {
            if let Some(triplet) = graph.get_edge(id) {
                return Some(triplet);
            }
        }
        None
    }
    /* ------------ END GETTERS -------------------- */

    /* ------------ START PROBS -------------------- */

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

    /// Check if id is node or edge from the whole graph,
    /// returns true if node or edge exists, false otherwise
    pub fn is_exist(&self, id: &EntityId) -> bool {
        self.is_node(id) || self.is_edge(id)
    }

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
    fn add_subgraph(&mut self, name: &PathBuf, graph: Graph) -> Result<(), UnexistentPathError> {
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
        current_graph.subgraphs.insert(subgraph_name, graph);
        Ok(())
    }

    fn __add_node_with_id(&mut self, id: Uuid, field: Field) -> Result<(), NodeAlreadyExistsError> {
        if self.field(&id).is_some() {
            return Err(NodeAlreadyExistsError(id));
        }

        self.entities.insert(id, field);
        Ok(())
    }

    pub fn add_node(&mut self, field: Field) -> Result<Uuid, NodeAlreadyExistsError> {
        let id = Uuid::new_v4();
        self.__add_node_with_id(id, field).map(|_| id)
    }

    fn __add_edge_with_id(
        &mut self,
        id: Uuid,
        source: Uuid,
        target: Uuid,
    ) -> Result<(), AddEdgeError> {
        if self.get_edge(&id).is_some() {
            return Err(AddEdgeError::EdgeAlreadyExists(id));
        }

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

        let triplet = Triplet { source, target, id };
        let source_local =
            self.entities.contains_key(&source) || self.edges.iter().any(|t| t.id == source);
        let target_local =
            self.entities.contains_key(&target) || self.edges.iter().any(|t| t.id == target);
        if source_local && target_local {
            self.edges.push(triplet);
        } else {
            self.beetween_edges.push(triplet);
        }
        Ok(())
    }

    pub fn add_edge(&mut self, source: Uuid, target: Uuid) -> Result<EdgeID, AddEdgeError> {
        let edge_id = Uuid::new_v4();
        self.__add_edge_with_id(edge_id, source, target)
            .map(|_| edge_id)?;
        Ok(edge_id)
    }
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
            Patch::AddNode { id, field } => self
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
}

mod tests {
    use super::*;

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
        fn test_add_node() {
            let mut graph = Graph::default();
            let field = Field::String("test".to_string());
            let result = graph.add_node(field.clone());
            assert!(result.is_ok());
            let node_id = result.unwrap();
            assert_eq!(graph.field(&node_id), Some(&field));
        }

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
}

fn main() {
    println!("Hello, world!");
}
