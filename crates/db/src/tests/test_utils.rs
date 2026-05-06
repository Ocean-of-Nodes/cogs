//! Shared test helpers: small object builders, sample-graph
//! factories, and the cross-index invariant checker that every
//! mutating-op test should call after the operation under test.

use super::*;

pub fn create_simple_obj(field_name: &str) -> Object {
    let mut obj = Object::new();
    obj.insert(field_name.into(), Field::Null);
    obj
}

/// Cross-check `pointee_uses` and `entity_to_path_pointees`
/// against `edges` / `hyper_edge`. Panics on the first
/// inconsistency. Call after any mutation to assert the
/// reverse-index invariants are intact.
pub fn check_index_invariant(g: &Graph) {
    // 1) Every Pointee::Path key in pointee_uses must be
    //    tracked in entity_to_path_pointees under its entity.
    for key in g.pointee_uses.keys() {
        if let Pointee::Path(gp) = key {
            let tracked = g
                .entity_to_path_pointees
                .get(&gp.entity())
                .is_some_and(|s| s.contains(key));
            assert!(
                tracked,
                "path pointee {:?} present in pointee_uses but missing \
                 from entity_to_path_pointees",
                key
            );
        }
    }

    // 2) entity_to_path_pointees has no stale or empty entries.
    for (entity, paths) in &g.entity_to_path_pointees {
        assert!(
            !paths.is_empty(),
            "entity_to_path_pointees[{}] is empty (should have been removed)",
            entity
        );
        for p in paths {
            assert!(
                g.pointee_uses.contains_key(p),
                "stale entry in entity_to_path_pointees[{}]: {:?} not in pointee_uses",
                entity,
                p
            );
        }
    }

    // 3) Every edge has both endpoints registered in pointee_uses.
    for (eid, (src, tgt)) in &g.edges {
        let src_ok = g
            .pointee_uses
            .get(src)
            .is_some_and(|b| b.edges_as_source.contains(eid));
        assert!(src_ok, "edge {} not registered in source bucket {:?}", eid, src);
        let tgt_ok = g
            .pointee_uses
            .get(tgt)
            .is_some_and(|b| b.edges_as_target.contains(eid));
        assert!(tgt_ok, "edge {} not registered in target bucket {:?}", eid, tgt);
    }

    // 4) Every hyperedge member has the hyperedge registered.
    for (hid, members) in &g.hyper_edge {
        for m in members {
            let ok = g
                .pointee_uses
                .get(m)
                .is_some_and(|b| b.hyperedges.contains(hid));
            assert!(ok, "hyperedge {} not registered in member bucket {:?}", hid, m);
        }
    }

    // 5) No empty buckets — they must have been removed.
    for (k, b) in &g.pointee_uses {
        assert!(
            !b.is_empty(),
            "pointee_uses[{:?}] is empty (should have been removed)",
            k
        );
    }

    // 6) Reverse direction: every (eid, source/target/hyperedge)
    //    in pointee_uses corresponds to a live structural element.
    for (pointee, uses) in &g.pointee_uses {
        for eid in &uses.edges_as_source {
            let edge = g.edges.get(eid);
            assert!(
                edge.is_some_and(|(s, _)| s == pointee),
                "pointee_uses[{:?}].edges_as_source has stale eid {}",
                pointee,
                eid
            );
        }
        for eid in &uses.edges_as_target {
            let edge = g.edges.get(eid);
            assert!(
                edge.is_some_and(|(_, t)| t == pointee),
                "pointee_uses[{:?}].edges_as_target has stale eid {}",
                pointee,
                eid
            );
        }
        for hid in &uses.hyperedges {
            let members = g.hyper_edge.get(hid);
            assert!(
                members.is_some_and(|ms| ms.contains(pointee)),
                "pointee_uses[{:?}].hyperedges has stale hid {}",
                pointee,
                hid
            );
        }
    }
}

// Built graph:
// ```text
//  ---------
//  | n1 ---|----e1---n2
//  |       |    |    |
//  |       |    |    |
//  |       |    e3---e4-----
//  |       |    |          |
//  | n3 ---|---e2----n4    |
//  --h------               |
//    |                     |
//    |----------------------
// ````
pub fn create_sample_graph1() -> (
    Graph,
    NodeId,
    NodeId,
    NodeId,
    NodeId,
    EdgeId,
    EdgeId,
    EdgeId,
    EdgeId,
    EdgeId,
    HyperedgeId,
) {
    let mut g = Graph::default();
    let obj = create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj.clone());
    let n4 = g.add_node(obj.clone());

    let e1 = g.add_edge(n1, n2).unwrap();
    let e2 = g.add_edge(n3, n4).unwrap();
    let e3 = g.add_edge(e1, e2).unwrap();
    let e4 = g.add_edge(e3, n2).unwrap();

    let mut m = HashSet::new();
    m.insert(n1.into());
    m.insert(n3.into());

    let h = g.create_hyperedge(m).unwrap();
    let e5 = g.add_edge(h, e4).unwrap();

    (g, n1, n2, n3, n4, e1, e2, e3, e4, e5, h)
}

// Built graph:
// ```text
//  n1 ---- e_a ----- n2
//  |\
//  |  ----------------
//  |                 |
//  edge_to_h     meta_edge
//  |                 |
//  -------           |     --------
//  |  n3-|----------e_b----|---n4 |
//  |     |-----------------|      |
//  |                              |
//  |-----------h------------------|
// ```
pub fn create_sample_graph2() -> (
    Graph,
    NodeId,
    NodeId,
    NodeId,
    NodeId,
    EdgeId,
    EdgeId,
    EdgeId,
    EdgeId,
    HyperedgeId,
) {
    let mut graph = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");

    let n1 = graph.add_node(obj.clone());
    let n2 = graph.add_node(obj.clone());
    let n3 = graph.add_node(obj.clone());
    let n4 = graph.add_node(obj.clone());

    let e_a = graph.add_edge(n1, n2).unwrap();
    let e_b = graph.add_edge(n3, n4).unwrap();
    let meta_edge = graph.add_edge(n1, e_b).unwrap();

    let mut m = HashSet::new();
    m.insert(n3.into());
    m.insert(n4.into());

    let h = graph.create_hyperedge(m).unwrap();
    let edge_to_h = graph.add_edge(n1, h).unwrap();

    (graph, n1, n2, n3, n4, e_a, e_b, meta_edge, edge_to_h, h)
}

/// Built graph (note: `e2` is intentionally directed `n3 → n1`,
/// not `n1 → n3`):
///
/// ```text
///
///  n1 ----------- e1 ---------- n2
///   ^                          / |
///    \         /----- e3 -----   |
///     -- e2 - n3 -------- e4 ----
/// ```
pub fn create_sample_graph3() -> (
    Graph,
    NodeId,
    NodeId,
    NodeId,
    EdgeId,
    EdgeId,
    EdgeId,
    EdgeId,
) {
    let mut graph = Graph::default();
    let obj = test_utils::create_simple_obj("attached");

    let n1 = graph.add_node(obj.clone());
    let n2 = graph.add_node(obj.clone());
    let n3 = graph.add_node(obj.clone());

    let e1 = graph.add_edge(n1, n2).unwrap();
    let e2 = graph.add_edge(n3, n1).unwrap(); // reversed on purpose
    let e3 = graph.add_edge(n3, n2).unwrap();
    let e4 = graph.add_edge(n3, n2).unwrap(); // parallel to e3

    (graph, n1, n2, n3, e1, e2, e3, e4)
}
