//! WASM-side library used by user code (views, mutators, procedures).
//!
//! This crate is meant to be compiled to `wasm32-unknown-unknown`.
//! It declares host intrinsics as `extern "C"` imports and wraps them
//! in safe APIs (`Graph::add_node`, ...). The host (`jit` crate)
//! supplies real implementations.
//!
//! Object/Field/Pointee come from `common` so that wire/host/guest
//! all share one data model.

use std::collections::HashSet;

pub use common::abi::{
    AddEdgeError, AddHyperedgeMembersError, AttachObjectError, CreateHyperedgeError,
    EdgeNotFoundError, GetEdgeError, HyperedgeNotFoundError, NoAttachedObjectError,
    NodeNotFoundError, RemoveHyperedgeMembersError, RetargetError,
};
pub use common::{
    AttachTargetId, EdgeId, EdgeView, EntityId, Field, HyperedgeId, NodeId, Object, Pattern,
    Pointee, PointeeKind, RetargetEdge,
};
pub use sdk_macros::view;

#[doc(hidden)]
pub use common::abi::ViewMeta;

use common::abi;
use serde::{Serialize, de::DeserializeOwned};

// On `wasm32-unknown-unknown` there is no entropy source, but std's
// `HashMap` calls `getrandom` on first use to seed its hasher. Register
// a deterministic stub so `Object::new()` (which is `HashMap::new()`)
// doesn't trap. Hash-flooding is irrelevant here — every WASM module
// runs in a sandboxed, single-threaded host invocation.
#[cfg(target_arch = "wasm32")]
fn always_zero(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    buf.fill(0);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
getrandom::register_custom_getrandom!(always_zero);

/// Every intrinsic follows the convention in [`common::abi`]:
/// bincode arguments in, packed bincode result buffer out.
type Intrinsic = unsafe extern "C" fn(args_ptr: u32, args_len: u32) -> u64;

unsafe extern "C" {
    fn __cog_add_node(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_add_edge(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_create_hyperedge(args_ptr: u32, args_len: u32) -> u64;

    fn __cog_remove_node(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_remove_edge(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_remove_hyperedge(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_remove_attached(args_ptr: u32, args_len: u32) -> u64;

    fn __cog_attach_obj(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_add_hyperedge_members(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_remove_hyperedge_members(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_replace_node(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_replace_attached_obj(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_retarget_edge(args_ptr: u32, args_len: u32) -> u64;

    fn __cog_iter_entities(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_iter_nodes(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_iter_edges(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_iter_hyperedges(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_iter_attached(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_obj(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_edge(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_hyperedge_members(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_classify_pointee(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_is_exist(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_is_pointee_exist(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_field(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_out_edges(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_in_edges(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_hyperedges_of(args_ptr: u32, args_len: u32) -> u64;
    fn __cog_nodes_where(args_ptr: u32, args_len: u32) -> u64;
}

/// Allocator the host calls to hand buffers to the guest: intrinsic
/// results (reclaimed in [`call`]) and view keys (reclaimed in
/// [`__args`]).
#[unsafe(no_mangle)]
pub extern "C" fn __cog_alloc(len: u32) -> u32 {
    let buf = vec![0u8; len as usize].into_boxed_slice();
    Box::into_raw(buf) as *mut u8 as u32
}

/// Release a buffer the guest handed to the host with [`__ret`].
#[unsafe(no_mangle)]
pub extern "C" fn __cog_free(ptr: u32, len: u32) {
    drop(unsafe { take_buf(ptr, len) });
}

/// Take back ownership of a buffer made by `Box<[u8]>::into_raw`.
unsafe fn take_buf(ptr: u32, len: u32) -> Box<[u8]> {
    let slice = core::ptr::slice_from_raw_parts_mut(ptr as *mut u8, len as usize);
    unsafe { Box::from_raw(slice) }
}

/// Encode `args`, invoke `intrinsic`, take ownership of the result
/// buffer the host produced via [`__cog_alloc`] and decode it.
fn call<A: Serialize, R: DeserializeOwned>(intrinsic: Intrinsic, args: &A) -> R {
    let bytes = bincode::serialize(args).expect("encode intrinsic args");
    let packed = unsafe { intrinsic(bytes.as_ptr() as u32, bytes.len() as u32) };
    let (ptr, len) = abi::unpack_buf(packed);
    let buf = unsafe { take_buf(ptr, len) };
    bincode::deserialize(&buf).expect("decode intrinsic result")
}

/// Decode the arguments the host passed to a `#[view]` entry point.
/// Used by macro-generated code.
#[doc(hidden)]
pub fn __args<T: DeserializeOwned>(ptr: u32, len: u32) -> T {
    let buf = unsafe { take_buf(ptr, len) };
    bincode::deserialize(&buf).expect("decode view arguments")
}

/// Hand `value` to the host as a packed buffer it releases with
/// [`__cog_free`]. Used by macro-generated code.
#[doc(hidden)]
pub fn __ret<T: Serialize>(value: &T) -> u64 {
    let bytes = bincode::serialize(value)
        .expect("encode value for the host")
        .into_boxed_slice();
    let len = bytes.len() as u32;
    abi::pack_buf(Box::into_raw(bytes) as *mut u8 as u32, len)
}

/// The graph as the running view invocation sees it.
///
/// - Every read is recorded: when what it returned may have changed,
///   the invocation runs again.
/// - The view owns what it creates. It can change or remove only
///   entities it created in the current run; for anything else the
///   methods fail as for a missing entity.
/// - What the invocation wrote in its previous run is hidden until it
///   writes it again, so a re-run sees what a first run would see.
/// - Lists of ids are sorted.
///
/// Methods mirror `db::Graph`; ids are taken by value and borrowed
/// results are returned as owned copies.
pub struct Graph {
    _private: (),
}

impl Graph {
    /// Constructed by macro-generated entry points; not meant for
    /// direct user use.
    #[doc(hidden)]
    pub fn __new() -> Self {
        Self { _private: () }
    }

    // ------------ CONSTRUCTORS ------------ //

    /// Insert a node carrying `obj` and return its host-minted id.
    pub fn add_node(&mut self, obj: Object) -> NodeId {
        call(__cog_add_node, &obj)
    }

    /// Add an edge from `source` to `target`. Both endpoints must
    /// already exist.
    pub fn add_edge(
        &mut self,
        source: impl Into<Pointee>,
        target: impl Into<Pointee>,
    ) -> Result<EdgeId, AddEdgeError> {
        call(__cog_add_edge, &(source.into(), target.into()))
    }

    /// Create a hyperedge over `members`. Every member must exist and
    /// the set must be non-empty.
    pub fn create_hyperedge(
        &mut self,
        members: HashSet<Pointee>,
    ) -> Result<HyperedgeId, CreateHyperedgeError> {
        call(__cog_create_hyperedge, &members)
    }

    // ------------ DESTRUCTORS ------------ //

    /// Remove a node (cascading to everything that references it) and
    /// return its object.
    pub fn remove_node(&mut self, id: NodeId) -> Result<Object, NodeNotFoundError> {
        call(__cog_remove_node, &id)
    }

    /// Remove an edge (cascading to everything that references it).
    pub fn remove_edge(&mut self, id: EdgeId) -> Result<EdgeView, EdgeNotFoundError> {
        call(__cog_remove_edge, &id)
    }

    /// Remove a hyperedge (cascading) and return its members.
    pub fn remove_hyperedge(
        &mut self,
        id: HyperedgeId,
    ) -> Result<HashSet<Pointee>, HyperedgeNotFoundError> {
        call(__cog_remove_hyperedge, &id)
    }

    /// Drop the object attached to an edge or hyperedge.
    pub fn remove_attached(&mut self, id: AttachTargetId) -> Result<(), NoAttachedObjectError> {
        call(__cog_remove_attached, &id)
    }

    // ------------ MODIFIERS ------------ //

    /// Attach `obj` to an edge or hyperedge that has none yet.
    pub fn attach_obj(&mut self, id: AttachTargetId, obj: Object) -> Result<(), AttachObjectError> {
        call(__cog_attach_obj, &(id, obj))
    }

    pub fn add_hyperedge_members(
        &mut self,
        id: HyperedgeId,
        members: HashSet<Pointee>,
    ) -> Result<(), AddHyperedgeMembersError> {
        call(__cog_add_hyperedge_members, &(id, members))
    }

    pub fn remove_hyperedge_members(
        &mut self,
        id: HyperedgeId,
        members: HashSet<Pointee>,
    ) -> Result<(), RemoveHyperedgeMembersError> {
        call(__cog_remove_hyperedge_members, &(id, members))
    }

    /// Replace a node's object; returns the old one as `Field::Object`.
    pub fn replace_node(&mut self, id: NodeId, obj: Object) -> Result<Field, NodeNotFoundError> {
        call(__cog_replace_node, &(id, obj))
    }

    /// Replace an attached object; returns the old one as
    /// `Field::Object`.
    pub fn replace_attached_obj(
        &mut self,
        id: AttachTargetId,
        obj: Object,
    ) -> Result<Field, NoAttachedObjectError> {
        call(__cog_replace_attached_obj, &(id, obj))
    }

    /// Move one endpoint of an edge.
    pub fn retarget_edge(
        &mut self,
        id: EdgeId,
        new_target: RetargetEdge,
    ) -> Result<(), RetargetError> {
        call(__cog_retarget_edge, &(id, new_target))
    }

    // ------------ QUERIES ------------ //

    pub fn iter_entities(&self) -> Vec<EntityId> {
        call(__cog_iter_entities, &())
    }

    pub fn iter_nodes(&self) -> Vec<NodeId> {
        call(__cog_iter_nodes, &())
    }

    pub fn iter_edges(&self) -> Vec<EdgeId> {
        call(__cog_iter_edges, &())
    }

    pub fn iter_hyperedges(&self) -> Vec<HyperedgeId> {
        call(__cog_iter_hyperedges, &())
    }

    /// Edges and hyperedges that carry an attached object.
    pub fn iter_attached(&self) -> Vec<AttachTargetId> {
        call(__cog_iter_attached, &())
    }

    /// Copy of the object stored at `id`, or `None` if there is none
    /// (missing entity, or edge/hyperedge without attached object).
    pub fn obj(&self, id: EntityId) -> Option<Object> {
        call(__cog_obj, &id)
    }

    pub fn edge(&self, id: EdgeId) -> Result<EdgeView, GetEdgeError> {
        call(__cog_edge, &id)
    }

    pub fn hyperedge_members(&self, id: HyperedgeId) -> Option<HashSet<Pointee>> {
        call(__cog_hyperedge_members, &id)
    }

    pub fn classify_pointee(&self, p: &Pointee) -> Option<PointeeKind> {
        call(__cog_classify_pointee, p)
    }

    pub fn is_exist(&self, id: EntityId) -> bool {
        call(__cog_is_exist, &id)
    }

    pub fn is_pointee_exist(&self, p: &Pointee) -> bool {
        call(__cog_is_pointee_exist, p)
    }

    /// One top-level field of the object at `id`. Depends on that
    /// field only, so prefer it to [`Graph::obj`].
    pub fn field(&self, id: EntityId, name: &str) -> Option<Field> {
        call(__cog_field, &(id, name))
    }

    /// Edges whose source is `p`.
    pub fn out_edges(&self, p: impl Into<Pointee>) -> Vec<EdgeId> {
        call(__cog_out_edges, &p.into())
    }

    /// Edges whose target is `p`.
    pub fn in_edges(&self, p: impl Into<Pointee>) -> Vec<EdgeId> {
        call(__cog_in_edges, &p.into())
    }

    /// Hyperedges that have `p` as a member.
    pub fn hyperedges_of(&self, p: impl Into<Pointee>) -> Vec<HyperedgeId> {
        call(__cog_hyperedges_of, &p.into())
    }

    /// Nodes whose object has top-level `field` equal to `value`.
    /// Depends on that pair, so a node that starts matching later
    /// runs the view again even if nothing matched now.
    pub fn nodes_where(&self, field: &str, value: impl Into<Field>) -> Vec<NodeId> {
        call(__cog_nodes_where, &(field, value.into()))
    }
}
