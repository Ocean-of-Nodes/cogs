//! What a running invocation may see and change.

use super::*;

/// Run one view once over `db` and return the report.
fn run_once(
    db: &mut Database,
    mut view: impl FnMut(&mut ViewGraph) -> Result<(), String>,
) -> Report {
    db.propagate_with(|_, _, g| view(g)).unwrap()
}

#[test]
fn view_cannot_change_inputs() {
    let mut db = Database::new(Graph::default());
    let a = db.graph_mut().add_node(def("a"));
    let b = db.graph_mut().add_node(def("b"));
    let e = db.graph_mut().add_edge(a, b).unwrap();
    let h = db
        .graph_mut()
        .create_hyperedge(HashSet::from([a.into()]))
        .unwrap();
    let before = content(db.graph());
    db.register_view("vandal", None).unwrap();

    let report = run_once(&mut db, |g| {
        assert_eq!(g.remove_node(&a), Err(NodeNotFoundError { id: a }));
        assert_eq!(
            g.replace_node(&a, def("x")),
            Err(NodeNotFoundError { id: a })
        );
        assert_eq!(g.remove_edge(&e), Err(EdgeNotFoundError { id: e }));
        assert_eq!(
            g.remove_hyperedge(&h),
            Err(HyperedgeNotFoundError { id: h })
        );
        assert!(matches!(
            g.attach_obj(e, def("x")),
            Err(AttachObjectError::AttachTargetNotFound(_))
        ));
        assert!(matches!(
            g.retarget_edge(&e, RetargetEdge::Target(a.into())),
            Err(RetargetError::EdgeNotFound(_))
        ));
        assert!(matches!(
            g.add_hyperedge_members(h, HashSet::from([b.into()])),
            Err(AddHyperedgeMembersError::HyperedgeNotFound(_))
        ));
        assert!(matches!(
            g.remove_hyperedge_members(h, HashSet::from([a.into()])),
            Err(RemoveHyperedgeMembersError::HyperedgeNotFound(_))
        ));
        // Reading and pointing at inputs is fine.
        assert_eq!(g.obj(&a), Some(def("a")));
        let own = g.add_edge(a, b).unwrap();
        assert_ne!(own, e);
        g.remove_edge(&own).unwrap();
        Ok(())
    });
    assert_eq!(report.failures, vec![]);
    assert_eq!(content(db.graph()), before);
}

