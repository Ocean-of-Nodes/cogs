use super::*;

/// Unknown id — neither node nor attach target.
#[test]
fn unknown_id() {
    let mut g = Graph::default();
    let err = g.remove_attached(Uuid::new_v4()).unwrap_err();
    assert!(matches!(err, NoAttachedObjectError { .. }));
}

/// A bare node has no attached object — removal must fail
/// rather than silently delete the node.
#[test]
fn rejects_node_id() {
    let mut g = Graph::default();
    let n1 = g.add_node(test_utils::create_simple_obj("f"));

    let err = g.remove_attached(n1).unwrap_err();
    assert!(matches!(err, NoAttachedObjectError { .. }));
    assert!(g.is_exist(&n1));
    assert!(g.entities.contains_key(&n1));
}

/// A bare edge (no attach_obj called) — nothing to remove.
#[test]
fn rejects_bare_edge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);
    let e = g.add_edge(n1, n2).unwrap();

    let err = g.remove_attached(e).unwrap_err();
    assert!(matches!(err, NoAttachedObjectError { .. }));
    assert!(g.edges.contains_key(&e));
}

/// Attached object on an edge: edge stays alive, attached
/// object gone, RemoveEdgeData patch recorded.
#[test]
fn removes_edge_attached() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let e = g.add_edge(n1, n2).unwrap();
    g.attach_obj(e, obj).unwrap();

    g.remove_attached(e).unwrap();

    assert!(g.edges.contains_key(&e));
    assert!(!g.entities.contains_key(&e));
    assert_eq!(*g.events.last().unwrap(), Patch::RemoveEdgeData { id: e });
    test_utils::check_index_invariant(&g);
}

/// Attached object on a hyperedge: hyperedge stays alive,
/// attached object gone, RemoveHyperEdgeData patch recorded.
#[test]
fn removes_hyperedge_attached() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());

    let mut m = HashSet::new();
    m.insert(n1.into());
    let h = g.create_hyperedge(m).unwrap();
    g.attach_obj(h, obj).unwrap();

    g.remove_attached(h).unwrap();

    assert!(g.hyper_edge.contains_key(&h));
    assert!(!g.entities.contains_key(&h));
    assert_eq!(
        *g.events.last().unwrap(),
        Patch::RemoveHyperedgeData { id: h }
    );
    test_utils::check_index_invariant(&g);
}

/// EntityId references survive — only Path references die.
#[test]
fn entity_id_references_survive() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let e = g.add_edge(n1, n2).unwrap();
    g.attach_obj(e, obj).unwrap();

    // Edge that points at `e` as a whole entity.
    let n3 = g.add_node(test_utils::create_simple_obj("g"));
    let meta = g.add_edge(n3, e).unwrap();

    g.remove_attached(e).unwrap();

    assert!(g.edges.contains_key(&e));
    assert!(g.edges.contains_key(&meta));
    test_utils::check_index_invariant(&g);
}

/// Path references through the attach target die.
#[test]
fn cascades_path_references() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let e = g.add_edge(n1, n2).unwrap();
    let attached = test_utils::create_simple_obj("data");
    g.attach_obj(e, attached).unwrap();

    // Edge whose endpoint is a path through `e`'s attached object.
    let n3 = g.add_node(test_utils::create_simple_obj("g"));
    let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
    let dangling = g.add_edge(n3, path).unwrap();

    g.remove_attached(e).unwrap();

    assert!(g.edges.contains_key(&e));
    assert!(!g.edges.contains_key(&dangling));
    assert!(!g.entity_to_path_pointees.contains_key(&e));
    test_utils::check_index_invariant(&g);
}

/// Records exactly one data-removal patch even when the
/// path-cascade kills downstream structures.
#[test]
fn records_single_patch_on_cascade() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let e = g.add_edge(n1, n2).unwrap();
    g.attach_obj(e, test_utils::create_simple_obj("data"))
        .unwrap();

    let n3 = g.add_node(obj);
    let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
    let _dangling = g.add_edge(n3, path).unwrap();

    g.remove_attached(e).unwrap();

    let count = g
        .events
        .iter()
        .filter(|p| {
            matches!(
                p,
                Patch::RemoveEdgeData { .. } | Patch::RemoveHyperedgeData { .. }
            )
        })
        .count();
    assert_eq!(count, 1);
}
