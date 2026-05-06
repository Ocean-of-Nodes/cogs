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
        .replace_node(&Uuid::new_v4(), test_utils::create_simple_obj("f"))
        .unwrap_err();
    assert!(matches!(err, NodeNotFoundError { .. }));
}

/// Edges (with attached object) are not nodes — rejected.
#[test]
fn rejects_attached_target() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let e = g.add_edge(n1, n2).unwrap();
    g.attach_obj(e, obj.clone()).unwrap();

    let err = g.replace_node(&e, obj).unwrap_err();
    assert!(matches!(err, NodeNotFoundError { .. }));
}

/// Returns the previous object (wrapped in `Field::Object`).
#[test]
fn returns_old_object() {
    let mut g = Graph::default();
    let old = obj_with(&[("a", Field::Number(1))]);
    let n1 = g.add_node(old.clone());
    let new = obj_with(&[("a", Field::Number(2))]);

    let returned = g.replace_node(&n1, new).unwrap();
    assert_eq!(returned, Field::Object(old));
}

/// Few changed fields (delta_size <= new_size) → ChangeNode.
#[test]
fn small_delta_emits_change_node() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[
        ("a", Field::Number(1)),
        ("b", Field::Number(2)),
        ("c", Field::Number(3)),
    ]));

    let new = obj_with(&[
        ("a", Field::Number(1)),
        ("b", Field::Number(2)),
        ("c", Field::Number(99)), // only c changed
    ]);
    g.replace_node(&n1, new).unwrap();

    match g.events.last().unwrap() {
        Patch::ChangeNode { id, delta } => {
            assert_eq!(*id, n1);
            assert_eq!(delta.len(), 1);
            assert!(matches!(
                &delta[0],
                ObjectPatch::UpsertField { name, .. } if name == "c"
            ));
        }
        other => panic!("expected ChangeNode, got {:?}", other),
    }
}

/// Many changes (delta_size > new_size) → UpsertNode.
/// Old: {a,b,c}, New: {x,y,z} → delta would be 6 ops, new has 3 fields.
#[test]
fn large_delta_emits_upsert_node() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[
        ("a", Field::Number(1)),
        ("b", Field::Number(2)),
        ("c", Field::Number(3)),
    ]));

    let new = obj_with(&[
        ("x", Field::Number(10)),
        ("y", Field::Number(20)),
        ("z", Field::Number(30)),
    ]);
    g.replace_node(&n1, new.clone()).unwrap();

    assert_eq!(
        *g.events.last().unwrap(),
        Patch::UpsertNode { id: n1, obj: new }
    );
}

/// Boundary `delta_size == new_size` — still ChangeNode (`<=`).
#[test]
fn equal_size_emits_change_node() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[
        ("a", Field::Number(1)),
        ("b", Field::Number(2)),
    ]));

    // Both fields changed → 2 UpsertField ops, new has 2 fields.
    let new = obj_with(&[
        ("a", Field::Number(10)),
        ("b", Field::Number(20)),
    ]);
    g.replace_node(&n1, new).unwrap();

    assert!(matches!(
        g.events.last().unwrap(),
        Patch::ChangeNode { .. }
    ));
}

/// Identical replacement: delta is empty → ChangeNode with empty delta.
#[test]
fn identical_obj_emits_empty_change_node() {
    let mut g = Graph::default();
    let obj = obj_with(&[("a", Field::Number(1))]);
    let n1 = g.add_node(obj.clone());

    g.replace_node(&n1, obj).unwrap();

    match g.events.last().unwrap() {
        Patch::ChangeNode { id, delta } => {
            assert_eq!(*id, n1);
            assert!(delta.is_empty());
        }
        other => panic!("expected ChangeNode, got {:?}", other),
    }
}

/// Path-pointee that still resolves under the new object survives.
#[test]
fn path_pointee_survives_when_field_kept() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("data", Field::Number(1))]));
    let n2 = g.add_node(obj_with(&[("x", Field::Null)]));
    let path = Pointee::Path(GlobalObjPath::new(n1, "data").unwrap());
    let e = g.add_edge(n2, path.clone()).unwrap();

    // Replace but keep `data` field.
    g.replace_node(&n1, obj_with(&[("data", Field::Number(99))]))
        .unwrap();

    assert!(g.edges.contains_key(&e));
    assert!(g.pointee_uses.contains_key(&path));
    test_utils::check_index_invariant(&g);
}

/// Path-pointee that no longer resolves cascades away.
#[test]
fn path_pointee_cascades_when_field_dropped() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("data", Field::Number(1))]));
    let n2 = g.add_node(obj_with(&[("x", Field::Null)]));
    let path = Pointee::Path(GlobalObjPath::new(n1, "data").unwrap());
    let e = g.add_edge(n2, path.clone()).unwrap();

    // Replace with an object that lacks `data`.
    g.replace_node(&n1, obj_with(&[("other", Field::Null)]))
        .unwrap();

    assert!(!g.edges.contains_key(&e));
    assert!(!g.pointee_uses.contains_key(&path));
    assert!(!g.entity_to_path_pointees.contains_key(&n1));
    // Node itself still alive.
    assert!(g.is_exist(&n1));
    test_utils::check_index_invariant(&g);
}

/// EntityId references survive — only path references can die.
#[test]
fn entity_id_references_survive() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("a", Field::Null)]));
    let n2 = g.add_node(obj_with(&[("b", Field::Null)]));
    let e = g.add_edge(n2, n1).unwrap();

    g.replace_node(&n1, obj_with(&[("c", Field::Null)]))
        .unwrap();

    assert!(g.edges.contains_key(&e));
    test_utils::check_index_invariant(&g);
}
