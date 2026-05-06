use super::*;

fn obj_with(fields: &[(&str, Field)]) -> Object {
    let mut o = Object::new();
    for (k, v) in fields {
        o.insert((*k).into(), v.clone());
    }
    o
}

/// Unknown id is rejected.
#[test]
fn unknown_id() {
    let mut g = Graph::default();
    let err = g
        .replace_attached_obj(&Uuid::new_v4(), obj_with(&[]))
        .unwrap_err();
    assert!(matches!(err, NoAttachedObjectError { .. }));
}

/// A node is not an attach target.
#[test]
fn rejects_node() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("a", Field::Null)]));
    let err = g.replace_attached_obj(&n1, obj_with(&[])).unwrap_err();
    assert!(matches!(err, NoAttachedObjectError { .. }));
}

/// A bare edge (no attach_obj called) — rejected.
#[test]
fn rejects_bare_edge() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("a", Field::Null)]));
    let n2 = g.add_node(obj_with(&[("a", Field::Null)]));
    let e = g.add_edge(n1, n2).unwrap();

    let err = g.replace_attached_obj(&e, obj_with(&[])).unwrap_err();
    assert!(matches!(err, NoAttachedObjectError { .. }));
}

/// A bare hyperedge — rejected.
#[test]
fn rejects_bare_hyperedge() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("a", Field::Null)]));
    let mut m = HashSet::new();
    m.insert(n1.into());
    let h = g.create_hyperedge(m).unwrap();

    let err = g.replace_attached_obj(&h, obj_with(&[])).unwrap_err();
    assert!(matches!(err, NoAttachedObjectError { .. }));
}

/// Returns the previous attached object.
#[test]
fn returns_old_object() {
    let mut g = Graph::default();
    let obj = obj_with(&[("a", Field::Null)]);
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj);
    let e = g.add_edge(n1, n2).unwrap();
    let old = obj_with(&[("data", Field::Number(1))]);
    g.attach_obj(e, old.clone()).unwrap();

    let returned = g
        .replace_attached_obj(&e, obj_with(&[("data", Field::Number(2))]))
        .unwrap();
    assert_eq!(returned, Field::Object(old));
}

/// Edge with small delta → ChangeEdgeData.
#[test]
fn edge_small_delta_emits_change() {
    let mut g = Graph::default();
    let leaf = obj_with(&[("a", Field::Null)]);
    let n1 = g.add_node(leaf.clone());
    let n2 = g.add_node(leaf);
    let e = g.add_edge(n1, n2).unwrap();
    g.attach_obj(
        e,
        obj_with(&[
            ("a", Field::Number(1)),
            ("b", Field::Number(2)),
            ("c", Field::Number(3)),
        ]),
    )
    .unwrap();

    g.replace_attached_obj(
        &e,
        obj_with(&[
            ("a", Field::Number(1)),
            ("b", Field::Number(2)),
            ("c", Field::Number(99)),
        ]),
    )
    .unwrap();

    match g.events.last().unwrap() {
        Patch::ChangeEdgeData { id, delta } => {
            assert_eq!(*id, e);
            assert_eq!(delta.len(), 1);
        }
        other => panic!("expected ChangeEdgeData, got {:?}", other),
    }
}

/// Edge with large delta → UpsertEdgeData.
#[test]
fn edge_large_delta_emits_upsert() {
    let mut g = Graph::default();
    let leaf = obj_with(&[("a", Field::Null)]);
    let n1 = g.add_node(leaf.clone());
    let n2 = g.add_node(leaf);
    let e = g.add_edge(n1, n2).unwrap();
    g.attach_obj(
        e,
        obj_with(&[
            ("a", Field::Number(1)),
            ("b", Field::Number(2)),
            ("c", Field::Number(3)),
        ]),
    )
    .unwrap();

    let new = obj_with(&[
        ("x", Field::Number(10)),
        ("y", Field::Number(20)),
        ("z", Field::Number(30)),
    ]);
    g.replace_attached_obj(&e, new.clone()).unwrap();

    assert_eq!(
        *g.events.last().unwrap(),
        Patch::UpsertEdgeData { id: e, obj: new }
    );
}

