//! Patch emission and (future) listener machinery.

use common::*;

use crate::graph::ListenerId;
use crate::graph::{Graph, Modification, Writer};

impl Graph {
    /// Note a mutation, together with what its cascade removed, in
    /// the journal and — for input mutations — in the event log.
    pub(crate) fn record(&mut self, operation: FullPatch, cascade: FullDelta) {
        self.journal_extend(std::iter::once(&operation).chain(&cascade));
        if self.writer == Writer::Input {
            self.events.push(Modification { cascade, operation });
        }
    }

    /// Append `patches` to the journal, if there is one.
    pub(crate) fn journal_extend<'a>(&mut self, patches: impl IntoIterator<Item = &'a FullPatch>) {
        if let Some(journal) = &mut self.journal {
            journal.extend(patches.into_iter().cloned());
        }
    }

    /// Subscribe a listener that fires on every emitted patch.
    /// Stub — not yet implemented.
    pub fn subscribe_on_change(&mut self, _listener: Box<dyn FnMut(FullPatch)>) -> ListenerId {
        todo!()
    }

    /// Drop a previously-subscribed listener.
    /// Stub — not yet implemented.
    pub fn unsubscribe_on_change(&mut self, _id: ListenerId) {
        todo!()
    }
}
