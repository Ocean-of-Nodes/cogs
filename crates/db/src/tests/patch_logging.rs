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
        ThinPatch::from(&g.events.last().unwrap().operation),
        ThinPatch::CreateHyperedge { id: h, members: m }
    )
}

#[test]
fn test_add_node() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("field");
    let n1 = g.add_node(obj.clone());

    assert_eq!(ThinPatch::from(&g.events.last().unwrap().operation), ThinPatch::AddNode { id: n1, obj })
}

#[test]
fn test_add_edge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let e = g.add_edge(n1, n2).unwrap();

    assert_eq!(
        ThinPatch::from(&g.events.last().unwrap().operation),
        ThinPatch::AddEdge {
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
        ThinPatch::from(&g.events[0].operation),
        ThinPatch::AddNode {
            id: n1,
            obj: obj.clone()
        }
    );
    assert_eq!(ThinPatch::from(&g.events[1].operation), ThinPatch::AddNode { id: n2, obj: obj.clone() });
    assert_eq!(
        ThinPatch::from(&g.events[2].operation),
        ThinPatch::AddEdge {
            id: e,
            source: n1.into(),
            target: n2.into()
        }
    );
    assert_eq!(
        g.events[3].operation,
        FullPatch::RemoveNode {
            id: n1,
            obj: obj.clone()
        }
    );
    assert_eq!(
        g.events[3].cascade,
        vec![FullPatch::RemoveEdge {
            id: e,
            source: n1.into(),
            target: n2.into(),
            data: None,
        }]
    );
}

#[test]
fn test_cascade_records_removed_hyperedge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());

    let h = g.create_hyperedge(HashSet::from([n1.into()])).unwrap();
    let data = test_utils::create_simple_obj("data");
    g.attach_obj(h, data.clone()).unwrap();
    let e = g.add_edge(h, n2).unwrap();

    g.remove_node(&n1).unwrap();

    let last = g.events.last().unwrap();
    assert_eq!(last.operation, FullPatch::RemoveNode { id: n1, obj });
    assert_eq!(
        last.cascade,
        vec![
            FullPatch::RemoveHyperedge {
                id: h,
                members: HashSet::from([n1.into()]),
                data: Some(data),
            },
            FullPatch::RemoveEdge {
                id: e,
                source: h.into(),
                target: n2.into(),
                data: None,
            },
        ]
    );
}

#[test]
fn test_cascade_records_removed_member_of_live_hyperedge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let h = g
        .create_hyperedge(HashSet::from([n1.into(), n2.into()]))
        .unwrap();

    g.remove_node(&n1).unwrap();

    assert_eq!(
        g.events.last().unwrap().cascade,
        vec![FullPatch::RemoveHyperedgeMembers {
            id: h,
            members: HashSet::from([n1.into()]),
        }]
    );
}

#[test]
fn test_cascade_after_emptying_hyperedge_members() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let h = g.create_hyperedge(HashSet::from([n1.into()])).unwrap();
    let e = g.add_edge(h, n2).unwrap();

    g.remove_hyperedge_members(h, HashSet::from([n1.into()]))
        .unwrap();

    assert_eq!(
        g.events.last().unwrap().cascade,
        vec![
            FullPatch::RemoveHyperedge {
                id: h,
                members: HashSet::new(),
                data: None,
            },
            FullPatch::RemoveEdge {
                id: e,
                source: h.into(),
                target: n2.into(),
                data: None,
            },
        ]
    );
}

#[test]
fn test_remove_edge_cascades_to_referencing_edges() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);

    let e1 = g.add_edge(n1, n2).unwrap();
    let e2 = g.add_edge(e1, n2).unwrap();

    g.remove_edge(&e1).unwrap();

    assert!(!g.edges.contains_key(&e2));
    test_utils::check_index_invariant(&g);
    assert_eq!(
        g.events.last().unwrap().cascade,
        vec![FullPatch::RemoveEdge {
            id: e2,
            source: e1.into(),
            target: n2.into(),
            data: None,
        }]
    );
}

#[test]
fn test_no_cascade_without_dependents() {
    let mut g = Graph::default();
    let n1 = g.add_node(test_utils::create_simple_obj("field"));

    g.remove_node(&n1).unwrap();

    assert!(g.events.last().unwrap().cascade.is_empty());
}

#[test]
fn test_full_patch_into_thin() {
    let id = uuid::Uuid::new_v4();
    let full = FullPatch::RemoveNode {
        id,
        obj: test_utils::create_simple_obj("field"),
    };
    assert_eq!(ThinPatch::from(full), ThinPatch::RemoveNode { id });
}

#[test]
fn test_remove_edge() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());

    let e = g.add_edge(n1, n2).unwrap();

    g.remove_edge(&e).unwrap();

    assert_eq!(ThinPatch::from(&g.events.last().unwrap().operation), ThinPatch::RemoveEdge { id: e });
}
