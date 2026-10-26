//! Host runtime for COG WASM modules.
//!
//! Loads a `.wasm` produced from the `sdk` dialect, links host
//! intrinsics (`__cog_*`) against a [`Graph`], discovers every
//! `__cog_view_*` export and invokes them in module-declaration
//! order.
//!
//! Every intrinsic goes through the checked public API of `db`.

mod to_abi;

use std::collections::HashSet;

use anyhow::{Context, Result};
use common::{
    AttachTargetId, EdgeId, EntityId, HyperedgeId, NodeId, Object, Pointee, RetargetEdge, abi,
};
use db::Graph;
use serde::{Serialize, de::DeserializeOwned};
use wasmtime::{Caller, Engine, Linker, Memory, Module, Store};

use to_abi::ToAbi;

const VIEW_EXPORT_PREFIX: &str = "__cog_view_";

pub struct Runtime {
    engine: Engine,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

struct HostState {
    graph: Graph,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    /// Load `wasm_bytes`, link host intrinsics against `graph`,
    /// invoke every `__cog_view_*` export, and return the graph
    /// with mutations applied.
    pub fn run_views(&self, wasm_bytes: &[u8], graph: Graph) -> Result<Graph> {
        let module = Module::new(&self.engine, wasm_bytes).context("compile wasm module")?;

        let view_exports: Vec<String> = module
            .exports()
            .filter(|e| e.name().starts_with(VIEW_EXPORT_PREFIX))
            .map(|e| e.name().to_string())
            .collect();

        if view_exports.is_empty() {
            anyhow::bail!("module has no `{VIEW_EXPORT_PREFIX}*` exports");
        }

        let mut store = Store::new(&self.engine, HostState { graph });
        let mut linker: Linker<HostState> = Linker::new(&self.engine);

        link_intrinsics(&mut linker)?;

        let instance = linker
            .instantiate(&mut store, &module)
            .context("instantiate module")?;

        for export_name in &view_exports {
            let func = instance
                .get_typed_func::<(), ()>(&mut store, export_name.as_str())
                .with_context(|| format!("look up `{export_name}`"))?;
            func.call(&mut store, ())
                .with_context(|| format!("`{export_name}` trapped"))?;
        }

        Ok(store.into_data().graph)
    }
}

/// Register every graph intrinsic. Each one mirrors a public method
/// of [`Graph`]; see [`abi`] for the calling convention.
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
    host_fn(linker, "__cog_undo", |g, (): ()| {
        g.undo().map_err(ToAbi::to_abi)
    })?;

    // ----- queries -----
    host_fn(linker, "__cog_iter_entities", |g, (): ()| {
        g.iter_entities().collect::<Vec<_>>()
    })?;
    host_fn(linker, "__cog_iter_nodes", |g, (): ()| {
        g.iter_nodes().collect::<Vec<_>>()
    })?;
    host_fn(linker, "__cog_iter_edges", |g, (): ()| {
        g.iter_edges().collect::<Vec<_>>()
    })?;
    host_fn(linker, "__cog_iter_hyperedges", |g, (): ()| {
        g.iter_hyperedges().collect::<Vec<_>>()
    })?;
    host_fn(linker, "__cog_iter_attached", |g, (): ()| {
        g.iter_attached().collect::<Vec<_>>()
    })?;
    host_fn(linker, "__cog_obj", |g, id: EntityId| g.obj(&id).cloned())?;
    host_fn(linker, "__cog_edge", |g, id: EdgeId| {
        g.edge(&id).map_err(ToAbi::to_abi)
    })?;
    host_fn(linker, "__cog_hyperedge_members", |g, id: HyperedgeId| {
        g.hyperedge_members(&id).cloned()
    })?;
    host_fn(linker, "__cog_classify_pointee", |g, p: Pointee| {
        g.classify_pointee(&p)
    })?;
    host_fn(linker, "__cog_is_exist", |g, id: EntityId| g.is_exist(&id))?;
    host_fn(linker, "__cog_is_pointee_exist", |g, p: Pointee| {
        g.is_pointee_exist(&p)
    })?;

    Ok(())
}

/// Register intrinsic `name` that decodes its arguments as `A`,
/// runs `f` against the graph and hands the bincode of `R` back to
/// the guest.
fn host_fn<A, R, F>(linker: &mut Linker<HostState>, name: &str, f: F) -> Result<()>
where
    A: DeserializeOwned,
    R: Serialize,
    F: Fn(&mut Graph, A) -> R + Send + Sync + 'static,
{
    linker
        .func_wrap(
            "env",
            name,
            move |mut caller: Caller<'_, HostState>, ptr: u32, len: u32| -> Result<u64> {
                let args: A = read_bincode(&mut caller, ptr, len)?;
                let out = f(&mut caller.data_mut().graph, args);
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
