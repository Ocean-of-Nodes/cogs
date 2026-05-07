//! Raw graph storage. No invariant checks, no cascade, no trackers.
//!
//! `db` will eventually wrap this with semantic operations. The JIT
//! may bypass `db` and call `Storage` directly when static analysis
//! proves checks are redundant.

use std::collections::{HashMap, HashSet};

use common::{EdgeId, EntityId, HyperedgeId, NodeId, Object, Pointee};
use uuid::Uuid;

#[derive(Default, Debug)]
pub struct Storage {
    nodes: HashMap<NodeId, Object>,
    edges: HashMap<EdgeId, (Pointee, Pointee)>,
    hyperedges: HashMap<HyperedgeId, HashSet<Pointee>>,
}

impl Storage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put_node(&mut self, id: NodeId, obj: Object) {
        self.nodes.insert(id, obj);
    }

    pub fn get_node(&self, id: &NodeId) -> Option<&Object> {
        self.nodes.get(id)
    }

    pub fn remove_node(&mut self, id: &NodeId) -> Option<Object> {
        self.nodes.remove(id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = (&NodeId, &Object)> {
        self.nodes.iter()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn put_edge(&mut self, id: EdgeId, source: Pointee, target: Pointee) {
        self.edges.insert(id, (source, target));
    }

    pub fn get_edge(&self, id: &EdgeId) -> Option<&(Pointee, Pointee)> {
        self.edges.get(id)
    }

    pub fn edges(&self) -> impl Iterator<Item = (&EdgeId, &(Pointee, Pointee))> {
        self.edges.iter()
    }

    pub fn put_hyperedge(&mut self, id: HyperedgeId, members: HashSet<Pointee>) {
        self.hyperedges.insert(id, members);
    }

    pub fn get_hyperedge(&self, id: &HyperedgeId) -> Option<&HashSet<Pointee>> {
        self.hyperedges.get(id)
    }
}

/// Mint a fresh entity id. Storage doesn't care about uniqueness across
/// entity kinds — node/edge/hyperedge ids share the same UUID space.
pub fn fresh_id() -> EntityId {
    Uuid::new_v4()
}