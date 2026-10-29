//! Exercises every `sdk::Graph` method across the guest ↔ host
//! boundary. Each view checks its own area with `assert!`: a failed
//! check traps, and the host reports which `__cog_view_*` trapped.
//! Views only look at entities they created themselves, except
//! `ownership_view`, which checks that another view's node can be
//! pointed at but not changed.

use std::collections::HashSet;

use sdk::{
    AddEdgeError, AddHyperedgeMembersError, AttachObjectError, CreateHyperedgeError,
    EdgeNotFoundError, EdgeView, EntityId, Field, GetEdgeError, Graph, HyperedgeNotFoundError,
    NoAttachedObjectError, NodeNotFoundError, Object, Pointee, PointeeKind,
    RemoveHyperedgeMembersError, RetargetEdge, RetargetError, view,
};

fn named(name: &str) -> Object {
    let mut obj = Object::new();
    obj.insert("name".to_string(), Field::String(name.to_string()));
    obj
}

fn set(ids: &[EntityId]) -> HashSet<Pointee> {
    ids.iter().map(|id| Pointee::from(*id)).collect()
}

fn kind(g: &Graph, id: EntityId) -> Option<PointeeKind> {
    g.classify_pointee(&id.into())
}

/// Round trip checked from the host side: the outcome is written
/// into a `report` node that the jit test inspects.
#[view]
fn roundtrip_view(g: &mut Graph) {
    let a = g.add_node(named("a"));
    let b = g.add_node(named("b"));

    let edge = g.add_edge(a, b).expect("both endpoints exist");

    let echoed = g.obj(a).expect("node a carries an object");
    let missing_obj = g.obj(EntityId::nil()).is_none();
    let missing_edge = g.add_edge(a, EntityId::nil()) == Err(AddEdgeError::MissingEndpoints);

    let mut report = Object::new();
    report.insert("edge".to_string(), Field::Link(edge.into()));
    report.insert("echo".to_string(), echoed["name"].clone());
    report.insert("missing_obj".to_string(), Field::Bool(missing_obj));
    report.insert("missing_edge".to_string(), Field::Bool(missing_edge));
    g.add_node(report);
}

#[view]
fn nodes_view(g: &mut Graph) {
    let n = g.add_node(named("n"));
    assert!(g.is_exist(n));
    assert!(g.is_pointee_exist(&n.into()));
    assert_eq!(kind(g, n), Some(PointeeKind::Node));
    assert!(g.iter_nodes().contains(&n));
    assert!(g.iter_entities().contains(&n));
    assert_eq!(g.obj(n), Some(named("n")));

    assert_eq!(
        g.replace_node(n, named("n2")),
        Ok(Field::Object(named("n")))
    );
    assert_eq!(g.obj(n), Some(named("n2")));

    assert_eq!(g.remove_node(n), Ok(named("n2")));
    assert!(!g.is_exist(n));
    assert_eq!(kind(g, n), None);
    assert_eq!(g.remove_node(n), Err(NodeNotFoundError));
    assert_eq!(g.replace_node(n, named("x")), Err(NodeNotFoundError));
}

#[view]
fn edges_view(g: &mut Graph) {
    let a = g.add_node(named("a"));
    let b = g.add_node(named("b"));
    let c = g.add_node(named("c"));

    let e = g.add_edge(a, b).unwrap();
    assert!(g.iter_edges().contains(&e));
    assert_eq!(kind(g, e), Some(PointeeKind::Edge));
    assert_eq!(
        g.edge(e),
        Ok(EdgeView {
            id: e,
            source: a.into(),
            target: b.into()
        })
    );
    assert_eq!(g.edge(a), Err(GetEdgeError::IncorrectType));
    assert_eq!(g.edge(EntityId::nil()), Err(GetEdgeError::NotFound));

    g.retarget_edge(e, RetargetEdge::Target(c.into())).unwrap();
    assert_eq!(g.edge(e).unwrap().target, c.into());
    assert_eq!(
        g.retarget_edge(e, RetargetEdge::Source(EntityId::nil().into())),
        Err(RetargetError::InvalidTarget)
    );
    assert_eq!(
        g.retarget_edge(EntityId::nil(), RetargetEdge::Source(a.into())),
        Err(RetargetError::EdgeNotFound)
    );

    let meta = g.add_edge(e, a).unwrap();
    assert_eq!(kind(g, meta), Some(PointeeKind::MetaEdge));

    // Removing `c` cascades to `e` (targets `c`) and to `meta`
    // (references `e`).
    g.remove_node(c).unwrap();
    assert!(!g.is_exist(e));
    assert!(!g.is_exist(meta));
    assert_eq!(g.remove_edge(e), Err(EdgeNotFoundError));

    let f = g.add_edge(a, b).unwrap();
    assert_eq!(
        g.remove_edge(f),
        Ok(EdgeView {
            id: f,
            source: a.into(),
            target: b.into()
        })
    );
}

