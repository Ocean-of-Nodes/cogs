use common::{Field, Pointee};
use db::Graph;
use jit::Runtime;

const GRAPH_WASM: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/graph_view/target/wasm32-unknown-unknown/release/graph_view.wasm"
);

/// Every view in `graph_view` asserts its part of the `sdk::Graph`
/// API inside the guest, so a successful run means every intrinsic
/// round-tripped. `roundtrip_view` additionally leaves a report node
/// that is checked against host state here.
#[test]
fn guest_uses_every_graph_method() {
    let wasm_bytes = std::fs::read(GRAPH_WASM).unwrap_or_else(|err| {
        panic!(
            "could not read {GRAPH_WASM}: {err}\n\
             build the example first:\n  \
             (cd examples/graph_view && cargo build --release)"
        )
    });

    let graph = Runtime::new()
        .run_views(&wasm_bytes, Graph::default())
        .unwrap_or_else(|err| panic!("{err:?}"));

    let report = graph
        .iter_nodes()
        .filter_map(|id| graph.obj(&id))
        .find(|obj| obj.contains_key("echo"))
        .expect("report node");

    assert_eq!(report["echo"], Field::String("a".to_string()));
    assert_eq!(report["missing_obj"], Field::Bool(true));
    assert_eq!(report["missing_edge"], Field::Bool(true));

    // The edge id the guest got back is the one the host stored.
    let Field::Link(Pointee::EntityId(edge_id)) = &report["edge"] else {
        panic!("edge field is not a link: {:?}", report["edge"]);
    };
    let edge = graph
        .edge(edge_id)
        .expect("edge id returned to guest exists");
    let name_of = |p: &Pointee| match p {
        Pointee::EntityId(id) => graph.obj(id).map(|o| o["name"].clone()),
        Pointee::Path(_) => None,
    };
    assert_eq!(name_of(&edge.source), Some(Field::String("a".to_string())));
    assert_eq!(name_of(&edge.target), Some(Field::String("b".to_string())));
}
