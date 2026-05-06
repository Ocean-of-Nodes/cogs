//! Error types returned by [`crate::Graph`] operations.
//!
//! Convention:
//! - All errors are named-field structs/enums.
//! - Each error carries the data needed to diagnose the failure
//!   (offending id, the conflicting set, etc.).
//! - Inner data structs end in `Error`; enum variants do not repeat
//!   the suffix.

use std::collections::HashSet;

use common::*;

// ----- Singleton "not found" / "already exists" errors --------------

/// The id is not currently a hyperedge in the graph.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct HyperEdgeNotFoundError {
    pub id: HyperEdgeId,
}

/// The id is not a known entity (node, edge, or hyperedge).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct EntityNotFoundError {
    pub id: EntityId,
}

/// The id is not a known node — it might be missing entirely, or
/// it might exist as an edge / hyperedge / attached-object id.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NodeNotFoundError {
    pub id: NodeId,
}

/// The id is not a known edge.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct EdgeNotFoundError {
    pub id: EdgeID,
}

/// `add_node` (or replay of the same) collided with an existing id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NodeAlreadyExistsError {
    pub id: NodeId,
}

/// `silent_create_hyperedge_with_id` collided with an existing id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HyperEdgeAlreadyExistsError {
    pub id: HyperEdgeId,
}

/// `silent_add_edge_with_id` collided with an existing id.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct EdgeAlreadyExistsError {
    pub id: EdgeID,
}

/// One or both endpoints of an edge being added do not currently
/// resolve to an existing pointee.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MissingEndpointsError {
    /// The endpoints that failed `is_pointee_exist`.
    pub missing_endpoints: Vec<Pointee>,
}

/// One or more pointees passed to a hyperedge op do not exist.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PointeesNotFoundError {
    pub pointees: HashSet<Pointee>,
}

/// `add_hyperedge_members` was asked to insert pointees that are
/// already members of the hyperedge.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MembersAlreadyExistError {
    pub members: Vec<Pointee>,
}

/// `remove_hyperedge_members` was asked to drop pointees that are
/// not currently members of the hyperedge.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MembersNotInHyperedgeError {
    pub members: Vec<Pointee>,
}

/// Type-check failure: the id resolved to an entity whose kind is
/// not in the operation's accepted set.
#[derive(Debug)]
pub(crate) struct IncorrectTypeError {
    /// The id whose type didn't match.
    pub entity_id: EntityId,
    /// Human-readable list of accepted entity types.
    pub expected_type: Vec<String>,
    /// Human-readable description of the actual type.
    pub actual_type: String,
}

/// `attach_obj` was called on an id that doesn't exist at all.
#[derive(Debug)]
pub(crate) struct AttachTargetNotFoundError {
    pub id: AttachTargetID,
}

/// `remove_attached` / `replace_attached_obj` was called on an id
/// that is not a current attach target — either the id doesn't
/// exist, or it exists as a bare structural element (no attached
/// object) or as a node.
#[derive(Debug)]
pub(crate) struct NoAttachedObjectError {
    pub id: AttachTargetID,
}

/// `retarget_edge` was given a new endpoint that doesn't resolve.
#[derive(Debug)]
pub(crate) struct InvalidRetargetError {
    pub edge_id: EdgeID,
    pub new_target: RetargetEdge,
}

// ----- Composite enum errors ----------------------------------------

/// Errors returned by [`crate::Graph::create_hyperedge`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CreateHyperEdgeError {
    /// At least one member pointee doesn't exist.
    PointeesNotFound(PointeesNotFoundError),
    /// The chosen id is already taken by another hyperedge.
    HyperEdgeAlreadyExists(HyperEdgeAlreadyExistsError),
    /// Empty membership is rejected — every hyperedge has ≥1 member.
    EmptyHyperEdge,
}

/// Errors returned by [`crate::Graph::add_edge`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AddEdgeError {
    MissingEndpoints(MissingEndpointsError),
    EdgeAlreadyExists(EdgeAlreadyExistsError),
}

/// Errors returned by [`crate::Graph::retarget_edge`].
#[derive(Debug)]
pub(crate) enum RetargetError {
    EdgeNotFound(EdgeNotFoundError),
    InvalidTarget(InvalidRetargetError),
}

/// Errors returned by [`crate::Graph::attach_obj`].
#[derive(Debug)]
pub(crate) enum AttachObjectError {
    /// Target id doesn't exist anywhere in the graph.
    AttachTargetNotFound(AttachTargetNotFoundError),
    /// Target exists but isn't a valid attach target (e.g. it's a
    /// node, or already has an attached object — see
    /// `replace_attached_obj` for the latter).
    IncorrectType(IncorrectTypeError),
}

/// Errors returned by [`crate::Graph::edge`].
#[derive(Debug)]
pub(crate) enum GetEdgeError {
    /// Id doesn't exist in the graph.
    NotFound(EntityNotFoundError),
    /// Id exists but isn't an edge.
    IncorrectType(IncorrectTypeError),
}

