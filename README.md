# COG's

![Logo](readme_asserts/logo.webp)

COG's is database espicaly design for language tools such as: translators (disassemblers, transpilers, compilers), IDE, static analyzer, symbolic code analyzer and so on. 

Key features:
* **Powerful data model**
* **Rust dialect instead of query DSL** - **Partially implemented**
* **JIT on board** - with storage-aware and code-aware optimization: **Partially implemented**
* **Delta-patches based persistence**
* **Disk persistence** - store state on disk: **Not currently implemented**
* **DLT (Diagnostic Log and Trace)** - in machine-readable (as JSON): **Not currently implemented**

# Ready to use?

Short answer: no.

Longer answer: I wrote this alone, with heavy assistance from an LLM. There are bugs I haven't found, bugs I'm pretending I haven't seen. Classic chicken-and-egg — the project needs users to surface bugs, and the bugs are what's keeping users away. If you'd like to be the chicken (or the egg, dealer's choice), godspeed.

Please accept the gif below as official documentation of my regret.

![Were sorry](readme_asserts/were-sorry.gif)



