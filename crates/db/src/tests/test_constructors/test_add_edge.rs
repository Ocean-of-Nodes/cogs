use super::*;

/// Adding a basic edge stores its (source, target) pair.
#[test]
fn add_basic_edge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let e1 = g.add_edge(n1, n2).unwrap();
    assert_eq!(
        g.edge(&e1).unwrap(),
        Triplet {
            id: e1,
            source: Pointee::EntityId(n1),
            target: Pointee::EntityId(n2),
        }
    )
}

/// Self-loop: an edge with both endpoints equal is allowed.
#[test]
fn allows_self_loop() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());

    let e1 = g.add_edge(n1, n1).unwrap();
    assert_eq!(
        g.edge(&e1).unwrap(),
        Triplet {
            id: e1,
            source: Pointee::EntityId(n1),
            target: Pointee::EntityId(n1),
        }
    )
}

/// Both endpoints unresolved → MissingEndpoints with both ids.
#[test]
fn rejects_both_endpoints_missing() {
    let mut g = Graph::default();
    let n1 = Uuid::new_v4();
    let n2 = Uuid::new_v4();

    let err = g.add_edge(n1, n2).unwrap_err();
    assert_eq!(
        err,
        AddEdgeError::MissingEndpoints(MissingEndpointsError {
            missing_endpoints: vec![Pointee::EntityId(n1), Pointee::EntityId(n2)],
        })
    )
}

/// Re-inserting an edge under the same id is rejected.
#[test]
fn rejects_duplicate_edge_id() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let e1 = g.add_edge(n1, n2).unwrap();
    let err = g
        .silent_add_edge_with_id(e1, n1.into(), n2.into())
        .unwrap_err();
    assert_eq!(
        err,
        AddEdgeError::EdgeAlreadyExists(EdgeAlreadyExistsError { id: e1 })
    )
}
