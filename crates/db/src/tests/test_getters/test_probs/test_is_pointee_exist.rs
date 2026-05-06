use super::*;

#[test]
fn entity_node() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj);
    assert!(g.is_pointee_exist(&n1.into()));
}

#[test]
fn entity_edge() {
    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();
    assert!(g.is_pointee_exist(&e_a.into()));
}

#[test]
fn entity_hyperedge() {
    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
        test_utils::create_sample_graph2();
    assert!(g.is_pointee_exist(&h.into()));
}

#[test]
fn entity_meta_edge() {
    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();
    assert!(g.is_pointee_exist(&meta_edge.into()));
}

#[test]
fn entity_attached() {
    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();
    let obj = test_utils::create_simple_obj("attached");
    g.attach_obj(e_a, obj).unwrap();
    assert!(g.is_pointee_exist(&e_a.into()));
}

#[test]
fn entity_unknown() {
    let g = Graph::default();
    let unknown: Pointee = Uuid::new_v4().into();
    assert!(!g.is_pointee_exist(&unknown));
}

/// Path resolves to a real top-level field.
#[test]
fn path_resolves() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj);
    let p = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
    assert!(g.is_pointee_exist(&p));
}

/// Path descends through a nested `Field::Object`.
#[test]
fn path_resolves_nested() {
    let mut g = Graph::default();
    let mut inner = Object::new();
    inner.insert("leaf".into(), Field::Null);
    let mut obj = Object::new();
    obj.insert("nested".into(), Field::Object(inner));
    let n1 = g.add_node(obj);

    let mut path = GlobalObjPath::new(n1, "nested").unwrap();
    path.push("leaf").unwrap();
    let p = Pointee::Path(path);
    assert!(g.is_pointee_exist(&p));
}

/// Entity isn't in the graph.
#[test]
fn path_unknown_entity() {
    let g = Graph::default();
    let p = Pointee::Path(GlobalObjPath::new(Uuid::new_v4(), "x").unwrap());
    assert!(!g.is_pointee_exist(&p));
}

/// Entity exists but the first segment doesn't match
/// any top-level field.
#[test]
fn path_missing_field() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj);
    let p = Pointee::Path(GlobalObjPath::new(n1, "no_such_field").unwrap());
    assert!(!g.is_pointee_exist(&p));
}

/// Walking a path through a non-Object field
/// (e.g. `Field::Null`) must fail.
#[test]
fn path_through_non_object_fails() {
    let mut g = Graph::default();
    // "test_field" is a Field::Null — it is not an
    // Object, so any further descent must fail.
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj);

    let mut path = GlobalObjPath::new(n1, "test_field").unwrap();
    path.push("anything").unwrap();
    let p = Pointee::Path(path);
    assert!(!g.is_pointee_exist(&p));
}

/// Edge id with a sub-path doesn't navigate — only
/// `entities` is consulted, and a bare edge has no
/// attached object.
#[test]
fn path_on_bare_edge_fails() {
    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();
    let p = Pointee::Path(GlobalObjPath::new(e_a, "x").unwrap());
    assert!(!g.is_pointee_exist(&p));
}
