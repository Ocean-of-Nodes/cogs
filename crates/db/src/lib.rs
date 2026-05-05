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
    
    /* ------------ END PROBS -------------------- */

    /* ------------ START CONSTRUCTORS ----------- */
    
    fn create_hyperedge(&mut self, members: Vec<Pointee>) -> HyperEdgeId {
        let id = Uuid::new_v4();
        self.hyper_edge.insert(id, members);
        id
    }

    fn __create_hyperedge_with_id(&mut self, id: HyperEdgeId, members: Vec<Pointee>) {
        self.hyper_edge.insert(id, members);
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
            Patch::CreateHyperEdge { id, members } => {
                self.__create_hyperedge_with_id(id, members);
                Ok(())
            }
            _ => Err(ApplyPatchError::NotImplemented),
        }
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

    pub fn remove_hyperedge() {
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
