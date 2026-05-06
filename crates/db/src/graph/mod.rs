//! In-memory graph storage and the operations that mutate it.
//!
//! [`Graph`] is the central type — it holds nodes/edges/hyperedges,
//! the reverse index for fast cascade lookups, and the patch log of
//! every mutation.
//!
//! ## File layout
//!
//! - [`mod.rs`](self) — `struct Graph` and its fields.
//! - `index.rs` — [`PointeeUses`] and `track_pointee_entity` helpers.
//! - `queries.rs` — non-mutating reads: iterators, getters,
//!   predicates ([`Graph::is_pointee_exist`], [`Graph::get_type`]).
//! - `cascade.rs` — recursive-removal helpers (`drain_pointee_bucket`,
//!   `cascade_remove_id`, etc.).
//! - `constructors.rs` — [`Graph::add_node`], [`Graph::add_edge`],
//!   [`Graph::create_hyperedge`] + their `silent_*` cousins.
//! - `destructors.rs` — `remove_*` family.
//! - `modifiers.rs` — `attach_obj`, `replace_*`, `retarget_edge`,
//!   `*_hyperedge_members`.
//! - `apply_patch.rs` — patch-log replay.
//! - `listeners.rs` — `emit_patch` + listener stubs.
//!
//! ## Invariants
//!
//! - Every edge endpoint resolves at insertion time
//!   ([`Graph::is_pointee_exist`]).
//! - Every hyperedge has at least one member; cascade-removing the
//!   last member deletes the hyperedge.
//! - The reverse indexes (`pointee_uses`, `entity_to_path_pointees`)
//!   are derivable from the structural maps and are kept consistent
//!   on every mutation; see `tests::test_utils::check_index_invariant`.
//! - Removing an entity transitively removes everything that
//!   referenced it (directly via `Pointee::EntityId` or through a
//!   `Pointee::Path`); see `cascade_remove_id`.
//!
//! ## Replay
//!
//! Every public mutation appends a [`Patch`] to `events`. Replaying
//! the recorded log via [`Graph::apply_patch`] on a fresh graph
//! reconstructs an identical state (verified by round-trip tests in
//! `mod test_apply_patch`).

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use common::*;

mod apply_patch;
mod cascade;
mod constructors;
mod destructors;
mod index;
mod listeners;
mod modifiers;
mod queries;

pub(crate) use index::PointeeUses;

/// Identifier returned by [`Graph::subscribe_on_change`] and used to
/// remove the listener via [`Graph::unsubscribe_on_change`].
pub(crate) type ListenerId = Uuid;

#[derive(Default)]
pub(crate) struct Graph {
    /// Object attached to each id — nodes always, edges/hyperedges
    /// only when [`Graph::attach_obj`] has been called on them.
    pub(crate) entities: HashMap<EntityId, Object>,

    /// `EdgeID → (source, target)` pair.
    pub(crate) edges: HashMap<EdgeID, (Pointee, Pointee)>,

    /// `HyperEdgeId → set of members`.
    pub(crate) hyper_edge: HashMap<HyperEdgeId, HashSet<Pointee>>,

    /// Reverse index: for each [`Pointee`] referenced as an
    /// edge-endpoint or hyperedge-member, who references it.
    pub(crate) pointee_uses: HashMap<Pointee, PointeeUses>,

    /// Secondary index: for each entity, every `Pointee::Path`
    /// rooted at it that's currently live in `pointee_uses`. Lets
    /// cascade removal find paths through an entity in
    /// O(in-degree-by-paths) instead of scanning the whole index.
    pub(crate) entity_to_path_pointees: HashMap<EntityId, HashSet<Pointee>>,

    /// Patch log — appended on every public mutation.
    pub(crate) events: Delta,
}
