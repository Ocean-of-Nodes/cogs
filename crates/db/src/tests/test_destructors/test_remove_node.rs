use super::*;

/// `remove_node` rejects an unknown id.
#[test]
fn unknown_id() {
    let mut g = Graph::default();
    let err = g.remove_node(&Uuid::new_v4()).unwrap_err();
    assert!(matches!(err, NodeNotFoundError { .. }));
}

/// Removing a node returns its attached object as Field::Object.
#[test]
fn returns_object() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let returned = g.remove_node(&n1).unwrap();
    assert_eq!(returned, obj);
    assert!(!g.is_exist(&n1));
    test_utils::check_index_invariant(&g);
}

/// Removing `n1` cascades to `e: n1 → n2`.
/// `n2` survives but its target bucket no longer contains `e`.
#[test]
fn cascades_direct_edge_reference() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);
    let e = g.add_edge(n1, n2).unwrap();

    g.remove_node(&n1).unwrap();

    assert!(!g.is_exist(&n1));
    assert!(!g.edges.contains_key(&e));
    assert!(g.is_exist(&n2));
    test_utils::check_index_invariant(&g);
}

/// Removing `n2` cascades to `e: n1 → n2/field` (Path-pointee).
/// Verifies the `entity_to_path_pointees` lookup path.
#[test]
fn cascades_path_reference() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
    let e = g.add_edge(n1, path.clone()).unwrap();

    g.remove_node(&n2).unwrap();

    assert!(!g.is_exist(&n2));
    assert!(!g.edges.contains_key(&e));
    assert!(g.is_exist(&n1));
    assert!(!g.entity_to_path_pointees.contains_key(&n2));
    test_utils::check_index_invariant(&g);
}

/// Self-loop `e: n1 → n1`: index is fully drained on removal.
#[test]
fn cascades_self_loop() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj);
    let e = g.add_edge(n1, n1).unwrap();

    g.remove_node(&n1).unwrap();

    assert!(!g.edges.contains_key(&e));
    assert!(g.pointee_uses.is_empty());
    assert!(g.entity_to_path_pointees.is_empty());
    test_utils::check_index_invariant(&g);
}

/// Chain: `e1: n1 → n2`, `e2: n3 → e1`. Removing `n1` must
/// cascade to `e1` and then to `e2` (since `e1` is `e2`'s target).
#[test]
fn cascades_through_meta_edge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);

    let e1 = g.add_edge(n1, n2).unwrap();
    let e2 = g.add_edge(n3, e1).unwrap();

    g.remove_node(&n1).unwrap();

    assert!(!g.edges.contains_key(&e1));
    assert!(!g.edges.contains_key(&e2));
    assert!(g.is_exist(&n2));
    assert!(g.is_exist(&n3));
    test_utils::check_index_invariant(&g);
}

/// Hyperedge with two members loses one — survives with the other.
#[test]
fn hyperedge_loses_member_but_survives() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut m = HashSet::new();
    m.insert(n1.into());
    m.insert(n2.into());
    let h = g.create_hyperedge(m).unwrap();

    g.remove_node(&n1).unwrap();

    let mut expected = HashSet::new();
    expected.insert(n2.into());
    assert_eq!(g.hyperedge_members(&h), Some(&expected));
    test_utils::check_index_invariant(&g);
}

/// Hyperedge with the only member `n1` becomes empty when `n1` dies
/// and is itself cascade-removed.
#[test]
fn hyperedge_empties_and_dies() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj);

    let mut m = HashSet::new();
    m.insert(n1.into());
    let h = g.create_hyperedge(m).unwrap();

    g.remove_node(&n1).unwrap();

    assert!(!g.hyperedges.contains_key(&h));
    test_utils::check_index_invariant(&g);
}

/// Cascade reaches edges that pointed at a hyperedge that
/// itself died from emptying.
#[test]
fn cascade_through_dead_hyperedge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut m = HashSet::new();
    m.insert(n1.into());
    let h = g.create_hyperedge(m).unwrap();
    // edge from n2 to the soon-to-die hyperedge
    let e = g.add_edge(n2, h).unwrap();

    g.remove_node(&n1).unwrap();

    assert!(!g.hyperedges.contains_key(&h));
    assert!(!g.edges.contains_key(&e));
    assert!(g.is_exist(&n2));
    test_utils::check_index_invariant(&g);
}

/// `remove_node` rejects ids that live in `entities` because of
/// `attach_obj` on an edge — those are NOT nodes.
#[test]
fn rejects_attached_object_id() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let e = g.add_edge(n1, n2).unwrap();
    g.attach_obj(e, obj).unwrap();

    let err = g.remove_node(&e).unwrap_err();
    assert!(matches!(err, NodeNotFoundError { .. }));
    // Edge itself untouched.
    assert!(g.edges.contains_key(&e));
    test_utils::check_index_invariant(&g);
}

/// Records exactly one `Patch::RemoveNode` even on a cascade.
#[test]
fn records_single_patch_on_cascade() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);
    let _e = g.add_edge(n1, n2).unwrap();

    g.remove_node(&n1).unwrap();

    let last = g.events.last().unwrap();
    assert_eq!(*last, Patch::RemoveNode { id: n1 });
    let remove_count = g
        .events
        .iter()
        .filter(|p| matches!(p, Patch::RemoveNode { .. }))
        .count();
    assert_eq!(remove_count, 1);
}
