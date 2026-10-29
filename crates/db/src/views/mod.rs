//! Views: derived facts kept up to date as the graph changes. The
//! model is described in the book ("How COG does it"); in short:
//!
//! - a view runs as one *invocation* per key of its `for_each`
//!   pattern, or as a single invocation without one;
//! - an invocation sees the graph through a [`ViewGraph`] that
//!   records its reads as [`DepKey`]s and owns its writes;
//! - every change of the graph is turned into the keys it touches,
//!   and invocations that read one of them become dirty;
//! - a re-run is reconciled with the previous run, so an unchanged
//!   result changes nothing and stops the propagation.
//!
//! [`Database`] keeps the graph and that bookkeeping but does not
//! run views itself: whoever has their code drives it.
//!
//! ```text
//! while let Some(invocation) = db.begin_next()? {
//!     run the view against db.view_graph()
//!     db.finish()?;
//! }
//! ```
//!
//! [`Database::propagate_with`] does that for views written as Rust
//! closures; the `jit` crate does it for WASM views.

mod deps;
pub(crate) mod ids;
mod view_graph;

use std::collections::btree_map;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt;

use common::*;

use crate::graph::{Graph, Writer};

pub use deps::DepKey;
pub(crate) use ids::is_derived;
pub use view_graph::ViewGraph;

use view_graph::Run;

/// Index of a registered view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId(u32);

/// One view applied to one key; `key` is `None` for a view without
/// `for_each`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InvocationId {
    pub view: ViewId,
    pub key: Option<EntityId>,
}

/// What happened during a propagation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Every invocation that ran, in order.
    pub runs: Vec<InvocationId>,
    /// Runs that failed, with the reason.
    pub failures: Vec<(InvocationId, String)>,
}

impl Report {
    pub fn record(&mut self, invocation: InvocationId, outcome: Result<(), String>) {
        self.runs.push(invocation);
        if let Err(reason) = outcome {
            self.failures.push((invocation, reason));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterViewError {
    /// A view with this name is already registered.
    Duplicate { name: String },
}

impl fmt::Display for RegisterViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate { name } => write!(f, "view `{name}` is already registered"),
        }
    }
}

impl std::error::Error for RegisterViewError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropagateError {
    /// Changes came back to an invocation (or to the key set of a
    /// `for_each` view) that caused them. `runs` are the invocations
    /// on the way, sorted.
    Cycle { runs: Vec<InvocationId> },
}

impl fmt::Display for PropagateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cycle { runs } => write!(f, "dependency cycle between invocations {runs:?}"),
        }
    }
}

impl std::error::Error for PropagateError {}

/// Something that made an invocation dirty, directly or not. The
/// causes travel with the dirty marks; meeting yourself among them
/// is a cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Cause {
    /// The changes of a run.
    Run(InvocationId),
    /// A refresh of a view's key set.
    Keys(ViewId),
}

struct View {
    name: String,
    for_each: Option<Pattern>,
    /// Keys that currently have an invocation.
    keys: BTreeSet<EntityId>,
}

#[derive(Default)]
struct Invocation {
    /// Read set of the last run.
    deps: HashSet<DepKey>,
    /// Entities the invocation owns.
    outputs: HashSet<EntityId>,
}

struct Running {
    run: Run,
    causes: HashSet<Cause>,
}

/// A graph with views over it.
pub struct Database {
    graph: Graph,
    views: Vec<View>,
    invocations: HashMap<InvocationId, Invocation>,
    /// Reverse index of read sets.
    readers: HashMap<DepKey, HashSet<InvocationId>>,
    /// `for_each` views by the key their pattern depends on.
    patterns: HashMap<DepKey, Vec<ViewId>>,
    /// Owner of every live derived entity.
    owners: HashMap<EntityId, InvocationId>,
    /// Dirty invocations in the order they became dirty...
    queue: VecDeque<InvocationId>,
    /// ...and what made them dirty.
    dirty: HashMap<InvocationId, HashSet<Cause>>,
    /// `for_each` views whose key set may be out of date.
    dirty_keys: BTreeMap<ViewId, HashSet<Cause>>,
    running: Option<Running>,
}

