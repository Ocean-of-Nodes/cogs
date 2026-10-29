//! Inputs and derived facts: input writes cannot see derived
//! entities, and derived changes stay out of the undo log.

use super::*;

/// A use resolved to a definition: the resolution edge and the
/// group hyperedge are derived.
fn resolved_graph() -> (Database, NodeId, NodeId, EdgeId, HyperedgeId) {
    let mut db = compiler(Graph::default());
    let d = db.graph_mut().add_node(def("foo"));
    let u = db.graph_mut().add_node(use_site("foo"));
    propagate(&mut db);
    let g = db.graph();
    let edge = g.out_edges(&u.into()).next().expect("resolution edge");
    let group = g.hyperedges_of(&d.into()).next().expect("group hyperedge");
    (db, d, u, edge, group)
}

#[test]
fn input_writes_do_not_see_derived_entities() {
    let (mut db, d, u, edge, group) = resolved_graph();
    let summary = db
        .graph()
        .nodes_where("kind", &"summary".into())
        .next()
        .unwrap();
    let before = content(db.graph());
    let g = db.graph_mut();

    assert_eq!(
        g.remove_node(&summary),
        Err(NodeNotFoundError { id: summary })
    );
    assert_eq!(
        g.replace_node(&summary, def("x")),
        Err(NodeNotFoundError { id: summary })
    );
    assert_eq!(g.remove_edge(&edge), Err(EdgeNotFoundError { id: edge }));
    assert_eq!(
        g.remove_hyperedge(&group),
        Err(HyperedgeNotFoundError { id: group })
    );
    assert!(matches!(
        g.remove_attached(edge),
        Err(NoAttachedObjectError { .. })
    ));
    assert!(matches!(
        g.replace_attached_obj(&edge, def("x")),
        Err(NoAttachedObjectError { .. })
    ));
    assert!(matches!(
        g.retarget_edge(&edge, RetargetEdge::Target(u.into())),
        Err(RetargetError::EdgeNotFound(_))
    ));
    assert!(matches!(
        g.add_hyperedge_members(group, HashSet::from([u.into()])),
        Err(AddHyperedgeMembersError::HyperedgeNotFound(_))
    ));
    assert!(matches!(
        g.remove_hyperedge_members(group, HashSet::from([d.into()])),
        Err(RemoveHyperedgeMembersError::HyperedgeNotFound(_))
    ));

    // Inputs cannot point at derived entities either.
    assert_eq!(
        g.add_edge(u, summary),
        Err(AddEdgeError::MissingEndpoints(MissingEndpointsError {
            missing_endpoints: vec![summary.into()]
        }))
    );
    assert_eq!(
        g.create_hyperedge(HashSet::from([u.into(), summary.into()])),
        Err(CreateHyperedgeError::PointeesNotFound(
            PointeesNotFoundError {
                pointees: HashSet::from([summary.into()])
            }
        ))
    );
    let input_edge = g.add_edge(u, d).unwrap();
    assert!(matches!(
        g.retarget_edge(&input_edge, RetargetEdge::Target(summary.into())),
        Err(RetargetError::InvalidTarget(_))
    ));
    let path = Pointee::Path(GlobalObjPath::new(summary, "kind").unwrap());
    assert!(matches!(
        g.add_edge(u, path),
        Err(AddEdgeError::MissingEndpoints(_))
    ));

    g.remove_edge(&input_edge).unwrap();
    assert_eq!(content(db.graph()), before);
}

#[test]
fn derived_changes_are_not_in_the_undo_log() {
    let mut db = compiler(Graph::default());
    db.graph_mut().add_node(def("foo"));
    db.graph_mut().add_node(use_site("foo"));
    db.graph_mut().add_node(use_site("bar"));
    propagate(&mut db);

    assert_eq!(db.graph().events.len(), 3);
    assert!(
        db.graph()
            .events
            .iter()
            .all(|m| !crate::views::is_derived(&crate::views::subject(&m.operation)))
    );
}

#[test]
fn new_database_drops_derived_entities_and_derives_them_again() {
    let (db, ..) = resolved_graph();
    let derived = content(db.graph());
    let graph = db.into_graph();

    let mut db = compiler(graph);
    let g = db.graph();
    assert!(g.iter_entities().all(|id| !crate::views::is_derived(&id)));
    test_utils::check_index_invariant(g);

    propagate(&mut db);
    assert_eq!(content(db.graph()), derived);
}
