use super::*;
#[test]
fn node_exists() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    assert!(g.is_exist(&n1))
}

#[test]
fn edge_exists() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);
    let e1 = g.add_edge(n1, n2).unwrap();
    assert!(g.is_exist(&e1))
}

#[test]
fn hyperedge_exists() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);
    let mut m = HashSet::new();
    m.insert(n1.into());
    m.insert(n2.into());
    let h = g.create_hyperedge(m).unwrap();
    assert!(g.is_exist(&h))
}

/// Meta-edge: an edge whose endpoint is another edge.
#[test]
fn meta_edge_exists() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);
    let e1 = g.add_edge(n1, n2).unwrap();
    let meta_edge = g.add_edge(n1, e1).unwrap();
    assert!(g.is_exist(&meta_edge))
}
