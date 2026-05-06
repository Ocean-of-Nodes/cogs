use super::*;

/// Unknown hyperedge id is rejected.
#[test]
fn unknown_hyperedge() {
    let mut g = Graph::default();
    let n1 = g.add_node(test_utils::create_simple_obj("f"));
    let mut m = HashSet::new();
    m.insert(n1.into());
    let err = g.add_hyperedge_members(Uuid::new_v4(), m).unwrap_err();
    assert!(matches!(err, AddHyperedgeMembersError::HyperedgeNotFound(_)));
}

/// Members that don't exist as pointees are rejected.
#[test]
fn missing_pointee() {
    let mut g = Graph::default();
    let n1 = g.add_node(test_utils::create_simple_obj("f"));
    let mut original = HashSet::new();
    original.insert(n1.into());
    let h = g.create_hyperedge(original).unwrap();

    let mut m = HashSet::new();
    m.insert(Pointee::EntityId(Uuid::new_v4()));
    let err = g.add_hyperedge_members(h, m).unwrap_err();
    assert!(matches!(err, AddHyperedgeMembersError::PointeesNotFound(_)));
}

/// Adding a member that's already there is rejected,
/// and nothing is partially applied.
#[test]
fn duplicate_member_rejected_atomically() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut original = HashSet::new();
    original.insert(n1.into());
    let h = g.create_hyperedge(original.clone()).unwrap();

    let mut m = HashSet::new();
    m.insert(n1.into()); // duplicate
    m.insert(n2.into()); // would-be new
    let err = g.add_hyperedge_members(h, m).unwrap_err();
    assert!(matches!(err, AddHyperedgeMembersError::MembersAlreadyExist(_)));

    // Atomicity: n2 was NOT added.
    assert_eq!(g.hyperedge_members(&h), Some(&original));
    test_utils::check_index_invariant(&g);
}

/// Successful add: members extended, reverse index updated,
/// patch recorded.
#[test]
fn adds_members_and_records_patch() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let n3 = g.add_node(obj);

    let mut original = HashSet::new();
    original.insert(n1.into());
    let h = g.create_hyperedge(original).unwrap();

    let mut to_add = HashSet::new();
    to_add.insert(n2.into());
    to_add.insert(n3.into());
    g.add_hyperedge_members(h, to_add.clone()).unwrap();

    let mut expected = HashSet::new();
    expected.insert(n1.into());
    expected.insert(n2.into());
    expected.insert(n3.into());
    assert_eq!(g.hyperedge_members(&h), Some(&expected));

    assert_eq!(
        *g.events.last().unwrap(),
        Patch::AddHyperedgeMembers {
            id: h,
            members: to_add,
        }
    );
    test_utils::check_index_invariant(&g);
}

/// Adding a Path-pointee tracks it in `entity_to_path_pointees`.
#[test]
fn tracks_path_member() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut original = HashSet::new();
    original.insert(n1.into());
    let h = g.create_hyperedge(original).unwrap();

    let path = Pointee::Path(GlobalObjPath::new(n2, "test_field").unwrap());
    let mut to_add = HashSet::new();
    to_add.insert(path.clone());
    g.add_hyperedge_members(h, to_add).unwrap();

    assert!(g
        .entity_to_path_pointees
        .get(&n2)
        .is_some_and(|s| s.contains(&path)));
    test_utils::check_index_invariant(&g);
}

/// Empty input is a no-op success.
#[test]
fn empty_input_is_noop() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj);
    let mut original = HashSet::new();
    original.insert(n1.into());
    let h = g.create_hyperedge(original.clone()).unwrap();

    g.add_hyperedge_members(h, HashSet::new()).unwrap();

    assert_eq!(g.hyperedge_members(&h), Some(&original));
    test_utils::check_index_invariant(&g);
}
