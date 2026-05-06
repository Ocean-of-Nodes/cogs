use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use common::*;

/// Identifier returned by [`Graph::subscribe_on_change`] and used to
/// remove the listener via [`Graph::unsubscribe_on_change`].
type ListenerId = Uuid;

// =====================================================================
//                            ERROR TYPES
// =====================================================================
//
// Convention:
//   - All errors are named-field structs/enums.
//   - Each error carries the data needed to diagnose the failure
//     (offending id, the conflicting set, etc.).
//   - Inner data structs end in `Error`; enum variants do not repeat
//     the suffix.

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

/// Errors returned by [`Graph::create_hyperedge`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CreateHyperEdgeError {
    /// At least one member pointee doesn't exist.
    PointeesNotFound(PointeesNotFoundError),
    /// The chosen id is already taken by another hyperedge.
    HyperEdgeAlreadyExists(HyperEdgeAlreadyExistsError),
    /// Empty membership is rejected — every hyperedge has ≥1 member.
    EmptyHyperEdge,
}

/// Errors returned by [`Graph::add_edge`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AddEdgeError {
    MissingEndpoints(MissingEndpointsError),
    EdgeAlreadyExists(EdgeAlreadyExistsError),
}

/// Errors returned by [`Graph::retarget_edge`].
#[derive(Debug)]
pub(crate) enum RetargetError {
    EdgeNotFound(EdgeNotFoundError),
    InvalidTarget(InvalidRetargetError),
}

/// Errors returned by [`Graph::attach_obj`].
#[derive(Debug)]
pub(crate) enum AttachObjectError {
    /// Target id doesn't exist anywhere in the graph.
    AttachTargetNotFound(AttachTargetNotFoundError),
    /// Target exists but isn't a valid attach target (e.g. it's a
    /// node, or already has an attached object — see
    /// `replace_attached_obj` for the latter).
    IncorrectType(IncorrectTypeError),
}

/// Errors returned by [`Graph::edge`].
#[derive(Debug)]
pub(crate) enum GetEdgeError {
    /// Id doesn't exist in the graph.
    NotFound(EntityNotFoundError),
    /// Id exists but isn't an edge.
    IncorrectType(IncorrectTypeError),
}

/// Errors returned by [`Graph::add_hyperedge_members`].
#[derive(Debug)]
pub(crate) enum AddHyperedgeMembersError {
    /// The hyperedge id doesn't exist.
    HyperEdgeNotFound(HyperEdgeNotFoundError),
    /// Some passed pointees are already members.
    MembersAlreadyExist(MembersAlreadyExistError),
    /// Some passed pointees don't exist as graph entities.
    PointeesNotFound(PointeesNotFoundError),
}

/// Errors returned by [`Graph::remove_hyperedge_members`].
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

/// Errors returned by [`Graph::obj_apply_patch`].
#[derive(Debug)]
pub(crate) enum DeltaError {
    /// The id has no `Object` to patch (could be unknown, or an
    /// edge/hyperedge with no attached object).
    NotFound(EntityNotFoundError),
    /// Failure inside the inner patch application.
    Delta(ObjectPatchError),
}

/// Errors returned by [`Graph::apply_patch`] — wraps the failure of
/// whichever silent op the offending [`Patch`] mapped to.
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

/// Apply a sequence of `ObjectPatch` to an `Object` in place. Used by
/// `obj_apply_patch` to replay `Change*` patches.
fn apply_object_patches(obj: &mut Object, patches: Vec<ObjectPatch>) -> Result<(), ObjectPatchError> {
    for p in patches {
        match p {
            ObjectPatch::AddField { name, field } => {
                if obj.contains_key(&name) {
                    return Err(ObjectPatchError::FieldAlreadyExists { field_name: name });
                }
                obj.insert(name, field);
            }
            ObjectPatch::RemoveField { name } => {
                if obj.remove(&name).is_none() {
                    return Err(ObjectPatchError::FieldNotFound { field_name: name });
                }
            }
            ObjectPatch::UpsertField { name, field } => {
                obj.insert(name, field);
            }
            ObjectPatch::ArrayPatch {
                name,
                removed_indices,
                added_fields,
            } => {
                let arr = match obj.get_mut(&name) {
                    Some(Field::Array(a)) => a,
                    Some(_) => return Err(ObjectPatchError::NotAnArray { field_name: name }),
                    None => return Err(ObjectPatchError::FieldNotFound { field_name: name }),
                };
                for &idx in &removed_indices {
                    if idx >= arr.len() {
                        return Err(ObjectPatchError::IndexOutOfBounds { index: idx });
                    }
                }
                let mut to_remove = removed_indices;
                to_remove.sort_unstable_by(|a, b| b.cmp(a));
                for idx in to_remove {
                    arr.remove(idx);
                }
                let mut to_add = added_fields;
                to_add.sort_by_key(|(idx, _)| *idx);
                for (idx, field) in to_add {
                    if idx > arr.len() {
                        return Err(ObjectPatchError::IndexOutOfBounds { index: idx });
                    }
                    arr.insert(idx, field);
                }
            }
            ObjectPatch::SubObjectPatch { path, delta } => {
                let mut cursor: &mut Object = obj;
                for seg in &path {
                    match cursor.get_mut(seg) {
                        Some(Field::Object(inner)) => cursor = inner,
                        Some(_) => {
                            return Err(ObjectPatchError::NotAnObject {
                                field_name: seg.to_string(),
                            })
                        }
                        None => {
                            return Err(ObjectPatchError::FieldNotFound {
                                field_name: seg.to_string(),
                            })
                        }
                    }
                }
                apply_object_patches(cursor, delta)?;
            }
        }
    }
    Ok(())
}

/// Shallow diff: produce a patch sequence that turns `old` into `new`.
/// Used by `replace_node` to decide whether emitting a delta is more
/// compact than emitting the full object via Upsert.
fn diff_object(old: &Object, new: &Object) -> Vec<ObjectPatch> {
    let mut patches = Vec::new();
    for k in old.keys() {
        if !new.contains_key(k) {
            patches.push(ObjectPatch::RemoveField { name: k.clone() });
        }
    }
    for (k, v) in new {
        match old.get(k) {
            Some(old_v) if old_v == v => {}
            Some(_) => patches.push(ObjectPatch::UpsertField {
                name: k.clone(),
                field: v.clone(),
            }),
            None => patches.push(ObjectPatch::AddField {
                name: k.clone(),
                field: v.clone(),
            }),
        }
    }
    patches
}

/// Reverse-index bucket: which edges/hyperedges currently point at
/// a given [`Pointee`]. Maintained by every mutating op so cascade
/// removal can find dangling references in O(in-degree).
#[derive(Default)]
struct PointeeUses {
    /// Edges whose `source` is this pointee.
    edges_as_source: HashSet<EdgeID>,
    /// Edges whose `target` is this pointee.
    edges_as_target: HashSet<EdgeID>,
    /// Hyperedges that include this pointee as a member.
    hyperedges: HashSet<HyperEdgeId>,
}

impl PointeeUses {
    /// True when no structural element references this pointee. The
    /// bucket is dropped from the index whenever this is true.
    fn is_empty(&self) -> bool {
        self.edges_as_source.is_empty()
            && self.edges_as_target.is_empty()
            && self.hyperedges.is_empty()
    }
}

/// In-memory graph storing nodes, edges, hyperedges, optional
/// attached `Object`s on edges/hyperedges, and the patch log of
/// every mutation.
///
/// ## Invariants
///
/// - Every edge endpoint resolves at insertion time
///   ([`Graph::is_pointee_exist`]).
/// - Every hyperedge has at least one member; cascade-removing the
///   last member deletes the hyperedge.
/// - The reverse indexes (`pointee_uses`, `entity_to_path_pointees`)
///   are derivable from the structural maps and are kept consistent
///   on every mutation; see `tests::test_utils::check_index_invariant`.
/// - Removing an entity transitively removes everything that
///   referenced it (directly via `Pointee::EntityId` or through a
///   `Pointee::Path`); see [`Graph::cascade_remove_id`].
///
/// ## Replay
///
/// Every public mutation appends a [`Patch`] to `events`. Replaying
/// the recorded log via [`Graph::apply_patch`] on a fresh graph
/// reconstructs an identical state (verified by round-trip tests in
/// `mod test_apply_patch`).
#[derive(Default)]
struct Graph {
    /// Object attached to each id — nodes always, edges/hyperedges
    /// only when [`Graph::attach_obj`] has been called on them.
    entities: HashMap<EntityId, Object>,

    /// `EdgeID → (source, target)` pair.
    edges: HashMap<EdgeID, (Pointee, Pointee)>,

    /// `HyperEdgeId → set of members`.
    hyper_edge: HashMap<HyperEdgeId, HashSet<Pointee>>,

    /// Reverse index: for each [`Pointee`] referenced as an
    /// edge-endpoint or hyperedge-member, who references it.
    pointee_uses: HashMap<Pointee, PointeeUses>,

    /// Secondary index: for each entity, every `Pointee::Path`
    /// rooted at it that's currently live in `pointee_uses`. Lets
    /// cascade removal find paths through an entity in
    /// O(in-degree-by-paths) instead of scanning the whole index.
    entity_to_path_pointees: HashMap<EntityId, HashSet<Pointee>>,

    /// Patch log
    events: Delta,
}

impl Graph {
    // -------------- START INDEXES ----------------------- //
    fn track_pointee_entity(&mut self, p: &Pointee) {
        if let Pointee::Path(gp) = p {
            self.entity_to_path_pointees
                .entry(gp.entity())
                .or_default()
                .insert(p.clone());
        }
    }

    fn untrack_pointee_entity(&mut self, p: &Pointee) {
        if let Pointee::Path(gp) = p {
            if let Some(set) = self.entity_to_path_pointees.get_mut(&gp.entity()) {
                set.remove(p);
                if set.is_empty() {
                    self.entity_to_path_pointees.remove(&gp.entity());
                }
            }
        }
    }

