//! Host runtime for COG WASM modules.
//!
//! A [`Host`] owns a [`Database`] and the WASM modules loaded into
//! it. Every `__cog_view_*` export of a module is registered as a
//! view; [`Host::propagate`] runs the dirty invocations until derived
//! facts are up to date.
//!
//! Graph intrinsics (`__cog_*`) exist only while a view runs. They go
//! through [`ViewGraph`], which records what the view reads and keeps
//! its writes to entities it owns. See [`abi`] for the calling
//! convention.

mod to_abi;

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use common::{
    AttachTargetId, EdgeId, EntityId, Field, HyperedgeId, NodeId, Object, Pointee, RetargetEdge,
    abi,
};
use db::{Database, Graph, InvocationId, PropagateError, Report, ViewGraph, ViewId};
use serde::{Serialize, de::DeserializeOwned};
use wasmtime::{Caller, Engine, Instance, Linker, Memory, Module, Store, TypedFunc};

use to_abi::ToAbi;

pub struct Runtime {
    engine: Engine,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

struct HostState {
    db: Database,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    /// A host with views over `graph`; load modules into it.
    pub fn host(&self, graph: Graph) -> Result<Host> {
        let mut linker: Linker<HostState> = Linker::new(&self.engine);
        link_intrinsics(&mut linker)?;
        Ok(Host {
            store: Store::new(
                &self.engine,
                HostState {
                    db: Database::new(graph),
                },
            ),
            linker,
            guests: Vec::new(),
            views: HashMap::new(),
        })
    }

    /// Load `wasm_bytes` into a host over `graph`, run its views and
    /// return the graph with derived facts. Fails if a view fails.
    pub fn run_views(&self, wasm_bytes: &[u8], graph: Graph) -> Result<Graph> {
        let mut host = self.host(graph)?;
        host.load(wasm_bytes)?;
        let report = host.propagate()?;
        if let Some((invocation, reason)) = report.failures.first() {
            anyhow::bail!("`{}` failed: {reason}", host.db().describe(*invocation));
        }
        Ok(host.into_graph())
    }
}

/// A database with WASM views.
pub struct Host {
    store: Store<HostState>,
    linker: Linker<HostState>,
    guests: Vec<Guest>,
    views: HashMap<ViewId, ViewFunc>,
}

/// One instantiated module.
struct Guest {
    alloc: TypedFunc<u32, u32>,
    memory: Memory,
}

/// The entry point of a view and the module it lives in.
#[derive(Clone)]
struct ViewFunc {
    guest: usize,
    func: TypedFunc<(u32, u32), ()>,
}

impl Host {
    /// Instantiate a module and register its views. Returns their ids
    /// in export order; they are dirty until the next propagation.
    pub fn load(&mut self, wasm_bytes: &[u8]) -> Result<Vec<ViewId>> {
        let module = Module::new(self.store.engine(), wasm_bytes).context("compile wasm module")?;
        let names: Vec<String> = module
            .exports()
            .filter_map(|e| e.name().strip_prefix(abi::VIEW_EXPORT_PREFIX))
            .map(str::to_string)
            .collect();
        if names.is_empty() {
            anyhow::bail!("module has no `{}*` exports", abi::VIEW_EXPORT_PREFIX);
        }

        let instance = self
            .linker
            .instantiate(&mut self.store, &module)
            .context("instantiate module")?;
        let memory = instance
            .get_memory(&mut self.store, "memory")
            .context("guest exports no `memory`")?;
        let guest = self.guests.len();
        self.guests.push(Guest {
            alloc: instance
                .get_typed_func(&mut self.store, abi::ALLOC_EXPORT)
                .with_context(|| format!("look up `{}`", abi::ALLOC_EXPORT))?,
            memory,
        });

        let mut ids = Vec::new();
        for name in names {
            let export = format!("{}{name}", abi::VIEW_EXPORT_PREFIX);
            let func = instance
                .get_typed_func::<(u32, u32), ()>(&mut self.store, &export)
                .with_context(|| format!("look up `{export}`"))?;
            let meta = self.read_meta(&instance, memory, &name)?;
            let id = self
                .store
                .data_mut()
                .db
                .register_view(name, meta.for_each)?;
            self.views.insert(id, ViewFunc { guest, func });
            ids.push(id);
        }
        Ok(ids)
    }

    /// Bring derived facts up to date. A view that traps is reported
    /// in [`Report::failures`] and does not stop the propagation; a
    /// dependency cycle does.
    pub fn propagate(&mut self) -> Result<Report> {
        let mut report = Report::default();
        loop {
            let next = self.store.data_mut().db.begin_next();
            let Some(invocation) = next.map_err(|err| self.explain(err))? else {
                return Ok(report);
            };
            let outcome = self.call_view(invocation);
            let finished = self.store.data_mut().db.finish();
            finished.map_err(|err| self.explain(err))?;
            report.record(invocation, outcome.map_err(|err| format!("{err:?}")));
        }
    }

