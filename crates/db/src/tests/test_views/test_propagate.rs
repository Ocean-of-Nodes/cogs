use super::*;

/// `def foo`, `use foo`, `use bar`, propagated.
fn program() -> (Database, NodeId, NodeId, NodeId) {
    let mut db = compiler(Graph::default());
    let d_foo = db.graph_mut().add_node(def("foo"));
    let u_foo = db.graph_mut().add_node(use_site("foo"));
    let u_bar = db.graph_mut().add_node(use_site("bar"));
    propagate(&mut db);
    (db, d_foo, u_foo, u_bar)
}

fn summary_errors(g: &Graph) -> Field {
    let summary = g.nodes_where("kind", &"summary".into()).next().unwrap();
    g.obj(&summary).unwrap()["errors"].clone()
}

#[test]
fn first_propagation_runs_every_invocation() {
    let mut db = compiler(Graph::default());
    let d_foo = db.graph_mut().add_node(def("foo"));
    let u_foo = db.graph_mut().add_node(use_site("foo"));
    let u_bar = db.graph_mut().add_node(use_site("bar"));
    let report = propagate(&mut db);

    for run in [
        run_of("resolve", u_foo),
        run_of("resolve", u_bar),
        run_of("unresolved", u_foo),
        run_of("unresolved", u_bar),
        run_of("summary", None),
        run_of("group", d_foo),
    ] {
        assert!(ran(&db, &report).contains(&run), "{run:?} did not run");
    }
    let g = db.graph();
    assert_eq!(
        resolutions(g),
        BTreeSet::from([(u_foo.into(), d_foo.into())])
    );
    assert_eq!(diagnosed(g), BTreeSet::from([u_bar.into()]));
    assert_eq!(summary_errors(g), Field::Number(1));
    assert_eq!(
        g.hyperedges_of(&d_foo.into())
            .map(|h| g.hyperedge_members(&h).unwrap().clone())
            .collect::<Vec<_>>(),
        vec![HashSet::from([d_foo.into(), u_foo.into()])]
    );
}

#[test]
fn propagating_twice_runs_nothing() {
    let (mut db, ..) = program();
    let before = content(db.graph());
    assert_eq!(propagate(&mut db).runs, vec![]);
    assert_eq!(content(db.graph()), before);
}

#[test]
fn new_definition_reruns_only_what_depends_on_it() {
    let (mut db, d_foo, u_foo, u_bar) = program();
    let d_bar = db.graph_mut().add_node(def("bar"));
    let report = propagate(&mut db);

    assert_eq!(
        ran(&db, &report),
        BTreeSet::from([
            run_of("resolve", u_bar),
            run_of("unresolved", u_bar),
            run_of("summary", None),
            run_of("group", d_bar),
        ])
    );
    let g = db.graph();
    assert_eq!(
        resolutions(g),
        BTreeSet::from([(u_foo.into(), d_foo.into()), (u_bar.into(), d_bar.into())])
    );
    assert_eq!(diagnosed(g), BTreeSet::new());
    assert_eq!(summary_errors(g), Field::Number(0));
}

#[test]
fn unread_field_change_runs_nothing() {
    let (mut db, d_foo, ..) = program();
    let mut documented = def("foo");
    documented.insert("doc".into(), "the foo".into());
    db.graph_mut().replace_node(&d_foo, documented).unwrap();

    assert_eq!(propagate(&mut db).runs, vec![]);
}

#[test]
fn rename_moves_the_resolution() {
    let (mut db, d_foo, u_foo, u_bar) = program();
    db.graph_mut().replace_node(&d_foo, def("baz")).unwrap();
    let report = propagate(&mut db);

    assert_eq!(
        ran(&db, &report),
        BTreeSet::from([
            run_of("resolve", u_foo),
            run_of("unresolved", u_foo),
            run_of("summary", None),
            run_of("group", d_foo),
        ])
    );
    let g = db.graph();
    assert_eq!(resolutions(g), BTreeSet::new());
    assert_eq!(diagnosed(g), BTreeSet::from([u_foo.into(), u_bar.into()]));
    assert_eq!(summary_errors(g), Field::Number(2));
}

#[test]
fn unchanged_result_stops_the_propagation() {
    let (mut db, _, u_foo, _) = program();
    let before = content(db.graph());
    // Matches `name == foo`, so `resolve(u_foo)` re-runs, but it is
    // not a definition: the result is the same and nothing else runs.
    let comment = obj(&[("kind", "comment".into()), ("name", "foo".into())]);
    let c = db.graph_mut().add_node(comment.clone());
    let report = propagate(&mut db);

    assert_eq!(
        ran(&db, &report),
        BTreeSet::from([run_of("resolve", u_foo)])
    );
    let mut expected = before;
    expected.0.insert(c, comment);
    assert_eq!(content(db.graph()), expected);
}

#[test]
fn derived_ids_survive_reruns() {
    let (mut db, _, u_foo, _) = program();
    let edge = db.graph().out_edges(&u_foo.into()).next().unwrap();
    assert!(crate::views::is_derived(&edge));
    let resolve = db.view_id("resolve").unwrap();
    assert_eq!(
        db.owner(&edge),
        Some(InvocationId {
            view: resolve,
            key: Some(u_foo)
        })
    );

    db.graph_mut()
        .add_node(obj(&[("kind", "comment".into()), ("name", "foo".into())]));
    propagate(&mut db);
    assert_eq!(
        db.graph().out_edges(&u_foo.into()).collect::<Vec<_>>(),
        vec![edge]
    );
}

