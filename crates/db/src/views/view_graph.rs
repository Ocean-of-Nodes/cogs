//! [`ViewGraph`]: the graph as the running invocation sees it.
//!
//! - Every read records [`DepKey`]s, whatever it returns.
//! - Outputs of the previous run are hidden until the run writes
//!   them again, so a re-run sees what a first run would see.
//! - Writes create entities with derived ids. An id that the
//!   previous run produced is reused, and only a real difference
//!   reaches the graph.
//! - Only entities written in this run can be changed or removed;
//!   for anything else the methods fail as for a missing entity.
//! - Lists of ids are sorted, so a view sees them in a stable order.

use std::collections::{HashMap, HashSet};

use common::*;

use crate::errors::{
    AddEdgeError, AddHyperedgeMembersError, AttachObjectError, AttachTargetNotFoundError,
    CreateHyperedgeError, EdgeNotFoundError, EntityNotFoundError, GetEdgeError,
    HyperedgeNotFoundError, InvalidRetargetError, MissingEndpointsError, NoAttachedObjectError,
    NodeNotFoundError, PointeesNotFoundError, RemoveHyperedgeMembersError, RetargetError,
};
use crate::graph::Graph;
use crate::types::EdgeView;
use crate::views::InvocationId;
use crate::views::deps::DepKey;
use crate::views::ids::{Local, derive_id};

/// State of the invocation that is running.
pub(crate) struct Run {
    pub(crate) invocation: InvocationId,
    /// Name of the view, part of every derived id.
    view: String,
    /// Outputs of the previous run that this run has not written
    /// again. Hidden from reads; removed when the run ends.
    pub(crate) stale: HashSet<EntityId>,
    /// Own edges and hyperedges whose attached object is left over
    /// from the previous run. The object is hidden until attached
    /// again; removed when the run ends.
    pub(crate) stale_attached: HashSet<AttachTargetId>,
    /// Entities written in this run: the only ones it may change.
    pub(crate) written: HashSet<EntityId>,
    /// Read set of this run.
    pub(crate) deps: HashSet<DepKey>,
    nodes: u64,
    hyperedges: u64,
    edges: HashMap<(Pointee, Pointee), u64>,
}

impl Run {
    pub(crate) fn new(
        invocation: InvocationId,
        view: String,
        stale: HashSet<EntityId>,
        stale_attached: HashSet<AttachTargetId>,
    ) -> Self {
        Self {
            invocation,
            view,
            stale,
            stale_attached,
            written: HashSet::new(),
            deps: HashSet::new(),
            nodes: 0,
            hyperedges: 0,
            edges: HashMap::new(),
        }
    }

    fn derive(&self, local: Local<'_>) -> EntityId {
        derive_id(&self.view, self.invocation.key.as_ref(), local)
    }
}

/// The graph as the running invocation sees it. Mirrors the
/// [`Graph`] API.
pub struct ViewGraph<'a> {
    pub(crate) graph: &'a mut Graph,
    pub(crate) run: &'a mut Run,
}

