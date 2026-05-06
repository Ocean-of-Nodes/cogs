use super::*;

#[test]
fn test_create_hyperedge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let mut m = HashSet::new();
    m.insert(n1.into());
    m.insert(n2.into());

    let h = g.create_hyperedge(m.clone()).unwrap();

    assert_eq!(
        *g.events.last().unwrap(),
        Patch::CreateHyperEdge { id: h, members: m }
    )
}

#[test]
fn test_add_node() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("field");
    let n1 = g.add_node(obj.clone());

    assert_eq!(*g.events.last().unwrap(), Patch::AddNode { id: n1, obj })
}

#[test]
fn test_add_edge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let e = g.add_edge(n1, n2).unwrap();

    assert_eq!(
        *g.events.last().unwrap(),
        Patch::AddEdge {
            id: e,
            source: n1.into(),
            target: n2.into()
        }
    )
}

#[test]
fn test_remove_node() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());

    let e = g.add_edge(n1, n2).unwrap();

    g.remove_node(&n1).unwrap();

    // To ensure that remove doesn't produce remove event's
    assert_eq!(
        g.events[0],
        Patch::AddNode {
            id: n1,
            obj: obj.clone()
        }
    );
    assert_eq!(g.events[1], Patch::AddNode { id: n2, obj: obj });
    assert_eq!(
        g.events[2],
        Patch::AddEdge {
            id: e,
            source: n1.into(),
            target: n2.into()
        }
    );
    assert_eq!(g.events[3], Patch::RemoveNode { id: n1 })
}

#[test]
fn test_remove_edge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());

    let e = g.add_edge(n1, n2).unwrap();

    g.remove_edge(&e).unwrap();

    assert_eq!(*g.events.last().unwrap(), Patch::RemoveEdge { id: e });
}