    /// `err` with view names instead of ids.
    fn explain(&self, err: PropagateError) -> anyhow::Error {
        match &err {
            PropagateError::Cycle { runs } => {
                let runs: Vec<String> = runs.iter().map(|run| self.db().describe(*run)).collect();
                anyhow::anyhow!("dependency cycle between {}", runs.join(", "))
            }
        }
    }

    pub fn db(&self) -> &Database {
        &self.store.data().db
    }

    pub fn graph(&self) -> &Graph {
        self.db().graph()
    }

    /// The graph for input changes; [`Host::propagate`] afterwards.
    pub fn graph_mut(&mut self) -> &mut Graph {
        self.store.data_mut().db.graph_mut()
    }

    pub fn into_graph(self) -> Graph {
        self.store.into_data().db.into_graph()
    }

    fn call_view(&mut self, invocation: InvocationId) -> Result<()> {
        let ViewFunc { guest, func } = self.views[&invocation.view].clone();
        let args = bincode::serialize(&invocation.key).context("encode view key")?;
        let guest = &self.guests[guest];
        let ptr = guest.alloc.call(&mut self.store, args.len() as u32)?;
        guest
            .memory
            .write(&mut self.store, ptr as usize, &args)
            .context("write guest memory")?;
        func.call(&mut self.store, (ptr, args.len() as u32))
    }

