//! Input/derived layer separation: an input write cannot see derived
//! entities, so inputs never depend on what views derive from them.
//! The public mutation methods report a derived target or reference
//! with the same error as a missing one.

use std::collections::HashSet;

use common::*;

use crate::graph::{Graph, Writer};
use crate::views::is_derived;

impl Graph {
    /// Whether the current writer may change or reference `id`.
    pub(crate) fn writable(&self, id: &EntityId) -> bool {
        self.writer == Writer::Derived || !is_derived(id)
    }

    /// [`Graph::writable`] for the entity `p` is or lives inside of.
    pub(crate) fn writable_pointee(&self, p: &Pointee) -> bool {
        self.writable(&p.entity())
    }

    /// Pointees among `ps` the current writer cannot see, together
    /// with the ones that don't exist at all — what a failed
    /// existence check should report. Empty when every pointee is
    /// writable, even if some are missing: then the regular check
    /// reports them.
    pub(crate) fn unwritable_pointees<'a, I>(&self, ps: I) -> HashSet<Pointee>
    where
        I: IntoIterator<Item = &'a Pointee> + Clone,
    {
        if ps.clone().into_iter().all(|p| self.writable_pointee(p)) {
            return HashSet::new();
        }
        ps.into_iter()
            .filter(|p| !self.writable_pointee(p) || !self.is_pointee_exist(p))
            .cloned()
            .collect()
    }
}
