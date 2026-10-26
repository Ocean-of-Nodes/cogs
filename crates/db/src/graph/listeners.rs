//! Patch emission and (future) listener machinery.

use common::*;

use crate::graph::ListenerId;
use crate::graph::{Graph, Modification};

impl Graph {
    /// Append a mutation to the graph's event log, together with
    /// what its cascade removed.
    pub(crate) fn record(&mut self, operation: FullPatch, cascade: FullDelta) {
        self.events.push(Modification { cascade, operation });
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