    /// The `__cog_meta_<name>` export, or the defaults if the module
    /// has none.
    fn read_meta(
        &mut self,
        instance: &Instance,
        memory: Memory,
        name: &str,
    ) -> Result<abi::ViewMeta> {
        let export = format!("{}{name}", abi::META_EXPORT_PREFIX);
        let Ok(meta) = instance.get_typed_func::<(), u64>(&mut self.store, &export) else {
            return Ok(abi::ViewMeta::default());
        };
        let (ptr, len) = abi::unpack_buf(meta.call(&mut self.store, ())?);
        let mut bytes = vec![0u8; len as usize];
        memory
            .read(&self.store, ptr as usize, &mut bytes)
            .context("read guest memory")?;
        instance
            .get_typed_func::<(u32, u32), ()>(&mut self.store, abi::FREE_EXPORT)
            .with_context(|| format!("look up `{}`", abi::FREE_EXPORT))?
            .call(&mut self.store, (ptr, len))?;
        bincode::deserialize(&bytes).with_context(|| format!("decode `{export}`"))
    }
}

/// Register every graph intrinsic. Each one mirrors a method of
/// [`ViewGraph`]; see [`abi`] for the calling convention.
fn link_intrinsics(linker: &mut Linker<HostState>) -> Result<()> {
    // ----- constructors -----
    host_fn(linker, "__cog_add_node", |g, obj: Object| g.add_node(obj))?;
    host_fn(
        linker,
        "__cog_add_edge",
        |g, (source, target): (Pointee, Pointee)| g.add_edge(source, target).map_err(ToAbi::to_abi),
    )?;
    host_fn(
        linker,
        "__cog_create_hyperedge",
        |g, members: HashSet<Pointee>| g.create_hyperedge(members).map_err(ToAbi::to_abi),
    )?;

    // ----- destructors -----
    host_fn(linker, "__cog_remove_node", |g, id: NodeId| {
        g.remove_node(&id).map_err(ToAbi::to_abi)
    })?;
    host_fn(linker, "__cog_remove_edge", |g, id: EdgeId| {
        g.remove_edge(&id).map_err(ToAbi::to_abi)
    })?;
    host_fn(linker, "__cog_remove_hyperedge", |g, id: HyperedgeId| {
        g.remove_hyperedge(&id).map_err(ToAbi::to_abi)
    })?;
    host_fn(linker, "__cog_remove_attached", |g, id: AttachTargetId| {
        g.remove_attached(id).map_err(ToAbi::to_abi)
    })?;

    // ----- modifiers -----
    host_fn(
        linker,
        "__cog_attach_obj",
        |g, (id, obj): (AttachTargetId, Object)| g.attach_obj(id, obj).map_err(ToAbi::to_abi),
    )?;
    host_fn(
        linker,
        "__cog_add_hyperedge_members",
        |g, (id, members): (HyperedgeId, HashSet<Pointee>)| {
            g.add_hyperedge_members(id, members).map_err(ToAbi::to_abi)
        },
    )?;
    host_fn(
        linker,
        "__cog_remove_hyperedge_members",
        |g, (id, members): (HyperedgeId, HashSet<Pointee>)| {
            g.remove_hyperedge_members(id, members)
                .map_err(ToAbi::to_abi)
        },
    )?;
    host_fn(
        linker,
        "__cog_replace_node",
        |g, (id, obj): (NodeId, Object)| g.replace_node(&id, obj).map_err(ToAbi::to_abi),
    )?;
    host_fn(
        linker,
        "__cog_replace_attached_obj",
        |g, (id, obj): (AttachTargetId, Object)| {
            g.replace_attached_obj(&id, obj).map_err(ToAbi::to_abi)
        },
    )?;
    host_fn(
        linker,
        "__cog_retarget_edge",
        |g, (id, new_target): (EdgeId, RetargetEdge)| {
            g.retarget_edge(&id, new_target).map_err(ToAbi::to_abi)
        },
    )?;

    // ----- queries -----
    host_fn(linker, "__cog_iter_entities", |g, (): ()| g.iter_entities())?;
    host_fn(linker, "__cog_iter_nodes", |g, (): ()| g.iter_nodes())?;
    host_fn(linker, "__cog_iter_edges", |g, (): ()| g.iter_edges())?;
    host_fn(linker, "__cog_iter_hyperedges", |g, (): ()| {
        g.iter_hyperedges()
    })?;
    host_fn(linker, "__cog_iter_attached", |g, (): ()| g.iter_attached())?;
    host_fn(linker, "__cog_obj", |g, id: EntityId| g.obj(&id))?;
    host_fn(
        linker,
        "__cog_field",
        |g, (id, name): (EntityId, String)| g.field(&id, &name),
    )?;
    host_fn(linker, "__cog_edge", |g, id: EdgeId| {
        g.edge(&id).map_err(ToAbi::to_abi)
    })?;
    host_fn(linker, "__cog_hyperedge_members", |g, id: HyperedgeId| {
        g.hyperedge_members(&id)
    })?;
    host_fn(linker, "__cog_classify_pointee", |g, p: Pointee| {
        g.classify_pointee(&p)
    })?;
    host_fn(linker, "__cog_is_exist", |g, id: EntityId| g.is_exist(&id))?;
    host_fn(linker, "__cog_is_pointee_exist", |g, p: Pointee| {
        g.is_pointee_exist(&p)
    })?;
    host_fn(linker, "__cog_out_edges", |g, p: Pointee| g.out_edges(&p))?;
    host_fn(linker, "__cog_in_edges", |g, p: Pointee| g.in_edges(&p))?;
    host_fn(linker, "__cog_hyperedges_of", |g, p: Pointee| {
        g.hyperedges_of(&p)
    })?;
    host_fn(
        linker,
        "__cog_nodes_where",
        |g, (field, value): (String, Field)| g.nodes_where(&field, &value),
    )?;

    Ok(())
}

/// Register intrinsic `name` that decodes its arguments as `A`,
/// runs `f` against the running invocation's graph and hands the
/// bincode of `R` back to the guest.
fn host_fn<A, R, F>(linker: &mut Linker<HostState>, name: &str, f: F) -> Result<()>
where
    A: DeserializeOwned,
    R: Serialize,
    F: Fn(&mut ViewGraph<'_>, A) -> R + Send + Sync + 'static,
{
    let intrinsic = name.to_string();
    linker
        .func_wrap(
            "env",
            name,
            move |mut caller: Caller<'_, HostState>, ptr: u32, len: u32| -> Result<u64> {
                let args: A = read_bincode(&mut caller, ptr, len)?;
                let out = {
                    let mut graph =
                        caller.data_mut().db.view_graph().with_context(|| {
                            format!("`{intrinsic}` called outside of a view run")
                        })?;
                    f(&mut graph, args)
                };
                let bytes = bincode::serialize(&out).context("encode intrinsic result")?;
                return_buf(&mut caller, &bytes)
            },
        )
        .with_context(|| format!("register {name}"))?;
    Ok(())
}

fn guest_memory(caller: &mut Caller<'_, HostState>) -> Result<Memory> {
    caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .context("guest exports no `memory`")
}

fn read_bincode<T: DeserializeOwned>(
    caller: &mut Caller<'_, HostState>,
    ptr: u32,
    len: u32,
) -> Result<T> {
    let memory = guest_memory(caller)?;
    let mut bytes = vec![0u8; len as usize];
    memory
        .read(&*caller, ptr as usize, &mut bytes)
        .context("read guest memory")?;
    bincode::deserialize(&bytes).context("decode guest payload")
}

/// Copy `bytes` into a fresh guest buffer obtained from
/// [`abi::ALLOC_EXPORT`] and return it packed for the guest.
fn return_buf(caller: &mut Caller<'_, HostState>, bytes: &[u8]) -> Result<u64> {
    let alloc = caller
        .get_export(abi::ALLOC_EXPORT)
        .and_then(|e| e.into_func())
        .with_context(|| format!("guest exports no `{}`", abi::ALLOC_EXPORT))?
        .typed::<u32, u32>(&*caller)?;
    let len = bytes.len() as u32;
    let ptr = alloc.call(&mut *caller, len)?;
    let memory = guest_memory(caller)?;
    memory
        .write(&mut *caller, ptr as usize, bytes)
        .context("write guest memory")?;
    Ok(abi::pack_buf(ptr, len))
}
