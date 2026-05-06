pub mod global_path;
pub mod local_path;

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use serde::{Deserialize, Serialize};

pub use global_path::GlobalObjPath;
pub use local_path::LocalObjPath;

/// Used by db for tracker's that's accumulate changes 
/// for caller that's want get it by next call
pub type TrackerId = Uuid;
/// Delta is array of `Patch`
pub type Delta = Vec<Patch>;

/// Entity is a common type for nodes/edges/metaedge/hyperedge
pub type EntityId = Uuid;

/// What an edge endpoint or a `Field::Link` can point at — either
/// a whole entity (by id) or a sub-object inside an entity (by path).
#[derive(PartialEq, Eq, Debug, Clone, Hash, Serialize, Deserialize)]
pub enum Pointee {
    EntityId(EntityId),
    Path(GlobalObjPath),
}

impl From<EntityId> for Pointee {
    fn from(id: EntityId) -> Self {
        Pointee::EntityId(id)
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

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum Patch {
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
