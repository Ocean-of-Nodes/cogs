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

use serde::{Deserialize, Serialize};

/// Guest export `fn(len: u32) -> u32` that allocates `len` bytes
/// and returns the pointer.
pub const ALLOC_EXPORT: &str = "__cog_alloc";

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

/// The event log no longer matches the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UndoError {
    Apply,
    UnresolvedCascade,
}
