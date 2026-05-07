//! Host runtime for COG WASM modules.
//!
//! Loads a `.wasm` produced from the `sdk` dialect, links host
//! intrinsics (`__cog_*`) against a [`Storage`], discovers every
//! `__cog_view_*` export and invokes them in module-declaration
//! order.
//!
//! For the prototype the JIT bypasses the semantic `db` layer and
//! writes directly to `Storage` — every intrinsic takes the "raw"
//! path. Once `db` exists as a checked wrapper we'll select between
//! checked/raw bindings per module based on static analysis.

use anyhow::{Context, Result};
use common::Object;
use storage::Storage;
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
    storage: Storage,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    /// Load `wasm_bytes`, link host intrinsics against `storage`,
    /// invoke every `__cog_view_*` export, and return the storage
    /// with mutations applied.
    pub fn run_views(&self, wasm_bytes: &[u8], storage: Storage) -> Result<Storage> {
        let module = Module::new(&self.engine, wasm_bytes).context("compile wasm module")?;

        let view_exports: Vec<String> = module
            .exports()
            .filter(|e| e.name().starts_with(VIEW_EXPORT_PREFIX))
            .map(|e| e.name().to_string())
            .collect();

        if view_exports.is_empty() {
            anyhow::bail!("module has no `{VIEW_EXPORT_PREFIX}*` exports");
        }

        let mut store = Store::new(&self.engine, HostState { storage });
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
                    let id = storage::fresh_id();
                    caller.data_mut().storage.put_node(id, obj);
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

        Ok(store.into_data().storage)
    }
}