/// Hyperedge with small delta → ChangeHyperEdgeData.
#[test]
fn hyperedge_small_delta_emits_change() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("x", Field::Null)]));
    let mut m = HashSet::new();
    m.insert(n1.into());
    let h = g.create_hyperedge(m).unwrap();
    g.attach_obj(
        h,
        obj_with(&[("a", Field::Number(1)), ("b", Field::Number(2))]),
    )
    .unwrap();

    g.replace_attached_obj(
        &h,
        obj_with(&[("a", Field::Number(1)), ("b", Field::Number(99))]),
    )
    .unwrap();

    match g.events.last().unwrap() {
        Patch::ChangeHyperEdgeData { id, delta } => {
            assert_eq!(*id, h);
            assert_eq!(delta.len(), 1);
        }
        other => panic!("expected ChangeHyperEdgeData, got {:?}", other),
    }
}

/// Hyperedge with large delta → UpsertHyperEdgeData.
#[test]
fn hyperedge_large_delta_emits_upsert() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("x", Field::Null)]));
    let mut m = HashSet::new();
    m.insert(n1.into());
    let h = g.create_hyperedge(m).unwrap();
    g.attach_obj(
        h,
        obj_with(&[
            ("a", Field::Number(1)),
            ("b", Field::Number(2)),
            ("c", Field::Number(3)),
        ]),
    )
    .unwrap();

    let new = obj_with(&[
        ("x", Field::Number(10)),
        ("y", Field::Number(20)),
        ("z", Field::Number(30)),
    ]);
    g.replace_attached_obj(&h, new.clone()).unwrap();

    assert_eq!(
        *g.events.last().unwrap(),
        Patch::UpsertHyperEdgeData { id: h, obj: new }
    );
}

/// Path-pointee through the attach target gets cascaded if
/// the field it pointed at is dropped by the replacement.
#[test]
fn path_pointee_cascades_when_field_dropped() {
    let mut g = Graph::default();
    let leaf = obj_with(&[("a", Field::Null)]);
    let n1 = g.add_node(leaf.clone());
    let n2 = g.add_node(leaf);
    let e = g.add_edge(n1, n2).unwrap();
    g.attach_obj(e, obj_with(&[("data", Field::Number(1))]))
        .unwrap();

    let n3 = g.add_node(obj_with(&[("a", Field::Null)]));
    let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
    let dangling = g.add_edge(n3, path.clone()).unwrap();

    g.replace_attached_obj(&e, obj_with(&[("other", Field::Null)]))
        .unwrap();

    assert!(g.edges.contains_key(&e));
    assert!(!g.edges.contains_key(&dangling));
    assert!(!g.entity_to_path_pointees.contains_key(&e));
    test_utils::check_index_invariant(&g);
}

/// Path-pointee survives when the referenced field is kept.
#[test]
fn path_pointee_survives_when_field_kept() {
    let mut g = Graph::default();
    let leaf = obj_with(&[("a", Field::Null)]);
    let n1 = g.add_node(leaf.clone());
    let n2 = g.add_node(leaf);
    let e = g.add_edge(n1, n2).unwrap();
    g.attach_obj(e, obj_with(&[("data", Field::Number(1))]))
        .unwrap();

    let n3 = g.add_node(obj_with(&[("a", Field::Null)]));
    let path = Pointee::Path(GlobalObjPath::new(e, "data").unwrap());
    let kept = g.add_edge(n3, path.clone()).unwrap();

    g.replace_attached_obj(&e, obj_with(&[("data", Field::Number(99))]))
        .unwrap();

    assert!(g.edges.contains_key(&kept));
    assert!(g.pointee_uses.contains_key(&path));
    test_utils::check_index_invariant(&g);
}