    // -------------- END INDEXES ------------------------- //

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
            None => Err(GetEdgeError::NotFound(EntityNotFoundError { id: *id })),
            Some(ty) => Err(GetEdgeError::IncorrectType(IncorrectTypeError {
                entity_id: *id,
                expected_type: vec!["Edge".to_string()],
                actual_type: ty.to_string(),
            })),
        }
    }

    /// Direct lookup of a hyperedge's members. `None` if `id` is
    /// not a hyperedge. Companion to [`Graph::edge`].
    pub fn hyperedge_members(&self, id: &HyperEdgeId) -> Option<&HashSet<Pointee>> {
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

    /// Walk `local` through `obj`, descending into nested objects
    /// segment by segment. Returns the field reached by the last
    /// segment, or `None` if any segment is missing or attempts to
    /// traverse a non-object field.
    fn navigate_object<'a>(obj: &'a Object, local: &LocalObjPath) -> Option<&'a Field> {
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

    fn is_node(&self, id: &NodeId) -> bool {
        self.entities.contains_key(id)
            && !self.edges.contains_key(id)
            && !self.hyper_edge.contains_key(id)
    }

    /* ------------ END PROBS -------------------- */

    /* ------------ START CONSTRUCTORS ----------- */

    fn silent_create_hyperedge_with_id(
        &mut self,
        id: &HyperEdgeId,
        members: HashSet<Pointee>,
    ) -> Result<(), CreateHyperEdgeError> {
        if members.is_empty() {
            return Err(CreateHyperEdgeError::EmptyHyperEdge);
        }

        if self.hyper_edge.contains_key(id) {
            return Err(CreateHyperEdgeError::HyperEdgeAlreadyExists(
                HyperEdgeAlreadyExistsError { id: id.clone() },
            ));
        }

        let mut unexist = HashSet::new();
        for member in members.iter() {
            if !self.is_pointee_exist(member) {
                unexist.insert(member.clone());
            }
        }

        if !unexist.is_empty() {
            return Err(CreateHyperEdgeError::PointeesNotFound(
                PointeesNotFoundError { pointees: unexist },
            ));
        }

        // Update reverse index BEFORE storing — `track_pointee_entity`
        // needs the original `members` set; we register each member's
        // bucket and entity-to-paths secondary.
        for member in &members {
            self.track_pointee_entity(member);
            self.pointee_uses
                .entry(member.clone())
                .or_default()
                .hyperedges
                .insert(*id);
        }

        self.hyper_edge.insert(id.clone(), members);
        Ok(())
    }

    /// Create a hyperedge containing the given non-empty set of
    /// existing pointees. Generates a new id and records
    /// [`Patch::CreateHyperEdge`].
    pub fn create_hyperedge(
        &mut self,
        members: HashSet<Pointee>,
    ) -> Result<HyperEdgeId, CreateHyperEdgeError> {
        let id = Uuid::new_v4();
        self.silent_create_hyperedge_with_id(&id, members.clone())?;
        self.emit_patch(Patch::CreateHyperEdge { id, members });
        Ok(id)
    }

    fn silent_add_node_with_id(
        &mut self,
        id: NodeId,
        obj: Object,
    ) -> Result<(), NodeAlreadyExistsError> {
        if self.entities.contains_key(&id) {
            return Err(NodeAlreadyExistsError { id });
        }

        self.entities.insert(id, obj);
        Ok(())
    }

    /// Add a fresh node carrying `obj`. Generates a new id and
    /// records [`Patch::AddNode`] in the patch log.
    pub fn add_node(&mut self, obj: Object) -> NodeId {
        let id = Uuid::new_v4();
        self.silent_add_node_with_id(id, obj.clone());
        self.emit_patch(Patch::AddNode { id, obj: obj });
        id
    }

    fn silent_add_edge_with_id(
        &mut self,
        id: Uuid,
        source: Pointee,
        target: Pointee,
    ) -> Result<(), AddEdgeError> {
        let source_exists = self.is_pointee_exist(&source);
        let target_exists = self.is_pointee_exist(&target);
        if !source_exists || !target_exists {
            let mut missing_endpoints = Vec::new();
            if !source_exists {
                missing_endpoints.push(source);
            }
            if !target_exists {
                missing_endpoints.push(target);
            }
            return Err(AddEdgeError::MissingEndpoints(MissingEndpointsError {
                missing_endpoints,
            }));
        }

        if self.edges.contains_key(&id) {
            return Err(AddEdgeError::EdgeAlreadyExists(EdgeAlreadyExistsError {
                id,
            }));
        }

        // Update reverse index alongside the structural insert.
        self.track_pointee_entity(&source);
        self.pointee_uses
            .entry(source.clone())
            .or_default()
            .edges_as_source
            .insert(id);

        self.track_pointee_entity(&target);
        self.pointee_uses
            .entry(target.clone())
            .or_default()
            .edges_as_target
            .insert(id);

        self.edges.insert(id, (source, target));
        Ok(())
    }

    /// Add an edge from `source` to `target`. Both endpoints must
    /// resolve via [`Graph::is_pointee_exist`] at insertion time.
    /// Generates a new id and records [`Patch::AddEdge`].
    pub fn add_edge(
        &mut self,
        source: impl Into<Pointee>,
        target: impl Into<Pointee>,
    ) -> Result<EdgeID, AddEdgeError> {
        let source_pointee = source.into();
        let target_pointee = target.into();
        let edge_id = Uuid::new_v4();
        self.silent_add_edge_with_id(edge_id, source_pointee.clone(), target_pointee.clone())?;
        self.emit_patch(Patch::AddEdge {
            id: edge_id,
            source: source_pointee,
            target: target_pointee,
        });
        Ok(edge_id)
    }

    /* ------------ END CONSTRUCTORS ------------- */

    /* ------------ START DESTRUCTORS ----------- */

    /// Drains a single pointee bucket: removes referencing edges,
    /// strips the pointee from hyperedge memberships (killing
    /// hyperedges that empty out). New dangling structural ids are
    /// pushed onto `worklist` for the caller to process.
    fn drain_pointee_bucket(&mut self, pointee: &Pointee, worklist: &mut Vec<EntityId>) {
        let Some(uses) = self.pointee_uses.remove(pointee) else {
            return;
        };

        let dead_edges: HashSet<EdgeID> = uses
            .edges_as_source
            .iter()
            .chain(uses.edges_as_target.iter())
            .copied()
            .collect();
        for eid in dead_edges {
            if let Some((src, tgt)) = self.edges.remove(&eid) {
                for endpoint in [&src, &tgt] {
                    if endpoint == pointee {
                        continue;
                    }
                    if let Some(bucket) = self.pointee_uses.get_mut(endpoint) {
                        bucket.edges_as_source.remove(&eid);
                        bucket.edges_as_target.remove(&eid);
                        if bucket.is_empty() {
                            self.pointee_uses.remove(endpoint);
                            self.untrack_pointee_entity(endpoint);
                        }
                    }
                }
                self.entities.remove(&eid);
                worklist.push(eid);
            }
        }

        for hid in uses.hyperedges {
            if let Some(members) = self.hyper_edge.get_mut(&hid) {
                members.remove(pointee);
                if members.is_empty() {
                    self.hyper_edge.remove(&hid);
                    self.entities.remove(&hid);
                    worklist.push(hid);
                }
            }
        }
    }

    /// For one fully-dead entity, drain every pointee that becomes
    /// invalid: `Pointee::EntityId(dead_id)` and every `Pointee::Path`
    /// through it (looked up via `entity_to_path_pointees`).
    fn cascade_drain_id(&mut self, dead_id: EntityId, worklist: &mut Vec<EntityId>) {
        let mut affected: Vec<Pointee> = vec![Pointee::EntityId(dead_id)];
        if let Some(paths) = self.entity_to_path_pointees.remove(&dead_id) {
            affected.extend(paths);
        }
        for pointee in affected {
            self.drain_pointee_bucket(&pointee, worklist);
        }
    }

    fn cascade_remove_id(&mut self, removed: EntityId) {
        let mut worklist: Vec<EntityId> = vec![removed];
        while let Some(dead_id) = worklist.pop() {
            self.cascade_drain_id(dead_id, &mut worklist);
        }
    }

    /// Cascade-clean every reference that depended on `entity` having
    /// an attached object — i.e., every `Pointee::Path` rooted at this
    /// entity. The entity itself (edge or hyperedge) stays alive, so
    /// `Pointee::EntityId(entity)` references are preserved. Used by
    /// `remove_attached`.
    fn cascade_path_references_through(&mut self, entity: EntityId) {
        let Some(paths) = self.entity_to_path_pointees.remove(&entity) else {
            return;
        };
        let mut worklist: Vec<EntityId> = Vec::new();
        for pointee in paths {
            self.drain_pointee_bucket(&pointee, &mut worklist);
        }
        while let Some(dead_id) = worklist.pop() {
            self.cascade_drain_id(dead_id, &mut worklist);
        }
    }

    /// After mutating `entity`'s object, drop every `Pointee::Path`
    /// through it that no longer resolves under the new shape. The
    /// entity itself stays alive, so `Pointee::EntityId(entity)`
    /// references are preserved. Used by `replace_node` (and any
    /// future field-mutating op).
    fn cascade_invalid_paths_through(&mut self, entity: EntityId) {
        let candidates: Vec<Pointee> = match self.entity_to_path_pointees.get(&entity) {
            Some(set) => set.iter().cloned().collect(),
            None => return,
        };
        let dead: Vec<Pointee> = candidates
            .into_iter()
            .filter(|p| !self.is_pointee_exist(p))
            .collect();
        if dead.is_empty() {
            return;
        }
        let mut worklist: Vec<EntityId> = Vec::new();
        for p in &dead {
            self.untrack_pointee_entity(p);
            self.drain_pointee_bucket(p, &mut worklist);
        }
        while let Some(dead_id) = worklist.pop() {
            self.cascade_drain_id(dead_id, &mut worklist);
        }
    }

    fn silent_remove_node(&mut self, id: &NodeId) -> Result<Field, NodeNotFoundError> {
        if !self.is_node(id) {
            return Err(NodeNotFoundError { id: *id });
        }
        let obj = self
            .entities
            .remove(id)
            .ok_or(NodeNotFoundError { id: *id })?;
        self.cascade_remove_id(*id);
        Ok(Field::Object(obj))
    }

    /// Remove a node and cascade-delete every edge / hyperedge that
    /// referenced it (directly or via a `Pointee::Path`). Records
    /// one [`Patch::RemoveNode`] regardless of cascade depth.
    /// Returns the node's previous object as `Field::Object`.
    pub fn remove_node(&mut self, id: &NodeId) -> Result<Field, NodeNotFoundError> {
        let field = self.silent_remove_node(id)?;
        self.emit_patch(Patch::RemoveNode { id: *id });
        Ok(field)
    }

    fn silent_remove_edge(&mut self, id: &EdgeID) -> Result<Triplet, EdgeNotFoundError> {
        let (source, target) = self
            .edges
            .remove(id)
            .ok_or(EdgeNotFoundError { id: *id })?;

        // Strip eid from both endpoints' buckets; clean up empty buckets.
        for (endpoint, is_source) in [(&source, true), (&target, false)] {
            if let Some(bucket) = self.pointee_uses.get_mut(endpoint) {
                if is_source {
                    bucket.edges_as_source.remove(id);
                } else {
                    bucket.edges_as_target.remove(id);
                }
                if bucket.is_empty() {
                    self.pointee_uses.remove(endpoint);
                    self.untrack_pointee_entity(endpoint);
                }
            }
        }

        // Drop attached object on this edge, if any.
        self.entities.remove(id);

        Ok(Triplet {
            id: *id,
            source,
            target,
        })
    }

    /// Remove an edge by id. The edge's reverse-index entries are
    /// cleaned up. Records [`Patch::RemoveEdge`].
    /// Returns the [`Triplet`] of the removed edge.
    pub fn remove_edge(&mut self, id: &EdgeID) -> Result<Triplet, EdgeNotFoundError> {
        let res = self.silent_remove_edge(id)?;
        self.emit_patch(Patch::RemoveEdge { id: *id });
        Ok(res)
    }

    fn silent_remove_hyperedge(
        &mut self,
        hid: &HyperEdgeId,
    ) -> Result<HashSet<Pointee>, HyperEdgeNotFoundError> {
        let members = self
            .hyper_edge
            .remove(hid)
            .ok_or(HyperEdgeNotFoundError { id: *hid })?;

        // Strip `hid` from each member's reverse-index bucket.
        for member in &members {
            if let Some(bucket) = self.pointee_uses.get_mut(member) {
                bucket.hyperedges.remove(hid);
                if bucket.is_empty() {
                    self.pointee_uses.remove(member);
                    self.untrack_pointee_entity(member);
                }
            }
        }

        // Drop attached object on this hyperedge, if any.
        self.entities.remove(hid);

        // Anything that pointed at `hid` (edges with EntityId/Path
        // endpoints, other hyperedges that had `hid` as member) is
        // now dangling — let the cascade clean it up.
        self.cascade_remove_id(*hid);

        Ok(members)
    }

    /// Remove a hyperedge by id. Cascades to anything that
    /// referenced it (edges, parent hyperedges that lose this
    /// member and become empty). Records one
    /// [`Patch::RemoveHyperEdge`]. Returns the previous member set.
    pub fn remove_hyperedge(
        &mut self,
        hid: &HyperEdgeId,
    ) -> Result<HashSet<Pointee>, HyperEdgeNotFoundError> {
        let members = self.silent_remove_hyperedge(hid)?;
        self.emit_patch(Patch::RemoveHyperEdge { id: *hid });
        Ok(members)
    }

    /// Remove the attached object on an edge or hyperedge. The
    /// structural element itself stays alive; only the `Object`
    /// stored on top of it is dropped, along with any `Pointee::Path`
    /// references that depended on the attached object's fields.
    pub fn silent_remove_attached(
        &mut self,
        target: AttachTargetID,
    ) -> Result<(), NoAttachedObjectError> {
        let is_attach_target = self.entities.contains_key(&target)
            && (self.edges.contains_key(&target) || self.hyper_edge.contains_key(&target));
        if !is_attach_target {
            return Err(NoAttachedObjectError { id: target });
        }

        self.entities.remove(&target);
        self.cascade_path_references_through(target);
        Ok(())
    }

    pub fn remove_attached(
        &mut self,
        target: AttachTargetID,
    ) -> Result<(), NoAttachedObjectError> {
        // Determine the patch variant *before* the silent op — both
        // structures stay alive after the call, but resolving the
        // type is conceptually a pre-condition.
        let is_edge = self.edges.contains_key(&target);
        let is_hyper = self.hyper_edge.contains_key(&target);

        self.silent_remove_attached(target)?;

        if is_edge {
            self.emit_patch(Patch::RemoveEdgeData { id: target });
        } else if is_hyper {
            self.emit_patch(Patch::RemoveHyperEdgeData { id: target });
        }
        Ok(())
    }

    /* ------------ END DESTRUCTORS ------------- */

    /* ------------ START MODIFIERS ----------- */

    fn silent_attach_obj(
        &mut self,
        target: AttachTargetID,
        obj: Object,
    ) -> Result<(), AttachObjectError> {
        let ty = match self.get_type(target) {
            Some(t) => t,
            None => {
                return Err(AttachObjectError::AttachTargetNotFound(
                    AttachTargetNotFoundError { id: target },
                ))
            }
        };
        if !ty.is_attach_target() {
            return Err(AttachObjectError::IncorrectType(IncorrectTypeError {
                entity_id: target,
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

    pub fn attach_obj(
        &mut self,
        target: AttachTargetID,
        obj: Object,
    ) -> Result<(), AttachObjectError> {
        // Resolve target type *before* mutating so we can pick the
        // right patch variant. After silent_attach_obj succeeds, the
        // type is guaranteed to be one of the attach-target kinds.
        let is_hyper = self.hyper_edge.contains_key(&target);

        self.silent_attach_obj(target, obj.clone())?;

        if is_hyper {
            self.emit_patch(Patch::UpsertHyperEdgeData { id: target, obj });
        } else {
            // MetaEdge is structurally an edge, so it lives in `self.edges`.
            self.emit_patch(Patch::UpsertEdgeData { id: target, obj });
        }
        Ok(())
    }

    fn silent_add_hyperedge_members(
        &mut self,
        id: HyperEdgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), AddHyperedgeMembersError> {
        if !self.hyper_edge.contains_key(&id) {
            return Err(AddHyperedgeMembersError::HyperEdgeNotFound(
                HyperEdgeNotFoundError { id },
            ));
        }

        let mut missing: HashSet<Pointee> = HashSet::new();
        for p in &m {
            if !self.is_pointee_exist(p) {
                missing.insert(p.clone());
            }
        }
        if !missing.is_empty() {
            return Err(AddHyperedgeMembersError::PointeesNotFound(
                PointeesNotFoundError { pointees: missing },
            ));
        }

        let existing = self.hyper_edge.get(&id).expect("checked above");
        let duplicates: Vec<Pointee> =
            m.iter().filter(|p| existing.contains(*p)).cloned().collect();
        if !duplicates.is_empty() {
            return Err(AddHyperedgeMembersError::MembersAlreadyExist(
                MembersAlreadyExistError { members: duplicates },
            ));
        }

        let members_set = self.hyper_edge.get_mut(&id).expect("checked above");
        for p in &m {
            members_set.insert(p.clone());
        }

        for p in &m {
            self.track_pointee_entity(p);
            self.pointee_uses
                .entry(p.clone())
                .or_default()
                .hyperedges
                .insert(id);
        }

        Ok(())
    }

    pub fn add_hyperedge_members(
        &mut self,
        id: HyperEdgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), AddHyperedgeMembersError> {
        self.silent_add_hyperedge_members(id, m.clone())?;
        self.emit_patch(Patch::AddElementsToHyperEdge { id, members: m });
        Ok(())
    }

    fn silent_remove_hyperedge_members(
        &mut self,
        id: HyperEdgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), RemoveHyperedgeMembersError> {
        let Some(current) = self.hyper_edge.get(&id) else {
            return Err(RemoveHyperedgeMembersError::HyperEdgeNotFound(
                HyperEdgeNotFoundError { id },
            ));
        };

        let not_present: Vec<Pointee> = m
            .iter()
            .filter(|p| !current.contains(*p))
            .cloned()
            .collect();
        if !not_present.is_empty() {
            return Err(RemoveHyperedgeMembersError::MembersNotInHyperedge(
                MembersNotInHyperedgeError { members: not_present },
            ));
        }

        let members_set = self.hyper_edge.get_mut(&id).expect("checked above");
        for p in &m {
            members_set.remove(p);
        }
        let now_empty = members_set.is_empty();

        // Strip `id` from each removed member's reverse-index bucket.
        for p in &m {
            if let Some(bucket) = self.pointee_uses.get_mut(p) {
                bucket.hyperedges.remove(&id);
                if bucket.is_empty() {
                    self.pointee_uses.remove(p);
                    self.untrack_pointee_entity(p);
                }
            }
        }

        // Invariant: hyperedges are never empty. If membership went to
        // zero, kill the hyperedge and cascade.
        if now_empty {
            self.hyper_edge.remove(&id);
            self.entities.remove(&id);
            self.cascade_remove_id(id);
        }

        Ok(())
    }

    pub fn remove_hyperedge_members(
        &mut self,
        id: HyperEdgeId,
        m: HashSet<Pointee>,
    ) -> Result<(), RemoveHyperedgeMembersError> {
        self.silent_remove_hyperedge_members(id, m.clone())?;
        self.emit_patch(Patch::RemoveElementsFromHyperEdge { id, members: m });
        Ok(())
    }

    fn silent_replace_node(
        &mut self,
        id: &NodeId,
        obj: Object,
    ) -> Result<Object, NodeNotFoundError> {
        if !self.is_node(id) {
            return Err(NodeNotFoundError { id: *id });
        }
        // is_node checked the key exists, so insert returns Some(old).
        let old = self
            .entities
            .insert(*id, obj)
            .expect("is_node checked above");
        // Path-pointees through this node may no longer resolve
        // under the new object — cascade the dead ones.
        self.cascade_invalid_paths_through(*id);
        Ok(old)
    }

    /// Replace the object on a node. Records either `ChangeNode` (with
    /// a delta) or `UpsertNode` (full object), whichever is more
    /// compact: delta wins when the number of patch ops is `<=` the
    /// number of fields in the new object.
    pub fn replace_node(&mut self, id: &NodeId, obj: Object) -> Result<Field, NodeNotFoundError> {
        let new_field_count = obj.len();
        let old = self.silent_replace_node(id, obj.clone())?;

        let delta = diff_object(&old, &obj);
        if delta.len() <= new_field_count {
            self.emit_patch(Patch::ChangeNode { id: *id, delta });
        } else {
            self.emit_patch(Patch::UpsertNode { id: *id, obj });
        }

        Ok(Field::Object(old))
    }

    /// Set the attached object on `id`, regardless of whether one was
    /// already there. Used by `apply_patch` for `Upsert{Edge,HyperEdge}Data`
    /// patches — replay must work for both initial attach and replace.
    fn silent_upsert_attached_obj(
        &mut self,
        id: AttachTargetID,
        obj: Object,
    ) -> Result<(), AttachObjectError> {
        let ty = match self.get_type(id) {
            Some(t) => t,
            None => {
                return Err(AttachObjectError::AttachTargetNotFound(
                    AttachTargetNotFoundError { id },
                ))
            }
        };
        // Allow Edge/HyperEdge/MetaEdge (initial attach) AND
        // AttachedObject (already attached → upsert replaces).
        if !matches!(
            ty,
            EntityType::Edge
                | EntityType::HyperEdge
                | EntityType::MetaEdge
                | EntityType::AttachedObject
        ) {
            return Err(AttachObjectError::IncorrectType(IncorrectTypeError {
                entity_id: id,
                expected_type: vec![
                    EntityType::Edge.to_string(),
                    EntityType::HyperEdge.to_string(),
                    EntityType::MetaEdge.to_string(),
                ],
                actual_type: ty.to_string(),
            }));
        }
        self.entities.insert(id, obj);
        // Replace case: paths through this id may no longer resolve.
        // Initial-attach case: no paths existed yet (unattached id couldn't
        // produce a valid Pointee::Path at add time), so this is a no-op.
        self.cascade_invalid_paths_through(id);
        Ok(())
    }

    fn silent_replace_attached_obj(
        &mut self,
        id: &AttachTargetID,
        obj: Object,
    ) -> Result<Object, NoAttachedObjectError> {
        let is_attach_target = self.entities.contains_key(id)
            && (self.edges.contains_key(id) || self.hyper_edge.contains_key(id));
        if !is_attach_target {
            return Err(NoAttachedObjectError { id: *id });
        }
        let old = self
            .entities
            .insert(*id, obj)
            .expect("is_attach_target checked above");
        self.cascade_invalid_paths_through(*id);
        Ok(old)
    }

    /// Replace the attached object on an edge or hyperedge. Strict —
    /// fails if the target has no attached object yet (use
    /// `attach_obj` for that). Records `ChangeEdgeData`/`UpsertEdgeData`
    /// or the HyperEdge variants based on which compresses better:
    /// delta wins when its op count is `<=` the new object's field
    /// count.
    pub fn replace_attached_obj(
        &mut self,
        id: &AttachTargetID,
        obj: Object,
    ) -> Result<Field, NoAttachedObjectError> {
        let new_field_count = obj.len();
        let is_hyper = self.hyper_edge.contains_key(id);

        let old = self.silent_replace_attached_obj(id, obj.clone())?;
        let delta = diff_object(&old, &obj);
        let use_change = delta.len() <= new_field_count;

        match (is_hyper, use_change) {
            (true, true) => self.emit_patch(Patch::ChangeHyperEdgeData { id: *id, delta }),
            (true, false) => self.emit_patch(Patch::UpsertHyperEdgeData { id: *id, obj }),
            (false, true) => self.emit_patch(Patch::ChangeEdgeData { id: *id, delta }),
            (false, false) => self.emit_patch(Patch::UpsertEdgeData { id: *id, obj }),
        }

        Ok(Field::Object(old))
    }

    fn silent_retarget_edge(
        &mut self,
        id: &Uuid,
        new_target: RetargetEdge,
    ) -> Result<(), RetargetError> {
        let (old_source, old_target) = match self.edges.get(id) {
            Some((s, t)) => (s.clone(), t.clone()),
            None => return Err(RetargetError::EdgeNotFound(EdgeNotFoundError { id: *id })),
        };

        let new_pointee = match &new_target {
            RetargetEdge::Source(p) | RetargetEdge::Target(p) => p,
        };
        if !self.is_pointee_exist(new_pointee) {
            return Err(RetargetError::InvalidTarget(InvalidRetargetError {
                edge_id: *id,
                new_target,
            }));
        }

        // Identify which endpoint is being swapped.
        let is_source = matches!(new_target, RetargetEdge::Source(_));
        let (old_endpoint, new_endpoint) = if is_source {
            (old_source.clone(), new_pointee.clone())
        } else {
            (old_target.clone(), new_pointee.clone())
        };

        // No-op — same pointee.
        if old_endpoint == new_endpoint {
            return Ok(());
        }

        // Rewrite the edge.
        let new_pair = if is_source {
            (new_endpoint.clone(), old_target)
        } else {
            (old_source, new_endpoint.clone())
        };
        self.edges.insert(*id, new_pair);

        // Strip eid from the old endpoint's bucket (might empty it).
        if let Some(bucket) = self.pointee_uses.get_mut(&old_endpoint) {
            if is_source {
                bucket.edges_as_source.remove(id);
            } else {
                bucket.edges_as_target.remove(id);
            }
            if bucket.is_empty() {
                self.pointee_uses.remove(&old_endpoint);
                self.untrack_pointee_entity(&old_endpoint);
            }
        }

        // Register eid on the new endpoint's bucket.
        self.track_pointee_entity(&new_endpoint);
        let bucket = self.pointee_uses.entry(new_endpoint).or_default();
        if is_source {
            bucket.edges_as_source.insert(*id);
        } else {
            bucket.edges_as_target.insert(*id);
        }

        Ok(())
    }

    pub fn retarget_edge(
        &mut self,
        id: &Uuid,
        new_target: RetargetEdge,
    ) -> Result<(), RetargetError> {
        self.silent_retarget_edge(id, new_target.clone())?;
        self.emit_patch(Patch::RetargetEdge {
            id: *id,
            new_target,
        });
        Ok(())
    }

    /// Apply a sequence of `ObjectPatch` to a node's or attached
    /// object's `Object` at `id`. Cascades any `Pointee::Path`
    /// references through `id` that no longer resolve under the new
    /// shape. An empty `patch` is a successful no-op.
    fn obj_apply_patch(&mut self, id: Uuid, patch: Vec<ObjectPatch>) -> Result<(), DeltaError> {
        let obj = self
            .entities
            .get_mut(&id)
            .ok_or(DeltaError::NotFound(EntityNotFoundError { id }))?;
        apply_object_patches(obj, patch).map_err(DeltaError::Delta)?;
        self.cascade_invalid_paths_through(id);
        Ok(())
    }

    /// Apply a sequence of [`Patch`]es to this graph in order.
    pub(crate) fn apply_patch(&mut self, delta: Delta) -> Result<(), ApplyPatchError> {
        for patch in delta {
            match patch {
                Patch::AddNode { id, obj } => self.silent_add_node_with_id(id, obj)?,
                Patch::RemoveNode { id } => {
                    self.silent_remove_node(&id)?;
                }
                Patch::ChangeNode { id, delta } => self.obj_apply_patch(id, delta)?,
                Patch::UpsertNode { id, obj } => {
                    self.silent_replace_node(&id, obj)?;
                }
                Patch::AddEdge { id, source, target } => {
                    self.silent_add_edge_with_id(id, source, target)?
                }
                Patch::RemoveEdge { id } => {
                    self.silent_remove_edge(&id)?;
                }
                Patch::RetargetEdge { id, new_target } => {
                    self.silent_retarget_edge(&id, new_target)?
                }
                Patch::UpsertEdgeData { id, obj } => self.silent_upsert_attached_obj(id, obj)?,
                Patch::ChangeEdgeData { id, delta } => self.obj_apply_patch(id, delta)?,
                Patch::RemoveEdgeData { id } => self.silent_remove_attached(id)?,
                Patch::CreateHyperEdge { id, members } => {
                    self.silent_create_hyperedge_with_id(&id, members)?
                }
                Patch::RemoveHyperEdge { id } => {
                    self.silent_remove_hyperedge(&id)?;
                }
                Patch::AddElementsToHyperEdge { id, members } => {
                    self.silent_add_hyperedge_members(id, members)?
                }
                Patch::RemoveElementsFromHyperEdge { id, members } => {
                    self.silent_remove_hyperedge_members(id, members)?
                }
                Patch::UpsertHyperEdgeData { id, obj } => {
                    self.silent_upsert_attached_obj(id, obj)?
                }
                Patch::ChangeHyperEdgeData { id, delta } => self.obj_apply_patch(id, delta)?,
                Patch::RemoveHyperEdgeData { id } => self.silent_remove_attached(id)?,
            }
        }

        Ok(())
    }

    /* ------------ END MODIFIERS ------------- */

    /* ------------ START LISTENERS ----------- */

    fn emit_patch(&mut self, patch: Patch) {
        self.events.push(patch);
    }

    pub fn subscribe_on_change(&mut self, listener: Box<dyn FnMut(Patch)>) -> ListenerId {
        todo!()
    }

    pub fn unsubscribe_on_change(&mut self, id: ListenerId) {
        todo!()
    }

    /* ------------ END LISTENERS ------------- */
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

        /// Cross-check `pointee_uses` and `entity_to_path_pointees`
        /// against `edges` / `hyper_edge`. Panics on the first
        /// inconsistency. Call after any mutation to assert the
        /// reverse-index invariants are intact.
        pub fn check_index_invariant(g: &Graph) {
            // 1) Every Pointee::Path key in pointee_uses must be
            //    tracked in entity_to_path_pointees under its entity.
            for key in g.pointee_uses.keys() {
                if let Pointee::Path(gp) = key {
                    let tracked = g
                        .entity_to_path_pointees
                        .get(&gp.entity())
                        .is_some_and(|s| s.contains(key));
                    assert!(
                        tracked,
                        "path pointee {:?} present in pointee_uses but missing \
                         from entity_to_path_pointees",
                        key
                    );
                }
            }

            // 2) entity_to_path_pointees has no stale or empty entries.
            for (entity, paths) in &g.entity_to_path_pointees {
                assert!(
                    !paths.is_empty(),
                    "entity_to_path_pointees[{}] is empty (should have been removed)",
                    entity
                );
                for p in paths {
                    assert!(
                        g.pointee_uses.contains_key(p),
                        "stale entry in entity_to_path_pointees[{}]: {:?} not in pointee_uses",
                        entity,
                        p
                    );
                }
            }

            // 3) Every edge has both endpoints registered in pointee_uses.
            for (eid, (src, tgt)) in &g.edges {
                let src_ok = g
                    .pointee_uses
                    .get(src)
                    .is_some_and(|b| b.edges_as_source.contains(eid));
                assert!(src_ok, "edge {} not registered in source bucket {:?}", eid, src);
                let tgt_ok = g
                    .pointee_uses
                    .get(tgt)
                    .is_some_and(|b| b.edges_as_target.contains(eid));
                assert!(tgt_ok, "edge {} not registered in target bucket {:?}", eid, tgt);
            }

            // 4) Every hyperedge member has the hyperedge registered.
            for (hid, members) in &g.hyper_edge {
                for m in members {
                    let ok = g
                        .pointee_uses
                        .get(m)
                        .is_some_and(|b| b.hyperedges.contains(hid));
                    assert!(ok, "hyperedge {} not registered in member bucket {:?}", hid, m);
                }
            }

            // 5) No empty buckets — they must have been removed.
            for (k, b) in &g.pointee_uses {
                assert!(
                    !b.is_empty(),
                    "pointee_uses[{:?}] is empty (should have been removed)",
                    k
                );
            }

            // 6) Reverse direction: every (eid, source/target/hyperedge)
            //    in pointee_uses corresponds to a live structural element.
            for (pointee, uses) in &g.pointee_uses {
                for eid in &uses.edges_as_source {
                    let edge = g.edges.get(eid);
                    assert!(
                        edge.is_some_and(|(s, _)| s == pointee),
                        "pointee_uses[{:?}].edges_as_source has stale eid {}",
                        pointee,
                        eid
                    );
                }
                for eid in &uses.edges_as_target {
                    let edge = g.edges.get(eid);
                    assert!(
                        edge.is_some_and(|(_, t)| t == pointee),
                        "pointee_uses[{:?}].edges_as_target has stale eid {}",
                        pointee,
                        eid
                    );
                }
                for hid in &uses.hyperedges {
                    let members = g.hyper_edge.get(hid);
                    assert!(
                        members.is_some_and(|ms| ms.contains(pointee)),
                        "pointee_uses[{:?}].hyperedges has stale hid {}",
                        pointee,
                        hid
                    );
                }
            }
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
        pub fn create_sample_graph1() -> (
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

            let mut m = HashSet::new();
            m.insert(n1.into());
            m.insert(n3.into());

            let h = g.create_hyperedge(m).unwrap();
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
        pub fn create_sample_graph2() -> (
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

            let mut m = HashSet::new();
            m.insert(n3.into());
            m.insert(n4.into());

            let h = graph.create_hyperedge(m).unwrap();
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
        pub fn create_sample_graph3() -> (
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

            /// Iterator yields nodes, edges, and hyperedges all together.
            #[test]
            fn yields_all_entity_kinds() {
                let (g, n1, n2, n3, n4, e1, e2, e3, e4, e5, h) = test_utils::create_sample_graph1();

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
            fn deduplicates_attached_target_ids() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());

                let e1 = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e1, obj.clone()).unwrap();

                let mut m = HashSet::new();
                m.insert(n1.into());
                m.insert(n2.into());

                let h = g.create_hyperedge(m).unwrap();
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
            fn yields_all_edges_in_sample_graph() {
                let (g, _n1, _n2, _n3, _n4, e1, e2, e3, e4, e5, _h) =
                    test_utils::create_sample_graph1();

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
                    test_utils::create_sample_graph1();

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
                    test_utils::create_sample_graph1();

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

        mod test_iter_attached {
            use super::*;

            /// No attached objects yet — only nodes live in
            /// `entities`, so `iter_attached` is empty.
            #[test]
            fn empty_when_only_nodes() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                g.add_node(obj.clone());
                g.add_node(obj);
                assert_eq!(g.iter_attached().count(), 0);
            }

            /// Yields exactly the ids on which `attach_obj` placed an
            /// object — both edges and hyperedges qualify.
            #[test]
            fn yields_attach_targets() {
                let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_sample_graph2();
                let obj = test_utils::create_simple_obj("attached");

                g.attach_obj(e_a, obj.clone()).unwrap();
                g.attach_obj(h, obj).unwrap();

                let actual: HashSet<_> = g.iter_attached().collect();
                let expected: HashSet<_> = [e_a, h].into_iter().collect();
                assert_eq!(actual, expected);
            }

            /// Complement of `iter_nodes`: a node id, even if it has
            /// an object, must NOT appear in `iter_attached`.
            #[test]
            fn excludes_nodes() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj);
                let actual: HashSet<_> = g.iter_attached().collect();
                assert!(!actual.contains(&n1));
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
                    test_utils::create_sample_graph2();

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

            /// Unknown id resolves to `None`.
            #[test]
            fn obj_unknown() {
                let g = Graph::default();
                assert!(g.obj(&Uuid::new_v4()).is_none());
            }

            /// A bare edge (no `attach_obj` call) has no object.
            #[test]
            fn obj_bare_edge_is_none() {
                let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                    test_utils::create_sample_graph2();
                assert!(g.obj(&e_a).is_none());
            }

            /// A bare hyperedge has no object.
            #[test]
            fn obj_bare_hyperedge_is_none() {
                let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_sample_graph2();
                assert!(g.obj(&h).is_none());
            }
        }

        mod test_edge {
            use super::*;

            /// Regular node-to-node edge round-trips through `edge`.
            #[test]
            fn edge1() {
                let (graph, n1, n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                    test_utils::create_sample_graph2();

                let u = graph.edge(&e_a).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: e_a,
                        source: n1.into(),
                        target: n2.into()
                    }
                )
            }

            /// Meta-edge: target is another edge — still in the
            /// `edges` map, so `edge` returns its triplet.
            #[test]
            fn edge2() {
                let (graph, n1, _n2, _n3, _n4, _e_a, e_b, meta_edge, _edge_to_h, _h) =
                    test_utils::create_sample_graph2();

                let u = graph.edge(&meta_edge).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: meta_edge,
                        source: n1.into(),
                        target: e_b.into(),
                    }
                )
            }

            /// Edge whose target is a hyperedge — also lives in the
            /// `edges` map.
            #[test]
            fn edge3() {
                let (graph, n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, edge_to_h, h) =
                    test_utils::create_sample_graph2();

                let u = graph.edge(&edge_to_h).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: edge_to_h,
                        source: n1.into(),
                        target: h.into(),
                    }
                )
            }

            /// Self-loop is a valid edge.
            #[test]
            fn edge4() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj);
                let e1 = g.add_edge(n1, n1).unwrap();

                let u = g.edge(&e1).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: e1,
                        source: n1.into(),
                        target: n1.into(),
                    }
                )
            }

            /// Endpoints can be sub-object paths; `edge` returns
            /// them verbatim.
            #[test]
            fn edge5() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let p1 = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
                let p2 = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let e1 = g.add_edge(p1.clone(), p2.clone()).unwrap();

                let u = g.edge(&e1).unwrap();
                assert_eq!(
                    u,
                    Triplet {
                        id: e1,
                        source: p1,
                        target: p2,
                    }
                )
            }

            /// Unknown id → `NotFound`.
            #[test]
            fn edge_not_found() {
                let g = Graph::default();
                let id = Uuid::new_v4();
                let err = g.edge(&id).unwrap_err();
                assert!(matches!(
                    err,
                    GetEdgeError::NotFound(EntityNotFoundError { id: x }) if x == id
                ));
            }

            /// A node id is a known entity but not an edge →
            /// `IncorrectType("Node")`.
            #[test]
            fn edge_incorrect_type_node() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj);

                let err = g.edge(&n1).unwrap_err();
                match err {
                    GetEdgeError::IncorrectType(e) => {
                        assert_eq!(e.entity_id, n1);
                        assert_eq!(e.actual_type, "Node");
                    }
                    other => panic!("expected IncorrectType, got {other:?}"),
                }
            }

            /// A hyperedge id → `IncorrectType("HyperEdge")`.
            #[test]
            fn edge_incorrect_type_hyperedge() {
                let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_sample_graph2();

                let err = g.edge(&h).unwrap_err();
                match err {
                    GetEdgeError::IncorrectType(e) => {
                        assert_eq!(e.entity_id, h);
                        assert_eq!(e.actual_type, "HyperEdge");
                    }
                    other => panic!("expected IncorrectType, got {other:?}"),
                }
            }

            /// An attached-object id (object placed on top of a
            /// hyperedge) → `IncorrectType("AttachedObject")`. The
            /// hyperedge map lookup wins over the edges map only
            /// because attaching to a hyperedge keeps the id outside
            /// `edges`.
            #[test]
            fn edge_incorrect_type_attached() {
                let (mut g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_sample_graph2();
                let obj = test_utils::create_simple_obj("attached");
                g.attach_obj(h, obj).unwrap();

                let err = g.edge(&h).unwrap_err();
                match err {
                    GetEdgeError::IncorrectType(e) => {
                        assert_eq!(e.entity_id, h);
                        assert_eq!(e.actual_type, "AttachedObject");
                    }
                    other => panic!("expected IncorrectType, got {other:?}"),
                }
            }
        }

        mod test_hyperedge {
            use super::*;

            #[test]
            fn hyperedge1() {
                let (graph, _n1, _n2, n3, n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_sample_graph2();

                let members = graph.hyperedge_members(&h).unwrap();
                let expected: HashSet<Pointee> = [n3.into(), n4.into()].into_iter().collect();
                assert_eq!(members, &expected);
            }

            /// Unknown id → None.
            #[test]
            fn hyperedge_unknown() {
                let g = Graph::default();
                assert!(g.hyperedge_members(&Uuid::new_v4()).is_none());
            }

            /// An edge id is not a hyperedge → None.
            #[test]
            fn hyperedge_for_edge_id() {
                let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                    test_utils::create_sample_graph2();
                assert!(g.hyperedge_members(&e_a).is_none());
            }

            /// `hyperedge_members` returns `None` for an unknown id.
            #[test]
            fn hyperedge_members_unknown() {
                let g = Graph::default();
                assert!(g.hyperedge_members(&Uuid::new_v4()).is_none());
            }
        }

        mod test_probs {
            use super::*;

            mod test_get_type {
                use super::*;

                /// Unknown id resolves to `None`.
                #[test]
                fn unknown_is_none() {
                    let g = Graph::default();
                    assert!(g.get_type(Uuid::new_v4()).is_none());
                }

                /// Pure node — only in `entities`.
                #[test]
                fn node() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    assert!(matches!(g.get_type(n1), Some(EntityType::Node)));
                }

                /// Regular edge — both endpoints are nodes.
                #[test]
                fn edge() {
                    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert!(matches!(g.get_type(e_a), Some(EntityType::Edge)));
                }

                /// Edge whose endpoint is another edge → MetaEdge.
                #[test]
                fn meta_edge_with_edge_endpoint() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert!(matches!(g.get_type(meta_edge), Some(EntityType::MetaEdge)));
                }

                /// Edge whose endpoint is a hyperedge → MetaEdge.
                #[test]
                fn meta_edge_with_hyperedge_endpoint() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert!(matches!(g.get_type(edge_to_h), Some(EntityType::MetaEdge)));
                }

                /// Pure hyperedge — only in `hyper_edge`.
                #[test]
                fn hyperedge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                        test_utils::create_sample_graph2();
                    assert!(matches!(g.get_type(h), Some(EntityType::HyperEdge)));
                }

                /// Object attached on top of an edge — id collides
                /// in both `entities` and `edges` → AttachedObject.
                #[test]
                fn attached_on_edge() {
                    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    let obj = test_utils::create_simple_obj("attached");
                    g.attach_obj(e_a, obj).unwrap();
                    assert!(matches!(g.get_type(e_a), Some(EntityType::AttachedObject)));
                }

                /// Object attached on top of a hyperedge — id
                /// collides in both `entities` and `hyper_edge` →
                /// AttachedObject.
                #[test]
                fn attached_on_hyperedge() {
                    let (mut g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                        test_utils::create_sample_graph2();
                    let obj = test_utils::create_simple_obj("attached");
                    g.attach_obj(h, obj).unwrap();
                    assert!(matches!(g.get_type(h), Some(EntityType::AttachedObject)));
                }

                /// `Pointee::Path` endpoint must NOT promote an edge
                /// to MetaEdge — only edge/hyperedge endpoints do.
                #[test]
                fn path_endpoint_stays_edge() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    let n2 = g.add_node(obj);

                    let p1 = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
                    let e1 = g.add_edge(p1, n2).unwrap();
                    assert!(matches!(g.get_type(e1), Some(EntityType::Edge)));
                }
            }

            mod test_classify_pointee {
                use super::*;

                #[test]
                fn entity_node() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    assert_eq!(g.classify_pointee(&n1.into()), Some(PointeeKind::Node));
                }

                #[test]
                fn entity_edge() {
                    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert_eq!(g.classify_pointee(&e_a.into()), Some(PointeeKind::Edge));
                }

                #[test]
                fn entity_hyperedge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                        test_utils::create_sample_graph2();
                    assert_eq!(g.classify_pointee(&h.into()), Some(PointeeKind::HyperEdge));
                }

                #[test]
                fn entity_meta_edge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert_eq!(
                        g.classify_pointee(&meta_edge.into()),
                        Some(PointeeKind::MetaEdge)
                    );
                }

                #[test]
                fn entity_attached() {
                    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    let obj = test_utils::create_simple_obj("attached");
                    g.attach_obj(e_a, obj).unwrap();
                    assert_eq!(
                        g.classify_pointee(&e_a.into()),
                        Some(PointeeKind::AttachedObject)
                    );
                }

                #[test]
                fn entity_unknown() {
                    let g = Graph::default();
                    let unknown: Pointee = Uuid::new_v4().into();
                    assert_eq!(g.classify_pointee(&unknown), None);
                }

                /// Path that resolves to a real field → Subobject.
                #[test]
                fn path_resolves() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    let p = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
                    assert_eq!(g.classify_pointee(&p), Some(PointeeKind::Subobject));
                }

                /// Path whose entity isn't in the graph → None.
                #[test]
                fn path_unknown_entity() {
                    let g = Graph::default();
                    let p = Pointee::Path(GlobalObjPath::new(Uuid::new_v4(), "x").unwrap());
                    assert_eq!(g.classify_pointee(&p), None);
                }

                /// Path whose entity exists but the field is missing
                /// → None.
                #[test]
                fn path_missing_field() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    let p = Pointee::Path(GlobalObjPath::new(n1, "no_such_field").unwrap());
                    assert_eq!(g.classify_pointee(&p), None);
                }
            }

            mod test_is_exist {
                use super::*;
                #[test]
                fn node_exists() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    assert!(g.is_exist(&n1))
                }

                #[test]
                fn edge_exists() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    let n2 = g.add_node(obj);
                    let e1 = g.add_edge(n1, n2).unwrap();
                    assert!(g.is_exist(&e1))
                }

                #[test]
                fn hyperedge_exists() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    let n2 = g.add_node(obj);
                    let mut m = HashSet::new();
                    m.insert(n1.into());
                    m.insert(n2.into());
                    let h = g.create_hyperedge(m).unwrap();
                    assert!(g.is_exist(&h))
                }

                /// Meta-edge: an edge whose endpoint is another edge.
                #[test]
                fn meta_edge_exists() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj.clone());
                    let n2 = g.add_node(obj);
                    let e1 = g.add_edge(n1, n2).unwrap();
                    let meta_edge = g.add_edge(n1, e1).unwrap();
                    assert!(g.is_exist(&meta_edge))
                }
            }

            mod test_is_pointee_exist {
                use super::*;

                #[test]
                fn entity_node() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    assert!(g.is_pointee_exist(&n1.into()));
                }

                #[test]
                fn entity_edge() {
                    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert!(g.is_pointee_exist(&e_a.into()));
                }

                #[test]
                fn entity_hyperedge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
                        test_utils::create_sample_graph2();
                    assert!(g.is_pointee_exist(&h.into()));
                }

                #[test]
                fn entity_meta_edge() {
                    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    assert!(g.is_pointee_exist(&meta_edge.into()));
                }

                #[test]
                fn entity_attached() {
                    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    let obj = test_utils::create_simple_obj("attached");
                    g.attach_obj(e_a, obj).unwrap();
                    assert!(g.is_pointee_exist(&e_a.into()));
                }

                #[test]
                fn entity_unknown() {
                    let g = Graph::default();
                    let unknown: Pointee = Uuid::new_v4().into();
                    assert!(!g.is_pointee_exist(&unknown));
                }

                /// Path resolves to a real top-level field.
                #[test]
                fn path_resolves() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    let p = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
                    assert!(g.is_pointee_exist(&p));
                }

                /// Path descends through a nested `Field::Object`.
                #[test]
                fn path_resolves_nested() {
                    let mut g = Graph::default();
                    let mut inner = Object::new();
                    inner.insert("leaf".into(), Field::Null);
                    let mut obj = Object::new();
                    obj.insert("nested".into(), Field::Object(inner));
                    let n1 = g.add_node(obj);

                    let mut path = GlobalObjPath::new(n1, "nested").unwrap();
                    path.push("leaf").unwrap();
                    let p = Pointee::Path(path);
                    assert!(g.is_pointee_exist(&p));
                }

                /// Entity isn't in the graph.
                #[test]
                fn path_unknown_entity() {
                    let g = Graph::default();
                    let p = Pointee::Path(GlobalObjPath::new(Uuid::new_v4(), "x").unwrap());
                    assert!(!g.is_pointee_exist(&p));
                }

                /// Entity exists but the first segment doesn't match
                /// any top-level field.
                #[test]
                fn path_missing_field() {
                    let mut g = Graph::default();
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);
                    let p = Pointee::Path(GlobalObjPath::new(n1, "no_such_field").unwrap());
                    assert!(!g.is_pointee_exist(&p));
                }

                /// Walking a path through a non-Object field
                /// (e.g. `Field::Null`) must fail.
                #[test]
                fn path_through_non_object_fails() {
                    let mut g = Graph::default();
                    // "test_field" is a Field::Null — it is not an
                    // Object, so any further descent must fail.
                    let obj = test_utils::create_simple_obj("test_field");
                    let n1 = g.add_node(obj);

                    let mut path = GlobalObjPath::new(n1, "test_field").unwrap();
                    path.push("anything").unwrap();
                    let p = Pointee::Path(path);
                    assert!(!g.is_pointee_exist(&p));
                }

                /// Edge id with a sub-path doesn't navigate — only
                /// `entities` is consulted, and a bare edge has no
                /// attached object.
                #[test]
                fn path_on_bare_edge_fails() {
                    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
                        test_utils::create_sample_graph2();
                    let p = Pointee::Path(GlobalObjPath::new(e_a, "x").unwrap());
                    assert!(!g.is_pointee_exist(&p));
                }
            }
        }
    }

    mod test_constructors {
        use super::*;

        mod test_create_hyperedge {
            use super::*;

            /// Create a hyperedge with two members and verify
            /// both id presence and members round-trip.
            #[test]
            fn members_round_trip() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut members = HashSet::new();
                members.insert(n1.into());
                members.insert(n2.into());

                let h = g.create_hyperedge(members.clone()).unwrap();
                assert!(g.is_exist(&h));
                assert_eq!(g.hyperedge_members(&h), Some(&members));
            }

            /// An empty member set is rejected — every hyperedge
            /// must have at least one member.
            #[test]
            fn create_hyperedge_empty_rejected() {
                let mut g = Graph::default();
                let err = g.create_hyperedge(HashSet::new()).unwrap_err();
                assert_eq!(err, CreateHyperEdgeError::EmptyHyperEdge);
            }

            /// Members may include other hyperedges (nesting) and
            /// edge ids — `create_hyperedge` doesn't validate
            /// membership shape.
            #[test]
            fn create_hyperedge_with_edge_and_hyperedge_members() {
                let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, h) =
                    test_utils::create_sample_graph2();

                let mut members = HashSet::new();
                members.insert(e_a.into());
                members.insert(h.into());

                let h2 = g.create_hyperedge(members.clone()).unwrap();
                assert_eq!(g.hyperedge_members(&h2), Some(&members));
            }

            /// `__create_hyperedge_with_id` rejects a duplicate id.
            #[test]
            fn create_hyperedge_already_exists() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let mut members = HashSet::new();
                members.insert(n1.into());
                let h = g.create_hyperedge(members.clone()).unwrap();
                let err = g
                    .silent_create_hyperedge_with_id(&h, members)
                    .unwrap_err();
                assert_eq!(
                    err,
                    CreateHyperEdgeError::HyperEdgeAlreadyExists(HyperEdgeAlreadyExistsError {
                        id: h
                    })
                );
            }

            /// Re-inserting with the same id must NOT clobber the
            /// existing members — the original hyperedge stays
            /// intact.
            #[test]
            fn create_hyperedge_already_exists_preserves_members() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                // Try to overwrite with a different (non-empty) member set.
                let mut other = HashSet::new();
                other.insert(n2.into());
                let _ = g.silent_create_hyperedge_with_id(&h, other);

                assert_eq!(g.hyperedge_members(&h), Some(&original));
            }
        }

        mod test_add_node {
            use super::*;

            /// Adding a node returns an id and the object can be looked up.
            #[test]
            fn add_then_lookup() {
                let mut graph = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let node_id = graph.add_node(obj.clone());
                assert_eq!(graph.obj(&node_id), Some(&obj));
            }

            /// Re-inserting under the same id is rejected and the
            /// original object stays untouched.
            #[test]
            fn rejects_duplicate_id() {
                let mut graph = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = graph.add_node(obj.clone());
                let obj2 = test_utils::create_simple_obj("test_field2");
                let result2 = graph.silent_add_node_with_id(n1, obj2.clone());
                assert_eq!(
                    result2.clone().unwrap_err(),
                    NodeAlreadyExistsError {
                        id: result2.unwrap_err().id
                    }
                );
                // Check thats change doesnt apply
                assert_eq!(graph.obj(&n1), Some(&obj))
            }
        }

        mod test_add_edge {
            use super::*;

            /// Adding a basic edge stores its (source, target) pair.
            #[test]
            fn add_basic_edge() {
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
            fn allows_self_loop() {
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

            /// Both endpoints unresolved → MissingEndpoints with both ids.
            #[test]
            fn rejects_both_endpoints_missing() {
                let mut g = Graph::default();
                let n1 = Uuid::new_v4();
                let n2 = Uuid::new_v4();

                let err = g.add_edge(n1, n2).unwrap_err();
                assert_eq!(
                    err,
                    AddEdgeError::MissingEndpoints(MissingEndpointsError {
                        missing_endpoints: vec![Pointee::EntityId(n1), Pointee::EntityId(n2)],
                    })
                )
            }

            /// Re-inserting an edge under the same id is rejected.
            #[test]
            fn rejects_duplicate_edge_id() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let e1 = g.add_edge(n1, n2).unwrap();
                let err = g
                    .silent_add_edge_with_id(e1, n1.into(), n2.into())
                    .unwrap_err();
                assert_eq!(
                    err,
                    AddEdgeError::EdgeAlreadyExists(EdgeAlreadyExistsError { id: e1 })
                )
            }
        }
    }

    mod test_destructors {
        use super::*;

        mod test_remove_node {
            use super::*;

            /// `remove_node` rejects an unknown id.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g.remove_node(&Uuid::new_v4()).unwrap_err();
                assert!(matches!(err, NodeNotFoundError { .. }));
            }

            /// Removing a node returns its attached object as Field::Object.
            #[test]
            fn returns_object() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let returned = g.remove_node(&n1).unwrap();
                assert_eq!(returned, Field::Object(obj));
                assert!(!g.is_exist(&n1));
                test_utils::check_index_invariant(&g);
            }

            /// Removing `n1` cascades to `e: n1 → n2`.
            /// `n2` survives but its target bucket no longer contains `e`.
            #[test]
            fn cascades_direct_edge_reference() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.is_exist(&n1));
                assert!(!g.edges.contains_key(&e));
                assert!(g.is_exist(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// Removing `n2` cascades to `e: n1 → n2/field` (Path-pointee).
            /// Verifies the `entity_to_path_pointees` lookup path.
            #[test]
            fn cascades_path_reference() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let e = g.add_edge(n1, path.clone()).unwrap();

                g.remove_node(&n2).unwrap();

                assert!(!g.is_exist(&n2));
                assert!(!g.edges.contains_key(&e));
                assert!(g.is_exist(&n1));
                assert!(!g.entity_to_path_pointees.contains_key(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// Self-loop `e: n1 → n1`: index is fully drained on removal.
            #[test]
            fn cascades_self_loop() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj);
                let e = g.add_edge(n1, n1).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.edges.contains_key(&e));
                assert!(g.pointee_uses.is_empty());
                assert!(g.entity_to_path_pointees.is_empty());
                test_utils::check_index_invariant(&g);
            }

            /// Chain: `e1: n1 → n2`, `e2: n3 → e1`. Removing `n1` must
            /// cascade to `e1` and then to `e2` (since `e1` is `e2`'s target).
            #[test]
            fn cascades_through_meta_edge() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);

                let e1 = g.add_edge(n1, n2).unwrap();
                let e2 = g.add_edge(n3, e1).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.edges.contains_key(&e1));
                assert!(!g.edges.contains_key(&e2));
                assert!(g.is_exist(&n2));
                assert!(g.is_exist(&n3));
                test_utils::check_index_invariant(&g);
            }

            /// Hyperedge with two members loses one — survives with the other.
            #[test]
            fn hyperedge_loses_member_but_survives() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                m.insert(n2.into());
                let h = g.create_hyperedge(m).unwrap();

                g.remove_node(&n1).unwrap();

                let mut expected = HashSet::new();
                expected.insert(n2.into());
                assert_eq!(g.hyperedge_members(&h), Some(&expected));
                test_utils::check_index_invariant(&g);
            }

            /// Hyperedge with the only member `n1` becomes empty when `n1` dies
            /// and is itself cascade-removed.
            #[test]
            fn hyperedge_empties_and_dies() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.hyper_edge.contains_key(&h));
                test_utils::check_index_invariant(&g);
            }

            /// Cascade reaches edges that pointed at a hyperedge that
            /// itself died from emptying.
            #[test]
            fn cascade_through_dead_hyperedge() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                // edge from n2 to the soon-to-die hyperedge
                let e = g.add_edge(n2, h).unwrap();

                g.remove_node(&n1).unwrap();

                assert!(!g.hyper_edge.contains_key(&h));
                assert!(!g.edges.contains_key(&e));
                assert!(g.is_exist(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// `remove_node` rejects ids that live in `entities` because of
            /// `attach_obj` on an edge — those are NOT nodes.
            #[test]
            fn rejects_attached_object_id() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj).unwrap();

                let err = g.remove_node(&e).unwrap_err();
                assert!(matches!(err, NodeNotFoundError { .. }));
                // Edge itself untouched.
                assert!(g.edges.contains_key(&e));
                test_utils::check_index_invariant(&g);
            }

            /// Records exactly one `Patch::RemoveNode` even on a cascade.
            #[test]
            fn records_single_patch_on_cascade() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let _e = g.add_edge(n1, n2).unwrap();

                g.remove_node(&n1).unwrap();

                let last = g.events.last().unwrap();
                assert_eq!(*last, Patch::RemoveNode { id: n1 });
                let remove_count = g
                    .events
                    .iter()
                    .filter(|p| matches!(p, Patch::RemoveNode { .. }))
                    .count();
                assert_eq!(remove_count, 1);
            }
        }

        mod test_remove_hyperedge {
            use super::*;

            /// Removing an unknown id returns an error.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g.remove_hyperedge(&Uuid::new_v4()).unwrap_err();
                assert!(matches!(err, HyperEdgeNotFoundError { .. }));
            }

            /// Plain remove: hyperedge gone, members survive without
            /// `hid` in their `hyperedges` set.
            #[test]
            fn removes_and_unregisters_members() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                m.insert(n2.into());
                let h = g.create_hyperedge(m.clone()).unwrap();

                let returned = g.remove_hyperedge(&h).unwrap();

                assert_eq!(returned, m);
                assert!(!g.hyper_edge.contains_key(&h));
                assert!(g.is_exist(&n1));
                assert!(g.is_exist(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// Edges that pointed at the hyperedge cascade away.
            #[test]
            fn cascades_to_referencing_edges() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                let e = g.add_edge(n2, h).unwrap();

                g.remove_hyperedge(&h).unwrap();

                assert!(!g.hyper_edge.contains_key(&h));
                assert!(!g.edges.contains_key(&e));
                assert!(g.is_exist(&n2));
                test_utils::check_index_invariant(&g);
            }

            /// Removing a hyperedge that's a member of another hyperedge:
            /// the parent loses this member; if parent becomes empty, it
            /// dies too.
            #[test]
            fn cascades_to_parent_hyperedge_when_emptied() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj);

                let mut inner_m = HashSet::new();
                inner_m.insert(n1.into());
                let inner = g.create_hyperedge(inner_m).unwrap();

                let mut outer_m = HashSet::new();
                outer_m.insert(inner.into());
                let outer = g.create_hyperedge(outer_m).unwrap();

                g.remove_hyperedge(&inner).unwrap();

                assert!(!g.hyper_edge.contains_key(&inner));
                assert!(!g.hyper_edge.contains_key(&outer));
                test_utils::check_index_invariant(&g);
            }

            /// Parent hyperedge with multiple members loses one — survives.
            #[test]
            fn parent_hyperedge_loses_only_this_member() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut inner_m = HashSet::new();
                inner_m.insert(n1.into());
                let inner = g.create_hyperedge(inner_m).unwrap();

                let mut outer_m = HashSet::new();
                outer_m.insert(inner.into());
                outer_m.insert(n2.into());
                let outer = g.create_hyperedge(outer_m).unwrap();

                g.remove_hyperedge(&inner).unwrap();

                let mut expected = HashSet::new();
                expected.insert(n2.into());
                assert_eq!(g.hyperedge_members(&outer), Some(&expected));
                test_utils::check_index_invariant(&g);
            }

            /// Attached object on the hyperedge is dropped along with it.
            #[test]
            fn drops_attached_object() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                g.attach_obj(h, obj).unwrap();
                assert!(g.entities.contains_key(&h));

                g.remove_hyperedge(&h).unwrap();

                assert!(!g.entities.contains_key(&h));
                test_utils::check_index_invariant(&g);
            }

            /// Records exactly one `Patch::RemoveHyperEdge` even on a cascade.
            #[test]
            fn records_single_patch_on_cascade() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                let _e = g.add_edge(n2, h).unwrap();

                g.remove_hyperedge(&h).unwrap();

                let last = g.events.last().unwrap();
                assert_eq!(*last, Patch::RemoveHyperEdge { id: h });
                let count = g
                    .events
                    .iter()
                    .filter(|p| matches!(p, Patch::RemoveHyperEdge { .. }))
                    .count();
                assert_eq!(count, 1);
            }
        }

        mod test_remove_attached {
            use super::*;

            /// Unknown id — neither node nor attach target.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g.remove_attached(Uuid::new_v4()).unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
            }

            /// A bare node has no attached object — removal must fail
            /// rather than silently delete the node.
            #[test]
            fn rejects_node_id() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));

                let err = g.remove_attached(n1).unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
                assert!(g.is_exist(&n1));
                assert!(g.entities.contains_key(&n1));
            }

            /// A bare edge (no attach_obj called) — nothing to remove.
            #[test]
            fn rejects_bare_edge() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                let err = g.remove_attached(e).unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
                assert!(g.edges.contains_key(&e));
            }

            /// Attached object on an edge: edge stays alive, attached
            /// object gone, RemoveEdgeData patch recorded.
            #[test]
            fn removes_edge_attached() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj).unwrap();

                g.remove_attached(e).unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(!g.entities.contains_key(&e));
                assert_eq!(*g.events.last().unwrap(), Patch::RemoveEdgeData { id: e });
                test_utils::check_index_invariant(&g);
            }

            /// Attached object on a hyperedge: hyperedge stays alive,
            /// attached object gone, RemoveHyperEdgeData patch recorded.
            #[test]
            fn removes_hyperedge_attached() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());

                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                g.attach_obj(h, obj).unwrap();

                g.remove_attached(h).unwrap();

                assert!(g.hyper_edge.contains_key(&h));
                assert!(!g.entities.contains_key(&h));
                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::RemoveHyperEdgeData { id: h }
                );
                test_utils::check_index_invariant(&g);
            }

            /// EntityId references survive — only Path references die.
            #[test]
            fn entity_id_references_survive() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj).unwrap();

                // Edge that points at `e` as a whole entity.
                let n3 = g.add_node(test_utils::create_simple_obj("g"));
                let meta = g.add_edge(n3, e).unwrap();

                g.remove_attached(e).unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(g.edges.contains_key(&meta));
                test_utils::check_index_invariant(&g);
            }

            /// Path references through the attach target die.
            #[test]
            fn cascades_path_references() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                let attached = test_utils::create_simple_obj("data");
                g.attach_obj(e, attached).unwrap();

                // Edge whose endpoint is a path through `e`'s attached object.
                let n3 = g.add_node(test_utils::create_simple_obj("g"));
                let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
                let dangling = g.add_edge(n3, path).unwrap();

                g.remove_attached(e).unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(!g.edges.contains_key(&dangling));
                assert!(!g.entity_to_path_pointees.contains_key(&e));
                test_utils::check_index_invariant(&g);
            }

            /// Records exactly one data-removal patch even when the
            /// path-cascade kills downstream structures.
            #[test]
            fn records_single_patch_on_cascade() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, test_utils::create_simple_obj("data"))
                    .unwrap();

                let n3 = g.add_node(obj);
                let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
                let _dangling = g.add_edge(n3, path).unwrap();

                g.remove_attached(e).unwrap();

                let count = g
                    .events
                    .iter()
                    .filter(|p| {
                        matches!(
                            p,
                            Patch::RemoveEdgeData { .. } | Patch::RemoveHyperEdgeData { .. }
                        )
                    })
                    .count();
                assert_eq!(count, 1);
            }
        }
    }

    mod test_modifiers {
        use super::*;

        mod test_attach_obj {
            use super::*;

            /// Attaching to an unknown id is rejected — no patch recorded.
            #[test]
            fn unknown_target() {
                let mut g = Graph::default();
                let err = g
                    .attach_obj(Uuid::new_v4(), test_utils::create_simple_obj("f"))
                    .unwrap_err();
                assert!(matches!(err, AttachObjectError::AttachTargetNotFound(_)));
                assert!(g.events.is_empty());
            }

            /// A node is not an attach target.
            #[test]
            fn rejects_node() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let err = g
                    .attach_obj(n1, test_utils::create_simple_obj("g"))
                    .unwrap_err();
                assert!(matches!(err, AttachObjectError::IncorrectType(_)));
                let extra_events = g
                    .events
                    .iter()
                    .filter(|p| {
                        matches!(p, Patch::UpsertEdgeData { .. } | Patch::UpsertHyperEdgeData { .. })
                    })
                    .count();
                assert_eq!(extra_events, 0);
            }

            /// Re-attaching is rejected (target is already AttachedObject).
            #[test]
            fn rejects_double_attach() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj.clone()).unwrap();

                let err = g.attach_obj(e, obj).unwrap_err();
                assert!(matches!(err, AttachObjectError::IncorrectType(_)));
                let count = g
                    .events
                    .iter()
                    .filter(|p| matches!(p, Patch::UpsertEdgeData { .. }))
                    .count();
                assert_eq!(count, 1, "second attach must NOT have recorded a patch");
            }

            /// Attach onto a plain edge → records UpsertEdgeData.
            #[test]
            fn records_upsert_edge_data() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                let attached = test_utils::create_simple_obj("data");

                g.attach_obj(e, attached.clone()).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::UpsertEdgeData {
                        id: e,
                        obj: attached,
                    }
                );
            }

            /// Attach onto a hyperedge → records UpsertHyperEdgeData.
            #[test]
            fn records_upsert_hyperedge_data() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                let attached = test_utils::create_simple_obj("data");

                g.attach_obj(h, attached.clone()).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::UpsertHyperEdgeData {
                        id: h,
                        obj: attached,
                    }
                );
            }

            /// Attach onto a meta-edge (edge whose endpoint is another
            /// edge) — still recorded as UpsertEdgeData since meta-edges
            /// live in `self.edges`.
            #[test]
            fn meta_edge_records_upsert_edge_data() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e1 = g.add_edge(n1, n2).unwrap();
                let n3 = g.add_node(obj.clone());
                let meta = g.add_edge(n3, e1).unwrap();
                let attached = test_utils::create_simple_obj("data");

                g.attach_obj(meta, attached.clone()).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::UpsertEdgeData {
                        id: meta,
                        obj: attached,
                    }
                );
            }
        }

        mod test_add_hyperedge_members {
            use super::*;

            /// Unknown hyperedge id is rejected.
            #[test]
            fn unknown_hyperedge() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let mut m = HashSet::new();
                m.insert(n1.into());
                let err = g.add_hyperedge_members(Uuid::new_v4(), m).unwrap_err();
                assert!(matches!(err, AddHyperedgeMembersError::HyperEdgeNotFound(_)));
            }

            /// Members that don't exist as pointees are rejected.
            #[test]
            fn missing_pointee() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original).unwrap();

                let mut m = HashSet::new();
                m.insert(Pointee::EntityId(Uuid::new_v4()));
                let err = g.add_hyperedge_members(h, m).unwrap_err();
                assert!(matches!(err, AddHyperedgeMembersError::PointeesNotFound(_)));
            }

            /// Adding a member that's already there is rejected,
            /// and nothing is partially applied.
            #[test]
            fn duplicate_member_rejected_atomically() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                let mut m = HashSet::new();
                m.insert(n1.into()); // duplicate
                m.insert(n2.into()); // would-be new
                let err = g.add_hyperedge_members(h, m).unwrap_err();
                assert!(matches!(err, AddHyperedgeMembersError::MembersAlreadyExist(_)));

                // Atomicity: n2 was NOT added.
                assert_eq!(g.hyperedge_members(&h), Some(&original));
                test_utils::check_index_invariant(&g);
            }

            /// Successful add: members extended, reverse index updated,
            /// patch recorded.
            #[test]
            fn adds_members_and_records_patch() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original).unwrap();

                let mut to_add = HashSet::new();
                to_add.insert(n2.into());
                to_add.insert(n3.into());
                g.add_hyperedge_members(h, to_add.clone()).unwrap();

                let mut expected = HashSet::new();
                expected.insert(n1.into());
                expected.insert(n2.into());
                expected.insert(n3.into());
                assert_eq!(g.hyperedge_members(&h), Some(&expected));

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::AddElementsToHyperEdge {
                        id: h,
                        members: to_add,
                    }
                );
                test_utils::check_index_invariant(&g);
            }

            /// Adding a Path-pointee tracks it in `entity_to_path_pointees`.
            #[test]
            fn tracks_path_member() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original).unwrap();

                let path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let mut to_add = HashSet::new();
                to_add.insert(path.clone());
                g.add_hyperedge_members(h, to_add).unwrap();

                assert!(g
                    .entity_to_path_pointees
                    .get(&n2)
                    .is_some_and(|s| s.contains(&path)));
                test_utils::check_index_invariant(&g);
            }

            /// Empty input is a no-op success.
            #[test]
            fn empty_input_is_noop() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj);
                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                g.add_hyperedge_members(h, HashSet::new()).unwrap();

                assert_eq!(g.hyperedge_members(&h), Some(&original));
                test_utils::check_index_invariant(&g);
            }
        }

        mod test_remove_hyperedge_members {
            use super::*;

            /// Unknown hyperedge id is rejected.
            #[test]
            fn unknown_hyperedge() {
                let mut g = Graph::default();
                let mut m = HashSet::new();
                m.insert(Pointee::EntityId(Uuid::new_v4()));
                let err = g.remove_hyperedge_members(Uuid::new_v4(), m).unwrap_err();
                assert!(matches!(err, RemoveHyperedgeMembersError::HyperEdgeNotFound(_)));
            }

            /// Removing a pointee that's not a current member is rejected,
            /// and nothing is partially applied.
            #[test]
            fn member_not_in_hyperedge_atomic() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                original.insert(n2.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                let mut m = HashSet::new();
                m.insert(n1.into()); // valid
                m.insert(n3.into()); // not a member
                let err = g.remove_hyperedge_members(h, m).unwrap_err();
                assert!(matches!(
                    err,
                    RemoveHyperedgeMembersError::MembersNotInHyperedge(_)
                ));

                // Atomicity: n1 was NOT removed.
                assert_eq!(g.hyperedge_members(&h), Some(&original));
                test_utils::check_index_invariant(&g);
            }

            /// Successful partial removal: hyperedge survives with the rest.
            #[test]
            fn removes_subset_and_records_patch() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                original.insert(n2.into());
                let h = g.create_hyperedge(original).unwrap();

                let mut to_remove = HashSet::new();
                to_remove.insert(n1.into());
                g.remove_hyperedge_members(h, to_remove.clone()).unwrap();

                let mut expected = HashSet::new();
                expected.insert(n2.into());
                assert_eq!(g.hyperedge_members(&h), Some(&expected));

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::RemoveElementsFromHyperEdge {
                        id: h,
                        members: to_remove,
                    }
                );
                test_utils::check_index_invariant(&g);
            }

            /// Reverse index is cleaned: removed member's bucket no longer
            /// references this hyperedge.
            #[test]
            fn removed_member_loses_hyperedge_link() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                original.insert(n2.into());
                let h = g.create_hyperedge(original).unwrap();

                let mut to_remove = HashSet::new();
                to_remove.insert(n1.into());
                g.remove_hyperedge_members(h, to_remove).unwrap();

                // n1 had only this hyperedge link → bucket fully gone.
                assert!(!g.pointee_uses.contains_key(&Pointee::EntityId(n1)));
                test_utils::check_index_invariant(&g);
            }

            /// Removing all members empties the hyperedge — it dies and
            /// any references to it cascade.
            #[test]
            fn empties_and_kills_hyperedge() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();
                let e = g.add_edge(n2, h).unwrap();

                g.remove_hyperedge_members(h, original).unwrap();

                assert!(!g.hyper_edge.contains_key(&h));
                assert!(!g.edges.contains_key(&e));
                test_utils::check_index_invariant(&g);
            }

            /// Empty input is a no-op success.
            #[test]
            fn empty_input_is_noop() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let mut original = HashSet::new();
                original.insert(n1.into());
                let h = g.create_hyperedge(original.clone()).unwrap();

                g.remove_hyperedge_members(h, HashSet::new()).unwrap();

                assert_eq!(g.hyperedge_members(&h), Some(&original));
                test_utils::check_index_invariant(&g);
            }

            /// Path-pointee removal also untracks from entity_to_path_pointees
            /// (when its bucket fully empties).
            #[test]
            fn removes_path_member_and_untracks() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);

                let path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let mut original = HashSet::new();
                original.insert(n1.into());
                original.insert(path.clone());
                let h = g.create_hyperedge(original).unwrap();

                let mut to_remove = HashSet::new();
                to_remove.insert(path.clone());
                g.remove_hyperedge_members(h, to_remove).unwrap();

                assert!(!g.pointee_uses.contains_key(&path));
                assert!(!g.entity_to_path_pointees.contains_key(&n2));
                test_utils::check_index_invariant(&g);
            }
        }

        mod test_replace_node {
            use super::*;

            fn obj_with(fields: &[(&str, Field)]) -> Object {
                let mut o = Object::new();
                for (k, v) in fields {
                    o.insert((*k).into(), v.clone());
                }
                o
            }

            /// Unknown id is rejected.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g
                    .replace_node(&Uuid::new_v4(), test_utils::create_simple_obj("f"))
                    .unwrap_err();
                assert!(matches!(err, NodeNotFoundError { .. }));
            }

            /// Edges (with attached object) are not nodes — rejected.
            #[test]
            fn rejects_attached_target() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj.clone()).unwrap();

                let err = g.replace_node(&e, obj).unwrap_err();
                assert!(matches!(err, NodeNotFoundError { .. }));
            }

            /// Returns the previous object (wrapped in `Field::Object`).
            #[test]
            fn returns_old_object() {
                let mut g = Graph::default();
                let old = obj_with(&[("a", Field::Number(1))]);
                let n1 = g.add_node(old.clone());
                let new = obj_with(&[("a", Field::Number(2))]);

                let returned = g.replace_node(&n1, new).unwrap();
                assert_eq!(returned, Field::Object(old));
            }

            /// Few changed fields (delta_size <= new_size) → ChangeNode.
            #[test]
            fn small_delta_emits_change_node() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[
                    ("a", Field::Number(1)),
                    ("b", Field::Number(2)),
                    ("c", Field::Number(3)),
                ]));

                let new = obj_with(&[
                    ("a", Field::Number(1)),
                    ("b", Field::Number(2)),
                    ("c", Field::Number(99)), // only c changed
                ]);
                g.replace_node(&n1, new).unwrap();

                match g.events.last().unwrap() {
                    Patch::ChangeNode { id, delta } => {
                        assert_eq!(*id, n1);
                        assert_eq!(delta.len(), 1);
                        assert!(matches!(
                            &delta[0],
                            ObjectPatch::UpsertField { name, .. } if name == "c"
                        ));
                    }
                    other => panic!("expected ChangeNode, got {:?}", other),
                }
            }

            /// Many changes (delta_size > new_size) → UpsertNode.
            /// Old: {a,b,c}, New: {x,y,z} → delta would be 6 ops, new has 3 fields.
            #[test]
            fn large_delta_emits_upsert_node() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[
                    ("a", Field::Number(1)),
                    ("b", Field::Number(2)),
                    ("c", Field::Number(3)),
                ]));

                let new = obj_with(&[
                    ("x", Field::Number(10)),
                    ("y", Field::Number(20)),
                    ("z", Field::Number(30)),
                ]);
                g.replace_node(&n1, new.clone()).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::UpsertNode { id: n1, obj: new }
                );
            }

            /// Boundary `delta_size == new_size` — still ChangeNode (`<=`).
            #[test]
            fn equal_size_emits_change_node() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[
                    ("a", Field::Number(1)),
                    ("b", Field::Number(2)),
                ]));

                // Both fields changed → 2 UpsertField ops, new has 2 fields.
                let new = obj_with(&[
                    ("a", Field::Number(10)),
                    ("b", Field::Number(20)),
                ]);
                g.replace_node(&n1, new).unwrap();

                assert!(matches!(
                    g.events.last().unwrap(),
                    Patch::ChangeNode { .. }
                ));
            }

            /// Identical replacement: delta is empty → ChangeNode with empty delta.
            #[test]
            fn identical_obj_emits_empty_change_node() {
                let mut g = Graph::default();
                let obj = obj_with(&[("a", Field::Number(1))]);
                let n1 = g.add_node(obj.clone());

                g.replace_node(&n1, obj).unwrap();

                match g.events.last().unwrap() {
                    Patch::ChangeNode { id, delta } => {
                        assert_eq!(*id, n1);
                        assert!(delta.is_empty());
                    }
                    other => panic!("expected ChangeNode, got {:?}", other),
                }
            }

            /// Path-pointee that still resolves under the new object survives.
            #[test]
            fn path_pointee_survives_when_field_kept() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("data", Field::Number(1))]));
                let n2 = g.add_node(obj_with(&[("x", Field::Null)]));
                let path = Pointee::Path(GlobalObjPath::new(n1, "data").unwrap());
                let e = g.add_edge(n2, path.clone()).unwrap();

                // Replace but keep `data` field.
                g.replace_node(&n1, obj_with(&[("data", Field::Number(99))]))
                    .unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(g.pointee_uses.contains_key(&path));
                test_utils::check_index_invariant(&g);
            }

            /// Path-pointee that no longer resolves cascades away.
            #[test]
            fn path_pointee_cascades_when_field_dropped() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("data", Field::Number(1))]));
                let n2 = g.add_node(obj_with(&[("x", Field::Null)]));
                let path = Pointee::Path(GlobalObjPath::new(n1, "data").unwrap());
                let e = g.add_edge(n2, path.clone()).unwrap();

                // Replace with an object that lacks `data`.
                g.replace_node(&n1, obj_with(&[("other", Field::Null)]))
                    .unwrap();

                assert!(!g.edges.contains_key(&e));
                assert!(!g.pointee_uses.contains_key(&path));
                assert!(!g.entity_to_path_pointees.contains_key(&n1));
                // Node itself still alive.
                assert!(g.is_exist(&n1));
                test_utils::check_index_invariant(&g);
            }

            /// EntityId references survive — only path references can die.
            #[test]
            fn entity_id_references_survive() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("a", Field::Null)]));
                let n2 = g.add_node(obj_with(&[("b", Field::Null)]));
                let e = g.add_edge(n2, n1).unwrap();

                g.replace_node(&n1, obj_with(&[("c", Field::Null)]))
                    .unwrap();

                assert!(g.edges.contains_key(&e));
                test_utils::check_index_invariant(&g);
            }
        }

        mod test_replace_attached_obj {
            use super::*;

            fn obj_with(fields: &[(&str, Field)]) -> Object {
                let mut o = Object::new();
                for (k, v) in fields {
                    o.insert((*k).into(), v.clone());
                }
                o
            }

            /// Unknown id is rejected.
            #[test]
            fn unknown_id() {
                let mut g = Graph::default();
                let err = g
                    .replace_attached_obj(&Uuid::new_v4(), obj_with(&[]))
                    .unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
            }

            /// A node is not an attach target.
            #[test]
            fn rejects_node() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("a", Field::Null)]));
                let err = g.replace_attached_obj(&n1, obj_with(&[])).unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
            }

            /// A bare edge (no attach_obj called) — rejected.
            #[test]
            fn rejects_bare_edge() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("a", Field::Null)]));
                let n2 = g.add_node(obj_with(&[("a", Field::Null)]));
                let e = g.add_edge(n1, n2).unwrap();

                let err = g.replace_attached_obj(&e, obj_with(&[])).unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
            }

            /// A bare hyperedge — rejected.
            #[test]
            fn rejects_bare_hyperedge() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("a", Field::Null)]));
                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();

                let err = g.replace_attached_obj(&h, obj_with(&[])).unwrap_err();
                assert!(matches!(err, NoAttachedObjectError { .. }));
            }

            /// Returns the previous attached object.
            #[test]
            fn returns_old_object() {
                let mut g = Graph::default();
                let obj = obj_with(&[("a", Field::Null)]);
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();
                let old = obj_with(&[("data", Field::Number(1))]);
                g.attach_obj(e, old.clone()).unwrap();

                let returned = g
                    .replace_attached_obj(&e, obj_with(&[("data", Field::Number(2))]))
                    .unwrap();
                assert_eq!(returned, Field::Object(old));
            }

            /// Edge with small delta → ChangeEdgeData.
            #[test]
            fn edge_small_delta_emits_change() {
                let mut g = Graph::default();
                let leaf = obj_with(&[("a", Field::Null)]);
                let n1 = g.add_node(leaf.clone());
                let n2 = g.add_node(leaf);
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(
                    e,
                    obj_with(&[
                        ("a", Field::Number(1)),
                        ("b", Field::Number(2)),
                        ("c", Field::Number(3)),
                    ]),
                )
                .unwrap();

                g.replace_attached_obj(
                    &e,
                    obj_with(&[
                        ("a", Field::Number(1)),
                        ("b", Field::Number(2)),
                        ("c", Field::Number(99)),
                    ]),
                )
                .unwrap();

                match g.events.last().unwrap() {
                    Patch::ChangeEdgeData { id, delta } => {
                        assert_eq!(*id, e);
                        assert_eq!(delta.len(), 1);
                    }
                    other => panic!("expected ChangeEdgeData, got {:?}", other),
                }
            }

            /// Edge with large delta → UpsertEdgeData.
            #[test]
            fn edge_large_delta_emits_upsert() {
                let mut g = Graph::default();
                let leaf = obj_with(&[("a", Field::Null)]);
                let n1 = g.add_node(leaf.clone());
                let n2 = g.add_node(leaf);
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(
                    e,
                    obj_with(&[
                        ("a", Field::Number(1)),
                        ("b", Field::Number(2)),
                        ("c", Field::Number(3)),
                    ]),
                )
                .unwrap();

                let new = obj_with(&[
                    ("x", Field::Number(10)),
                    ("y", Field::Number(20)),
                    ("z", Field::Number(30)),
                ]);
                g.replace_attached_obj(&e, new.clone()).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::UpsertEdgeData { id: e, obj: new }
                );
            }

            /// Hyperedge with small delta → ChangeHyperEdgeData.
            #[test]
            fn hyperedge_small_delta_emits_change() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("x", Field::Null)]));
                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                g.attach_obj(
                    h,
                    obj_with(&[("a", Field::Number(1)), ("b", Field::Number(2))]),
                )
                .unwrap();

                g.replace_attached_obj(
                    &h,
                    obj_with(&[("a", Field::Number(1)), ("b", Field::Number(99))]),
                )
                .unwrap();

                match g.events.last().unwrap() {
                    Patch::ChangeHyperEdgeData { id, delta } => {
                        assert_eq!(*id, h);
                        assert_eq!(delta.len(), 1);
                    }
                    other => panic!("expected ChangeHyperEdgeData, got {:?}", other),
                }
            }

            /// Hyperedge with large delta → UpsertHyperEdgeData.
            #[test]
            fn hyperedge_large_delta_emits_upsert() {
                let mut g = Graph::default();
                let n1 = g.add_node(obj_with(&[("x", Field::Null)]));
                let mut m = HashSet::new();
                m.insert(n1.into());
                let h = g.create_hyperedge(m).unwrap();
                g.attach_obj(
                    h,
                    obj_with(&[
                        ("a", Field::Number(1)),
                        ("b", Field::Number(2)),
                        ("c", Field::Number(3)),
                    ]),
                )
                .unwrap();

                let new = obj_with(&[
                    ("x", Field::Number(10)),
                    ("y", Field::Number(20)),
                    ("z", Field::Number(30)),
                ]);
                g.replace_attached_obj(&h, new.clone()).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::UpsertHyperEdgeData { id: h, obj: new }
                );
            }

            /// Path-pointee through the attach target gets cascaded if
            /// the field it pointed at is dropped by the replacement.
            #[test]
            fn path_pointee_cascades_when_field_dropped() {
                let mut g = Graph::default();
                let leaf = obj_with(&[("a", Field::Null)]);
                let n1 = g.add_node(leaf.clone());
                let n2 = g.add_node(leaf);
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj_with(&[("data", Field::Number(1))]))
                    .unwrap();

                let n3 = g.add_node(obj_with(&[("a", Field::Null)]));
                let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
                let dangling = g.add_edge(n3, path.clone()).unwrap();

                g.replace_attached_obj(&e, obj_with(&[("other", Field::Null)]))
                    .unwrap();

                assert!(g.edges.contains_key(&e));
                assert!(!g.edges.contains_key(&dangling));
                assert!(!g.entity_to_path_pointees.contains_key(&e));
                test_utils::check_index_invariant(&g);
            }

            /// Path-pointee survives when the referenced field is kept.
            #[test]
            fn path_pointee_survives_when_field_kept() {
                let mut g = Graph::default();
                let leaf = obj_with(&[("a", Field::Null)]);
                let n1 = g.add_node(leaf.clone());
                let n2 = g.add_node(leaf);
                let e = g.add_edge(n1, n2).unwrap();
                g.attach_obj(e, obj_with(&[("data", Field::Number(1))]))
                    .unwrap();

                let n3 = g.add_node(obj_with(&[("a", Field::Null)]));
                let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
                let kept = g.add_edge(n3, path.clone()).unwrap();

                g.replace_attached_obj(&e, obj_with(&[("data", Field::Number(99))]))
                    .unwrap();

                assert!(g.edges.contains_key(&kept));
                assert!(g.pointee_uses.contains_key(&path));
                test_utils::check_index_invariant(&g);
            }
        }

        mod test_retarget_edge {
            use super::*;

            /// Unknown edge id is rejected.
            #[test]
            fn unknown_edge() {
                let mut g = Graph::default();
                let n1 = g.add_node(test_utils::create_simple_obj("f"));
                let err = g
                    .retarget_edge(&Uuid::new_v4(), RetargetEdge::Source(n1.into()))
                    .unwrap_err();
                assert!(matches!(err, RetargetError::EdgeNotFound(_)));
            }

            /// New endpoint must resolve in the graph.
            #[test]
            fn invalid_target_pointee() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                let err = g
                    .retarget_edge(&e, RetargetEdge::Target(Pointee::EntityId(Uuid::new_v4())))
                    .unwrap_err();
                assert!(matches!(err, RetargetError::InvalidTarget(_)));
                // Edge is unchanged.
                assert_eq!(
                    g.edges.get(&e),
                    Some(&(Pointee::EntityId(n1), Pointee::EntityId(n2)))
                );
                test_utils::check_index_invariant(&g);
            }

            /// Retarget the source: edge updated, indexes swapped.
            #[test]
            fn retargets_source() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                g.retarget_edge(&e, RetargetEdge::Source(n3.into())).unwrap();

                assert_eq!(
                    g.edges.get(&e),
                    Some(&(Pointee::EntityId(n3), Pointee::EntityId(n2)))
                );
                // Old source bucket is now empty (n1 had only this edge).
                assert!(!g.pointee_uses.contains_key(&Pointee::EntityId(n1)));
                // New source bucket has the edge.
                assert!(g
                    .pointee_uses
                    .get(&Pointee::EntityId(n3))
                    .is_some_and(|b| b.edges_as_source.contains(&e)));
                test_utils::check_index_invariant(&g);
            }

            /// Retarget the target: edge updated, indexes swapped.
            #[test]
            fn retargets_target() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                g.retarget_edge(&e, RetargetEdge::Target(n3.into())).unwrap();

                assert_eq!(
                    g.edges.get(&e),
                    Some(&(Pointee::EntityId(n1), Pointee::EntityId(n3)))
                );
                assert!(!g.pointee_uses.contains_key(&Pointee::EntityId(n2)));
                assert!(g
                    .pointee_uses
                    .get(&Pointee::EntityId(n3))
                    .is_some_and(|b| b.edges_as_target.contains(&e)));
                test_utils::check_index_invariant(&g);
            }

            /// No-op when the new endpoint equals the old one.
            #[test]
            fn no_op_same_endpoint() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                g.retarget_edge(&e, RetargetEdge::Source(n1.into())).unwrap();

                assert_eq!(
                    g.edges.get(&e),
                    Some(&(Pointee::EntityId(n1), Pointee::EntityId(n2)))
                );
                test_utils::check_index_invariant(&g);
            }

            /// Retargeting to a Path-pointee tracks it in
            /// `entity_to_path_pointees`; removing the last reference
            /// to the old path-pointee untracks it.
            #[test]
            fn path_endpoints_track_and_untrack() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("test_field");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);

                let old_path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
                let e = g.add_edge(n1, old_path.clone()).unwrap();
                assert!(g.entity_to_path_pointees.contains_key(&n2));

                let new_path = Pointee::Path(GlobalObjPath::new(n3, "test_field").unwrap());
                g.retarget_edge(&e, RetargetEdge::Target(new_path.clone()))
                    .unwrap();

                // Old path's entity untracked (was its only reference).
                assert!(!g.entity_to_path_pointees.contains_key(&n2));
                assert!(!g.pointee_uses.contains_key(&old_path));

                // New path tracked.
                assert!(g
                    .entity_to_path_pointees
                    .get(&n3)
                    .is_some_and(|s| s.contains(&new_path)));
                assert!(g.pointee_uses.contains_key(&new_path));
                test_utils::check_index_invariant(&g);
            }

            /// Self-loop: retargeting source while target equals source —
            /// the bucket isn't lost mid-op since the same pointee is still
            /// the target.
            #[test]
            fn self_loop_retarget_source_preserves_target_bucket() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj);
                let e = g.add_edge(n1, n1).unwrap();

                g.retarget_edge(&e, RetargetEdge::Source(n2.into())).unwrap();

                assert_eq!(
                    g.edges.get(&e),
                    Some(&(Pointee::EntityId(n2), Pointee::EntityId(n1)))
                );
                // n1 still tracked as target.
                assert!(g
                    .pointee_uses
                    .get(&Pointee::EntityId(n1))
                    .is_some_and(|b| b.edges_as_target.contains(&e)));
                test_utils::check_index_invariant(&g);
            }

            /// Records the patch.
            #[test]
            fn records_patch() {
                let mut g = Graph::default();
                let obj = test_utils::create_simple_obj("f");
                let n1 = g.add_node(obj.clone());
                let n2 = g.add_node(obj.clone());
                let n3 = g.add_node(obj);
                let e = g.add_edge(n1, n2).unwrap();

                g.retarget_edge(&e, RetargetEdge::Target(n3.into())).unwrap();

                assert_eq!(
                    *g.events.last().unwrap(),
                    Patch::RetargetEdge {
                        id: e,
                        new_target: RetargetEdge::Target(Pointee::EntityId(n3)),
                    }
                );
            }
        }
    }

    mod test_obj_apply_patch {
        use super::*;

        fn obj_with(fields: &[(&str, Field)]) -> Object {
            let mut o = Object::new();
            for (k, v) in fields {
                o.insert((*k).into(), v.clone());
            }
            o
        }

        #[test]
        fn unknown_id() {
            let mut g = Graph::default();
            let err = g
                .obj_apply_patch(Uuid::new_v4(), vec![])
                .unwrap_err();
            assert!(matches!(err, DeltaError::NotFound(_)));
        }

        #[test]
        fn empty_patch_is_noop() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("a", Field::Number(1))]));
            g.obj_apply_patch(n1, vec![]).unwrap();
            assert_eq!(g.obj(&n1), Some(&obj_with(&[("a", Field::Number(1))])));
            test_utils::check_index_invariant(&g);
        }

        #[test]
        fn add_field_on_fresh_key() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[]));
            g.obj_apply_patch(
                n1,
                vec![ObjectPatch::AddField {
                    name: "x".into(),
                    field: Field::Number(7),
                }],
            )
            .unwrap();
            assert_eq!(g.obj(&n1).unwrap().get("x"), Some(&Field::Number(7)));
        }

        #[test]
        fn add_field_on_existing_key_errors() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("x", Field::Number(1))]));
            let err = g
                .obj_apply_patch(
                    n1,
                    vec![ObjectPatch::AddField {
                        name: "x".into(),
                        field: Field::Number(7),
                    }],
                )
                .unwrap_err();
            assert!(matches!(
                err,
                DeltaError::Delta(ObjectPatchError::FieldAlreadyExists { .. })
            ));
        }

        #[test]
        fn remove_field_existing() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("x", Field::Number(1)), ("y", Field::Null)]));
            g.obj_apply_patch(
                n1,
                vec![ObjectPatch::RemoveField { name: "x".into() }],
            )
            .unwrap();
            assert!(!g.obj(&n1).unwrap().contains_key("x"));
            assert!(g.obj(&n1).unwrap().contains_key("y"));
        }

        #[test]
        fn remove_field_missing_errors() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[]));
            let err = g
                .obj_apply_patch(n1, vec![ObjectPatch::RemoveField { name: "x".into() }])
                .unwrap_err();
            assert!(matches!(
                err,
                DeltaError::Delta(ObjectPatchError::FieldNotFound { .. })
            ));
        }

        #[test]
        fn upsert_field_inserts_and_replaces() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("x", Field::Number(1))]));
            g.obj_apply_patch(
                n1,
                vec![
                    ObjectPatch::UpsertField {
                        name: "x".into(),
                        field: Field::Number(99),
                    },
                    ObjectPatch::UpsertField {
                        name: "y".into(),
                        field: Field::Null,
                    },
                ],
            )
            .unwrap();
            assert_eq!(g.obj(&n1).unwrap().get("x"), Some(&Field::Number(99)));
            assert_eq!(g.obj(&n1).unwrap().get("y"), Some(&Field::Null));
        }

        #[test]
        fn array_patch_adds_and_removes() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[(
                "arr",
                Field::Array(vec![
                    Field::Number(1),
                    Field::Number(2),
                    Field::Number(3),
                ]),
            )]));
            g.obj_apply_patch(
                n1,
                vec![ObjectPatch::ArrayPatch {
                    name: "arr".into(),
                    removed_indices: vec![0],
                    added_fields: vec![(2, Field::Number(99))],
                }],
            )
            .unwrap();
            // After remove(0): [2, 3]; after insert at 2: [2, 3, 99].
            assert_eq!(
                g.obj(&n1).unwrap().get("arr"),
                Some(&Field::Array(vec![
                    Field::Number(2),
                    Field::Number(3),
                    Field::Number(99),
                ]))
            );
        }

        #[test]
        fn array_patch_on_non_array_errors() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("x", Field::Number(1))]));
            let err = g
                .obj_apply_patch(
                    n1,
                    vec![ObjectPatch::ArrayPatch {
                        name: "x".into(),
                        removed_indices: vec![],
                        added_fields: vec![],
                    }],
                )
                .unwrap_err();
            assert!(matches!(
                err,
                DeltaError::Delta(ObjectPatchError::NotAnArray { .. })
            ));
        }

        #[test]
        fn array_patch_index_out_of_bounds() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("arr", Field::Array(vec![Field::Number(1)]))]));
            let err = g
                .obj_apply_patch(
                    n1,
                    vec![ObjectPatch::ArrayPatch {
                        name: "arr".into(),
                        removed_indices: vec![5],
                        added_fields: vec![],
                    }],
                )
                .unwrap_err();
            assert!(matches!(
                err,
                DeltaError::Delta(ObjectPatchError::IndexOutOfBounds { index: 5 })
            ));
        }

        #[test]
        fn sub_object_patch_navigates_and_applies() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[(
                "inner",
                Field::Object(obj_with(&[("a", Field::Number(1))])),
            )]));
            g.obj_apply_patch(
                n1,
                vec![ObjectPatch::SubObjectPatch {
                    path: LocalObjPath::new("inner").unwrap(),
                    delta: vec![ObjectPatch::UpsertField {
                        name: "a".into(),
                        field: Field::Number(99),
                    }],
                }],
            )
            .unwrap();
            match g.obj(&n1).unwrap().get("inner") {
                Some(Field::Object(inner)) => {
                    assert_eq!(inner.get("a"), Some(&Field::Number(99)));
                }
                _ => panic!("inner should be an Object"),
            }
        }

        #[test]
        fn sub_object_patch_through_non_object_errors() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("inner", Field::Number(1))]));
            let err = g
                .obj_apply_patch(
                    n1,
                    vec![ObjectPatch::SubObjectPatch {
                        path: LocalObjPath::new("inner").unwrap(),
                        delta: vec![],
                    }],
                )
                .unwrap_err();
            assert!(matches!(
                err,
                DeltaError::Delta(ObjectPatchError::NotAnObject { .. })
            ));
        }

        #[test]
        fn cascades_path_pointees_after_field_removal() {
            let mut g = Graph::default();
            let n1 = g.add_node(obj_with(&[("data", Field::Number(1))]));
            let n2 = g.add_node(obj_with(&[("a", Field::Null)]));
            let path = Pointee::Path(GlobalObjPath::new(n1, "data").unwrap());
            let e = g.add_edge(n2, path.clone()).unwrap();

            g.obj_apply_patch(n1, vec![ObjectPatch::RemoveField { name: "data".into() }])
                .unwrap();

            // The Pointee::Path no longer resolves → cascade kills the edge.
            assert!(!g.edges.contains_key(&e));
            assert!(!g.pointee_uses.contains_key(&path));
            assert!(g.is_exist(&n1));
            test_utils::check_index_invariant(&g);
        }
    }

    mod test_apply_patch {
        use super::*;

        /// Replay the recorded events on a fresh graph and assert
        /// structural equality with the original.
        fn assert_replay_matches(original: &Graph) {
            let mut replayed = Graph::default();
            replayed
                .apply_patch(original.events.clone())
                .expect("replay must succeed");
            test_utils::check_index_invariant(&replayed);
            assert_eq!(original.entities, replayed.entities, "entities mismatch");
            assert_eq!(original.edges, replayed.edges, "edges mismatch");
            assert_eq!(
                original.hyper_edge, replayed.hyper_edge,
                "hyper_edge mismatch"
            );
        }

        #[test]
        fn nodes_and_edges() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("f");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());
            let n3 = g.add_node(obj);
            g.add_edge(n1, n2).unwrap();
            g.add_edge(n3, n1).unwrap();

            assert_replay_matches(&g);
        }

        #[test]
        fn hyperedge_lifecycle() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("f");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());
            let n3 = g.add_node(obj);

            let mut m = HashSet::new();
            m.insert(n1.into());
            m.insert(n2.into());
            let h = g.create_hyperedge(m).unwrap();

            let mut to_add = HashSet::new();
            to_add.insert(n3.into());
            g.add_hyperedge_members(h, to_add).unwrap();

            let mut to_remove = HashSet::new();
            to_remove.insert(n1.into());
            g.remove_hyperedge_members(h, to_remove).unwrap();

            assert_replay_matches(&g);
        }

        #[test]
        fn attach_then_replace_attached() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("f");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());
            let e = g.add_edge(n1, n2).unwrap();
            g.attach_obj(e, obj.clone()).unwrap();
            // Replace with same key but different value → small delta path.
            let mut new = Object::new();
            new.insert("f".into(), Field::Number(42));
            g.replace_attached_obj(&e, new).unwrap();

            assert_replay_matches(&g);
        }

        #[test]
        fn replace_node_change_path() {
            let mut g = Graph::default();
            let mut o1 = Object::new();
            o1.insert("a".into(), Field::Number(1));
            o1.insert("b".into(), Field::Number(2));
            let n1 = g.add_node(o1);

            let mut o2 = Object::new();
            o2.insert("a".into(), Field::Number(1));
            o2.insert("b".into(), Field::Number(99)); // small delta
            g.replace_node(&n1, o2).unwrap();

            assert_replay_matches(&g);
        }

        #[test]
        fn replace_node_upsert_path() {
            let mut g = Graph::default();
            let mut o1 = Object::new();
            o1.insert("a".into(), Field::Number(1));
            let n1 = g.add_node(o1);

            // 2 ops (Remove a, Add x) > 1 field → Upsert path.
            let mut o2 = Object::new();
            o2.insert("x".into(), Field::Number(99));
            g.replace_node(&n1, o2).unwrap();

            assert_replay_matches(&g);
        }

        #[test]
        fn retarget_edge() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("f");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());
            let n3 = g.add_node(obj);
            let e = g.add_edge(n1, n2).unwrap();
            g.retarget_edge(&e, RetargetEdge::Target(n3.into())).unwrap();

            assert_replay_matches(&g);
        }

        #[test]
        fn remove_node_cascade_replays() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("f");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());
            let n3 = g.add_node(obj);
            g.add_edge(n1, n2).unwrap();
            g.add_edge(n3, n1).unwrap();
            g.remove_node(&n1).unwrap();

            assert_replay_matches(&g);
        }

        /// A patch whose precondition is violated propagates as an error.
        #[test]
        fn missing_precondition_errors() {
            let mut g = Graph::default();
            let err = g
                .apply_patch(vec![Patch::RemoveNode { id: Uuid::new_v4() }])
                .unwrap_err();
            assert!(matches!(err, ApplyPatchError::NodeNotFound(_)));
        }
    }

    mod lift_events {
        use super::*;

        #[test]
        fn test_create_hyperedge() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("field");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj);

            let mut m = HashSet::new();
            m.insert(n1.into());
            m.insert(n2.into());

            let h = g.create_hyperedge(m.clone()).unwrap();

            assert_eq!(
                *g.events.last().unwrap(),
                Patch::CreateHyperEdge { id: h, members: m }
            )
        }

        #[test]
        fn test_add_node() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("field");
            let n1 = g.add_node(obj.clone());

            assert_eq!(*g.events.last().unwrap(), Patch::AddNode { id: n1, obj })
        }

        #[test]
        fn test_add_edge() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("field");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj);

            let e = g.add_edge(n1, n2).unwrap();

            assert_eq!(
                *g.events.last().unwrap(),
                Patch::AddEdge {
                    id: e,
                    source: n1.into(),
                    target: n2.into()
                }
            )
        }

        #[test]
        fn test_remove_node() {
            let mut g = Graph::default();
            let obj = test_utils::create_simple_obj("field");
            let n1 = g.add_node(obj.clone());
            let n2 = g.add_node(obj.clone());

            let e = g.add_edge(n1, n2).unwrap();

            g.remove_node(&n1).unwrap();

            // To ensure that remove doesn't produce remove event's
            assert_eq!(
                g.events[0],
                Patch::AddNode {
                    id: n1,
                    obj: obj.clone()
                }
            );
            assert_eq!(g.events[1], Patch::AddNode { id: n2, obj: obj });
            assert_eq!(
                g.events[2],
                Patch::AddEdge {
                    id: e,
                    source: n1.into(),
                    target: n2.into()
                }
            );
            assert_eq!(g.events[3], Patch::RemoveNode { id: n1 })
        }
    }

    #[test]
    fn test_remove_edge() {
        let mut g = Graph::default();
        let obj = test_utils::create_simple_obj("field");
        let n1 = g.add_node(obj.clone());
        let n2 = g.add_node(obj.clone());

        let e = g.add_edge(n1, n2).unwrap();

        g.remove_edge(&e).unwrap();

        assert_eq!(*g.events.last().unwrap(), Patch::RemoveEdge { id: e });
    }
}
