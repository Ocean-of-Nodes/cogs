pub mod path;
pub mod local_path;

use std::collections::HashMap;
use uuid::Uuid;

use serde::{Deserialize, Serialize};

pub use path::Path;
pub use local_path::LocalPath;

/// Used by db for tracker's that's accumulate changes 
/// for caller that's want get it by next call
pub type TrackerId = Uuid;

/// Entity is a common type for nodes/edges/metaedge/hyperedge
pub type EntityId = Uuid;

/// What an edge endpoint or a `Field::Link` can point at — either
/// a whole entity (by id) or a sub-object inside an entity (by path).
#[derive(PartialEq, Eq, Debug, Clone, Hash, Serialize, Deserialize)]
pub enum Pointee {
    EntityId(EntityId),
    Path(Path),
}

impl From<EntityId> for Pointee {
    fn from(id: EntityId) -> Self {
        Pointee::EntityId(id)
    }
}

/// Target edges/metaedge/hyperedge for thats object 
/// can be attached 
pub type AttachTargetID = Uuid;
/// Node is an ends of edges that's not edge,
/// but contains data as object
pub type NodeId = Uuid;
/// Edge is an entity that contains data as
/// object and link between two entities
pub type EdgeID = Uuid;
/// Hyper edge thats hold some entities
pub type HyperEdgeId = Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectDelta {
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
        /// Path is a slash-separated string representing the path
        /// to the nested object
        path: LocalPath,
        delta: Vec<ObjectDelta>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetrargetEdge {
    Source(EntityId),
    Target(EntityId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Patch {
    // ------------- START NODE DELTA --------------
    AddNode {
        id: EntityId,
        obj: Object,
    },
    RemoveNode {
        id: NodeId,
    },
    ChangeNode {
        id: NodeId,
        delta: Vec<ObjectDelta>,
    },

    // ------------- END NODE DELTA --------------

    // --------------- START EDGE DELTA ----------
    AddEdge {
        id: EdgeID,
        source: EntityId,
        target: EntityId,
    },
    RemoveEdge {
        id: EdgeID,
    },
    RetrargetEdge {
        id: EdgeID,
        new_target: RetrargetEdge,
    },

    // ------------- END EDGE DELTA --------------

    // ------------- START HYPER EDGE --------------
    CreateHyperEdge {
        id: HyperEdgeId,
        entities: Vec<EntityId>,
    },
    RemoveHyperEdge {
        id: HyperEdgeId,
    },
    AddElementsToHyperEdge {
        id: HyperEdgeId,
        entities: Vec<EntityId>,
    },
    RemoveElementsFromHyperEdge {
        id: HyperEdgeId,
        entities: Vec<EntityId>,
    },
    MergeHyperEdge {
        lhs: HyperEdgeId,
        rhs: HyperEdgeId,
    },
    // ------------- END HYPER EDGE --------------
}

pub fn is_delta_order_valid(delta: &Vec<Patch>) -> bool {
    todo!()
}
