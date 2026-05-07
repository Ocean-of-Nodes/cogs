use sdk::{Field, Graph, Object, view};

#[view]
fn hello_view(g: &mut Graph) {
    let mut obj = Object::new();
    obj.insert("Hello".to_string(), Field::String("World".to_string()));
    g.add_node(obj);
}

#[view]
fn farewell_view(g: &mut Graph) {
    let mut obj = Object::new();
    obj.insert("Goodbye".to_string(), Field::String("World".to_string()));
    g.add_node(obj);
}