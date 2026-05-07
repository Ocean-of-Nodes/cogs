# Optimization's

![Red line](redline.gif)

This note attempts to cover all significant optimizations made. So, yes, a bit of technoporn.

## Storage

### Data-aware

**Not currently implemented**

## Code-aware

**Not currently implemented**

## JIT

### Remove unnecessary validation

The call to `db.add_edge(a, b)` checks that `a` and `b` are alive. But if the WASM module's stats analysis sees that a was created by the call to `add_node` on the line above, the check is redundant. The JIT simply routes this call directly to `storage.put_edge`.

**Not currently implemented**

### Query fusion

**Not currently implemented**

### Differential dataflow

**Not currently implemented**