/// Errors returned by [`crate::Graph::add_hyperedge_members`].
#[derive(Debug)]
pub(crate) enum AddHyperedgeMembersError {
    /// The hyperedge id doesn't exist.
    HyperEdgeNotFound(HyperEdgeNotFoundError),
    /// Some passed pointees are already members.
    MembersAlreadyExist(MembersAlreadyExistError),
    /// Some passed pointees don't exist as graph entities.
    PointeesNotFound(PointeesNotFoundError),
}

/// Errors returned by [`crate::Graph::remove_hyperedge_members`].
#[derive(Debug)]
pub(crate) enum RemoveHyperedgeMembersError {
    /// The hyperedge id doesn't exist.
    HyperEdgeNotFound(HyperEdgeNotFoundError),
    /// Some passed pointees aren't current members.
    MembersNotInHyperedge(MembersNotInHyperedgeError),
}

// ----- Object-patch and high-level errors ---------------------------

/// Errors returned while applying a single [`ObjectPatch`] to an
/// `Object`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ObjectPatchError {
    /// `AddField` on a name that already exists.
    FieldAlreadyExists { field_name: String },
    /// `RemoveField` / `ArrayPatch` / `SubObjectPatch` referenced a
    /// name that's not in the object.
    FieldNotFound { field_name: String },
    /// Path navigation landed on a non-`Object` field.
    NotAnObject { field_name: String },
    /// `ArrayPatch` named a field that is not a `Field::Array`.
    NotAnArray { field_name: String },
    /// An array index is past the array's bounds at apply time.
    IndexOutOfBounds { index: usize },
}

/// Errors returned by [`crate::Graph::obj_apply_patch`].
#[derive(Debug)]
pub(crate) enum DeltaError {
    /// The id has no `Object` to patch (could be unknown, or an
    /// edge/hyperedge with no attached object).
    NotFound(EntityNotFoundError),
    /// Failure inside the inner patch application.
    Delta(ObjectPatchError),
}

/// Errors returned by [`crate::Graph::apply_patch`] — wraps the
/// failure of whichever silent op the offending [`Patch`] mapped to.
#[derive(Debug)]
pub(crate) enum ApplyPatchError {
    NodeAlreadyExists(NodeAlreadyExistsError),
    AddEdge(AddEdgeError),
    CreateHyperEdge(CreateHyperEdgeError),
    NodeNotFound(NodeNotFoundError),
    EdgeNotFound(EdgeNotFoundError),
    HyperEdgeNotFound(HyperEdgeNotFoundError),
    Retarget(RetargetError),
    Change(DeltaError),
    AddHyperedgeMembers(AddHyperedgeMembersError),
    RemoveHyperedgeMembers(RemoveHyperedgeMembersError),
    NoAttachedObject(NoAttachedObjectError),
    Attach(AttachObjectError),
}

// ----- From impls: enable `?`-conversion in `apply_patch` -----------
//
// One impl per silent-op error → ApplyPatchError variant. Lets us
// write `self.silent_*(...)?` instead of `.map_err(ApplyPatchError::*)?`.

impl From<NodeAlreadyExistsError> for ApplyPatchError {
    fn from(e: NodeAlreadyExistsError) -> Self {
        Self::NodeAlreadyExists(e)
    }
}

impl From<AddEdgeError> for ApplyPatchError {
    fn from(e: AddEdgeError) -> Self {
        Self::AddEdge(e)
    }
}

impl From<CreateHyperEdgeError> for ApplyPatchError {
    fn from(e: CreateHyperEdgeError) -> Self {
        Self::CreateHyperEdge(e)
    }
}

impl From<NodeNotFoundError> for ApplyPatchError {
    fn from(e: NodeNotFoundError) -> Self {
        Self::NodeNotFound(e)
    }
}

impl From<EdgeNotFoundError> for ApplyPatchError {
    fn from(e: EdgeNotFoundError) -> Self {
        Self::EdgeNotFound(e)
    }
}

impl From<HyperEdgeNotFoundError> for ApplyPatchError {
    fn from(e: HyperEdgeNotFoundError) -> Self {
        Self::HyperEdgeNotFound(e)
    }
}

impl From<RetargetError> for ApplyPatchError {
    fn from(e: RetargetError) -> Self {
        Self::Retarget(e)
    }
}

impl From<DeltaError> for ApplyPatchError {
    fn from(e: DeltaError) -> Self {
        Self::Change(e)
    }
}

impl From<AddHyperedgeMembersError> for ApplyPatchError {
    fn from(e: AddHyperedgeMembersError) -> Self {
        Self::AddHyperedgeMembers(e)
    }
}

impl From<RemoveHyperedgeMembersError> for ApplyPatchError {
    fn from(e: RemoveHyperedgeMembersError) -> Self {
        Self::RemoveHyperedgeMembers(e)
    }
}

impl From<NoAttachedObjectError> for ApplyPatchError {
    fn from(e: NoAttachedObjectError) -> Self {
        Self::NoAttachedObject(e)
    }
}

impl From<AttachObjectError> for ApplyPatchError {
    fn from(e: AttachObjectError) -> Self {
        Self::Attach(e)
    }
}
