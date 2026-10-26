use common::Field;
use jit::Runtime;
use db::Graph;

const HELLO_WASM: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/hello_view/target/wasm32-unknown-unknown/release/hello_view.wasm"
);

#[test]
fn views_create_nodes_in_graph() {
    let wasm_bytes = std::fs::read(HELLO_WASM).unwrap_or_else(|err| {
        panic!(
            "could not read {HELLO_WASM}: {err}\n\
             build the example first:\n  \
             (cd examples/hello_view && cargo build --release)"
        )
    });

    let runtime = Runtime::new();
    let graph = runtime.run_views(&wasm_bytes, Graph::default()).unwrap();

    assert_eq!(graph.iter_nodes().count(), 2, "expected one node per view");

    let mut payloads: Vec<(String, Field)> = graph
        .iter_nodes()
        .map(|id| {
            let obj = graph.obj(&id).expect("node carries an object");
            let (k, v) = obj.iter().next().expect("each view writes one field");
            (k.clone(), v.clone())
        })
        .collect();
    payloads.sort_by(|a, b| a.0.cmp(&b.0));

    let world = Field::String("World".to_string());
    assert_eq!(
        payloads,
        vec![
            ("Goodbye".to_string(), world.clone()),
            ("Hello".to_string(), world),
        ],
        "view payloads mismatch"
    );
}