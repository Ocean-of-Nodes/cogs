//! Patch emission and (future) listener machinery.

use common::*;

use crate::graph::ListenerId;
use crate::graph::Graph;

impl Graph {
    /// Append a patch to the graph's event log.
    pub(crate) fn emit_patch(&mut self, patch: Patch) {
        self.events.push(patch);
    }

    /// Subscribe a listener that fires on every emitted patch.
    /// Stub — not yet implemented.
    pub fn subscribe_on_change(&mut self, _listener: Box<dyn FnMut(Patch)>) -> ListenerId {
        todo!()
    }

    /// Drop a previously-subscribed listener.
    /// Stub — not yet implemented.
    pub fn unsubscribe_on_change(&mut self, _id: ListenerId) {
        todo!()
    }
}
