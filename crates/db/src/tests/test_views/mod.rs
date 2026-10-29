//! Tests of the view engine. Views here are plain Rust functions
//! run through [`Database::propagate_with`]; the "compiler" below
//! is the running example: definitions and uses linked by name.

use std::collections::BTreeSet;

use super::*;

mod test_cycles;
mod test_ids;
mod test_layers;
mod test_propagate;
mod test_scratch;
mod test_undo;
mod test_view_graph;

pub fn obj(fields: &[(&str, Field)]) -> Object {
    fields
        .iter()
        .map(|(name, value)| (name.to_string(), value.clone()))
        .collect()
}

pub fn def(name: &str) -> Object {
    obj(&[("kind", "def".into()), ("name", name.into())])
}

pub fn use_site(name: &str) -> Object {
    obj(&[("kind", "use".into()), ("name", name.into())])
}

fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}

/// `for_each` use: link the use to every definition with its name.
pub fn resolve(g: &mut ViewGraph, use_id: NodeId) -> Result<(), String> {
    let Some(name) = g.field(&use_id, "name") else {
        return Ok(());
    };
    for candidate in g.nodes_where("name", &name) {
        if g.field(&candidate, "kind") == Some(Field::from("def")) {
            let edge = g.add_edge(use_id, candidate).map_err(err)?;
            g.attach_obj(edge, obj(&[("rel", "resolves_to".into())]))
                .map_err(err)?;
        }
    }
    Ok(())
}

/// `for_each` use: a diagnostic for a use that resolves to nothing.
pub fn unresolved(g: &mut ViewGraph, use_id: NodeId) -> Result<(), String> {
    let out = g.out_edges(&use_id.into());
    let resolved = out
        .iter()
        .any(|e| g.field(e, "rel") == Some(Field::from("resolves_to")));
    if resolved {
        return Ok(());
    }
    let name = g.field(&use_id, "name").unwrap_or(Field::Null);
    let diagnostic = g.add_node(obj(&[
        ("kind", "error".into()),
        ("message", format!("unresolved {name:?}").into()),
    ]));
    g.add_edge(diagnostic, use_id).map_err(err)?;
    Ok(())
}

/// No key: one node counting the diagnostics.
pub fn summary(g: &mut ViewGraph) -> Result<(), String> {
    let errors = g.nodes_where("kind", &"error".into()).len() as i128;
    g.add_node(obj(&[
        ("kind", "summary".into()),
        ("errors", errors.into()),
    ]));
    Ok(())
}

/// `for_each` def: a hyperedge grouping the definition with the
/// uses that resolve to it.
pub fn group(g: &mut ViewGraph, def_id: NodeId) -> Result<(), String> {
    let mut members = HashSet::from([Pointee::from(def_id)]);
    for edge in g.in_edges(&def_id.into()) {
        if g.field(&edge, "rel") == Some(Field::from("resolves_to")) {
            members.insert(g.edge(&edge).map_err(err)?.source);
        }
    }
    g.create_hyperedge(members).map_err(err)?;
    Ok(())
}

/// A database with the compiler's views registered.
pub fn compiler(graph: Graph) -> Database {
    let mut db = Database::new(graph);
    let uses = Some(Pattern::nodes_where("kind", "use"));
    let defs = Some(Pattern::nodes_where("kind", "def"));
    db.register_view("resolve", uses.clone()).unwrap();
    db.register_view("unresolved", uses).unwrap();
    db.register_view("summary", None).unwrap();
    db.register_view("group", defs).unwrap();
    db
}

pub fn run_compiler(name: &str, key: Option<EntityId>, g: &mut ViewGraph) -> Result<(), String> {
    match (name, key) {
        ("resolve", Some(key)) => resolve(g, key),
        ("unresolved", Some(key)) => unresolved(g, key),
        ("summary", None) => summary(g),
        ("group", Some(key)) => group(g, key),
        _ => Err(format!("unknown view {name}")),
    }
}

/// Propagate with the compiler's views; no run may fail.
pub fn propagate(db: &mut Database) -> Report {
    let report = db.propagate_with(run_compiler).expect("no cycles");
    assert_eq!(report.failures, vec![], "views failed");
    test_utils::check_index_invariant(db.graph());
    report
}

/// `(use, def)` pairs linked by `resolve`.
pub fn resolutions(g: &Graph) -> BTreeSet<(Pointee, Pointee)> {
    g.iter_edges()
        .filter(|e| {
            g.obj(e)
                .is_some_and(|o| o.get("rel") == Some(&Field::from("resolves_to")))
        })
        .map(|e| {
            let edge = g.edge(&e).unwrap();
            (edge.source, edge.target)
        })
        .collect()
}

/// Uses that have a diagnostic.
pub fn diagnosed(g: &Graph) -> BTreeSet<Pointee> {
    g.nodes_where("kind", &"error".into())
        .flat_map(|diagnostic| g.out_edges(&diagnostic.into()).collect::<Vec<_>>())
        .map(|e| g.edge(&e).unwrap().target)
        .collect()
}

/// Which views ran, by name and key, ignoring the order.
pub fn ran(db: &Database, report: &Report) -> BTreeSet<(String, Option<EntityId>)> {
    report
        .runs
        .iter()
        .map(|inv| (db.view_name(inv.view).to_string(), inv.key))
        .collect()
}

pub fn run_of(name: &str, key: impl Into<Option<EntityId>>) -> (String, Option<EntityId>) {
    (name.to_string(), key.into())
}

pub type Content = (
    HashMap<EntityId, Object>,
    HashMap<EdgeId, (Pointee, Pointee)>,
    HashMap<HyperedgeId, HashSet<Pointee>>,
);

/// Structural content of a graph, for comparisons.
pub fn content(g: &Graph) -> Content {
    (g.entities.clone(), g.edges.clone(), g.hyperedges.clone())
}
