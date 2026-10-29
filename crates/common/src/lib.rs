pub mod abi;
pub mod global_path;
pub mod local_path;

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use uuid::Uuid;

use serde::{Deserialize, Serialize};

pub use global_path::GlobalObjPath;
pub use local_path::LocalObjPath;

/// Used by db for tracker's that's accumulate changes 
/// for caller that's want get it by next call
pub type TrackerId = Uuid;
/// Delta is array of `ThinPatch` — what goes over the network.
pub type Delta = Vec<ThinPatch>;
/// Array of `FullPatch` — what the db keeps in its log.
pub type FullDelta = Vec<FullPatch>;

/// Entity is a common type for nodes/edges/metaedge/hyperedge
pub type EntityId = Uuid;

/// What an edge endpoint or a `Field::Link` can point at — either
/// a whole entity (by id) or a sub-object inside an entity (by path).
#[derive(PartialEq, Eq, Debug, Clone, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Pointee {
    EntityId(EntityId),
    Path(GlobalObjPath),
}

impl From<EntityId> for Pointee {
    fn from(id: EntityId) -> Self {
        Pointee::EntityId(id)
    }
}

impl Pointee {
    /// The entity this pointee is, or lives inside of.
    pub fn entity(&self) -> EntityId {
        match self {
            Pointee::EntityId(id) => *id,
            Pointee::Path(path) => path.entity(),
        }
    }
}

/// Target edges/metaedge/hyperedge for thats object 
/// can be attached 
pub type AttachTargetId = Uuid;
/// Node is an ends of edges that's not edge,
/// but contains data as object
pub type NodeId = Uuid;
/// Edge is an entity that contains data as
/// object and link between two entities
pub type EdgeId = Uuid;
/// Hyper edge thats hold some entities
pub type HyperedgeId = Uuid;

pub type Object = HashMap<String, Field>;

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum Field {
    // ------- START COMPOSITE TYPES --------------
    Array(Vec<Field>),
    Object(Object),

    // ------- END COMPOSITE TYPES --------------

    // ------- START FUNDAMENTAL TYPES --------------
    String(String),
    Bool(bool),
    Number(i128),
    Link(Pointee),
    Null,
    // ------- END FUNDAMENTAL TYPES --------------
}

/// Consistent with `Eq`: an [`Object`] is hashed independently of
/// its iteration order, so equal objects hash equally.
impl Hash for Field {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Field::Array(items) => items.hash(state),
            Field::Object(obj) => {
                // Combine per-entry hashes with a commutative sum.
                let sum = obj.iter().fold(0u64, |acc, entry| {
                    let mut entry_hasher = DefaultHasher::new();
                    entry.hash(&mut entry_hasher);
                    acc.wrapping_add(entry_hasher.finish())
                });
                obj.len().hash(state);
                sum.hash(state);
            }
            Field::String(s) => s.hash(state),
            Field::Bool(b) => b.hash(state),
            Field::Number(n) => n.hash(state),
            Field::Link(p) => p.hash(state),
            Field::Null => {}
        }
    }
}

impl From<&str> for Field {
    fn from(s: &str) -> Self {
        Field::String(s.to_string())
    }
}

impl From<String> for Field {
    fn from(s: String) -> Self {
        Field::String(s)
    }
}

impl From<bool> for Field {
    fn from(b: bool) -> Self {
        Field::Bool(b)
    }
}

impl From<i32> for Field {
    fn from(n: i32) -> Self {
        Field::Number(n.into())
    }
}

impl From<i64> for Field {
    fn from(n: i64) -> Self {
        Field::Number(n.into())
    }
}

impl From<i128> for Field {
    fn from(n: i128) -> Self {
        Field::Number(n)
    }
}

impl From<Pointee> for Field {
    fn from(p: Pointee) -> Self {
        Field::Link(p)
    }
}

impl From<Object> for Field {
    fn from(obj: Object) -> Self {
        Field::Object(obj)
    }
}

/// Which keys a view runs for: `#[view(for_each = ...)]`. The view
/// gets one invocation per matching node, keyed by its id.
#[derive(PartialEq, Eq, Debug, Clone, Hash, Serialize, Deserialize)]
pub enum Pattern {
    /// Every node.
    Nodes,
    /// Nodes whose object has top-level `field` equal to `value`.
    NodesWhere { field: String, value: Field },
}