impl ViewGraph<'_> {
    /// The invocation this graph belongs to.
    pub fn invocation(&self) -> InvocationId {
        self.run.invocation
    }

    // ------------ VISIBILITY ------------ //

    fn read(&mut self, key: DepKey) {
        self.run.deps.insert(key);
    }

    fn read_pointee(&mut self, p: &Pointee) {
        self.run.deps.extend(DepKey::pointee(p));
    }

    fn hidden(&self, id: &EntityId) -> bool {
        self.run.stale.contains(id)
    }

    fn visible_obj(&self, id: &EntityId) -> Option<&Object> {
        if self.hidden(id) || self.run.stale_attached.contains(id) {
            return None;
        }
        self.graph.obj(id)
    }

    fn visible_pointee(&self, p: &Pointee) -> bool {
        match p {
            Pointee::EntityId(id) => !self.hidden(id) && self.graph.is_exist(id),
            Pointee::Path(path) => self
                .visible_obj(&path.entity())
                .and_then(|obj| Graph::walk_path(obj, path.local()))
                .is_some(),
        }
    }

    /// `ids` without hidden ones, sorted.
    fn visible_ids(&self, ids: impl Iterator<Item = EntityId>) -> Vec<EntityId> {
        let mut ids: Vec<EntityId> = ids.filter(|id| !self.hidden(id)).collect();
        ids.sort();
        ids
    }

    // ------------ QUERIES ------------ //

    pub fn iter_entities(&mut self) -> Vec<EntityId> {
        self.read(DepKey::AllEntities);
        self.visible_ids(self.graph.iter_entities())
    }

    pub fn iter_nodes(&mut self) -> Vec<NodeId> {
        self.read(DepKey::AllNodes);
        self.visible_ids(self.graph.iter_nodes())
    }

    pub fn iter_edges(&mut self) -> Vec<EdgeId> {
        self.read(DepKey::AllEdges);
        self.visible_ids(self.graph.iter_edges())
    }

    pub fn iter_hyperedges(&mut self) -> Vec<HyperedgeId> {
        self.read(DepKey::AllHyperedges);
        self.visible_ids(self.graph.iter_hyperedges())
    }

    pub fn iter_attached(&mut self) -> Vec<AttachTargetId> {
        self.read(DepKey::AllAttached);
        let attached = self
            .graph
            .iter_attached()
            .filter(|id| !self.run.stale_attached.contains(id));
        self.visible_ids(attached)
    }

    pub fn obj(&mut self, id: &EntityId) -> Option<Object> {
        self.read(DepKey::Obj(*id));
        self.visible_obj(id).cloned()
    }

    /// One top-level field of the object at `id`. Depends on that
    /// field only, unlike [`ViewGraph::obj`].
    pub fn field(&mut self, id: &EntityId, name: &str) -> Option<Field> {
        self.read(DepKey::Field(*id, name.to_string()));
        self.visible_obj(id)?.get(name).cloned()
    }

    pub fn edge(&mut self, id: &EdgeId) -> Result<EdgeView, GetEdgeError> {
        self.read(DepKey::Entity(*id));
        self.read(DepKey::Edge(*id));
        if self.hidden(id) {
            return Err(GetEdgeError::NotFound(EntityNotFoundError { id: *id }));
        }
        self.graph.edge(id)
    }

    pub fn hyperedge_members(&mut self, id: &HyperedgeId) -> Option<HashSet<Pointee>> {
        self.read(DepKey::Members(*id));
        if self.hidden(id) {
            return None;
        }
        let members = self.graph.hyperedge_members(id)?;
        Some(
            members
                .iter()
                .filter(|m| self.visible_pointee(m))
                .cloned()
                .collect(),
        )
    }

    pub fn classify_pointee(&mut self, p: &Pointee) -> Option<PointeeKind> {
        self.read_pointee(p);
        let Pointee::EntityId(id) = p else {
            return self.visible_pointee(p).then_some(PointeeKind::Subobject);
        };
        // Whether an edge is a metaedge depends on its endpoints.
        self.read(DepKey::Edge(*id));
        if self.hidden(id) {
            return None;
        }
        if self.run.stale_attached.contains(id) {
            return self.graph.structural_type(*id).map(PointeeKind::from);
        }
        self.graph.classify_pointee(p)
    }

    pub fn is_exist(&mut self, id: &EntityId) -> bool {
        self.read(DepKey::Entity(*id));
        !self.hidden(id) && self.graph.is_exist(id)
    }

    pub fn is_pointee_exist(&mut self, p: &Pointee) -> bool {
        self.read_pointee(p);
        self.visible_pointee(p)
    }

    /// Edges whose source is `p`.
    pub fn out_edges(&mut self, p: &Pointee) -> Vec<EdgeId> {
        self.read(DepKey::OutEdges(p.clone()));
        self.visible_ids(self.graph.out_edges(p))
    }

    /// Edges whose target is `p`.
    pub fn in_edges(&mut self, p: &Pointee) -> Vec<EdgeId> {
        self.read(DepKey::InEdges(p.clone()));
        self.visible_ids(self.graph.in_edges(p))
    }

    /// Hyperedges that have `p` as a member.
    pub fn hyperedges_of(&mut self, p: &Pointee) -> Vec<HyperedgeId> {
        self.read(DepKey::HyperedgesOf(p.clone()));
        self.visible_ids(self.graph.hyperedges_of(p))
    }

    /// Nodes whose object has top-level `field` equal to `value`.
    /// Depends on that pair, so a node that starts matching later
    /// re-runs the invocation even if nothing matched now.
    pub fn nodes_where(&mut self, field: &str, value: &Field) -> Vec<NodeId> {
        self.read(DepKey::NodesWhere(field.to_string(), value.clone()));
        self.visible_ids(self.graph.nodes_where(field, value))
    }

    // ------------ CONSTRUCTORS ------------ //

    /// Add a node carrying `obj`. If the previous run created the
    /// same node, it is kept and only a changed object is written.
    pub fn add_node(&mut self, obj: Object) -> NodeId {
        let id = self.run.derive(Local::Node(self.run.nodes));
        self.run.nodes += 1;

        if self.run.stale.remove(&id) && self.graph.is_node(&id) {
            if self.graph.obj(&id) != Some(&obj) {
                self.graph
                    .replace_node(&id, obj)
                    .expect("checked: the node exists");
            }
        } else {
            self.graph
                .add_node_with_id(id, obj)
                .expect("derived ids are unique");
        }
        self.run.written.insert(id);
        id
    }

    /// Add an edge from `source` to `target`; both must be visible.
    /// If the previous run created the same edge, it is kept.
    pub fn add_edge(
        &mut self,
        source: impl Into<Pointee>,
        target: impl Into<Pointee>,
    ) -> Result<EdgeId, AddEdgeError> {
        let (source, target) = (source.into(), target.into());
        self.read_pointee(&source);
        self.read_pointee(&target);
        let n = {
            let count = self
                .run
                .edges
                .entry((source.clone(), target.clone()))
                .or_default();
            *count += 1;
            *count - 1
        };
        let id = self.run.derive(Local::Edge(&source, &target, n));

        let missing_endpoints: Vec<Pointee> = [&source, &target]
            .into_iter()
            .filter(|p| !self.visible_pointee(p))
            .cloned()
            .collect();
        if !missing_endpoints.is_empty() {
            return Err(AddEdgeError::MissingEndpoints(MissingEndpointsError {
                missing_endpoints,
            }));
        }

        if self.run.stale.remove(&id) && self.graph.edges.contains_key(&id) {
            // A retarget in the previous run is undone.
            let (old_source, old_target) = self.graph.edges[&id].clone();
            if old_source != source {
                self.graph
                    .retarget_edge(&id, RetargetEdge::Source(source))
                    .expect("checked: the edge and the endpoint exist");
            }
            if old_target != target {
                self.graph
                    .retarget_edge(&id, RetargetEdge::Target(target))
                    .expect("checked: the edge and the endpoint exist");
            }
        } else {
            self.graph.add_edge_with_id(id, source, target)?;
        }
        self.run.written.insert(id);
        Ok(id)
    }

    /// Create a hyperedge over `members`; all must be visible. If the
    /// previous run created the same hyperedge, it is kept and only
    /// the difference in members is written.
    pub fn create_hyperedge(
        &mut self,
        members: HashSet<Pointee>,
    ) -> Result<HyperedgeId, CreateHyperedgeError> {
        for m in &members {
            self.read_pointee(m);
        }
        let id = self.run.derive(Local::Hyperedge(self.run.hyperedges));
        self.run.hyperedges += 1;

        if members.is_empty() {
            return Err(CreateHyperedgeError::EmptyHyperedge);
        }
        let pointees: HashSet<Pointee> = members
            .iter()
            .filter(|m| !self.visible_pointee(m))
            .cloned()
            .collect();
        if !pointees.is_empty() {
            return Err(CreateHyperedgeError::PointeesNotFound(
                PointeesNotFoundError { pointees },
            ));
        }

        if self.run.stale.remove(&id) && self.graph.hyperedges.contains_key(&id) {
            let current = self.graph.hyperedges[&id].clone();
            let added: HashSet<Pointee> = members.difference(&current).cloned().collect();
            let removed: HashSet<Pointee> = current.difference(&members).cloned().collect();
            // Add first, so the hyperedge never becomes empty.
            if !added.is_empty() {
                self.graph
                    .add_hyperedge_members(id, added)
                    .expect("checked: the hyperedge and the members exist");
            }
            if !removed.is_empty() {
                self.graph
                    .remove_hyperedge_members(id, removed)
                    .expect("checked: these are members");
            }
        } else {
            self.graph.create_hyperedge_with_id(id, members)?;
        }
        self.run.written.insert(id);
        Ok(id)
    }

    // ------------ DESTRUCTORS ------------ //

    pub fn remove_node(&mut self, id: &NodeId) -> Result<Object, NodeNotFoundError> {
        if !self.run.written.contains(id) {
            return Err(NodeNotFoundError { id: *id });
        }
        let obj = self.graph.remove_node(id)?;
        self.run.written.remove(id);
        Ok(obj)
    }

    pub fn remove_edge(&mut self, id: &EdgeId) -> Result<EdgeView, EdgeNotFoundError> {
        if !self.run.written.contains(id) {
            return Err(EdgeNotFoundError { id: *id });
        }
        let view = self.graph.remove_edge(id)?;
        self.run.written.remove(id);
        self.run.stale_attached.remove(id);
        Ok(view)
    }

    pub fn remove_hyperedge(
        &mut self,
        id: &HyperedgeId,
    ) -> Result<HashSet<Pointee>, HyperedgeNotFoundError> {
        if !self.run.written.contains(id) {
            return Err(HyperedgeNotFoundError { id: *id });
        }
        let members = self.graph.remove_hyperedge(id)?;
        self.run.written.remove(id);
        self.run.stale_attached.remove(id);
        Ok(members)
    }

    pub fn remove_attached(&mut self, id: AttachTargetId) -> Result<(), NoAttachedObjectError> {
        if !self.run.written.contains(&id) || self.run.stale_attached.contains(&id) {
            return Err(NoAttachedObjectError { id });
        }
        self.graph.remove_attached(id)
    }

    // ------------ MODIFIERS ------------ //

    /// Attach `obj` to an edge or hyperedge written in this run. An
    /// object attached by the previous run is replaced only if it
    /// differs.
    pub fn attach_obj(&mut self, id: AttachTargetId, obj: Object) -> Result<(), AttachObjectError> {
        if !self.run.written.contains(&id) {
            return Err(AttachObjectError::AttachTargetNotFound(
                AttachTargetNotFoundError { id },
            ));
        }
        if self.run.stale_attached.remove(&id) {
            if self.graph.obj(&id) != Some(&obj) {
                self.graph
                    .replace_attached_obj(&id, obj)
                    .expect("checked: an object is attached");
            }
            return Ok(());
        }
        self.graph.attach_obj(id, obj)
    }

    pub fn add_hyperedge_members(
        &mut self,
        id: HyperedgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), AddHyperedgeMembersError> {
        for p in &m {
            self.read_pointee(p);
        }
        if !self.run.written.contains(&id) {
            return Err(AddHyperedgeMembersError::HyperedgeNotFound(
                HyperedgeNotFoundError { id },
            ));
        }
        let pointees: HashSet<Pointee> = m
            .iter()
            .filter(|p| !self.visible_pointee(p))
            .cloned()
            .collect();
        if self.graph.hyperedges.contains_key(&id) && !pointees.is_empty() {
            return Err(AddHyperedgeMembersError::PointeesNotFound(
                PointeesNotFoundError { pointees },
            ));
        }
        self.graph.add_hyperedge_members(id, m)
    }

    pub fn remove_hyperedge_members(
        &mut self,
        id: HyperedgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), RemoveHyperedgeMembersError> {
        if !self.run.written.contains(&id) {
            return Err(RemoveHyperedgeMembersError::HyperedgeNotFound(
                HyperedgeNotFoundError { id },
            ));
        }
        self.graph.remove_hyperedge_members(id, m)
    }

    pub fn replace_node(&mut self, id: &NodeId, obj: Object) -> Result<Field, NodeNotFoundError> {
        if !self.run.written.contains(id) {
            return Err(NodeNotFoundError { id: *id });
        }
        self.graph.replace_node(id, obj)
    }

    pub fn replace_attached_obj(
        &mut self,
        id: &AttachTargetId,
        obj: Object,
    ) -> Result<Field, NoAttachedObjectError> {
        if !self.run.written.contains(id) || self.run.stale_attached.contains(id) {
            return Err(NoAttachedObjectError { id: *id });
        }
        self.graph.replace_attached_obj(id, obj)
    }

    pub fn retarget_edge(
        &mut self,
        id: &EdgeId,
        new_target: RetargetEdge,
    ) -> Result<(), RetargetError> {
        let new_pointee = match &new_target {
            RetargetEdge::Source(p) | RetargetEdge::Target(p) => p.clone(),
        };
        self.read_pointee(&new_pointee);
        if !self.run.written.contains(id) {
            return Err(RetargetError::EdgeNotFound(EdgeNotFoundError { id: *id }));
        }
        if self.graph.edges.contains_key(id) && !self.visible_pointee(&new_pointee) {
            return Err(RetargetError::InvalidTarget(InvalidRetargetError {
                edge_id: *id,
                new_target,
            }));
        }
        self.graph.retarget_edge(id, new_target)
    }
}
