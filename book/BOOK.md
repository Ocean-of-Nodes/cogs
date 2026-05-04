# Data Model

COG implements its own data model.

## Nodes

A node is a container for an object.

![Object](obj.png)

Object fields are dynamically typed: string, number, bool, null, or link. A link is a reference to another entity — node, edge, hyperedge, or a sub-object inside another field.

A node can be *free* — having no incoming or outgoing edges and not being a member of any hyperedge.

## Edges

An edge is a directed link between any two entities — nodes, edges, or hyperedges — so any combination of endpoints is allowed, including self-loops.

An edge whose at least one endpoint is itself an edge or hyperedge is called a *metaedge*. Metaedge is a classification, not a separate storage type: every metaedge is just an edge.

## Hyperedges

A hyperedge groups an arbitrary number of entities without imposing direction or pairing.

## Attached objects

An edge or hyperedge can carry an object of its own, attached on top. This lets structural elements hold data without introducing a proxy node.

## Invariants

- *Endpoints exist.* Every edge has both endpoints alive. Deleting an entity cascades to all edges that have it as source or target, and recursively to metaedges that depended on those edges.
- *Hyperedge membership.* When a member of a hyperedge is deleted, it is removed from the hyperedge. A hyperedge that becomes empty is itself deleted (which in turn cascades to any edges that have it as source or target).

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