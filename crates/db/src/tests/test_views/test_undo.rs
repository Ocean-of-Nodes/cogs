use super::*;

#[test]
fn undo_of_an_input_brings_derived_facts_back() {
    let mut db = compiler(Graph::default());
    db.graph_mut().add_node(def("foo"));
    let u_bar = db.graph_mut().add_node(use_site("bar"));
    propagate(&mut db);
    let before = content(db.graph());

    let d_bar = db.graph_mut().add_node(def("bar"));
    propagate(&mut db);
    assert_eq!(
        resolutions(db.graph()),
        BTreeSet::from([(u_bar.into(), d_bar.into())])
    );

    db.graph_mut()
        .undo()
        .unwrap()
        .expect("the add_node is logged");
    propagate(&mut db);
    assert_eq!(content(db.graph()), before);
}

#[test]
fn undo_does_not_restore_derived_entities_itself() {
    let mut db = compiler(Graph::default());
    let d_foo = db.graph_mut().add_node(def("foo"));
    let u_foo = db.graph_mut().add_node(use_site("foo"));
    propagate(&mut db);
    let before = content(db.graph());
    let edge = db.graph().out_edges(&u_foo.into()).next().unwrap();

    // The cascade takes the derived edge and the group's member.
    db.graph_mut().remove_node(&d_foo).unwrap();
    propagate(&mut db);
    assert!(!db.graph().is_exist(&edge));

    db.graph_mut().undo().unwrap();
    assert!(db.graph().is_exist(&d_foo));
    assert!(!db.graph().is_exist(&edge), "views recreate it, not undo");
    test_utils::check_index_invariant(db.graph());

    propagate(&mut db);
    assert_eq!(content(db.graph()), before);
}

#[test]
fn undo_cascading_into_derived_entities_reruns_their_owners() {
    let mut db = compiler(Graph::default());
    let u_foo = db.graph_mut().add_node(use_site("foo"));
    propagate(&mut db);
    let before = content(db.graph());

    // Undoing this add removes the definition, and the cascade takes
    // the derived edge to it.
    db.graph_mut().add_node(def("foo"));
    propagate(&mut db);
    assert_eq!(resolutions(db.graph()).len(), 1);

    db.graph_mut().undo().unwrap();
    propagate(&mut db);
    assert_eq!(content(db.graph()), before);
    assert_eq!(diagnosed(db.graph()), BTreeSet::from([u_foo.into()]));
}
