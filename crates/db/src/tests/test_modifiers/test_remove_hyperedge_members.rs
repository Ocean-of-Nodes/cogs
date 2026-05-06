use super::*;

/// Unknown hyperedge id is rejected.
#[test]
fn unknown_hyperedge() {
    let mut g = Graph::default();
    let mut m = HashSet::new();
    m.insert(Pointee::EntityId(Uuid::new_v4()));
    let err = g.remove_hyperedge_members(Uuid::new_v4(), m).unwrap_err();
    assert!(matches!(err, RemoveHyperedgeMembersError::HyperedgeNotFound(_)));
}

/// Removing a pointee that's not a current member is rejected,
/// and nothing is partially applied.
#[test]
fn member_not_in_hyperedge_atomic() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);

    let mut original = HashSet::new();
    original.insert(n1.into());
    original.insert(n2.into());
    let h = g.create_hyperedge(original.clone()).unwrap();

    let mut m = HashSet::new();
    m.insert(n1.into()); // valid
    m.insert(n3.into()); // not a member
    let err = g.remove_hyperedge_members(h, m).unwrap_err();
    assert!(matches!(
        err,
        RemoveHyperedgeMembersError::MembersNotInHyperedge(_)
    ));

    // Atomicity: n1 was NOT removed.
    assert_eq!(g.hyperedge_members(&h), Some(&original));
    test_utils::check_index_invariant(&g);
}

/// Successful partial removal: hyperedge survives with the rest.
#[test]
fn removes_subset_and_records_patch() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut original = HashSet::new();
    original.insert(n1.into());
    original.insert(n2.into());
    let h = g.create_hyperedge(original).unwrap();

    let mut to_remove = HashSet::new();
    to_remove.insert(n1.into());
    g.remove_hyperedge_members(h, to_remove.clone()).unwrap();

    let mut expected = HashSet::new();
    expected.insert(n2.into());
    assert_eq!(g.hyperedge_members(&h), Some(&expected));

    assert_eq!(
        *g.events.last().unwrap(),
        Patch::RemoveHyperedgeMembers {
            id: h,
            members: to_remove,
        }
    );
    test_utils::check_index_invariant(&g);
}

/// Reverse index is cleaned: removed member's bucket no longer
/// references this hyperedge.
#[test]
fn removed_member_loses_hyperedge_link() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut original = HashSet::new();
    original.insert(n1.into());
    original.insert(n2.into());
    let h = g.create_hyperedge(original).unwrap();

    let mut to_remove = HashSet::new();
    to_remove.insert(n1.into());
    g.remove_hyperedge_members(h, to_remove).unwrap();

    // n1 had only this hyperedge link → bucket fully gone.
    assert!(!g.pointee_uses.contains_key(&Pointee::EntityId(n1)));
    test_utils::check_index_invariant(&g);
}

/// Removing all members empties the hyperedge — it dies and
/// any references to it cascade.
#[test]
fn empties_and_kills_hyperedge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut original = HashSet::new();
    original.insert(n1.into());
    let h = g.create_hyperedge(original.clone()).unwrap();
    let e = g.add_edge(n2, h).unwrap();

    g.remove_hyperedge_members(h, original).unwrap();

    assert!(!g.hyper_edge.contains_key(&h));
    assert!(!g.edges.contains_key(&e));
    test_utils::check_index_invariant(&g);
}

/// Empty input is a no-op success.
#[test]
fn empty_input_is_noop() {
    let mut g = Graph::default();
    let n1 = g.add_node(test_utils::create_simple_obj("f"));
    let mut original = HashSet::new();
    original.insert(n1.into());
    let h = g.create_hyperedge(original.clone()).unwrap();

    g.remove_hyperedge_members(h, HashSet::new()).unwrap();

    assert_eq!(g.hyperedge_members(&h), Some(&original));
    test_utils::check_index_invariant(&g);
}

/// Path-pointee removal also untracks from entity_to_path_pointees
/// (when its bucket fully empties).
#[test]
fn removes_path_member_and_untracks() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
    let mut original = HashSet::new();
    original.insert(n1.into());
    original.insert(path.clone());
    let h = g.create_hyperedge(original).unwrap();

    let mut to_remove = HashSet::new();
    to_remove.insert(path.clone());
    g.remove_hyperedge_members(h, to_remove).unwrap();

    assert!(!g.pointee_uses.contains_key(&path));
    assert!(!g.entity_to_path_pointees.contains_key(&n2));
    test_utils::check_index_invariant(&g);
}
