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

mod errors;
mod graph;
mod object_patch;
mod types;

pub use errors::*;
pub use graph::{Graph, Modification};
pub use types::{EdgeView, EntityType, PointeeKind};

#[cfg(test)]
mod tests;
