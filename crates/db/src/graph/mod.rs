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
//!   `cascade_remove_entity`, etc.).
//! - `constructors.rs` — [`Graph::add_node`], [`Graph::add_edge`],
//!   [`Graph::create_hyperedge`] + their `silent_*` cousins.
//! - `destructors.rs` — `remove_*` family.
//! - `modifiers.rs` — `attach_obj`, `replace_*`, `retarget_edge`,
//!   `*_hyperedge_members`.
//! - `apply_delta.rs` — replay of a [`Delta`] of `ThinPatch`es.
//! - `listeners.rs` — `record` + listener stubs.
//! - `undo.rs` — [`Graph::undo`]: revert the last logged mutation.
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
//!   `Pointee::Path`); see `cascade_remove_entity`.
//!
//! ## Replay
//!
//! Every public mutation appends a [`Modification`] to `events`.
//! Converting the recorded `operation`s to `ThinPatch` and replaying
//! them via [`Graph::apply_delta`] on a fresh graph reconstructs an
//! identical state — the cascade is recomputed, so `cascade` is not
//! replayed (verified by round-trip tests in `mod test_apply_delta`).

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use common::*;

mod apply_delta;
mod cascade;
mod constructors;
mod destructors;
mod index;
mod listeners;
mod modifiers;
mod queries;
mod undo;

pub(crate) use index::PointeeUses;
pub(crate) use queries::AttachKind;

/// Identifier returned by [`Graph::subscribe_on_change`] and used to
/// remove the listener via [`Graph::unsubscribe_on_change`].
pub(crate) type ListenerId = Uuid;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Modification {
    /// Everything the cascade of this mutation removed — whole
    /// edges/hyperedges and members stripped from surviving
    /// hyperedges — in the order it happened (after `operation`).
    pub cascade: FullDelta,

    /// Main patch for this mutation.
    pub operation: FullPatch,
}

#[derive(Default)]
pub struct Graph {
    /// Object attached to each id — nodes always, edges/hyperedges
    /// only when [`Graph::attach_obj`] has been called on them.
    pub(crate) entities: HashMap<EntityId, Object>,

    /// `EdgeId → (source, target)` pair.
    pub(crate) edges: HashMap<EdgeId, (Pointee, Pointee)>,

    /// `HyperedgeId → set of members`.
    pub(crate) hyperedges: HashMap<HyperedgeId, HashSet<Pointee>>,

    /// Reverse index: for each [`Pointee`] referenced as an
    /// edge-endpoint or hyperedge-member, who references it.
    pub(crate) pointee_uses: HashMap<Pointee, PointeeUses>,

    /// Secondary index: for each entity, every `Pointee::Path`
    /// rooted at it that's currently live in `pointee_uses`. Lets
    /// cascade removal find paths through an entity in
    /// O(in-degree-by-paths) instead of scanning the whole index.
    pub(crate) entity_to_path_pointees: HashMap<EntityId, HashSet<Pointee>>,

    /// Patch log — appended on every public mutation.
    pub(crate) events: Vec<Modification>,
}