#[test]
fn view_cannot_change_other_views_outputs() {
    let mut db = Database::new(Graph::default());
    db.register_view("maker", None).unwrap();
    db.register_view("taker", None).unwrap();
    let report = db
        .propagate_with(|name, _, g| {
            if name == "maker" {
                g.add_node(def("made"));
                return Ok(());
            }
            let made = g.nodes_where("name", &"made".into())[0];
            assert_eq!(g.remove_node(&made), Err(NodeNotFoundError { id: made }));
            // ...but can point at it.
            let own = g.add_node(def("own"));
            g.add_edge(own, made).map_err(err)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(report.failures, vec![]);
    assert_eq!(db.graph().iter_edges().count(), 1);
}

#[test]
fn own_entities_from_this_run_can_be_changed() {
    let mut db = Database::new(Graph::default());
    let input = db.graph_mut().add_node(def("input"));
    db.register_view("v", None).unwrap();
    let report = run_once(&mut db, |g| {
        let n = g.add_node(def("n"));
        assert_eq!(g.replace_node(&n, def("m")), Ok(Field::Object(def("n"))));
        let e = g.add_edge(n, input).unwrap();
        g.attach_obj(e, def("e")).unwrap();
        assert_eq!(
            g.replace_attached_obj(&e, def("f")).unwrap(),
            Field::Object(def("e"))
        );
        g.retarget_edge(&e, RetargetEdge::Source(input.into()))
            .unwrap();
        let h = g.create_hyperedge(HashSet::from([n.into()])).unwrap();
        g.add_hyperedge_members(h, HashSet::from([input.into()]))
            .unwrap();
        g.remove_hyperedge_members(h, HashSet::from([n.into()]))
            .unwrap();
        assert_eq!(g.hyperedge_members(&h), Some(HashSet::from([input.into()])));
        g.remove_attached(e).unwrap();
        g.remove_node(&n).unwrap();
        assert!(!g.is_exist(&n));
        Ok(())
    });
    assert_eq!(report.failures, vec![]);
    test_utils::check_index_invariant(db.graph());
}

#[test]
fn previous_outputs_are_hidden_until_written_again() {
    let mut db = Database::new(Graph::default());
    let input = db.graph_mut().add_node(obj(&[("phase", 1.into())]));
    db.register_view("v", None).unwrap();

    let run = |_: &str, _: Option<EntityId>, g: &mut ViewGraph| {
        let phase = g.field(&input, "phase");
        if phase == Some(Field::Number(1)) {
            let n = g.add_node(def("n"));
            let e = g.add_edge(n, input).map_err(err)?;
            g.attach_obj(e, def("e")).map_err(err)?;
            return Ok(());
        }
        // Second run: the outputs of the first one are invisible...
        let none: Vec<EntityId> = vec![];
        assert_eq!(g.iter_nodes(), vec![input]);
        assert_eq!(g.iter_edges(), none);
        assert_eq!(g.in_edges(&input.into()), none);
        // ...until written again. The attached object stays hidden
        // until it is attached again.
        let n = g.add_node(def("n"));
        let e = g.add_edge(n, input).map_err(err)?;
        assert_eq!(g.obj(&e), None);
        assert_eq!(g.classify_pointee(&e.into()), Some(PointeeKind::Edge));
        assert_eq!(g.iter_attached(), none);
        g.attach_obj(e, def("e")).map_err(err)?;
        assert_eq!(
            g.classify_pointee(&e.into()),
            Some(PointeeKind::AttachedObject)
        );
        Ok(())
    };

    db.propagate_with(run).unwrap();
    let before = content(db.graph());
    db.graph_mut()
        .replace_node(&input, obj(&[("phase", 2.into())]))
        .unwrap();
    let report = db.propagate_with(run).unwrap();
    assert_eq!(report.failures, vec![]);

    // Same outputs, same ids: only the input changed.
    let mut expected = before;
    expected.0.insert(input, obj(&[("phase", 2.into())]));
    assert_eq!(content(db.graph()), expected);
}

#[test]
fn edge_to_a_missing_node_is_retried_when_it_appears() {
    let mut db = Database::new(Graph::default());
    let target = db.graph_mut().add_node(def("target"));
    let source = db
        .graph_mut()
        .add_node(obj(&[("link", Field::Link(target.into()))]));
    db.graph_mut().remove_node(&target).unwrap();
    db.register_view("linker", None).unwrap();

    let run = |_: &str, _: Option<EntityId>, g: &mut ViewGraph| {
        if let Some(Field::Link(target)) = g.field(&source, "link") {
            let _ = g.add_edge(source, target);
        }
        Ok(())
    };
    db.propagate_with(run).unwrap();
    assert_eq!(db.graph().iter_edges().count(), 0);

    // Undo brings the target back with its old id.
    db.graph_mut().undo().unwrap();
    let report = db.propagate_with(run).unwrap();
    assert_eq!(report.runs.len(), 1);
    assert_eq!(
        db.graph().out_edges(&source.into()).count(),
        1,
        "the edge is added once the target exists"
    );
}

#[test]
fn hyperedge_members_are_reconciled() {
    let mut db = Database::new(Graph::default());
    let a = db.graph_mut().add_node(def("a"));
    let b = db.graph_mut().add_node(def("b"));
    let c = db.graph_mut().add_node(def("c"));
    db.register_view("group", None).unwrap();
    let run = |_: &str, _: Option<EntityId>, g: &mut ViewGraph| {
        let members = g
            .nodes_where("kind", &"def".into())
            .into_iter()
            .map(Pointee::from)
            .collect();
        g.create_hyperedge(members).map_err(err)?;
        Ok(())
    };
    db.propagate_with(run).unwrap();
    let h = db.graph().iter_hyperedges().next().unwrap();

    db.graph_mut().remove_node(&a).unwrap();
    let d = db.graph_mut().add_node(def("d"));
    db.propagate_with(run).unwrap();

    let g = db.graph();
    assert_eq!(g.iter_hyperedges().collect::<Vec<_>>(), vec![h]);
    assert_eq!(
        g.hyperedge_members(&h),
        Some(&HashSet::from([b.into(), c.into(), d.into()]))
    );
    test_utils::check_index_invariant(g);
}
