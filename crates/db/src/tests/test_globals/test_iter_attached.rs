use super::*;

/// No attached objects yet — only nodes live in
/// `entities`, so `iter_attached` is empty.
#[test]
fn empty_when_only_nodes() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    g.add_node(obj.clone());
    g.add_node(obj);
    assert_eq!(g.iter_attached().count(), 0);
}

/// Yields exactly the ids on which `attach_obj` placed an
/// object — both edges and hyperedges qualify.
#[test]
fn yields_attach_targets() {
    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, h) =
        test_utils::create_sample_graph2();
    let obj = test_utils::create_simple_obj("attached");

    g.attach_obj(e_a, obj.clone()).unwrap();
    g.attach_obj(h, obj).unwrap();

    let actual: HashSet<_> = g.iter_attached().collect();
    let expected: HashSet<_> = [e_a, h].into_iter().collect();
    assert_eq!(actual, expected);
}

/// Complement of `iter_nodes`: a node id, even if it has
/// an object, must NOT appear in `iter_attached`.
#[test]
fn excludes_nodes() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj);
    let actual: HashSet<_> = g.iter_attached().collect();
    assert!(!actual.contains(&n1));
}
