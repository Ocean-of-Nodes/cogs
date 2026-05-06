use super::*;

/// Replay the recorded events on a fresh graph and assert
/// structural equality with the original.
fn assert_replay_matches(original: &Graph) {
    let mut replayed = Graph::default();
    replayed
        .apply_delta(original.events.clone())
        .expect("replay must succeed");
    test_utils::check_index_invariant(&replayed);
    assert_eq!(original.entities, replayed.entities, "entities mismatch");
    assert_eq!(original.edges, replayed.edges, "edges mismatch");
    assert_eq!(
        original.hyper_edge, replayed.hyper_edge,
        "hyper_edge mismatch"
    );
}

#[test]
fn nodes_and_edges() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);
    g.add_edge(n1, n2).unwrap();
    g.add_edge(n3, n1).unwrap();

    assert_replay_matches(&g);
}

#[test]
fn hyperedge_lifecycle() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);

    let mut m = HashSet::new();
    m.insert(n1.into());
    m.insert(n2.into());
    let h = g.create_hyperedge(m).unwrap();

    let mut to_add = HashSet::new();
    to_add.insert(n3.into());
    g.add_hyperedge_members(h, to_add).unwrap();

    let mut to_remove = HashSet::new();
    to_remove.insert(n1.into());
    g.remove_hyperedge_members(h, to_remove).unwrap();

    assert_replay_matches(&g);
}

#[test]
fn attach_then_replace_attached() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let e = g.add_edge(n1, n2).unwrap();
    g.attach_obj(e, obj.clone()).unwrap();
    // Replace with same key but different value → small delta path.
    let mut new = Object::new();
    new.insert("f".into(), Field::Number(42));
    g.replace_attached_obj(&e, new).unwrap();

    assert_replay_matches(&g);
}

#[test]
fn replace_node_change_path() {
    let mut g = Graph::default();
    let mut o1 = Object::new();
    o1.insert("a".into(), Field::Number(1));
    o1.insert("b".into(), Field::Number(2));
    let n1 = g.add_node(o1);

    let mut o2 = Object::new();
    o2.insert("a".into(), Field::Number(1));
    o2.insert("b".into(), Field::Number(99)); // small delta
    g.replace_node(&n1, o2).unwrap();

    assert_replay_matches(&g);
}

#[test]
fn replace_node_upsert_path() {
    let mut g = Graph::default();
    let mut o1 = Object::new();
    o1.insert("a".into(), Field::Number(1));
    let n1 = g.add_node(o1);

    // 2 ops (Remove a, Add x) > 1 field → Upsert path.
    let mut o2 = Object::new();
    o2.insert("x".into(), Field::Number(99));
    g.replace_node(&n1, o2).unwrap();

    assert_replay_matches(&g);
}

#[test]
fn retarget_edge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);
    let e = g.add_edge(n1, n2).unwrap();
    g.retarget_edge(&e, RetargetEdge::Target(n3.into())).unwrap();

    assert_replay_matches(&g);
}

#[test]
fn remove_node_cascade_replays() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);
    g.add_edge(n1, n2).unwrap();
    g.add_edge(n3, n1).unwrap();
    g.remove_node(&n1).unwrap();

    assert_replay_matches(&g);
}

/// A patch whose precondition is violated propagates as an error.
#[test]
fn missing_precondition_errors() {
    let mut g = Graph::default();
    let err = g
        .apply_delta(vec![Patch::RemoveNode { id: Uuid::new_v4() }])
        .unwrap_err();
    assert!(matches!(err, ApplyPatchError::NodeNotFound(_)));
}
