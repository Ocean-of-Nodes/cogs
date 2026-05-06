use super::*;

/// Iterator yields nodes, edges, and hyperedges all together.
#[test]
fn yields_all_entity_kinds() {
    let (g, n1, n2, n3, n4, e1, e2, e3, e4, e5, h) = test_utils::create_sample_graph1();

    let mut expected = HashSet::new();
    expected.insert(n1);
    expected.insert(n2);
    expected.insert(n3);
    expected.insert(n4);
    expected.insert(e1);
    expected.insert(e2);
    expected.insert(e3);
    expected.insert(e4);
    expected.insert(e5);
    expected.insert(h);

    let actual: Vec<_> = g.iter_entities().collect();

    // 1. No duplicates: every entity appears at most once.
    let mut counts: HashMap<EntityId, usize> = HashMap::new();
    for e in &actual {
        *counts.entry(*e).or_insert(0) += 1;
    }
    let duplicates: Vec<_> = counts
        .iter()
        .filter(|(_, c)| **c > 1)
        .map(|(e, c)| (*e, *c))
        .collect();
    if !duplicates.is_empty() {
        panic!("global_entities returned duplicates: {:?}", duplicates);
    }

    // 2. Coverage matches exactly: no missing, no extras.
    let actual_set: HashSet<_> = actual.iter().copied().collect();
    let missing: Vec<_> = expected.difference(&actual_set).copied().collect();
    let unexpected: Vec<_> = actual_set.difference(&expected).copied().collect();
    if !missing.is_empty() || !unexpected.is_empty() {
        panic!(
            "global_entities mismatch — missing: {:?}, unexpected: {:?}",
            missing, unexpected
        );
    }
}

/// Verify deduplication: an id that lives in more than one
/// storage map must be yielded by `iter_entities` only
/// once.
///
/// Setup creates two cross-map collisions on purpose:
/// - `e1` lives as an edge *and*, after `attach_obj`, as
///   an entity (object attached to that edge).
/// - `h`  lives as a hyperedge *and*, after `attach_obj`,
///   as an entity.
///
/// If `iter_entities` ever stops deduplicating (e.g.
/// someone removes the trailing `collect::<HashSet<_>>()`),
/// `e1` and `h` would each show up twice and this test
/// catches it.
#[test]
fn deduplicates_attached_target_ids() {
    let mut g = Graph::default();
    let obj = test_utils::create_simple_obj("test_field");
    let n1 = g.add_node(obj.clone());
    let n2 = g.add_node(obj.clone());

    let e1 = g.add_edge(n1, n2).unwrap();
    g.attach_obj(e1, obj.clone()).unwrap();

    let mut m = HashSet::new();
    m.insert(n1.into());
    m.insert(n2.into());

    let h = g.create_hyperedge(m).unwrap();
    g.attach_obj(h, obj.clone()).unwrap();

    let actual: Vec<_> = g.iter_entities().collect();

    // 1. No id appears more than once — in particular,
    //    e1 and h, which sit in two maps each.
    let mut counts: HashMap<EntityId, usize> = HashMap::new();
    for id in &actual {
        *counts.entry(*id).or_insert(0) += 1;
    }
    assert_eq!(
        counts.get(&e1).copied(),
        Some(1),
        "e1 yielded {:?} times, expected 1",
        counts.get(&e1)
    );
    assert_eq!(
        counts.get(&h).copied(),
        Some(1),
        "h yielded {:?} times, expected 1",
        counts.get(&h)
    );

    // 2. Coverage: exactly the four distinct ids.
    let actual_set: HashSet<_> = actual.iter().copied().collect();
    let expected: HashSet<_> = [n1, n2, e1, h].into_iter().collect();
    assert_eq!(actual_set, expected);
}
