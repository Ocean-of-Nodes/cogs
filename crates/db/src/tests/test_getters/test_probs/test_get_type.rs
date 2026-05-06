use super::*;

/// Unknown id resolves to `None`.
#[test]
fn unknown_is_none() {
    let g = Graph::default();
    assert!(g.get_type(Uuid::new_v4()).is_none());
}

/// Pure node — only in `entities`.
#[test]
fn node() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj);
    assert!(matches!(g.get_type(n1), Some(EntityType::Node)));
}

/// Regular edge — both endpoints are nodes.
#[test]
fn edge() {
    let (g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();
    assert!(matches!(g.get_type(e_a), Some(EntityType::Edge)));
}

/// Edge whose endpoint is another edge → MetaEdge.
#[test]
fn meta_edge_with_edge_endpoint() {
    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();
    assert!(matches!(g.get_type(meta_edge), Some(EntityType::MetaEdge)));
}

/// Edge whose endpoint is a hyperedge → MetaEdge.
#[test]
fn meta_edge_with_hyperedge_endpoint() {
    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, edge_to_h, _h) =
        test_utils::create_sample_graph2();
    assert!(matches!(g.get_type(edge_to_h), Some(EntityType::MetaEdge)));
}

/// Pure hyperedge — only in `hyper_edge`.
#[test]
fn hyperedge() {
    let (g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
        test_utils::create_sample_graph2();
    assert!(matches!(g.get_type(h), Some(EntityType::HyperEdge)));
}

/// Object attached on top of an edge — id collides
/// in both `entities` and `edges` → AttachedObject.
#[test]
fn attached_on_edge() {
    let (mut g, _n1, _n2, _n3, _n4, e_a, _e_b, _meta_edge, _edge_to_h, _h) =
        test_utils::create_sample_graph2();
    let obj = test_utils::create_simple_obj("attached");
    g.attach_obj(e_a, obj).unwrap();
    assert!(matches!(g.get_type(e_a), Some(EntityType::AttachedObject)));
}

/// Object attached on top of a hyperedge — id
/// collides in both `entities` and `hyper_edge` →
/// AttachedObject.
#[test]
fn attached_on_hyperedge() {
    let (mut g, _n1, _n2, _n3, _n4, _e_a, _e_b, _meta_edge, _edge_to_h, h) =
        test_utils::create_sample_graph2();
    let obj = test_utils::create_simple_obj("attached");
    g.attach_obj(h, obj).unwrap();
    assert!(matches!(g.get_type(h), Some(EntityType::AttachedObject)));
}

/// `Pointee::Path` endpoint must NOT promote an edge
/// to MetaEdge — only edge/hyperedge endpoints do.
#[test]
fn path_endpoint_stays_edge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let p1 = Pointee::Path(GlobalObjPath::new(n1, "test_field").unwrap());
    let e1 = g.add_edge(p1, n2).unwrap();
    assert!(matches!(g.get_type(e1), Some(EntityType::Edge)));
}
