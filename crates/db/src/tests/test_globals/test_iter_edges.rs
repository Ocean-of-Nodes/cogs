use super::*;

#[test]
fn yields_all_edges_in_sample_graph() {
    let (g, _n1, _n2, _n3, _n4, e1, e2, e3, e4, e5, _h) =
        test_utils::create_sample_graph1();

    let actual: Vec<_> = g.iter_edges().collect();

    // No duplicates (HashMap keys can't repeat, but we
    // assert anyway in case the impl changes).
    let mut counts: HashMap<EdgeID, usize> = HashMap::new();
    for id in &actual {
        *counts.entry(*id).or_insert(0) += 1;
    }
    let duplicates: Vec<_> = counts
        .iter()
        .filter(|(_, c)| **c > 1)
        .map(|(e, c)| (*e, *c))
        .collect();
    if !duplicates.is_empty() {
        panic!("iter_edges returned duplicates: {:?}", duplicates);
    }

    // Exactly the five edges added (e1..e5). Nodes and
    // the hyperedge `h` must NOT appear.
    let actual_set: HashSet<_> = actual.iter().copied().collect();
    let expected: HashSet<_> = [e1, e2, e3, e4, e5].into_iter().collect();
    assert_eq!(actual_set, expected);
}
