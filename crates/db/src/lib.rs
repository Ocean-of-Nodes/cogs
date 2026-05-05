mod incidence;
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
    missing_endpoints: Vec<Pointee>,
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
    hyper_edge: HashMap<HyperEdgeId, Vec<Pointee>>,
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

    /// Return all simple paths from `lhs` to `rhs` packed into a
    /// single sub-graph. The direction of each edge is **ignored** —
    /// every edge can be traversed both ways. The result is the
    /// union of nodes and edges that lie on at least one such path,
    /// with edges keeping their original `(source, target)`
    /// orientation (we only relax direction during traversal, not in
    /// the returned graph).
    ///
    /// "Simple" means each node is visited at most once on a single
    /// path. Parallel edges between the same pair of nodes contribute
    /// distinct paths and all show up in the result.
    pub fn undirected_paths(&mut self, lhs: NodeId, rhs: NodeId) -> Graph {
        let mut result = Graph::default();

        if lhs == rhs {
            if let Some(obj) = self.entities.get(&lhs) {
                result.entities.insert(lhs, obj.clone());
            }
            return result;
        }

        let lhs_p = Pointee::EntityId(lhs);
        let rhs_p = Pointee::EntityId(rhs);

        // Adjacency: each edge contributes BOTH directions so DFS
        // can traverse against the original `source → target`.
        let mut adj: HashMap<Pointee, Vec<(EdgeID, Pointee)>> = HashMap::new();
        for (eid, (s, t)) in &self.edges {
            adj.entry(s.clone()).or_default().push((*eid, t.clone()));
            adj.entry(t.clone()).or_default().push((*eid, s.clone()));
        }
        let mut visited: HashSet<Pointee> = HashSet::from([lhs_p.clone()]);
        // Stack only holds edge ids; original source/target are
        // looked up in `self.edges` when a path is recorded, so the
        // result preserves the real direction regardless of how DFS
        // walked the edge.
        let mut stack: Vec<EdgeID> = Vec::new();

        fn dfs(
            cur: &Pointee,
            target: &Pointee,
            adj: &HashMap<Pointee, Vec<(EdgeID, Pointee)>>,
            edges_table: &HashMap<EdgeID, (Pointee, Pointee)>,
            entities: &HashMap<EntityId, Object>,
            visited: &mut HashSet<Pointee>,
            stack: &mut Vec<EdgeID>,
            result: &mut Graph,
        ) {
            let Some(outgoing) = adj.get(cur) else {
                return;
            };
            for (eid, next) in outgoing {
                if visited.contains(next) {
                    continue;
                }
                stack.push(*eid);
                if next == target {
                    for path_eid in &*stack {
                        if let Some((s, t)) = edges_table.get(path_eid) {
                            result.edges.insert(*path_eid, (s.clone(), t.clone()));
                            copy_entity_obj(s, entities, &mut result.entities);
                            copy_entity_obj(t, entities, &mut result.entities);
                        }
                    }
                } else {
                    visited.insert(next.clone());
                    dfs(next, target, adj, edges_table, entities, visited, stack, result);
                    visited.remove(next);
                }
                stack.pop();
            }
        }

        fn copy_entity_obj(
            p: &Pointee,
            src_entities: &HashMap<EntityId, Object>,
            dst_entities: &mut HashMap<EntityId, Object>,
        ) {
            if let Pointee::EntityId(id) = p {
                if let Some(obj) = src_entities.get(id) {
                    dst_entities.insert(*id, obj.clone());
                }
            }
        }

        dfs(
            &lhs_p,
            &rhs_p,
            &adj,
            &self.edges,
            &self.entities,
            &mut visited,
            &mut stack,
            &mut result,
        );

        result
    }

    /// Return all simple directed paths from `lhs` to `rhs` packed
    /// into a single sub-graph: the union of nodes and edges that
    /// lie on at least one path. Edges shared by several paths
    /// appear once. Direction is honoured (`source → target` only);
    /// hyperedges are ignored.
    ///
    /// "Simple" means no node is visited twice on a single path —
    /// cycles never blow up the search.
    ///
    /// If no path exists the result is empty. If `lhs == rhs` the
    /// result is a singleton graph with just `lhs`.
    ///
    /// # Complexity
    ///
    /// In the worst case the number of simple paths is exponential
    /// in the number of nodes, so this is a heavy operation on
    /// dense graphs. It's fine for analysis-style queries on small
    /// neighbourhoods.
    pub fn directed_paths(&mut self, lhs: NodeId, rhs: NodeId) -> Graph {
        let mut result = Graph::default();

        if lhs == rhs {
            if let Some(obj) = self.entities.get(&lhs) {
                result.entities.insert(lhs, obj.clone());
            }
            return result;
        }

        let lhs_p = Pointee::EntityId(lhs);
        let rhs_p = Pointee::EntityId(rhs);

        // Adjacency: source → [(edge_id, target), ...]. Built once
        // in O(E); turns BFS/DFS into O(V + E) instead of O(V * E).
        let mut adj: HashMap<Pointee, Vec<(EdgeID, Pointee)>> = HashMap::new();
        for (eid, (s, t)) in &self.edges {
            adj.entry(s.clone()).or_default().push((*eid, t.clone()));
        }
        let mut visited: HashSet<Pointee> = HashSet::from([lhs_p.clone()]);
        let mut stack: Vec<(EdgeID, Pointee, Pointee)> = Vec::new();

        // Recursive DFS, enumerating simple paths. On every successful
        // arrival at `rhs`, the entire current path is dumped into
        // `result` (which dedupes via HashMap).
        fn dfs(
            cur: &Pointee,
            target: &Pointee,
            adj: &HashMap<Pointee, Vec<(EdgeID, Pointee)>>,
            entities: &HashMap<EntityId, Object>,
            visited: &mut HashSet<Pointee>,
            stack: &mut Vec<(EdgeID, Pointee, Pointee)>,
            result: &mut Graph,
        ) {
            fn copy_entity_obj(
                p: &Pointee,
                src_entities: &HashMap<EntityId, Object>,
                dst_entities: &mut HashMap<EntityId, Object>,
            ) {
                if let Pointee::EntityId(id) = p {
                    if let Some(obj) = src_entities.get(id) {
                        dst_entities.insert(*id, obj.clone());
                    }
                }
            }

            let Some(outgoing) = adj.get(cur) else {
                return;
            };
            for (eid, next) in outgoing {
                if visited.contains(next) {
                    continue;
                }
                stack.push((*eid, cur.clone(), next.clone()));
                if next == target {
                    for (eid, src, tgt) in &*stack {
                        result.edges.insert(*eid, (src.clone(), tgt.clone()));
                        copy_entity_obj(src, entities, &mut result.entities);
                        copy_entity_obj(tgt, entities, &mut result.entities);
                    }
                } else {
                    visited.insert(next.clone());
                    dfs(next, target, adj, entities, visited, stack, result);
                    visited.remove(next);
                }
                stack.pop();
            }
        }

        dfs(
            &lhs_p,
            &rhs_p,
            &adj,
            &self.entities,
            &mut visited,
            &mut stack,
            &mut result,
        );

        result
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
    pub fn hyperedge_members(&self, id: &HyperEdgeId) -> Option<&Vec<Pointee>> {
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
    fn navigate_object<'a>(obj: &'a Object, local: &LocalPath) -> Option<&'a Field> {
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
    /// - `reachable(e1, e4)` → `Ok(true)`  — via `e1 — e3 — e2 — e5 — e4`
    ///   (same route as `is_existing_path`).
    /// - `reachable(n5, n7)` → `Ok(true)`  — via `n5 — e4 — n6 — e6 — n7`
    ///   (same route as `is_existing_path`).
    /// - `reachable(e1, n6)` → `Ok(true)`  — via
    ///   `e1 — e3 — e2 — e5 — e4 — n6`. The same query is `Ok(false)`
    ///   for [`Graph::is_existing_path`], which forbids crossing
    ///   between an edge and a node.
    ///
    /// # Errors
    ///
    /// Returns [`MissingEndpointsError`] if `source` or `target` does
    /// not exist anywhere in this graph or its subgraphs.
    pub fn reachable(
        &self,
        source: &EntityId,
        target: &EntityId,
    ) -> Result<bool, MissingEndpointsError> {
        self.__ensure_endpoints_exist(source, target)?;

        unimplemented!()
    }
    */
    /* ------------ END PROBS -------------------- */

    /* ------------ START CONSTRUCTORS ----------- */
    
    fn create_hyperedge(&mut self, members: Vec<Pointee>) -> HyperEdgeId {
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

            let h = g.create_hyperedge(vec![n1.into(), n3.into()]);
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

            let h = graph.create_hyperedge(vec![n3.into(), n4.into()]);
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

                let h = g.create_hyperedge(vec![n1.into(), n2.into()]);
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
        }

        mod test_undirected_paths {
            use super::*;

            /// Direction is ignored, so even though `e2` is recorded
            /// `n3 → n1`, the undirected walk uses it as `n1 → n3`.
            /// All four edges should appear in the result.
            #[test]
            fn undirected_paths() {
                let (mut graph, n1, n2, n3, e1, e2, e3, e4) =
                    test_utils::create_semple_graph3();

                let result = graph.undirected_paths(n1, n2);

                let result_nodes: HashSet<_> = result.iter_nodes().collect();
                let result_edges: HashSet<_> = result.iter_edges().collect();

                assert_eq!(result_nodes, [n1, n2, n3].into_iter().collect());
                assert_eq!(result_edges, [e1, e2, e3, e4].into_iter().collect());
            }
        }

        mod test_directed_paths {
            use super::*;

            /// Direction matters. The only `n1 → n2` path is `e1`:
            /// `e2` goes `n3 → n1` (wrong way), so we never reach
            /// `n3` from `n1`, and `e3`/`e4` are unreachable.
            #[test]
            fn directed_paths() {
                let (mut graph, n1, n2, _n3, e1, _e2, _e3, _e4) =
                    test_utils::create_semple_graph3();

                let result = graph.directed_paths(n1, n2);

                let result_nodes: HashSet<_> = result.iter_nodes().collect();
                let result_edges: HashSet<_> = result.iter_edges().collect();

                assert_eq!(result_nodes, [n1, n2].into_iter().collect());
                assert_eq!(result_edges, [e1].into_iter().collect());
            }
        }

        mod test_edge {
            use super::*;

            #[test]
            fn edge() {

            }

             // TODO: test_get_edges in different subgraphs
             /* 
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
                fn test_get_edge_beetween_subgraphs_at_different_lvl() {
                    unimplemented!()
                }
             */
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
                let obj = test_utils::create_simple_obj("test_field");
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
    /*
    mod test_constructors_and_getters {
        use super::*;

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
