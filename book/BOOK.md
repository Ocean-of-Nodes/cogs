# Data Model

COG implements its own data model with node, edges and hyperedges.

## Nodes

A node is a container for an object.

![Object](obj.png)

Object fields are dynamically typed: string, number, bool, null, link or subobject. A link is a reference to another entity — node, edge, hyperedge, or a sub-object inside another field.

A node can be *free* — having no incoming or outgoing edges and not being a member of any hyperedge.

### Subobjects

![Subobject](subobj.png)

A *subobject* is a field inside an entity's `Object` whose value is itself an `Object` (or any nested field reachable through a chain of object-typed fields). A subobject has **no identity of its own**: it doesn't appear in `iter_entities`, has no UUID, and is addressed by a `GlobalPath` of the form `<entity-uuid>/<field>[/<field>...]`.

Subobjects are not entities. They cannot:
- be classified by `get_type` (which is for ids);
- carry an *attached object* (see below);
- be the target of `attach_obj`.

Subobjects **can** be:
- the source or target of an edge,
- a member of a hyperedge,
- the target of a `Field::Link` (i.e. `Pointee::Path`).

## Edges

An edge is a directed link between any two entities — nodes, suboject, edges, or hyperedges — so any combination of endpoints is allowed, including self-loops.

An edge whose at least one endpoint is itself an edge or hyperedge is called a *metaedge*. Metaedge is a classification, not a separate storage type: every metaedge is just an edge.

## Hyperedges

![Hyperedge](hyperedge.png)

A hyperedge groups an arbitrary number of pointees (entities or subobjects) without imposing direction or pairing. A hyperedge is essentially a subgraph or what will later be called a `space`.

## Attached objects

An *attached object* is an `Object` that piggy-backs on top of an edge or hyperedge, sharing its id. It lets structural elements (edges, hyperedges) carry data without introducing a proxy node.

## Invariants

- *Endpoints exist.* Every edge has both endpoints alive. Deleting an entity cascades to all edges that have it as source or target, and recursively to metaedges that depended on those edges.
- *Hyperedge membership.* When a member of a hyperedge is deleted, it is removed from the hyperedge. A hyperedge that becomes empty is itself deleted (which in turn cascades to any edges that have it as source or target).

# Coding model

So, how to interact with the database using code and organize it?

## Dialect and not query DSL

Instead of creating a query language, COG's proposes a Rust dialect. When we say dialect, we mean that the Rust side provides intrinsics and macros that will be processed by the JIT.

## DB stored code

### For what?

The primary way to build COG applications is to place code within the database. This aligns with DAD: firstly, we strive to maintain code and data locality, and secondly, we want to inspect the code just-in-time (JIT) for subsequent optimizations.

### Your first module

Ok, lets create new folder `hellow-view`.

Next, create `Cargo.toml` with following code:

```toml
[package]
name = "hellow-view"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]
```

Ok, next create file `src/lib.rs` with following content:

```rust
    #[view]
    fn hellow_view(main: &mut Graph) {
        let mut obj = Object::new();
        obj.insert("Hello", Field::String("World".string()));
        
        main.add_node(obj.clone());
    }
```

### 
In traditional databases, views are created using schemas and modified by queries. COGs use functions to create views.

You use macro `view` for marking function. That's create new a incrimental materialized view. That is, this is the function fn(g1, g2, ...) -> gr, that's accept some spaces.

```rust
    #[view]
    fn hellow_view(#[path = "/graph1"] g1: Graph, #[path = "/graph1"] g2: Graph) -> Graph {

    }
```

You can path singl graph parameter thats mean that root (whole graph) be used as parameter. 

Also function can chang graph without return new `view` thats function call `mutator`.

Data base can also contain `procedure` it interface the same as `view` function. The different that procedure doesnt recall by observed (captured) space changes. Insted this function can be called by client by three different way: fist snapshot based - function call once and return result; second - update by recall, the first time you call it, you get a snapshot, later, when you call it again, you get delta patches; third - observation, the first time you call it, you receive a snapshot, then, with each change in the database, it automatically sends you a delta patch.

# Queries

