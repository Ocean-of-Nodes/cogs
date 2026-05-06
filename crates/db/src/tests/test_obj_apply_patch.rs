use super::*;

fn obj_with(fields: &[(&str, Field)]) -> Object {
    let mut o = Object::new();
    for (k, v) in fields {
        o.insert((*k).into(), v.clone());
    }
    o
}

#[test]
fn unknown_id() {
    let mut g = Graph::default();
    let err = g
        .obj_apply_patch(Uuid::new_v4(), vec![])
        .unwrap_err();
    assert!(matches!(err, DeltaError::NotFound(_)));
}

#[test]
fn empty_patch_is_noop() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("a", Field::Number(1))]));
    g.obj_apply_patch(n1, vec![]).unwrap();
    assert_eq!(g.obj(&n1), Some(&obj_with(&[("a", Field::Number(1))])));
    test_utils::check_index_invariant(&g);
}

#[test]
fn add_field_on_fresh_key() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[]));
    g.obj_apply_patch(
        n1,
        vec![ObjectPatch::AddField {
            name: "x".into(),
            field: Field::Number(7),
        }],
    )
    .unwrap();
    assert_eq!(g.obj(&n1).unwrap().get("x"), Some(&Field::Number(7)));
}

#[test]
fn add_field_on_existing_key_errors() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("x", Field::Number(1))]));
    let err = g
        .obj_apply_patch(
            n1,
            vec![ObjectPatch::AddField {
                name: "x".into(),
                field: Field::Number(7),
            }],
        )
        .unwrap_err();
    assert!(matches!(
        err,
        DeltaError::Delta(ObjectPatchError::FieldAlreadyExists { .. })
    ));
}

#[test]
fn remove_field_existing() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("x", Field::Number(1)), ("y", Field::Null)]));
    g.obj_apply_patch(
        n1,
        vec![ObjectPatch::RemoveField { name: "x".into() }],
    )
    .unwrap();
    assert!(!g.obj(&n1).unwrap().contains_key("x"));
    assert!(g.obj(&n1).unwrap().contains_key("y"));
}

#[test]
fn remove_field_missing_errors() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[]));
    let err = g
        .obj_apply_patch(n1, vec![ObjectPatch::RemoveField { name: "x".into() }])
        .unwrap_err();
    assert!(matches!(
        err,
        DeltaError::Delta(ObjectPatchError::FieldNotFound { .. })
    ));
}

#[test]
fn upsert_field_inserts_and_replaces() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("x", Field::Number(1))]));
    g.obj_apply_patch(
        n1,
        vec![
            ObjectPatch::UpsertField {
                name: "x".into(),
                field: Field::Number(99),
            },
            ObjectPatch::UpsertField {
                name: "y".into(),
                field: Field::Null,
            },
        ],
    )
    .unwrap();
    assert_eq!(g.obj(&n1).unwrap().get("x"), Some(&Field::Number(99)));
    assert_eq!(g.obj(&n1).unwrap().get("y"), Some(&Field::Null));
}

#[test]
fn array_patch_adds_and_removes() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[(
        "arr",
        Field::Array(vec![
            Field::Number(1),
            Field::Number(2),
            Field::Number(3),
        ]),
    )]));
    g.obj_apply_patch(
        n1,
        vec![ObjectPatch::ArrayPatch {
            name: "arr".into(),
            removed_indices: vec![0],
            added_fields: vec![(2, Field::Number(99))],
        }],
    )
    .unwrap();
    // After remove(0): [2, 3]; after insert at 2: [2, 3, 99].
    assert_eq!(
        g.obj(&n1).unwrap().get("arr"),
        Some(&Field::Array(vec![
            Field::Number(2),
            Field::Number(3),
            Field::Number(99),
        ]))
    );
}

#[test]
fn array_patch_on_non_array_errors() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("x", Field::Number(1))]));
    let err = g
        .obj_apply_patch(
            n1,
            vec![ObjectPatch::ArrayPatch {
                name: "x".into(),
                removed_indices: vec![],
                added_fields: vec![],
            }],
        )
        .unwrap_err();
    assert!(matches!(
        err,
        DeltaError::Delta(ObjectPatchError::NotAnArray { .. })
    ));
}

#[test]
fn array_patch_index_out_of_bounds() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("arr", Field::Array(vec![Field::Number(1)]))]));
    let err = g
        .obj_apply_patch(
            n1,
            vec![ObjectPatch::ArrayPatch {
                name: "arr".into(),
                removed_indices: vec![5],
                added_fields: vec![],
            }],
        )
        .unwrap_err();
    assert!(matches!(
        err,
        DeltaError::Delta(ObjectPatchError::IndexOutOfBounds { index: 5 })
    ));
}

#[test]
fn sub_object_patch_navigates_and_applies() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[(
        "inner",
        Field::Object(obj_with(&[("a", Field::Number(1))])),
    )]));
    g.obj_apply_patch(
        n1,
        vec![ObjectPatch::SubObjectPatch {
            path: LocalObjPath::new("inner").unwrap(),
            delta: vec![ObjectPatch::UpsertField {
                name: "a".into(),
                field: Field::Number(99),
            }],
        }],
    )
    .unwrap();
    match g.obj(&n1).unwrap().get("inner") {
        Some(Field::Object(inner)) => {
            assert_eq!(inner.get("a"), Some(&Field::Number(99)));
        }
        _ => panic!("inner should be an Object"),
    }
}

#[test]
fn sub_object_patch_through_non_object_errors() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("inner", Field::Number(1))]));
    let err = g
        .obj_apply_patch(
            n1,
            vec![ObjectPatch::SubObjectPatch {
                path: LocalObjPath::new("inner").unwrap(),
                delta: vec![],
            }],
        )
        .unwrap_err();
    assert!(matches!(
        err,
        DeltaError::Delta(ObjectPatchError::NotAnObject { .. })
    ));
}

#[test]
fn cascades_path_pointees_after_field_removal() {
    let mut g = Graph::default();
    let n1 = g.add_node(obj_with(&[("data", Field::Number(1))]));
    let n2 = g.add_node(obj_with(&[("a", Field::Null)]));
    let path = Pointee::Path(GlobalObjPath::new(n1, "data").unwrap());
    let e = g.add_edge(n2, path.clone()).unwrap();

    g.obj_apply_patch(n1, vec![ObjectPatch::RemoveField { name: "data".into() }])
        .unwrap();

    // The Pointee::Path no longer resolves → cascade kills the edge.
    assert!(!g.edges.contains_key(&e));
    assert!(!g.pointee_uses.contains_key(&path));
    assert!(g.is_exist(&n1));
    test_utils::check_index_invariant(&g);
}