#[view]
fn hyperedges_view(g: &mut Graph) {
    let a = g.add_node(named("a"));
    let b = g.add_node(named("b"));
    let c = g.add_node(named("c"));

    assert_eq!(
        g.create_hyperedge(HashSet::new()),
        Err(CreateHyperedgeError::EmptyHyperedge)
    );
    assert_eq!(
        g.create_hyperedge(set(&[a, EntityId::nil()])),
        Err(CreateHyperedgeError::PointeesNotFound)
    );

    let h = g.create_hyperedge(set(&[a, b])).unwrap();
    assert!(g.iter_hyperedges().contains(&h));
    assert_eq!(kind(g, h), Some(PointeeKind::Hyperedge));
    assert_eq!(g.hyperedge_members(h), Some(set(&[a, b])));

    g.add_hyperedge_members(h, set(&[c])).unwrap();
    assert_eq!(
        g.add_hyperedge_members(h, set(&[c])),
        Err(AddHyperedgeMembersError::MembersAlreadyExist)
    );
    assert_eq!(
        g.add_hyperedge_members(EntityId::nil(), set(&[c])),
        Err(AddHyperedgeMembersError::HyperedgeNotFound)
    );

    g.remove_hyperedge_members(h, set(&[a])).unwrap();
    assert_eq!(
        g.remove_hyperedge_members(h, set(&[a])),
        Err(RemoveHyperedgeMembersError::MembersNotInHyperedge)
    );
    assert_eq!(g.hyperedge_members(h), Some(set(&[b, c])));

    assert_eq!(g.remove_hyperedge(h), Ok(set(&[b, c])));
    assert_eq!(g.hyperedge_members(h), None);
    assert_eq!(g.remove_hyperedge(h), Err(HyperedgeNotFoundError));
}

#[view]
fn attached_view(g: &mut Graph) {
    let a = g.add_node(named("a"));
    let b = g.add_node(named("b"));
    let e = g.add_edge(a, b).unwrap();

    assert_eq!(g.obj(e), None);
    g.attach_obj(e, named("e1")).unwrap();
    assert_eq!(g.obj(e), Some(named("e1")));
    assert_eq!(kind(g, e), Some(PointeeKind::AttachedObject));
    assert!(g.iter_attached().contains(&e));

    assert_eq!(
        g.attach_obj(e, named("again")),
        Err(AttachObjectError::IncorrectType)
    );
    assert_eq!(
        g.attach_obj(a, named("node")),
        Err(AttachObjectError::IncorrectType)
    );
    assert_eq!(
        g.attach_obj(EntityId::nil(), named("nil")),
        Err(AttachObjectError::AttachTargetNotFound)
    );

    assert_eq!(
        g.replace_attached_obj(e, named("e2")),
        Ok(Field::Object(named("e1")))
    );
    assert_eq!(g.obj(e), Some(named("e2")));

    g.remove_attached(e).unwrap();
    assert_eq!(g.obj(e), None);
    assert_eq!(g.remove_attached(e), Err(NoAttachedObjectError));
    assert_eq!(
        g.replace_attached_obj(e, named("e3")),
        Err(NoAttachedObjectError)
    );
}

#[view]
fn neighbourhood_view(g: &mut Graph) {
    let mut tagged = named("tagged");
    tagged.insert(
        "tag".to_string(),
        Field::String("neighbourhood".to_string()),
    );
    let a = g.add_node(tagged);
    let b = g.add_node(named("b"));
    let e = g.add_edge(a, b).unwrap();
    let h = g.create_hyperedge(set(&[a])).unwrap();

    assert_eq!(
        g.field(a, "name"),
        Some(Field::String("tagged".to_string()))
    );
    assert_eq!(g.field(a, "missing"), None);
    assert_eq!(g.out_edges(a), vec![e]);
    assert_eq!(g.in_edges(b), vec![e]);
    assert_eq!(g.out_edges(b), Vec::<EntityId>::new());
    assert_eq!(g.hyperedges_of(a), vec![h]);
    assert_eq!(g.nodes_where("tag", "neighbourhood"), vec![a]);
}

/// Views own what they create, and nothing else.
#[view]
fn ownership_view(g: &mut Graph) {
    // Until `neighbourhood_view` runs there is nothing to look at;
    // its node re-runs this view.
    let Some(&foreign) = g.nodes_where("name", "tagged").first() else {
        return;
    };
    assert_eq!(g.remove_node(foreign), Err(NodeNotFoundError));
    assert_eq!(g.replace_node(foreign, named("x")), Err(NodeNotFoundError));

    // Pointing at someone else's entity is fine.
    let own = g.add_node(named("own"));
    g.add_edge(own, foreign).unwrap();
}
