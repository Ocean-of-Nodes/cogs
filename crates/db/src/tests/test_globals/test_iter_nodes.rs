use super::*;

#[test]
fn iter_nodes() {
    let (g, n1, n2, n3, n4, _e1, _e2, _e3, _e4, _e5, _h) =
        test_utils::create_sample_graph1();

    let actual: Vec<_> = g.iter_nodes().collect();

    // No duplicates — entities is a HashMap, but assert
    // anyway in case the impl chains in extra sources later.
    let mut counts: HashMap<NodeId, usize> = HashMap::new();
    for id in &actual {
        *counts.entry(*id).or_insert(0) += 1;
    }
    let duplicates: Vec<_> = counts
        .iter()
        .filter(|(_, c)| **c > 1)
        .map(|(e, c)| (*e, *c))
        .collect();
    if !duplicates.is_empty() {
        panic!("iter_nodes returned duplicates: {:?}", duplicates);
    }

    // Exactly the four node ids — edges (e1..e5) and the
    // hyperedge `h` must NOT appear.
    let actual_set: HashSet<_> = actual.iter().copied().collect();
    let expected: HashSet<_> = [n1, n2, n3, n4].into_iter().collect();
    assert_eq!(actual_set, expected);
}
