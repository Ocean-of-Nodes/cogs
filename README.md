<div align="center">

# COG's

![Logo](readme_asserts/logo.webp)

*A database purpose-built for language tooling — translators (disassemblers, transpilers, compilers), IDEs, static analyzers, symbolic execution engines, and the like.*

</div>

---

## Key Features

| Feature                               | Description                                    |  Status    |
| ------------------------------------- | ---------------------------------------------- | ---------- |
| **Powerful data model**               | Designed around the needs of language tools    |     ✅     |
| **Delta-patch persistence**           | State is captured as a stream of deltas        |     ✅     |
| **Rust dialect instead of query DSL** | Write queries in the language you already know | 🚧 Partial |
| **JIT on board**                      | Storage-aware and code-aware optimization      | 🚧 Partial |
| **Disk persistence**                  | Persist state to disk                          | ⏳ Planned |
| **DLT (Diagnostic Log and Trace)**    | Machine-readable diagnostics (JSON)            | ⏳ Planned |

---

## Ready to Use?

**Short answer:** no.

**Longer answer:** I wrote this alone, with heavy assistance from an LLM. There are bugs I haven't found, and bugs I'm pretending I haven't seen. Classic chicken-and-egg — the project needs users to surface bugs, and the bugs are what's keeping users away. If you'd like to be the chicken (or the egg, dealer's choice), godspeed.

Please accept the gif below as official documentation of my regret.

<div align="center">

![We're sorry](readme_asserts/were-sorry.gif)

</div>
