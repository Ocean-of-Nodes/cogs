use super::*;

/// Removing an unknown id returns an error.
#[test]
fn unknown_id() {
    let mut g = Graph::default();
    let err = g.remove_hyperedge(&Uuid::new_v4()).unwrap_err();
    assert!(matches!(err, HyperedgeNotFoundError { .. }));
}

/// Plain remove: hyperedge gone, members survive without
/// `hid` in their `hyperedges` set.
#[test]
fn removes_and_unregisters_members() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut m = HashSet::new();
    m.insert(n1.into());
    m.insert(n2.into());
    let h = g.create_hyperedge(m.clone()).unwrap();

    let returned = g.remove_hyperedge(&h).unwrap();

    assert_eq!(returned, m);
    assert!(!g.hyperedges.contains_key(&h));
    assert!(g.is_exist(&n1));
    assert!(g.is_exist(&n2));
    test_utils::check_index_invariant(&g);
}

/// Edges that pointed at the hyperedge cascade away.
#[test]
fn cascades_to_referencing_edges() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut m = HashSet::new();
    m.insert(n1.into());
    let h = g.create_hyperedge(m).unwrap();
    let e = g.add_edge(n2, h).unwrap();

    g.remove_hyperedge(&h).unwrap();

    assert!(!g.hyperedges.contains_key(&h));
    assert!(!g.edges.contains_key(&e));
    assert!(g.is_exist(&n2));
    test_utils::check_index_invariant(&g);
}

/// Removing a hyperedge that's a member of another hyperedge:
/// the parent loses this member; if parent becomes empty, it
/// dies too.
#[test]
fn cascades_to_parent_hyperedge_when_emptied() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj);

    let mut inner_m = HashSet::new();
    inner_m.insert(n1.into());
    let inner = g.create_hyperedge(inner_m).unwrap();

    let mut outer_m = HashSet::new();
    outer_m.insert(inner.into());
    let outer = g.create_hyperedge(outer_m).unwrap();

    g.remove_hyperedge(&inner).unwrap();

    assert!(!g.hyperedges.contains_key(&inner));
    assert!(!g.hyperedges.contains_key(&outer));
    test_utils::check_index_invariant(&g);
}

/// Parent hyperedge with multiple members loses one — survives.
#[test]
fn parent_hyperedge_loses_only_this_member() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut inner_m = HashSet::new();
    inner_m.insert(n1.into());
    let inner = g.create_hyperedge(inner_m).unwrap();

    let mut outer_m = HashSet::new();
    outer_m.insert(inner.into());
    outer_m.insert(n2.into());
    let outer = g.create_hyperedge(outer_m).unwrap();

    g.remove_hyperedge(&inner).unwrap();

    let mut expected = HashSet::new();
    expected.insert(n2.into());
    assert_eq!(g.hyperedge_members(&outer), Some(&expected));
    test_utils::check_index_invariant(&g);
}

/// Attached object on the hyperedge is dropped along with it.
#[test]
fn drops_attached_object() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());

    let mut m = HashSet::new();
    m.insert(n1.into());
    let h = g.create_hyperedge(m).unwrap();
    g.attach_obj(h, obj).unwrap();
    assert!(g.entities.contains_key(&h));

    g.remove_hyperedge(&h).unwrap();

    assert!(!g.entities.contains_key(&h));
    test_utils::check_index_invariant(&g);
}

/// Records exactly one `Patch::RemoveHyperedge` even on a cascade.
#[test]
fn records_single_patch_on_cascade() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut m = HashSet::new();
    m.insert(n1.into());
    let h = g.create_hyperedge(m).unwrap();
    let _e = g.add_edge(n2, h).unwrap();

    g.remove_hyperedge(&h).unwrap();

    let last = g.events.last().unwrap();
    assert_eq!(*last, Patch::RemoveHyperedge { id: h });
    let count = g
        .events
        .iter()
        .filter(|p| matches!(p, Patch::RemoveHyperedge { .. }))
        .count();
    assert_eq!(count, 1);
}
