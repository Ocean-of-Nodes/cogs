use std::collections::{BTreeMap, BTreeSet};

use common::{Field, NodeId, Object, Pointee};
use db::{Graph, Report};
use jit::{Host, Runtime};

const RESOLVE_WASM: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/resolve_view/target/wasm32-unknown-unknown/release/resolve_view.wasm"
);

fn host() -> Host {
    let wasm_bytes = std::fs::read(RESOLVE_WASM).unwrap_or_else(|err| {
        panic!(
            "could not read {RESOLVE_WASM}: {err}\n\
             build the example first:\n  \
             (cd examples/resolve_view && cargo build --release)"
        )
    });
    let mut host = Runtime::new().host(Graph::default()).unwrap();
    host.load(&wasm_bytes).unwrap();
    host
}

fn node(fields: &[(&str, &str)]) -> Object {
    fields
        .iter()
        .map(|(name, value)| (name.to_string(), Field::from(*value)))
        .collect()
}

fn def(name: &str) -> Object {
    node(&[("kind", "def"), ("name", name)])
}

fn use_site(name: &str) -> Object {
    node(&[("kind", "use"), ("name", name)])
}

fn propagate(host: &mut Host) -> Report {
    let report = host.propagate().unwrap();
    assert_eq!(report.failures, vec![], "views failed");
    report
}

/// `(view, key)` of every run, ignoring the order.
fn runs(host: &Host, report: &Report) -> BTreeSet<(String, NodeId)> {
    report
        .runs
        .iter()
        .map(|inv| {
            let view = host.db().view_name(inv.view).to_string();
            (view, inv.key.expect("both views are keyed"))
        })
        .collect()
}

fn run(view: &str, key: NodeId) -> (String, NodeId) {
    (view.to_string(), key)
}

fn node_id(p: &Pointee) -> NodeId {
    match p {
        Pointee::EntityId(id) => *id,
        Pointee::Path(path) => panic!("unexpected path {path}"),
    }
}

/// `(use, def)` pairs linked by `resolve`.
fn resolutions(g: &Graph) -> BTreeSet<(NodeId, NodeId)> {
    g.iter_edges()
        .filter(|e| {
            g.obj(e)
                .is_some_and(|o| o["rel"] == Field::from("resolves_to"))
        })
        .map(|e| {
            let edge = g.edge(&e).unwrap();
            (node_id(&edge.source), node_id(&edge.target))
        })
        .collect()
}

/// Diagnostic message of every use that has one.
fn diagnostics(g: &Graph) -> BTreeMap<NodeId, Field> {
    g.nodes_where("kind", &"error".into())
        .map(|diagnostic| {
            let edge = g.out_edges(&diagnostic.into()).next().unwrap();
            let use_site = node_id(&g.edge(&edge).unwrap().target);
            (use_site, g.obj(&diagnostic).unwrap()["message"].clone())
        })
        .collect()
}

#[test]
fn name_resolution_follows_the_inputs() {
    let mut host = host();
    let d_foo = host.graph_mut().add_node(def("foo"));
    let u_foo = host.graph_mut().add_node(use_site("foo"));
    let u_bar = host.graph_mut().add_node(use_site("bar"));

    let report = propagate(&mut host);
    assert_eq!(report.runs.len(), 4, "two views for two uses");
    assert_eq!(resolutions(host.graph()), BTreeSet::from([(u_foo, d_foo)]));
    assert_eq!(
        diagnostics(host.graph()),
        BTreeMap::from([(u_bar, Field::from("unresolved name `bar`"))])
    );

    // Defining `bar` re-runs only what was about `bar`.
    let d_bar = host.graph_mut().add_node(def("bar"));
    let report = propagate(&mut host);
    assert_eq!(
        runs(&host, &report),
        BTreeSet::from([run("resolve", u_bar), run("unresolved", u_bar)])
    );
    assert_eq!(
        resolutions(host.graph()),
        BTreeSet::from([(u_foo, d_foo), (u_bar, d_bar)])
    );
    assert_eq!(diagnostics(host.graph()), BTreeMap::new());

    // Nobody reads `doc`: nothing runs.
    let documented = node(&[("kind", "def"), ("name", "foo"), ("doc", "the foo")]);
    host.graph_mut().replace_node(&d_foo, documented).unwrap();
    assert_eq!(propagate(&mut host).runs, vec![]);

    // Renaming the definition unlinks its use.
    let renamed = node(&[("kind", "def"), ("name", "baz"), ("doc", "the foo")]);
    host.graph_mut().replace_node(&d_foo, renamed).unwrap();
    let report = propagate(&mut host);
    assert_eq!(
        runs(&host, &report),
        BTreeSet::from([run("resolve", u_foo), run("unresolved", u_foo)])
    );
    assert_eq!(resolutions(host.graph()), BTreeSet::from([(u_bar, d_bar)]));
    assert_eq!(
        diagnostics(host.graph()),
        BTreeMap::from([(u_foo, Field::from("unresolved name `foo`"))])
    );

    // Undo reverts inputs; the views follow.
    host.graph_mut().undo().unwrap();
    host.graph_mut().undo().unwrap();
    propagate(&mut host);
    assert_eq!(
        resolutions(host.graph()),
        BTreeSet::from([(u_foo, d_foo), (u_bar, d_bar)])
    );
    assert_eq!(diagnostics(host.graph()), BTreeMap::new());

    // A removed use takes its invocations and their outputs along.
    host.graph_mut().remove_node(&u_bar).unwrap();
    propagate(&mut host);
    assert_eq!(resolutions(host.graph()), BTreeSet::from([(u_foo, d_foo)]));
    assert_eq!(host.db().invocations().len(), 2);
}