impl Pattern {
    pub fn nodes() -> Self {
        Pattern::Nodes
    }

    pub fn nodes_where(field: impl Into<String>, value: impl Into<Field>) -> Self {
        Pattern::NodesWhere {
            field: field.into(),
            value: value.into(),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum ObjectPatch {
    AddField {
        name: String,
        field: Field,
    },
    RemoveField {
        name: String,
    },
    UpsertField {
        name: String,
        field: Field,
    },
    ArrayPatch {
        name: String,
        removed_indices: Vec<usize>,
        added_fields: Vec<(usize, Field)>,
    },
    SubObjectPatch {
        /// Path is a slash-separated string representing the path
        /// to the nested object
        path: LocalObjPath,
        delta: Vec<ObjectPatch>,
    },
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum RetargetEdge {
    Source(Pointee),
    Target(Pointee),
}

/// Minimal patch: just enough to replay the change forward.
/// Sent over the network.
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum ThinPatch {
    // ------------- START NODE DELTA --------------
    AddNode {
        id: NodeId,
        obj: Object,
    },
    RemoveNode {
        id: NodeId,
    },
    ChangeNode {
        id: NodeId,
        delta: Vec<ObjectPatch>,
    },
    UpsertNode {
        id: NodeId,
         obj: Object,
    },

    // ------------- END NODE DELTA --------------

    // --------------- START EDGE DELTA ----------
    AddEdge {
        id: EdgeId,
        source: Pointee,
        target: Pointee,
    },
    RemoveEdge {
        id: EdgeId,
    },
    RetargetEdge {
        id: EdgeId,
        new_target: RetargetEdge,
    },
    UpsertEdgeData {
        id: EdgeId,
        obj: Object,
    },
    ChangeEdgeData {
        id: EdgeId,
        delta: Vec<ObjectPatch>,
    },
    RemoveEdgeData {
        id: EdgeId,
    },

    // ------------- END EDGE DELTA --------------

    // ------------- START HYPER EDGE --------------
    
    CreateHyperedge {
        id: HyperedgeId,
        members: HashSet<Pointee>,
    },
    RemoveHyperedge {
        id: HyperedgeId,
    },
    AddHyperedgeMembers {
        id: HyperedgeId,
        members: HashSet<Pointee>,
    },
    RemoveHyperedgeMembers {
        id: HyperedgeId,
        members: HashSet<Pointee>,
    },
    UpsertHyperedgeData {
        id: HyperedgeId,
        obj: Object,
    },
    ChangeHyperedgeData {
        id: HyperedgeId,
        delta: Vec<ObjectPatch>,
    },
    RemoveHyperedgeData {
        id: HyperedgeId,
    },

    // ------------- END HYPER EDGE --------------
}

/// Patch that also carries the state the change overwrote/removed,
/// so it can be reverted. Kept in the db log; convert to
/// [`ThinPatch`] via `From` before sending over the network.
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum FullPatch {
    // ------------- START NODE DELTA --------------
    AddNode {
        id: NodeId,
        obj: Object,
    },
    RemoveNode {
        id: NodeId,
        /// Removed object.
        obj: Object,
    },
    ChangeNode {
        id: NodeId,
        delta: Vec<ObjectPatch>,
        /// Object before `delta` was applied.
        old: Object,
    },
    UpsertNode {
        id: NodeId,
        obj: Object,
        /// Object before the upsert.
        old: Object,
    },

    // ------------- END NODE DELTA --------------

    // --------------- START EDGE DELTA ----------
    AddEdge {
        id: EdgeId,
        source: Pointee,
        target: Pointee,
    },
    RemoveEdge {
        id: EdgeId,
        source: Pointee,
        target: Pointee,
        /// Attached object, if the edge had one.
        data: Option<Object>,
    },
    RetargetEdge {
        id: EdgeId,
        new_target: RetargetEdge,
        /// Same endpoint side as `new_target`, with the old pointee.
        old_target: RetargetEdge,
    },
    UpsertEdgeData {
        id: EdgeId,
        obj: Object,
        /// Attached object before the upsert; `None` on first attach.
        old: Option<Object>,
    },
    ChangeEdgeData {
        id: EdgeId,
        delta: Vec<ObjectPatch>,
        old: Object,
    },
    RemoveEdgeData {
        id: EdgeId,
        /// Removed attached object.
        obj: Object,
    },

    // ------------- END EDGE DELTA --------------

    // ------------- START HYPER EDGE --------------

    CreateHyperedge {
        id: HyperedgeId,
        members: HashSet<Pointee>,
    },
    RemoveHyperedge {
        id: HyperedgeId,
        /// Members at the moment of removal. Empty when the
        /// hyperedge is removed right after a `RemoveHyperedgeMembers`
        /// that emptied it.
        members: HashSet<Pointee>,
        /// Attached object, if the hyperedge had one.
        data: Option<Object>,
    },
    AddHyperedgeMembers {
        id: HyperedgeId,
        members: HashSet<Pointee>,
    },
    RemoveHyperedgeMembers {
        id: HyperedgeId,
        members: HashSet<Pointee>,
    },
    UpsertHyperedgeData {
        id: HyperedgeId,
        obj: Object,
        old: Option<Object>,
    },
    ChangeHyperedgeData {
        id: HyperedgeId,
        delta: Vec<ObjectPatch>,
        old: Object,
    },
    RemoveHyperedgeData {
        id: HyperedgeId,
        obj: Object,
    },

    // ------------- END HYPER EDGE --------------
}

impl From<FullPatch> for ThinPatch {
    fn from(patch: FullPatch) -> Self {
        match patch {
            FullPatch::AddNode { id, obj } => ThinPatch::AddNode { id, obj },
            FullPatch::RemoveNode { id, .. } => ThinPatch::RemoveNode { id },
            FullPatch::ChangeNode { id, delta, .. } => ThinPatch::ChangeNode { id, delta },
            FullPatch::UpsertNode { id, obj, .. } => ThinPatch::UpsertNode { id, obj },
            FullPatch::AddEdge { id, source, target } => {
                ThinPatch::AddEdge { id, source, target }
            }
            FullPatch::RemoveEdge { id, .. } => ThinPatch::RemoveEdge { id },
            FullPatch::RetargetEdge { id, new_target, .. } => {
                ThinPatch::RetargetEdge { id, new_target }
            }
            FullPatch::UpsertEdgeData { id, obj, .. } => ThinPatch::UpsertEdgeData { id, obj },
            FullPatch::ChangeEdgeData { id, delta, .. } => ThinPatch::ChangeEdgeData { id, delta },
            FullPatch::RemoveEdgeData { id, .. } => ThinPatch::RemoveEdgeData { id },
            FullPatch::CreateHyperedge { id, members } => {
                ThinPatch::CreateHyperedge { id, members }
            }
            FullPatch::RemoveHyperedge { id, .. } => ThinPatch::RemoveHyperedge { id },
            FullPatch::AddHyperedgeMembers { id, members } => {
                ThinPatch::AddHyperedgeMembers { id, members }
            }
            FullPatch::RemoveHyperedgeMembers { id, members } => {
                ThinPatch::RemoveHyperedgeMembers { id, members }
            }
            FullPatch::UpsertHyperedgeData { id, obj, .. } => {
                ThinPatch::UpsertHyperedgeData { id, obj }
            }
            FullPatch::ChangeHyperedgeData { id, delta, .. } => {
                ThinPatch::ChangeHyperedgeData { id, delta }
            }
            FullPatch::RemoveHyperedgeData { id, .. } => ThinPatch::RemoveHyperedgeData { id },
        }
    }
}

impl From<&FullPatch> for ThinPatch {
    fn from(patch: &FullPatch) -> Self {
        patch.clone().into()
    }
}

/// A complete description of an edge — its id and both endpoints.
/// Returned by `db::Graph::edge` and `db::Graph::remove_edge`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeView {
    pub id: EdgeId,
    pub source: Pointee,
    pub target: Pointee,
}

/// Classification of a [`Pointee`]. Mirrors `db::EntityType` for the
/// `Pointee::EntityId` case and adds [`PointeeKind::Subobject`] for
/// the `Pointee::Path` case.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum PointeeKind {
    Node,
    Edge,
    Hyperedge,
    MetaEdge,
    AttachedObject,
    Subobject,
}

/// One entry of the db event log: a mutation and everything its
/// cascade removed. Returned by `db::Graph::undo`.
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct Modification {
    /// Everything the cascade of this mutation removed — whole
    /// edges/hyperedges and members stripped from surviving
    /// hyperedges — in the order it happened (after `operation`).
    pub cascade: FullDelta,

    /// Main patch for this mutation.
    pub operation: FullPatch,
}