impl Database {
    /// Attach views to `graph`. Derived entities already in it are
    /// dropped: without their invocations they are an orphaned
    /// cache, and the views will derive them again.
    pub fn new(mut graph: Graph) -> Self {
        graph.writer = Writer::Derived;
        let mut derived: Vec<EntityId> = graph.iter_entities().filter(is_derived).collect();
        derived.sort();
        for id in derived {
            remove_entity(&mut graph, &id);
        }
        graph.writer = Writer::Input;
        graph.journal = Some(FullDelta::new());

        Self {
            graph,
            views: Vec::new(),
            invocations: HashMap::new(),
            readers: HashMap::new(),
            patterns: HashMap::new(),
            owners: HashMap::new(),
            queue: VecDeque::new(),
            dirty: HashMap::new(),
            dirty_keys: BTreeMap::new(),
            running: None,
        }
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// The graph for input changes. Views that depend on them become
    /// dirty; derived entities cannot be changed this way.
    pub fn graph_mut(&mut self) -> &mut Graph {
        assert!(
            self.running.is_none(),
            "inputs cannot change while a view runs"
        );
        &mut self.graph
    }

    /// Detach the views; derived entities stay in the graph.
    pub fn into_graph(mut self) -> Graph {
        self.graph.journal = None;
        self.graph.writer = Writer::Input;
        self.graph
    }

    /// Register a view. Without `for_each` it gets one invocation;
    /// with it, one per matching node. New invocations are dirty.
    pub fn register_view(
        &mut self,
        name: impl Into<String>,
        for_each: Option<Pattern>,
    ) -> Result<ViewId, RegisterViewError> {
        let name = name.into();
        if self.views.iter().any(|view| view.name == name) {
            return Err(RegisterViewError::Duplicate { name });
        }
        let view = ViewId(self.views.len() as u32);
        match &for_each {
            None => {
                let invocation = InvocationId { view, key: None };
                self.invocations.insert(invocation, Invocation::default());
                self.mark_dirty(invocation, &HashSet::new())
                    .expect("nothing caused it, so no cycle");
            }
            Some(pattern) => {
                self.patterns
                    .entry(pattern_key(pattern))
                    .or_default()
                    .push(view);
                self.dirty_keys.entry(view).or_default();
            }
        }
        self.views.push(View {
            name,
            for_each,
            keys: BTreeSet::new(),
        });
        Ok(view)
    }

    pub fn view_name(&self, view: ViewId) -> &str {
        &self.views[view.0 as usize].name
    }

    pub fn view_id(&self, name: &str) -> Option<ViewId> {
        self.views
            .iter()
            .position(|view| view.name == name)
            .map(|i| ViewId(i as u32))
    }

    /// `name` or `name(key)`, for messages.
    pub fn describe(&self, invocation: InvocationId) -> String {
        let name = self.view_name(invocation.view);
        match invocation.key {
            None => name.to_string(),
            Some(key) => format!("{name}({key})"),
        }
    }

    /// Invocations that currently exist, sorted.
    pub fn invocations(&self) -> Vec<InvocationId> {
        let mut invocations: Vec<InvocationId> = self.invocations.keys().copied().collect();
        invocations.sort();
        invocations
    }

    /// The invocation that owns a derived entity.
    pub fn owner(&self, id: &EntityId) -> Option<InvocationId> {
        self.owners.get(id).copied()
    }

    // ------------ PROPAGATION ------------ //

    /// Bring derived facts up to date, running views with `run`. It
    /// gets the view name, the key and the graph; an `Err` marks the
    /// run as failed — what it wrote before failing stays.
    pub fn propagate_with<F>(&mut self, mut run: F) -> Result<Report, PropagateError>
    where
        F: FnMut(&str, Option<EntityId>, &mut ViewGraph<'_>) -> Result<(), String>,
    {
        let mut report = Report::default();
        while let Some(invocation) = self.begin_next()? {
            let name = self.view_name(invocation.view).to_string();
            let mut graph = self.view_graph().expect("an invocation is running");
            let outcome = run(&name, invocation.key, &mut graph);
            self.finish()?;
            report.record(invocation, outcome);
        }
        Ok(report)
    }

    /// Process the changes made since the last step and start the
    /// next dirty invocation. `None` when everything is up to date.
    pub fn begin_next(&mut self) -> Result<Option<InvocationId>, PropagateError> {
        assert!(self.running.is_none(), "the previous run is not finished");
        self.absorb(None, &HashSet::new())?;
        loop {
            if let Some((view, causes)) = self.dirty_keys.pop_first() {
                self.refresh_keys(view, causes)?;
                continue;
            }
            let Some(invocation) = self.queue.pop_front() else {
                return Ok(None);
            };
            // Dropped while it waited.
            let Some(causes) = self.dirty.remove(&invocation) else {
                continue;
            };
            self.begin(invocation, causes);
            return Ok(Some(invocation));
        }
    }

    /// The graph as the running invocation sees it.
    pub fn view_graph(&mut self) -> Option<ViewGraph<'_>> {
        let running = self.running.as_mut()?;
        Some(ViewGraph {
            graph: &mut self.graph,
            run: &mut running.run,
        })
    }

    /// End the running invocation: remove what it did not write
    /// again, install its new read set and outputs, and mark dirty
    /// whatever its changes touched.
    pub fn finish(&mut self) -> Result<(), PropagateError> {
        let Running { mut run, causes } = self.running.take().expect("no invocation is running");
        let invocation = run.invocation;

        // Whatever the run did not write again is gone.
        let mut stale: Vec<EntityId> = run.stale.drain().collect();
        stale.sort();
        for id in stale {
            remove_entity(&mut self.graph, &id);
        }
        let mut detached: Vec<AttachTargetId> = run.stale_attached.drain().collect();
        detached.sort();
        for id in detached {
            if self.graph.has_attached_object(&id) {
                self.graph
                    .remove_attached(id)
                    .expect("checked: an object is attached");
            }
        }
        self.graph.writer = Writer::Input;

        let absorbed = self.absorb(Some(invocation), &causes);

        let outputs: HashSet<EntityId> = run
            .written
            .into_iter()
            .filter(|id| self.graph.is_exist(id))
            .collect();
        for id in &outputs {
            self.owners.insert(*id, invocation);
        }
        for key in &run.deps {
            self.readers
                .entry(key.clone())
                .or_default()
                .insert(invocation);
        }
        let state = self
            .invocations
            .get_mut(&invocation)
            .expect("a running invocation exists");
        state.deps = run.deps;
        state.outputs = outputs;

        absorbed
    }

    fn begin(&mut self, invocation: InvocationId, causes: HashSet<Cause>) {
        let state = self
            .invocations
            .get_mut(&invocation)
            .expect("a dirty invocation exists");
        let deps = std::mem::take(&mut state.deps);
        let stale = state.outputs.clone();
        self.forget_reads(invocation, &deps);

        let stale_attached = stale
            .iter()
            .filter(|id| self.graph.has_attached_object(id))
            .copied()
            .collect();
        let name = self.view_name(invocation.view).to_string();
        self.graph.writer = Writer::Derived;
        self.running = Some(Running {
            run: Run::new(invocation, name, stale, stale_attached),
            causes,
        });
    }

    /// Drain the journal: update ownership and mark dirty every
    /// invocation that read a touched key or lost an output to
    /// someone else. `source` is the invocation that made the
    /// changes, if any; it never dirties itself.
    fn absorb(
        &mut self,
        source: Option<InvocationId>,
        causes: &HashSet<Cause>,
    ) -> Result<(), PropagateError> {
        let Some(journal) = self.graph.journal.as_mut() else {
            return Ok(());
        };
        let patches = std::mem::take(journal);
        if patches.is_empty() {
            return Ok(());
        }

        let mut touched = HashSet::new();
        let mut dirty = BTreeSet::new();
        for patch in &patches {
            deps::touched_keys(patch, &mut touched);
            let id = subject(patch);
            if let Some(&owner) = self.owners.get(&id) {
                if is_removal(patch) {
                    self.owners.remove(&id);
                    if let Some(state) = self.invocations.get_mut(&owner) {
                        state.outputs.remove(&id);
                    }
                }
                dirty.insert(owner);
            }
        }

        let mut keyed = BTreeSet::new();
        for key in &touched {
            if let Some(readers) = self.readers.get(key) {
                dirty.extend(readers.iter().copied());
            }
            if let Some(views) = self.patterns.get(key) {
                keyed.extend(views.iter().copied());
            }
        }
        if let Some(source) = source {
            dirty.remove(&source);
        }

        let mut causes = causes.clone();
        causes.extend(source.map(Cause::Run));
        let mut result = Ok(());
        for invocation in dirty {
            if let Err(cycle) = self.mark_dirty(invocation, &causes) {
                result = result.and(Err(cycle));
            }
        }
        for view in keyed {
            if let Err(cycle) = self.mark_keys_dirty(view, &causes) {
                result = result.and(Err(cycle));
            }
        }
        result
    }

    fn mark_dirty(
        &mut self,
        invocation: InvocationId,
        causes: &HashSet<Cause>,
    ) -> Result<(), PropagateError> {
        if !self.invocations.contains_key(&invocation) {
            return Ok(());
        }
        if causes.contains(&Cause::Run(invocation)) {
            return Err(cycle(causes));
        }
        match self.dirty.entry(invocation) {
            Entry::Occupied(mut entry) => entry.get_mut().extend(causes),
            Entry::Vacant(entry) => {
                entry.insert(causes.clone());
                self.queue.push_back(invocation);
            }
        }
        Ok(())
    }

    fn mark_keys_dirty(
        &mut self,
        view: ViewId,
        causes: &HashSet<Cause>,
    ) -> Result<(), PropagateError> {
        if causes.contains(&Cause::Keys(view)) {
            return Err(cycle(causes));
        }
        match self.dirty_keys.entry(view) {
            btree_map::Entry::Occupied(mut entry) => entry.get_mut().extend(causes),
            btree_map::Entry::Vacant(entry) => {
                entry.insert(causes.clone());
            }
        }
        Ok(())
    }

    /// Bring a `for_each` view's invocations in line with the nodes
    /// its pattern matches now.
    fn refresh_keys(
        &mut self,
        view: ViewId,
        mut causes: HashSet<Cause>,
    ) -> Result<(), PropagateError> {
        let index = view.0 as usize;
        let pattern = self.views[index]
            .for_each
            .clone()
            .expect("only `for_each` views have keys");
        let wanted = pattern_keys(&self.graph, &pattern);
        let current = std::mem::replace(&mut self.views[index].keys, wanted.clone());
        causes.insert(Cause::Keys(view));

        for key in current.difference(&wanted) {
            self.drop_invocation(InvocationId {
                view,
                key: Some(*key),
            });
        }
        let mut result = Ok(());
        for key in wanted.difference(&current) {
            let invocation = InvocationId {
                view,
                key: Some(*key),
            };
            self.invocations.insert(invocation, Invocation::default());
            if let Err(cycle) = self.mark_dirty(invocation, &causes) {
                result = result.and(Err(cycle));
            }
        }
        // What the dropped invocations owned is gone now.
        result.and(self.absorb(None, &causes))
    }

    /// Forget an invocation and remove everything it owns.
    fn drop_invocation(&mut self, invocation: InvocationId) {
        let Some(state) = self.invocations.remove(&invocation) else {
            return;
        };
        self.forget_reads(invocation, &state.deps);
        self.dirty.remove(&invocation);

        let mut outputs: Vec<EntityId> = state.outputs.into_iter().collect();
        outputs.sort();
        self.graph.writer = Writer::Derived;
        for id in outputs {
            remove_entity(&mut self.graph, &id);
        }
        self.graph.writer = Writer::Input;
    }

    fn forget_reads(&mut self, invocation: InvocationId, deps: &HashSet<DepKey>) {
        for key in deps {
            if let Entry::Occupied(mut readers) = self.readers.entry(key.clone()) {
                readers.get_mut().remove(&invocation);
                if readers.get().is_empty() {
                    readers.remove();
                }
            }
        }
    }
}

fn cycle(causes: &HashSet<Cause>) -> PropagateError {
    let mut runs: Vec<InvocationId> = causes
        .iter()
        .filter_map(|cause| match cause {
            Cause::Run(invocation) => Some(*invocation),
            Cause::Keys(_) => None,
        })
        .collect();
    runs.sort();
    PropagateError::Cycle { runs }
}

/// The dependency key a `for_each` pattern's key set depends on.
fn pattern_key(pattern: &Pattern) -> DepKey {
    match pattern {
        Pattern::Nodes => DepKey::AllNodes,
        Pattern::NodesWhere { field, value } => DepKey::NodesWhere(field.clone(), value.clone()),
    }
}

/// Nodes a `for_each` pattern matches.
fn pattern_keys(graph: &Graph, pattern: &Pattern) -> BTreeSet<EntityId> {
    match pattern {
        Pattern::Nodes => graph.iter_nodes().collect(),
        Pattern::NodesWhere { field, value } => graph.nodes_where(field, value).collect(),
    }
}

/// Remove a node, edge or hyperedge, whichever `id` is. It may be
/// gone already, taken by a cascade.
fn remove_entity(graph: &mut Graph, id: &EntityId) {
    if graph.is_node(id) {
        graph.remove_node(id).expect("checked: it is a node");
    } else if graph.edges.contains_key(id) {
        graph.remove_edge(id).expect("checked: it is an edge");
    } else if graph.hyperedges.contains_key(id) {
        graph
            .remove_hyperedge(id)
            .expect("checked: it is a hyperedge");
    }
}

/// The entity a patch is about.
pub(crate) fn subject(patch: &FullPatch) -> EntityId {
    match patch {
        FullPatch::AddNode { id, .. }
        | FullPatch::RemoveNode { id, .. }
        | FullPatch::ChangeNode { id, .. }
        | FullPatch::UpsertNode { id, .. }
        | FullPatch::AddEdge { id, .. }
        | FullPatch::RemoveEdge { id, .. }
        | FullPatch::RetargetEdge { id, .. }
        | FullPatch::UpsertEdgeData { id, .. }
        | FullPatch::ChangeEdgeData { id, .. }
        | FullPatch::RemoveEdgeData { id, .. }
        | FullPatch::CreateHyperedge { id, .. }
        | FullPatch::RemoveHyperedge { id, .. }
        | FullPatch::AddHyperedgeMembers { id, .. }
        | FullPatch::RemoveHyperedgeMembers { id, .. }
        | FullPatch::UpsertHyperedgeData { id, .. }
        | FullPatch::ChangeHyperedgeData { id, .. }
        | FullPatch::RemoveHyperedgeData { id, .. } => *id,
    }
}

fn is_removal(patch: &FullPatch) -> bool {
    matches!(
        patch,
        FullPatch::RemoveNode { .. }
            | FullPatch::RemoveEdge { .. }
            | FullPatch::RemoveHyperedge { .. }
    )
}
