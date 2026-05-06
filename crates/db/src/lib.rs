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

// Re-exports for the test module — production code reaches into the
// submodules directly (`use crate::errors::*` etc.).
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use errors::*;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use graph::Graph;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use types::{EntityType, PointeeKind, EdgeView};

#[cfg(test)]
mod tests;
