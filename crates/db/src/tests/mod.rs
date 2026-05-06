//! Unit tests for the `db` crate.
//!
//! Layout: each major op-family lives in its own subdirectory of
//! leaf modules. Shared helpers (sample-graph factories, the
//! cross-index invariant checker) live in [`test_utils`].

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use common::*;

use super::*;

pub(crate) mod test_utils;

mod test_constructors;
mod test_destructors;
mod test_getters;
mod test_globals;
mod test_modifiers;

mod lift_events;
mod test_apply_delta;
mod test_apply_object_delta;
