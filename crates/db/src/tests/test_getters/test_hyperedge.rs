use super::*;

#[test]
fn hyperedge1() {
    let (graph, _n1, _n2, n3, n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
        test_utils::create_sample_graph2();

    let members = graph.hyperedge_members(&h).unwrap();
    let expected: HashSet<Pointee> = [n3.into(), n4.into()].into_iter().collect();
    assert_eq!(members, &expected);
}

/// Unknown id → None.
#[test]
fn hyperedge_unknown() {
    let g = Graph::default();
    assert!(g.hyperedge_members(&Uuid::new_v4()).is_none());
}

/// An edge id is not a hyperedge → None.
#[test]
fn hyperedge_for_edge_id() {
    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();
    assert!(g.hyperedge_members(&e_a).is_none());
}

/// `hyperedge_members` returns `None` for an unknown id.
#[test]
fn hyperedge_members_unknown() {
    let g = Graph::default();
    assert!(g.hyperedge_members(&Uuid::new_v4()).is_none());
}
