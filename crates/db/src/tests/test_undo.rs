use super::*;

type Snapshot = (
    HashMap<EntityId, Object>,
    HashMap<EdgeId, (Pointee, Pointee)>,
    HashMap<HyperedgeId, HashSet<Pointee>>,
);

fn snapshot(g: &Graph) -> Snapshot {
    (g.entities.clone(), g.edges.clone(), g.hyperedges.clone())
}

/// Run `op`, undo it, and assert the graph is back to where it was.
fn assert_undo_restores(g: &mut Graph, op: impl FnOnce(&mut Graph)) {
    let before = snapshot(g);
    let events_before = g.events.len();

    op(g);
    assert_eq!(g.events.len(), events_before + 1, "op must log one event");

    g.undo().expect("undo must succeed").expect("log is not empty");
    test_utils::check_index_invariant(g);
    assert_eq!(snapshot(g), before);
    assert_eq!(g.events.len(), events_before);
}

#[test]
fn undo_on_empty_log() {
    let mut g = Graph::default();
    assert!(g.undo().unwrap().is_none());
}

#[test]
fn undo_add_node() {
    let mut g = Graph::default();
    assert_undo_restores(&mut g, |g| {
        g.add_node(test_utils::create_simple_obj("f"));
    });
}

#[test]
fn undo_remove_node_with_cascade() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);

    // Survives, but loses n1.
    g.create_hyperedge(HashSet::from([n1.into(), n2.into()]))
        .unwrap();
    // Dies with n1, and takes everything pointing at it.
    let h = g.create_hyperedge(HashSet::from([n1.into()])).unwrap();
    g.attach_obj(h, test_utils::create_simple_obj("data")).unwrap();
    let e1 = g.add_edge(h, n2).unwrap();
    g.add_edge(e1, n3).unwrap();
    // Edge into h that is drained before h itself dies.
    g.add_edge(n1, h).unwrap();
    g.add_edge(n2, n1).unwrap();

    assert_undo_restores(&mut g, |g| {
        g.remove_node(&n1).unwrap();
    });
}

#[test]
fn undo_replace_node_restores_dead_paths() {
    let mut g = Graph::default();
    let n1 = g.add_node(test_utils::create_simple_obj("data"));
    let n2 = g.add_node(test_utils::create_simple_obj("f"));
    let path = Pointee::Path(GlobalObjPath::new(n1, "data").unwrap());
    g.add_edge(path.clone(), n2).unwrap();
    g.create_hyperedge(HashSet::from([path])).unwrap();

    // `data` disappears — every path through it dies.
    assert_undo_restores(&mut g, |g| {
        g.replace_node(&n1, test_utils::create_simple_obj("other"))
            .unwrap();
    });
}

#[test]
fn undo_add_and_remove_edge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    assert_undo_restores(&mut g, |g| {
        g.add_edge(n1, n2).unwrap();
    });

    let e1 = g.add_edge(n1, n2).unwrap();
    g.attach_obj(e1, test_utils::create_simple_obj("data")).unwrap();
    g.add_edge(e1, n2).unwrap();
    assert_undo_restores(&mut g, |g| {
        g.remove_edge(&e1).unwrap();
    });
}

#[test]
fn undo_retarget_edge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);
    let e = g.add_edge(n1, n2).unwrap();

    assert_undo_restores(&mut g, |g| {
        g.retarget_edge(&e, RetargetEdge::Source(n3.into())).unwrap();
    });
    assert_undo_restores(&mut g, |g| {
        g.retarget_edge(&e, RetargetEdge::Target(n3.into())).unwrap();
    });
}

#[test]
fn undo_attached_object_ops() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);
    let e = g.add_edge(n1, n2).unwrap();
    let h = g.create_hyperedge(HashSet::from([n1.into()])).unwrap();

    for target in [e, h] {
        assert_undo_restores(&mut g, |g| {
            g.attach_obj(target, test_utils::create_simple_obj("data"))
                .unwrap();
        });

        g.attach_obj(target, test_utils::create_simple_obj("data"))
            .unwrap();
        let path = Pointee::Path(GlobalObjPath::new(target, "data").unwrap());
        g.add_edge(path, n2).unwrap();

        assert_undo_restores(&mut g, |g| {
            g.replace_attached_obj(&target, test_utils::create_simple_obj("other"))
                .unwrap();
        });
        assert_undo_restores(&mut g, |g| {
            g.remove_attached(target).unwrap();
        });
    }
}

#[test]
fn undo_hyperedge_ops() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    assert_undo_restores(&mut g, |g| {
        g.create_hyperedge(HashSet::from([n1.into()])).unwrap();
    });

    let h = g.create_hyperedge(HashSet::from([n1.into()])).unwrap();
    assert_undo_restores(&mut g, |g| {
        g.add_hyperedge_members(h, HashSet::from([n2.into()]))
            .unwrap();
    });

    g.attach_obj(h, test_utils::create_simple_obj("data")).unwrap();
    g.add_edge(h, n2).unwrap();
    assert_undo_restores(&mut g, |g| {
        g.remove_hyperedge(&h).unwrap();
    });
}

#[test]
fn undo_remove_members_that_empties_hyperedge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);
    let h = g
        .create_hyperedge(HashSet::from([n1.into(), n2.into()]))
        .unwrap();
    g.attach_obj(h, test_utils::create_simple_obj("data")).unwrap();
    g.add_edge(h, n2).unwrap();

    assert_undo_restores(&mut g, |g| {
        g.remove_hyperedge_members(h, HashSet::from([n2.into()]))
            .unwrap();
    });
    assert_undo_restores(&mut g, |g| {
        g.remove_hyperedge_members(h, HashSet::from([n1.into(), n2.into()]))
            .unwrap();
    });
}

#[test]
fn undo_whole_log_empties_graph() {
    let (mut g, ..) = test_utils::create_sample_graph1();
    while g.undo().unwrap().is_some() {}
    test_utils::check_index_invariant(&g);
    assert_eq!(snapshot(&g), snapshot(&Graph::default()));
}
