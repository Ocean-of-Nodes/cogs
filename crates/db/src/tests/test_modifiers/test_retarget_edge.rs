use super::*;

/// Unknown edge id is rejected.
#[test]
fn unknown_edge() {
    let mut g = Graph::default();
    let n1 = g.add_node(test_utils::create_simple_obj("f"));
    let err = g
        .retarget_edge(&Uuid::new_v4(), RetargetEdge::Source(n1.into()))
        .unwrap_err();
    assert!(matches!(err, RetargetError::EdgeNotFound(_)));
}

/// New endpoint must resolve in the graph.
#[test]
fn invalid_target_pointee() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);
    let e = g.add_edge(n1, n2).unwrap();

    let err = g
        .retarget_edge(&e, RetargetEdge::Target(Pointee::EntityId(Uuid::new_v4())))
        .unwrap_err();
    assert!(matches!(err, RetargetError::InvalidTarget(_)));
    // Edge is unchanged.
    assert_eq!(
        g.edges.get(&e),
        Some(&(Pointee::EntityId(n1), Pointee::EntityId(n2)))
    );
    test_utils::check_index_invariant(&g);
}

/// Retarget the source: edge updated, indexes swapped.
#[test]
fn retargets_source() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);
    let e = g.add_edge(n1, n2).unwrap();

    g.retarget_edge(&e, RetargetEdge::Source(n3.into())).unwrap();

    assert_eq!(
        g.edges.get(&e),
        Some(&(Pointee::EntityId(n3), Pointee::EntityId(n2)))
    );
    // Old source bucket is now empty (n1 had only this edge).
    assert!(!g.pointee_uses.contains_key(&Pointee::EntityId(n1)));
    // New source bucket has the edge.
    assert!(g
        .pointee_uses
        .get(&Pointee::EntityId(n3))
        .is_some_and(|b| b.edges_as_source.contains(&e)));
    test_utils::check_index_invariant(&g);
}

/// Retarget the target: edge updated, indexes swapped.
#[test]
fn retargets_target() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);
    let e = g.add_edge(n1, n2).unwrap();

    g.retarget_edge(&e, RetargetEdge::Target(n3.into())).unwrap();

    assert_eq!(
        g.edges.get(&e),
        Some(&(Pointee::EntityId(n1), Pointee::EntityId(n3)))
    );
    assert!(!g.pointee_uses.contains_key(&Pointee::EntityId(n2)));
    assert!(g
        .pointee_uses
        .get(&Pointee::EntityId(n3))
        .is_some_and(|b| b.edges_as_target.contains(&e)));
    test_utils::check_index_invariant(&g);
}

/// No-op when the new endpoint equals the old one.
#[test]
fn no_op_same_endpoint() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);
    let e = g.add_edge(n1, n2).unwrap();

    g.retarget_edge(&e, RetargetEdge::Source(n1.into())).unwrap();

    assert_eq!(
        g.edges.get(&e),
        Some(&(Pointee::EntityId(n1), Pointee::EntityId(n2)))
    );
    test_utils::check_index_invariant(&g);
}

/// Retargeting to a Path-pointee tracks it in
/// `entity_to_path_pointees`; removing the last reference
/// to the old path-pointee untracks it.
#[test]
fn path_endpoints_track_and_untrack() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);

    let old_path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
    let e = g.add_edge(n1, old_path.clone()).unwrap();
    assert!(g.entity_to_path_pointees.contains_key(&n2));

    let new_path = Pointee::Path(GlobalObjPath::new(n3, "test_field").unwrap());
    g.retarget_edge(&e, RetargetEdge::Target(new_path.clone()))
        .unwrap();

    // Old path's entity untracked (was its only reference).
    assert!(!g.entity_to_path_pointees.contains_key(&n2));
    assert!(!g.pointee_uses.contains_key(&old_path));

    // New path tracked.
    assert!(g
        .entity_to_path_pointees
        .get(&n3)
        .is_some_and(|s| s.contains(&new_path)));
    assert!(g.pointee_uses.contains_key(&new_path));
    test_utils::check_index_invariant(&g);
}

/// Self-loop: retargeting source while target equals source —
/// the bucket isn't lost mid-op since the same pointee is still
/// the target.
#[test]
fn self_loop_retarget_source_preserves_target_bucket() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);
    let e = g.add_edge(n1, n1).unwrap();

    g.retarget_edge(&e, RetargetEdge::Source(n2.into())).unwrap();

    assert_eq!(
        g.edges.get(&e),
        Some(&(Pointee::EntityId(n2), Pointee::EntityId(n1)))
    );
    // n1 still tracked as target.
    assert!(g
        .pointee_uses
        .get(&Pointee::EntityId(n1))
        .is_some_and(|b| b.edges_as_target.contains(&e)));
    test_utils::check_index_invariant(&g);
}

/// Records the patch.
#[test]
fn records_patch() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);
    let e = g.add_edge(n1, n2).unwrap();

    g.retarget_edge(&e, RetargetEdge::Target(n3.into())).unwrap();

    assert_eq!(
        *g.events.last().unwrap(),
        Patch::RetargetEdge {
            id: e,
            new_target: RetargetEdge::Target(Pointee::EntityId(n3)),
        }
    );
}
