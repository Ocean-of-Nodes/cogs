use std::collections::HashMap;
use std::path::PathBuf;

use uuid::Uuid;

/// Entity is a common type for nodes and edges and hyper edge
pub type EntityId = Uuid;
/// Node is an ends of edges that's not edge,
/// but contains data as object
pub type NodeId = Uuid;
/// Edge is an entity that contains data as
/// object and link between two entities
pub type EdgeID = Uuid;
/// Hyper edge (also space)
pub type HyperEdgeId = Uuid;

pub type Object = HashMap<String, Field>;

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Field {
    // ------- START COMPOSITE TYPES --------------
    Array(Vec<Field>),
    Object(Object),

    // ------- END COMPOSITE TYPES --------------

    // ------- START FUNDAMENTAL TYPES --------------
    String(String),
    Bool(bool),
    Number(i128),
    Link(EntityId),
    Null,
    // ------- END FUNDAMENTAL TYPES --------------
}

#[derive(Debug, Clone)]
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
        /// Path is a slash-separated string representing the path to the nested object
        path: PathBuf,
        delta: Vec<ObjectDelta>,
    },
}

#[derive(Debug, Clone)]
pub enum RetrargetEdge {
    Source(EntityId),
    Target(EntityId),
}

#[derive(Debug, Clone)]
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
