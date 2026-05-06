use super::*;

#[test]
fn test_iter_hyperedge() {
    let (g, _n1, _n2, _n3, _n4, _e1, _e2, _e3, _e4, _e5, h) =
        test_utils::create_sample_graph1();

    let actual: Vec<_> = g.iter_hyperedges().collect();

    // No duplicates — `hyperedges` is a HashMap, but
    // assert anyway in case the impl chains in extra
    // sources later.
    let mut counts: HashMap<HyperedgeId, usize> = HashMap::new();
    for id in &actual {
        *counts.entry(*id).or_insert(0) += 1;
    }
    let duplicates: Vec<_> = counts
        .iter()
        .filter(|(_, c)| **c > 1)
        .map(|(e, c)| (*e, *c))
        .collect();
    if !duplicates.is_empty() {
        panic!("iter_hyperedges returned duplicates: {:?}", duplicates);
    }

    // Exactly the one hyperedge — nodes and edges must
    // NOT appear.
    let actual_set: HashSet<_> = actual.iter().copied().collect();
    let expected: HashSet<_> = [h].into_iter().collect();
    assert_eq!(actual_set, expected);
}
