use super::*;

/// Attaching to an unknown id is rejected — no patch recorded.
#[test]
fn unknown_target() {
    let mut g = Graph::default();
    let err = g
        .attach_obj(Uuid::new_v4(), test_utils::create_simple_obj("f"))
        .unwrap_err();
    assert!(matches!(err, AttachObjectError::AttachTargetNotFound(_)));
    assert!(g.events.is_empty());
}

/// A node is not an attach target.
#[test]
fn rejects_node() {
    let mut g = Graph::default();
    let n1 = g.add_node(test_utils::create_simple_obj("f"));
    let err = g
        .attach_obj(n1, test_utils::create_simple_obj("g"))
        .unwrap_err();
    assert!(matches!(err, AttachObjectError::IncorrectType(_)));
    let extra_events = g
        .events
        .iter()
        .filter(|p| {
            matches!(p, Patch::UpsertEdgeData { .. } | Patch::UpsertHyperEdgeData { .. })
        })
        .count();
    assert_eq!(extra_events, 0);
}

/// Re-attaching is rejected (target is already AttachedObject).
#[test]
fn rejects_double_attach() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let e = g.add_edge(n1, n2).unwrap();
    g.attach_obj(e, obj.clone()).unwrap();

    let err = g.attach_obj(e, obj).unwrap_err();
    assert!(matches!(err, AttachObjectError::IncorrectType(_)));
    let count = g
        .events
        .iter()
        .filter(|p| matches!(p, Patch::UpsertEdgeData { .. }))
        .count();
    assert_eq!(count, 1, "second attach must NOT have recorded a patch");
}

/// Attach onto a plain edge → records UpsertEdgeData.
#[test]
fn records_upsert_edge_data() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let e = g.add_edge(n1, n2).unwrap();
    let attached = test_utils::create_simple_obj("data");

    g.attach_obj(e, attached.clone()).unwrap();

    assert_eq!(
        *g.events.last().unwrap(),
        Patch::UpsertEdgeData {
            id: e,
            obj: attached,
        }
    );
}

/// Attach onto a hyperedge → records UpsertHyperEdgeData.
#[test]
fn records_upsert_hyperedge_data() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let mut m = HashSet::new();
    m.insert(n1.into());
    let h = g.create_hyperedge(m).unwrap();
    let attached = test_utils::create_simple_obj("data");

    g.attach_obj(h, attached.clone()).unwrap();

    assert_eq!(
        *g.events.last().unwrap(),
        Patch::UpsertHyperEdgeData {
            id: h,
            obj: attached,
        }
    );
}

/// Attach onto a meta-edge (edge whose endpoint is another
/// edge) — still recorded as UpsertEdgeData since meta-edges
/// live in `self.edges`.
#[test]
fn meta_edge_records_upsert_edge_data() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("f");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());
    let e1 = g.add_edge(n1, n2).unwrap();
    let n3 = g.add_node(obj.clone());
    let meta = g.add_edge(n3, e1).unwrap();
    let attached = test_utils::create_simple_obj("data");

    g.attach_obj(meta, attached.clone()).unwrap();

    assert_eq!(
        *g.events.last().unwrap(),
        Patch::UpsertEdgeData {
            id: meta,
            obj: attached,
        }
    );
}
