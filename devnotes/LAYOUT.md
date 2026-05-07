# Project layout

The project layout is quite complex, but we'll try to figure it out. Let's get started.

For example we have module `hellow-view` that's we wont load to DB then we need `dialect`. 
`dialect` it is crate's that provide on wasm side API for code and on the other side 
provide an ABI for the execution engine. `dialect` represented by two crates: `sdk` and `sdk-macros`.

Next we have the `common` crate, it contains some common types such as objects and identifiers.
It shared multiple time along all side's that need it: `sdk`, `sdk-macros`, `db`, `jit`, `storage`, `delta`.



```
                                 ┌──────────────────────────────────────┐
                                 │ user crate  (e.g. hellow-view)       │
                                 │                                      │
                                 │   #[view]                            │
                                 │   fn hellow(g: &mut Graph) {         │
                                 │       g.add_node(obj);               │
                                 │   }                                  │
                                 └──────────────┬───────────────────────┘
                                                │ uses
                            ┌───────────────────┴───────────────────┐
                            ▼                                       ▼
                  ┌──────────────────────┐              ┌─────────────────────┐
                  │  sdk        (rlib)   │              │  sdk-macros (proc)  │
                  │                      │              │                     │
                  │  • Graph, Node       │              │  #[view]            │
                  │  • safe wrappers     │              │  #[mutator]         │
                  │  • extern "C" decls  │              │  #[procedure]       │
                  │    of intrinsics     │              │  generates ABI glue │
                  │  • re-export types   │              │                     │
                  └──────────┬───────────┘              └──────────┬──────────┘
                             │                                     │
                             └──────────────┬──────────────────────┘
                                            ▼
                              ┌────────────────────────────┐
                              │  common  (no_std-friendly) │
                              │                            │
                              │  Field, Object, Pointee,   │
                              │  EntityId, GlobalObjPath,  │
                              │  Patch, Delta, ...         │
                              │  serde behind feature      │
                              └─────────────┬──────────────┘
                                            │
                                            │ shared types
                                            │
        ═════════════════════════════ WASM ═╪═ BOUNDARY ═══════════════════════
                                            │
                                            ▼
                  ┌──────────────────────────────────────────────────────────┐
                  │  jit         (host runtime)                              │
                  │                                                          │
                  │  ┌─────────────────┐    ┌─────────────────────────┐      │
                  │  │   wasmtime      │ ◀▶ │  host-fn registry       │      │
                  │  │   (engine, run) │    │   __cog_add_node        │      │
                  │  └─────────────────┘    │   __cog_add_edge_safe   │      │
                  │                         │   __cog_add_edge_raw  ◀─┼───┐  │
                  │  ┌─────────────────┐    │   __cog_iter_edges      │   │  │
                  │  │  wasmparser     │    │   ...                   │   │  │
                  │  │  static analysis│ ─▶ │  decides which version  │   │  │
                  │  │  (pattern det.) │    │   to bind per module    │   │  │
                  │  └─────────────────┘    └─────────────┬───────────┘   │  │
                  │                                       │ calls         │  │
                  └───────────────────────────────────────┼───────────────┼──┘
                                                          │               │
                                            checked path  │   raw path    │
                                                          ▼               │
                                              ┌─────────────────┐         │
                                              │  db             │         │
                                              │                 │         │
                                              │ • semantic API  │         │
                                              │ • inv: endpts   │         │
                                              │ • cascade del   │         │
                                              │ • view registry │         │
                                              │ • trackers ──▶  │         │
                                              │   emits Patch   │         │
                                              └────────┬────────┘         │
                                                       │ raw ops          │
                                                       ▼                  │
                                              ┌─────────────────┐         │
                                              │  storage        │ ◀───────┘
                                              │                 │  (JIT may
                                              │ • put/get/del   │   bypass db
                                              │ • by-endpoint   │   when safe)
                                              │   index         │
                                              │ • by-member     │
                                              │   index         │
                                              │ • NO checks     │
                                              │ • NO cascade    │
                                              │ • NO trackers   │
                                              └─────────────────┘
                                                       ▲
                                                       │ materializes
                                                       │ view results
                                                       │ as ordinary records


           db emits Patch ───▶ ┌────────────┐ ───▶ ┌────────────┐ ───▶ ┌──────────┐
                               │  protocol  │      │ transports │      │  client  │
                               └────────────┘      └────────────┘      └──────────┘
```