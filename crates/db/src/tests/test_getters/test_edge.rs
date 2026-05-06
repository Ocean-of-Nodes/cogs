use super::*;

/// Regular node-to-node edge round-trips through `edge`.
#[test]
fn edge1() {
    let (graph, n1, n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();

    let u = graph.edge(&e_a).unwrap();
    assert_eq!(
        u,
        Triplet {
            id: e_a,
            source: n1.into(),
            target: n2.into()
        }
    )
}

/// Meta-edge: target is another edge — still in the
/// `edges` map, so `edge` returns its triplet.
#[test]
fn edge2() {
    let (graph, n1, _n2, _n3, _n4, _e_a, e_b, meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();

    let u = graph.edge(&meta_edge).unwrap();
    assert_eq!(
        u,
        Triplet {
            id: meta_edge,
            source: n1.into(),
            target: e_b.into(),
        }
    )
}

/// Edge whose target is a hyperedge — also lives in the
/// `edges` map.
#[test]
fn edge3() {
    let (graph, n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, edge_to_h, h) =
        test_utils::create_sample_graph2();

    let u = graph.edge(&edge_to_h).unwrap();
    assert_eq!(
        u,
        Triplet {
            id: edge_to_h,
            source: n1.into(),
            target: h.into(),
        }
    )
}

/// Self-loop is a valid edge.
#[test]
fn edge4() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj);
    let e1 = g.add_edge(n1, n1).unwrap();

    let u = g.edge(&e1).unwrap();
    assert_eq!(
        u,
        Triplet {
            id: e1,
            source: n1.into(),
            target: n1.into(),
        }
    )
}

/// Endpoints can be sub-object paths; `edge` returns
/// them verbatim.
#[test]
fn edge5() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let p1 = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
    let p2 = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
    let e1 = g.add_edge(p1.clone(), p2.clone()).unwrap();

    let u = g.edge(&e1).unwrap();
    assert_eq!(
        u,
        Triplet {
            id: e1,
            source: p1,
            target: p2,
        }
    )
}

/// Unknown id → `NotFound`.
#[test]
fn edge_not_found() {
    let g = Graph::default();
    let id = Uuid::new_v4();
    let err = g.edge(&id).unwrap_err();
    assert!(matches!(
        err,
        GetEdgeError::NotFound(EntityNotFoundError { id: x }) if x == id
    ));
}

/// A node id is a known entity but not an edge →
/// `IncorrectType("Node")`.
#[test]
fn edge_incorrect_type_node() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj);

    let err = g.edge(&n1).unwrap_err();
    match err {
        GetEdgeError::IncorrectType(e) => {
            assert_eq!(e.entity_id, n1);
            assert_eq!(e.actual_type, "Node");
        }
        other => panic!("expected IncorrectType, got {other:?}"),
    }
}

/// A hyperedge id → `IncorrectType("HyperEdge")`.
#[test]
fn edge_incorrect_type_hyperedge() {
    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
        test_utils::create_sample_graph2();

    let err = g.edge(&h).unwrap_err();
    match err {
        GetEdgeError::IncorrectType(e) => {
            assert_eq!(e.entity_id, h);
            assert_eq!(e.actual_type, "HyperEdge");
        }
        other => panic!("expected IncorrectType, got {other:?}"),
    }
}

/// An attached-object id (object placed on top of a
/// hyperedge) → `IncorrectType("AttachedObject")`. The
/// hyperedge map lookup wins over the edges map only
/// because attaching to a hyperedge keeps the id outside
/// `edges`.
#[test]
fn edge_incorrect_type_attached() {
    let (mut g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
        test_utils::create_sample_graph2();
    let obj = test_utils::create_simple_obj("attached");
    g.attach_obj(h, obj).unwrap();

    let err = g.edge(&h).unwrap_err();
    match err {
        GetEdgeError::IncorrectType(e) => {
            assert_eq!(e.entity_id, h);
            assert_eq!(e.actual_type, "AttachedObject");
        }
        other => panic!("expected IncorrectType, got {other:?}"),
    }
}
