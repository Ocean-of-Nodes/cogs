use super::*;

/// Adding a node returns an id and the object can be looked up.
#[test]
fn add_then_lookup() {
    let mut graph = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let node_id = graph.add_node(obj.clone());
    assert_eq!(graph.obj(&node_id), Some(&obj));
}

/// Re-inserting under the same id is rejected and the
/// original object stays untouched.
#[test]
fn rejects_duplicate_id() {
    let mut graph = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = graph.add_node(obj.clone());
    let obj2 = test_utils::create_simple_obj("test_field2");
    let result2 = graph.silent_add_node_with_id(n1, obj2.clone());
    assert_eq!(
        result2.clone().unwrap_err(),
        NodeAlreadyExistsError {
            id: result2.unwrap_err().id
        }
    );
    // Check thats change doesnt apply
    assert_eq!(graph.obj(&n1), Some(&obj))
}
