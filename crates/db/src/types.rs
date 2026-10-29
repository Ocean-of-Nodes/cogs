//! Domain taxonomy types: how the graph distinguishes nodes, edges,
//! hyperedges, and friends.

pub use common::{EdgeView, PointeeKind};

/// Internal classification of an [`EntityId`] — what kind of entity
/// it represents within the graph's storage model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityType {
    Node,
    Edge,
    Hyperedge,
    /// An edge whose at least one endpoint is itself an edge or
    /// hyperedge. Structurally still in `edges`.
    MetaEdge,
    /// An id that lives in `entities` AND in `edges`/`hyperedges` —
    /// i.e. an edge or hyperedge with `attach_obj` already called on
    /// it.
    AttachedObject,
}

impl EntityType {
    /// Whether this entity kind is a valid target for `attach_obj`
    /// (an edge or hyperedge without an attached object yet).
    pub(crate) fn is_attach_target(&self) -> bool {
        match self {
            EntityType::Node | EntityType::AttachedObject => false,
            EntityType::Edge | EntityType::Hyperedge | EntityType::MetaEdge => true,
        }
    }

    /// Whether this entity kind can carry an `Object` payload.
    pub(crate) fn can_contain_object(&self) -> bool {
        match self {
            EntityType::Node | EntityType::Edge | EntityType::Hyperedge | EntityType::MetaEdge => {
                true
            }
            EntityType::AttachedObject => false,
        }
    }
}

impl From<EntityType> for PointeeKind {
    fn from(ty: EntityType) -> Self {
        match ty {
            EntityType::Node => PointeeKind::Node,
            EntityType::Edge => PointeeKind::Edge,
            EntityType::Hyperedge => PointeeKind::Hyperedge,
            EntityType::MetaEdge => PointeeKind::MetaEdge,
            EntityType::AttachedObject => PointeeKind::AttachedObject,
        }
    }
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EntityType::Node => "Node",
            EntityType::Edge => "Edge",
            EntityType::Hyperedge => "HyperEdge",
            EntityType::MetaEdge => "MetaEdge",
            EntityType::AttachedObject => "AttachedObject",
        })
    }
}
