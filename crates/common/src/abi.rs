//! Wire contract between WASM guests (`sdk`) and the host (`jit`).
//!
//! Every graph intrinsic has the same shape:
//!
//! ```text
//! fn __cog_<method>(args_ptr: u32, args_len: u32) -> u64
//! ```
//!
//! - Arguments are one bincode value (a tuple for several arguments)
//!   in guest memory at `(args_ptr, args_len)`.
//! - The result is one bincode value written into a buffer the host
//!   obtains from the guest export [`ALLOC_EXPORT`]; the intrinsic
//!   returns that buffer as [`pack_buf`]-ed `u64`. Ownership of the
//!   buffer passes to the guest.
//! - Fallible methods return `Result<T, E>` where `E` is one of the
//!   error mirrors below. They only name the failure kind; the
//!   detailed `db` error stays on the host.
//!
//! Views are exported by the guest:
//!
//! ```text
//! fn __cog_view_<name>(args_ptr: u32, args_len: u32)
//! fn __cog_meta_<name>() -> u64
//! ```
//!
//! - The view entry takes the invocation key as a bincode
//!   `Option<EntityId>` in a buffer the host obtained from
//!   [`ALLOC_EXPORT`]; ownership of the buffer passes to the guest.
//! - The meta export returns a [`pack_buf`]-ed bincode [`ViewMeta`]
//!   in a guest buffer; the host releases it with [`FREE_EXPORT`].

use serde::{Deserialize, Serialize};

use crate::Pattern;

/// Guest export `fn(len: u32) -> u32` that allocates `len` bytes
/// and returns the pointer.
pub const ALLOC_EXPORT: &str = "__cog_alloc";

/// Guest export `fn(ptr: u32, len: u32)` that releases a buffer the
/// guest handed to the host.
pub const FREE_EXPORT: &str = "__cog_free";

/// Prefix of view entry exports; the rest of the name is the view
/// name.
pub const VIEW_EXPORT_PREFIX: &str = "__cog_view_";

/// Prefix of the export describing the view with the same name.
pub const META_EXPORT_PREFIX: &str = "__cog_meta_";

/// What a view declares about itself in `#[view(...)]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewMeta {
    /// Run once per matching node instead of once for the whole graph.
    pub for_each: Option<Pattern>,
}

/// Pack a guest buffer `(ptr, len)` into one `u64` return value.
pub fn pack_buf(ptr: u32, len: u32) -> u64 {
    ((ptr as u64) << 32) | len as u64
}

/// Inverse of [`pack_buf`].
pub fn unpack_buf(packed: u64) -> (u32, u32) {
    ((packed >> 32) as u32, packed as u32)
}

// ----- Error mirrors of `db` errors ---------------------------------

/// The id is not currently a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeNotFoundError;

/// The id is not currently an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeNotFoundError;

/// The id is not currently a hyperedge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HyperedgeNotFoundError;

/// The id has no attached object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoAttachedObjectError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddEdgeError {
    MissingEndpoints,
    EdgeAlreadyExists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateHyperedgeError {
    PointeesNotFound,
    HyperedgeAlreadyExists,
    EmptyHyperedge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetargetError {
    EdgeNotFound,
    InvalidTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttachObjectError {
    AttachTargetNotFound,
    IncorrectType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetEdgeError {
    NotFound,
    IncorrectType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddHyperedgeMembersError {
    HyperedgeNotFound,
    MembersAlreadyExist,
    PointeesNotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoveHyperedgeMembersError {
    HyperedgeNotFound,
    MembersNotInHyperedge,
}
