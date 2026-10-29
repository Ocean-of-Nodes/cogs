//! Name resolution as incremental views.
//!
//! Inputs are nodes `{kind: "def" | "use", name}`. `resolve` links
//! every use to the definitions with its name; `unresolved` reports
//! the uses that `resolve` could not link. Change a name and only the
//! affected uses run again.

use sdk::{Field, Graph, NodeId, Object, Pattern, view};

fn obj(fields: &[(&str, Field)]) -> Object {
    fields
        .iter()
        .map(|(name, value)| (name.to_string(), value.clone()))
        .collect()
}

/// Link every use site to the definitions with the same name.
#[view(for_each = Pattern::nodes_where("kind", "use"))]
fn resolve(g: &mut Graph, use_site: NodeId) {
    let Some(name) = g.field(use_site, "name") else {
        return;
    };
    for def in g.nodes_where("name", name) {
        if g.field(def, "kind") == Some(Field::from("def")) {
            let edge = g.add_edge(use_site, def).unwrap();
            g.attach_obj(edge, obj(&[("rel", "resolves_to".into())]))
                .unwrap();
        }
    }
}

/// A diagnostic for every use that resolves to nothing.
#[view(for_each = Pattern::nodes_where("kind", "use"))]
fn unresolved(g: &mut Graph, use_site: NodeId) {
    let resolves_to = Some(Field::from("resolves_to"));
    if g.out_edges(use_site)
        .into_iter()
        .any(|edge| g.field(edge, "rel") == resolves_to)
    {
        return;
    }
    let message = match g.field(use_site, "name") {
        Some(Field::String(name)) => format!("unresolved name `{name}`"),
        _ => "use without a name".to_string(),
    };
    let diagnostic = g.add_node(obj(&[
        ("kind", "error".into()),
        ("message", message.into()),
    ]));
    g.add_edge(diagnostic, use_site).unwrap();
}
