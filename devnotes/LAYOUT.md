# Project layout

The project layout is quite complex, but we'll try to figure it out. Let's get started.

For example we have module `hellow-view` that's we wont load to DB then we need `dialect`. 
`dialect` it is crate's that provide on wasm side API for code and on the other side 
provide an ABI for the execution engine. `dialect` represented by two crates: `sdk` and `sdk-macros`.

Next we have the `common` crate, it contains some common types such as objects and identifiers.
It shared multiple time along all side's that need it: `sdk`, `sdk-macros`, `db`, `jit`, `delta`.



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
                  │  └─────────────────┘    │   __cog_add_edge        │      │
                  │                         │   __cog_iter_edges      │      │
                  │                         │   ...                   │      │
                  │                         └─────────────┬───────────┘      │
                  │                                       │ calls            │
                  └───────────────────────────────────────┼──────────────────┘
                                                          │
                                                          ▼
                                              ┌─────────────────────┐
                                              │  db                 │
                                              │                     │
                                              │ • Graph: semantic   │
                                              │   API + storage     │
                                              │ • inv: endpts       │
                                              │ • cascade del       │
                                              │ • reverse index     │
                                              │   (pointee uses)    │
                                              │ • view registry     │
                                              │ • trackers ──▶      │
                                              │   emits Patch       │
                                              └─────────────────────┘

           db emits Patch ───▶ ┌────────────┐ ───▶ ┌────────────┐ ───▶ ┌──────────┐
                               │  protocol  │      │ transports │      │  client  │
                               └────────────┘      └────────────┘      └──────────┘
```