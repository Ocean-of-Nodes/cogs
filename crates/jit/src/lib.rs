//! Host runtime for COG WASM modules.
//!
//! Loads a `.wasm` produced from the `sdk` dialect, links host
//! intrinsics (`__cog_*`) against a [`Graph`], discovers every
//! `__cog_view_*` export and invokes them in module-declaration
//! order.
//!
//! Every intrinsic goes through the checked public API of `db`.

use anyhow::{Context, Result};
use common::Object;
use db::Graph;
use wasmtime::{Caller, Engine, Linker, Module, Store};

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

        linker
            .func_wrap(
                "env",
                "__cog_add_node",
                |mut caller: Caller<'_, HostState>, ptr: u32, len: u32| -> Result<()> {
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .context("guest exports no `memory`")?;
                    let mut bytes = vec![0u8; len as usize];
                    memory
                        .read(&caller, ptr as usize, &mut bytes)
                        .context("read guest memory")?;
                    let obj: Object =
                        bincode::deserialize(&bytes).context("decode Object payload")?;
                    caller.data_mut().graph.add_node(obj);
                    Ok(())
                },
            )
            .context("register __cog_add_node")?;

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