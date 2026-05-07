//! WASM-side library used by user code (views, mutators, procedures).
//!
//! This crate is meant to be compiled to `wasm32-unknown-unknown`.
//! It declares host intrinsics as `extern "C"` imports and wraps them
//! in safe APIs (`Graph::add_node`, ...). The host (`jit` crate)
//! supplies real implementations.
//!
//! Object/Field/Pointee come from `common` so that wire/host/guest
//! all share one data model.

pub use common::{EntityId, Field, Object, Pointee};
pub use sdk_macros::view;

// On `wasm32-unknown-unknown` there is no entropy source, but std's
// `HashMap` calls `getrandom` on first use to seed its hasher. Register
// a deterministic stub so `Object::new()` (which is `HashMap::new()`)
// doesn't trap. Hash-flooding is irrelevant here — every WASM module
// runs in a sandboxed, single-threaded host invocation.
#[cfg(target_arch = "wasm32")]
fn always_zero(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    buf.fill(0);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
getrandom::register_custom_getrandom!(always_zero);

unsafe extern "C" {
    fn __cog_add_node(ptr: u32, len: u32);
}

/// Handle to the graph backing the current invocation.
///
/// In the prototype this is a zero-sized marker — there is exactly
/// one graph per invocation, hardcoded on the host side. Later it
/// will carry an opaque host handle so the same module can address
/// multiple graphs (captured spaces).
pub struct Graph {
    _private: (),
}

impl Graph {
    /// Constructed by macro-generated entry points; not meant for
    /// direct user use.
    #[doc(hidden)]
    pub fn __new() -> Self {
        Self { _private: () }
    }

    /// Insert a node carrying `obj`. Fire-and-forget: the host mints
    /// the id and inserts into storage. A future revision will return
    /// the new `NodeId`.
    pub fn add_node(&mut self, obj: Object) {
        let bytes = bincode::serialize(&obj).expect("serialize Object");
        unsafe {
            __cog_add_node(bytes.as_ptr() as u32, bytes.len() as u32);
        }
    }
}