#[test]
fn key_that_disappears_drops_its_invocations_and_outputs() {
    let (mut db, _, _, u_bar) = program();
    let before = db.invocations().len();
    db.graph_mut().remove_node(&u_bar).unwrap();
    propagate(&mut db);

    let g = db.graph();
    assert_eq!(diagnosed(g), BTreeSet::new());
    assert_eq!(g.nodes_where("kind", &"error".into()).count(), 0);
    assert_eq!(summary_errors(g), Field::Number(0));
    assert_eq!(db.invocations().len(), before - 2);
    assert!(db.invocations().iter().all(|inv| inv.key != Some(u_bar)));
}

#[test]
fn key_appears_when_a_node_starts_matching() {
    let (mut db, d_foo, u_foo, u_bar) = program();
    let comment = db
        .graph_mut()
        .add_node(obj(&[("kind", "comment".into()), ("name", "foo".into())]));
    propagate(&mut db);
    db.graph_mut()
        .replace_node(&comment, use_site("foo"))
        .unwrap();
    let report = propagate(&mut db);

    assert!(ran(&db, &report).contains(&run_of("resolve", comment)));
    assert_eq!(
        resolutions(db.graph()),
        BTreeSet::from([(u_foo.into(), d_foo.into()), (comment.into(), d_foo.into())])
    );
    assert_eq!(diagnosed(db.graph()), BTreeSet::from([u_bar.into()]));
}

#[test]
fn removing_a_definition_cascades_and_reruns_its_uses() {
    let (mut db, d_foo, u_foo, u_bar) = program();
    db.graph_mut().remove_node(&d_foo).unwrap();
    propagate(&mut db);

    let g = db.graph();
    assert_eq!(resolutions(g), BTreeSet::new());
    assert_eq!(diagnosed(g), BTreeSet::from([u_foo.into(), u_bar.into()]));
    assert_eq!(g.iter_hyperedges().count(), 0);
}

#[test]
fn derived_nodes_can_be_keys() {
    let mut db = Database::new(Graph::default());
    db.register_view("expand", None).unwrap();
    db.register_view("resolve", Some(Pattern::nodes_where("kind", "use")))
        .unwrap();
    let d = db.graph_mut().add_node(def("foo"));
    let run = |name: &str, key: Option<EntityId>, g: &mut ViewGraph| match (name, key) {
        ("expand", None) => {
            g.add_node(use_site("foo"));
            Ok(())
        }
        ("resolve", Some(key)) => resolve(g, key),
        _ => Err(format!("unknown view {name}")),
    };
    db.propagate_with(run).unwrap();

    let g = db.graph();
    let expanded = g.nodes_where("kind", &"use".into()).next().unwrap();
    assert!(crate::views::is_derived(&expanded));
    assert_eq!(
        resolutions(g),
        BTreeSet::from([(expanded.into(), d.into())])
    );
}

#[test]
fn view_does_not_see_its_previous_output() {
    // Counts every node, its own output included once written.
    let mut db = Database::new(Graph::default());
    db.register_view("count", None).unwrap();
    let run = |_: &str, _: Option<EntityId>, g: &mut ViewGraph| {
        let seen = g.iter_nodes().len() as i128;
        g.add_node(obj(&[("count", seen.into())]));
        Ok(())
    };
    let count = |db: &Database| {
        let g = db.graph();
        let node = g.iter_nodes().find(crate::views::is_derived).unwrap();
        g.obj(&node).unwrap()["count"].clone()
    };

    db.graph_mut().add_node(def("a"));
    db.propagate_with(run).unwrap();
    assert_eq!(count(&db), Field::Number(1));

    db.graph_mut().add_node(def("b"));
    let report = db.propagate_with(run).unwrap();
    assert_eq!(report.runs.len(), 1);
    assert_eq!(count(&db), Field::Number(2));
}

#[test]
fn failed_run_keeps_its_output_and_is_retried() {
    let mut db = Database::new(Graph::default());
    db.register_view("fragile", Some(Pattern::nodes())).unwrap();
    let run = |_: &str, key: Option<EntityId>, g: &mut ViewGraph| {
        let key = key.unwrap();
        let edge = g.add_edge(key, key).map_err(err)?;
        match g.field(&key, "ok") {
            Some(Field::Bool(true)) => Ok(()),
            _ => {
                g.attach_obj(edge, obj(&[("broken", true.into())]))
                    .map_err(err)?;
                Err("not ok".to_string())
            }
        }
    };

    let n = db.graph_mut().add_node(obj(&[("ok", false.into())]));
    let report = db.propagate_with(run).unwrap();
    assert_eq!(report.failures.len(), 1);
    let edge = db.graph().out_edges(&n.into()).next().unwrap();
    assert!(db.graph().obj(&edge).is_some());

    db.graph_mut()
        .replace_node(&n, obj(&[("ok", true.into())]))
        .unwrap();
    let report = db.propagate_with(run).unwrap();
    assert_eq!(report.failures, vec![]);
    assert_eq!(
        db.graph().out_edges(&n.into()).collect::<Vec<_>>(),
        vec![edge]
    );
    assert_eq!(db.graph().obj(&edge), None);
}

#[test]
fn duplicate_view_names_are_rejected() {
    let mut db = Database::new(Graph::default());
    db.register_view("v", None).unwrap();
    assert_eq!(
        db.register_view("v", None),
        Err(RegisterViewError::Duplicate { name: "v".into() })
    );
}
