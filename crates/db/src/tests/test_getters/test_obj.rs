use super::*;

// Test all kind of object holder: an object can be
// attached to a regular edge, a meta-edge, and an
// edge-to-hyperedge alike. Hitting all three structural
// shapes here exercises every `is_attach_target() == true`
// branch of `EntityType`.
//
// n1 is used to verify that nodes — the "default" holder
// (object goes in via `add_node`) — keep their object
// unchanged after these attaches.
#[test]
fn test_obj() {
    let (mut graph, n1, _n2, _n3, _n4, e_a, _e_b, meta_edge, edge_to_h, _h) =
        test_utils::create_sample_graph2();

    let obj = test_utils::create_simple_obj("attached");

    graph.attach_obj(e_a, obj.clone()).unwrap();
    graph.attach_obj(meta_edge, obj.clone()).unwrap();
    graph.attach_obj(edge_to_h, obj.clone()).unwrap();

    assert_eq!(graph.obj(&e_a), Some(&obj));
    assert_eq!(graph.obj(&meta_edge), Some(&obj));
    assert_eq!(graph.obj(&edge_to_h), Some(&obj));

    // n1 is a Node: its object came from `add_node`,
    // independent of any attach. It must still be there.
    assert!(graph.obj(&n1).is_some());
}

/// Unknown id resolves to `None`.
#[test]
fn obj_unknown() {
    let g = Graph::default();
    assert!(g.obj(&Uuid::new_v4()).is_none());
}

/// A bare edge (no `attach_obj` call) has no object.
#[test]
fn obj_bare_edge_is_none() {
    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();
    assert!(g.obj(&e_a).is_none());
}

/// A bare hyperedge has no object.
#[test]
fn obj_bare_hyperedge_is_none() {
    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
        test_utils::create_sample_graph2();
    assert!(g.obj(&h).is_none());
}
