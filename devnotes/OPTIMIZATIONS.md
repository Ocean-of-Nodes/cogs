# Optimization's

![Red line](redline.gif)

This note attempts to cover all significant optimizations made. So, yes, a bit of technoporn.

## Storage

### Data-aware

**Not currently implemented**

### Index for `nodes_where`

`Graph::nodes_where(field, value)` scans every node, and so does the key set of a `for_each` view. Views depend on the `(field, value)` pair, not on how it is looked up, so an index by value only changes the speed.

**Not currently implemented**

## Code-aware

**Not currently implemented**

## JIT

### Remove unnecessary validation

The call to `db.add_edge(a, b)` checks that `a` and `b` are alive. But if the WASM module's stats analysis sees that a was created by the call to `add_node` on the line above, the check is redundant. The JIT simply routes this call to an unchecked (`silent_*`) path inside `db`.

**Not currently implemented**

### Query fusion

**Not currently implemented**

### Differential dataflow

**Not currently implemented**

