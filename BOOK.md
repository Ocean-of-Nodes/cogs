# Data Model

COG's implements its own data model. The first is nodes that represent objects. Nodes can be free (having no incoming/outgoing edges). Edges must connect two nodes. The edge-to-nowhere invariant is not allowed, meaning that when a node is deleted, the edges for which it is an endpoint will be deleted. But in addition, the database supports meta edges (edges that connect two other edges or an edge and a node or space and node\edge\space). It is worth talking separately about spaces. Graphs are called spaces. In essence, a graph is a hyperedge.

# Reducer bestiary

In traditional databases, views are created using schemas and modified by queries. COGs use functions to create views.

You use macro `view` for marking function. That's create new a materialized view (by differential data flow). That is, this is the function fn(g1, g2, ...) -> gr, that's accept some spaces.

```rust
    #[view]
    #[result = "/some_path/"]
    fn my_new_view(#[path = "/graph1"] g1: Graph, #[path = "/graph1"] g2: Graph) -> Graph {

    }
```

You can path singl graph parameter thats mean that root (whole graph) be used as parameter. 

Also function can chang graph without return new `view` thats function call `mutator`.

Data base can also contain `procedure` it interface the same as `view` function. The different that procedure doesnt recall by observed (captured) space changes. Insted this function can be called by client by three different way: fist snapshot based - function call once and return result; second - update by recall, the first time you call it, you get a snapshot, later, when you call it again, you get delta patches; third - observation, the first time you call it, you receive a snapshot, then, with each change in the database, it automatically sends you a delta patch.