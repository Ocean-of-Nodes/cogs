//! Persistent graph storage for the engine.
//!
//! ## Module layout
//!
//! - [`errors`] — every error type returned by `Graph` operations.
//! - [`types`] — domain taxonomy: `EntityType`, `PointeeKind`,
//!   `EdgeView`.
//! - [`object_patch`] — pure helpers over `Object` (apply / diff).
//! - [`graph`] — the `Graph` struct and all its impls, split by
//!   responsibility (queries, cascade, constructors, destructors,
//!   modifiers, replay, listeners, reverse-index).
//! - [`views`] — [`Database`]: views over a graph, kept up to date
//!   incrementally.

mod errors;
mod graph;
mod object_patch;
mod types;
mod views;

pub use errors::*;
pub use graph::{Graph, Modification};
pub use types::{EdgeView, EntityType, PointeeKind};
pub use views::{
    Database, DepKey, InvocationId, PropagateError, RegisterViewError, Report, ViewGraph, ViewId,
};

#[cfg(test)]
mod tests;
