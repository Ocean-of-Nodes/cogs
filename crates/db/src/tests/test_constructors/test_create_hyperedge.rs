use super::*;

/// Create a hyperedge with two members and verify
/// both id presence and members round-trip.
#[test]
fn members_round_trip() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut members = HashSet::new();
    members.insert(n1.into());
    members.insert(n2.into());

    let h = g.create_hyperedge(members.clone()).unwrap();
    assert!(g.is_exist(&h));
    assert_eq!(g.hyperedge_members(&h), Some(&members));
}

/// An empty member set is rejected — every hyperedge
/// must have at least one member.
#[test]
fn create_hyperedge_empty_rejected() {
    let mut g = Graph::default();
    let err = g.create_hyperedge(HashSet::new()).unwrap_err();
    assert_eq!(err, CreateHyperedgeError::EmptyHyperedge);
}

/// Members may include other hyperedges (nesting) and
/// edge ids — `create_hyperedge` doesn't validate
/// membership shape.
#[test]
fn create_hyperedge_with_edge_and_hyperedge_members() {
    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, h) =
        test_utils::create_sample_graph2();

    let mut members = HashSet::new();
    members.insert(e_a.into());
    members.insert(h.into());

    let h2 = g.create_hyperedge(members.clone()).unwrap();
    assert_eq!(g.hyperedge_members(&h2), Some(&members));
}

/// `__create_hyperedge_with_id` rejects a duplicate id.
#[test]
fn create_hyperedge_already_exists() {
    let mut g = Graph::default();
    let n1 = g.add_node(test_utils::create_simple_obj("f"));
    let mut members = HashSet::new();
    members.insert(n1.into());
    let h = g.create_hyperedge(members.clone()).unwrap();
    let err = g
        .silent_create_hyperedge_with_id(&h, members)
        .unwrap_err();
    assert_eq!(
        err,
        CreateHyperedgeError::HyperedgeAlreadyExists(HyperedgeAlreadyExistsError {
            id: h
        })
    );
}

/// Re-inserting with the same id must NOT clobber the
/// existing members — the original hyperedge stays
/// intact.
#[test]
fn create_hyperedge_already_exists_preserves_members() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut original = HashSet::new();
    original.insert(n1.into());
    let h = g.create_hyperedge(original.clone()).unwrap();

    // Try to overwrite with a different (non-empty) member set.
    let mut other = HashSet::new();
    other.insert(n2.into());
    let _ = g.silent_create_hyperedge_with_id(&h, other);

    assert_eq!(g.hyperedge_members(&h), Some(&original));
}
