# TODO

## Reverse index: Pointee → edges / hyperedges

Once edges and hyperedges accept subobjects as endpoints/members
(BOOK: "An edge is a directed link between any two entities — nodes,
suboject, edges, or hyperedges"), the database has to keep its
"endpoints exist" invariant under two new failure modes:

1. **Subobject removal.** Editing an `Object` via
   `ObjectDelta::RemoveField` / `ReplaceField` / `SubObjectDelta`
   may delete (or change the type of) a field that an edge points
   to. That edge becomes dangling and must be removed.
2. **Entity removal that takes its subobjects with it.** Removing
   an entity already cascades to incident edges, but it must also
   take with it every edge that referenced *any* subobject of that
   entity — i.e. any `Pointee::Path { entity: <removed>, .. }`.

The straightforward approach today — iterate every edge on every
removal — is O(E) per delete. Unacceptable past toy graphs.

### Plan

Maintain a reverse index alongside `Graph::edges` /
`Graph::hyper_edge`:

```rust
// each Pointee that's currently used as an endpoint or hyperedge
// member maps to the structural elements that reference it.
pointee_uses: HashMap<Pointee, PointeeUses>,

struct PointeeUses {
    edges_as_source: HashSet<EdgeID>,
    edges_as_target: HashSet<EdgeID>,
    hyperedges: HashSet<HyperEdgeId>,
}
```

Maintenance:

- `add_edge(src, tgt)` — insert edge into `pointee_uses[src].edges_as_source`
  and `pointee_uses[tgt].edges_as_target`.
- `remove_edge(eid)` — strip the edge from both buckets; drop the
  bucket entry if it goes empty.
- `retarget_edge` — move the edge between the affected buckets.
- `create_hyperedge(members)` /
  `add_elements_to_hyperedge` /
  `remove_elements_from_hyperedge` — keep `hyperedges` updated
  per-member.
- `remove_node` / entity removal — drop `Pointee::EntityId(removed)`
  bucket; also walk *every* `Pointee::Path { entity: removed, .. }`
  bucket in the index and remove their referenced edges /
  hyperedge-memberships. (Path lookups by entity → use a secondary
  index `entity_to_path_pointees: HashMap<EntityId, HashSet<Path>>`,
  otherwise this stays O(N).)
- `apply ObjectDelta` that deletes / retypes a field — for each
  surviving `Pointee::Path` whose path now fails
  `is_pointee_exist`, cascade-remove the referencing edges and
  hyperedge-memberships.

### Open questions

- Should the index distinguish `edges_as_source` vs
  `edges_as_target`? It costs an extra bucket but makes
  "find edges *into* X" and "find edges *out of* X" both O(1).
  Without the split they become O(E_X) — fine in most cases.
- Hyperedge cascade when membership becomes empty: the
  hyperedge itself is deleted, which must in turn cascade to
  edges that have *that* hyperedge as endpoint. The index has
  to handle the recursion without infinite loops.
- For `Pointee::Path` keys, the key includes the full local path.
  If we delete an *intermediate* field, every path that has that
  field as a prefix should be invalidated — but they sit in
  separate buckets. Either walk all keys with matching entity
  and check prefix, or maintain a trie-shaped secondary index.
