use super::*;

fn value_of(g: &mut ViewGraph, by: &str) -> i128 {
    let Some(node) = g.nodes_where("by", &by.into()).first().copied() else {
        return 0;
    };
    match g.field(&node, "value") {
        Some(Field::Number(n)) => n,
        _ => 0,
    }
}

#[test]
fn views_feeding_each_other_are_a_cycle() {
    let mut db = Database::new(Graph::default());
    let p = db.register_view("p", None).unwrap();
    let q = db.register_view("q", None).unwrap();
    // p = q + 1, q = p + 1: no fixed point.
    let result = db.propagate_with(|name, _, g| {
        let other = if name == "p" { "q" } else { "p" };
        let value = value_of(g, other) + 1;
        g.add_node(obj(&[("by", name.into()), ("value", value.into())]));
        Ok(())
    });

    let Err(PropagateError::Cycle { runs }) = result else {
        panic!("expected a cycle, got {result:?}");
    };
    let p = InvocationId { view: p, key: None };
    let q = InvocationId { view: q, key: None };
    assert!(runs.contains(&p) && runs.contains(&q), "{runs:?}");
}

#[test]
fn view_creating_its_own_keys_is_a_cycle() {
    let mut db = Database::new(Graph::default());
    db.register_view("spawn", Some(Pattern::nodes_where("kind", "item")))
        .unwrap();
    db.graph_mut().add_node(obj(&[("kind", "item".into())]));
    let result = db.propagate_with(|_, _, g| {
        g.add_node(obj(&[("kind", "item".into())]));
        Ok(())
    });
    assert!(
        matches!(result, Err(PropagateError::Cycle { .. })),
        "{result:?}"
    );
}

#[test]
fn diamond_is_not_a_cycle() {
    // top → left, top → right, left + right → bottom. `bottom` may
    // run twice, but nothing comes back to its cause.
    let mut db = Database::new(Graph::default());
    for name in ["bottom", "left", "right", "top"] {
        db.register_view(name, None).unwrap();
    }
    let input = db.graph_mut().add_node(obj(&[("value", 1.into())]));
    let run = |name: &str, _: Option<EntityId>, g: &mut ViewGraph| {
        let value = match name {
            "top" => match g.field(&input, "value") {
                Some(Field::Number(n)) => n,
                _ => 0,
            },
            "left" | "right" => value_of(g, "top") * 10,
            "bottom" => value_of(g, "left") + value_of(g, "right"),
            _ => unreachable!(),
        };
        g.add_node(obj(&[("by", name.into()), ("value", value.into())]));
        Ok(())
    };

    db.propagate_with(run).unwrap();
    db.graph_mut()
        .replace_node(&input, obj(&[("value", 2.into())]))
        .unwrap();
    db.propagate_with(run).unwrap();

    let g = db.graph();
    let bottom = g.nodes_where("by", &"bottom".into()).next().unwrap();
    assert_eq!(g.obj(&bottom).unwrap()["value"], Field::Number(40));
}
