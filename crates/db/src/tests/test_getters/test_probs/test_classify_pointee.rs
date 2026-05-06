use super::*;

#[test]
fn entity_node() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj);
    assert_eq!(g.classify_pointee(&n1.into()), Some(PointeeKind::Node));
}

#[test]
fn entity_edge() {
    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();
    assert_eq!(g.classify_pointee(&e_a.into()), Some(PointeeKind::Edge));
}

#[test]
fn entity_hyperedge() {
    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
        test_utils::create_sample_graph2();
    assert_eq!(g.classify_pointee(&h.into()), Some(PointeeKind::HyperEdge));
}

#[test]
fn entity_meta_edge() {
    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();
    assert_eq!(
        g.classify_pointee(&meta_edge.into()),
        Some(PointeeKind::MetaEdge)
    );
}

#[test]
fn entity_attached() {
    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();
    let obj = test_utils::create_simple_obj("attached");
    g.attach_obj(e_a, obj).unwrap();
    assert_eq!(
        g.classify_pointee(&e_a.into()),
        Some(PointeeKind::AttachedObject)
    );
}

#[test]
fn entity_unknown() {
    let g = Graph::default();
    let unknown: Pointee = Uuid::new_v4().into();
    assert_eq!(g.classify_pointee(&unknown), None);
}

/// Path that resolves to a real field → Subobject.
#[test]
fn path_resolves() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj);
    let p = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
    assert_eq!(g.classify_pointee(&p), Some(PointeeKind::Subobject));
}

/// Path whose entity isn't in the graph → None.
#[test]
fn path_unknown_entity() {
    let g = Graph::default();
    let p = Pointee::Path(GlobalObjPath::new(Uuid::new_v4(), "x").unwrap());
    assert_eq!(g.classify_pointee(&p), None);
}

/// Path whose entity exists but the field is missing
/// → None.
#[test]
fn path_missing_field() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj);
    let p = Pointee::Path(GlobalObjPath::new(n1, "no_such_field").unwrap());
    assert_eq!(g.classify_pointee(&p), None);
}
